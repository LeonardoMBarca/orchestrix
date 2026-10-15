//! Bounded historical integration reads. No Git, restore, approval or effects.
use super::{Store, StoreError, StoreResult};
use crate::{domain::LocalRef, git_records::content_hash, integration_records::*};
use rusqlite::{OptionalExtension, params};
use serde_json::{Value, json};

impl Store {
    pub fn daemon_integration_view(&self, id: &str) -> StoreResult<Value> {
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = self.integration_historical_record(id).and_then(|record| {
            let value = json!({"historical":true,"currentVerification":"not-revalidated","summary":record.summary(),
                "intent":record.intent,"phases":record.observations.iter().map(|o|json!({"status":o.status,"reason":o.details.get("reason")})).collect::<Vec<_>>()});
            if serde_json::to_vec(&value).map_err(|_|StoreError::InvalidState)?.len() > 900_000 { return Err(StoreError::Configuration); }
            Ok(value)
        });
        self.connection.execute_batch("ROLLBACK")?;
        result
    }
    pub(super) fn integration_historical_record(&self, id: &str) -> StoreResult<IntegrationRecord> {
        LocalRef::new(id)?;
        let row: Option<(String, String, String)> = self
            .connection
            .query_row(
                "SELECT run_id,task_id,intent FROM serial_integrations WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let (run, task, body) = row.ok_or(StoreError::InvalidState)?;
        if body.len() > 1_048_576 {
            return Err(StoreError::InvalidState);
        }
        let intent: IntegrationIntent =
            serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?;
        if intent.id != id || intent.run_id != run || intent.task_id != task {
            return Err(StoreError::InvalidState);
        }
        let mut record = IntegrationRecord {
            intent,
            observations: vec![],
        };
        let mut query=self.connection.prepare("SELECT ordinal,body,digest FROM integration_observations WHERE integration_id=?1 ORDER BY ordinal LIMIT 129")?;
        for row in query.query_map([id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })? {
            let (ordinal, body, digest) = row?;
            if usize::try_from(ordinal).ok() != Some(record.observations.len() + 1)
                || body.len() > 4_194_304
                || content_hash(body.as_bytes()) != digest
            {
                return Err(StoreError::InvalidState);
            }
            record
                .observations
                .push(serde_json::from_str(&body).map_err(|_| StoreError::InvalidState)?);
        }
        if !record.validate() {
            return Err(StoreError::InvalidState);
        }
        Ok(record)
    }
    pub(super) fn integration_affected_by_file(&self, path: &str) -> StoreResult<Vec<String>> {
        let mut query=self.connection.prepare(r#"WITH candidates AS (SELECT i.id,i.intent,json_extract(i.intent,'$.worktree.path') AS worktree,json_extract(i.intent,'$.target.directory') AS target FROM serial_integrations i JOIN integration_observations o ON o.integration_id=i.id AND o.ordinal=(SELECT max(last.ordinal) FROM integration_observations last WHERE last.integration_id=i.id) WHERE json_extract(o.body,'$.status') IN ('prepared','approved')) SELECT id FROM candidates WHERE (substr(?1,1,length(worktree))=worktree AND substr(?1,length(worktree)+1,1) IN (char(92),'/')) OR (substr(?1,1,length(target))=target AND substr(?1,length(target)+1,1) IN (char(92),'/')) ORDER BY id LIMIT 129"#)?;
        let ids = query
            .query_map([path], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        if ids.len() > 128 {
            return Err(StoreError::Configuration);
        }
        for id in &ids {
            let record = self.integration_historical_record(id)?;
            if !std::path::Path::new(path).starts_with(&record.intent.worktree.path)
                && !std::path::Path::new(path).starts_with(&record.intent.target.directory)
            {
                return Err(StoreError::InvalidState);
            }
        }
        Ok(ids)
    }
    pub fn daemon_integration_blocks_admission(&self) -> StoreResult<bool> {
        let (unknown,blocking):(bool,bool)=self.connection.query_row("WITH latest AS (SELECT coalesce(json_extract(o.body,'$.status'),'reserved') AS status FROM serial_integrations i LEFT JOIN integration_observations o ON o.integration_id=i.id AND o.ordinal=(SELECT max(last.ordinal) FROM integration_observations last WHERE last.integration_id=i.id)) SELECT EXISTS(SELECT 1 FROM latest WHERE status NOT IN ('reserved','preparing','materialized','checking','checked','commit-planned','integrated','prepared','approved','applying','applied','delivered','blocked','invalidated','uncertain')),EXISTS(SELECT 1 FROM latest WHERE status IN ('reserved','preparing','materialized','checking','checked','commit-planned','applying','uncertain'))",[],|r|Ok((r.get(0)?,r.get(1)?)))?;
        if unknown {
            return Err(StoreError::InvalidState);
        }
        Ok(blocking)
    }
    pub fn daemon_integration_operation_recorded(
        &self,
        id: &str,
        kind: &str,
        hash: &str,
    ) -> StoreResult<bool> {
        if !self.daemon_integration_exists(id)? {
            return Ok(false);
        }
        let record = self.integration_historical_record(id)?;
        Ok(match kind {
            "integration-prepare" => record.intent.request_hash == hash,
            "integration-approve" => record
                .detail("approval")
                .is_some_and(|a| a["requestHash"] == hash),
            "integration-apply" => record
                .detail("applicationRequestHash")
                .is_some_and(|h| h == hash),
            _ => false,
        })
    }
    pub(super) fn integration_worker_history(
        &self,
        worker: &str,
    ) -> StoreResult<(Vec<Value>, u64)> {
        LocalRef::new(worker)?;
        let mut query=self.connection.prepare("SELECT id FROM serial_integrations WHERE json_extract(intent,'$.worker_id')=?1 ORDER BY rowid DESC LIMIT 16")?;
        let ids = query
            .query_map(params![worker], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let count: i64 = self.connection.query_row(
            "SELECT count(*) FROM serial_integrations WHERE json_extract(intent,'$.worker_id')=?1",
            [worker],
            |r| r.get(0),
        )?;
        let summaries = ids
            .iter()
            .map(|id| self.integration_historical_record(id).map(|r| r.summary()))
            .collect::<StoreResult<Vec<_>>>()?;
        Ok((
            summaries,
            u64::try_from(count).map_err(|_| StoreError::InvalidState)?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_integration_history_has_no_admission_count_ceiling() {
        let connection = rusqlite::Connection::open_in_memory().unwrap();
        connection.execute_batch("CREATE TABLE serial_integrations(id TEXT);CREATE TABLE integration_observations(integration_id TEXT,ordinal INTEGER,body TEXT);
            WITH RECURSIVE ids(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM ids WHERE n<1025) INSERT INTO serial_integrations SELECT 'terminal-'||n FROM ids;
            INSERT INTO integration_observations SELECT id,1,'{\"status\":\"delivered\"}' FROM serial_integrations;").unwrap();
        let store = Store {
            connection,
            path: std::path::PathBuf::from("historical-projection-only.sqlite"),
        };
        assert!(!store.daemon_integration_blocks_admission().unwrap());
        store
            .connection
            .execute("INSERT INTO serial_integrations VALUES('pending')", [])
            .unwrap();
        assert!(store.daemon_integration_blocks_admission().unwrap());
        store.connection.execute("INSERT INTO integration_observations VALUES('pending',1,'{\"status\":\"uncertain\"}')",[]).unwrap();
        assert!(store.daemon_integration_blocks_admission().unwrap());
        store.connection.execute("INSERT INTO integration_observations VALUES('pending',2,'{\"status\":\"invented\"}')",[]).unwrap();
        assert!(store.daemon_integration_blocks_admission().is_err());
    }
}
