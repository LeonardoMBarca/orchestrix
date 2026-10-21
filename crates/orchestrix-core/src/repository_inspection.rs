//! Ephemeral, serial read jobs in the existing daemon. Independent of worker
//! admission and SQLite writers. No shell, provider, durable authorization or
//! automatic replay after reconnect/restart.
use crate::{
    domain::LocalRef,
    mechanical::{Query, QueryKind},
    runtime::CancellationToken,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::VecDeque, sync::mpsc, thread::JoinHandle};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum InspectionRequest {
    Start { id: String, query: Query },
    Inspect { id: String },
    Cancel { id: String },
}
struct Job {
    id: String,
    fingerprint: String,
    cancel: CancellationToken,
    thread: Option<JoinHandle<()>>,
    done: mpsc::Receiver<Result<Value, String>>,
    result: Option<Result<Value, String>>,
}
#[derive(Default)]
pub struct RepositoryInspectionService {
    jobs: VecDeque<Job>,
}
impl RepositoryInspectionService {
    pub fn drain(&mut self) {
        for job in &mut self.jobs {
            if job.thread.as_ref().is_some_and(|t| t.is_finished()) {
                let joined = job.thread.take().expect("checked read job").join().is_ok();
                job.result = Some(if joined {
                    job.done
                        .try_recv()
                        .unwrap_or_else(|_| Err("repository-read-lost".into()))
                } else {
                    Err("repository-read-lost".into())
                });
            }
        }
    }
    pub fn is_idle(&self) -> bool {
        self.jobs.iter().all(|j| j.thread.is_none())
    }
    pub fn cancel_all(&self) {
        for job in &self.jobs {
            job.cancel.cancel();
        }
    }
    pub fn handle(
        &mut self,
        request: &InspectionRequest,
        shutting_down: bool,
    ) -> Result<Value, String> {
        self.drain();
        let id = match request {
            InspectionRequest::Start { id, .. }
            | InspectionRequest::Inspect { id }
            | InspectionRequest::Cancel { id } => id,
        };
        LocalRef::new(id).map_err(|_| "repository-read-id")?;
        if let InspectionRequest::Start { query, .. } = request {
            if shutting_down {
                return Err("daemon-shutting-down".into());
            }
            if !query.authorized || LocalRef::new(&query.authorization_ref).is_err() {
                return Err("query-not-authorized".into());
            }
            if !matches!(
                query.query,
                QueryKind::RepositoryPreflight { .. } | QueryKind::GitStatus { .. }
            ) {
                return Err("repository-read-kind".into());
            }
            let fingerprint = crate::git_records::content_hash(
                &serde_json::to_vec(query).map_err(|_| "repository-read-schema")?,
            );
            if let Some(job) = self.jobs.iter().find(|j| j.id == *id) {
                if job.fingerprint != fingerprint {
                    return Err("repository-read-id-conflict".into());
                }
                return Ok(view(job));
            }
            if !self.is_idle() {
                return Err("repository-read-busy".into());
            }
            while self.jobs.len() >= 4 {
                self.jobs.pop_front();
            }
            let query = query.clone();
            let cancel = CancellationToken::new();
            let token = cancel.clone();
            let (tx, rx) = mpsc::sync_channel(1);
            let thread = std::thread::Builder::new()
                .name("orchestrix-repository-read".into())
                .spawn(move || {
                    let result = if token.is_cancelled() {
                        Err("repository-read-cancelled".into())
                    } else {
                        match &query.query {
                            QueryKind::RepositoryPreflight {
                                repository,
                                base_commit,
                            } => crate::git::inspect_repository(
                                repository,
                                base_commit.as_deref(),
                                &token,
                            )
                            .map_err(|e| e.to_string())
                            .and_then(|r| {
                                serde_json::to_value(r).map_err(|_| "repository-read-schema".into())
                            }),
                            _ => crate::mechanical::execute(None, &query).map_err(|e| e.0.into()),
                        }
                    };
                    let _ = tx.send(if token.is_cancelled() {
                        Err("repository-read-cancelled".into())
                    } else {
                        result
                    });
                })
                .map_err(|_| "repository-read-thread")?;
            self.jobs.push_back(Job {
                id: id.clone(),
                fingerprint,
                cancel,
                thread: Some(thread),
                done: rx,
                result: None,
            });
        }
        let job = self
            .jobs
            .iter()
            .find(|j| j.id == *id)
            .ok_or("repository-read-not-found-reinspect-explicitly")?;
        if matches!(request, InspectionRequest::Cancel { .. }) {
            job.cancel.cancel();
        }
        Ok(view(job))
    }
}
fn view(job: &Job) -> Value {
    match &job.result {
        None => json!({"id":job.id,"state":"pending","modelCalled":false}),
        Some(Ok(result)) => {
            json!({"id":job.id,"state":"finished","result":result,"modelCalled":false})
        }
        Some(Err(error)) => json!({"id":job.id,"state":"failed","error":error,"modelCalled":false}),
    }
}
impl Drop for RepositoryInspectionService {
    fn drop(&mut self) {
        self.cancel_all();
        for job in &mut self.jobs {
            if let Some(thread) = job.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_busy_and_query_allowlist_remain_local_and_bounded() {
        let mut service = RepositoryInspectionService::default();
        let (tx, rx) = mpsc::sync_channel(1);
        let (release, wait) = mpsc::channel();
        let cancel = CancellationToken::new();
        let token = cancel.clone();
        let thread = std::thread::spawn(move || {
            wait.recv().unwrap();
            let _ = tx.send(if token.is_cancelled() {
                Err("repository-read-cancelled".into())
            } else {
                Ok(json!({"modelCalled":false}))
            });
        });
        service.jobs.push_back(Job {
            id: "pending".into(),
            fingerprint: "test-scope".into(),
            cancel,
            thread: Some(thread),
            done: rx,
            result: None,
        });
        let view = service
            .handle(
                &InspectionRequest::Inspect {
                    id: "pending".into(),
                },
                false,
            )
            .unwrap();
        assert_eq!(view["state"], "pending");
        let query = Query {
            authorization_ref: "read-consent".into(),
            authorized: true,
            query: QueryKind::GitStatus {
                repository: std::env::temp_dir(),
            },
        };
        assert_eq!(
            service
                .handle(
                    &InspectionRequest::Start {
                        id: "other".into(),
                        query: query.clone()
                    },
                    false
                )
                .unwrap_err(),
            "repository-read-busy"
        );
        assert_eq!(
            service
                .handle(
                    &InspectionRequest::Start {
                        id: "other".into(),
                        query: Query {
                            query: QueryKind::GitDiff {
                                repository: std::env::temp_dir()
                            },
                            ..query
                        }
                    },
                    false
                )
                .unwrap_err(),
            "repository-read-kind"
        );
        service
            .handle(
                &InspectionRequest::Cancel {
                    id: "pending".into(),
                },
                false,
            )
            .unwrap();
        assert!(service.jobs[0].cancel.is_cancelled());
        release.send(()).unwrap();
        let thread = service.jobs[0].thread.as_ref().unwrap();
        while !thread.is_finished() {
            std::thread::yield_now();
        }
        service.drain();
        let result = service
            .handle(
                &InspectionRequest::Inspect {
                    id: "pending".into(),
                },
                false,
            )
            .unwrap();
        assert_eq!(result["state"], "failed");
        assert_eq!(result["error"], "repository-read-cancelled");
        assert!(service.is_idle());
    }
}
