//! In-memory Run aggregate: dependency release, integration and final delivery.
use super::{
    ArtifactVersion, AttemptState, CheckResult, DependencyResult, DomainError, DomainResult,
    EvidenceId, IntegrationEvidence, Project, ProjectId, RepositoryId, ReviewEvidence, RevisionId,
    RunId, Task, TaskAttempt, TaskId, TaskKind, TaskState, VerificationEvidence, require,
};
use crate::runtime_contract::EventOrigin;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeliveryRequirement {
    FinalApplication,
    AcceptedArtifacts,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunState {
    Working,
    ReadyToApply,
    Applied,
    Blocked,
    Cancelled,
    Delivered,
}

/// An opaque candidate issued by this Run. Copying it does not approve it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryCandidate {
    pub(super) run: RunId,
    pub(super) repository: RepositoryId,
    pub(super) result: RevisionId,
    pub(super) target_base: RevisionId,
    pub(super) generation: u64,
    pub(super) artifacts: Vec<ArtifactVersion>,
}
impl DeliveryCandidate {
    pub fn run(&self) -> &RunId {
        &self.run
    }
    pub fn repository(&self) -> &RepositoryId {
        &self.repository
    }
    pub fn result(&self) -> &RevisionId {
        &self.result
    }
    pub fn target_base(&self) -> &RevisionId {
        &self.target_base
    }
    pub fn artifacts(&self) -> &[ArtifactVersion] {
        &self.artifacts
    }
}

/// Explicit supplied confirmation; no Git/check commands run in this module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationEvidence {
    pub run: RunId,
    pub repository: RepositoryId,
    pub result: RevisionId,
    pub target_base: RevisionId,
    pub applied_revision: RevisionId,
    pub reference: EvidenceId,
    pub validation: CheckResult,
}

/// A host-observed conflict retains the exact candidate without approving it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationConflict {
    pub(super) candidate: DeliveryCandidate,
    pub(super) reference: EvidenceId,
}
impl ApplicationConflict {
    pub fn candidate(&self) -> &DeliveryCandidate {
        &self.candidate
    }
    pub fn reference(&self) -> &EvidenceId {
        &self.reference
    }
}

pub struct Run {
    pub(super) id: RunId,
    pub(super) project: Option<ProjectId>,
    pub(super) repository: Option<RepositoryId>,
    pub(super) initial_revision: RevisionId,
    pub(super) result_revision: RevisionId,
    pub(super) delivery: DeliveryRequirement,
    pub(super) state: RunState,
    pub(super) tasks: BTreeMap<TaskId, Task>,
    pub(super) generation: u64,
    pub(super) candidate: Option<DeliveryCandidate>,
    pub(super) approval: Option<EvidenceId>,
    pub(super) application: Option<ApplicationEvidence>,
    pub(super) application_failures: Vec<ApplicationEvidence>,
    pub(super) integration_order: Vec<(TaskId, EvidenceId)>,
    pub(super) conflicts: Vec<ApplicationConflict>,
    pub(super) cancellation_requested: bool,
}
impl Run {
    pub fn new(
        id: RunId,
        project: Option<&Project>,
        base: RevisionId,
        delivery: DeliveryRequirement,
    ) -> DomainResult<Self> {
        require(
            delivery != DeliveryRequirement::FinalApplication || project.is_some(),
            DomainError::MissingEvidence,
        )?;
        Ok(Self {
            id,
            project: project.map(|p| p.id().clone()),
            repository: project.map(|p| p.repository().id().clone()),
            initial_revision: base.clone(),
            result_revision: base,
            delivery,
            state: RunState::Working,
            tasks: BTreeMap::new(),
            generation: 0,
            candidate: None,
            approval: None,
            application: None,
            application_failures: Vec::new(),
            integration_order: Vec::new(),
            conflicts: Vec::new(),
            cancellation_requested: false,
        })
    }
    pub fn id(&self) -> &RunId {
        &self.id
    }
    pub fn project(&self) -> Option<&ProjectId> {
        self.project.as_ref()
    }
    pub fn repository(&self) -> Option<&RepositoryId> {
        self.repository.as_ref()
    }
    pub fn result_revision(&self) -> &RevisionId {
        &self.result_revision
    }
    pub fn state(&self) -> RunState {
        self.state
    }
    pub fn tasks(&self) -> &BTreeMap<TaskId, Task> {
        &self.tasks
    }
    pub fn task(&self, id: &TaskId) -> DomainResult<&Task> {
        self.tasks.get(id).ok_or(DomainError::UnknownEntity)
    }
    pub fn candidate(&self) -> Option<&DeliveryCandidate> {
        self.candidate.as_ref()
    }
    pub fn approval(&self) -> Option<&EvidenceId> {
        self.approval.as_ref()
    }
    pub fn application(&self) -> Option<&ApplicationEvidence> {
        self.application.as_ref()
    }
    pub fn application_failures(&self) -> &[ApplicationEvidence] {
        &self.application_failures
    }
    pub fn conflicts(&self) -> &[ApplicationConflict] {
        &self.conflicts
    }
    pub fn cancellation_requested(&self) -> bool {
        self.cancellation_requested
    }
    fn require_mutable(&self) -> DomainResult<()> {
        // Restored counters must reject exhaustion instead of overflowing the
        // next guarded transition's generation increment.
        require(self.generation < u64::MAX, DomainError::InvalidVersion)?;
        require(
            !matches!(
                self.state,
                RunState::Applied | RunState::Cancelled | RunState::Delivered
            ) && !self.cancellation_requested,
            DomainError::InvalidTransition,
        )
    }
    fn invalidate_delivery(&mut self) {
        self.generation += 1;
        self.candidate = None;
        self.approval = None;
        self.state = if self.tasks.values().any(|task| {
            task.state() == TaskState::Blocked
                || task
                    .integrations()
                    .last()
                    .is_some_and(|integration| integration.validation == CheckResult::Failed)
        }) {
            RunState::Blocked
        } else {
            RunState::Working
        };
    }
    fn task_mut(&mut self, id: &TaskId) -> DomainResult<&mut Task> {
        self.tasks.get_mut(id).ok_or(DomainError::UnknownEntity)
    }
    pub fn add_task(&mut self, task: Task) -> DomainResult<()> {
        self.require_mutable()?;
        require(
            task.state() == TaskState::Created,
            DomainError::InvalidTransition,
        )?;
        require(
            !self.tasks.contains_key(task.id()),
            DomainError::DuplicateIdentity,
        )?;
        if task.kind() == TaskKind::Code {
            require(
                self.delivery == DeliveryRequirement::FinalApplication && self.repository.is_some(),
                DomainError::Policy,
            )?;
        }
        // References are resolved on insertion; immutable edges and this order prevent cycles.
        for dependency in task.dependencies() {
            let prerequisite = self.task(&dependency.task)?;
            require(
                matches!(
                    (dependency.result, prerequisite.kind()),
                    (DependencyResult::CodeIntegrated, TaskKind::Code)
                        | (
                            DependencyResult::AnalysisArtifactAccepted,
                            TaskKind::Analysis
                        )
                ),
                DomainError::Policy,
            )?;
        }
        self.tasks.insert(task.id().clone(), task);
        self.invalidate_delivery();
        Ok(())
    }
    pub fn wait_task(&mut self, id: &TaskId) -> DomainResult<()> {
        self.require_mutable()?;
        self.task_mut(id)?.wait()
    }
    pub fn dependencies_satisfied(&self, id: &TaskId) -> DomainResult<bool> {
        Ok(self.task(id)?.dependencies().iter().all(|d| {
            self.tasks
                .get(&d.task)
                .is_some_and(|t| t.releases(d.result))
        }))
    }
    pub fn make_ready(&mut self, id: &TaskId) -> DomainResult<()> {
        self.require_mutable()?;
        require(
            self.dependencies_satisfied(id)?,
            DomainError::DependencyPending,
        )?;
        self.task_mut(id)?.make_ready()
    }
    pub fn assign_attempt(&mut self, id: &TaskId, attempt: TaskAttempt) -> DomainResult<()> {
        self.require_mutable()?;
        require(
            self.dependencies_satisfied(id)?,
            DomainError::DependencyPending,
        )?;
        require(
            attempt.inputs().project.as_ref() == self.project.as_ref(),
            DomainError::Ownership,
        )?;
        require(
            !self
                .tasks
                .values()
                .flat_map(|t| t.attempts())
                .any(|a| a.id() == attempt.id()),
            DomainError::DuplicateIdentity,
        )?;
        require(
            !self
                .tasks
                .values()
                .flat_map(|t| t.attempts())
                .any(|existing| {
                    existing.session().id() == attempt.session().id()
                        && existing.session() != attempt.session()
                }),
            DomainError::Ownership,
        )?;
        require(
            !self
                .tasks
                .values()
                .flat_map(|t| t.attempts())
                .any(|existing| {
                    existing.session().id() == attempt.session().id()
                        && !matches!(
                            existing.state(),
                            AttemptState::Succeeded
                                | AttemptState::Failed
                                | AttemptState::Cancelled
                        )
                }),
            DomainError::InvalidTransition,
        )?;
        self.task_mut(id)?.assign(attempt)
    }
    pub fn accept_frame(
        &mut self,
        id: &TaskId,
        frame: &str,
        origin: EventOrigin,
    ) -> DomainResult<()> {
        require(
            !matches!(
                self.state,
                RunState::Applied | RunState::Delivered | RunState::Cancelled
            ),
            DomainError::InvalidTransition,
        )?;
        let result = self.task_mut(id)?.observe(frame, origin);
        if self.task(id)?.state() == TaskState::Blocked {
            self.state = RunState::Blocked;
        }
        result
    }
    pub fn transport_lost(&mut self, id: &TaskId, reason: &str) -> DomainResult<()> {
        require(
            !matches!(
                self.state,
                RunState::Applied | RunState::Delivered | RunState::Cancelled
            ),
            DomainError::InvalidTransition,
        )?;
        self.task_mut(id)?.transport_lost(reason)?;
        if self.task(id)?.state() == TaskState::Blocked {
            self.state = RunState::Blocked;
        }
        Ok(())
    }
    pub fn request_task_cancel(&mut self, id: &TaskId) -> DomainResult<()> {
        self.require_mutable()?;
        self.task_mut(id)?.request_cancel()
    }
    pub fn publish_artifact(&mut self, id: &TaskId, artifact: ArtifactVersion) -> DomainResult<()> {
        self.require_mutable()?;
        self.task_mut(id)?.publish(artifact)
    }
    pub fn verify_artifact(
        &mut self,
        id: &TaskId,
        evidence: VerificationEvidence,
    ) -> DomainResult<()> {
        self.require_mutable()?;
        self.task_mut(id)?.verify(evidence)
    }
    pub fn review_artifact(&mut self, id: &TaskId, evidence: ReviewEvidence) -> DomainResult<()> {
        self.require_mutable()?;
        self.task_mut(id)?.review(evidence)
    }
    pub fn integrate(&mut self, evidence: IntegrationEvidence) -> DomainResult<()> {
        self.require_mutable()?;
        require(evidence.run == self.id, DomainError::Ownership)?;
        require(
            evidence.input_base == self.result_revision,
            DomainError::StaleEvidence,
        )?;
        let passed = evidence.validation == CheckResult::Passed;
        let revision = evidence.result_revision.clone();
        let id = evidence.task.clone();
        let reference = evidence.reference.clone();
        self.task_mut(&id)?.integrate(evidence)?;
        self.integration_order.push((id, reference));
        if passed {
            self.result_revision = revision;
            self.invalidate_delivery();
            Ok(())
        } else {
            self.candidate = None;
            self.approval = None;
            self.state = RunState::Blocked;
            Err(DomainError::VerificationFailed)
        }
    }
    pub fn complete_analysis(&mut self, id: &TaskId) -> DomainResult<()> {
        self.require_mutable()?;
        self.task_mut(id)?.complete_analysis()?;
        self.invalidate_delivery();
        Ok(())
    }
    fn all_completed(&self) -> bool {
        !self.tasks.is_empty()
            && self
                .tasks
                .values()
                .all(|t| t.state() == TaskState::Completed)
    }
    pub fn prepare_delivery(&mut self, target_base: RevisionId) -> DomainResult<DeliveryCandidate> {
        self.require_mutable()?;
        require(
            self.delivery == DeliveryRequirement::FinalApplication && self.all_completed(),
            DomainError::MissingEvidence,
        )?;
        self.generation += 1;
        let candidate = DeliveryCandidate {
            run: self.id.clone(),
            repository: self
                .repository
                .clone()
                .ok_or(DomainError::MissingEvidence)?,
            result: self.result_revision.clone(),
            target_base,
            generation: self.generation,
            artifacts: self
                .tasks
                .values()
                .filter_map(|t| t.artifact().cloned())
                .collect(),
        };
        self.candidate = Some(candidate.clone());
        self.approval = None;
        self.state = RunState::ReadyToApply;
        Ok(candidate)
    }
    fn require_candidate(
        &mut self,
        candidate: &DeliveryCandidate,
        target_base: &RevisionId,
    ) -> DomainResult<()> {
        if self.candidate.as_ref() != Some(candidate)
            || &candidate.target_base != target_base
            || candidate.result != self.result_revision
            || !self.all_completed()
        {
            self.candidate = None;
            self.approval = None;
            self.state = RunState::Blocked;
            return Err(DomainError::StaleEvidence);
        }
        Ok(())
    }
    pub fn approve_delivery(
        &mut self,
        candidate: &DeliveryCandidate,
        current_target_base: &RevisionId,
        reference: EvidenceId,
    ) -> DomainResult<()> {
        self.require_mutable()?;
        self.require_candidate(candidate, current_target_base)?;
        require(
            self.state == RunState::ReadyToApply && self.approval.is_none(),
            DomainError::InvalidTransition,
        )?;
        self.approval = Some(reference);
        Ok(())
    }
    pub fn observe_target_base(&mut self, target_base: &RevisionId) -> DomainResult<()> {
        self.require_mutable()?;
        if self
            .candidate
            .as_ref()
            .is_some_and(|c| &c.target_base != target_base)
        {
            self.candidate = None;
            self.approval = None;
            self.state = RunState::Blocked;
        }
        Ok(())
    }
    pub fn report_application_conflict(
        &mut self,
        candidate: &DeliveryCandidate,
        target_base: &RevisionId,
        reference: EvidenceId,
    ) -> DomainResult<()> {
        self.require_mutable()?;
        self.require_candidate(candidate, target_base)?;
        require(
            self.state == RunState::ReadyToApply,
            DomainError::InvalidTransition,
        )?;
        require(
            !self
                .conflicts
                .iter()
                .any(|conflict| conflict.reference == reference),
            DomainError::DuplicateIdentity,
        )?;
        self.conflicts.push(ApplicationConflict {
            candidate: candidate.clone(),
            reference,
        });
        self.candidate = None;
        self.approval = None;
        self.state = RunState::Blocked;
        Ok(())
    }
    pub fn confirm_application(
        &mut self,
        candidate: &DeliveryCandidate,
        evidence: ApplicationEvidence,
    ) -> DomainResult<()> {
        self.require_mutable()?;
        self.require_candidate(candidate, &evidence.target_base)?;
        require(
            self.state == RunState::ReadyToApply && self.approval.is_some(),
            DomainError::MissingEvidence,
        )?;
        require(
            evidence.run == self.id && Some(&evidence.repository) == self.repository.as_ref(),
            DomainError::Ownership,
        )?;
        require(
            evidence.result == candidate.result,
            DomainError::StaleEvidence,
        )?;
        if evidence.validation == CheckResult::Failed {
            self.application_failures.push(evidence);
            self.candidate = None;
            self.approval = None;
            self.state = RunState::Blocked;
            return Err(DomainError::VerificationFailed);
        }
        self.application = Some(evidence);
        self.state = RunState::Applied;
        Ok(())
    }
    pub fn mark_delivered(&mut self) -> DomainResult<()> {
        require(
            !self.cancellation_requested && self.all_completed(),
            DomainError::MissingEvidence,
        )?;
        match self.delivery {
            DeliveryRequirement::FinalApplication => require(
                self.state == RunState::Applied && self.application.is_some(),
                DomainError::MissingEvidence,
            )?,
            DeliveryRequirement::AcceptedArtifacts => require(
                self.state == RunState::Working
                    && self.tasks.values().all(|t| t.kind() == TaskKind::Analysis),
                DomainError::InvalidTransition,
            )?,
        }
        self.state = RunState::Delivered;
        Ok(())
    }
    pub fn request_cancel(&mut self) -> DomainResult<()> {
        self.require_mutable()?;
        for task in self.tasks.values_mut() {
            if !matches!(task.state(), TaskState::Completed | TaskState::Cancelled) {
                task.request_cancel()?;
            }
        }
        self.cancellation_requested = true;
        self.candidate = None;
        self.approval = None;
        Ok(())
    }
    pub fn confirm_cancel(&mut self) -> DomainResult<()> {
        require(
            self.cancellation_requested
                && !matches!(
                    self.state,
                    RunState::Applied | RunState::Delivered | RunState::Cancelled
                ),
            DomainError::InvalidTransition,
        )?;
        require(
            self.tasks.values().all(|t| {
                t.current_attempt().is_none_or(|a| {
                    matches!(
                        a.state(),
                        AttemptState::Succeeded | AttemptState::Failed | AttemptState::Cancelled
                    )
                })
            }),
            DomainError::MissingEvidence,
        )?;
        self.state = RunState::Cancelled;
        Ok(())
    }
}
