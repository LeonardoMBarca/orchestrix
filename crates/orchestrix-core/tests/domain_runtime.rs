//! Cross-boundary invariants: typed domain operations consume OX-003 facts.
use orchestrix_core::domain::*;
use orchestrix_core::runtime_contract::{EventOrigin, normalize_descriptor, normalize_request};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/runtime-conformance-v1.json")).unwrap()
}
fn inputs() -> AttemptInputs {
    AttemptInputs {
        plan_revision: RevisionId::new("plan-1").unwrap(),
        context: ContextVersion {
            reference: ContextPackId::new("context-pack-1").unwrap(),
            version: DefinitionVersion::new(1).unwrap(),
        },
        mode: ActionMode::new(
            ModeId::new("research").unwrap(),
            DefinitionVersion::new(1).unwrap(),
            "Research",
            "Explain the evidence",
        )
        .unwrap(),
        profile: AgentProfile::new(
            ProfileId::new("analyst").unwrap(),
            DefinitionVersion::new(1).unwrap(),
            "Analyst",
            "Analyze",
            LocalRef::new("instructions-1").unwrap(),
        )
        .unwrap(),
        base: RevisionId::new("base-1").unwrap(),
        worktree: LocalRef::new("workspace-1").unwrap(),
        project: None,
        correction_of: None,
    }
}
fn setup() -> (Connection, RuntimeSessionRef, TaskAttempt) {
    let data = fixture();
    let connection =
        Connection::new(normalize_descriptor(&data["descriptors"]["base"]).unwrap()).unwrap();
    let session = RuntimeSessionRef::new(RuntimeSessionId::new("session-1").unwrap(), &connection);
    let attempt = TaskAttempt::new(
        &connection,
        &session,
        normalize_request(&data["requests"]["base"]).unwrap(),
        inputs(),
    )
    .unwrap();
    (connection, session, attempt)
}
fn run(attempt: TaskAttempt) -> (Run, TaskId) {
    let id = TaskId::new("analysis-task").unwrap();
    let mut run = Run::new(
        RunId::new("analysis-run").unwrap(),
        None,
        RevisionId::new("base-1").unwrap(),
        DeliveryRequirement::AcceptedArtifacts,
    )
    .unwrap();
    run.add_task(
        Task::new(
            id.clone(),
            "Analyze",
            TaskKind::Analysis,
            vec!["Explain the result".into()],
            vec![],
        )
        .unwrap(),
    )
    .unwrap();
    run.make_ready(&id).unwrap();
    run.assign_attempt(&id, attempt).unwrap();
    (run, id)
}
fn event(sequence: u64, kind: &str, fields: Value) -> String {
    let mut event = json!({"version":{"major":1,"minor":1},"connectionId":"connection-1","connectionRevision":1,"sessionId":"session-1","attemptId":"attempt-1","sequence":sequence,"type":kind});
    for (key, value) in fields.as_object().unwrap() {
        event[key] = value.clone();
    }
    event.to_string()
}

#[test]
fn connection_revision_context_and_subscription_admission_are_checked_before_an_attempt_exists() {
    let (connection, session, _) = setup();
    let data = fixture();
    let mut other = data["descriptors"]["base"].clone();
    other["connection"]["id"] = json!("connection-2");
    let other = Connection::new(normalize_descriptor(&other).unwrap()).unwrap();
    assert_eq!(
        TaskAttempt::new(
            &other,
            &session,
            normalize_request(&data["requests"]["base"]).unwrap(),
            inputs()
        )
        .unwrap_err(),
        DomainError::Ownership
    );
    let mut context = inputs();
    context.context.reference = ContextPackId::new("wrong-context").unwrap();
    assert_eq!(
        TaskAttempt::new(
            &connection,
            &session,
            normalize_request(&data["requests"]["base"]).unwrap(),
            context
        )
        .unwrap_err(),
        DomainError::MissingEvidence
    );
    let mut api = data["descriptors"]["base"].clone();
    api["connection"]["billing"] = json!("api");
    let api = Connection::new(normalize_descriptor(&api).unwrap()).unwrap();
    let session = RuntimeSessionRef::new(RuntimeSessionId::new("session-1").unwrap(), &api);
    let error = TaskAttempt::new(
        &api,
        &session,
        normalize_request(&data["requests"]["base"]).unwrap(),
        inputs(),
    )
    .unwrap_err();
    assert!(matches!(error, DomainError::Runtime(ref contract) if contract.code()=="policy"));
}

#[test]
fn factual_completion_preserves_absent_start_unknown_configuration_and_output_production_result() {
    let (_, _, attempt) = setup();
    let (mut run, id) = run(attempt);
    run.accept_frame(
        &id,
        &event(
            1,
            "attempt.terminal-observed",
            json!({"outcome":"completed"}),
        ),
        EventOrigin::Runtime,
    )
    .unwrap();
    let attempt = run.task(&id).unwrap().current_attempt().unwrap();
    assert_eq!(attempt.state(), AttemptState::Succeeded);
    assert_eq!(attempt.result(), AttemptResult::AwaitingArtifact);
    let observation = attempt.runtime_snapshot();
    assert_eq!(observation["startObserved"], false);
    assert_eq!(observation["admissionObserved"], false);
    assert_eq!(observation["effective"]["model"]["state"], "unknown");
    assert_eq!(observation["requested"]["strategy"], "balanced");
    assert_eq!(run.task(&id).unwrap().state(), TaskState::NeedsReview);
    assert_eq!(
        run.complete_analysis(&id),
        Err(DomainError::InvalidTransition)
    );
    let artifact = ArtifactVersion::new(
        ArtifactId::new("analysis").unwrap(),
        RevisionId::new("content-1").unwrap(),
        RevisionId::new("base-1").unwrap(),
    );
    run.publish_artifact(&id, artifact).unwrap();
    assert_eq!(
        run.task(&id).unwrap().current_attempt().unwrap().result(),
        AttemptResult::ArtifactProduced
    );
    assert_eq!(run.task(&id).unwrap().state(), TaskState::NeedsReview);
}

#[test]
fn owned_exit_tree_and_interrupt_ack_cannot_confirm_attempt_or_task_completion() {
    let (_, _, attempt) = setup();
    let (mut run, id) = run(attempt);
    run.accept_frame(
        &id,
        &event(1, "attempt.start-observed", json!({})),
        EventOrigin::Runtime,
    )
    .unwrap();
    run.request_task_cancel(&id).unwrap();
    run.accept_frame(
        &id,
        &event(
            2,
            "process.exited",
            json!({"role":"runtime","code":137,"signal":"killed"}),
        ),
        EventOrigin::Supervisor,
    )
    .unwrap();
    run.accept_frame(&id, &event(3, "process.tree-observed", json!({"ownership":"confirmed","source":"owned-job","termination":"confirmed","activeCount":0,"evidenceRef":"supervisor-proof"})), EventOrigin::Supervisor).unwrap();
    run.accept_frame(
        &id,
        &event(4, "attempt.interrupt-result", json!({"acknowledged":true})),
        EventOrigin::Runtime,
    )
    .unwrap();
    let attempt = run.task(&id).unwrap().current_attempt().unwrap();
    assert_eq!(attempt.result(), AttemptResult::Uncertain);
    assert!(attempt.cancellation_requested());
    assert_eq!(attempt.runtime_snapshot()["terminalConfirmed"], false);
    assert_eq!(run.task(&id).unwrap().state(), TaskState::Blocked);
    assert_eq!(run.state(), RunState::Blocked);
    assert_eq!(run.make_ready(&id), Err(DomainError::InvalidTransition));
    run.request_cancel().unwrap();
    assert_eq!(run.confirm_cancel(), Err(DomainError::MissingEvidence));
}

#[test]
fn final_application_conflict_keeps_accepted_artifacts_and_invalidates_approval() {
    let data = fixture();
    let (connection, session, _) = setup();
    let project = Project::new(
        ProjectId::new("project-1").unwrap(),
        Repository::new(
            RepositoryId::new("repository-1").unwrap(),
            LocalRef::new("directory-1").unwrap(),
        ),
    );
    let mut input = inputs();
    input.project = Some(project.id().clone());
    let attempt = TaskAttempt::new(
        &connection,
        &session,
        normalize_request(&data["requests"]["base"]).unwrap(),
        input,
    )
    .unwrap();
    let id = TaskId::new("analysis-task").unwrap();
    let mut run = Run::new(
        RunId::new("analysis-run").unwrap(),
        Some(&project),
        RevisionId::new("base-1").unwrap(),
        DeliveryRequirement::FinalApplication,
    )
    .unwrap();
    run.add_task(
        Task::new(
            id.clone(),
            "Write document",
            TaskKind::Analysis,
            vec!["Reviewed document".into()],
            vec![],
        )
        .unwrap(),
    )
    .unwrap();
    run.make_ready(&id).unwrap();
    run.assign_attempt(&id, attempt).unwrap();
    run.accept_frame(
        &id,
        &event(
            1,
            "attempt.terminal-observed",
            json!({"outcome":"completed"}),
        ),
        EventOrigin::Runtime,
    )
    .unwrap();
    let artifact = ArtifactVersion::new(
        ArtifactId::new("document").unwrap(),
        RevisionId::new("content-1").unwrap(),
        RevisionId::new("base-1").unwrap(),
    );
    run.publish_artifact(&id, artifact.clone()).unwrap();
    run.review_artifact(
        &id,
        ReviewEvidence {
            artifact: artifact.clone(),
            reference: EvidenceId::new("review-1").unwrap(),
            result: ReviewResult::Accepted,
        },
    )
    .unwrap();
    run.complete_analysis(&id).unwrap();
    let target = RevisionId::new("target-1").unwrap();
    let candidate = run.prepare_delivery(target.clone()).unwrap();
    run.approve_delivery(&candidate, &target, EvidenceId::new("approval-1").unwrap())
        .unwrap();
    run.report_application_conflict(&candidate, &target, EvidenceId::new("conflict-1").unwrap())
        .unwrap();
    assert_eq!(run.state(), RunState::Blocked);
    assert!(run.approval().is_none());
    assert!(run.application().is_none());
    assert_eq!(run.conflicts()[0].candidate(), &candidate);
    assert_eq!(run.task(&id).unwrap().artifact(), Some(&artifact));
    assert_eq!(run.mark_delivered(), Err(DomainError::MissingEvidence));
    assert_eq!(
        run.approve_delivery(
            &candidate,
            &target,
            EvidenceId::new("old-approval").unwrap()
        ),
        Err(DomainError::StaleEvidence)
    );
    let fresh = run.prepare_delivery(target.clone()).unwrap();
    assert_ne!(fresh, candidate);
    run.approve_delivery(&fresh, &target, EvidenceId::new("new-approval").unwrap())
        .unwrap();
    assert_eq!(run.mark_delivered(), Err(DomainError::MissingEvidence));
}
