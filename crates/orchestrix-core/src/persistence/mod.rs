//! Synchronous SQLite boundary. The domain has no database or process types.
//! Every mutation uses detached state and a single, bounded writer transaction.
mod commands;
mod daemon;
mod desktop;
mod git;
mod integration;
mod integration_projection;
mod review;
mod schema;
mod state;
mod worker;

use crate::domain::{DomainError, LocalRef};
use crate::persistence_privacy::{PrivacyError, sanitize_json};
use crate::persistence_records::{RecordError, SnapshotInput};
pub use commands::{Command, RunAction};
pub use daemon::{DaemonCommandReceipt, DaemonCursor, DaemonEvent, DaemonState};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
pub use schema::SCHEMA_VERSION;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
pub use state::SavedState;
use std::{fmt, path::Path};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    InvalidPath,
    FutureSchema,
    UnknownDatabase,
    Configuration,
    InvalidState,
    Busy,
    Constraint,
    Storage,
    IdempotencyConflict,
    StaleRevision,
    InconsistentCommand,
    InjectedRollback,
    ReplyLost,
    Domain(DomainError),
    Records(RecordError),
    Privacy(PrivacyError),
}
impl fmt::Display for StoreError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        // No SQL, frames, paths, commands or credentials in operational errors.
        let code = match self {
            Self::InvalidPath => "invalid-path",
            Self::FutureSchema => "future-schema",
            Self::UnknownDatabase => "unknown-database",
            Self::Configuration => "configuration",
            Self::InvalidState => "invalid-state",
            Self::Busy => "busy",
            Self::Constraint => "constraint",
            Self::Storage => "storage",
            Self::IdempotencyConflict => "idempotency-conflict",
            Self::StaleRevision => "stale-revision",
            Self::InconsistentCommand => "inconsistent-command",
            Self::InjectedRollback => "injected-rollback",
            Self::ReplyLost => "reply-lost",
            Self::Domain(_) => "domain",
            Self::Records(_) => "records",
            Self::Privacy(_) => "privacy",
        };
        write!(out, "Local persistence rejected operation: {code}")
    }
}
impl std::error::Error for StoreError {}
impl From<rusqlite::Error> for StoreError {
    fn from(value: rusqlite::Error) -> Self {
        match value.sqlite_error_code() {
            Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
                Self::Busy
            }
            Some(rusqlite::ErrorCode::ConstraintViolation) => Self::Constraint,
            _ => Self::Storage,
        }
    }
}
impl From<DomainError> for StoreError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}
pub type StoreResult<T> = Result<T, StoreError>;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AttemptSnapshot {
    pub definition: SnapshotInput,
    /// IDs of immutable observations used, not a claim that they are valid now.
    pub diagnostics: Vec<String>,
}

pub struct Request {
    /// Namespace within this installation/database (not a provider thread ID).
    pub scope: LocalRef,
    pub key: LocalRef,
    pub expected_revision: u64,
    pub command: Command,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Disposition {
    Applied,
    InvalidNoChange,
    ObservedFailureCommitted,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CommandResult {
    pub schema_version: u64,
    pub revision: u64,
    pub disposition: Disposition,
    pub error_code: Option<String>,
    pub output: Value,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Receipt {
    pub result: CommandResult,
    pub replayed: bool,
}

/// Explicit fault injection seam, never selected by normal execute().
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaultPoint {
    None,
    AfterState,
    AfterEvent,
    AfterResult,
    AfterCommit,
}

pub struct Store {
    connection: Connection,
    path: std::path::PathBuf,
}
impl Store {
    /// Canonical database identity for the host's OS lease, never a domain permission.
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Parent directory must exist; no implicit app directory or in-memory database.
    pub fn open(path: impl AsRef<Path>) -> StoreResult<Self> {
        let path = path.as_ref();
        if !path.is_absolute()
            || path.is_dir()
            || path.file_name().is_none()
            || !path.parent().is_some_and(Path::is_dir)
        {
            return Err(StoreError::InvalidPath);
        }
        let mut connection = Connection::open(path)?;
        schema::initialize(&mut connection)?;
        let store = Self {
            connection,
            path: std::fs::canonicalize(path).map_err(|_| StoreError::InvalidPath)?,
        };
        store.load()?; // Reject incompatible contents without rewriting the database.
        Ok(store)
    }
    /// One consistent SQLite read snapshot, never a resumed runtime.
    pub fn load(&self) -> StoreResult<SavedState> {
        self.connection.execute_batch("BEGIN DEFERRED")?;
        let result = state::load(&self.connection);
        self.connection.execute_batch("ROLLBACK")?;
        result
    }
    pub fn execute(&mut self, request: &Request) -> StoreResult<Receipt> {
        self.execute_with_fault(request, FaultPoint::None)
    }
    pub fn execute_with_fault(
        &mut self,
        request: &Request,
        fault: FaultPoint,
    ) -> StoreResult<Receipt> {
        for id in [request.scope.as_str(), request.key.as_str()] {
            if crate::persistence_privacy::sanitize_text(id).map_err(StoreError::Privacy)? != id {
                return Err(DomainError::InvalidReference.into());
            }
        }
        if let Command::Git(command) = &request.command
            && !command.input_bounded()
        {
            return Err(StoreError::Privacy(
                crate::persistence_privacy::PrivacyError::Limit,
            ));
        }
        if let Command::Worker(command) = &request.command
            && !command.bounded()
        {
            return Err(StoreError::Privacy(
                crate::persistence_privacy::PrivacyError::Limit,
            ));
        }
        if let Command::Review(command) = &request.command
            && !command.bounded()
        {
            return Err(StoreError::InvalidState);
        }
        if let Command::Integration(command) = &request.command
            && !command.bounded()
        {
            return Err(StoreError::InvalidState);
        }
        let body = request.command.body();
        // Check supported input bounds before hashing; only the digest is stored.
        sanitize_json(&body).map_err(StoreError::Privacy)?;
        let fingerprint = digest(
            json!({"commandSchema":1,"expectedRevision":request.expected_revision,"body":body})
                .to_string()
                .as_bytes(),
        );
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous: Option<(String, String)> = transaction
            .query_row(
                "SELECT fingerprint,result FROM commands WHERE scope=?1 AND key=?2",
                params![request.scope.as_str(), request.key.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((stored, result)) = previous {
            if stored != fingerprint {
                return Err(StoreError::IdempotencyConflict);
            }
            let result = decode_result(&result)?;
            // No writes; dropping the transaction releases the writer reservation.
            return Ok(Receipt {
                result,
                replayed: true,
            });
        }
        let mut staged = state::load(&transaction)?;
        if staged.revision != request.expected_revision {
            return Err(StoreError::StaleRevision);
        }
        let before = staged.checkpoint();
        let operation = request.command.apply(&mut staged);
        let changed = staged.checkpoint() != before;
        let (disposition, error_code, output) = match operation {
            Ok(output) => (Disposition::Applied, None, output),
            Err(StoreError::Domain(error))
                if changed && request.command.observed_failure(&error) =>
            {
                (
                    Disposition::ObservedFailureCommitted,
                    Some(error.to_string()),
                    Value::Null,
                )
            }
            Err(StoreError::Domain(error)) if !changed => (
                Disposition::InvalidNoChange,
                Some(error.to_string()),
                Value::Null,
            ),
            Err(StoreError::Records(error)) if !changed => (
                Disposition::InvalidNoChange,
                Some(error.to_string()),
                Value::Null,
            ),
            Err(StoreError::Domain(_) | StoreError::Records(_)) => {
                return Err(StoreError::InconsistentCommand);
            }
            Err(error) => return Err(error),
        };
        let mut persisted = staged.sanitized()?;
        if changed {
            persisted.revision = persisted
                .revision
                .checked_add(1)
                .filter(|v| *v <= i64::MAX as u64)
                .ok_or(StoreError::InvalidState)?;
            state::save(&transaction, &persisted)?;
        }
        if fault == FaultPoint::AfterState {
            return Err(StoreError::InjectedRollback);
        }
        let result = CommandResult {
            schema_version: 1,
            revision: persisted.revision,
            disposition,
            error_code,
            output: sanitize_json(&output).map_err(StoreError::Privacy)?,
        };
        if changed {
            let audit = json!({"operation":request.command.name(),"disposition":result.disposition,"errorCode":result.error_code});
            let (run, task, attempt) = request.command.work_references(&persisted);
            transaction.execute("INSERT INTO events(schema_version,revision,scope,command_key,kind,run_id,task_id,attempt_id,body) VALUES(1,?1,?2,?3,?4,?5,?6,?7,?8)",params![sql_int(result.revision)?,request.scope.as_str(),request.key.as_str(),request.command.name(),run,task,attempt,audit.to_string()])?;
        }
        if fault == FaultPoint::AfterEvent {
            return Err(StoreError::InjectedRollback);
        }
        transaction.execute(
            "INSERT INTO commands(scope,key,fingerprint,result) VALUES(?1,?2,?3,?4)",
            params![
                request.scope.as_str(),
                request.key.as_str(),
                fingerprint,
                serde_json::to_string(&result).map_err(|_| StoreError::InvalidState)?
            ],
        )?;
        if fault == FaultPoint::AfterResult {
            return Err(StoreError::InjectedRollback);
        }
        transaction.commit()?;
        if fault == FaultPoint::AfterCommit {
            return Err(StoreError::ReplyLost);
        }
        Ok(Receipt {
            result,
            replayed: false,
        })
    }
    pub fn event_count(&self) -> StoreResult<u64> {
        u64::try_from(
            self.connection
                .query_row("SELECT count(*) FROM events", [], |row| {
                    row.get::<_, i64>(0)
                })?,
        )
        .map_err(|_| StoreError::InvalidState)
    }
    pub fn command_count(&self) -> StoreResult<u64> {
        u64::try_from(
            self.connection
                .query_row("SELECT count(*) FROM commands", [], |row| {
                    row.get::<_, i64>(0)
                })?,
        )
        .map_err(|_| StoreError::InvalidState)
    }
    /// Read-only lookup of a validated receipt. It grants no authority to replay
    /// different content: execute() still checks the original fingerprint.
    pub fn command_result(
        &self,
        scope: &LocalRef,
        key: &LocalRef,
    ) -> StoreResult<Option<CommandResult>> {
        for value in [scope.as_str(), key.as_str()] {
            if crate::persistence_privacy::sanitize_text(value).map_err(StoreError::Privacy)?
                != value
            {
                return Err(DomainError::InvalidReference.into());
            }
        }
        let value: Option<String> = self
            .connection
            .query_row(
                "SELECT result FROM commands WHERE scope=?1 AND key=?2",
                params![scope.as_str(), key.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        value.as_deref().map(decode_result).transpose()
    }
    pub fn sqlite_version() -> &'static str {
        rusqlite::version()
    }
}
pub(super) fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn decode_result(text: &str) -> StoreResult<CommandResult> {
    let result: CommandResult = serde_json::from_str(text).map_err(|_| StoreError::InvalidState)?;
    if result.schema_version != 1
        || result.revision > i64::MAX as u64
        || (result.disposition == Disposition::Applied) != result.error_code.is_none()
    {
        return Err(StoreError::InvalidState);
    }
    let value = serde_json::to_value(&result).map_err(|_| StoreError::InvalidState)?;
    if sanitize_json(&value).map_err(StoreError::Privacy)? != value {
        return Err(StoreError::InvalidState);
    }
    if !result.output.is_null() {
        if let Some(artifact) = result.output.get("gitArtifact") {
            if result.disposition != Disposition::Applied
                || result.output.as_object().is_none_or(|map| map.len() != 1)
                || artifact.as_object().is_none_or(|map| map.len() != 3)
                || crate::domain::ArtifactId::new(
                    artifact["id"].as_str().ok_or(StoreError::InvalidState)?,
                )
                .is_err()
                || !crate::git_records::is_hash(
                    artifact["revision"]
                        .as_str()
                        .ok_or(StoreError::InvalidState)?,
                )
                || LocalRef::new(
                    artifact["operationId"]
                        .as_str()
                        .ok_or(StoreError::InvalidState)?,
                )
                .is_err()
            {
                return Err(StoreError::InvalidState);
            }
        } else {
            crate::domain::DeliveryCandidate::restore_checkpoint(&result.output)
                .map_err(|_| StoreError::InvalidState)?;
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests;

pub(super) fn sql_int(value: u64) -> StoreResult<i64> {
    i64::try_from(value).map_err(|_| StoreError::InvalidState)
}
