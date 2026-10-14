//! Validated, immutable supplementary records. No SQL, provider or I/O effects.
use crate::domain::{
    ActionMode, AgentProfile, AttemptId, ConnectionId, ContextPackId, DefinitionVersion, LocalRef,
    ModeId, ProfileId,
};
use crate::persistence_privacy::{PrivacyError, sanitize_json, sanitize_text};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

const MAX_RECORDS: usize = 8192;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordError {
    Invalid,
    Conflict,
    UnknownReference,
    StaleRevision,
    Limit,
}
impl fmt::Display for RecordError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = match self {
            Self::Invalid => "invalid",
            Self::Conflict => "conflict",
            Self::UnknownReference => "unknown-reference",
            Self::StaleRevision => "stale-revision",
            Self::Limit => "limit",
        };
        write!(out, "Persistence record rejected: {code}")
    }
}
impl std::error::Error for RecordError {}
impl From<PrivacyError> for RecordError {
    fn from(_: PrivacyError) -> Self {
        Self::Limit
    }
}
type Result<T> = std::result::Result<T, RecordError>;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum DefinitionKind {
    Mode,
    Profile,
    Context,
    Configuration,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct DefinitionRef {
    pub kind: DefinitionKind,
    pub id: String,
    pub version: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DefinitionInput {
    pub kind: DefinitionKind,
    pub id: String,
    pub version: u64,
    pub body: Value,
    pub references: Vec<DefinitionRef>,
}
impl DefinitionInput {
    pub fn reference(&self) -> DefinitionRef {
        DefinitionRef {
            kind: self.kind,
            id: self.id.clone(),
            version: self.version,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SnapshotInput {
    pub id: String,
    pub attempt_id: String,
    pub mode: DefinitionRef,
    pub profile: DefinitionRef,
    pub context: DefinitionRef,
    pub configuration: DefinitionRef,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub enum RecordsCommand {
    AddDefinition(DefinitionInput),
    AddSnapshot(SnapshotInput),
    RegisterConnection {
        connection_id: String,
        revision: u64,
    },
    Diagnostic(DiagnosticInput),
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticScopeKind {
    Connection,
    Model,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticScope {
    pub kind: DiagnosticScopeKind,
    pub reference: Option<String>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSource {
    Declared,
    Catalog,
    Configuration,
    RuntimeMetadata,
    Synthetic,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticOutcome {
    Success,
    Failed,
    Blocked,
    Cancelled,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DiagnosticInput {
    pub id: String,
    pub connection_id: String,
    pub connection_revision: u64,
    pub version: u64,
    pub scope: DiagnosticScope,
    pub source: DiagnosticSource,
    pub outcome: DiagnosticOutcome,
    pub checked_at: Option<String>,
    pub valid_until: Option<String>,
    pub complete: bool,
    pub values: Value,
    pub error_code: Option<String>,
}

/// A head is a copied view. Failure never rewrites the successful observation.
#[derive(Clone, Debug, PartialEq)]
pub struct DiagnosticHead {
    latest: DiagnosticInput,
    last_success: Option<DiagnosticInput>,
}
impl DiagnosticHead {
    pub fn latest(&self) -> &DiagnosticInput {
        &self.latest
    }
    pub fn last_success(&self) -> Option<&DiagnosticInput> {
        self.last_success.as_ref()
    }
    /// Validity is explicit and dated; no clock, auth or capability inference.
    pub fn has_valid_success_at(&self, now: &str) -> Result<bool> {
        let now = timestamp(now)?;
        let Some(success) = &self.last_success else {
            return Ok(false);
        };
        let (Some(observed), Some(expires)) = (&success.checked_at, &success.valid_until) else {
            return Ok(false);
        };
        Ok(timestamp(observed)? <= now && now < timestamp(expires)?)
    }
}

#[derive(Clone, Debug, Default)]
pub struct DocumentRecords {
    definitions: Vec<DefinitionInput>,
    snapshots: Vec<SnapshotInput>,
    connections: BTreeMap<String, u64>,
    diagnostics: Vec<DiagnosticInput>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    schema_version: u64,
    definitions: Vec<DefinitionInput>,
    snapshots: Vec<SnapshotInput>,
    connections: BTreeMap<String, u64>,
    diagnostics: Vec<DiagnosticInput>,
}

fn local(value: &str) -> Result<()> {
    LocalRef::new(value)
        .map(|_| ())
        .map_err(|_| RecordError::Invalid)
}
fn version(value: u64) -> Result<DefinitionVersion> {
    DefinitionVersion::new(value).map_err(|_| RecordError::Invalid)
}
fn timestamp(value: &str) -> Result<DateTime<Utc>> {
    let parsed = DateTime::parse_from_rfc3339(value).map_err(|_| RecordError::Invalid)?;
    if parsed.timestamp_subsec_nanos() >= 1_000_000_000 {
        return Err(RecordError::Invalid);
    }
    let utc = parsed.with_timezone(&Utc);
    if utc.to_rfc3339_opts(SecondsFormat::Millis, true) != value {
        return Err(RecordError::Invalid);
    }
    Ok(utc)
}
fn reference(value: &DefinitionRef) -> Result<()> {
    local(&value.id)?;
    version(value.version)?;
    Ok(())
}
fn strict_body(body: &Value, fields: &[&str]) -> Result<()> {
    let object = body.as_object().ok_or(RecordError::Invalid)?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(RecordError::Invalid);
    }
    Ok(())
}
fn text<'a>(body: &'a Value, key: &str) -> Result<&'a str> {
    body[key].as_str().ok_or(RecordError::Invalid)
}

impl DocumentRecords {
    pub fn apply(&mut self, command: &RecordsCommand) -> Result<()> {
        match command {
            RecordsCommand::AddDefinition(input) => self.add_definition(input.clone()),
            RecordsCommand::AddSnapshot(input) => self.add_snapshot(input.clone()),
            RecordsCommand::RegisterConnection {
                connection_id,
                revision,
            } => self.register_connection(
                &ConnectionId::new(connection_id).map_err(|_| RecordError::Invalid)?,
                *revision,
            ),
            RecordsCommand::Diagnostic(input) => self.record_diagnostic(input.clone()),
        }
    }
    pub fn definitions(&self) -> &[DefinitionInput] {
        &self.definitions
    }
    pub fn snapshots(&self) -> &[SnapshotInput] {
        &self.snapshots
    }
    pub fn diagnostics(&self) -> &[DiagnosticInput] {
        &self.diagnostics
    }
    pub fn diagnostic_exists(&self, id: &str) -> bool {
        self.diagnostics.iter().any(|value| value.id == id)
    }
    pub fn diagnostic(&self, id: &str) -> Option<&DiagnosticInput> {
        self.diagnostics.iter().find(|value| value.id == id)
    }
    pub fn connection_revisions(&self) -> &BTreeMap<String, u64> {
        &self.connections
    }
    pub fn definition(&self, reference: &DefinitionRef) -> Option<&DefinitionInput> {
        self.definitions
            .iter()
            .find(|value| value.reference() == *reference)
    }
    pub fn add_definition(&mut self, mut input: DefinitionInput) -> Result<()> {
        if self.definitions.len() >= MAX_RECORDS {
            return Err(RecordError::Limit);
        }
        reference(&input.reference())?;
        let revision = version(input.version)?;
        if self.definitions.iter().any(|value| {
            value.kind == input.kind && value.id == input.id && value.version >= input.version
        }) {
            return Err(RecordError::Conflict);
        }
        if input.references.len() > 64 {
            return Err(RecordError::Limit);
        }
        let mut unique = BTreeSet::new();
        for linked in &input.references {
            reference(linked)?;
            if !unique.insert(linked.clone()) {
                return Err(RecordError::Conflict);
            }
            if self.definition(linked).is_none() {
                return Err(RecordError::UnknownReference);
            }
        }
        input.body = sanitize_json(&input.body)?;
        match input.kind {
            DefinitionKind::Mode => {
                strict_body(&input.body, &["name", "purpose"])?;
                ActionMode::new(
                    ModeId::new(&input.id).map_err(|_| RecordError::Invalid)?,
                    revision,
                    text(&input.body, "name")?,
                    text(&input.body, "purpose")?,
                )
                .map_err(|_| RecordError::Invalid)?;
            }
            DefinitionKind::Profile => {
                strict_body(&input.body, &["name", "role", "instructions_ref"])?;
                AgentProfile::new(
                    ProfileId::new(&input.id).map_err(|_| RecordError::Invalid)?,
                    revision,
                    text(&input.body, "name")?,
                    text(&input.body, "role")?,
                    LocalRef::new(text(&input.body, "instructions_ref")?)
                        .map_err(|_| RecordError::Invalid)?,
                )
                .map_err(|_| RecordError::Invalid)?;
            }
            DefinitionKind::Context => {
                ContextPackId::new(&input.id).map_err(|_| RecordError::Invalid)?;
                strict_body(&input.body, &["manifest_ref"])?;
                local(text(&input.body, "manifest_ref")?)?;
            }
            DefinitionKind::Configuration => {
                if !input.body.is_object() {
                    return Err(RecordError::Invalid);
                }
            }
        }
        self.definitions.push(input);
        Ok(())
    }
    pub fn add_snapshot(&mut self, input: SnapshotInput) -> Result<()> {
        if self.snapshots.len() >= MAX_RECORDS {
            return Err(RecordError::Limit);
        }
        local(&input.id)?;
        AttemptId::new(&input.attempt_id).map_err(|_| RecordError::Invalid)?;
        if self
            .snapshots
            .iter()
            .any(|value| value.id == input.id || value.attempt_id == input.attempt_id)
        {
            return Err(RecordError::Conflict);
        }
        for (linked, expected) in [
            (&input.mode, DefinitionKind::Mode),
            (&input.profile, DefinitionKind::Profile),
            (&input.context, DefinitionKind::Context),
            (&input.configuration, DefinitionKind::Configuration),
        ] {
            reference(linked)?;
            if linked.kind != expected {
                return Err(RecordError::Invalid);
            }
            if self.definition(linked).is_none() {
                return Err(RecordError::UnknownReference);
            }
        }
        self.snapshots.push(input);
        Ok(())
    }
    /// Bumping a connection revision invalidates current heads, preserving history.
    pub fn register_connection(&mut self, id: &ConnectionId, revision: u64) -> Result<()> {
        version(revision)?;
        if self
            .connections
            .get(id.as_str())
            .is_some_and(|prior| *prior > revision)
        {
            return Err(RecordError::StaleRevision);
        }
        if !self.connections.contains_key(id.as_str()) && self.connections.len() >= MAX_RECORDS {
            return Err(RecordError::Limit);
        }
        self.connections.insert(id.as_str().to_owned(), revision);
        Ok(())
    }
    pub fn record_diagnostic(&mut self, input: DiagnosticInput) -> Result<()> {
        if self.connections.get(&input.connection_id) != Some(&input.connection_revision) {
            return Err(RecordError::StaleRevision);
        }
        self.push_diagnostic(input)
    }
    fn push_diagnostic(&mut self, mut input: DiagnosticInput) -> Result<()> {
        if self.diagnostics.len() >= MAX_RECORDS {
            return Err(RecordError::Limit);
        }
        local(&input.id)?;
        ConnectionId::new(&input.connection_id).map_err(|_| RecordError::Invalid)?;
        version(input.connection_revision)?;
        version(input.version)?;
        let registered = self
            .connections
            .get(&input.connection_id)
            .ok_or(RecordError::UnknownReference)?;
        if *registered < input.connection_revision {
            return Err(RecordError::StaleRevision);
        }
        match (&input.scope.kind, &input.scope.reference) {
            (DiagnosticScopeKind::Connection, None) => {}
            (DiagnosticScopeKind::Model, Some(id)) => local(id)?,
            _ => return Err(RecordError::Invalid),
        }
        if self.diagnostics.iter().any(|value| value.id == input.id) {
            return Err(RecordError::Conflict);
        }
        if let Some(previous) =
            self.diagnostics.iter().rev().find(|value| {
                value.connection_id == input.connection_id && value.scope == input.scope
            })
            && (previous.version >= input.version
                || previous.connection_revision > input.connection_revision)
        {
            return Err(RecordError::Conflict);
        }
        let checked = input.checked_at.as_deref().map(timestamp).transpose()?;
        let expires = input.valid_until.as_deref().map(timestamp).transpose()?;
        if expires.is_some() && (checked.is_none() || expires <= checked) {
            return Err(RecordError::Invalid);
        }
        if let Some(code) = &input.error_code {
            local(code)?
        }
        if input.outcome == DiagnosticOutcome::Success {
            if input.error_code.is_some() || !input.values.is_object() {
                return Err(RecordError::Invalid);
            }
        } else if !input.values.is_null() || input.valid_until.is_some() || input.complete {
            return Err(RecordError::Invalid);
        }
        input.values = sanitize_json(&input.values)?;
        self.diagnostics.push(input);
        Ok(())
    }
    pub fn diagnostic_head(
        &self,
        connection: &ConnectionId,
        scope: &DiagnosticScope,
    ) -> Option<DiagnosticHead> {
        let revision = self.connections.get(connection.as_str())?;
        let mut matching = self.diagnostics.iter().rev().filter(|value| {
            value.connection_id == connection.as_str()
                && value.connection_revision == *revision
                && &value.scope == scope
        });
        let latest = matching.next()?.clone();
        let last_success = if latest.outcome == DiagnosticOutcome::Success {
            Some(latest.clone())
        } else {
            matching
                .find(|value| value.outcome == DiagnosticOutcome::Success)
                .cloned()
        };
        Some(DiagnosticHead {
            latest,
            last_success,
        })
    }
    pub fn checkpoint(&self) -> Value {
        json!({"schema_version": 1, "definitions": self.definitions, "snapshots": self.snapshots, "connections": self.connections, "diagnostics": self.diagnostics})
    }
    pub fn restore(value: &Value) -> Result<Self> {
        let input: Checkpoint =
            serde_json::from_value(value.clone()).map_err(|_| RecordError::Invalid)?;
        if input.schema_version != 1
            || input.definitions.len() > MAX_RECORDS
            || input.snapshots.len() > MAX_RECORDS
            || input.connections.len() > MAX_RECORDS
            || input.diagnostics.len() > MAX_RECORDS
        {
            return Err(RecordError::Invalid);
        }
        let mut records = Self::default();
        for (id, revision) in input.connections {
            records.register_connection(
                &ConnectionId::new(id).map_err(|_| RecordError::Invalid)?,
                revision,
            )?;
        }
        for definition in input.definitions {
            records.add_definition(definition)?;
        }
        for snapshot in input.snapshots {
            records.add_snapshot(snapshot)?;
        }
        for observation in input.diagnostics {
            records.push_diagnostic(observation)?;
        }
        // Storage must already contain normalized/sanitized records; don't repair corruption.
        if records.checkpoint() != *value {
            return Err(RecordError::Invalid);
        }
        Ok(records)
    }
}

/// Sanitize text before storing user-facing summaries, messages or event text.
pub fn sanitize_record_text(value: &str) -> Result<String> {
    sanitize_text(value).map_err(Into::into)
}
