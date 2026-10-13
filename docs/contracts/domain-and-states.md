# Domain and states — OX-004

Status: implemented in memory in the [existing Core crate](../../crates/orchestrix-core/src/domain/mod.rs), **13 October 2026**. This implements the [backlog OX-004](../DEVELOPMENT_BACKLOG.md#ox-004-domínio-e-estados), [plan domain/contracts](../DEVELOPMENT_PLAN.md#7-contratos-de-execução-e-integração) and [architecture task states](../ARCHITECTURE.md#7-task-state-machine). It does not replace the runtime contract, implement a scheduler or produce Git/check evidence.

## Module and identity boundaries

| Module | Responsibility |
| --- | --- |
| [ids](../../crates/orchestrix-core/src/domain/ids.rs) | Separate validated types for project, repository, connection, capacity group, conversation, runtime session, Run, Task, Attempt, definitions, context, artifact, revision and evidence. |
| [workspace](../../crates/orchestrix-core/src/domain/workspace.rs) | Projects/repositories, connections, session references/registry, capacity observations and UI conversations/work references. |
| [profiles](../../crates/orchestrix-core/src/domain/profiles.rs) | Immutable ActionMode and AgentProfile definitions with strictly increasing versions. |
| [attempt](../../crates/orchestrix-core/src/domain/attempt.rs) | Immutable configuration inputs, produced artifact history and the existing OX-003 observer. |
| [artifact](../../crates/orchestrix-core/src/domain/artifact.rs) | Exact artifact revision/base plus explicit verification, review and integration evidence. |
| [task](../../crates/orchestrix-core/src/domain/task.rs) | Guarded task transitions, attempt/evidence history and criteria for internal completion. |
| [run](../../crates/orchestrix-core/src/domain/run.rs) | Owning aggregate, dependency conditions, integration result and final delivery/cancellation. |

Project contains one Repository in V1; their types/IDs are different. Connection wraps a validated RuntimeDescriptor, retaining authentication/billing/capabilities as observations rather than copying credentials. RuntimeSessionRef is a **local** binding to the original connection/revision/adapter. SessionRegistry checks reuse across its scope; Run also enforces binding and one unresolved attempt per session even if a caller constructs a reference directly. The future host/persistence layer must maintain the registry across Runs. Distinct sessions/connections do not establish independent quotas. CapacityGroup represents explicit shared observations or Unknown; it is not a capacity reservation or scheduler.

## Conversations and snapshots

`prepare()` creates a PendingComposer, optionally in a known project. HOME, empty drafts and whitespace-only sends save no Conversation. The first successful nonempty `send()` consumes a new ID; later messages preserve it and their order. UI messages belong to Orchestrix, not the provider's internal transcript. Associating a saved conversation with a project preserves messages, their original project bindings and immutable work links.

`link_work()` captures the actual Run result revision, project and optional Task/Attempt/plan references. It checks that the task and attempt belong to the Run and that the current project binding matches; invalid links append nothing. A project may have zero conversations; a conversation and analysis Run may remain avulsa. This is an in-memory rule, not database persistence or reload/UI behavior.

Each TaskAttempt captures plan revision, Context Pack reference/version, full mode/profile definitions, project binding, producer base, worktree reference and optional correction source. Requested/effective policy and descriptor evidence remain in its owned runtime observer. Revising definitions creates new values and affects only future attempts. Routing Strategy remains the request's Efficiency/Performance/Balanced/Custom choice, separate from mode and profile. There is no routing/precedence resolver or specialized mode engine here.

## Execution and task transitions

The Task state vocabulary follows the existing architecture: `Created`, `Waiting`, `Ready`, `Assigned`, `Running`, `NeedsReview`, `NeedsWork`, `Approved`, `Completed`, `Failed`, `Blocked`, `Cancelled`. State fields are private; callers operate through Run commands, with readonly entity access. `wait_task` records waiting; explicit `make_ready` checks dependencies and can prepare a newly created task directly when they are already satisfied. There is no automatic scheduler.

Assignment validates project/attempt/session ownership and preserves earlier attempts. A new correction must name the exact prior artifact and start from its revision; the earlier attempt is sealed only after these checks succeed. An uncertain attempt cannot be retried or replaced merely by `make_ready`. A late owned terminal may reconcile ordinary signal loss, preserving the OX-003 quarantine rules. Cleanup after a confirmed terminal cannot undo explicit Ready/Cancelled decisions.

The attempt **execution state** derives only from its OX-003 observer. Its separate **output result** is Pending, AwaitingArtifact, ArtifactProduced, ExecutionFailed, Cancelled or Uncertain. Runtime completed without output stays AwaitingArtifact; produced output is not artifact acceptance or Task completion. A factual 1.1 terminal without start/ACK retains those absences and may move the Task to NeedsReview without manufacturing an intermediate runtime event. Policy admission is checked independently at attempt construction.

Verification/review bind the full `ArtifactVersion {id, revision, base}`. Changed content/base cannot borrow another version's approval. Failed checks or rejected review move to NeedsWork; historical artifacts/evidence remain accessible. Exact reuse of an earlier attempt's artifact is rejected. Cancel requests are intent: active/uncertain execution remains unresolved until an owned terminal exists. Process/helper exit, empty tree, transport loss and interrupt ACK never generate that terminal.

## Completion, dependencies and final application

| Operation/result | Required evidence |
| --- | --- |
| Code Task Approved | Published artifact, at least one passing verification, all recorded checks for that artifact passing and accepted review. |
| Code Task Completed | Approved artifact plus validated internal integration into this Run, binding source artifact, current input branch base and result revision. |
| Analysis Task Completed | Published artifact and accepted review; no Git integration required. A recorded failed verification/rejection still prevents completion. |
| Code dependency released | Source Task Completed with passed internal integration. Accepted but unintegrated code is insufficient. |
| Analysis dependency released | Source analysis Task Completed with accepted artifact. |
| Final-application Run Delivered | All Tasks internally complete, exact candidate approved, explicit successful application/validation recorded, then `mark_delivered`. |
| Accepted-artifacts Run Delivered | All Tasks are completed analyses; code cannot use this policy to bypass final application. |

Dependencies refer to existing tasks with the appropriate result kind. Insertion order and immutable edges prevent cycles in this initial aggregate; graph editing/planning remains later work. No speculative consumption of code patches is admitted.

Run states are Working, ReadyToApply, Applied, Blocked, Cancelled and Delivered. Independent work cannot clear a Blocked task or failed integration. A candidate captures Run/repository, result, target base, artifacts and generation. Changed target base or new result invalidates approval; an old candidate cannot approve/apply a replacement. Combined validation is explicit evidence for the exact internal integration/application, supplied by the future trusted services. `report_application_conflict()` records the exact candidate/evidence, preserves accepted artifacts and invalidates approval while blocking the Run. Applying and marking delivered remain separate operations. Confirming Run cancellation requires all owned attempts to have a known terminal; a request alone cannot deliver/cancel the result.

## Limits and validation

Evidence structs are **trusted host inputs**, not cryptographic attestations or proof that a command ran. OX-006/OX-009 and the future application layer must produce/validate actual artifacts, checks, reviews and Git results. The domain never runs processes, Git, provider APIs, tests, tools or timers. It contains no SQLite/Tauri/OS/Node/provider message types, credentials or provider transcript store. Persistence, concurrency reservations, supervision, routing and refresh retain OX-005 onward.

See [integrated lifecycle tests](../../crates/orchestrix-core/tests/domain_lifecycle.rs), [runtime-boundary tests](../../crates/orchestrix-core/tests/domain_runtime.rs), [conversation/definition tests](../../crates/orchestrix-core/tests/domain_workspace.rs) and the [increment evidence](../research/ox-004-domain-increment.md). Existing OX-003 conformance remains unchanged. M0 and production gates are not concluded by this pure domain increment.
