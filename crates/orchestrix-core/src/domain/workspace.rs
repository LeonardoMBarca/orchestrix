use super::{
    AttemptId, CapacityGroupId, ConnectionId, ConversationId, DomainError, DomainResult,
    EvidenceId, LocalRef, ProjectId, RepositoryId, RevisionId, Run, RunId, RuntimeSessionId,
    TaskId, require,
};
use crate::runtime_contract::RuntimeDescriptor;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Repository {
    pub(super) id: RepositoryId,
    pub(super) location: LocalRef,
}
impl Repository {
    pub fn new(id: RepositoryId, location: LocalRef) -> Self {
        Self { id, location }
    }
    pub fn id(&self) -> &RepositoryId {
        &self.id
    }
    pub fn location(&self) -> &LocalRef {
        &self.location
    }
}

/// The V1 relationship is one repository per project, with distinct identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Project {
    pub(super) id: ProjectId,
    pub(super) repository: Repository,
}
impl Project {
    pub fn new(id: ProjectId, repository: Repository) -> Self {
        Self { id, repository }
    }
    pub fn id(&self) -> &ProjectId {
        &self.id
    }
    pub fn repository(&self) -> &Repository {
        &self.repository
    }
}

/// Connection identity comes from the validated portable descriptor, not secrets.
#[derive(Clone, Debug)]
pub struct Connection {
    pub(super) id: ConnectionId,
    pub(super) revision: u64,
    pub(super) adapter_id: String,
    pub(super) descriptor: RuntimeDescriptor,
}
impl Connection {
    pub fn new(descriptor: RuntimeDescriptor) -> DomainResult<Self> {
        let data = descriptor.as_json();
        let id = ConnectionId::new(
            data["connection"]["id"]
                .as_str()
                .ok_or(DomainError::InvalidReference)?,
        )?;
        let revision = data["connection"]["revision"]
            .as_u64()
            .ok_or(DomainError::InvalidVersion)?;
        let adapter_id = data["adapterId"]
            .as_str()
            .ok_or(DomainError::InvalidReference)?
            .to_owned();
        Ok(Self {
            id,
            revision,
            adapter_id,
            descriptor,
        })
    }
    pub fn id(&self) -> &ConnectionId {
        &self.id
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn adapter_id(&self) -> &str {
        &self.adapter_id
    }
    pub fn descriptor(&self) -> &RuntimeDescriptor {
        &self.descriptor
    }
}

/// Explicit host evidence using one consistent unit; never inferred per account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapacityObservation {
    pub(super) limit: u64,
    pub(super) remaining: u64,
    pub(super) evidence: EvidenceId,
}
impl CapacityObservation {
    pub fn new(limit: u64, remaining: u64, evidence: EvidenceId) -> DomainResult<Self> {
        require(
            limit > 0 && remaining <= limit,
            DomainError::InvalidReference,
        )?;
        Ok(Self {
            limit,
            remaining,
            evidence,
        })
    }
    pub fn limit(&self) -> u64 {
        self.limit
    }
    pub fn remaining(&self) -> u64 {
        self.remaining
    }
    pub fn evidence(&self) -> &EvidenceId {
        &self.evidence
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SharedCapacity {
    Unknown,
    Observed(CapacityObservation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapacityGroup {
    pub(super) id: CapacityGroupId,
    pub(super) members: BTreeSet<ConnectionId>,
    pub(super) capacity: SharedCapacity,
}
impl CapacityGroup {
    pub fn new(id: CapacityGroupId, capacity: SharedCapacity) -> Self {
        Self {
            id,
            members: BTreeSet::new(),
            capacity,
        }
    }
    pub fn id(&self) -> &CapacityGroupId {
        &self.id
    }
    pub fn members(&self) -> &BTreeSet<ConnectionId> {
        &self.members
    }
    pub fn capacity(&self) -> &SharedCapacity {
        &self.capacity
    }
    pub fn contains(&self, id: &ConnectionId) -> bool {
        self.members.contains(id)
    }
    pub fn add_member(&mut self, id: ConnectionId) -> DomainResult<()> {
        require(self.members.insert(id), DomainError::DuplicateIdentity)
    }
    pub fn remove_member(&mut self, id: &ConnectionId) -> DomainResult<()> {
        require(self.members.remove(id), DomainError::UnknownEntity)
    }
    pub fn record_capacity(&mut self, capacity: SharedCapacity) {
        self.capacity = capacity;
    }
}

/// Local handle only: the adapter owns native sessions and provider transcripts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeSessionRef {
    pub(super) id: RuntimeSessionId,
    pub(super) connection_id: ConnectionId,
    pub(super) connection_revision: u64,
    pub(super) adapter_id: String,
}
impl RuntimeSessionRef {
    pub fn new(id: RuntimeSessionId, connection: &Connection) -> Self {
        Self {
            id,
            connection_id: connection.id.clone(),
            connection_revision: connection.revision,
            adapter_id: connection.adapter_id.clone(),
        }
    }
    pub fn id(&self) -> &RuntimeSessionId {
        &self.id
    }
    pub fn connection_id(&self) -> &ConnectionId {
        &self.connection_id
    }
    pub fn connection_revision(&self) -> u64 {
        self.connection_revision
    }
    pub fn adapter_id(&self) -> &str {
        &self.adapter_id
    }
}

#[derive(Default, Debug)]
pub struct SessionRegistry {
    pub(super) sessions: BTreeMap<RuntimeSessionId, RuntimeSessionRef>,
}
impl SessionRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, session: RuntimeSessionRef) -> DomainResult<()> {
        if let Some(existing) = self.sessions.get(session.id()) {
            return Err(if existing == &session {
                DomainError::DuplicateIdentity
            } else {
                DomainError::Ownership
            });
        }
        self.sessions.insert(session.id.clone(), session);
        Ok(())
    }
    pub fn get(&self, id: &RuntimeSessionId) -> Option<&RuntimeSessionRef> {
        self.sessions.get(id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Orchestrator,
}

/// Only UI content, with the project binding captured when it was appended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationMessage {
    pub(super) order: u64,
    pub(super) role: MessageRole,
    pub(super) content: String,
    pub(super) project_at_send: Option<ProjectId>,
}
impl ConversationMessage {
    pub fn order(&self) -> u64 {
        self.order
    }
    pub fn role(&self) -> MessageRole {
        self.role
    }
    pub fn content(&self) -> &str {
        &self.content
    }
    pub fn project_at_send(&self) -> Option<&ProjectId> {
        self.project_at_send.as_ref()
    }
}

/// Captured local work/version references, not a provider transcript or status.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConversationWorkRef {
    pub(super) run_id: RunId,
    pub(super) run_result_revision: RevisionId,
    pub(super) project_at_link: Option<ProjectId>,
    pub(super) task_id: Option<TaskId>,
    pub(super) attempt_id: Option<AttemptId>,
    pub(super) plan_revision: Option<RevisionId>,
}
impl ConversationWorkRef {
    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }
    pub fn run_result_revision(&self) -> &RevisionId {
        &self.run_result_revision
    }
    pub fn project_at_link(&self) -> Option<&ProjectId> {
        self.project_at_link.as_ref()
    }
    pub fn task_id(&self) -> Option<&TaskId> {
        self.task_id.as_ref()
    }
    pub fn attempt_id(&self) -> Option<&AttemptId> {
        self.attempt_id.as_ref()
    }
    pub fn plan_revision(&self) -> Option<&RevisionId> {
        self.plan_revision.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversation {
    pub(super) id: ConversationId,
    pub(super) project_id: Option<ProjectId>,
    pub(super) messages: Vec<ConversationMessage>,
    pub(super) work_references: Vec<ConversationWorkRef>,
}
impl Conversation {
    pub fn id(&self) -> &ConversationId {
        &self.id
    }
    pub fn project_id(&self) -> Option<&ProjectId> {
        self.project_id.as_ref()
    }
    pub fn messages(&self) -> &[ConversationMessage] {
        &self.messages
    }
    pub fn work_references(&self) -> &[ConversationWorkRef] {
        &self.work_references
    }
    fn append(&mut self, role: MessageRole, content: String) -> DomainResult<()> {
        require(!content.trim().is_empty(), DomainError::EmptyContent)?;
        let order = u64::try_from(self.messages.len())
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(DomainError::InvalidTransition)?;
        self.messages.push(ConversationMessage {
            order,
            role,
            content,
            project_at_send: self.project_id.clone(),
        });
        Ok(())
    }
}

/// HOME/New session may hold a draft but owns no saved conversation initially.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingComposer {
    pub(super) project_id: Option<ProjectId>,
    pub(super) draft: String,
    pub(super) conversation_id: Option<ConversationId>,
}
impl PendingComposer {
    pub fn project_id(&self) -> Option<&ProjectId> {
        self.project_id.as_ref()
    }
    pub fn draft(&self) -> &str {
        &self.draft
    }
    pub fn set_draft(&mut self, draft: impl Into<String>) {
        self.draft = draft.into();
    }
    pub fn conversation_id(&self) -> Option<&ConversationId> {
        self.conversation_id.as_ref()
    }
}

#[derive(Default, Debug)]
pub struct ConversationStore {
    pub(super) projects: BTreeMap<ProjectId, Project>,
    pub(super) conversations: BTreeMap<ConversationId, Conversation>,
}
impl ConversationStore {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add_project(&mut self, project: Project) -> DomainResult<()> {
        require(
            !self.projects.contains_key(project.id()),
            DomainError::DuplicateIdentity,
        )?;
        self.projects.insert(project.id.clone(), project);
        Ok(())
    }
    pub fn project(&self, id: &ProjectId) -> Option<&Project> {
        self.projects.get(id)
    }
    pub fn projects(&self) -> impl Iterator<Item = &Project> {
        self.projects.values()
    }
    pub fn conversation(&self, id: &ConversationId) -> Option<&Conversation> {
        self.conversations.get(id)
    }
    pub fn conversations(&self) -> impl Iterator<Item = &Conversation> {
        self.conversations.values()
    }
    pub fn project_conversations(&self, id: &ProjectId) -> DomainResult<Vec<&Conversation>> {
        require(self.projects.contains_key(id), DomainError::UnknownEntity)?;
        Ok(self
            .conversations
            .values()
            .filter(|conversation| conversation.project_id() == Some(id))
            .collect())
    }
    pub fn prepare(&self, project_id: Option<ProjectId>) -> DomainResult<PendingComposer> {
        if let Some(id) = &project_id {
            require(self.projects.contains_key(id), DomainError::UnknownEntity)?;
        }
        Ok(PendingComposer {
            project_id,
            draft: String::new(),
            conversation_id: None,
        })
    }
    /// A fresh ID is consumed only on the first successful nonempty send.
    pub fn send(
        &mut self,
        composer: &mut PendingComposer,
        fresh_id: ConversationId,
    ) -> DomainResult<ConversationId> {
        require(!composer.draft.trim().is_empty(), DomainError::EmptyContent)?;
        let id = if let Some(existing) = &composer.conversation_id {
            let conversation = self
                .conversations
                .get_mut(existing)
                .ok_or(DomainError::UnknownEntity)?;
            conversation.append(MessageRole::User, composer.draft.clone())?;
            existing.clone()
        } else {
            require(
                !self.conversations.contains_key(&fresh_id),
                DomainError::DuplicateIdentity,
            )?;
            if let Some(project) = &composer.project_id {
                require(
                    self.projects.contains_key(project),
                    DomainError::UnknownEntity,
                )?;
            }
            let mut conversation = Conversation {
                id: fresh_id.clone(),
                project_id: composer.project_id.clone(),
                messages: Vec::new(),
                work_references: Vec::new(),
            };
            conversation.append(MessageRole::User, composer.draft.clone())?;
            self.conversations.insert(fresh_id.clone(), conversation);
            fresh_id
        };
        composer.conversation_id = Some(id.clone());
        composer.draft.clear();
        Ok(id)
    }
    pub fn append_reply(
        &mut self,
        id: &ConversationId,
        content: impl Into<String>,
    ) -> DomainResult<()> {
        self.conversations
            .get_mut(id)
            .ok_or(DomainError::UnknownEntity)?
            .append(MessageRole::Orchestrator, content.into())
    }
    pub fn append_message(
        &mut self,
        id: &ConversationId,
        role: MessageRole,
        content: impl Into<String>,
    ) -> DomainResult<()> {
        self.conversations
            .get_mut(id)
            .ok_or(DomainError::UnknownEntity)?
            .append(role, content.into())
    }
    pub fn attach_project(
        &mut self,
        id: &ConversationId,
        project_id: ProjectId,
    ) -> DomainResult<()> {
        require(
            self.projects.contains_key(&project_id),
            DomainError::UnknownEntity,
        )?;
        self.conversations
            .get_mut(id)
            .ok_or(DomainError::UnknownEntity)?
            .project_id = Some(project_id);
        Ok(())
    }
    pub fn link_work(
        &mut self,
        id: &ConversationId,
        run: &Run,
        task_id: Option<&TaskId>,
        attempt_id: Option<&AttemptId>,
    ) -> DomainResult<()> {
        let conversation = self
            .conversations
            .get_mut(id)
            .ok_or(DomainError::UnknownEntity)?;
        require(
            conversation.project_id() == run.project(),
            DomainError::Ownership,
        )?;
        require(
            attempt_id.is_none() || task_id.is_some(),
            DomainError::Ownership,
        )?;
        let task = task_id.map(|task_id| run.task(task_id)).transpose()?;
        let attempt = match (task, attempt_id) {
            (Some(task), Some(attempt_id)) => Some(
                task.attempts()
                    .iter()
                    .find(|attempt| attempt.id() == attempt_id)
                    .ok_or(DomainError::Ownership)?,
            ),
            _ => None,
        };
        let reference = ConversationWorkRef {
            run_id: run.id().clone(),
            run_result_revision: run.result_revision().clone(),
            project_at_link: run.project().cloned(),
            task_id: task.map(|task| task.id().clone()),
            attempt_id: attempt.map(|attempt| attempt.id().clone()),
            plan_revision: attempt.map(|attempt| attempt.inputs().plan_revision.clone()),
        };
        require(
            !conversation.work_references.contains(&reference),
            DomainError::DuplicateIdentity,
        )?;
        conversation.work_references.push(reference);
        Ok(())
    }
}
