use super::{Store, StoreError, StoreResult};
use crate::connection_refresh::{
    ObservationValidity, RefreshConfiguration, RefreshSettings, binding_scope,
};
use crate::connections::{ConnectionChange, ConnectionDiagnostic, ConnectionLink, now_ms};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde_json::{Value, json};

pub(super) const MIGRATION_V8: &str = r#"
CREATE TABLE connection_links(id TEXT NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),body TEXT NOT NULL,digest TEXT NOT NULL,PRIMARY KEY(id,revision));
CREATE TABLE connection_selection(singleton INTEGER PRIMARY KEY CHECK(singleton=1),id TEXT);
INSERT INTO connection_selection VALUES(1,NULL);
CREATE TABLE connection_diagnostic_runs(id TEXT NOT NULL,ordinal INTEGER NOT NULL CHECK(ordinal>0),connection_id TEXT NOT NULL,connection_revision INTEGER NOT NULL,registration_revision INTEGER NOT NULL,body TEXT NOT NULL,digest TEXT NOT NULL,PRIMARY KEY(id,ordinal),FOREIGN KEY(connection_id,registration_revision) REFERENCES connection_links(id,revision));
CREATE INDEX connection_diagnostics_scope ON connection_diagnostic_runs(connection_id,connection_revision,id,ordinal);
CREATE TABLE connection_commands(id TEXT PRIMARY KEY,fingerprint TEXT NOT NULL,result TEXT NOT NULL);
CREATE TRIGGER connection_links_no_update BEFORE UPDATE ON connection_links BEGIN SELECT RAISE(ABORT,'immutable connection revision'); END;
CREATE TRIGGER connection_links_no_delete BEFORE DELETE ON connection_links BEGIN SELECT RAISE(ABORT,'retained connection history'); END;
CREATE TRIGGER connection_diagnostic_no_update BEFORE UPDATE ON connection_diagnostic_runs BEGIN SELECT RAISE(ABORT,'append-only connection diagnostic'); END;
CREATE TRIGGER connection_diagnostic_no_delete BEFORE DELETE ON connection_diagnostic_runs BEGIN SELECT RAISE(ABORT,'retained connection diagnostic'); END;
CREATE TRIGGER connection_commands_no_update BEFORE UPDATE ON connection_commands BEGIN SELECT RAISE(ABORT,'immutable connection command'); END;
CREATE TRIGGER connection_commands_no_delete BEFORE DELETE ON connection_commands BEGIN SELECT RAISE(ABORT,'retained connection command'); END;
"#;
pub(super) const MIGRATION_V9: &str = r#"
CREATE TABLE connection_refresh_configurations(connection_id TEXT NOT NULL,revision INTEGER NOT NULL CHECK(revision>0),body TEXT NOT NULL,digest TEXT NOT NULL,PRIMARY KEY(connection_id,revision));
CREATE TRIGGER connection_refresh_no_update BEFORE UPDATE ON connection_refresh_configurations BEGIN SELECT RAISE(ABORT,'immutable refresh configuration'); END;
CREATE TRIGGER connection_refresh_no_delete BEFORE DELETE ON connection_refresh_configurations BEGIN SELECT RAISE(ABORT,'retained refresh configuration'); END;
CREATE TABLE connection_diagnostic_validity(id TEXT PRIMARY KEY,refresh_revision INTEGER NOT NULL,policy_version INTEGER NOT NULL,body TEXT NOT NULL,digest TEXT NOT NULL);
CREATE TRIGGER connection_validity_no_update BEFORE UPDATE ON connection_diagnostic_validity BEGIN SELECT RAISE(ABORT,'immutable diagnostic validity'); END;
CREATE TRIGGER connection_validity_no_delete BEFORE DELETE ON connection_diagnostic_validity BEGIN SELECT RAISE(ABORT,'retained diagnostic validity'); END;
"#;
fn refresh_configuration(c: &Connection, id: &str) -> StoreResult<Option<RefreshConfiguration>> {
    crate::domain::ConnectionId::new(id)?;
    let row: Option<(i64,String,String)> = c.query_row(
        "SELECT revision,body,digest FROM connection_refresh_configurations WHERE connection_id=?1 ORDER BY revision DESC LIMIT 1",
        [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    row.map(|(revision, body, hash)| {
        if body.len() > 8192 || digest(&body) != hash {
            return Err(StoreError::InvalidState);
        }
        let config: RefreshConfiguration =
            serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
        config.validate().map_err(|_| StoreError::InvalidState)?;
        if config.connection_id != id || config.revision != revision as u64 {
            return Err(StoreError::InvalidState);
        }
        Ok(config)
    })
    .transpose()
}
fn policy_validity(c: &Connection) -> StoreResult<ObservationValidity> {
    // Compact authoritative configuration projection, not full worker/records restoration.
    let body:Option<String>=c.query_row("SELECT body FROM definitions WHERE kind='\"configuration\"' AND id='desktop-settings' ORDER BY version DESC LIMIT 1",[],|r|r.get(0)).optional()?;
    let settings: crate::desktop::Settings = body
        .map(|s| serde_json::from_str(&s))
        .transpose()
        .map_err(|_| StoreError::InvalidState)?
        .unwrap_or_default();
    if !settings.validate() {
        return Err(StoreError::InvalidState);
    }
    let ttl = settings.policy.observation_ttls;
    Ok(ObservationValidity {
        identity: ttl.identity_billing_ms,
        availability: ttl.availability_ms,
        configuration: ttl.capability_configuration_ms,
        models: ttl.capability_configuration_ms,
    })
}
pub(super) fn effective_validity(c: &Connection, id: &str) -> StoreResult<ObservationValidity> {
    let requested =
        refresh_configuration(c, id)?.map_or_else(RefreshSettings::default, |v| v.settings);
    Ok(requested.validity_ms.effective(&policy_validity(c)?))
}
fn minimum_validity(a: ObservationValidity, b: &ObservationValidity) -> ObservationValidity {
    ObservationValidity {
        identity: a.identity.min(b.identity),
        configuration: a.configuration.min(b.configuration),
        availability: a.availability.min(b.availability),
        models: a.models.min(b.models),
    }
}
/// Lease restrictions at capture and all later configuration revisions only
/// tighten that observation. Increasing a preference applies to a fresh read.
fn diagnostic_validity_cap(
    c: &Connection,
    d: &ConnectionDiagnostic,
) -> StoreResult<ObservationValidity> {
    let mut result = effective_validity(c, &d.connection_id)?;
    let cap:Option<(i64,i64,String,String)>=c.query_row("SELECT refresh_revision,policy_version,body,digest FROM connection_diagnostic_validity WHERE id=?1",[&d.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
    let (refresh_revision, policy_version) = if let Some((refresh, policy, body, hash)) = cap {
        if digest(&body) != hash {
            return Err(StoreError::InvalidState);
        }
        let captured: ObservationValidity =
            serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
        captured.validate().map_err(|_| StoreError::InvalidState)?;
        result = minimum_validity(result, &captured);
        (refresh, policy)
    } else {
        (0, 0)
    };
    let mut refreshes=c.prepare("SELECT body,digest FROM connection_refresh_configurations WHERE connection_id=?1 AND revision>=?2 ORDER BY revision")?;
    for row in refreshes.query_map(params![d.connection_id, refresh_revision], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })? {
        let (body, hash) = row?;
        if digest(&body) != hash {
            return Err(StoreError::InvalidState);
        }
        let config: RefreshConfiguration =
            serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
        config.validate().map_err(|_| StoreError::InvalidState)?;
        result = minimum_validity(result, &config.settings.validity_ms);
    }
    let mut policies=c.prepare("SELECT body FROM definitions WHERE kind='\"configuration\"' AND id='desktop-settings' AND version>=?1 ORDER BY version")?;
    for row in policies.query_map([policy_version], |r| r.get::<_, String>(0))? {
        let settings: crate::desktop::Settings =
            serde_json::from_str(&row?).map_err(|_| StoreError::InvalidState)?;
        if !settings.validate() {
            return Err(StoreError::InvalidState);
        }
        let p = settings.policy.observation_ttls;
        result = minimum_validity(
            result,
            &ObservationValidity {
                identity: p.identity_billing_ms,
                availability: p.availability_ms,
                configuration: p.capability_configuration_ms,
                models: p.capability_configuration_ms,
            },
        );
    }
    Ok(result)
}
pub(super) fn diagnostic_effective_validity(
    c: &Connection,
    d: &ConnectionDiagnostic,
) -> StoreResult<ObservationValidity> {
    let mut result = diagnostic_validity_cap(c, d)?;
    if d.cached && d.observed_at_ms.is_some() {
        for prior in diagnostics(c, &d.connection_id)?.iter().filter(|prior| {
            prior.id != d.id
                && prior.observed_at_ms == d.observed_at_ms
                && prior.scope_key == d.scope_key
                && prior.connection_revision == d.connection_revision
                && matches!(prior.state.as_str(), "complete" | "partial")
        }) {
            result = minimum_validity(result, &diagnostic_validity_cap(c, prior)?);
        }
    }
    Ok(result)
}
fn encode<T: serde::Serialize>(v: &T) -> StoreResult<String> {
    serde_json::to_string(v).map_err(|_| StoreError::InvalidState)
}
fn digest(s: &str) -> String {
    crate::git_records::content_hash(s.as_bytes())
}
fn decode_link(
    body: String,
    expected_hash: String,
    id: &str,
    revision: i64,
) -> StoreResult<ConnectionLink> {
    if body.len() > 32768 || digest(&body) != expected_hash {
        return Err(StoreError::InvalidState);
    }
    let v: ConnectionLink = serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
    v.validate().map_err(|_| StoreError::InvalidState)?;
    if v.id != id || v.revision != revision as u64 {
        return Err(StoreError::InvalidState);
    }
    Ok(v)
}
pub(super) fn current(c: &Connection, id: &str) -> StoreResult<Option<ConnectionLink>> {
    crate::domain::ConnectionId::new(id)?;
    let row:Option<(i64,String,String)>=c.query_row("SELECT revision,body,digest FROM connection_links WHERE id=? ORDER BY revision DESC LIMIT 1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional()?;
    let mut link = row.map(|(r, b, h)| decode_link(b, h, id, r)).transpose()?;
    if let Some(l) = &mut link
        && l.baseline_observation.is_none()
        && let Some(d) = diagnostics(c, id)?
            .into_iter()
            .filter(|d| {
                d.connection_revision == l.binding_revision
                    && matches!(d.state.as_str(), "complete" | "partial")
            })
            .min_by_key(|d| d.observed_at_ms)
    {
        l.baseline_observation = d.observation_fingerprint;
        l.identity_assurance = d.identity_assurance;
    }
    Ok(link)
}
fn auth_store(v: &ConnectionLink) -> String {
    let home = match &v.runtime {
        crate::daemon::RuntimeChoice::Codex { home, .. }
        | crate::daemon::RuntimeChoice::Synthetic { home, .. } => home,
    };
    home.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('/', r"\")
        .trim_end_matches('\\')
        .to_lowercase()
}
fn duplicate_auth_store(c: &Connection, v: &ConnectionLink) -> StoreResult<Option<String>> {
    let mut q=c.prepare("SELECT l.id,l.revision,l.body,l.digest FROM connection_links l WHERE l.id!=?1 AND NOT EXISTS(SELECT 1 FROM connection_links n WHERE n.id=l.id AND n.revision>l.revision) ORDER BY l.id LIMIT 65")?;
    let mut count = 0;
    for row in q.query_map([&v.id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
        ))
    })? {
        count += 1;
        if count > 64 {
            return Err(StoreError::Configuration);
        }
        let (id, revision, body, hash) = row?;
        let old = decode_link(body, hash, &id, revision)?;
        if old.linked && auth_store(&old) == auth_store(v) {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

fn insert_link(c: &Connection, v: &ConnectionLink) -> StoreResult<()> {
    v.validate().map_err(|_| StoreError::InvalidState)?;
    let b = encode(v)?;
    c.execute(
        "INSERT INTO connection_links(id,revision,body,digest) VALUES(?1,?2,?3,?4)",
        params![v.id, v.revision as i64, b, digest(&b)],
    )?;
    c.execute("INSERT INTO daemon_events(kind,body) VALUES('connection-changed',?1)",[json!({"connectionId":v.id,"connectionRevision":v.revision,"enabled":v.enabled,"linked":v.linked}).to_string()])?;
    Ok(())
}
fn command(c: &Connection, id: &str, fingerprint: &str) -> StoreResult<Option<Value>> {
    crate::domain::LocalRef::new(id)?;
    if !crate::connections::hash(fingerprint) {
        return Err(StoreError::InvalidState);
    }
    let row: Option<(String, String)> = c
        .query_row(
            "SELECT fingerprint,result FROM connection_commands WHERE id=?",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    row.map(|(f, v)| {
        if f != fingerprint {
            Err(StoreError::IdempotencyConflict)
        } else {
            serde_json::from_str(&v).map_err(|_| StoreError::InvalidState)
        }
    })
    .transpose()
}
fn save_command(c: &Connection, id: &str, fingerprint: &str, result: &Value) -> StoreResult<()> {
    crate::domain::LocalRef::new(id)?;
    if !crate::connections::hash(fingerprint) {
        return Err(StoreError::InvalidState);
    }
    c.execute(
        "INSERT INTO connection_commands(id,fingerprint,result) VALUES(?1,?2,?3)",
        params![id, fingerprint, encode(result)?],
    )?;
    Ok(())
}
pub(super) fn diagnostics(c: &Connection, id: &str) -> StoreResult<Vec<ConnectionDiagnostic>> {
    crate::domain::ConnectionId::new(id)?;
    let mut q=c.prepare("SELECT id,ordinal,connection_revision,registration_revision,body,digest FROM connection_diagnostic_runs WHERE connection_id=?1 ORDER BY id,ordinal")?;
    let mut rows: Vec<ConnectionDiagnostic> = Vec::new();
    let mut previous: Option<(String, i64)> = None;
    for row in q.query_map([id], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, String>(5)?,
        ))
    })? {
        let (run, ordinal, revision, registration, body, hash) = row?;
        if rows.len() >= 512
            || body.len() > 262144
            || digest(&body) != hash
            || ordinal
                != previous
                    .as_ref()
                    .filter(|(p, _)| p == &run)
                    .map_or(1, |(_, o)| o + 1)
        {
            return Err(StoreError::InvalidState);
        }
        let v: ConnectionDiagnostic =
            serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
        v.validate().map_err(|_| StoreError::InvalidState)?;
        if v.id != run
            || v.connection_id != id
            || v.connection_revision != revision as u64
            || v.registration_revision != registration as u64
        {
            return Err(StoreError::InvalidState);
        }
        if ordinal == 1 {
            if v.state != "running" || v.finished_at_ms.is_some() {
                return Err(StoreError::InvalidState);
            }
        } else {
            let p = rows.last().ok_or(StoreError::InvalidState)?;
            if p.state != "running"
                || v.state == "running"
                || v.started_at_ms != p.started_at_ms
                || v.connection_revision != p.connection_revision
                || v.registration_revision != p.registration_revision
                || v.synthetic != p.synthetic
                || v.daemon_generation != p.daemon_generation
                || v.purpose != p.purpose
                || v.finished_at_ms.is_none()
            {
                return Err(StoreError::InvalidState);
            }
        }
        previous = Some((run, ordinal));
        rows.push(v);
    }
    Ok(rows)
}
fn summary(
    d: &ConnectionDiagnostic,
    now: u64,
    current_revision: Option<u64>,
    current_generation: u64,
    validity: &ObservationValidity,
) -> Value {
    let mut v = d.view(now, current_generation);
    let effective_until = d
        .observed_at_ms
        .map(|at| at.saturating_add(validity.minimum()))
        .zip(d.valid_until_ms)
        .map(|(configured, source)| configured.min(source));
    v["effectiveValidUntilMs"] = json!(effective_until);
    v["effectiveValidityMs"] = json!(validity);
    v["sourceValidityAuthority"] = json!("metadata-source-upper-bound");
    if effective_until.is_none_or(|until| now >= until) {
        v["current"] = json!(false);
    }
    if current_revision != Some(d.connection_revision) {
        v["current"] = json!(false);
        v["historicalRevision"] = json!(true);
    }
    v.as_object_mut().expect("struct").remove("descriptor");
    v.as_object_mut().expect("struct").remove("catalogLabels");
    v.as_object_mut()
        .expect("struct")
        .remove("controlObservations");
    v.as_object_mut()
        .expect("struct")
        .remove("structuredResultSupport");
    v
}
fn report(c: &Connection, id: &str, detailed: bool, now: u64) -> StoreResult<Value> {
    let current_revision = current(c, id)?.map(|l| l.binding_revision);
    let current_generation = c.query_row(
        "SELECT generation FROM daemon_meta WHERE singleton=1",
        [],
        |r| r.get::<_, i64>(0),
    )? as u64;
    let rows = diagnostics(c, id)?;
    let latest = rows
        .iter()
        .max_by_key(|d| (d.started_at_ms, d.finished_at_ms));
    let successful = rows
        .iter()
        .filter(|d| matches!(d.state.as_str(), "complete" | "partial"))
        .max_by_key(|d| d.observed_at_ms);
    let history = rows
        .iter()
        .filter(|d| d.finished_at_ms.is_some())
        .rev()
        .take(if detailed { 16 } else { 0 })
        .map(|d| {
            Ok(summary(
                d,
                now,
                current_revision,
                current_generation,
                &diagnostic_effective_validity(c, d)?,
            ))
        })
        .collect::<StoreResult<Vec<_>>>()?;
    let latest_summary = latest
        .map(|d| -> StoreResult<Value> {
            Ok(summary(
                d,
                now,
                current_revision,
                current_generation,
                &diagnostic_effective_validity(c, d)?,
            ))
        })
        .transpose()?;
    let successful_summary = successful
        .map(|d| -> StoreResult<Value> {
            Ok(summary(
                d,
                now,
                current_revision,
                current_generation,
                &diagnostic_effective_validity(c, d)?,
            ))
        })
        .transpose()?;
    let mut value = json!({"latest":latest_summary,"lastSuccessful":successful_summary,"lastSuccessfulAtMs":successful.and_then(|d|d.observed_at_ms),"identityChanged":rows.iter().any(|d|d.identity_changed&&current_revision==Some(d.connection_revision)),"history":history,"historyLimit":if detailed{16}else{0},"historyHasMore":rows.iter().filter(|d|d.finished_at_ms.is_some()).count()>if detailed{16}else{0},"observationRenewedOnRead":false});
    if detailed {
        value["catalog"] = successful
            .and_then(|d| d.descriptor.clone())
            .unwrap_or(Value::Null);
        value["catalogLabels"] = json!(successful.map(|d| &d.catalog_labels));
        value["controlObservations"] = json!(successful.map(|d| &d.control_observations));
        value["structuredResultSupport"] =
            json!(successful.and_then(|d| d.structured_result_support));
        value["controlEvidence"] = json!({"source":"configuration-backed-pinned-profile","effectiveTurnSettingsConfirmed":false});
        value["catalogDiagnosticId"] = json!(successful.map(|d| &d.id));
        value["catalogObservedAtMs"] = json!(successful.and_then(|d| d.observed_at_ms));
    }
    Ok(value)
}
impl Store {
    pub fn connection_refresh_configuration(
        &self,
        id: &str,
    ) -> StoreResult<Option<RefreshConfiguration>> {
        refresh_configuration(&self.connection, id)
    }
    pub(crate) fn connection_effective_validity(
        &self,
        id: &str,
    ) -> StoreResult<ObservationValidity> {
        effective_validity(&self.connection, id)
    }
    pub(crate) fn connection_read_maximum_age(&self, id: &str) -> StoreResult<u64> {
        let current = effective_validity(&self.connection, id)?.minimum();
        let previous = diagnostics(&self.connection, id)?
            .into_iter()
            .filter(|d| matches!(d.state.as_str(), "complete" | "partial"))
            .max_by_key(|d| d.observed_at_ms);
        Ok(if let Some(previous) = previous {
            current.min(diagnostic_effective_validity(&self.connection, &previous)?.minimum())
        } else {
            current
        })
    }
    pub(crate) fn connection_policy_validity(&self) -> StoreResult<ObservationValidity> {
        policy_validity(&self.connection)
    }
    pub(crate) fn connection_diagnostic_remaining(&self, id: &str) -> StoreResult<u64> {
        let used: i64 = self.connection.query_row(
            "SELECT count(*) FROM connection_diagnostic_runs WHERE connection_id=?1",
            [id],
            |r| r.get(0),
        )?;
        Ok(512_u64.saturating_sub(used as u64))
    }
    pub(crate) fn connection_links_current(&self) -> StoreResult<Vec<ConnectionLink>> {
        let mut q = self
            .connection
            .prepare("SELECT id FROM connection_links GROUP BY id ORDER BY id LIMIT 65")?;
        let ids = q
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        if ids.len() > 64 {
            return Err(StoreError::Configuration);
        }
        ids.iter()
            .map(|id| current(&self.connection, id)?.ok_or(StoreError::InvalidState))
            .collect()
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn connection_configure_refresh(
        &mut self,
        key: &str,
        fingerprint: &str,
        id: &str,
        expected_connection: u64,
        expected_refresh: u64,
        settings: &RefreshSettings,
        now: u64,
    ) -> StoreResult<Value> {
        settings.validate().map_err(|_| StoreError::InvalidState)?;
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(result) = command(&tx, key, fingerprint)? {
            return Ok(result);
        }
        let link = current(&tx, id)?.ok_or(StoreError::InvalidState)?;
        if link.revision != expected_connection
            || refresh_configuration(&tx, id)?.map_or(0, |v| v.revision) != expected_refresh
        {
            return Err(StoreError::StaleRevision);
        }
        if matches!(
            settings.mode,
            crate::connection_refresh::RefreshMode::Automatic
        ) {
            link.guard(expected_connection)
                .map_err(|_| StoreError::InvalidState)?;
            if link.baseline_observation.is_none()
                || diagnostics(&tx, id)?
                    .iter()
                    .any(|d| d.connection_revision == link.binding_revision && d.identity_changed)
            {
                return Err(StoreError::InvalidState);
            }
        }
        let config = RefreshConfiguration {
            schema_version: 1,
            connection_id: id.into(),
            revision: expected_refresh
                .checked_add(1)
                .ok_or(StoreError::InvalidState)?,
            binding_revision: link.binding_revision,
            scope_key: binding_scope(&link),
            settings: settings.clone(),
            configured_at_ms: now,
        };
        config.validate().map_err(|_| StoreError::InvalidState)?;
        let body = encode(&config)?;
        tx.execute(
            "INSERT INTO connection_refresh_configurations VALUES(?1,?2,?3,?4)",
            params![id, config.revision as i64, body, digest(&body)],
        )?;
        let result = json!({"connectionId":id,"connectionRevision":link.revision,"bindingRevision":link.binding_revision,"refreshRevision":config.revision,"settings":settings,"automaticReadStarted":false});
        tx.execute(
            "INSERT INTO daemon_events(kind,body) VALUES('connection-refresh-configured',?1)",
            [json!({"connectionId":id,"refreshRevision":config.revision}).to_string()],
        )?;
        save_command(&tx, key, fingerprint, &result)?;
        tx.commit()?;
        Ok(result)
    }
    pub(crate) fn connection_duplicate_auth_store(
        &self,
        v: &ConnectionLink,
    ) -> StoreResult<Option<String>> {
        duplicate_auth_store(&self.connection, v)
    }
    pub(crate) fn connection_registry_managed(&self) -> StoreResult<bool> {
        Ok(self
            .connection
            .query_row("SELECT EXISTS(SELECT 1 FROM connection_links)", [], |r| {
                r.get(0)
            })?)
    }
    pub(crate) fn connection_next_binding(&self, id: &str) -> StoreResult<u64> {
        crate::domain::ConnectionId::new(id)?;
        let n: i64 = self.connection.query_row(
            "SELECT coalesce(max(revision),0) FROM connections WHERE id=?1",
            [id],
            |r| r.get(0),
        )?;
        Ok((n as u64).saturating_add(1))
    }
    pub fn connection_link(&self, id: &str) -> StoreResult<Option<ConnectionLink>> {
        current(&self.connection, id)
    }
    pub fn connection_list(&self) -> StoreResult<Value> {
        self.connection_list_at(now_ms())
    }
    pub(crate) fn connection_list_at(&self, now: u64) -> StoreResult<Value> {
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = (|| {
            let selected: Option<String> = self.connection.query_row(
                "SELECT id FROM connection_selection WHERE singleton=1",
                [],
                |r| r.get(0),
            )?;
            let mut q = self.connection.prepare(
                "SELECT id,max(revision) FROM connection_links GROUP BY id ORDER BY id LIMIT 65",
            )?;
            let ids = q
                .query_map([], |r| r.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            if ids.len() > 64 {
                return Err(StoreError::Configuration);
            }
            let mut views = Vec::new();
            for id in ids {
                let l = current(&self.connection, &id)?.ok_or(StoreError::InvalidState)?;
                views.push(l.view(
                    selected.as_deref() == Some(&id),
                    report(&self.connection, &id, false, now)?,
                ));
            }
            Ok(json!({"connections":views,"selectedConnectionId":selected}))
        })();
        self.connection.execute_batch("ROLLBACK")?;
        result
    }
    pub(crate) fn connection_identity_changed(&self, id: &str, revision: u64) -> StoreResult<bool> {
        Ok(diagnostics(&self.connection, id)?
            .iter()
            .any(|d| d.connection_revision == revision && d.identity_changed))
    }
    pub fn connection_diagnostics(&self, id: &str) -> StoreResult<Value> {
        self.connection_diagnostics_run(id, None)
    }
    pub fn connection_diagnostics_run(&self, id: &str, run: Option<&str>) -> StoreResult<Value> {
        self.connection_diagnostics_run_at(id, run, now_ms())
    }
    pub(crate) fn connection_diagnostics_run_at(
        &self,
        id: &str,
        run: Option<&str>,
        now: u64,
    ) -> StoreResult<Value> {
        if let Some(run) = run {
            crate::domain::LocalRef::new(run)?;
            let rows = diagnostics(&self.connection, id)?;
            let d = rows
                .iter()
                .rev()
                .find(|d| d.id == run)
                .ok_or(StoreError::InvalidState)?;
            let current_revision = current(&self.connection, id)?.map(|l| l.binding_revision);
            let mut v = report(&self.connection, id, false, now)?;
            v["latest"] = summary(
                d,
                now,
                current_revision,
                self.daemon_state()?.generation,
                &diagnostic_effective_validity(&self.connection, d)?,
            );
            v["catalog"] = d.descriptor.clone().unwrap_or(Value::Null);
            v["catalogLabels"] = json!(d.catalog_labels);
            v["controlObservations"] = json!(d.control_observations);
            v["structuredResultSupport"] = json!(d.structured_result_support);
            v["controlEvidence"] = json!({"source":"configuration-backed-pinned-profile","effectiveTurnSettingsConfirmed":false});
            v["catalogDiagnosticId"] = json!(d.id);
            v["catalogObservedAtMs"] = json!(d.observed_at_ms);
            return Ok(v);
        }
        report(&self.connection, id, true, now)
    }
    pub(crate) fn connection_command(
        &self,
        id: &str,
        fingerprint: &str,
    ) -> StoreResult<Option<Value>> {
        command(&self.connection, id, fingerprint)
    }
    pub(crate) fn connection_save_command(
        &mut self,
        id: &str,
        fingerprint: &str,
        v: &Value,
    ) -> StoreResult<()> {
        save_command(&self.connection, id, fingerprint, v)
    }
    pub(crate) fn connection_register(
        &mut self,
        key: &str,
        fingerprint: &str,
        v: &ConnectionLink,
        expected: Option<u64>,
        diagnostic_id: Option<&str>,
    ) -> StoreResult<Value> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(old) = command(&tx, key, fingerprint)? {
            return Ok(old);
        }
        if current(&tx, &v.id)?.as_ref().map(|l| l.revision) != expected {
            return Err(StoreError::StaleRevision);
        }
        if duplicate_auth_store(&tx, v)?.is_some() {
            return Err(StoreError::Constraint);
        }
        insert_link(&tx, v)?;
        tx.execute(
            "UPDATE connection_selection SET id=?1 WHERE singleton=1 AND id IS NULL",
            [&v.id],
        )?;
        let result = json!({"id":v.id,"revision":v.revision,"bindingRevision":v.binding_revision,"linked":true,"authenticated":false,"metadataRequested":diagnostic_id.is_some(),"diagnosticId":diagnostic_id});
        save_command(&tx, key, fingerprint, &result)?;
        tx.commit()?;
        Ok(result)
    }
    #[cfg(test)]
    pub(crate) fn connection_update(
        &mut self,
        key: &str,
        fingerprint: &str,
        id: &str,
        revision: u64,
        change: &ConnectionChange,
    ) -> StoreResult<Value> {
        self.connection_update_at(key, fingerprint, id, revision, change, now_ms())
    }
    pub(crate) fn connection_update_at(
        &mut self,
        key: &str,
        fingerprint: &str,
        id: &str,
        revision: u64,
        change: &ConnectionChange,
        now: u64,
    ) -> StoreResult<Value> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(old) = command(&tx, key, fingerprint)? {
            return Ok(old);
        }
        let mut l = current(&tx, id)?.ok_or(StoreError::InvalidState)?;
        if l.revision != revision {
            return Err(StoreError::StaleRevision);
        }
        if matches!(change, ConnectionChange::Select) {
            l.guard(revision).map_err(|_| StoreError::InvalidState)?;
            tx.execute(
                "UPDATE connection_selection SET id=?1 WHERE singleton=1",
                [id],
            )?;
            tx.execute("INSERT INTO daemon_events(kind,body) VALUES('connection-selected',?1)",[json!({"connectionId":id,"connectionRevision":l.binding_revision,"registrationRevision":l.revision}).to_string()])?;
        } else {
            match change {
                ConnectionChange::Rename { name } => l.name = name.clone(),
                ConnectionChange::Disable => l.enabled = false,
                ConnectionChange::Enable => {
                    if !l.linked {
                        return Err(StoreError::InvalidState);
                    }
                    l.enabled = true;
                }
                ConnectionChange::Unlink => {
                    l.enabled = false;
                    l.linked = false;
                }
                ConnectionChange::Reconfirm { diagnostic_id } => {
                    let all = diagnostics(&tx, id)?;
                    let d = all
                        .iter()
                        .rev()
                        .find(|d| {
                            d.id == *diagnostic_id
                                && matches!(d.state.as_str(), "complete" | "partial")
                                && d.connection_revision == l.binding_revision
                        })
                        .ok_or(StoreError::InvalidState)?;
                    let generation = tx.query_row(
                        "SELECT generation FROM daemon_meta WHERE singleton=1",
                        [],
                        |r| r.get::<_, i64>(0),
                    )? as u64;
                    let maximum_age = diagnostic_effective_validity(&tx, d)?.minimum();
                    let effective_until = d
                        .observed_at_ms
                        .map(|observed| observed.saturating_add(maximum_age))
                        .zip(d.valid_until_ms)
                        .map(|(configured, source)| configured.min(source));
                    if d.daemon_generation != generation
                        || d.observed_at_ms.is_none_or(|t| now < t)
                        || effective_until.is_none_or(|t| now >= t)
                    {
                        return Err(StoreError::InvalidState);
                    }
                    l.baseline_observation = d.observation_fingerprint.clone();
                    l.identity_assurance = d.identity_assurance.clone();
                }
                ConnectionChange::Select => unreachable!(),
            }
            interrupt_pending(&tx, Some(id), "cancelled")?;
            if !matches!(change, ConnectionChange::Rename { .. }) {
                l.binding_revision = l
                    .binding_revision
                    .checked_add(1)
                    .ok_or(StoreError::InvalidState)?;
            }
            l.revision = l.revision.checked_add(1).ok_or(StoreError::InvalidState)?;
            insert_link(&tx, &l)?;
            if !l.enabled || !l.linked {
                tx.execute(
                    "UPDATE connection_selection SET id=NULL WHERE singleton=1 AND id=?1",
                    [id],
                )?;
            }
        }
        let result = json!({"id":l.id,"revision":l.revision,"bindingRevision":l.binding_revision,"enabled":l.enabled,"linked":l.linked,"activeExecutionChanged":false});
        save_command(&tx, key, fingerprint, &result)?;
        tx.commit()?;
        Ok(result)
    }
    pub(crate) fn connection_begin_diagnostic(
        &mut self,
        d: &ConnectionDiagnostic,
        command_data: Option<(&str, &str, &Value)>,
    ) -> StoreResult<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some((id, fingerprint, v)) = command_data {
            if command(&tx, id, fingerprint)?.is_some() {
                return Err(StoreError::Constraint);
            }
            save_command(&tx, id, fingerprint, v)?;
        }
        append_diagnostic(&tx, d, true)?;
        let validity = effective_validity(&tx, &d.connection_id)?;
        let refresh = refresh_configuration(&tx, &d.connection_id)?.map_or(0, |c| c.revision);
        let policy:i64=tx.query_row("SELECT coalesce(max(version),0) FROM definitions WHERE kind='\"configuration\"' AND id='desktop-settings'",[],|r|r.get(0))?;
        let body = encode(&validity)?;
        tx.execute(
            "INSERT INTO connection_diagnostic_validity VALUES(?1,?2,?3,?4,?5)",
            params![d.id, refresh as i64, policy, body, digest(&body)],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub(crate) fn connection_record_diagnostic(
        &mut self,
        d: &ConnectionDiagnostic,
    ) -> StoreResult<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        append_diagnostic(&tx, d, true)?;
        tx.commit()?;
        Ok(())
    }
    pub(crate) fn connection_interrupt_pending(&mut self) -> StoreResult<()> {
        let tx = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        interrupt_pending(&tx, None, "interrupted")?;
        tx.commit()?;
        Ok(())
    }
    /// Startup validation preserves history; unfinished reads are not rerun.
    pub(crate) fn connection_validate_history(&self) -> StoreResult<()> {
        let mut q = self
            .connection
            .prepare("SELECT id,revision,body,digest FROM connection_links ORDER BY id,revision")?;
        let mut previous: Option<(String, i64)> = None;
        for row in q.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })? {
            let (id, rev, body, hash) = row?;
            decode_link(body, hash, &id, rev)?;
            if rev
                != previous
                    .as_ref()
                    .filter(|(p, _)| p == &id)
                    .map_or(1, |(_, v)| v + 1)
            {
                return Err(StoreError::InvalidState);
            }
            if previous.as_ref().is_none_or(|(p, _)| p != &id) {
                diagnostics(&self.connection, &id)?;
            }
            previous = Some((id, rev));
        }
        let mut refreshes=self.connection.prepare("SELECT connection_id,revision,body,digest FROM connection_refresh_configurations ORDER BY connection_id,revision")?;
        let mut previous: Option<(String, i64)> = None;
        for row in refreshes.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })? {
            let (id, revision, body, hash) = row?;
            if body.len() > 8192 || digest(&body) != hash {
                return Err(StoreError::InvalidState);
            }
            let config: RefreshConfiguration =
                serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
            config.validate().map_err(|_| StoreError::InvalidState)?;
            if config.connection_id != id
                || config.revision != revision as u64
                || current(&self.connection, &id)?.is_none()
                || revision
                    != previous
                        .as_ref()
                        .filter(|(p, _)| p == &id)
                        .map_or(1, |(_, r)| r + 1)
            {
                return Err(StoreError::InvalidState);
            }
            previous = Some((id, revision));
        }
        let mut captures=self.connection.prepare("SELECT v.id,v.refresh_revision,v.policy_version,v.body,v.digest FROM connection_diagnostic_validity v")?;
        for row in captures.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        })? {
            let (id, refresh, policy, body, hash) = row?;
            if refresh < 0 || policy < 0 || body.len() > 2048 || digest(&body) != hash {
                return Err(StoreError::InvalidState);
            }
            let cap: ObservationValidity =
                serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
            cap.validate().map_err(|_| StoreError::InvalidState)?;
            let exists: bool = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM connection_diagnostic_runs WHERE id=?1)",
                [id],
                |r| r.get(0),
            )?;
            if !exists {
                return Err(StoreError::InvalidState);
            }
        }
        Ok(())
    }
}

fn append_diagnostic(
    c: &Connection,
    d: &ConnectionDiagnostic,
    current_required: bool,
) -> StoreResult<()> {
    d.validate().map_err(|_| StoreError::InvalidState)?;
    if current_required {
        let generation = c.query_row(
            "SELECT generation FROM daemon_meta WHERE singleton=1",
            [],
            |r| r.get::<_, i64>(0),
        )? as u64;
        if d.daemon_generation != generation {
            return Err(StoreError::StaleRevision);
        }
        let l = current(c, &d.connection_id)?.ok_or(StoreError::InvalidState)?;
        if l.revision != d.registration_revision
            || l.binding_revision != d.connection_revision
            || !l.enabled
            || !l.linked
        {
            return Err(StoreError::StaleRevision);
        }
    }
    let previous: Option<String> = c
        .query_row(
            "SELECT body FROM connection_diagnostic_runs WHERE id=?1 ORDER BY ordinal DESC LIMIT 1",
            [&d.id],
            |r| r.get(0),
        )
        .optional()?;
    let ordinal = if let Some(p) = previous {
        let p: ConnectionDiagnostic =
            serde_json::from_str(&p).map_err(|_| StoreError::InvalidState)?;
        if p.state != "running"
            || d.state == "running"
            || p.connection_id != d.connection_id
            || p.connection_revision != d.connection_revision
            || p.registration_revision != d.registration_revision
            || p.started_at_ms != d.started_at_ms
            || p.synthetic != d.synthetic
            || p.daemon_generation != d.daemon_generation
            || p.purpose != d.purpose
            || d.finished_at_ms.is_none()
        {
            return Err(StoreError::InvalidState);
        }
        2i64
    } else {
        if d.state != "running" || d.finished_at_ms.is_some() || d.observed_at_ms.is_some() {
            return Err(StoreError::InvalidState);
        }
        1
    };
    let count: i64 = c.query_row(
        "SELECT count(*) FROM connection_diagnostic_runs WHERE connection_id=?1",
        [&d.connection_id],
        |r| r.get(0),
    )?;
    if count >= 512 {
        return Err(StoreError::Configuration);
    }
    let b = encode(d)?;
    if b.len() > 262144 {
        return Err(StoreError::Configuration);
    }
    c.execute("INSERT INTO connection_diagnostic_runs(id,ordinal,connection_id,connection_revision,registration_revision,body,digest) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![d.id,ordinal,d.connection_id,d.connection_revision as i64,d.registration_revision as i64,b,digest(&b)])?;
    c.execute("INSERT INTO daemon_events(kind,body) VALUES('connection-diagnostic',?1)",[json!({"connectionId":d.connection_id,"connectionRevision":d.connection_revision,"diagnosticId":d.id,"state":d.state}).to_string()])?;
    Ok(())
}
fn interrupt_pending(c: &Connection, id: Option<&str>, state: &str) -> StoreResult<()> {
    let mut q=c.prepare("SELECT d.body FROM connection_diagnostic_runs d WHERE (?1 IS NULL OR d.connection_id=?1) AND d.ordinal=(SELECT max(p.ordinal) FROM connection_diagnostic_runs p WHERE p.id=d.id) AND json_extract(d.body,'$.state')='running' LIMIT 65")?;
    let bodies = q
        .query_map([id], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    if bodies.len() > 64 {
        return Err(StoreError::Configuration);
    }
    drop(q);
    for b in bodies {
        let mut d: ConnectionDiagnostic =
            serde_json::from_str(&b).map_err(|_| StoreError::InvalidState)?;
        d.state = state.into();
        d.finished_at_ms = Some(now_ms());
        d.error_code = Some(
            if state == "cancelled" {
                "metadata-invalidated"
            } else {
                "host-restarted-read-not-replayed"
            }
            .into(),
        );
        append_diagnostic(c, &d, false)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{connections::ConnectionLink, daemon::RuntimeChoice};
    fn store() -> Store {
        let root = std::env::temp_dir().join(format!(
            "orchestrix-connections-store-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        Store::open(root.join("state.sqlite")).unwrap()
    }
    fn link(s: &Store) -> ConnectionLink {
        ConnectionLink {
            schema_version: 1,
            id: "registered".into(),
            revision: 1,
            binding_revision: 1,
            name: "My Codex".into(),
            enabled: true,
            linked: true,
            runtime: RuntimeChoice::Synthetic {
                executable: std::env::current_exe().unwrap(),
                home: s.path().parent().unwrap().to_owned(),
                scenario: "connection-metadata".into(),
            },
            metadata_directory: s.path().parent().unwrap().to_owned(),
            executable_sha256: "a".repeat(64),
            baseline_observation: None,
            identity_assurance: "unknown".into(),
        }
    }
    fn diagnostic(id: &str) -> ConnectionDiagnostic {
        ConnectionDiagnostic {
            schema_version: 1,
            id: id.into(),
            connection_id: "registered".into(),
            connection_revision: 1,
            registration_revision: 1,
            daemon_generation: 0,
            synthetic: true,
            state: "running".into(),
            started_at_ms: 100,
            finished_at_ms: None,
            observed_at_ms: None,
            valid_until_ms: None,
            next_allowed_at_ms: None,
            error_code: None,
            identity_assurance: "unknown".into(),
            observation_fingerprint: None,
            identity_changed: false,
            descriptor: None,
            catalog_labels: Vec::new(),
            control_observations: Vec::new(),
            structured_result_support: None,
            complete: false,
            cached: false,
            purpose: "registration".into(),
            scope_key: None,
        }
    }
    fn succeed(d: &mut ConnectionDiagnostic, partial: bool) {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/runtime-conformance-v1.json"
        ))
        .unwrap();
        let mut desc = fixture["descriptors"]["base"].clone();
        desc["connection"]["id"] = json!("registered");
        desc["connection"]["revision"] = json!(1);
        if partial {
            desc["catalog"]["completeness"] = json!("partial")
        }
        d.state = if partial { "partial" } else { "complete" }.into();
        d.finished_at_ms = Some(120);
        d.observed_at_ms = Some(100);
        d.valid_until_ms = Some(60_100);
        d.identity_assurance = "synthetic".into();
        d.observation_fingerprint = Some("b".repeat(64));
        d.scope_key = Some("c".repeat(64));
        d.descriptor = Some(desc);
        d.complete = !partial;
    }
    #[test]
    fn lifecycle_cas_preserves_runtime_binding_on_rename_and_history_on_unlink() {
        let mut s = store();
        let l = link(&s);
        s.connection_register("link", &"1".repeat(64), &l, None, None)
            .unwrap();
        let mut d = diagnostic("observation");
        s.connection_record_diagnostic(&d).unwrap();
        succeed(&mut d, false);
        s.connection_record_diagnostic(&d).unwrap();
        s.connection_update(
            "rename",
            &"2".repeat(64),
            "registered",
            1,
            &ConnectionChange::Rename {
                name: "Renamed".into(),
            },
        )
        .unwrap();
        let renamed = s.connection_link("registered").unwrap().unwrap();
        assert_eq!((renamed.revision, renamed.binding_revision), (2, 1));
        assert_eq!(renamed.baseline_observation, Some("b".repeat(64)));
        assert!(renamed.guard_binding(1).is_ok());
        assert_eq!(
            s.connection_update(
                "stale",
                &"3".repeat(64),
                "registered",
                1,
                &ConnectionChange::Disable
            )
            .unwrap_err(),
            StoreError::StaleRevision
        );
        s.connection_update(
            "disable",
            &"4".repeat(64),
            "registered",
            2,
            &ConnectionChange::Disable,
        )
        .unwrap();
        let disabled = s.connection_link("registered").unwrap().unwrap();
        assert_eq!((disabled.revision, disabled.binding_revision), (3, 2));
        assert!(disabled.guard_binding(1).is_err());
        assert!(disabled.guard_binding(2).is_err());
        s.connection_update(
            "unlink",
            &"5".repeat(64),
            "registered",
            3,
            &ConnectionChange::Unlink,
        )
        .unwrap();
        assert!(!s.connection_link("registered").unwrap().unwrap().linked);
        assert_eq!(
            s.connection_diagnostics("registered").unwrap()["lastSuccessfulAtMs"],
            100
        );
        let path = s.path().to_owned();
        drop(s);
        let reopened = Store::open(path).unwrap();
        assert!(
            !reopened
                .connection_link("registered")
                .unwrap()
                .unwrap()
                .linked
        );
        assert_eq!(
            reopened.connection_diagnostics("registered").unwrap()["lastSuccessfulAtMs"],
            100
        );
    }
    #[test]
    fn partial_first_observation_establishes_limited_fence_and_failure_preserves_date() {
        let mut s = store();
        s.connection_register("link", &"1".repeat(64), &link(&s), None, None)
            .unwrap();
        let mut d = diagnostic("partial-first");
        s.connection_record_diagnostic(&d).unwrap();
        succeed(&mut d, true);
        s.connection_record_diagnostic(&d).unwrap();
        assert_eq!(
            s.connection_link("registered")
                .unwrap()
                .unwrap()
                .baseline_observation,
            Some("b".repeat(64))
        );
        let mut failed = diagnostic("later-failure");
        failed.started_at_ms = 200;
        s.connection_record_diagnostic(&failed).unwrap();
        failed.state = "failed".into();
        failed.finished_at_ms = Some(210);
        failed.error_code = Some("authentication".into());
        s.connection_record_diagnostic(&failed).unwrap();
        let report = s.connection_diagnostics("registered").unwrap();
        assert_eq!(report["lastSuccessfulAtMs"], 100);
        assert_eq!(report["lastSuccessful"]["state"], "partial");
        assert_eq!(report["lastSuccessful"]["complete"], false);
        assert_eq!(report["latest"]["state"], "failed");
        assert_eq!(report["catalog"]["catalog"]["completeness"], "partial");
        assert!(!report.to_string().contains(&"b".repeat(64)));
        let path = s.path().to_owned();
        drop(s);
        assert_eq!(
            Store::open(path)
                .unwrap()
                .connection_diagnostics("registered")
                .unwrap()["lastSuccessfulAtMs"],
            100
        );
    }
    #[test]
    fn pending_read_is_interrupted_without_replay_and_exact_run_is_preserved() {
        let mut s = store();
        s.connection_register("link", &"1".repeat(64), &link(&s), None, None)
            .unwrap();
        let d = diagnostic("interrupted");
        let reply = json!({"state":"accepted","diagnosticId":d.id});
        s.connection_begin_diagnostic(&d, Some(("diagnose", &"2".repeat(64), &reply)))
            .unwrap();
        s.connection_interrupt_pending().unwrap();
        s.connection_interrupt_pending().unwrap();
        let report = s
            .connection_diagnostics_run("registered", Some("interrupted"))
            .unwrap();
        assert_eq!(report["latest"]["state"], "interrupted");
        assert_eq!(
            s.connection_command("diagnose", &"2".repeat(64)).unwrap(),
            Some(reply)
        );
        assert_eq!(
            s.connection_command("diagnose", &"3".repeat(64))
                .unwrap_err(),
            StoreError::IdempotencyConflict
        );
        assert!(
            s.connection_diagnostics_run("registered", Some("missing"))
                .is_err()
        );
        assert!(s.connection_record_diagnostic(&d).is_err());
    }
    #[test]
    fn edited_registration_cancels_read_and_rejects_late_success() {
        let mut s = store();
        s.connection_register("link", &"1".repeat(64), &link(&s), None, None)
            .unwrap();
        let mut d = diagnostic("late");
        s.connection_record_diagnostic(&d).unwrap();
        s.connection_update(
            "rename",
            &"2".repeat(64),
            "registered",
            1,
            &ConnectionChange::Rename {
                name: "Another label".into(),
            },
        )
        .unwrap();
        succeed(&mut d, false);
        assert_eq!(
            s.connection_record_diagnostic(&d).unwrap_err(),
            StoreError::StaleRevision
        );
        assert_eq!(
            s.connection_diagnostics("registered").unwrap()["latest"]["state"],
            "cancelled"
        );
        assert!(
            s.connection_link("registered")
                .unwrap()
                .unwrap()
                .baseline_observation
                .is_none()
        );
    }
    #[test]
    fn link_history_digest_or_revision_corruption_is_rejected_without_repair() {
        let mut s = store();
        s.connection_register("link", &"1".repeat(64), &link(&s), None, None)
            .unwrap();
        let path = s.path().to_owned();
        s.connection
            .execute_batch(
                "DROP TRIGGER connection_links_no_update;UPDATE connection_links SET digest='bad'",
            )
            .unwrap();
        drop(s);
        assert!(matches!(Store::open(path), Err(StoreError::InvalidState)));
    }
    #[test]
    fn controlled_freshness_expires_without_renewing_dates_or_erasing_partial_catalog() {
        let mut s = store();
        s.connection_register("link", &"1".repeat(64), &link(&s), None, None)
            .unwrap();
        let mut d = diagnostic("clocked");
        s.connection_record_diagnostic(&d).unwrap();
        succeed(&mut d, true);
        s.connection_record_diagnostic(&d).unwrap();
        let fresh = s
            .connection_diagnostics_run_at("registered", Some("clocked"), 100)
            .unwrap();
        let expired = s
            .connection_diagnostics_run_at("registered", Some("clocked"), 60_100)
            .unwrap();
        assert_eq!(fresh["latest"]["current"], true);
        assert_eq!(expired["latest"]["current"], false);
        assert_eq!(
            fresh["latest"]["observedAtMs"],
            expired["latest"]["observedAtMs"]
        );
        assert_eq!(fresh["catalog"], expired["catalog"]);
        assert_eq!(expired["latest"]["complete"], false);
        assert_eq!(expired["observationRenewedOnRead"], false);
    }
    #[test]
    fn diagnostic_registration_column_must_match_its_hashed_body() {
        let mut s = store();
        s.connection_register("link", &"1".repeat(64), &link(&s), None, None)
            .unwrap();
        let mut d = diagnostic("crossed-column");
        s.connection_record_diagnostic(&d).unwrap();
        succeed(&mut d, false);
        s.connection_record_diagnostic(&d).unwrap();
        s.connection_update(
            "rename",
            &"2".repeat(64),
            "registered",
            1,
            &ConnectionChange::Rename {
                name: "Same binding".into(),
            },
        )
        .unwrap();
        s.connection.execute_batch("DROP TRIGGER connection_diagnostic_no_update;UPDATE connection_diagnostic_runs SET registration_revision=2").unwrap();
        let path = s.path().to_owned();
        drop(s);
        assert!(matches!(Store::open(path), Err(StoreError::InvalidState)));
    }
    #[test]
    fn a_recent_observation_from_a_previous_daemon_cannot_reconfirm_identity() {
        let mut s = store();
        s.connection_register("link", &"1".repeat(64), &link(&s), None, None)
            .unwrap();
        let observed = now_ms();
        let mut d = diagnostic("historical-but-within-ttl");
        d.started_at_ms = observed;
        s.connection_record_diagnostic(&d).unwrap();
        succeed(&mut d, false);
        d.finished_at_ms = Some(observed);
        d.observed_at_ms = Some(observed);
        d.valid_until_ms = Some(observed + 60_000);
        s.connection_record_diagnostic(&d).unwrap();
        assert_eq!(
            s.connection_diagnostics_run_at("registered", Some(&d.id), observed)
                .unwrap()["latest"]["current"],
            true
        );
        s.daemon_start().unwrap();
        assert!(
            s.connection_update_at(
                "reject-old-confirmation",
                &"2".repeat(64),
                "registered",
                1,
                &ConnectionChange::Reconfirm {
                    diagnostic_id: d.id
                },
                observed + 1
            )
            .is_err()
        );
        assert_eq!(
            s.connection_link("registered").unwrap().unwrap().revision,
            1
        );
    }
    #[test]
    fn recurring_configuration_has_own_cas_revocable_scope_and_safe_old_defaults() {
        let mut s = store();
        let link = link(&s);
        s.connection_register("link", &"1".repeat(64), &link, None, None)
            .unwrap();
        assert!(
            s.connection_refresh_configuration("registered")
                .unwrap()
                .is_none()
        );
        let mut settings = RefreshSettings {
            mode: crate::connection_refresh::RefreshMode::Automatic,
            ..RefreshSettings::default()
        };
        assert!(
            s.connection_configure_refresh(
                "unconsented",
                &"2".repeat(64),
                "registered",
                1,
                0,
                &settings,
                100
            )
            .is_err()
        );
        settings.recurring_consent = true;
        assert!(
            s.connection_configure_refresh(
                "unobserved",
                &"3".repeat(64),
                "registered",
                1,
                0,
                &settings,
                100
            )
            .is_err()
        );
        let mut d = diagnostic("observed");
        s.connection_record_diagnostic(&d).unwrap();
        succeed(&mut d, false);
        s.connection_record_diagnostic(&d).unwrap();
        s.connection_configure_refresh(
            "consented",
            &"4".repeat(64),
            "registered",
            1,
            0,
            &settings,
            100,
        )
        .unwrap();
        let config = s
            .connection_refresh_configuration("registered")
            .unwrap()
            .unwrap();
        assert!(config.authorized_for(&s.connection_link("registered").unwrap().unwrap()));
        assert_eq!(config.settings.effective_interval_ms(), 300_000);
        assert_eq!(
            s.connection_configure_refresh(
                "conflict",
                &"5".repeat(64),
                "registered",
                1,
                0,
                &settings,
                110
            )
            .unwrap_err(),
            StoreError::StaleRevision
        );
        s.connection_update(
            "rename",
            &"6".repeat(64),
            "registered",
            1,
            &ConnectionChange::Rename {
                name: "renamed".into(),
            },
        )
        .unwrap();
        assert!(config.authorized_for(&s.connection_link("registered").unwrap().unwrap()));
        s.connection_update(
            "disable",
            &"7".repeat(64),
            "registered",
            2,
            &ConnectionChange::Disable,
        )
        .unwrap();
        assert!(!config.authorized_for(&s.connection_link("registered").unwrap().unwrap()));
        let before = s.connection_diagnostics("registered").unwrap()["lastSuccessfulAtMs"].clone();
        settings = RefreshSettings::default();
        s.connection_configure_refresh(
            "revoke",
            &"8".repeat(64),
            "registered",
            3,
            1,
            &settings,
            120,
        )
        .unwrap();
        assert_eq!(
            s.connection_refresh_configuration("registered")
                .unwrap()
                .unwrap()
                .revision,
            2
        );
        assert_eq!(
            s.connection_diagnostics("registered").unwrap()["lastSuccessfulAtMs"],
            before
        );
        let path = s.path().to_owned();
        drop(s);
        let reopened = Store::open(path).unwrap();
        assert_eq!(
            reopened
                .connection_refresh_configuration("registered")
                .unwrap()
                .unwrap()
                .settings
                .mode,
            crate::connection_refresh::RefreshMode::Manual
        );
    }
    #[test]
    fn configured_shorter_validity_never_rewrites_source_dates_or_bypasses_policy() {
        let mut s = store();
        let link = link(&s);
        s.connection_register("link", &"1".repeat(64), &link, None, None)
            .unwrap();
        let mut d = diagnostic("source");
        s.connection_record_diagnostic(&d).unwrap();
        succeed(&mut d, false);
        s.connection_record_diagnostic(&d).unwrap();
        let mut settings = RefreshSettings::default();
        settings.validity_ms.models = 1_000;
        s.connection_configure_refresh(
            "shorter",
            &"2".repeat(64),
            "registered",
            1,
            0,
            &settings,
            200,
        )
        .unwrap();
        let fresh = s
            .connection_diagnostics_run_at("registered", Some("source"), 1_099)
            .unwrap();
        let expired = s
            .connection_diagnostics_run_at("registered", Some("source"), 1_100)
            .unwrap();
        assert_eq!(fresh["latest"]["current"], true);
        assert_eq!(expired["latest"]["current"], false);
        assert_eq!(expired["latest"]["validUntilMs"], 60_100);
        assert_eq!(expired["latest"]["effectiveValidUntilMs"], 1_100);
        assert_eq!(expired["latest"]["observedAtMs"], 100);
        settings.validity_ms.models = 900_000;
        s.connection_configure_refresh(
            "longer",
            &"3".repeat(64),
            "registered",
            1,
            1,
            &settings,
            2_000,
        )
        .unwrap();
        let later = s
            .connection_diagnostics_run_at("registered", Some("source"), 60_100)
            .unwrap();
        assert_eq!(later["latest"]["effectiveValidUntilMs"], 1_100);
        assert_eq!(later["latest"]["current"], false);
        assert_eq!(later["latest"]["observedAtMs"], 100);
        let mut newer = diagnostic("fresh-source");
        newer.started_at_ms = 60_200;
        s.connection_begin_diagnostic(&newer, None).unwrap();
        succeed(&mut newer, false);
        newer.observed_at_ms = Some(60_200);
        newer.finished_at_ms = Some(60_220);
        newer.valid_until_ms = Some(120_200);
        s.connection_record_diagnostic(&newer).unwrap();
        let fresh = s
            .connection_diagnostics_run_at("registered", Some("fresh-source"), 60_220)
            .unwrap();
        assert_eq!(fresh["latest"]["effectiveValidUntilMs"], 120_200);
        assert_eq!(fresh["latest"]["current"], true);
    }
    #[test]
    fn daemon_refresh_configuration_polls_do_not_read_or_create_work_and_reopen_defers() {
        use crate::connections::{ConnectionRequest, ConnectionService};
        use crate::runtime::ManualRefreshClock;
        use std::sync::Arc;
        let mut s = store();
        let link = link(&s);
        s.connection_register("link", &"1".repeat(64), &link, None, None)
            .unwrap();
        let mut d = diagnostic("observed");
        s.connection_record_diagnostic(&d).unwrap();
        succeed(&mut d, false);
        s.connection_record_diagnostic(&d).unwrap();
        let clock = Arc::new(ManualRefreshClock::new(100));
        let mut service = ConnectionService::with_clock(clock.clone());
        let settings = RefreshSettings {
            interval_ms: 1_000,
            mode: crate::connection_refresh::RefreshMode::Automatic,
            recurring_consent: true,
            ..RefreshSettings::default()
        };
        service
            .handle(
                &mut s,
                &ConnectionRequest::ConfigureRefresh {
                    command_id: "enable-refresh".into(),
                    id: "registered".into(),
                    expected_revision: 1,
                    expected_refresh_revision: 0,
                    settings,
                },
                false,
            )
            .unwrap();
        for _ in 0..4 {
            let status = service
                .handle(
                    &mut s,
                    &ConnectionRequest::RefreshStatus {
                        id: Some("registered".into()),
                    },
                    false,
                )
                .unwrap();
            assert_eq!(status["refresh"][0]["nextOpportunityAtMs"], 60_100);
            assert_eq!(status["refresh"][0]["effective"]["intervalMs"], 60_000);
            assert_eq!(status["refresh"][0]["recurringAuthorized"], true);
            service.tick(&mut s).unwrap();
            assert!(service.is_idle());
        }
        clock.set(60_099);
        service.tick(&mut s).unwrap();
        assert!(service.is_idle());
        assert_eq!(
            s.connection_diagnostics("registered").unwrap()["latest"]["id"],
            "observed"
        );
        assert_eq!(
            s.connection
                .query_row("SELECT count(*) FROM manual_workers", [], |r| r
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
        drop(service);
        clock.set(8_000_000);
        let mut reopened = ConnectionService::with_clock(clock.clone());
        let status = reopened
            .handle(
                &mut s,
                &ConnectionRequest::RefreshStatus {
                    id: Some("registered".into()),
                },
                false,
            )
            .unwrap();
        assert_eq!(status["refresh"][0]["nextOpportunityAtMs"], 8_060_000);
        reopened.tick(&mut s).unwrap();
        assert!(reopened.is_idle());
        let mut settings = RefreshSettings::default();
        settings.validity_ms.models = 5_000;
        reopened
            .handle(
                &mut s,
                &ConnectionRequest::ConfigureRefresh {
                    command_id: "revoke-refresh".into(),
                    id: "registered".into(),
                    expected_revision: 1,
                    expected_refresh_revision: 1,
                    settings,
                },
                false,
            )
            .unwrap();
        clock.set(9_000_000);
        reopened.tick(&mut s).unwrap();
        assert!(reopened.is_idle());
        let status = reopened
            .handle(
                &mut s,
                &ConnectionRequest::RefreshStatus {
                    id: Some("registered".into()),
                },
                false,
            )
            .unwrap();
        assert_eq!(status["refresh"][0]["reason"], "manual");
        assert_eq!(status["refresh"][0]["nextOpportunityAtMs"], Value::Null);
        assert_eq!(
            s.connection_diagnostics("registered").unwrap()["lastSuccessfulAtMs"],
            100
        );
    }
    #[test]
    fn intervening_policy_ttl_cannot_be_removed_from_existing_observation() {
        let mut s = store();
        let link = link(&s);
        s.connection_register("link", &"1".repeat(64), &link, None, None)
            .unwrap();
        let mut d = diagnostic("policy-source");
        s.connection_begin_diagnostic(&d, None).unwrap();
        succeed(&mut d, false);
        s.connection_record_diagnostic(&d).unwrap();
        let save = |store: &mut Store, command: &str, ttl: u64| {
            let revision = store.desktop_snapshot().unwrap()["revision"]
                .as_u64()
                .unwrap();
            let mut settings = crate::desktop::Settings::default();
            settings.policy.observation_ttls.identity_billing_ms = ttl;
            crate::desktop::handle(
                store,
                &crate::desktop::DesktopRequest::SaveSettings {
                    command_id: command.into(),
                    expected_revision: revision,
                    settings: Box::new(settings),
                },
            )
            .unwrap();
        };
        save(&mut s, "short-policy", 1_000);
        save(&mut s, "long-policy", 300_000);
        let report = s
            .connection_diagnostics_run_at("registered", Some("policy-source"), 1_100)
            .unwrap();
        assert_eq!(report["latest"]["effectiveValidUntilMs"], 1_100);
        assert_eq!(report["latest"]["current"], false);
        assert_eq!(report["latest"]["observedAtMs"], 100);
        let mut fresh = diagnostic("policy-fresh");
        fresh.started_at_ms = 2_000;
        s.connection_begin_diagnostic(&fresh, None).unwrap();
        succeed(&mut fresh, false);
        fresh.observed_at_ms = Some(2_000);
        fresh.finished_at_ms = Some(2_020);
        fresh.valid_until_ms = Some(62_000);
        s.connection_record_diagnostic(&fresh).unwrap();
        let report = s
            .connection_diagnostics_run_at("registered", Some("policy-fresh"), 2_020)
            .unwrap();
        assert_eq!(report["latest"]["effectiveValidUntilMs"], 62_000);
        assert_eq!(report["latest"]["current"], true);
    }
    #[test]
    fn reconfirm_requires_effective_identity_lease_without_altering_source_dates() {
        let mut s = store();
        let link = link(&s);
        s.connection_register("link", &"1".repeat(64), &link, None, None)
            .unwrap();
        let mut baseline = diagnostic("initial-identity");
        s.connection_begin_diagnostic(&baseline, None).unwrap();
        succeed(&mut baseline, false);
        s.connection_record_diagnostic(&baseline).unwrap();
        let mut changed = diagnostic("changed-identity");
        changed.started_at_ms = 200;
        s.connection_begin_diagnostic(&changed, None).unwrap();
        succeed(&mut changed, false);
        changed.observed_at_ms = Some(200);
        changed.finished_at_ms = Some(220);
        changed.valid_until_ms = Some(60_200);
        changed.identity_changed = true;
        changed.observation_fingerprint = Some("d".repeat(64));
        s.connection_record_diagnostic(&changed).unwrap();
        let mut settings = RefreshSettings::default();
        settings.validity_ms.identity = 1_000;
        s.connection_configure_refresh(
            "short-identity",
            &"2".repeat(64),
            "registered",
            1,
            0,
            &settings,
            230,
        )
        .unwrap();
        let expired = s
            .connection_diagnostics_run_at("registered", Some("changed-identity"), 1_200)
            .unwrap();
        assert_eq!(expired["latest"]["current"], false);
        assert_eq!(expired["latest"]["validUntilMs"], 60_200);
        assert_eq!(expired["latest"]["effectiveValidUntilMs"], 1_200);
        assert!(
            s.connection_update_at(
                "expired-reconfirm",
                &"3".repeat(64),
                "registered",
                1,
                &ConnectionChange::Reconfirm {
                    diagnostic_id: "changed-identity".into()
                },
                1_200
            )
            .is_err()
        );
        let before = s.connection_link("registered").unwrap().unwrap();
        assert_eq!((before.revision, before.binding_revision), (1, 1));
        assert_eq!(before.baseline_observation, Some("b".repeat(64)));
        let exact = s
            .connection_diagnostics_run_at("registered", Some("changed-identity"), 1_200)
            .unwrap();
        assert_eq!(exact["latest"]["observedAtMs"], 200);
        assert_eq!(exact["latest"]["validUntilMs"], 60_200);
        // A genuine new read restores eligibility. Confirmation retains both
        // that new source lease and the expired observation's original dates.
        let mut fresh = diagnostic("fresh-changed-identity");
        fresh.started_at_ms = 1_300;
        s.connection_begin_diagnostic(&fresh, None).unwrap();
        succeed(&mut fresh, false);
        fresh.observed_at_ms = Some(1_300);
        fresh.finished_at_ms = Some(1_320);
        fresh.valid_until_ms = Some(61_300);
        fresh.identity_changed = true;
        fresh.observation_fingerprint = Some("d".repeat(64));
        s.connection_record_diagnostic(&fresh).unwrap();
        s.connection_update_at(
            "current-reconfirm",
            &"4".repeat(64),
            "registered",
            1,
            &ConnectionChange::Reconfirm {
                diagnostic_id: "fresh-changed-identity".into(),
            },
            1_400,
        )
        .unwrap();
        let after = s.connection_link("registered").unwrap().unwrap();
        assert_eq!((after.revision, after.binding_revision), (2, 2));
        assert_eq!(after.baseline_observation, Some("d".repeat(64)));
        let exact = s
            .connection_diagnostics_run_at("registered", Some("changed-identity"), 1_400)
            .unwrap();
        assert_eq!(exact["latest"]["observedAtMs"], 200);
        assert_eq!(exact["latest"]["validUntilMs"], 60_200);
        let exact = s
            .connection_diagnostics_run_at("registered", Some("fresh-changed-identity"), 1_400)
            .unwrap();
        assert_eq!(exact["latest"]["observedAtMs"], 1_300);
        assert_eq!(exact["latest"]["validUntilMs"], 61_300);
    }
}
