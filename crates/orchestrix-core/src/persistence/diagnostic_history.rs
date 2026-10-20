//! Bounded operational projection over immutable diagnostic evidence in the same SQLite store.
use super::{Store, StoreError, StoreResult};
use crate::connections::ConnectionDiagnostic;
use rusqlite::{Connection, OptionalExtension, Params, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub(super) const OPERATIONAL_ROWS: u64 = 512;
pub(super) const ROTATE_AT: u64 = 384;
pub(super) const MAX_BODY_BYTES: usize = crate::connections::MAX_DIAGNOSTIC_BODY_BYTES;
pub(super) const TERMINAL_RESERVATION: u64 = MAX_BODY_BYTES as u64 + 64;
const ENVELOPE_ALLOWANCE: u64 = 4096;
const CONNECTION_BYTES: u64 = 256 * 1024 * 1024;
const GLOBAL_BYTES: u64 = 1024 * 1024 * 1024;
const PAGE_BYTES: usize = 65_536;
const DETAIL_BYTES: usize = 524_288;

#[cfg(test)]
thread_local! {static TEST_BUDGET:std::cell::Cell<Option<(u64,u64)>>=const{std::cell::Cell::new(None)};}
fn budgets() -> (u64, u64) {
    #[cfg(test)]
    if let Some(value) = TEST_BUDGET.with(|v| v.get()) {
        return value;
    }
    (CONNECTION_BYTES, GLOBAL_BYTES)
}
#[cfg(test)]
pub(super) fn with_budget<T>(connection: u64, global: u64, run: impl FnOnce() -> T) -> T {
    struct Reset(Option<(u64, u64)>);
    impl Drop for Reset {
        fn drop(&mut self) {
            TEST_BUDGET.with(|v| v.set(self.0));
        }
    }
    let prior = TEST_BUDGET.with(|v| v.replace(Some((connection, global))));
    let _reset = Reset(prior);
    run()
}

pub(super) const MIGRATION_V10: &str = r#"
CREATE TABLE connection_diagnostic_history(
 sequence INTEGER PRIMARY KEY AUTOINCREMENT, id TEXT NOT NULL, ordinal INTEGER NOT NULL,
 connection_id TEXT NOT NULL, binding_revision INTEGER NOT NULL, registration_revision INTEGER NOT NULL,
 state TEXT NOT NULL, started INTEGER NOT NULL, finished INTEGER, observed INTEGER, scope TEXT,
 identity_changed INTEGER NOT NULL, generation INTEGER NOT NULL, purpose TEXT NOT NULL, source_until INTEGER, archived INTEGER NOT NULL DEFAULT 0 CHECK(archived IN(0,1)),
 body_bytes INTEGER NOT NULL, evidence_bytes INTEGER NOT NULL, digest TEXT NOT NULL,
 UNIQUE(id,ordinal), FOREIGN KEY(id,ordinal) REFERENCES connection_diagnostic_runs(id,ordinal));
CREATE INDEX diagnostic_history_scope ON connection_diagnostic_history(connection_id,sequence DESC);
CREATE INDEX diagnostic_history_latest ON connection_diagnostic_history(connection_id,started DESC,finished DESC,id DESC,ordinal DESC);
CREATE INDEX diagnostic_history_success ON connection_diagnostic_history(connection_id,observed DESC,id DESC,ordinal DESC) WHERE state IN('complete','partial');
CREATE INDEX diagnostic_history_baseline ON connection_diagnostic_history(connection_id,binding_revision,observed,id,ordinal) WHERE state IN('complete','partial');
CREATE INDEX diagnostic_history_registration ON connection_diagnostic_history(connection_id,binding_revision,generation,purpose,observed DESC,id DESC) WHERE state IN('complete','partial');
CREATE INDEX diagnostic_history_identity ON connection_diagnostic_history(connection_id,binding_revision,identity_changed);
CREATE INDEX diagnostic_history_cohort ON connection_diagnostic_history(connection_id,binding_revision,scope,observed,state);
CREATE INDEX diagnostic_history_operational ON connection_diagnostic_history(connection_id,archived,ordinal,sequence);
CREATE TABLE connection_diagnostic_storage(connection_id TEXT PRIMARY KEY,operational_rows INTEGER NOT NULL,archived_rows INTEGER NOT NULL,used_bytes INTEGER NOT NULL,completed_rounds INTEGER NOT NULL);
CREATE TABLE connection_diagnostic_reservations(id TEXT PRIMARY KEY,connection_id TEXT NOT NULL,bytes INTEGER NOT NULL);
CREATE INDEX diagnostic_reservation_scope ON connection_diagnostic_reservations(connection_id);
INSERT INTO connection_diagnostic_history
 SELECT rowid,id,ordinal,connection_id,connection_revision,registration_revision,json_extract(body,'$.state'),
 json_extract(body,'$.startedAtMs'),json_extract(body,'$.finishedAtMs'),json_extract(body,'$.observedAtMs'),
 json_extract(body,'$.scopeKey'),json_extract(body,'$.identityChanged'),json_extract(body,'$.daemonGeneration'),json_extract(body,'$.purpose'),json_extract(body,'$.validUntilMs'),0,length(CAST(body AS BLOB)),
 length(CAST(body AS BLOB))+length(digest)+CASE WHEN ordinal=1 THEN 4096 ELSE 0 END,digest FROM connection_diagnostic_runs;
INSERT INTO connection_diagnostic_storage SELECT connection_id,count(*),0,sum(evidence_bytes),sum(CASE WHEN ordinal=2 THEN 1 ELSE 0 END) FROM connection_diagnostic_history GROUP BY connection_id;
INSERT INTO connection_diagnostic_reservations SELECT h.id,h.connection_id,262208 FROM connection_diagnostic_history h
 WHERE h.state='running' AND NOT EXISTS(SELECT 1 FROM connection_diagnostic_history p WHERE p.id=h.id AND p.ordinal>h.ordinal);
CREATE TRIGGER diagnostic_projection_no_delete BEFORE DELETE ON connection_diagnostic_history BEGIN SELECT RAISE(ABORT,'retained diagnostic projection'); END;
CREATE TRIGGER diagnostic_projection_immutable BEFORE UPDATE ON connection_diagnostic_history
 WHEN OLD.sequence IS NOT NEW.sequence OR OLD.id IS NOT NEW.id OR OLD.ordinal IS NOT NEW.ordinal OR OLD.connection_id IS NOT NEW.connection_id
 OR OLD.binding_revision IS NOT NEW.binding_revision OR OLD.registration_revision IS NOT NEW.registration_revision OR OLD.state IS NOT NEW.state
 OR OLD.started IS NOT NEW.started OR OLD.finished IS NOT NEW.finished OR OLD.observed IS NOT NEW.observed OR OLD.scope IS NOT NEW.scope
 OR OLD.identity_changed IS NOT NEW.identity_changed OR OLD.generation IS NOT NEW.generation OR OLD.purpose IS NOT NEW.purpose OR OLD.source_until IS NOT NEW.source_until OR OLD.body_bytes IS NOT NEW.body_bytes OR OLD.evidence_bytes IS NOT NEW.evidence_bytes OR OLD.digest IS NOT NEW.digest
 OR OLD.archived>NEW.archived OR (NEW.archived=1 AND NOT EXISTS(SELECT 1 FROM connection_diagnostic_runs terminal WHERE terminal.id=OLD.id AND terminal.ordinal=2)) BEGIN SELECT RAISE(ABORT,'immutable diagnostic projection'); END;
CREATE TRIGGER diagnostic_projection_insert AFTER INSERT ON connection_diagnostic_history BEGIN
 INSERT INTO connection_diagnostic_storage VALUES(NEW.connection_id,1,0,NEW.evidence_bytes,CASE WHEN NEW.ordinal=2 THEN 1 ELSE 0 END)
 ON CONFLICT(connection_id) DO UPDATE SET operational_rows=operational_rows+1,used_bytes=used_bytes+NEW.evidence_bytes,completed_rounds=completed_rounds+CASE WHEN NEW.ordinal=2 THEN 1 ELSE 0 END;
 END;
CREATE TRIGGER diagnostic_projection_archive AFTER UPDATE OF archived ON connection_diagnostic_history WHEN OLD.archived=0 AND NEW.archived=1 BEGIN
 UPDATE connection_diagnostic_storage SET operational_rows=operational_rows-1,archived_rows=archived_rows+1 WHERE connection_id=NEW.connection_id;
 END;
"#;

fn integer(value: u64) -> StoreResult<i64> {
    i64::try_from(value).map_err(|_| StoreError::InvalidState)
}
fn unsigned(r: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let value: i64 = r.get(index)?;
    u64::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}
fn hash(body: &str) -> String {
    crate::git_records::content_hash(body.as_bytes())
}
fn projection(d: &ConnectionDiagnostic) -> Value {
    json!([
        d.id,
        d.connection_id,
        d.connection_revision,
        d.registration_revision,
        d.state,
        d.started_at_ms,
        d.finished_at_ms,
        d.observed_at_ms,
        d.scope_key,
        d.identity_changed as u8,
        d.daemon_generation,
        d.purpose,
        d.valid_until_ms
    ])
}
pub(super) const SELECT: &str = "SELECT h.sequence,d.id,d.ordinal,d.connection_id,d.connection_revision,d.registration_revision,d.body,d.digest,h.archived,h.body_bytes,h.evidence_bytes,h.digest,json_array(h.id,h.connection_id,h.binding_revision,h.registration_revision,h.state,h.started,h.finished,h.observed,h.scope,h.identity_changed,h.generation,h.purpose,h.source_until) FROM connection_diagnostic_runs d JOIN connection_diagnostic_history h ON h.id=d.id AND h.ordinal=d.ordinal";

pub(super) struct Evidence {
    pub sequence: u64,
    pub ordinal: u32,
    pub archived: bool,
    pub digest: String,
    pub body_bytes: usize,
    pub diagnostic: ConnectionDiagnostic,
}
fn decode(r: &rusqlite::Row<'_>) -> rusqlite::Result<StoreResult<Evidence>> {
    let sequence: i64 = r.get(0)?;
    let id: String = r.get(1)?;
    let ordinal: i64 = r.get(2)?;
    let connection: String = r.get(3)?;
    let binding: i64 = r.get(4)?;
    let registration: i64 = r.get(5)?;
    let body: String = r.get(6)?;
    let digest: String = r.get(7)?;
    let archived: bool = r.get(8)?;
    let body_bytes: i64 = r.get(9)?;
    let evidence_bytes: i64 = r.get(10)?;
    let projection_digest: String = r.get(11)?;
    let projected: String = r.get(12)?;
    Ok((|| {
        if body.len() > MAX_BODY_BYTES
            || hash(&body) != digest
            || projection_digest != digest
            || sequence <= 0
            || !(1..=2).contains(&ordinal)
            || body_bytes != body.len() as i64
            || evidence_bytes
                != body_bytes
                    + 64
                    + if ordinal == 1 {
                        ENVELOPE_ALLOWANCE as i64
                    } else {
                        0
                    }
        {
            return Err(StoreError::InvalidState);
        }
        let diagnostic: ConnectionDiagnostic =
            serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
        diagnostic
            .validate()
            .map_err(|_| StoreError::InvalidState)?;
        if diagnostic.id != id
            || diagnostic.connection_id != connection
            || diagnostic.connection_revision != binding as u64
            || diagnostic.registration_revision != registration as u64
            || projection(&diagnostic)
                != serde_json::from_str::<Value>(&projected)
                    .map_err(|_| StoreError::InvalidState)?
        {
            return Err(StoreError::InvalidState);
        }
        Ok(Evidence {
            sequence: sequence as u64,
            ordinal: ordinal as u32,
            archived,
            digest,
            body_bytes: body.len(),
            diagnostic,
        })
    })())
}
pub(super) fn query<P: Params>(c: &Connection, suffix: &str, p: P) -> StoreResult<Vec<Evidence>> {
    let mut q = c.prepare(&format!("{SELECT} {suffix}"))?;
    q.query_map(p, decode)?
        .map(|r| r.map_err(StoreError::from).and_then(|v| v))
        .collect()
}

pub(super) fn exact(
    c: &Connection,
    connection: &str,
    id: &str,
    ordinal: Option<u32>,
) -> StoreResult<Option<Evidence>> {
    let rows = query(
        c,
        "WHERE h.connection_id=?1 AND h.id=?2 ORDER BY h.ordinal",
        params![connection, id],
    )?;
    let mut previous = None;
    for row in &rows {
        validate_transition(previous.as_ref(), &row.diagnostic, row.ordinal)?;
        previous = Some(row.diagnostic.clone());
    }
    Ok(rows
        .into_iter()
        .rev()
        .find(|r| ordinal.is_none_or(|v| r.ordinal == v)))
}
pub(super) fn validate_transition(
    previous: Option<&ConnectionDiagnostic>,
    d: &ConnectionDiagnostic,
    ordinal: u32,
) -> StoreResult<()> {
    if ordinal == 1 {
        if d.state != "running" || d.finished_at_ms.is_some() || d.observed_at_ms.is_some() {
            return Err(StoreError::InvalidState);
        }
    } else {
        let p = previous.ok_or(StoreError::InvalidState)?;
        if p.state != "running"
            || d.state == "running"
            || d.finished_at_ms.is_none()
            || p.id != d.id
            || p.connection_id != d.connection_id
            || p.started_at_ms != d.started_at_ms
            || p.connection_revision != d.connection_revision
            || p.registration_revision != d.registration_revision
            || p.synthetic != d.synthetic
            || p.daemon_generation != d.daemon_generation
            || p.purpose != d.purpose
        {
            return Err(StoreError::InvalidState);
        }
    }
    Ok(())
}
pub(super) fn validate(c: &Connection, id: &str) -> StoreResult<()> {
    let mut q = c.prepare(&format!(
        "{SELECT} WHERE d.connection_id=?1 ORDER BY d.id,d.ordinal"
    ))?;
    let mut previous: Option<ConnectionDiagnostic> = None;
    let mut count = 0_u64;
    let mut bytes = 0_u64;
    let mut operational = 0_u64;
    let mut completed = 0_u64;
    for row in q.query_map([id], decode)? {
        let e = row??;
        validate_transition(
            previous.as_ref().filter(|p| p.id == e.diagnostic.id),
            &e.diagnostic,
            e.ordinal,
        )?;
        previous = Some(e.diagnostic);
        count += 1;
        completed += u64::from(e.ordinal == 2);
        bytes += e.body_bytes as u64
            + 64
            + if e.ordinal == 1 {
                ENVELOPE_ALLOWANCE
            } else {
                0
            };
        operational += u64::from(!e.archived);
    }
    let original: i64 = c.query_row(
        "SELECT count(*) FROM connection_diagnostic_runs WHERE connection_id=?1",
        [id],
        |r| r.get(0),
    )?;
    let stats = storage(c, id)?;
    if count != original as u64
        || stats["totalRows"] != count
        || stats["operationalRows"] != operational
        || stats["usedBytes"] != bytes
        || stats["completedRounds"] != completed
    {
        return Err(StoreError::InvalidState);
    }
    let bad_reservations:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM connection_diagnostic_reservations r WHERE r.connection_id=?1 AND (r.bytes!=262208 OR NOT EXISTS(SELECT 1 FROM connection_diagnostic_history h WHERE h.id=r.id AND h.state='running') OR EXISTS(SELECT 1 FROM connection_diagnostic_history h WHERE h.id=r.id AND h.ordinal=2)))",[id],|r|r.get(0))?;
    let missing:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM connection_diagnostic_history h WHERE h.connection_id=?1 AND h.state='running' AND NOT EXISTS(SELECT 1 FROM connection_diagnostic_history p WHERE p.id=h.id AND p.ordinal=2) AND NOT EXISTS(SELECT 1 FROM connection_diagnostic_reservations r WHERE r.id=h.id))",[id],|r|r.get(0))?;
    if bad_reservations || missing {
        return Err(StoreError::InvalidState);
    }
    Ok(())
}
pub(super) fn maintain(c: &Connection, id: &str) -> StoreResult<u64> {
    let n:i64=c.query_row("SELECT coalesce((SELECT operational_rows FROM connection_diagnostic_storage WHERE connection_id=?1),0)",[id],|r|r.get(0))?;
    if n < ROTATE_AT as i64 {
        return Ok(0);
    }
    Ok(c.execute("UPDATE connection_diagnostic_history SET archived=1 WHERE connection_id=?1 AND archived=0 AND id IN(SELECT id FROM connection_diagnostic_history WHERE connection_id=?1 AND archived=0 AND ordinal=2 ORDER BY sequence LIMIT 64)",[id])? as u64)
}
pub(super) fn storage(c: &Connection, id: &str) -> StoreResult<Value> {
    crate::domain::ConnectionId::new(id)?;
    let (op,arch,used,completed):(u64,u64,u64,u64)=c.query_row("SELECT operational_rows,archived_rows,used_bytes,completed_rounds FROM connection_diagnostic_storage WHERE connection_id=?1",[id],|r|Ok((unsigned(r,0)?,unsigned(r,1)?,unsigned(r,2)?,unsigned(r,3)?))).optional()?.unwrap_or_default();
    let (slots,reserved):(u64,u64)=c.query_row("SELECT count(*),coalesce(sum(bytes),0) FROM connection_diagnostic_reservations WHERE connection_id=?1",[id],|r|Ok((unsigned(r,0)?,unsigned(r,1)?)))?;
    let global:u64=c.query_row("SELECT coalesce((SELECT sum(used_bytes) FROM connection_diagnostic_storage),0)+coalesce((SELECT sum(bytes) FROM connection_diagnostic_reservations),0)",[],|r|unsigned(r,0))?;
    let (connection_budget, global_budget) = budgets();
    let blocked = used
        .saturating_add(reserved)
        .saturating_add(TERMINAL_RESERVATION + ENVELOPE_ALLOWANCE + MAX_BODY_BYTES as u64 + 64)
        > connection_budget
        || global
            .saturating_add(TERMINAL_RESERVATION + ENVELOPE_ALLOWANCE + MAX_BODY_BYTES as u64 + 64)
            > global_budget;
    Ok(
        json!({"operationalRows":op,"archivedRows":arch,"totalRows":op+arch,"completedRounds":completed,"usedBytes":used,"reservedBytes":reserved,"reservedTerminalRows":slots,"operationalLimitRows":OPERATIONAL_ROWS,"maximumHistoryBytes":connection_budget,"globalUsedAndReservedBytes":global,"maximumGlobalHistoryBytes":global_budget,"maintenanceTriggerRows":ROTATE_AT,"maximumArchivedRoundsPerCycle":64,"pageLimit":20,"maximumPageBytes":PAGE_BYTES,"maximumDetailBytes":DETAIL_BYTES,"storageBlocked":blocked,"blockedReason":if blocked {Some("diagnostic-history-storage-budget-exhausted")} else {None},"nextAction":if blocked {Some("Keep history available; change the explicit storage budget in a supported future release before requesting new reads.")} else {None},"accounting":"UTF-8 evidence and digest bytes plus conservative 4096-byte per-round validity/index allowance; terminal reservation included"}),
    )
}
pub(super) fn remaining(c: &Connection, id: &str) -> StoreResult<u64> {
    let s = storage(c, id)?;
    if s["storageBlocked"] == true {
        return Ok(0);
    }
    let op = s["operationalRows"]
        .as_u64()
        .ok_or(StoreError::InvalidState)?;
    let slots = s["reservedTerminalRows"]
        .as_u64()
        .ok_or(StoreError::InvalidState)?;
    let reclaim:u64=c.query_row("SELECT count(*)*2 FROM (SELECT id FROM connection_diagnostic_history WHERE connection_id=?1 AND archived=0 AND ordinal=2 LIMIT 64)",[id],|r|unsigned(r,0))?;
    Ok(OPERATIONAL_ROWS
        .saturating_sub(op.saturating_sub(if op >= ROTATE_AT { reclaim } else { 0 }) + slots))
}
pub(super) fn insert(
    c: &Connection,
    d: &ConnectionDiagnostic,
    ordinal: u32,
    body: &str,
    digest: &str,
) -> StoreResult<()> {
    if ordinal == 2 {
        // A legacy full window did not reserve operational terminal rows. Reclaim
        // completed rounds before draining its reservations; history/bytes survive.
        let stats = storage(c, &d.connection_id)?;
        let occupied = stats["operationalRows"]
            .as_u64()
            .ok_or(StoreError::InvalidState)?
            .saturating_add(
                stats["reservedTerminalRows"]
                    .as_u64()
                    .ok_or(StoreError::InvalidState)?,
            );
        if occupied > OPERATIONAL_ROWS {
            maintain(c, &d.connection_id)?;
        }
    }
    if ordinal == 1 {
        maintain(c, &d.connection_id)?;
        if remaining(c, &d.connection_id)? < 2 {
            return Err(StoreError::Configuration);
        }
        // Admission reserves the largest supported terminal frame before any external read.
        c.execute(
            "INSERT INTO connection_diagnostic_reservations VALUES(?1,?2,?3)",
            params![d.id, d.connection_id, integer(TERMINAL_RESERVATION)?],
        )?;
    } else if c.execute(
        "DELETE FROM connection_diagnostic_reservations WHERE id=?1 AND connection_id=?2",
        params![d.id, d.connection_id],
    )? != 1
    {
        return Err(StoreError::InvalidState);
    }
    c.execute("INSERT INTO connection_diagnostic_history VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,0,?16,?17,?18)",params![Option::<i64>::None,d.id,ordinal,d.connection_id,integer(d.connection_revision)?,integer(d.registration_revision)?,d.state,integer(d.started_at_ms)?,d.finished_at_ms.map(integer).transpose()?,d.observed_at_ms.map(integer).transpose()?,d.scope_key,d.identity_changed,integer(d.daemon_generation)?,d.purpose,d.valid_until_ms.map(integer).transpose()?,body.len() as i64,integer(body.len() as u64+64+if ordinal==1 {ENVELOPE_ALLOWANCE}else{0})?,digest])?;
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cursor {
    version: u8,
    database_identity: String,
    connection_id: String,
    high_water: u64,
    before: u64,
}
impl Store {
    pub fn connection_diagnostic_storage(&self, id: &str) -> StoreResult<Value> {
        storage(&self.connection, id)
    }
    pub fn connection_diagnostic_history(
        &self,
        id: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> StoreResult<Value> {
        crate::domain::ConnectionId::new(id)?;
        if !(1..=20).contains(&limit) {
            return Err(StoreError::Configuration);
        }
        let identity: String = self.connection.query_row(
            "SELECT identity FROM daemon_meta WHERE singleton=1",
            [],
            |r| r.get(0),
        )?;
        let maximum:u64=self.connection.query_row("SELECT coalesce(max(sequence),0) FROM connection_diagnostic_history WHERE connection_id=?1",[id],|r|unsigned(r,0))?;
        let cursor = if let Some(text) = cursor {
            if text.len() > 1024 {
                return Err(StoreError::InvalidState);
            }
            let v: Cursor = serde_json::from_str(text).map_err(|_| StoreError::InvalidState)?;
            if v.version != 1
                || v.database_identity != identity
                || v.connection_id != id
                || v.high_water > maximum
                || v.before == 0
                || v.before > v.high_water.saturating_add(1)
            {
                return Err(StoreError::InvalidState);
            }
            v
        } else {
            let high = maximum;
            Cursor {
                version: 1,
                database_identity: identity,
                connection_id: id.into(),
                high_water: high,
                before: high.saturating_add(1),
            }
        };
        let rows = query(
            &self.connection,
            "WHERE h.connection_id=?1 AND h.sequence<=?2 AND h.sequence<?3 ORDER BY h.sequence DESC LIMIT ?4",
            params![
                id,
                integer(cursor.high_water)?,
                integer(cursor.before)?,
                limit + 1
            ],
        )?;
        let mut items = Vec::new();
        let mut next = None;
        let mut bytes = 0;
        for e in rows {
            let mut diagnostic = e.diagnostic.view(0, 0);
            for field in [
                "descriptor",
                "catalogLabels",
                "controlObservations",
                "structuredResultSupport",
                "capacity",
            ] {
                diagnostic
                    .as_object_mut()
                    .ok_or(StoreError::InvalidState)?
                    .remove(field);
            }
            diagnostic["current"] = json!(false);
            let item = json!({"sequence":e.sequence,"ordinal":e.ordinal,"archived":e.archived,"diagnostic":diagnostic,"digest":e.digest,"bodyBytes":e.body_bytes});
            let size = serde_json::to_vec(&item)
                .map_err(|_| StoreError::InvalidState)?
                .len();
            if items.len() >= limit as usize || bytes + size > PAGE_BYTES - 8192 {
                next = Some(
                    serde_json::to_string(&Cursor {
                        before: items
                            .last()
                            .and_then(|v: &Value| v["sequence"].as_u64())
                            .ok_or(StoreError::Configuration)?,
                        ..cursor
                    })
                    .map_err(|_| StoreError::InvalidState)?,
                );
                break;
            }
            bytes += size;
            items.push(item);
        }
        let value = json!({"connectionId":id,"items":items,"nextCursor":next,"highWaterSequence":cursor.high_water,"storage":storage(&self.connection,id)?,"historical":true,"observationRenewedOnRead":false});
        if serde_json::to_vec(&value)
            .map_err(|_| StoreError::InvalidState)?
            .len()
            > PAGE_BYTES
        {
            return Err(StoreError::Configuration);
        }
        Ok(value)
    }
    pub fn connection_diagnostic_observation(
        &self,
        id: &str,
        diagnostic_id: &str,
        ordinal: Option<u32>,
    ) -> StoreResult<Value> {
        crate::domain::ConnectionId::new(id)?;
        crate::domain::LocalRef::new(diagnostic_id)?;
        if ordinal.is_some_and(|v| !(1..=2).contains(&v)) {
            return Err(StoreError::Configuration);
        }
        let e =
            exact(&self.connection, id, diagnostic_id, ordinal)?.ok_or(StoreError::InvalidState)?;
        let mut diagnostic = e.diagnostic.view(0, 0);
        diagnostic["current"] = json!(false);
        let effective =
            super::connections::diagnostic_effective_validity(&self.connection, &e.diagnostic)?;
        diagnostic["effectiveValidityMs"] = json!(effective);
        diagnostic["effectiveValidUntilMs"] =
            json!(e.diagnostic.effective_valid_until(effective.minimum()));
        let validity:Option<(String,String,i64,i64)>=self.connection.query_row("SELECT body,digest,refresh_revision,policy_version FROM connection_diagnostic_validity WHERE id=?1",[diagnostic_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional()?;
        let validity=validity.map(|(body,digest,refresh,policy)|{if hash(&body)!=digest {return Err(StoreError::InvalidState);}serde_json::from_str::<Value>(&body).map(|v|json!({"captured":v,"digest":digest,"refreshRevision":refresh,"policyVersion":policy})).map_err(|_|StoreError::InvalidState)}).transpose()?;
        let value = json!({"connectionId":id,"sequence":e.sequence,"ordinal":e.ordinal,"archived":e.archived,"digest":e.digest,"bodyBytes":e.body_bytes,"diagnostic":diagnostic,"validity":validity,"historical":true,"grantsCurrentAuthority":false,"observationRenewedOnRead":false});
        if serde_json::to_vec(&value)
            .map_err(|_| StoreError::InvalidState)?
            .len()
            > DETAIL_BYTES
        {
            return Err(StoreError::Configuration);
        }
        Ok(value)
    }
}
