//! In-memory domain acceptance using the stabilized OX-003 consumer.
use orchestrix_core::domain::*;
use orchestrix_core::runtime_contract::{EventOrigin, normalize_descriptor, normalize_request};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/runtime-conformance-v1.json")).unwrap()
}
fn revision(value: &str) -> RevisionId {
    RevisionId::new(value).unwrap()
}
fn task_id(value: &str) -> TaskId {
    TaskId::new(value).unwrap()
}
fn proof(value: &str) -> EvidenceId {
    EvidenceId::new(value).unwrap()
}
fn project() -> Project {
    Project::new(
        ProjectId::new("project-1").unwrap(),
        Repository::new(
            RepositoryId::new("repository-1").unwrap(),
            LocalRef::new("granted-directory-1").unwrap(),
        ),
    )
}
fn inputs(project: Option<ProjectId>, base: &str) -> AttemptInputs {
    AttemptInputs {
        plan_revision: revision("plan-1"),
        context: ContextVersion {
            reference: ContextPackId::new("context-pack-1").unwrap(),
            version: DefinitionVersion::new(1).unwrap(),
        },
        mode: ActionMode::new(
            ModeId::new("development").unwrap(),
            DefinitionVersion::new(1).unwrap(),
            "Development",
            "Produce checked work",
        )
        .unwrap(),
        profile: AgentProfile::new(
            ProfileId::new("implementer").unwrap(),
            DefinitionVersion::new(1).unwrap(),
            "Implementer",
            "Write code",
            LocalRef::new("instructions-1").unwrap(),
        )
        .unwrap(),
        base: revision(base),
        worktree: LocalRef::new("worktree-1").unwrap(),
        project,
        correction_of: None,
    }
}
fn attempt(name: &str, input: AttemptInputs) -> TaskAttempt {
    attempt_in_session(name, input, "session-1")
}
fn attempt_in_session(name: &str, input: AttemptInputs, session_id: &str) -> TaskAttempt {
    let data = fixture();
    let connection =
        Connection::new(normalize_descriptor(&data["descriptors"]["base"]).unwrap()).unwrap();
    let session = RuntimeSessionRef::new(RuntimeSessionId::new(session_id).unwrap(), &connection);
    let mut request = data["requests"]["base"].clone();
    request["attemptId"] = json!(name);
    request["sessionId"] = json!(session_id);
    TaskAttempt::new(
        &connection,
        &session,
        normalize_request(&request).unwrap(),
        input,
    )
    .unwrap()
}
fn task(name: &str, kind: TaskKind, dependencies: Vec<TaskDependency>) -> Task {
    Task::new(
        task_id(name),
        "Produce a useful result",
        kind,
        vec!["Result follows the stated objective".into()],
        dependencies,
    )
    .unwrap()
}
fn frame(attempt: &str, sequence: u64, kind: &str, extra: Value) -> String {
    let mut event = json!({"version":{"major":1,"minor":1}, "connectionId":"connection-1",
        "connectionRevision":1,"sessionId":"session-1","attemptId":attempt,"sequence":sequence,"type":kind});
    for (key, value) in extra.as_object().unwrap() {
        event[key] = value.clone();
    }
    event.to_string()
}
fn session_frame(attempt: &str, sequence: u64, session: &str, kind: &str, extra: Value) -> String {
    let mut value: Value = serde_json::from_str(&frame(attempt, sequence, kind, extra)).unwrap();
    value["sessionId"] = json!(session);
    value.to_string()
}
fn completed(run: &mut Run, id: &TaskId, name: &str) {
    run.accept_frame(
        id,
        &frame(name, 1, "attempt.admission-observed", json!({})),
        EventOrigin::Runtime,
    )
    .unwrap();
    run.accept_frame(
        id,
        &frame(name, 2, "attempt.start-observed", json!({})),
        EventOrigin::Runtime,
    )
    .unwrap();
    run.accept_frame(
        id,
        &frame(
            name,
            3,
            "attempt.terminal-observed",
            json!({"outcome":"completed"}),
        ),
        EventOrigin::Runtime,
    )
    .unwrap();
}
fn artifact(name: &str, rev: &str, base: &str) -> ArtifactVersion {
    ArtifactVersion::new(
        ArtifactId::new(name).unwrap(),
        revision(rev),
        revision(base),
    )
}
fn approve(run: &mut Run, id: &TaskId, output: &ArtifactVersion) {
    run.verify_artifact(
        id,
        VerificationEvidence {
            artifact: output.clone(),
            reference: proof(&format!("check-{}", output.revision().as_str())),
            result: CheckResult::Passed,
        },
    )
    .unwrap();
    run.review_artifact(
        id,
        ReviewEvidence {
            artifact: output.clone(),
            reference: proof(&format!("review-{}", output.revision().as_str())),
            result: ReviewResult::Accepted,
        },
    )
    .unwrap();
}

#[test]
fn serial_code_flow_releases_dependency_after_integration_and_delivers_after_application() {
    let p = project();
    let first = task_id("first");
    let second = task_id("second");
    let mut run = Run::new(
        RunId::new("run-1").unwrap(),
        Some(&p),
        revision("base-1"),
        DeliveryRequirement::FinalApplication,
    )
    .unwrap();
    run.add_task(task("first", TaskKind::Code, vec![])).unwrap();
    run.add_task(task(
        "second",
        TaskKind::Analysis,
        vec![TaskDependency {
            task: first.clone(),
            result: DependencyResult::CodeIntegrated,
        }],
    ))
    .unwrap();
    run.wait_task(&first).unwrap();
    run.wait_task(&second).unwrap();
    assert_eq!(run.make_ready(&second), Err(DomainError::DependencyPending));
    run.make_ready(&first).unwrap();
    run.assign_attempt(
        &first,
        attempt("attempt-1", inputs(Some(p.id().clone()), "base-1")),
    )
    .unwrap();
    completed(&mut run, &first, "attempt-1");
    let output = artifact("code", "code-v1", "base-1");
    run.publish_artifact(&first, output.clone()).unwrap();
    approve(&mut run, &first, &output);
    assert_eq!(run.task(&first).unwrap().state(), TaskState::Approved);
    assert!(!run.dependencies_satisfied(&second).unwrap());
    assert_eq!(run.mark_delivered(), Err(DomainError::MissingEvidence));
    run.integrate(IntegrationEvidence {
        run: run.id().clone(),
        task: first.clone(),
        artifact: output,
        input_base: revision("base-1"),
        result_revision: revision("run-v1"),
        reference: proof("integration-1"),
        validation: CheckResult::Passed,
    })
    .unwrap();
    run.make_ready(&second).unwrap();
    run.assign_attempt(
        &second,
        attempt("attempt-2", inputs(Some(p.id().clone()), "run-v1")),
    )
    .unwrap();
    completed(&mut run, &second, "attempt-2");
    let analysis = artifact("analysis", "analysis-v1", "run-v1");
    run.publish_artifact(&second, analysis.clone()).unwrap();
    run.review_artifact(
        &second,
        ReviewEvidence {
            artifact: analysis,
            reference: proof("analysis-review"),
            result: ReviewResult::Accepted,
        },
    )
    .unwrap();
    run.complete_analysis(&second).unwrap();
    let candidate = run.prepare_delivery(revision("target-1")).unwrap();
    assert_eq!(run.state(), RunState::ReadyToApply);
    assert_eq!(candidate.repository(), p.repository().id());
    assert_eq!(run.mark_delivered(), Err(DomainError::MissingEvidence));
    run.approve_delivery(
        &candidate,
        &revision("target-1"),
        proof("delivery-approval"),
    )
    .unwrap();
    run.confirm_application(
        &candidate,
        ApplicationEvidence {
            run: run.id().clone(),
            repository: p.repository().id().clone(),
            result: revision("run-v1"),
            target_base: revision("target-1"),
            applied_revision: revision("destination-v1"),
            reference: proof("application-1"),
            validation: CheckResult::Passed,
        },
    )
    .unwrap();
    assert_eq!(run.state(), RunState::Applied);
    run.mark_delivered().unwrap();
    assert_eq!(run.state(), RunState::Delivered);
}

#[test]
fn runtime_success_without_checks_and_rejected_evidence_cannot_complete_code() {
    let p = project();
    let id = task_id("code");
    let mut run = Run::new(
        RunId::new("run-1").unwrap(),
        Some(&p),
        revision("base-1"),
        DeliveryRequirement::FinalApplication,
    )
    .unwrap();
    run.add_task(task("code", TaskKind::Code, vec![])).unwrap();
    run.make_ready(&id).unwrap();
    run.assign_attempt(
        &id,
        attempt("attempt-1", inputs(Some(p.id().clone()), "base-1")),
    )
    .unwrap();
    completed(&mut run, &id, "attempt-1");
    assert_eq!(run.task(&id).unwrap().state(), TaskState::NeedsReview);
    assert_eq!(
        run.complete_analysis(&id),
        Err(DomainError::InvalidTransition)
    );
    let output = artifact("code", "code-v1", "base-1");
    run.publish_artifact(&id, output.clone()).unwrap();
    run.review_artifact(
        &id,
        ReviewEvidence {
            artifact: output.clone(),
            reference: proof("accepted"),
            result: ReviewResult::Accepted,
        },
    )
    .unwrap();
    assert_eq!(run.task(&id).unwrap().state(), TaskState::NeedsReview);
    let integration = IntegrationEvidence {
        run: run.id().clone(),
        task: id.clone(),
        artifact: output.clone(),
        input_base: revision("base-1"),
        result_revision: revision("run-v1"),
        reference: proof("integration"),
        validation: CheckResult::Passed,
    };
    assert_eq!(
        run.integrate(integration),
        Err(DomainError::InvalidTransition)
    );
    run.verify_artifact(
        &id,
        VerificationEvidence {
            artifact: output,
            reference: proof("failed-check"),
            result: CheckResult::Failed,
        },
    )
    .unwrap();
    assert_eq!(run.task(&id).unwrap().state(), TaskState::NeedsWork);
    assert_eq!(
        run.prepare_delivery(revision("target-1")),
        Err(DomainError::MissingEvidence)
    );
}

#[test]
fn correction_keeps_previous_attempt_and_its_artifact_mode_profile_context() {
    let p = project();
    let id = task_id("code");
    let mut run = Run::new(
        RunId::new("run-1").unwrap(),
        Some(&p),
        revision("base-1"),
        DeliveryRequirement::FinalApplication,
    )
    .unwrap();
    run.add_task(task("code", TaskKind::Code, vec![])).unwrap();
    run.make_ready(&id).unwrap();
    let original = inputs(Some(p.id().clone()), "base-1");
    run.assign_attempt(&id, attempt("attempt-1", original.clone()))
        .unwrap();
    completed(&mut run, &id, "attempt-1");
    let output = artifact("code", "code-v1", "base-1");
    run.publish_artifact(&id, output.clone()).unwrap();
    run.review_artifact(
        &id,
        ReviewEvidence {
            artifact: output.clone(),
            reference: proof("rejected"),
            result: ReviewResult::Rejected,
        },
    )
    .unwrap();
    run.make_ready(&id).unwrap();
    assert_eq!(
        run.assign_attempt(&id, attempt("attempt-bad", original.clone())),
        Err(DomainError::StaleEvidence)
    );
    let mut corrected = original.clone();
    corrected.correction_of = Some(output.clone());
    assert_eq!(
        run.assign_attempt(&id, attempt("attempt-wrong-base", corrected.clone())),
        Err(DomainError::StaleEvidence)
    );
    assert_eq!(run.task(&id).unwrap().attempts().len(), 1);
    assert_eq!(
        run.task(&id).unwrap().attempts()[0].artifacts(),
        std::slice::from_ref(&output)
    );
    corrected.base = output.revision().clone();
    corrected.context.version = DefinitionVersion::new(2).unwrap();
    corrected.mode = original
        .mode
        .revise(
            DefinitionVersion::new(2).unwrap(),
            "Development",
            "Use narrower changes",
        )
        .unwrap();
    corrected.profile = original
        .profile
        .revise(
            DefinitionVersion::new(2).unwrap(),
            "Implementer",
            "Write small changes",
            LocalRef::new("instructions-2").unwrap(),
        )
        .unwrap();
    run.assign_attempt(&id, attempt("attempt-2", corrected))
        .unwrap();
    let history = run.task(&id).unwrap().attempts();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].inputs(), &original);
    assert_eq!(history[0].artifacts(), &[output]);
    assert_eq!(history[0].state(), AttemptState::Succeeded);
    assert_eq!(
        history[0].runtime_snapshot()["requested"]["strategy"],
        "balanced"
    );
    assert_eq!(history[1].inputs().mode.version().get(), 2);
    assert_eq!(run.task(&id).unwrap().reviews().len(), 1);
}

#[test]
fn late_cleanup_keeps_explicit_retry_and_cancellation_states() {
    for cancel_after_failure in [false, true] {
        let id = task_id("analysis");
        let mut run = Run::new(
            RunId::new("run-analysis").unwrap(),
            None,
            revision("base-1"),
            DeliveryRequirement::AcceptedArtifacts,
        )
        .unwrap();
        run.add_task(task("analysis", TaskKind::Analysis, vec![]))
            .unwrap();
        run.make_ready(&id).unwrap();
        run.assign_attempt(&id, attempt("attempt-1", inputs(None, "base-1")))
            .unwrap();
        run.accept_frame(
            &id,
            &frame(
                "attempt-1",
                1,
                "attempt.terminal-observed",
                json!({"outcome":"failed"}),
            ),
            EventOrigin::Runtime,
        )
        .unwrap();
        let expected = if cancel_after_failure {
            run.request_task_cancel(&id).unwrap();
            TaskState::Cancelled
        } else {
            run.make_ready(&id).unwrap();
            TaskState::Ready
        };
        run.accept_frame(
            &id,
            &frame(
                "attempt-1",
                2,
                "process.exited",
                json!({"role":"runtime","code":1}),
            ),
            EventOrigin::Supervisor,
        )
        .unwrap();
        assert_eq!(run.task(&id).unwrap().state(), expected);
        assert_eq!(
            run.task(&id).unwrap().current_attempt().unwrap().state(),
            AttemptState::Failed
        );
    }
}

#[test]
fn direct_session_reference_construction_cannot_mix_connections_in_run_history() {
    let first = task_id("first");
    let second = task_id("second");
    let mut run = Run::new(
        RunId::new("run-analysis").unwrap(),
        None,
        revision("base-1"),
        DeliveryRequirement::AcceptedArtifacts,
    )
    .unwrap();
    run.add_task(task("first", TaskKind::Analysis, vec![]))
        .unwrap();
    run.add_task(task("second", TaskKind::Analysis, vec![]))
        .unwrap();
    run.make_ready(&first).unwrap();
    run.make_ready(&second).unwrap();
    run.assign_attempt(&first, attempt("attempt-1", inputs(None, "base-1")))
        .unwrap();
    let data = fixture();
    for (connection_id, connection_revision) in [("connection-2", 1), ("connection-1", 2)] {
        let mut descriptor = data["descriptors"]["base"].clone();
        descriptor["connection"]["id"] = json!(connection_id);
        descriptor["connection"]["revision"] = json!(connection_revision);
        let connection = Connection::new(normalize_descriptor(&descriptor).unwrap()).unwrap();
        let session =
            RuntimeSessionRef::new(RuntimeSessionId::new("session-1").unwrap(), &connection);
        let mut request = data["requests"]["base"].clone();
        request["connectionId"] = json!(connection_id);
        request["connectionRevision"] = json!(connection_revision);
        request["attemptId"] = json!("attempt-2");
        let other = TaskAttempt::new(
            &connection,
            &session,
            normalize_request(&request).unwrap(),
            inputs(None, "base-1"),
        )
        .unwrap();
        assert_eq!(
            run.assign_attempt(&second, other),
            Err(DomainError::Ownership)
        );
        assert!(run.task(&second).unwrap().attempts().is_empty());
    }
}

#[test]
fn independent_work_does_not_clear_unresolved_attempt_blocking_run() {
    let p = project();
    let code = task_id("code");
    let analysis = task_id("analysis");
    let mut run = Run::new(
        RunId::new("run-1").unwrap(),
        Some(&p),
        revision("base-1"),
        DeliveryRequirement::FinalApplication,
    )
    .unwrap();
    run.add_task(task("code", TaskKind::Code, vec![])).unwrap();
    run.make_ready(&code).unwrap();
    run.assign_attempt(
        &code,
        attempt("attempt-1", inputs(Some(p.id().clone()), "base-1")),
    )
    .unwrap();
    run.accept_frame(
        &code,
        &frame("attempt-1", 1, "attempt.start-observed", json!({})),
        EventOrigin::Runtime,
    )
    .unwrap();
    run.transport_lost(&code, "timeout").unwrap();
    assert_eq!(run.state(), RunState::Blocked);
    run.add_task(task("analysis", TaskKind::Analysis, vec![]))
        .unwrap();
    assert_eq!(run.state(), RunState::Blocked);
    run.make_ready(&analysis).unwrap();
    run.assign_attempt(
        &analysis,
        attempt_in_session(
            "attempt-2",
            inputs(Some(p.id().clone()), "base-1"),
            "session-2",
        ),
    )
    .unwrap();
    run.accept_frame(
        &analysis,
        &session_frame(
            "attempt-2",
            1,
            "session-2",
            "attempt.terminal-observed",
            json!({"outcome":"completed"}),
        ),
        EventOrigin::Runtime,
    )
    .unwrap();
    let report = artifact("report", "report-v1", "base-1");
    run.publish_artifact(&analysis, report.clone()).unwrap();
    run.review_artifact(
        &analysis,
        ReviewEvidence {
            artifact: report,
            reference: proof("report-review"),
            result: ReviewResult::Accepted,
        },
    )
    .unwrap();
    run.complete_analysis(&analysis).unwrap();
    assert_eq!(run.state(), RunState::Blocked);
    run.accept_frame(
        &code,
        &frame(
            "attempt-1",
            2,
            "attempt.terminal-observed",
            json!({"outcome":"completed"}),
        ),
        EventOrigin::Runtime,
    )
    .unwrap();
    let output = artifact("code", "code-v1", "base-1");
    run.publish_artifact(&code, output.clone()).unwrap();
    approve(&mut run, &code, &output);
    run.integrate(IntegrationEvidence {
        run: run.id().clone(),
        task: code,
        artifact: output,
        input_base: revision("base-1"),
        result_revision: revision("run-v1"),
        reference: proof("integration"),
        validation: CheckResult::Passed,
    })
    .unwrap();
    assert_eq!(run.state(), RunState::Working);
}

#[test]
fn a_session_has_one_unresolved_turn_and_can_be_reused_after_confirmed_terminal() {
    let first = task_id("first");
    let second = task_id("second");
    let separate = task_id("separate");
    let mut run = Run::new(
        RunId::new("run-analysis").unwrap(),
        None,
        revision("base-1"),
        DeliveryRequirement::AcceptedArtifacts,
    )
    .unwrap();
    for name in ["first", "second", "separate"] {
        run.add_task(task(name, TaskKind::Analysis, vec![]))
            .unwrap();
        run.make_ready(&task_id(name)).unwrap();
    }
    run.assign_attempt(&first, attempt("attempt-1", inputs(None, "base-1")))
        .unwrap();
    assert_eq!(
        run.assign_attempt(&second, attempt("attempt-2", inputs(None, "base-1"))),
        Err(DomainError::InvalidTransition)
    );
    assert!(run.task(&second).unwrap().attempts().is_empty());
    run.assign_attempt(
        &separate,
        attempt_in_session("attempt-separate", inputs(None, "base-1"), "session-2"),
    )
    .unwrap();
    run.transport_lost(&first, "timeout").unwrap();
    assert_eq!(
        run.assign_attempt(&second, attempt("attempt-2", inputs(None, "base-1"))),
        Err(DomainError::InvalidTransition)
    );
    run.accept_frame(
        &first,
        &frame(
            "attempt-1",
            1,
            "attempt.terminal-observed",
            json!({"outcome":"completed"}),
        ),
        EventOrigin::Runtime,
    )
    .unwrap();
    run.assign_attempt(&second, attempt("attempt-2", inputs(None, "base-1")))
        .unwrap();
    assert_eq!(
        run.task(&first).unwrap().current_attempt().unwrap().state(),
        AttemptState::Succeeded
    );
    assert_eq!(run.task(&second).unwrap().state(), TaskState::Assigned);
    assert_eq!(
        run.task(&separate)
            .unwrap()
            .current_attempt()
            .unwrap()
            .session()
            .id()
            .as_str(),
        "session-2"
    );
}

#[test]
fn avulsa_analysis_acceptance_needs_no_git_but_code_cannot_bypass_application() {
    let id = task_id("analysis");
    let mut run = Run::new(
        RunId::new("run-analysis").unwrap(),
        None,
        revision("source-1"),
        DeliveryRequirement::AcceptedArtifacts,
    )
    .unwrap();
    assert_eq!(
        run.add_task(task("code", TaskKind::Code, vec![])),
        Err(DomainError::Policy)
    );
    run.add_task(task("analysis", TaskKind::Analysis, vec![]))
        .unwrap();
    run.make_ready(&id).unwrap();
    run.assign_attempt(&id, attempt("attempt-1", inputs(None, "source-1")))
        .unwrap();
    // A factual terminal need not invent an observed start or admission ACK.
    run.accept_frame(
        &id,
        &frame(
            "attempt-1",
            1,
            "attempt.terminal-observed",
            json!({"outcome":"completed"}),
        ),
        EventOrigin::Runtime,
    )
    .unwrap();
    assert_eq!(
        run.task(&id)
            .unwrap()
            .current_attempt()
            .unwrap()
            .runtime_snapshot()["startObserved"],
        false
    );
    let output = artifact("analysis", "analysis-v1", "source-1");
    run.publish_artifact(&id, output.clone()).unwrap();
    run.review_artifact(
        &id,
        ReviewEvidence {
            artifact: output,
            reference: proof("accepted-analysis"),
            result: ReviewResult::Accepted,
        },
    )
    .unwrap();
    assert_eq!(run.task(&id).unwrap().state(), TaskState::Approved);
    run.complete_analysis(&id).unwrap();
    assert!(run.task(&id).unwrap().integrations().is_empty());
    run.mark_delivered().unwrap();
    assert_eq!(run.state(), RunState::Delivered);
}

#[test]
fn cancellation_and_transport_uncertainty_do_not_confirm_termination_or_retry() {
    let id = task_id("analysis");
    let mut run = Run::new(
        RunId::new("run-analysis").unwrap(),
        None,
        revision("base-1"),
        DeliveryRequirement::AcceptedArtifacts,
    )
    .unwrap();
    run.add_task(task("analysis", TaskKind::Analysis, vec![]))
        .unwrap();
    run.make_ready(&id).unwrap();
    run.assign_attempt(&id, attempt("attempt-1", inputs(None, "base-1")))
        .unwrap();
    run.accept_frame(
        &id,
        &frame("attempt-1", 1, "attempt.start-observed", json!({})),
        EventOrigin::Runtime,
    )
    .unwrap();
    run.request_task_cancel(&id).unwrap();
    assert_eq!(run.task(&id).unwrap().state(), TaskState::Running);
    run.accept_frame(
        &id,
        &frame(
            "attempt-1",
            2,
            "attempt.interrupt-result",
            json!({"acknowledged":true}),
        ),
        EventOrigin::Runtime,
    )
    .unwrap();
    assert_eq!(
        run.task(&id).unwrap().current_attempt().unwrap().state(),
        AttemptState::Running
    );
    run.transport_lost(&id, "timeout").unwrap();
    assert_eq!(run.task(&id).unwrap().state(), TaskState::Blocked);
    assert_eq!(run.make_ready(&id), Err(DomainError::InvalidTransition));
    run.request_cancel().unwrap();
    assert_eq!(run.confirm_cancel(), Err(DomainError::MissingEvidence));
    run.accept_frame(
        &id,
        &frame(
            "attempt-1",
            3,
            "attempt.terminal-observed",
            json!({"outcome":"interrupted"}),
        ),
        EventOrigin::Runtime,
    )
    .unwrap();
    assert_eq!(run.task(&id).unwrap().state(), TaskState::Cancelled);
    run.confirm_cancel().unwrap();
    assert_eq!(run.state(), RunState::Cancelled);
}

#[test]
fn approval_is_bound_to_exact_artifact_and_target_base() {
    let p = project();
    let id = task_id("code");
    let mut run = Run::new(
        RunId::new("run-1").unwrap(),
        Some(&p),
        revision("base-1"),
        DeliveryRequirement::FinalApplication,
    )
    .unwrap();
    run.add_task(task("code", TaskKind::Code, vec![])).unwrap();
    run.make_ready(&id).unwrap();
    run.assign_attempt(
        &id,
        attempt("attempt-1", inputs(Some(p.id().clone()), "base-1")),
    )
    .unwrap();
    completed(&mut run, &id, "attempt-1");
    let old = artifact("code", "old", "base-1");
    let new = artifact("code", "new", "base-1");
    run.publish_artifact(&id, old.clone()).unwrap();
    run.review_artifact(
        &id,
        ReviewEvidence {
            artifact: old.clone(),
            reference: proof("old-review"),
            result: ReviewResult::Accepted,
        },
    )
    .unwrap();
    run.publish_artifact(&id, new.clone()).unwrap();
    assert_eq!(
        run.verify_artifact(
            &id,
            VerificationEvidence {
                artifact: old,
                reference: proof("old-check"),
                result: CheckResult::Passed
            }
        ),
        Err(DomainError::StaleEvidence)
    );
    approve(&mut run, &id, &new);
    let stale = IntegrationEvidence {
        run: run.id().clone(),
        task: id.clone(),
        artifact: new.clone(),
        input_base: revision("different-base"),
        result_revision: revision("run-v1"),
        reference: proof("integration"),
        validation: CheckResult::Passed,
    };
    assert_eq!(run.integrate(stale), Err(DomainError::StaleEvidence));
    run.integrate(IntegrationEvidence {
        run: run.id().clone(),
        task: id,
        artifact: new,
        input_base: revision("base-1"),
        result_revision: revision("run-v1"),
        reference: proof("integration"),
        validation: CheckResult::Passed,
    })
    .unwrap();
    let candidate = run.prepare_delivery(revision("target-1")).unwrap();
    run.approve_delivery(&candidate, &revision("target-1"), proof("approval"))
        .unwrap();
    run.observe_target_base(&revision("target-2")).unwrap();
    assert!(run.approval().is_none());
    assert_eq!(run.state(), RunState::Blocked);
    assert_eq!(
        run.approve_delivery(&candidate, &revision("target-2"), proof("stale")),
        Err(DomainError::StaleEvidence)
    );
    assert_eq!(run.mark_delivered(), Err(DomainError::MissingEvidence));
}

#[test]
fn explicit_failed_runtime_and_invalid_graph_edges_do_not_release_dependents() {
    let p = project();
    let id = task_id("code");
    let mut run = Run::new(
        RunId::new("run-1").unwrap(),
        Some(&p),
        revision("base-1"),
        DeliveryRequirement::FinalApplication,
    )
    .unwrap();
    run.add_task(task("code", TaskKind::Code, vec![])).unwrap();
    assert_eq!(
        run.add_task(task(
            "invalid",
            TaskKind::Analysis,
            vec![TaskDependency {
                task: id.clone(),
                result: DependencyResult::AnalysisArtifactAccepted
            }]
        )),
        Err(DomainError::Policy)
    );
    let child = task_id("child");
    run.add_task(task(
        "child",
        TaskKind::Code,
        vec![TaskDependency {
            task: id.clone(),
            result: DependencyResult::CodeIntegrated,
        }],
    ))
    .unwrap();
    assert_eq!(
        run.assign_attempt(
            &id,
            attempt("attempt-premature", inputs(Some(p.id().clone()), "base-1"))
        ),
        Err(DomainError::InvalidTransition)
    );
    run.make_ready(&id).unwrap();
    run.assign_attempt(
        &id,
        attempt("attempt-1", inputs(Some(p.id().clone()), "base-1")),
    )
    .unwrap();
    run.accept_frame(
        &id,
        &frame(
            "attempt-1",
            1,
            "attempt.terminal-observed",
            json!({"outcome":"failed"}),
        ),
        EventOrigin::Runtime,
    )
    .unwrap();
    assert_eq!(run.task(&id).unwrap().state(), TaskState::Failed);
    assert_eq!(
        run.publish_artifact(&id, artifact("code", "bad", "base-1")),
        Err(DomainError::InvalidTransition)
    );
    assert_eq!(run.make_ready(&child), Err(DomainError::DependencyPending));
    run.make_ready(&id).unwrap();
    run.assign_attempt(
        &id,
        attempt("attempt-2", inputs(Some(p.id().clone()), "base-1")),
    )
    .unwrap();
    assert_eq!(
        run.task(&id).unwrap().attempts()[0].state(),
        AttemptState::Failed
    );
}
