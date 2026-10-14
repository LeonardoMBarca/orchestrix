//! Durable-state boundary tests; no provider, process, filesystem or SQL effects.
use orchestrix_core::domain::*;
use orchestrix_core::runtime_contract::{
    EventOrigin, Limits, RuntimeObserver, normalize_descriptor, normalize_request,
};
use serde_json::{Value, json};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/runtime-conformance-v1.json")).unwrap()
}
fn revision(id: &str) -> RevisionId {
    RevisionId::new(id).unwrap()
}
fn proof(id: &str) -> EvidenceId {
    EvidenceId::new(id).unwrap()
}
fn tid() -> TaskId {
    TaskId::new("task-1").unwrap()
}
fn project() -> Project {
    Project::new(
        ProjectId::new("project-1").unwrap(),
        Repository::new(
            RepositoryId::new("repository-1").unwrap(),
            LocalRef::new("directory-1").unwrap(),
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
            "Coder",
            LocalRef::new("instructions-1").unwrap(),
        )
        .unwrap(),
        base: revision(base),
        worktree: LocalRef::new("worktree-1").unwrap(),
        project,
        correction_of: None,
    }
}
fn attempt(id: &str, input: AttemptInputs) -> TaskAttempt {
    let data = fixture();
    let connection =
        Connection::new(normalize_descriptor(&data["descriptors"]["base"]).unwrap()).unwrap();
    let session = RuntimeSessionRef::new(RuntimeSessionId::new("session-1").unwrap(), &connection);
    let mut request = data["requests"]["base"].clone();
    request["attemptId"] = json!(id);
    TaskAttempt::new(
        &connection,
        &session,
        normalize_request(&request).unwrap(),
        input,
    )
    .unwrap()
}
fn event(id: &str, sequence: u64, kind: &str, extra: Value) -> String {
    let mut value = json!({"version":{"major":1,"minor":1}, "connectionId":"connection-1", "connectionRevision":1, "sessionId":"session-1", "attemptId":id, "sequence":sequence, "type":kind});
    for (key, value_extra) in extra.as_object().unwrap() {
        value[key] = value_extra.clone();
    }
    value.to_string()
}
fn run(kind: TaskKind) -> Run {
    let p = project();
    let mut run = Run::new(
        RunId::new("run-1").unwrap(),
        (kind == TaskKind::Code).then_some(&p),
        revision("base-1"),
        if kind == TaskKind::Code {
            DeliveryRequirement::FinalApplication
        } else {
            DeliveryRequirement::AcceptedArtifacts
        },
    )
    .unwrap();
    run.add_task(
        Task::new(
            tid(),
            "Produce work",
            kind,
            vec!["Checked output".into()],
            vec![],
        )
        .unwrap(),
    )
    .unwrap();
    run.make_ready(&tid()).unwrap();
    run.assign_attempt(
        &tid(),
        attempt("attempt-1", inputs(run.project().cloned(), "base-1")),
    )
    .unwrap();
    run
}
fn finish(run: &mut Run, name: &str) {
    run.accept_frame(
        &tid(),
        &event(
            name,
            1,
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
fn approve(run: &mut Run, artifact: &ArtifactVersion, suffix: &str) {
    run.publish_artifact(&tid(), artifact.clone()).unwrap();
    run.verify_artifact(
        &tid(),
        VerificationEvidence {
            artifact: artifact.clone(),
            reference: proof(&format!("check-{suffix}")),
            result: CheckResult::Passed,
        },
    )
    .unwrap();
    run.review_artifact(
        &tid(),
        ReviewEvidence {
            artifact: artifact.clone(),
            reference: proof(&format!("review-{suffix}")),
            result: ReviewResult::Accepted,
        },
    )
    .unwrap();
}
fn integrated() -> Run {
    let mut run = run(TaskKind::Code);
    finish(&mut run, "attempt-1");
    let artifact = artifact("artifact-1", "output-1", "base-1");
    approve(&mut run, &artifact, "1");
    run.integrate(IntegrationEvidence {
        run: run.id().clone(),
        task: tid(),
        artifact,
        input_base: revision("base-1"),
        result_revision: revision("integrated-1"),
        reference: proof("integration-1"),
        validation: CheckResult::Passed,
    })
    .unwrap();
    run
}

#[test]
fn observer_restores_original_bytes_and_can_continue_without_raw_payloads() {
    let data = fixture();
    let mut observer = RuntimeObserver::new(
        normalize_descriptor(&data["descriptors"]["base"]).unwrap(),
        normalize_request(&data["requests"]["base"]).unwrap(),
        Limits::default(),
    )
    .unwrap();
    let frame = event(
        "attempt-1",
        1,
        "attempt.start-observed",
        json!({"futurePrivate":"synthetic-secret-never-retained", "unicode":"飞行"}),
    );
    observer.accept(&frame, EventOrigin::Runtime).unwrap();
    let checkpoint = observer.checkpoint();
    assert!(!checkpoint.to_string().contains("synthetic-secret"));
    let mut restored = RuntimeObserver::restore_checkpoint(&checkpoint).unwrap();
    assert_eq!(restored.snapshot(), observer.snapshot());
    assert_eq!(restored.snapshot()["budget"]["bytes"], frame.len());
    restored
        .accept(
            &event(
                "attempt-1",
                2,
                "attempt.terminal-observed",
                json!({"outcome":"completed"}),
            ),
            EventOrigin::Runtime,
        )
        .unwrap();
    assert_eq!(restored.snapshot()["outcome"], "completed");
    assert_eq!(restored.snapshot()["admissionObserved"], false);
}

#[test]
fn observer_quarantine_and_owned_cleanup_survive_checkpoint() {
    let data = fixture();
    let mut observer = RuntimeObserver::new(
        normalize_descriptor(&data["descriptors"]["base"]).unwrap(),
        normalize_request(&data["requests"]["base"]).unwrap(),
        Limits::default(),
    )
    .unwrap();
    assert!(
        observer
            .accept("{synthetic-secret-bad-frame", EventOrigin::Runtime)
            .is_err()
    );
    assert!(
        !observer
            .checkpoint()
            .to_string()
            .contains("synthetic-secret")
    );
    let mut restored = RuntimeObserver::restore_checkpoint(&observer.checkpoint()).unwrap();
    assert_eq!(restored.snapshot(), observer.snapshot());
    restored
        .accept(
            &event(
                "attempt-1",
                1,
                "process.exited",
                json!({"role":"runtime", "code":137, "signal":"killed"}),
            ),
            EventOrigin::Supervisor,
        )
        .unwrap();
    assert_eq!(restored.snapshot()["outcome"], "unknown");
    restored.close();
    let mut restored = RuntimeObserver::restore_checkpoint(&restored.checkpoint()).unwrap();
    assert!(
        restored
            .accept(
                &event(
                    "attempt-1",
                    2,
                    "attempt.terminal-observed",
                    json!({"outcome":"completed"})
                ),
                EventOrigin::Runtime
            )
            .is_err()
    );
    assert_eq!(restored.snapshot()["observation"], "closed");
}

#[test]
fn observer_checkpoint_tampering_cannot_bypass_limits_ownership_or_snapshot() {
    let data = fixture();
    let mut observer = RuntimeObserver::new(
        normalize_descriptor(&data["descriptors"]["base"]).unwrap(),
        normalize_request(&data["requests"]["base"]).unwrap(),
        Limits::default(),
    )
    .unwrap();
    observer
        .accept(
            &event("attempt-1", 1, "attempt.start-observed", json!({})),
            EventOrigin::Runtime,
        )
        .unwrap();
    for (pointer, replacement) in [
        ("/snapshot/outcome", json!("completed")),
        ("/history/0/event/attemptId", json!("other-attempt")),
        ("/history/0/bytes", json!(0)),
        ("/history/0/origin", json!("supervisor")),
        ("/limits/maxEvents", json!(0)),
        ("/checkpointVersion", json!(2)),
    ] {
        let mut checkpoint = observer.checkpoint();
        *checkpoint.pointer_mut(pointer).unwrap() = replacement;
        assert!(
            RuntimeObserver::restore_checkpoint(&checkpoint).is_err(),
            "{pointer}"
        );
    }
}

#[test]
fn run_checkpoint_reopens_active_attempt_then_continues_domain_delivery() {
    let original = run(TaskKind::Analysis);
    let mut run = Run::restore_checkpoint(&original.checkpoint()).unwrap();
    assert_eq!(run.checkpoint(), original.checkpoint());
    assert_eq!(
        run.task(&tid()).unwrap().current_attempt().unwrap().state(),
        AttemptState::Created
    );
    finish(&mut run, "attempt-1");
    let output = artifact("analysis-1", "analysis-content-1", "base-1");
    approve(&mut run, &output, "analysis");
    run.complete_analysis(&tid()).unwrap();
    run.mark_delivered().unwrap();
    let restored = Run::restore_checkpoint(&run.checkpoint()).unwrap();
    assert_eq!(restored.state(), RunState::Delivered);
    assert_eq!(restored.checkpoint(), run.checkpoint());
}

#[test]
fn integration_failure_reopens_blocked_with_evidence_and_can_be_retried_explicitly() {
    let mut run = run(TaskKind::Code);
    finish(&mut run, "attempt-1");
    let output = artifact("output", "content-1", "base-1");
    approve(&mut run, &output, "1");
    let evidence = IntegrationEvidence {
        run: run.id().clone(),
        task: tid(),
        artifact: output.clone(),
        input_base: revision("base-1"),
        result_revision: revision("failed-result"),
        reference: proof("failed-integration"),
        validation: CheckResult::Failed,
    };
    assert_eq!(
        run.integrate(evidence.clone()),
        Err(DomainError::VerificationFailed)
    );
    let mut restored = Run::restore_checkpoint(&run.checkpoint()).unwrap();
    assert_eq!(restored.state(), RunState::Blocked);
    assert_eq!(restored.task(&tid()).unwrap().state(), TaskState::Approved);
    assert_eq!(restored.task(&tid()).unwrap().integrations(), &[evidence]);
    restored
        .integrate(IntegrationEvidence {
            run: restored.id().clone(),
            task: tid(),
            artifact: output,
            input_base: revision("base-1"),
            result_revision: revision("integrated"),
            reference: proof("integration-retry"),
            validation: CheckResult::Passed,
        })
        .unwrap();
    assert_eq!(restored.state(), RunState::Working);
}

#[test]
fn failed_final_application_evidence_and_invalidated_approval_survive() {
    let mut run = integrated();
    let candidate = run.prepare_delivery(revision("target-1")).unwrap();
    run.approve_delivery(&candidate, &revision("target-1"), proof("approval-1"))
        .unwrap();
    let evidence = ApplicationEvidence {
        run: run.id().clone(),
        repository: run.repository().unwrap().clone(),
        result: run.result_revision().clone(),
        target_base: revision("target-1"),
        applied_revision: revision("failed-application"),
        reference: proof("failed-application-proof"),
        validation: CheckResult::Failed,
    };
    assert_eq!(
        run.confirm_application(&candidate, evidence.clone()),
        Err(DomainError::VerificationFailed)
    );
    let mut restored = Run::restore_checkpoint(&run.checkpoint()).unwrap();
    assert_eq!(restored.application_failures(), &[evidence]);
    assert!(restored.approval().is_none());
    assert!(restored.candidate().is_none());
    assert_eq!(
        restored.approve_delivery(&candidate, &revision("target-1"), proof("old-candidate")),
        Err(DomainError::StaleEvidence)
    );
    let next = restored.prepare_delivery(revision("target-1")).unwrap();
    assert_ne!(next.checkpoint(), candidate.checkpoint());
    let next = DeliveryCandidate::restore_checkpoint(&next.checkpoint()).unwrap();
    restored
        .approve_delivery(&next, &revision("target-1"), proof("approval-2"))
        .unwrap();
    let success = ApplicationEvidence {
        run: restored.id().clone(),
        repository: restored.repository().unwrap().clone(),
        result: restored.result_revision().clone(),
        target_base: revision("target-1"),
        applied_revision: revision("applied"),
        reference: proof("application-success"),
        validation: CheckResult::Passed,
    };
    restored.confirm_application(&next, success).unwrap();
    restored.mark_delivered().unwrap();
    assert_eq!(
        Run::restore_checkpoint(&restored.checkpoint())
            .unwrap()
            .state(),
        RunState::Delivered
    );
}

#[test]
fn cancellation_of_ready_candidate_reopens_without_recreating_approval() {
    let mut run = integrated();
    let candidate = run.prepare_delivery(revision("target-1")).unwrap();
    run.approve_delivery(
        &candidate,
        &revision("target-1"),
        proof("approval-before-cancel"),
    )
    .unwrap();
    run.request_cancel().unwrap();
    assert_eq!(run.state(), RunState::ReadyToApply);
    let mut restored = Run::restore_checkpoint(&run.checkpoint()).unwrap();
    assert!(restored.candidate().is_none());
    assert!(restored.approval().is_none());
    restored.confirm_cancel().unwrap();
    assert_eq!(
        Run::restore_checkpoint(&restored.checkpoint())
            .unwrap()
            .state(),
        RunState::Cancelled
    );
}

#[test]
fn restored_integration_history_cannot_invent_success_or_another_run_result() {
    let original = integrated();
    for (pointer, replacement) in [
        ("/data/resultRevision", json!("unobserved-result")),
        (
            "/data/tasks/0/data/integrations/0/inputBase",
            json!("unobserved-input"),
        ),
        ("/data/tasks/0/data/verifications", json!([])),
        ("/data/tasks/0/data/reviews", json!([])),
        ("/data/integrationOrder", json!([])),
    ] {
        let mut value = original.checkpoint();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(Run::restore_checkpoint(&value).is_err(), "{pointer}");
    }
}

#[test]
fn correction_restores_sealed_history_and_rejects_old_artifact_approval() {
    let mut run = run(TaskKind::Code);
    finish(&mut run, "attempt-1");
    let old = artifact("output", "content-1", "base-1");
    run.publish_artifact(&tid(), old.clone()).unwrap();
    run.review_artifact(
        &tid(),
        ReviewEvidence {
            artifact: old.clone(),
            reference: proof("rejected"),
            result: ReviewResult::Rejected,
        },
    )
    .unwrap();
    run.make_ready(&tid()).unwrap();
    let mut next_inputs = inputs(run.project().cloned(), "content-1");
    next_inputs.correction_of = Some(old.clone());
    run.assign_attempt(&tid(), attempt("attempt-2", next_inputs))
        .unwrap();
    let mut restored = Run::restore_checkpoint(&run.checkpoint()).unwrap();
    assert_eq!(restored.checkpoint(), run.checkpoint());
    assert_eq!(restored.task(&tid()).unwrap().attempts().len(), 2);
    finish(&mut restored, "attempt-2");
    let new = artifact("output", "content-2", "content-1");
    restored.publish_artifact(&tid(), new).unwrap();
    assert_eq!(
        restored.verify_artifact(
            &tid(),
            VerificationEvidence {
                artifact: old,
                reference: proof("stale-check"),
                result: CheckResult::Passed
            }
        ),
        Err(DomainError::StaleEvidence)
    );
    let mut forged = restored.checkpoint();
    forged["data"]["tasks"][0]["data"]["attempts"][0]["data"]["sealed"] = json!(false);
    assert!(Run::restore_checkpoint(&forged).is_err());
}

#[test]
fn current_artifact_cannot_revert_to_an_earlier_publish_in_the_same_attempt() {
    let mut run = run(TaskKind::Analysis);
    finish(&mut run, "attempt-1");
    let old = artifact("output", "content-1", "base-1");
    let latest = artifact("output", "content-2", "base-1");
    run.publish_artifact(&tid(), old.clone()).unwrap();
    run.publish_artifact(&tid(), latest).unwrap();
    assert_eq!(
        Run::restore_checkpoint(&run.checkpoint())
            .unwrap()
            .checkpoint(),
        run.checkpoint()
    );
    let mut checkpoint = run.checkpoint();
    checkpoint["data"]["tasks"][0]["data"]["artifact"] = old.checkpoint();
    assert!(Run::restore_checkpoint(&checkpoint).is_err());
    checkpoint["data"]["tasks"][0]["data"]["artifact"] = Value::Null;
    assert!(Run::restore_checkpoint(&checkpoint).is_err());
}

#[test]
fn transport_uncertainty_is_preserved_then_late_terminal_can_reconcile() {
    let mut run = run(TaskKind::Analysis);
    run.transport_lost(&tid(), "timeout").unwrap();
    let mut restored = Run::restore_checkpoint(&run.checkpoint()).unwrap();
    assert_eq!(restored.state(), RunState::Blocked);
    assert_eq!(
        restored
            .task(&tid())
            .unwrap()
            .current_attempt()
            .unwrap()
            .state(),
        AttemptState::Uncertain
    );
    assert_eq!(
        restored.make_ready(&tid()),
        Err(DomainError::InvalidTransition)
    );
    finish(&mut restored, "attempt-1");
    assert_eq!(
        restored.task(&tid()).unwrap().state(),
        TaskState::NeedsReview
    );
    assert_eq!(
        Run::restore_checkpoint(&restored.checkpoint())
            .unwrap()
            .state(),
        RunState::Blocked
    );
}

#[test]
fn malformed_domain_checkpoints_do_not_create_entities_or_lend_candidate_approval() {
    let mut run = integrated();
    let candidate = run.prepare_delivery(revision("target")).unwrap();
    run.approve_delivery(&candidate, &revision("target"), proof("approval"))
        .unwrap();
    for (pointer, replacement) in [
        ("/data/id", json!("bad id")),
        ("/data/project", Value::Null),
        ("/data/candidate/data/generation", json!(999)),
        (
            "/data/tasks/0/data/attempts/0/data/inputs/data/project",
            json!("other-project"),
        ),
        (
            "/data/tasks/0/data/artifact/data/revision",
            json!("not-approved"),
        ),
        ("/data/tasks/0/data/verifications/0/result", json!("Failed")),
    ] {
        let mut value = run.checkpoint();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(Run::restore_checkpoint(&value).is_err(), "{pointer}");
    }
    let mut value = run.checkpoint();
    value["data"]["candidate"]["data"]["future"] = json!(true);
    assert!(Run::restore_checkpoint(&value).is_err());
}

#[test]
fn workspace_restoration_keeps_standalone_history_projects_and_saved_composer() {
    let mut store = ConversationStore::new();
    store.add_project(project()).unwrap();
    let mut home = store.prepare(None).unwrap();
    home.set_draft("First question");
    let id = store
        .send(&mut home, ConversationId::new("conversation-1").unwrap())
        .unwrap();
    store.append_reply(&id, "A useful answer").unwrap();
    store
        .attach_project(&id, ProjectId::new("project-1").unwrap())
        .unwrap();
    let mut restored = ConversationStore::restore_checkpoint(&store.checkpoint()).unwrap();
    assert_eq!(restored.checkpoint(), store.checkpoint());
    assert_eq!(restored.projects().count(), 1);
    assert!(
        restored.conversation(&id).unwrap().messages()[0]
            .project_at_send()
            .is_none()
    );
    let mut saved = restored.composer(&id, "Continue here").unwrap();
    restored
        .send(&mut saved, ConversationId::new("unused").unwrap())
        .unwrap();
    assert_eq!(restored.conversations().count(), 1);
    assert_eq!(
        restored.conversation(&id).unwrap().messages()[2]
            .project_at_send()
            .unwrap()
            .as_str(),
        "project-1"
    );
    let mut value = restored.checkpoint();
    value["data"]["conversations"][0]["messages"][0]["order"] = json!(7);
    assert!(ConversationStore::restore_checkpoint(&value).is_err());
    let mut value = restored.checkpoint();
    value["data"]["conversations"][0]["messages"] = json!([]);
    assert!(ConversationStore::restore_checkpoint(&value).is_err());
}

#[test]
fn definitions_and_connection_checkpoints_revalidate_immutable_inputs() {
    let captured = inputs(None, "base-1");
    assert_eq!(
        AttemptInputs::restore_checkpoint(&captured.checkpoint()).unwrap(),
        captured
    );
    let mut invalid = captured.mode.checkpoint();
    invalid["data"]["version"] = json!(0);
    assert_eq!(
        ActionMode::restore_checkpoint(&invalid),
        Err(DomainError::InvalidVersion)
    );
    let data = fixture();
    let connection =
        Connection::new(normalize_descriptor(&data["descriptors"]["base"]).unwrap()).unwrap();
    assert_eq!(
        Connection::restore_checkpoint(&connection.checkpoint())
            .unwrap()
            .checkpoint(),
        connection.checkpoint()
    );
    let mut invalid = connection.checkpoint();
    invalid["data"]["descriptor"]["secret"] = json!("never accepted");
    assert!(Connection::restore_checkpoint(&invalid).is_err());
}

#[test]
fn restored_run_cannot_hide_current_blocking_facts() {
    let mut uncertain = run(TaskKind::Analysis);
    uncertain
        .accept_frame(&tid(), "{", EventOrigin::Runtime)
        .unwrap_err();
    assert_eq!(uncertain.state(), RunState::Blocked);
    let mut checkpoint = uncertain.checkpoint();
    checkpoint["data"]["state"] = json!("Working");
    assert!(Run::restore_checkpoint(&checkpoint).is_err());

    let mut failed = run(TaskKind::Code);
    finish(&mut failed, "attempt-1");
    let output = artifact("artifact-1", "output-1", "base-1");
    approve(&mut failed, &output, "failed");
    failed
        .integrate(IntegrationEvidence {
            run: failed.id().clone(),
            task: tid(),
            artifact: output,
            input_base: revision("base-1"),
            result_revision: revision("unused-result"),
            reference: proof("negative"),
            validation: CheckResult::Failed,
        })
        .unwrap_err();
    let mut checkpoint = failed.checkpoint();
    checkpoint["data"]["state"] = json!("Working");
    assert!(Run::restore_checkpoint(&checkpoint).is_err());
    // Explicit cancellation is a valid later transition despite retained failure history.
    failed.request_cancel().unwrap();
    failed.confirm_cancel().unwrap();
    assert_eq!(
        Run::restore_checkpoint(&failed.checkpoint())
            .unwrap()
            .state(),
        RunState::Cancelled
    );
    let mut exhausted = integrated().checkpoint();
    exhausted["data"]["generation"] = json!(u64::MAX);
    let mut exhausted = Run::restore_checkpoint(&exhausted).unwrap();
    assert!(matches!(
        exhausted.prepare_delivery(revision("target")),
        Err(DomainError::InvalidVersion)
    ));
}

#[test]
fn every_existing_conformance_observation_can_be_reopened_between_operations() {
    let corpus = fixture();
    for case in corpus["cases"].as_array().unwrap() {
        let descriptor = if let Some(name) = case["descriptor"].as_str() {
            &corpus["descriptors"][name]
        } else {
            &case["descriptor"]
        };
        let request = if let Some(name) = case["request"].as_str() {
            &corpus["requests"][name]
        } else {
            &case["request"]
        };
        let defaults = Limits::default();
        let limits = Limits {
            max_frame_bytes: case["limits"]["maxFrameBytes"]
                .as_u64()
                .unwrap_or(defaults.max_frame_bytes),
            max_events: case["limits"]["maxEvents"]
                .as_u64()
                .unwrap_or(defaults.max_events),
            max_total_bytes: case["limits"]["maxTotalBytes"]
                .as_u64()
                .unwrap_or(defaults.max_total_bytes),
        };
        let Ok(mut observer) = normalize_descriptor(descriptor).and_then(|d| {
            normalize_request(request).and_then(|r| RuntimeObserver::new(d, r, limits))
        }) else {
            continue;
        };
        for (index, step) in case["steps"].as_array().unwrap().iter().enumerate() {
            let action = &step["action"];
            match action["op"].as_str().unwrap() {
                "accept" => {
                    let origin = if action["origin"] == "supervisor" {
                        EventOrigin::Supervisor
                    } else {
                        EventOrigin::Runtime
                    };
                    let _ = observer.accept(action["frame"].as_str().unwrap(), origin);
                }
                "close" => {
                    observer.close();
                }
                "transport-lost" => {
                    let _ = observer.transport_lost(action["reason"].as_str().unwrap());
                }
                _ => {}
            }
            let before = observer.snapshot();
            observer = RuntimeObserver::restore_checkpoint(&observer.checkpoint())
                .unwrap_or_else(|error| panic!("{} step {index}: {error}", case["name"]));
            assert_eq!(observer.snapshot(), before, "{} step {index}", case["name"]);
            assert_eq!(
                observer.snapshot(),
                step["snapshot"],
                "{} step {index} canonical",
                case["name"]
            );
        }
    }
}
