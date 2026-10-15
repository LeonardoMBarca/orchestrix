//! Typed application records for the native Desktop. No execution or filesystem grants.
use crate::{
    domain::*,
    persistence::{Command, Request, Store, StoreError},
    persistence_records::{DefinitionInput, DefinitionKind, RecordsCommand},
    policy::PolicyDocument,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub language: String,
    pub theme: String,
    pub density: String,
    pub text_scale: u16,
    pub close_behavior: String,
    pub system_instructions: String,
    pub policy: PolicyDocument,
}
impl Default for Settings {
    fn default() -> Self {
        let mut policy = PolicyDocument::default();
        policy.mode_requirements.subscription_only = true;
        Self {
            language: "en".into(),
            theme: "studio".into(),
            density: "compact".into(),
            text_scale: 100,
            close_behavior: "continue".into(),
            system_instructions: String::new(),
            policy,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> bool {
        ["en", "pt-BR", "es"].contains(&self.language.as_str())
            && [
                "studio",
                "institutional",
                "atelier",
                "horizon",
                "deep-black",
                "medieval",
                "forest",
                "dawn",
            ]
            .contains(&self.theme.as_str())
            && ["compact", "comfortable"].contains(&self.density.as_str())
            && (80..=200).contains(&self.text_scale)
            && self.text_scale.is_multiple_of(5)
            && ["continue", "suspend"].contains(&self.close_behavior.as_str())
            && self.system_instructions.len() <= 8192
            && self.policy.validate().is_ok()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DesktopRequest {
    Bootstrap,
    Conversation {
        conversation_id: String,
        #[serde(default)]
        after_ordinal: u64,
        #[serde(default = "message_page_limit")]
        limit: usize,
    },
    CreateProject {
        command_id: String,
        expected_revision: u64,
        id: String,
        name: String,
        directory: String,
    },
    SendMessage {
        command_id: String,
        expected_revision: u64,
        conversation_id: String,
        project_id: Option<String>,
        content: String,
    },
    SaveDraft {
        command_id: String,
        expected_revision: u64,
        conversation_id: String,
        content: String,
    },
    RegisterConnection {
        command_id: String,
        expected_revision: u64,
        descriptor: Value,
    },
    AttachConversation {
        command_id: String,
        expected_revision: u64,
        conversation_id: String,
        project_id: String,
    },
    SaveSettings {
        command_id: String,
        expected_revision: u64,
        settings: Box<Settings>,
    },
    LinkWork {
        command_id: String,
        expected_revision: u64,
        conversation_id: String,
        worker_id: String,
    },
    InspectWorker {
        worker_id: String,
    },
    PrepareCorrection {
        review_id: String,
        new_id: String,
        authorization_ref: String,
        policy: Box<PolicyDocument>,
        classification: crate::policy::TaskClassification,
    },
    ArtifactText {
        artifact_id: String,
        revision: String,
        path: Option<String>,
        offset: usize,
        #[serde(default = "text_page_limit")]
        limit: usize,
    },
    AffectedWorkers {
        canonical_path: String,
    },
}
fn message_page_limit() -> usize {
    16
}
fn text_page_limit() -> usize {
    16_384
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DesktopCommand {
    CreateProject {
        id: String,
        name: String,
        directory: String,
    },
    SaveSettings {
        settings: Box<Settings>,
    },
}
impl DesktopCommand {
    pub(crate) fn apply(
        &self,
        state: &mut crate::persistence::SavedState,
    ) -> Result<Value, StoreError> {
        let (key, body) = match self {
            Self::CreateProject {
                id,
                name,
                directory,
            } => {
                let project = Project::new(
                    ProjectId::new(id)?,
                    Repository::new(
                        RepositoryId::new(format!("repo-{id}"))?,
                        LocalRef::new(format!("directory-{id}"))?,
                    ),
                );
                if name.trim().is_empty()
                    || name.len() > 120
                    || directory.len() > 4096
                    || !std::path::Path::new(directory).is_absolute()
                {
                    return Err(StoreError::InvalidPath);
                }
                Command::AddProject(project).apply(state)?;
                (
                    format!("desktop-project-{id}"),
                    json!({"name":name,"directory":directory}),
                )
            }
            Self::SaveSettings { settings } => {
                if !settings.validate() {
                    return Err(StoreError::InvalidState);
                }
                ("desktop-settings".into(), json!(settings))
            }
        };
        Command::Records(RecordsCommand::AddDefinition(DefinitionInput {
            kind: DefinitionKind::Configuration,
            id: key,
            version: state.revision() + 1,
            body,
            references: vec![],
        }))
        .apply(state)
    }
}

/// Atomic local commands retain the caller's original revision on replay. Reads
/// use bounded historical projections. Mutations and admission retain full validation.
pub(crate) fn handle(store: &mut Store, request: &DesktopRequest) -> Result<Value, String> {
    let (id, revision, command) = match request {
        DesktopRequest::AffectedWorkers { canonical_path } => {
            return store
                .desktop_affected_workers(canonical_path)
                .map_err(|_| "affected-workers-query".into());
        }
        DesktopRequest::Bootstrap => {
            return store
                .desktop_snapshot()
                .map_err(|_| "desktop-snapshot".into());
        }
        DesktopRequest::Conversation {
            conversation_id,
            after_ordinal,
            limit,
        } => {
            return store
                .desktop_conversation(conversation_id, *after_ordinal, *limit)
                .map_err(|_| "conversation-query".into());
        }
        DesktopRequest::InspectWorker { worker_id } => {
            return store
                .desktop_worker(worker_id)
                .map_err(|error| match error {
                    StoreError::Configuration => {
                        "worker-history-selection-ambiguous-or-budget".into()
                    }
                    _ => "worker-query".into(),
                });
        }
        DesktopRequest::PrepareCorrection {
            review_id,
            new_id,
            authorization_ref,
            policy,
            classification,
        } => {
            return serde_json::to_value(
                crate::review::ReviewPipeline::prepare_correction(
                    store,
                    review_id,
                    new_id,
                    authorization_ref,
                    *policy.clone(),
                    classification.clone(),
                )
                .map_err(|e| e.0)?,
            )
            .map_err(|_| "correction-template".into());
        }
        DesktopRequest::ArtifactText {
            artifact_id,
            revision,
            path,
            offset,
            limit,
        } => {
            return store
                .desktop_artifact_text(artifact_id, revision, path.as_deref(), *offset, *limit)
                .map_err(|_| "artifact-query".into());
        }
        DesktopRequest::CreateProject {
            command_id,
            expected_revision,
            id,
            name,
            directory,
        } => (
            command_id,
            *expected_revision,
            Command::Desktop(DesktopCommand::CreateProject {
                id: id.clone(),
                name: name.clone(),
                directory: directory.clone(),
            }),
        ),
        DesktopRequest::SendMessage {
            command_id,
            expected_revision,
            conversation_id,
            project_id,
            content,
        } => {
            if content.trim().is_empty() || content.len() > 32_768 {
                return Err("message-content".into());
            }
            (
                command_id,
                *expected_revision,
                Command::Send {
                    conversation: ConversationId::new(conversation_id)
                        .map_err(|_| "conversation-id")?,
                    project: project_id
                        .as_ref()
                        .map(ProjectId::new)
                        .transpose()
                        .map_err(|_| "project-id")?,
                    content: content.clone(),
                },
            )
        }
        DesktopRequest::SaveDraft {
            command_id,
            expected_revision,
            conversation_id,
            content,
        } => {
            if content.len() > 32_768 {
                return Err("draft-budget".into());
            }
            (
                command_id,
                *expected_revision,
                Command::SaveDraft {
                    conversation: ConversationId::new(conversation_id)
                        .map_err(|_| "conversation-id")?,
                    content: content.clone(),
                },
            )
        }
        DesktopRequest::RegisterConnection {
            command_id,
            expected_revision,
            descriptor,
        } => {
            let descriptor = crate::runtime_contract::normalize_descriptor(descriptor)
                .map_err(|_| "connection-descriptor")?;
            (
                command_id,
                *expected_revision,
                Command::AddConnection(
                    Connection::new(descriptor).map_err(|_| "connection-descriptor")?,
                ),
            )
        }
        DesktopRequest::AttachConversation {
            command_id,
            expected_revision,
            conversation_id,
            project_id,
        } => (
            command_id,
            *expected_revision,
            Command::Attach {
                conversation: ConversationId::new(conversation_id)
                    .map_err(|_| "conversation-id")?,
                project: ProjectId::new(project_id).map_err(|_| "project-id")?,
            },
        ),
        DesktopRequest::SaveSettings {
            command_id,
            expected_revision,
            settings,
        } => (
            command_id,
            *expected_revision,
            Command::Desktop(DesktopCommand::SaveSettings {
                settings: settings.clone(),
            }),
        ),
        DesktopRequest::LinkWork {
            command_id,
            expected_revision,
            conversation_id,
            worker_id,
        } => {
            let state = store.load().map_err(|_| "database")?;
            let worker = state.workers().get(worker_id).ok_or("worker-not-found")?;
            (
                command_id,
                *expected_revision,
                Command::LinkWork {
                    conversation: ConversationId::new(conversation_id)
                        .map_err(|_| "conversation-id")?,
                    run: RunId::new(&worker.intent.run_id).map_err(|_| "run-id")?,
                    task: Some(TaskId::new(&worker.intent.task_id).map_err(|_| "task-id")?),
                    attempt: Some(
                        AttemptId::new(&worker.intent.attempt_id).map_err(|_| "attempt-id")?,
                    ),
                },
            )
        }
    };
    let receipt = store
        .execute(&Request {
            scope: LocalRef::new("desktop").expect("constant"),
            key: LocalRef::new(id).map_err(|_| "command-id")?,
            expected_revision: revision,
            command,
        })
        .map_err(|e| match e {
            StoreError::StaleRevision => "stale-revision-resync",
            StoreError::IdempotencyConflict => "command-id-conflict",
            _ => "desktop-command",
        })?;
    if receipt.result.disposition != crate::persistence::Disposition::Applied {
        return Err(receipt
            .result
            .error_code
            .unwrap_or_else(|| "desktop-command-rejected".into()));
    }
    Ok(json!({"replayed":receipt.replayed,"result":receipt.result}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    fn store() -> Store {
        let parent = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "orchestrix-desktop-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&parent).unwrap();
        Store::open(parent.join("state.sqlite")).unwrap()
    }
    fn send(store: &mut Store, id: &str, conversation: &str, text: &str) -> DesktopRequest {
        DesktopRequest::SendMessage {
            command_id: id.into(),
            expected_revision: store.load().unwrap().revision(),
            conversation_id: conversation.into(),
            project_id: None,
            content: text.into(),
        }
    }
    #[test]
    fn home_and_empty_send_do_not_create_conversations_lost_reply_and_two_clients_are_safe() {
        let mut store = store();
        let empty = handle(&mut store, &DesktopRequest::Bootstrap).unwrap();
        assert_eq!(empty["conversations"], json!([]));
        let request = send(&mut store, "empty", "c1", " ");
        assert!(handle(&mut store, &request).is_err());
        assert_eq!(store.load().unwrap().revision(), 0);
        let request = send(&mut store, "first", "c1", "Investigate the parser");
        let first = handle(&mut store, &request).unwrap();
        let replay = handle(&mut store, &request).unwrap();
        assert_eq!(first["result"], replay["result"]);
        assert_eq!(replay["replayed"], true);
        let mut second = Store::open(store.path()).unwrap();
        let stale = send(&mut second, "stale", "c2", "another");
        let next = send(&mut store, "second", "c1", "a follow-up");
        handle(&mut store, &next).unwrap();
        assert_eq!(
            handle(&mut second, &stale).unwrap_err(),
            "stale-revision-resync"
        );
        let page = store.desktop_conversation("c1", 0, 16).unwrap();
        assert_eq!(page["messages"].as_array().unwrap().len(), 2);
        let changed = DesktopRequest::SendMessage {
            command_id: "first".into(),
            expected_revision: 0,
            conversation_id: "c1".into(),
            project_id: None,
            content: "changed request".into(),
        };
        assert_eq!(
            handle(&mut store, &changed).unwrap_err(),
            "command-id-conflict"
        );
        assert_eq!(
            store
                .load()
                .unwrap()
                .conversations()
                .conversations()
                .count(),
            1
        );
    }
    #[test]
    fn project_without_chat_draft_and_settings_survive_reopen() {
        let mut store = store();
        let directory = store.path().parent().unwrap().to_string_lossy().to_string();
        handle(
            &mut store,
            &DesktopRequest::CreateProject {
                command_id: "project".into(),
                expected_revision: 0,
                id: "p1".into(),
                name: "Parser".into(),
                directory,
            },
        )
        .unwrap();
        let snapshot = store.desktop_snapshot().unwrap();
        assert_eq!(snapshot["projects"][0]["name"], "Parser");
        assert_eq!(snapshot["conversations"], json!([]));
        let request = send(&mut store, "first", "c1", "First user message");
        handle(&mut store, &request).unwrap();
        let revision = store.load().unwrap().revision();
        handle(
            &mut store,
            &DesktopRequest::AttachConversation {
                command_id: "attach".into(),
                expected_revision: revision,
                conversation_id: "c1".into(),
                project_id: "p1".into(),
            },
        )
        .unwrap();
        let revision = store.load().unwrap().revision();
        handle(
            &mut store,
            &DesktopRequest::SaveDraft {
                command_id: "draft".into(),
                expected_revision: revision,
                conversation_id: "c1".into(),
                content: "Unsent draft".into(),
            },
        )
        .unwrap();
        let mut settings = Settings::default();
        settings.language = "pt-BR".into();
        settings.theme = "forest".into();
        settings.policy.revision = 2;
        let revision = store.load().unwrap().revision();
        let request = DesktopRequest::SaveSettings {
            command_id: "preferences".into(),
            expected_revision: revision,
            settings: Box::new(settings),
        };
        handle(&mut store, &request).unwrap();
        let path = store.path().to_owned();
        drop(store);
        let store = Store::open(path).unwrap();
        let page = store.desktop_conversation("c1", 0, 16).unwrap();
        assert_eq!(page["draft"], "Unsent draft");
        assert_eq!(page["projectId"], "p1");
        assert!(page["messages"][0]["projectAtSend"].is_null());
        assert_eq!(
            store.desktop_snapshot().unwrap()["settings"]["theme"],
            "forest"
        );
    }
    #[test]
    fn message_pages_stay_bounded_and_rejected_mutations_are_not_success() {
        let mut store = store();
        for n in 0..18 {
            let r = send(&mut store, &format!("msg-{n}"), "c1", &"x".repeat(32_768));
            handle(&mut store, &r).unwrap();
        }
        let page = store.desktop_conversation("c1", 0, 16).unwrap();
        assert_eq!(page["more"], true);
        assert_eq!(page["nextOrdinal"], 16);
        assert!(serde_json::to_vec(&page).unwrap().len() < 1_048_576);
        assert_eq!(
            store.desktop_conversation("c1", 16, 16).unwrap()["messages"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        let revision = store.load().unwrap().revision();
        assert!(
            handle(
                &mut store,
                &DesktopRequest::AttachConversation {
                    command_id: "invalid".into(),
                    expected_revision: revision,
                    conversation_id: "missing".into(),
                    project_id: "missing".into()
                }
            )
            .is_err()
        );
        assert!(
            Settings::default()
                .policy
                .mode_requirements
                .subscription_only
        );
    }
}
