use super::diagnostic_history as history;
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
// Index immutable source bodies: no new authority/projection, no rewrite of legacy evidence.
pub(super) const MIGRATION_V11: &str = r#"
CREATE INDEX connection_capacity_denied ON connection_diagnostic_runs(connection_id,connection_revision,CAST(json_extract(body,'$.observedAtMs') AS INTEGER) DESC)
 WHERE json_extract(body,'$.capacity.ordinaryUsage.state')='reported' AND json_extract(body,'$.capacity.ordinaryUsage.value')=0;
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
    apply_validity_history(
        c,
        &d.connection_id,
        result,
        refresh_revision,
        policy_version,
    )
}
fn apply_validity_history(
    c: &Connection,
    id: &str,
    mut result: ObservationValidity,
    refresh_revision: i64,
    policy_version: i64,
) -> StoreResult<ObservationValidity> {
    let mut refreshes=c.prepare("SELECT body,digest FROM connection_refresh_configurations WHERE connection_id=?1 AND revision>=?2 ORDER BY revision")?;
    for row in refreshes.query_map(params![id, refresh_revision], |r| {
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
struct CohortMinimum {
    refresh: Option<i64>,
    policy: Option<i64>,
    identity: Option<i64>,
    availability: Option<i64>,
    configuration: Option<i64>,
    models: Option<i64>,
}
pub(super) fn diagnostic_effective_validity(
    c: &Connection,
    d: &ConnectionDiagnostic,
) -> StoreResult<ObservationValidity> {
    let mut result = diagnostic_validity_cap(c, d)?;
    if d.cached && d.observed_at_ms.is_some() {
        // Immutable envelopes are hash/shape-validated on startup and captured in the
        // same transaction as the start. The indexed narrow aggregate preserves the
        // oldest restrictions of the original lease without decoding old catalogs.
        let caps=c.query_row(
            "SELECT min(coalesce(v.refresh_revision,0)),min(coalesce(v.policy_version,0)),min(json_extract(v.body,'$.identity')),min(json_extract(v.body,'$.availability')),min(json_extract(v.body,'$.configuration')),min(json_extract(v.body,'$.models')) FROM connection_diagnostic_history h LEFT JOIN connection_diagnostic_validity v ON v.id=h.id WHERE h.connection_id=?1 AND h.observed=?2 AND h.scope=?3 AND h.binding_revision=?4 AND h.state IN('complete','partial')",
            params![d.connection_id,d.observed_at_ms.map(|v|v as i64),d.scope_key,d.connection_revision as i64],|r|Ok(CohortMinimum {refresh:r.get(0)?,policy:r.get(1)?,identity:r.get(2)?,availability:r.get(3)?,configuration:r.get(4)?,models:r.get(5)?}))?;
        for (value, target) in [
            (caps.identity, &mut result.identity),
            (caps.availability, &mut result.availability),
            (caps.configuration, &mut result.configuration),
            (caps.models, &mut result.models),
        ] {
            if let Some(value) = value {
                let value = u64::try_from(value).map_err(|_| StoreError::InvalidState)?;
                if value == 0 {
                    return Err(StoreError::InvalidState);
                }
                *target = (*target).min(value);
            }
        }
        if let (Some(refresh), Some(policy)) = (caps.refresh, caps.policy) {
            result = apply_validity_history(c, &d.connection_id, result, refresh, policy)?;
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
        && let Some(d) = history::query(c,"WHERE h.connection_id=?1 AND h.binding_revision=?2 AND h.state IN('complete','partial') ORDER BY h.observed,h.id,h.ordinal LIMIT 1",params![id,l.binding_revision as i64])?.into_iter().next().map(|e|e.diagnostic)
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
#[cfg(test)]
pub(super) fn diagnostics(c: &Connection, id: &str) -> StoreResult<Vec<ConnectionDiagnostic>> {
    history::validate(c, id)?;
    Ok(
        history::query(c, "WHERE h.connection_id=?1 ORDER BY h.id,h.ordinal", [id])?
            .into_iter()
            .map(|e| e.diagnostic)
            .collect(),
    )
}
pub(super) fn identity_changed(c: &Connection, id: &str, revision: u64) -> StoreResult<bool> {
    let candidate:Option<String>=c.query_row("SELECT id FROM connection_diagnostic_history WHERE connection_id=?1 AND binding_revision=?2 AND identity_changed=1 LIMIT 1",params![id,revision as i64],|r|r.get(0)).optional()?;
    if let Some(run) = candidate {
        let e = history::exact(c, id, &run, None)?.ok_or(StoreError::InvalidState)?;
        if !e.diagnostic.identity_changed {
            return Err(StoreError::InvalidState);
        }
        return Ok(true);
    }
    Ok(false)
}
pub(super) fn latest_registration_success(
    c: &Connection,
    id: &str,
    binding: u64,
    generation: u64,
    now: u64,
) -> StoreResult<Option<ConnectionDiagnostic>> {
    Ok(history::query(c,"WHERE h.connection_id=?1 AND h.binding_revision=?2 AND h.generation=?3 AND h.purpose='registration' AND h.state IN('complete','partial') AND h.observed<=?4 AND h.source_until>?4 ORDER BY h.observed DESC,h.id DESC LIMIT 1",params![id,binding as i64,generation as i64,now as i64])?.into_iter().next().map(|e|e.diagnostic))
}
fn last_success(c: &Connection, id: &str) -> StoreResult<Option<ConnectionDiagnostic>> {
    Ok(history::query(c,"WHERE h.connection_id=?1 AND h.state IN('complete','partial') ORDER BY h.observed DESC,h.id DESC,h.ordinal DESC LIMIT 1",[id])?.into_iter().next().map(|e|e.diagnostic))
}

fn summary(
    d: &ConnectionDiagnostic,
    now: u64,
    current_revision: Option<u64>,
    current_generation: u64,
    validity: &ObservationValidity,
) -> Value {
    let mut v = d.view(now, current_generation);
    let effective_until = d.effective_valid_until(validity.minimum());
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
fn report(
    c: &Connection,
    id: &str,
    detailed: bool,
    capacity_details: bool,
    now: u64,
) -> StoreResult<Value> {
    let current_revision = current(c, id)?.map(|l| l.binding_revision);
    let current_generation = c.query_row(
        "SELECT generation FROM daemon_meta WHERE singleton=1",
        [],
        |r| r.get::<_, i64>(0),
    )? as u64;
    let latest = history::query(c,"WHERE h.connection_id=?1 ORDER BY h.started DESC,h.finished DESC,h.id DESC,h.ordinal DESC LIMIT 1",[id])?.into_iter().next().map(|e|e.diagnostic);
    let successful = last_success(c, id)?;
    let recent = history::query(
        c,
        "WHERE h.connection_id=?1 AND h.finished IS NOT NULL ORDER BY h.sequence DESC LIMIT ?2",
        params![id, if detailed { 16 } else { 0 }],
    )?;
    let history = recent
        .iter()
        .map(|e| {
            let d = &e.diagnostic;
            let mut item = summary(
                d,
                now,
                current_revision,
                current_generation,
                &diagnostic_effective_validity(c, d)?,
            );
            item.as_object_mut().expect("summary").remove("capacity");
            Ok(item)
        })
        .collect::<StoreResult<Vec<_>>>()?;
    let mut latest_summary = latest
        .as_ref()
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
    let mut successful_summary = successful
        .as_ref()
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
    if !capacity_details {
        for value in [&mut latest_summary, &mut successful_summary]
            .into_iter()
            .flatten()
        {
            value.as_object_mut().expect("summary").remove("capacity");
        }
    }
    let storage = history::storage(c, id)?;
    let mut value = json!({"latest":latest_summary,"lastSuccessful":successful_summary,"lastSuccessfulAtMs":successful.as_ref().and_then(|d|d.observed_at_ms),"identityChanged":current_revision.map(|r|identity_changed(c,id,r)).transpose()?.unwrap_or(false),"history":history,"historyLimit":if detailed{16}else{0},"historyHasMore":storage["completedRounds"].as_u64().unwrap_or(0)>if detailed{16}else{0},"storage":storage,"observationRenewedOnRead":false});
    if detailed {
        value["catalog"] = successful
            .as_ref()
            .and_then(|d| d.descriptor.clone())
            .unwrap_or(Value::Null);
        value["catalogLabels"] = json!(successful.as_ref().map(|d| &d.catalog_labels));
        value["controlObservations"] = json!(successful.as_ref().map(|d| &d.control_observations));
        value["structuredResultSupport"] = json!(
            successful
                .as_ref()
                .and_then(|d| d.structured_result_support)
        );
        value["controlEvidence"] = json!({"source":"configuration-backed-pinned-profile","effectiveTurnSettingsConfirmed":false});
        value["catalogDiagnosticId"] = json!(successful.as_ref().map(|d| &d.id));
        value["catalogObservedAtMs"] = json!(successful.as_ref().and_then(|d| d.observed_at_ms));
    }
    Ok(value)
}
impl Store {
    /// A later negative permission applies across metadata directories, even after
    /// expiration/archive. Only a newer explicit positive read can supersede it.
    pub(crate) fn connection_capacity_denied_at(
        &self,
        id: &str,
        revision: u64,
    ) -> StoreResult<Option<u64>> {
        let found: Option<String> = self.connection.query_row(
            "SELECT id FROM connection_diagnostic_runs WHERE connection_id=?1 AND connection_revision=?2 AND json_extract(body,'$.capacity.ordinaryUsage.state')='reported' AND json_extract(body,'$.capacity.ordinaryUsage.value')=0 ORDER BY CAST(json_extract(body,'$.observedAtMs') AS INTEGER) DESC LIMIT 1",
            params![id, revision as i64], |row| row.get(0)).optional()?;
        found
            .map(|key| {
                history::exact(&self.connection, id, &key, None)?
                    .filter(|e| {
                        e.diagnostic
                            .capacity
                            .as_ref()
                            .is_some_and(|c| c.usage_allowed() == Some(false))
                    })
                    .and_then(|e| e.diagnostic.observed_at_ms)
                    .ok_or(StoreError::InvalidState)
            })
            .transpose()
    }
    pub(crate) fn connection_capacity_groups(&self, id: &str) -> StoreResult<Vec<String>> {
        let mut query = self.connection.prepare("SELECT id,checkpoint FROM capacity_groups WHERE EXISTS(SELECT 1 FROM json_each(checkpoint,'$.data.members') WHERE value=?1) LIMIT 65")?;
        let groups = query
            .query_map([id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if groups.len() > 64 {
            return Err(StoreError::InvalidState);
        }
        groups
            .into_iter()
            .map(|(key, body)| {
                let group = crate::domain::CapacityGroup::restore_checkpoint(
                    &serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?,
                )?;
                if group.id().as_str() != key
                    || !group.contains(&crate::domain::ConnectionId::new(id)?)
                {
                    return Err(StoreError::InvalidState);
                }
                Ok(key)
            })
            .collect()
    }
    #[cfg(test)]
    pub(crate) fn connection_diagnostic_exact_raw(
        &self,
        id: &str,
        run: &str,
    ) -> StoreResult<ConnectionDiagnostic> {
        history::exact(&self.connection, id, run, None)?
            .map(|e| e.diagnostic)
            .ok_or(StoreError::InvalidState)
    }
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
        let previous = last_success(&self.connection, id)?;
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
        history::remaining(&self.connection, id)
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
                || identity_changed(&tx, id, link.binding_revision)?
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
                    report(&self.connection, &id, false, false, now)?,
                ));
            }
            Ok(json!({"connections":views,"selectedConnectionId":selected}))
        })();
        self.connection.execute_batch("ROLLBACK")?;
        result
    }
    pub(crate) fn connection_identity_changed(&self, id: &str, revision: u64) -> StoreResult<bool> {
        identity_changed(&self.connection, id, revision)
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
            let evidence =
                history::exact(&self.connection, id, run, None)?.ok_or(StoreError::InvalidState)?;
            let d = &evidence.diagnostic;
            let current_revision = current(&self.connection, id)?.map(|l| l.binding_revision);
            let mut v = report(&self.connection, id, false, true, now)?;
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
        report(&self.connection, id, true, true, now)
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
                    let evidence = history::exact(&tx, id, diagnostic_id, None)?
                        .ok_or(StoreError::InvalidState)?;
                    let d = &evidence.diagnostic;
                    if !matches!(d.state.as_str(), "complete" | "partial")
                        || d.connection_revision != l.binding_revision
                    {
                        return Err(StoreError::InvalidState);
                    }
                    let generation = tx.query_row(
                        "SELECT generation FROM daemon_meta WHERE singleton=1",
                        [],
                        |r| r.get::<_, i64>(0),
                    )? as u64;
                    let maximum_age = diagnostic_effective_validity(&tx, d)?.minimum();
                    let effective_until = d.effective_valid_until(maximum_age);
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
                history::validate(&self.connection, &id)?;
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
    let previous = history::exact(c, &d.connection_id, &d.id, None)?;
    let ordinal = if previous.is_some() { 2 } else { 1 };
    history::validate_transition(previous.as_ref().map(|e| &e.diagnostic), d, ordinal)?;
    let b = encode(d)?;
    if b.len() > history::MAX_BODY_BYTES {
        return Err(StoreError::Configuration);
    }
    let hash = digest(&b);
    c.execute("INSERT INTO connection_diagnostic_runs(id,ordinal,connection_id,connection_revision,registration_revision,body,digest) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![d.id,ordinal,d.connection_id,d.connection_revision as i64,d.registration_revision as i64,b,hash])?;
    history::insert(c, d, ordinal, &b, &hash)?;
    c.execute("INSERT INTO daemon_events(kind,body) VALUES('connection-diagnostic',?1)",[json!({"connectionId":d.connection_id,"connectionRevision":d.connection_revision,"diagnosticId":d.id,"state":d.state}).to_string()])?;
    Ok(())
}
fn interrupt_pending(c: &Connection, id: Option<&str>, state: &str) -> StoreResult<()> {
    // Only outstanding, durably reserved rounds need interruption. Completed and
    // archived catalogs are never scanned by local cancel/disable/recovery controls.
    let mut q=c.prepare(if id.is_some(){"SELECT connection_id,id FROM connection_diagnostic_reservations WHERE connection_id=?1 ORDER BY id LIMIT 65"}else{"SELECT connection_id,id FROM connection_diagnostic_reservations ORDER BY id LIMIT 65"})?;
    let pending = if let Some(id) = id {
        q.query_map([id], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?
    } else {
        q.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?
    };
    if pending.len() > 64 {
        return Err(StoreError::Configuration);
    }
    drop(q);
    for (connection, run) in pending {
        let evidence =
            history::exact(c, &connection, &run, None)?.ok_or(StoreError::InvalidState)?;
        if evidence.ordinal != 1 || evidence.diagnostic.state != "running" {
            return Err(StoreError::InvalidState);
        }
        let mut d = evidence.diagnostic;
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
            capacity: None,
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
    fn retention_succeed(d: &mut ConnectionDiagnostic) {
        static BASE: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
        let mut descriptor = BASE
            .get_or_init(|| {
                let fixture: Value = serde_json::from_str(include_str!(
                    "../../tests/fixtures/runtime-conformance-v1.json"
                ))
                .unwrap();
                fixture["descriptors"]["base"].clone()
            })
            .clone();
        descriptor["connection"]["id"] = json!("registered");
        descriptor["connection"]["revision"] = json!(1);
        d.state = "complete".into();
        d.finished_at_ms = Some(120);
        d.observed_at_ms = Some(100);
        d.valid_until_ms = Some(60100);
        d.identity_assurance = "synthetic".into();
        d.observation_fingerprint = Some("b".repeat(64));
        d.scope_key = Some("c".repeat(64));
        d.descriptor = Some(descriptor);
        d.complete = true;
    }
    fn retention_round(s: &mut Store, index: u32) -> ConnectionDiagnostic {
        let mut d = diagnostic(&format!("retention-{index:06}"));
        d.started_at_ms = 100 + u64::from(index) * 100;
        s.connection_begin_diagnostic(&d, None).unwrap();
        retention_succeed(&mut d);
        d.observed_at_ms = Some(d.started_at_ms);
        d.finished_at_ms = Some(d.started_at_ms + 20);
        d.valid_until_ms = Some(d.started_at_ms + 60_000);
        s.connection_record_diagnostic(&d).unwrap();
        d
    }
    fn retention_store() -> Store {
        let mut s = store();
        s.connection_register("link", &"1".repeat(64), &link(&s), None, None)
            .unwrap();
        s
    }
    #[test]
    fn capacity_negative_survives_archive_reopen_and_legacy_bytes_are_unchanged() {
        let mut s = retention_store();
        let mut d = diagnostic("capacity-negative");
        s.connection_begin_diagnostic(&d, None).unwrap();
        retention_succeed(&mut d);
        d.schema_version = 2;
        d.capacity=Some(serde_json::from_value(json!({"schemaVersion":1,"source":"codex-account-rate-limits","connectionId":"registered","connectionRevision":1,"runtimeVersion":d.descriptor.as_ref().unwrap()["runtimeVersion"],"observedAtMs":100,"synthetic":true,"ordinaryUsage":{"state":"reported","value":false},"bucketView":"not-reported","buckets":[],"legacyBucket":null,"completeness":"complete","coverage":"reported-subset","issues":[],"capacityGroupIds":[]})).unwrap());
        s.connection_record_diagnostic(&d).unwrap();
        let raw = |s: &Store, id: &str| {
            s.connection
                .query_row(
                    "SELECT body,digest FROM connection_diagnostic_runs WHERE id=?1 AND ordinal=2",
                    [id],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                )
                .unwrap()
        };
        let (original, hash) = raw(&s, &d.id);
        assert_eq!(
            s.connection_capacity_denied_at("registered", 1).unwrap(),
            Some(100)
        );
        assert_eq!(
            s.connection_capacity_denied_at("registered", 2).unwrap(),
            None
        );
        let detailed = s
            .connection_diagnostics_run_at("registered", Some(&d.id), 200)
            .unwrap();
        assert_eq!(
            detailed["latest"]["capacity"]["ordinaryUsage"]["value"],
            false
        );
        let compact = s.connection_list_at(200).unwrap();
        assert!(!compact.to_string().contains("ordinaryUsage"));
        for i in 1..280 {
            retention_round(&mut s, i);
        }
        let path = s.path().to_owned();
        drop(s);
        let s = Store::open(path).unwrap();
        let exact = s
            .connection_diagnostic_observation("registered", &d.id, Some(2))
            .unwrap();
        assert_eq!(exact["archived"], true);
        assert_eq!(exact["digest"], hash);
        assert_eq!(
            exact["diagnostic"]["capacity"]["ordinaryUsage"]["value"],
            false
        );
        assert_eq!(raw(&s, &d.id), (original, hash));
        assert_eq!(
            s.connection_capacity_denied_at("registered", 1).unwrap(),
            Some(100)
        );
        let page = s
            .connection_diagnostic_history("registered", None, 20)
            .unwrap();
        assert!(!page.to_string().contains("ordinaryUsage"));
        let before = raw(&s, "retention-000001");
        assert!(!before.0.contains("capacity"));
        s.connection
            .pragma_update(None, "user_version", 10)
            .unwrap();
        s.connection
            .execute_batch("DROP INDEX connection_capacity_denied")
            .unwrap();
        let path = s.path().to_owned();
        drop(s);
        let s = Store::open(path).unwrap();
        assert_eq!(raw(&s, "retention-000001"), before);
        assert_eq!(
            s.connection_capacity_denied_at("registered", 1).unwrap(),
            Some(100)
        );
    }
    #[test]
    fn retention_rotates_beyond_old_cap_and_keeps_exact_oldest_bytes_and_authority() {
        let mut s = retention_store();
        let started = std::time::Instant::now();
        let first = retention_round(&mut s, 0);
        let original: (String, String) = s
            .connection
            .query_row(
                "SELECT body,digest FROM connection_diagnostic_runs WHERE id=?1 AND ordinal=2",
                [&first.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        for i in 1..400 {
            retention_round(&mut s, i);
            let stats = s.connection_diagnostic_storage("registered").unwrap();
            assert!(stats["operationalRows"].as_u64().unwrap() <= 512);
            assert_eq!(stats["reservedBytes"], 0);
        }
        let stats = s.connection_diagnostic_storage("registered").unwrap();
        assert_eq!(stats["totalRows"], 800);
        assert!(stats["archivedRows"].as_u64().unwrap() > 0);
        let old = s
            .connection_diagnostic_observation("registered", &first.id, Some(2))
            .unwrap();
        assert_eq!(old["archived"], true);
        assert_eq!(old["digest"], original.1);
        assert_eq!(old["diagnostic"]["observedAtMs"], 100);
        assert_eq!(old["diagnostic"]["current"], false);
        assert_eq!(old["grantsCurrentAuthority"], false);
        let unchanged: (String, String) = s
            .connection
            .query_row(
                "SELECT body,digest FROM connection_diagnostic_runs WHERE id=?1 AND ordinal=2",
                [&first.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(original, unchanged);
        assert_eq!(
            s.connection_link("registered")
                .unwrap()
                .unwrap()
                .baseline_observation,
            first.observation_fingerprint
        );
        let path = s.path().to_owned();
        drop(s);
        let reopened = Store::open(path).unwrap();
        reopened.connection_validate_history().unwrap();
        assert_eq!(
            reopened
                .connection_diagnostic_storage("registered")
                .unwrap(),
            stats
        );
        assert_eq!(
            reopened
                .connection_diagnostic_observation("registered", &first.id, Some(2))
                .unwrap(),
            old
        );
        eprintln!(
            "retention mass+oldest+reopen elapsed {:?}",
            started.elapsed()
        );
    }
    #[test]
    fn retention_pagination_high_water_survives_other_window_arrivals_and_rejects_foreign_cursor() {
        let mut s = retention_store();
        for i in 0..25 {
            retention_round(&mut s, i);
        }
        let first = s
            .connection_diagnostic_history("registered", None, 7)
            .unwrap();
        let watermark = first["highWaterSequence"].as_u64().unwrap();
        let mut other = Store::open(s.path()).unwrap();
        retention_round(&mut other, 25);
        let mut seen = Vec::new();
        let mut page = first;
        loop {
            assert!(serde_json::to_vec(&page).unwrap().len() <= 65536);
            assert!(page["items"].as_array().unwrap().len() <= 7);
            for item in page["items"].as_array().unwrap() {
                let sequence = item["sequence"].as_u64().unwrap();
                assert!(sequence <= watermark);
                seen.push(sequence);
            }
            let Some(cursor) = page["nextCursor"].as_str() else {
                break;
            };
            page = s
                .connection_diagnostic_history("registered", Some(cursor), 7)
                .unwrap();
        }
        assert_eq!(seen.len(), 50);
        assert!(seen.windows(2).all(|v| v[0] > v[1]));
        let latest = s
            .connection_diagnostic_history("registered", None, 7)
            .unwrap();
        assert!(latest["highWaterSequence"].as_u64().unwrap() > watermark);
        let mut cursor: Value =
            serde_json::from_str(latest["nextCursor"].as_str().unwrap()).unwrap();
        cursor["highWater"] = json!(u64::MAX);
        assert!(
            s.connection_diagnostic_history("registered", Some(&cursor.to_string()), 7)
                .is_err()
        );
        let foreign = retention_store();
        assert!(
            foreign
                .connection_diagnostic_history("registered", latest["nextCursor"].as_str(), 7)
                .is_err()
        );
        assert!(
            s.connection_diagnostic_history("other", latest["nextCursor"].as_str(), 7)
                .is_err()
        );
        assert!(
            s.connection_diagnostic_history("registered", None, 21)
                .is_err()
        );
    }
    #[test]
    fn retention_archive_never_consumes_active_terminal_reservation_and_rollback_is_atomic() {
        let mut s = retention_store();
        for i in 0..191 {
            retention_round(&mut s, i);
        }
        let mut active = diagnostic("active-retention");
        s.connection_begin_diagnostic(&active, None).unwrap();
        let stats = s.connection_diagnostic_storage("registered").unwrap();
        assert_eq!(stats["reservedTerminalRows"], 1);
        assert_eq!(stats["operationalRows"], 383);
        retention_round(&mut s, 191);
        s.connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        let before = s.connection_diagnostic_storage("registered").unwrap();
        history::maintain(&s.connection, "registered").unwrap();
        s.connection.execute_batch("ROLLBACK").unwrap();
        assert_eq!(
            s.connection_diagnostic_storage("registered").unwrap(),
            before
        );
        s.connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert_eq!(history::maintain(&s.connection, "registered").unwrap(), 128);
        s.connection.execute_batch("COMMIT").unwrap();
        assert_eq!(
            s.connection_diagnostic_observation("registered", &active.id, None)
                .unwrap()["archived"],
            false
        );
        active.state = "cancelled".into();
        active.finished_at_ms = Some(130);
        active.error_code = Some("cancelled-by-user".into());
        s.connection_record_diagnostic(&active).unwrap();
        assert_eq!(
            s.connection_diagnostic_storage("registered").unwrap()["reservedTerminalRows"],
            0
        );
        s.connection_validate_history().unwrap();
        let another = diagnostic("crash-retention");
        s.connection_begin_diagnostic(&another, None).unwrap();
        let path = s.path().to_owned();
        drop(s);
        let mut reopened = Store::open(path).unwrap();
        reopened.connection_validate_history().unwrap();
        reopened.connection_interrupt_pending().unwrap();
        reopened.connection_interrupt_pending().unwrap();
        assert_eq!(
            reopened
                .connection_diagnostic_storage("registered")
                .unwrap()["reservedTerminalRows"],
            0
        );
        assert_eq!(
            reopened
                .connection_diagnostic_observation("registered", &another.id, None)
                .unwrap()["diagnostic"]["state"],
            "interrupted"
        );
    }
    #[test]
    fn retention_legacy_empty_near_and_at_old_limit_migrate_without_rewriting_evidence() {
        for rounds in [0, 255, 256] {
            let mut s = retention_store();
            for i in 0..rounds {
                retention_round(&mut s, i);
            }
            let before: Vec<(String, String)> = s
                .connection
                .prepare("SELECT body,digest FROM connection_diagnostic_runs ORDER BY rowid")
                .unwrap()
                .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            let path = s.path().to_owned();
            drop(s);
            let c = Connection::open(&path).unwrap();
            c.execute_batch("DROP INDEX connection_capacity_denied;DROP TABLE connection_diagnostic_reservations;DROP TABLE connection_diagnostic_history;DROP TABLE connection_diagnostic_storage;PRAGMA user_version=9;").unwrap();
            drop(c);
            let mut migrated = Store::open(path).unwrap();
            migrated.connection_validate_history().unwrap();
            assert_eq!(
                migrated
                    .connection_diagnostic_storage("registered")
                    .unwrap()["operationalRows"],
                rounds * 2
            );
            assert!(
                migrated
                    .connection_diagnostic_remaining("registered")
                    .unwrap()
                    >= 2
            );
            retention_round(&mut migrated, rounds);
            let after: Vec<(String, String)> = migrated
                .connection
                .prepare(
                    "SELECT body,digest FROM connection_diagnostic_runs ORDER BY rowid LIMIT ?1",
                )
                .unwrap()
                .query_map([rounds * 2], |r| Ok((r.get(0)?, r.get(1)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert_eq!(before, after);
            assert!(
                migrated
                    .connection_diagnostic_storage("registered")
                    .unwrap()["operationalRows"]
                    .as_u64()
                    .unwrap()
                    <= 512
            );
            migrated.connection_validate_history().unwrap();
        }
    }
    #[test]
    fn retention_total_budget_blocks_new_read_but_terminal_and_local_history_remain_available() {
        for (connection_budget, global_budget) in [(1_048_576, 4_194_304), (4_194_304, 1_048_576)] {
            history::with_budget(connection_budget, global_budget, || {
                let mut s = retention_store();
                let mut count = 0;
                while s.connection_diagnostic_remaining("registered").unwrap() >= 2 {
                    retention_round(&mut s, count);
                    count += 1;
                    assert!(count < 200);
                }
                assert!(count > 1);
                let before = s.connection_diagnostic_storage("registered").unwrap();
                assert_eq!(before["storageBlocked"], true);
                assert_eq!(
                    before["blockedReason"],
                    "diagnostic-history-storage-budget-exhausted"
                );
                let failed = diagnostic("budget-blocked");
                assert_eq!(
                    s.connection_begin_diagnostic(&failed, None).unwrap_err(),
                    StoreError::Configuration
                );
                assert_eq!(
                    s.connection_diagnostic_storage("registered").unwrap(),
                    before
                );
                assert!(
                    s.connection_diagnostic_history("registered", None, 20)
                        .is_ok()
                );
                assert!(
                    s.connection_diagnostic_observation("registered", "retention-000000", Some(2))
                        .is_ok()
                );
                s.connection_interrupt_pending().unwrap();
                s.connection_validate_history().unwrap();
                let path = s.path().to_owned();
                drop(s);
                let reopened = Store::open(path).unwrap();
                reopened.connection_validate_history().unwrap();
                assert_eq!(
                    reopened
                        .connection_diagnostic_storage("registered")
                        .unwrap(),
                    before
                );
            });
        }
        let mut s = retention_store();
        let mut d = diagnostic("reserved-before-budget");
        s.connection_begin_diagnostic(&d, None).unwrap();
        history::with_budget(1, 1, || {
            d.state = "failed".into();
            d.finished_at_ms = Some(120);
            d.error_code = Some("metadata-unavailable".into());
            s.connection_record_diagnostic(&d).unwrap();
            assert_eq!(
                s.connection_diagnostic_storage("registered").unwrap()["reservedTerminalRows"],
                0
            );
            s.connection_interrupt_pending().unwrap();
            assert!(
                s.connection_diagnostic_observation("registered", &d.id, None)
                    .is_ok()
            );
        });
    }
    #[test]
    fn retention_pending_identity_and_last_success_survive_archived_failures_and_reopen() {
        let mut s = retention_store();
        let first = retention_round(&mut s, 0);
        let mut changed = diagnostic("identity-change-retained");
        s.connection_begin_diagnostic(&changed, None).unwrap();
        retention_succeed(&mut changed);
        changed.identity_changed = true;
        changed.observed_at_ms = Some(110);
        changed.valid_until_ms = Some(60110);
        changed.observation_fingerprint = Some("d".repeat(64));
        s.connection_record_diagnostic(&changed).unwrap();
        for i in 1_u32..300 {
            let mut failed = diagnostic(&format!("failure-retention-{i:06}"));
            failed.started_at_ms = 100 + u64::from(i) * 100;
            s.connection_begin_diagnostic(&failed, None).unwrap();
            failed.state = "failed".into();
            failed.finished_at_ms = Some(failed.started_at_ms + 20);
            failed.error_code = Some("metadata-unavailable".into());
            s.connection_record_diagnostic(&failed).unwrap();
        }
        assert_eq!(
            s.connection_diagnostic_observation("registered", &changed.id, None)
                .unwrap()["archived"],
            true
        );
        assert!(s.connection_identity_changed("registered", 1).unwrap());
        assert_eq!(
            s.connection_link("registered")
                .unwrap()
                .unwrap()
                .baseline_observation,
            first.observation_fingerprint
        );
        let report = s.connection_diagnostics("registered").unwrap();
        assert_eq!(report["latest"]["state"], "failed");
        assert_eq!(report["lastSuccessfulAtMs"], 110);
        assert_eq!(report["identityChanged"], true);
        let path = s.path().to_owned();
        drop(s);
        let reopened = Store::open(path).unwrap();
        reopened.connection_validate_history().unwrap();
        assert_eq!(
            reopened.connection_diagnostics("registered").unwrap()["identityChanged"],
            true
        );
    }
    #[test]
    fn retention_projection_corruption_is_rejected_on_exact_read_and_startup_without_repair() {
        let mut s = retention_store();
        let d = retention_round(&mut s, 0);
        s.connection.execute_batch("DROP TRIGGER diagnostic_projection_immutable;UPDATE connection_diagnostic_history SET registration_revision=2 WHERE ordinal=2").unwrap();
        assert_eq!(
            s.connection_diagnostic_observation("registered", &d.id, Some(2))
                .unwrap_err(),
            StoreError::InvalidState
        );
        assert_eq!(
            s.connection_validate_history().unwrap_err(),
            StoreError::InvalidState
        );
    }
    #[test]
    fn retention_start_terminal_and_archive_rollback_leave_no_half_visible_round() {
        let mut s = retention_store();
        for i in 0..192 {
            retention_round(&mut s, i);
        }
        let before = s.connection_diagnostic_storage("registered").unwrap();
        let d = diagnostic("rollback-retention");
        s.connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        append_diagnostic(&s.connection, &d, true).unwrap();
        let in_transaction = s.connection_diagnostic_storage("registered").unwrap();
        assert!(
            in_transaction["archivedRows"].as_u64().unwrap()
                > before["archivedRows"].as_u64().unwrap()
        );
        s.connection.execute_batch("ROLLBACK").unwrap();
        assert_eq!(
            s.connection_diagnostic_storage("registered").unwrap(),
            before
        );
        assert!(
            s.connection_diagnostic_observation("registered", &d.id, None)
                .is_err()
        );
        s.connection_validate_history().unwrap();
    }
    #[test]
    fn retention_archived_cache_cohort_keeps_short_original_lease_after_preferences_expand() {
        let mut s = retention_store();
        let mut settings = RefreshSettings::default();
        settings.validity_ms.models = 1000;
        s.connection_configure_refresh(
            "shorten-retained",
            &"2".repeat(64),
            "registered",
            1,
            0,
            &settings,
            90,
        )
        .unwrap();
        let original = retention_round(&mut s, 0);
        s.connection_configure_refresh(
            "expand-retained",
            &"3".repeat(64),
            "registered",
            1,
            1,
            &RefreshSettings::default(),
            110,
        )
        .unwrap();
        for i in 1..250 {
            retention_round(&mut s, i);
        }
        let mut cached = diagnostic("cached-old-retained");
        cached.started_at_ms = 130;
        s.connection_begin_diagnostic(&cached, None).unwrap();
        retention_succeed(&mut cached);
        cached.finished_at_ms = Some(140);
        cached.cached = true;
        s.connection_record_diagnostic(&cached).unwrap();
        let old = s
            .connection_diagnostic_observation("registered", &original.id, Some(2))
            .unwrap();
        assert_eq!(old["archived"], true);
        assert_eq!(old["validity"]["captured"]["models"], 1000);
        let actual = s
            .connection_diagnostic_observation("registered", &cached.id, Some(2))
            .unwrap();
        assert_eq!(actual["validity"]["captured"]["models"], 60000);
        assert_eq!(actual["diagnostic"]["effectiveValidityMs"]["models"], 1000);
        assert_eq!(actual["diagnostic"]["effectiveValidUntilMs"], 1100);
        assert_eq!(actual["diagnostic"]["observedAtMs"], 100);
        assert_eq!(actual["diagnostic"]["validUntilMs"], 60100);
        assert_eq!(actual["diagnostic"]["cached"], true);
        assert_eq!(actual["diagnostic"]["provenance"], "synthetic");
        let path = s.path().to_owned();
        drop(s);
        let reopened = Store::open(path).unwrap();
        reopened.connection_validate_history().unwrap();
        assert_eq!(
            reopened
                .connection_diagnostic_observation("registered", &cached.id, Some(2))
                .unwrap(),
            actual
        );
    }
    #[test]
    fn retention_operational_queries_seek_indexes_without_sorting_or_decoding_history() {
        let mut s = retention_store();
        for i in 0..5 {
            retention_round(&mut s, i);
        }
        for sql in [
            "WHERE h.connection_id='registered' AND h.binding_revision=1 AND h.state IN('complete','partial') ORDER BY h.observed,h.id,h.ordinal LIMIT 1",
            "WHERE h.connection_id='registered' AND h.state IN('complete','partial') ORDER BY h.observed DESC,h.id DESC,h.ordinal DESC LIMIT 1",
            "WHERE h.connection_id='registered' AND h.binding_revision=1 AND h.generation=0 AND h.purpose='registration' AND h.state IN('complete','partial') AND h.observed<=1000 AND source_until>1000 ORDER BY h.observed DESC,h.id DESC LIMIT 1",
            "WHERE h.connection_id='registered' ORDER BY h.started DESC,h.finished DESC,h.id DESC,h.ordinal DESC LIMIT 1",
            "WHERE h.connection_id='registered' AND h.sequence<=100 AND h.sequence<90 ORDER BY h.sequence DESC LIMIT 21",
        ] {
            let plan: Vec<String> = s
                .connection
                .prepare(&format!("EXPLAIN QUERY PLAN {} {sql}", history::SELECT))
                .unwrap()
                .query_map([], |r| r.get(3))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert!(
                plan.iter()
                    .any(|line| line.contains("SEARCH") && line.contains("INDEX")),
                "{plan:?}"
            );
            assert!(
                plan.iter().all(|line| !line.contains("TEMP B-TREE")
                    && !line.contains("SCAN connection_diagnostic_history")),
                "{plan:?}"
            );
        }
    }
    #[test]
    fn retention_stable_sequence_and_cursor_survive_sqlite_vacuum() {
        let mut s = retention_store();
        for i in 0..5 {
            retention_round(&mut s, i);
        }
        let first = s
            .connection_diagnostic_history("registered", None, 3)
            .unwrap();
        let cursor = first["nextCursor"].as_str().unwrap().to_owned();
        let expected = s
            .connection_diagnostic_history("registered", Some(&cursor), 3)
            .unwrap();
        s.connection.execute_batch("VACUUM").unwrap();
        assert_eq!(
            s.connection_diagnostic_history("registered", Some(&cursor), 3)
                .unwrap(),
            expected
        );
        retention_round(&mut s, 5);
        s.connection_validate_history().unwrap();
        assert_eq!(
            s.connection_diagnostic_history("registered", Some(&cursor), 3)
                .unwrap()["items"],
            expected["items"]
        );
    }
    #[test]
    fn retention_large_catalog_stays_bounded_in_pages_and_exact_details() {
        let mut s = retention_store();
        let mut d = diagnostic("large-retention");
        s.connection_begin_diagnostic(&d, None).unwrap();
        retention_succeed(&mut d);
        let descriptor = d.descriptor.as_mut().unwrap();
        let cap = json!({"support":"supported","availability":"available","configuration":"enabled","evidence":{"source":"synthetic","scope":"model","reference":"r".repeat(80),"observedAt":"2026-10-13T00:00:00.000Z"}});
        let mut caps = serde_json::Map::new();
        for key in crate::runtime_contract::CAPABILITIES {
            caps.insert(key.into(), cap.clone());
        }
        descriptor["models"]=Value::Array((0..50).map(|i|json!({"id":format!("model-{i:02}-{}", "m".repeat(71)),"availability":"available","reasoningLevels":["low","medium","high"],"capabilities":caps.clone()})).collect());
        d.catalog_labels = (0..50)
            .map(|i| crate::runtime::RuntimeModelPresentation {
                id: format!("model-{i:02}-{}", "m".repeat(71)),
                label: "L".repeat(512),
                native_efforts: vec!["high".into()],
            })
            .collect();
        let body = encode(&d).unwrap();
        assert!(body.len() > 100_000);
        assert!(body.len() < history::MAX_BODY_BYTES);
        s.connection_record_diagnostic(&d).unwrap();
        let page = s
            .connection_diagnostic_history("registered", None, 20)
            .unwrap();
        assert!(serde_json::to_vec(&page).unwrap().len() < 65_536);
        assert!(page["items"][0]["diagnostic"]["descriptor"].is_null());
        let detail = s
            .connection_diagnostic_observation("registered", &d.id, Some(2))
            .unwrap();
        assert!(serde_json::to_vec(&detail).unwrap().len() < 524_288);
        assert_eq!(
            detail["diagnostic"]["descriptor"]["models"]
                .as_array()
                .unwrap()
                .len(),
            50
        );
        assert_eq!(detail["bodyBytes"], body.len());
        let mut oversized = diagnostic("oversized-retention");
        s.connection_begin_diagnostic(&oversized, None).unwrap();
        retention_succeed(&mut oversized);
        oversized.descriptor = d.descriptor.clone();
        oversized.catalog_labels = d.catalog_labels.clone();
        oversized.descriptor.as_mut().unwrap()["models"]=Value::Array((0..64).map(|i|json!({"id":format!("model-{i:02}-{}", "m".repeat(71)),"availability":"available","reasoningLevels":["low","medium","high"],"capabilities":caps.clone()})).collect());
        oversized.catalog_labels = (0..64)
            .map(|i| crate::runtime::RuntimeModelPresentation {
                id: format!("model-{i:02}-{}", "m".repeat(71)),
                label: "L".repeat(512),
                native_efforts: vec!["h".repeat(40); 8],
            })
            .collect();
        assert!(oversized.validate().is_ok());
        assert!(encode(&oversized).unwrap().len() > history::MAX_BODY_BYTES);
        assert_eq!(
            s.connection_record_diagnostic(&oversized).unwrap_err(),
            StoreError::Configuration
        );
        assert_eq!(
            s.connection_diagnostic_storage("registered").unwrap()["reservedTerminalRows"],
            1
        );
        oversized.state = "failed".into();
        oversized.complete = false;
        oversized.descriptor = None;
        oversized.catalog_labels.clear();
        oversized.observed_at_ms = None;
        oversized.valid_until_ms = None;
        oversized.error_code = Some("metadata-output-budget".into());
        s.connection_record_diagnostic(&oversized).unwrap();
        assert_eq!(
            s.connection_diagnostic_storage("registered").unwrap()["reservedTerminalRows"],
            0
        );
    }
    #[test]
    fn retention_archived_original_digest_corruption_is_explicit_and_not_repaired() {
        let mut s = retention_store();
        let d = retention_round(&mut s, 0);
        for i in 1..193 {
            retention_round(&mut s, i);
        }
        assert_eq!(
            s.connection_diagnostic_observation("registered", &d.id, Some(2))
                .unwrap()["archived"],
            true
        );
        s.connection.execute_batch("DROP TRIGGER connection_diagnostic_no_update;UPDATE connection_diagnostic_runs SET digest='bad' WHERE id='retention-000000' AND ordinal=2").unwrap();
        assert_eq!(
            s.connection_diagnostic_observation("registered", &d.id, Some(2))
                .unwrap_err(),
            StoreError::InvalidState
        );
        let path = s.path().to_owned();
        drop(s);
        assert!(matches!(Store::open(path), Err(StoreError::InvalidState)));
    }
    #[test]
    fn retention_legacy_full_with_multiple_pending_rounds_rotates_before_recovery_terminals() {
        let mut s = retention_store();
        for i in 0..255 {
            retention_round(&mut s, i);
        }
        for id in ["legacy-active-a", "legacy-active-b"] {
            s.connection_begin_diagnostic(&diagnostic(id), None)
                .unwrap();
        }
        let original: Vec<(String, String)> = s
            .connection
            .prepare("SELECT body,digest FROM connection_diagnostic_runs ORDER BY id,ordinal")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(original.len(), 512);
        let path = s.path().to_owned();
        drop(s);
        let c = Connection::open(&path).unwrap();
        c.execute_batch("DROP INDEX connection_capacity_denied;DROP TABLE connection_diagnostic_reservations;DROP TABLE connection_diagnostic_history;DROP TABLE connection_diagnostic_storage;PRAGMA user_version=9;").unwrap();
        drop(c);
        let mut migrated = Store::open(&path).unwrap();
        assert_eq!(
            migrated
                .connection_diagnostic_storage("registered")
                .unwrap()["operationalRows"],
            512
        );
        assert_eq!(
            migrated
                .connection_diagnostic_storage("registered")
                .unwrap()["reservedTerminalRows"],
            2
        );
        migrated.connection_interrupt_pending().unwrap();
        let stats = migrated
            .connection_diagnostic_storage("registered")
            .unwrap();
        assert_eq!(stats["totalRows"], 514);
        assert!(stats["operationalRows"].as_u64().unwrap() <= 512);
        assert_eq!(stats["reservedTerminalRows"], 0);
        assert_eq!(stats["archivedRows"], 128);
        for id in ["legacy-active-a", "legacy-active-b"] {
            assert_eq!(
                migrated
                    .connection_diagnostic_observation("registered", id, None)
                    .unwrap()["diagnostic"]["state"],
                "interrupted"
            );
        }
        let unchanged:Vec<(String,String)>=migrated.connection.prepare("SELECT body,digest FROM connection_diagnostic_runs WHERE NOT(id IN('legacy-active-a','legacy-active-b') AND ordinal=2) ORDER BY id,ordinal").unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().collect::<Result<_,_>>().unwrap();
        assert_eq!(unchanged, original);
        drop(migrated);
        let reopened = Store::open(path).unwrap();
        assert_eq!(
            reopened
                .connection_diagnostic_storage("registered")
                .unwrap(),
            stats
        );
    }
}
