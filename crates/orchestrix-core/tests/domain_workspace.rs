use orchestrix_core::domain::*;
use orchestrix_core::runtime_contract::{normalize_descriptor, normalize_request};
use serde_json::json;

fn connection(id: &str, revision: u64) -> Connection {
    Connection::new(
        normalize_descriptor(&json!({
            "version": {"major": 1, "minor": 1},
            "adapterId": "shared-adapter",
            "runtimeVersion": "fixture-1",
            "connection": {
                "id": id, "revision": revision, "authentication": "authenticated",
                "billing": "subscription", "availability": "available"
            },
            "models": []
        }))
        .unwrap(),
    )
    .unwrap()
}

fn project(name: &str) -> Project {
    Project::new(
        ProjectId::new(name).unwrap(),
        Repository::new(
            RepositoryId::new(format!("{name}-repository")).unwrap(),
            LocalRef::new("authorized-directory").unwrap(),
        ),
    )
}

#[test]
fn project_exists_without_saved_conversations_and_preserves_repository_identity() {
    let mut store = ConversationStore::new();
    let empty_project = project("empty-project");
    let id = empty_project.id().clone();
    let repository_id = empty_project.repository().id().clone();
    store.add_project(empty_project).unwrap();
    assert_eq!(
        store.project(&id).unwrap().repository().id(),
        &repository_id
    );
    assert_eq!(repository_id.as_str(), "empty-project-repository");
    assert!(store.project_conversations(&id).unwrap().is_empty());
    assert_eq!(store.conversations().count(), 0);
    assert_eq!(
        store.add_project(project("empty-project")),
        Err(DomainError::DuplicateIdentity)
    );
}

#[test]
fn home_draft_and_empty_send_do_not_create_saved_conversation() {
    let mut store = ConversationStore::new();
    let mut home = store.prepare(None).unwrap();
    assert!(home.conversation_id().is_none());
    home.set_draft("   \n\t ");
    assert_eq!(
        store.send(&mut home, ConversationId::new("unused").unwrap()),
        Err(DomainError::EmptyContent)
    );
    assert_eq!(home.draft(), "   \n\t ");
    assert!(home.conversation_id().is_none());
    assert_eq!(store.conversations().count(), 0);
    home.set_draft("Unsent idea");
    assert_eq!(store.conversations().count(), 0);
    drop(home);
    assert_eq!(store.conversations().count(), 0);
}

#[test]
fn project_selection_is_pending_until_send_and_captured_in_first_message() {
    let mut store = ConversationStore::new();
    let project_id = ProjectId::new("selected-project").unwrap();
    store.add_project(project("selected-project")).unwrap();
    let mut composer = store.prepare(Some(project_id.clone())).unwrap();
    composer.set_draft("Create a feature in this project");
    assert_eq!(composer.project_id(), Some(&project_id));
    assert!(store.project_conversations(&project_id).unwrap().is_empty());
    let id = store
        .send(
            &mut composer,
            ConversationId::new("project-conversation").unwrap(),
        )
        .unwrap();
    let conversation = store.conversation(&id).unwrap();
    assert_eq!(conversation.project_id(), Some(&project_id));
    assert_eq!(
        conversation.messages()[0].project_at_send(),
        Some(&project_id)
    );
    assert_eq!(store.project_conversations(&project_id).unwrap().len(), 1);
}

#[test]
fn first_send_creates_once_and_later_messages_keep_order_and_identity() {
    let mut store = ConversationStore::new();
    let mut composer = store.prepare(None).unwrap();
    composer.set_draft("Explain the architecture");
    let id = store
        .send(
            &mut composer,
            ConversationId::new("conversation-one").unwrap(),
        )
        .unwrap();
    assert_eq!(composer.conversation_id(), Some(&id));
    assert_eq!(composer.draft(), "");
    store.append_reply(&id, "Here are the boundaries").unwrap();
    composer.set_draft("Now explain the domain");
    assert_eq!(
        store
            .send(
                &mut composer,
                ConversationId::new("must-not-be-created").unwrap()
            )
            .unwrap(),
        id
    );
    let conversation = store.conversation(&id).unwrap();
    assert_eq!(conversation.id(), &id);
    assert!(conversation.project_id().is_none());
    assert_eq!(
        conversation
            .messages()
            .iter()
            .map(ConversationMessage::order)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(conversation.messages()[0].role(), MessageRole::User);
    assert_eq!(conversation.messages()[1].role(), MessageRole::Orchestrator);
    assert_eq!(
        conversation.messages()[2].content(),
        "Now explain the domain"
    );
    assert_eq!(store.conversations().count(), 1);
}

#[test]
fn attaching_standalone_conversation_preserves_history_and_captured_bindings() {
    let mut store = ConversationStore::new();
    store.add_project(project("target-project")).unwrap();
    let project_id = ProjectId::new("target-project").unwrap();
    let mut composer = store.prepare(None).unwrap();
    composer.set_draft("A standalone first message");
    let id = store
        .send(&mut composer, ConversationId::new("standalone").unwrap())
        .unwrap();
    store.append_reply(&id, "Standalone reply").unwrap();
    let captured_history = store.conversation(&id).unwrap().messages().to_vec();
    store.attach_project(&id, project_id.clone()).unwrap();
    assert_eq!(
        store.conversation(&id).unwrap().messages(),
        captured_history
    );
    assert!(captured_history[0].project_at_send().is_none());
    assert_eq!(
        store.conversation(&id).unwrap().project_id(),
        Some(&project_id)
    );
    composer.set_draft("Continue with shared project context");
    store
        .send(&mut composer, ConversationId::new("ignored").unwrap())
        .unwrap();
    assert_eq!(
        store.conversation(&id).unwrap().messages()[2].project_at_send(),
        Some(&project_id)
    );
    assert_eq!(
        store.project_conversations(&project_id).unwrap()[0].id(),
        &id
    );
}

#[test]
fn missing_project_and_duplicate_conversation_reject_without_losing_draft() {
    let mut store = ConversationStore::new();
    assert_eq!(
        store.prepare(Some(ProjectId::new("missing").unwrap())),
        Err(DomainError::UnknownEntity)
    );
    let mut first = store.prepare(None).unwrap();
    first.set_draft("One");
    let id = store
        .send(&mut first, ConversationId::new("reserved").unwrap())
        .unwrap();
    let before = store.conversation(&id).unwrap().clone();
    let mut second = store.prepare(None).unwrap();
    second.set_draft("Two");
    assert_eq!(
        store.send(&mut second, id.clone()),
        Err(DomainError::DuplicateIdentity)
    );
    assert_eq!(second.draft(), "Two");
    assert!(second.conversation_id().is_none());
    assert_eq!(store.conversation(&id), Some(&before));
    assert_eq!(
        store.attach_project(&id, ProjectId::new("missing").unwrap()),
        Err(DomainError::UnknownEntity)
    );
    assert_eq!(store.conversation(&id), Some(&before));
    assert_eq!(
        store.append_reply(&id, "  "),
        Err(DomainError::EmptyContent)
    );
    assert_eq!(store.conversation(&id), Some(&before));
}

#[test]
fn same_adapter_connections_keep_distinct_local_sessions_and_unknown_quotas() {
    let first = connection("first-account", 1);
    let second = connection("second-account", 1);
    assert_eq!(first.adapter_id(), second.adapter_id());
    assert_ne!(first.id(), second.id());
    assert_eq!(
        first.descriptor().as_json()["capabilities"]["quota"]["support"],
        "unknown"
    );
    let first_session =
        RuntimeSessionRef::new(RuntimeSessionId::new("session-first").unwrap(), &first);
    let second_session =
        RuntimeSessionRef::new(RuntimeSessionId::new("session-second").unwrap(), &second);
    let mut registry = SessionRegistry::new();
    registry.register(first_session.clone()).unwrap();
    registry.register(second_session.clone()).unwrap();
    assert_eq!(
        registry.get(first_session.id()).unwrap().connection_id(),
        first.id()
    );
    assert_eq!(
        registry.get(second_session.id()).unwrap().connection_id(),
        second.id()
    );
    assert_eq!(
        registry.register(RuntimeSessionRef::new(first_session.id().clone(), &second)),
        Err(DomainError::Ownership)
    );
    assert_eq!(registry.get(first_session.id()), Some(&first_session));
    assert_eq!(
        registry.register(first_session),
        Err(DomainError::DuplicateIdentity)
    );
}

#[test]
fn session_binding_is_immutable_across_connection_revision_change() {
    let original = connection("same-account", 1);
    let revised = connection("same-account", 2);
    let session = RuntimeSessionRef::new(
        RuntimeSessionId::new("session-original").unwrap(),
        &original,
    );
    let mut registry = SessionRegistry::new();
    registry.register(session.clone()).unwrap();
    assert_eq!(session.connection_revision(), 1);
    assert_eq!(
        registry.register(RuntimeSessionRef::new(session.id().clone(), &revised)),
        Err(DomainError::Ownership)
    );
    assert_eq!(registry.get(session.id()).unwrap().connection_revision(), 1);
}

#[test]
fn capacity_membership_does_not_assume_independent_limits() {
    let first = connection("first-account", 1);
    let second = connection("second-account", 1);
    let mut group = CapacityGroup::new(
        CapacityGroupId::new("shared-plan").unwrap(),
        SharedCapacity::Unknown,
    );
    group.add_member(first.id().clone()).unwrap();
    group.add_member(second.id().clone()).unwrap();
    assert_eq!(group.capacity(), &SharedCapacity::Unknown);
    assert_eq!(group.members().len(), 2);
    assert_eq!(
        group.add_member(first.id().clone()),
        Err(DomainError::DuplicateIdentity)
    );
    assert_eq!(group.members().len(), 2);
    let observed =
        CapacityObservation::new(100, 25, EvidenceId::new("provider-shared-limit").unwrap())
            .unwrap();
    group.record_capacity(SharedCapacity::Observed(observed.clone()));
    assert_eq!(group.capacity(), &SharedCapacity::Observed(observed));
    assert_eq!(
        group.remove_member(&ConnectionId::new("unknown-account").unwrap()),
        Err(DomainError::UnknownEntity)
    );
    group.remove_member(second.id()).unwrap();
    assert!(!group.contains(second.id()));
    assert_eq!(group.members().len(), 1);
    assert_eq!(
        CapacityObservation::new(1, 2, EvidenceId::new("invalid").unwrap()),
        Err(DomainError::InvalidReference)
    );
}

#[test]
fn immutable_mode_and_profile_revisions_preserve_captured_definitions() {
    let version_one = DefinitionVersion::new(1).unwrap();
    let version_two = DefinitionVersion::new(2).unwrap();
    let mode = ActionMode::new(
        ModeId::new("development").unwrap(),
        version_one,
        "Development",
        "Implement verified code",
    )
    .unwrap();
    let profile = AgentProfile::new(
        ProfileId::new("implementer").unwrap(),
        version_one,
        "Implementer",
        "coder",
        LocalRef::new("instructions-v1").unwrap(),
    )
    .unwrap();
    let captured = (mode.clone(), profile.clone());
    let new_mode = mode
        .revise(
            version_two,
            "Development",
            "Implement and document verified code",
        )
        .unwrap();
    let new_profile = profile
        .revise(
            version_two,
            "Implementer",
            "coder",
            LocalRef::new("instructions-v2").unwrap(),
        )
        .unwrap();
    assert_eq!(new_mode.id(), mode.id());
    assert_eq!(new_profile.id(), profile.id());
    assert_eq!(new_mode.version(), version_two);
    assert_eq!(new_profile.version(), version_two);
    assert_eq!(captured.0, mode);
    assert_eq!(captured.1, profile);
    assert_eq!(captured.0.purpose(), "Implement verified code");
    assert_eq!(captured.1.instructions().as_str(), "instructions-v1");
    assert_eq!(
        mode.revise(version_one, "Development", "New purpose"),
        Err(DomainError::InvalidVersion)
    );
    assert_eq!(
        profile.revise(
            version_one,
            "Implementer",
            "coder",
            LocalRef::new("instructions-v2").unwrap()
        ),
        Err(DomainError::InvalidVersion)
    );
}

#[test]
fn empty_mode_or_profile_definitions_and_invalid_versions_are_rejected() {
    let version = DefinitionVersion::new(1).unwrap();
    assert_eq!(DefinitionVersion::new(0), Err(DomainError::InvalidVersion));
    assert_eq!(
        ActionMode::new(ModeId::new("mode").unwrap(), version, "", "Purpose"),
        Err(DomainError::EmptyContent)
    );
    assert_eq!(
        AgentProfile::new(
            ProfileId::new("profile").unwrap(),
            version,
            "Profile",
            "  ",
            LocalRef::new("instructions").unwrap()
        ),
        Err(DomainError::EmptyContent)
    );
}

fn standalone_analysis_run() -> Run {
    Run::new(
        RunId::new("analysis-run").unwrap(),
        None,
        RevisionId::new("run-base").unwrap(),
        DeliveryRequirement::AcceptedArtifacts,
    )
    .unwrap()
}

fn analysis_task(id: &str) -> Task {
    Task::new(
        TaskId::new(id).unwrap(),
        "Explain the architecture",
        TaskKind::Analysis,
        vec!["References are identified".into()],
        vec![],
    )
    .unwrap()
}

fn analysis_attempt(name: &str, plan: &str) -> TaskAttempt {
    let connection = connection("analysis-account", 1);
    let session = RuntimeSessionRef::new(
        RuntimeSessionId::new("analysis-session").unwrap(),
        &connection,
    );
    let request = normalize_request(&json!({
        "version": {"major": 1, "minor": 1}, "connectionId": connection.id().as_str(),
        "connectionRevision": 1, "sessionId": session.id().as_str(), "attemptId": name,
        "contextPackRef": "analysis-context", "requested": {"modelPolicy": "runtime-default"}
    }))
    .unwrap();
    let version = DefinitionVersion::new(1).unwrap();
    TaskAttempt::new(
        &connection,
        &session,
        request,
        AttemptInputs {
            plan_revision: RevisionId::new(plan).unwrap(),
            context: ContextVersion {
                reference: ContextPackId::new("analysis-context").unwrap(),
                version,
            },
            mode: ActionMode::new(
                ModeId::new("research").unwrap(),
                version,
                "Research",
                "Produce an accepted analysis",
            )
            .unwrap(),
            profile: AgentProfile::new(
                ProfileId::new("analyst").unwrap(),
                version,
                "Analyst",
                "researcher",
                LocalRef::new("analysis-instructions").unwrap(),
            )
            .unwrap(),
            base: RevisionId::new("run-base").unwrap(),
            worktree: LocalRef::new("read-only-context").unwrap(),
            project: None,
            correction_of: None,
        },
    )
    .unwrap()
}

#[test]
fn standalone_work_link_is_versioned_and_survives_later_project_association() {
    let mut store = ConversationStore::new();
    store.add_project(project("later-project")).unwrap();
    let mut composer = store.prepare(None).unwrap();
    composer.set_draft("Investigate this architecture");
    let id = store
        .send(&mut composer, ConversationId::new("analysis-chat").unwrap())
        .unwrap();
    let mut run = standalone_analysis_run();
    let task_id = TaskId::new("analysis-task").unwrap();
    let attempt_id = AttemptId::new("analysis-attempt").unwrap();
    run.add_task(analysis_task(task_id.as_str())).unwrap();
    run.make_ready(&task_id).unwrap();
    run.assign_attempt(
        &task_id,
        analysis_attempt(attempt_id.as_str(), "analysis-plan-v3"),
    )
    .unwrap();
    store
        .link_work(&id, &run, Some(&task_id), Some(&attempt_id))
        .unwrap();
    let captured = store.conversation(&id).unwrap().work_references().to_vec();
    assert_eq!(captured[0].run_id(), run.id());
    assert_eq!(captured[0].run_result_revision(), run.result_revision());
    assert_eq!(captured[0].task_id(), Some(&task_id));
    assert_eq!(captured[0].attempt_id(), Some(&attempt_id));
    assert_eq!(
        captured[0].plan_revision().unwrap().as_str(),
        "analysis-plan-v3"
    );
    assert!(captured[0].project_at_link().is_none());
    assert_eq!(
        store.link_work(&id, &run, Some(&task_id), Some(&attempt_id)),
        Err(DomainError::DuplicateIdentity)
    );
    store
        .attach_project(&id, ProjectId::new("later-project").unwrap())
        .unwrap();
    assert_eq!(store.conversation(&id).unwrap().work_references(), captured);
    assert_eq!(
        store.link_work(&id, &run, None, None),
        Err(DomainError::Ownership)
    );
    assert_eq!(store.conversation(&id).unwrap().work_references(), captured);
}

#[test]
fn links_reject_missing_task_wrong_attempt_and_attempt_without_task_atomically() {
    let mut store = ConversationStore::new();
    let mut composer = store.prepare(None).unwrap();
    composer.set_draft("Compare these analyses");
    let id = store
        .send(
            &mut composer,
            ConversationId::new("ownership-chat").unwrap(),
        )
        .unwrap();
    let mut run = standalone_analysis_run();
    let first = TaskId::new("first-analysis").unwrap();
    let second = TaskId::new("second-analysis").unwrap();
    let attempt = AttemptId::new("owned-by-first").unwrap();
    run.add_task(analysis_task(first.as_str())).unwrap();
    run.add_task(analysis_task(second.as_str())).unwrap();
    run.make_ready(&first).unwrap();
    run.assign_attempt(&first, analysis_attempt(attempt.as_str(), "plan-one"))
        .unwrap();
    let before = store.conversation(&id).unwrap().clone();
    assert_eq!(
        store.link_work(&id, &run, Some(&TaskId::new("missing-task").unwrap()), None),
        Err(DomainError::UnknownEntity)
    );
    assert_eq!(
        store.link_work(&id, &run, Some(&second), Some(&attempt)),
        Err(DomainError::Ownership)
    );
    assert_eq!(
        store.link_work(&id, &run, None, Some(&attempt)),
        Err(DomainError::Ownership)
    );
    assert_eq!(
        store.link_work(
            &id,
            &run,
            Some(&first),
            Some(&AttemptId::new("missing-attempt").unwrap())
        ),
        Err(DomainError::Ownership)
    );
    assert_eq!(store.conversation(&id), Some(&before));
    store.link_work(&id, &run, Some(&second), None).unwrap();
    let reference = &store.conversation(&id).unwrap().work_references()[0];
    assert_eq!(reference.task_id(), Some(&second));
    assert!(reference.attempt_id().is_none());
    assert!(reference.plan_revision().is_none());
}

#[test]
fn work_link_requires_current_conversation_and_run_project_to_match() {
    let mut store = ConversationStore::new();
    let first = project("first-project");
    let second = project("second-project");
    store.add_project(first.clone()).unwrap();
    store.add_project(second.clone()).unwrap();
    let mut composer = store.prepare(Some(first.id().clone())).unwrap();
    composer.set_draft("Work on the first project");
    let id = store
        .send(&mut composer, ConversationId::new("project-chat").unwrap())
        .unwrap();
    let wrong = Run::new(
        RunId::new("wrong-project-run").unwrap(),
        Some(&second),
        RevisionId::new("base").unwrap(),
        DeliveryRequirement::AcceptedArtifacts,
    )
    .unwrap();
    let before = store.conversation(&id).unwrap().clone();
    assert_eq!(
        store.link_work(&id, &wrong, None, None),
        Err(DomainError::Ownership)
    );
    assert_eq!(store.conversation(&id), Some(&before));
    let correct = Run::new(
        RunId::new("first-project-run").unwrap(),
        Some(&first),
        RevisionId::new("base").unwrap(),
        DeliveryRequirement::AcceptedArtifacts,
    )
    .unwrap();
    store.link_work(&id, &correct, None, None).unwrap();
    let reference = &store.conversation(&id).unwrap().work_references()[0];
    assert_eq!(reference.project_at_link(), Some(first.id()));
    assert_eq!(reference.run_id(), correct.id());
    assert!(reference.task_id().is_none());
    assert!(reference.plan_revision().is_none());
}
