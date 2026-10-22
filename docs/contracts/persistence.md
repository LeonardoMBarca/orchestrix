# Persistence boundary — OX-005

Implemented boundary: [Core persistence](../../crates/orchestrix-core/src/persistence/mod.rs), **14 October 2026**. This complements [Domain and States](./domain-and-states.md), [Runtime Contract](./runtime-contract-v1.md) and [Context Ownership](../CONTEXT_OWNERSHIP.md). The current authoritative completion state remains [Development Status](../DEVELOPMENT_STATUS.md); this contract alone is not acceptance evidence.

## Ownership and storage model

The Core's `persistence` module owns SQLite connections, SQL, migrations and durable command processing. Domain entities retain private state and guarded operations; they do not depend on SQLite, database rows, Tauri or provider APIs. [ADR-0006](../adr/0006-synchronous-sqlite-persistence.md) records the access-library decision and its validation gate.

The database holds current state and an ordered append-only audit trail. State is authoritative for restoration; audit events explain committed operations and are not replayed as a complete event-sourced model. Persisted command results support replay independently of whether a caller received the first response.

One explicit local database path defines the store scope. Open failures, malformed data and incompatible schemas return typed errors; opening never silently deletes or recreates an existing file. No default personal database location, automatic process reconciliation or UI selection is implied.

## Connection and migration policy

Local mode/profile editing reuses immutable `Configuration` records (`mode-definition-*`, `profile-definition-*`, `scoped-policy-user`, identified Project scope keys) and the existing minimal Mode/Profile references. This increment adds no SQL tables or migration. Schema version 1 of the rich definitions carries stable identity, immutable version, edit revision, origin and pinned native base. Native rich definitions start at version 2; legacy version-1 worker records retain their exact minimal historical bytes. Catalog reads do not create records. Save/restore/archive and User-scope policy import are atomic commands with CAS, receipts and events; they never rewrite earlier definition versions or admitted attempts.

Each connection enables and verifies foreign keys before starting a transaction, sets a finite **250 ms busy timeout**, verifies WAL and uses `synchronous=FULL`. Writes start with `BEGIN IMMEDIATE`; contention returns an error after bounded waiting, without an internal indefinite retry loop. The connection is owned by its synchronous caller. Future async hosting must offload the whole short operation and must not retain a transaction across `.await` or external execution.

Versioned SQL migrations are retained separately in [schema.rs](../../crates/orchestrix-core/src/persistence/schema.rs). `PRAGMA user_version` identifies the schema revision, separately from domain checkpoint, event and runtime wire/input versions. `application_id=0x4f585354` identifies an Orchestrix store. An existing database with another application identity, or an unversioned database already containing tables, is refused. Migration statements and the corresponding schema-version change share a transaction. Open rejects a schema version newer than the implementation understands. A migration test starts from an actual older test database containing data, rather than only checking creation of an empty latest schema.

| Migration | Stored records and constraints |
| --- | --- |
| 1 | Store revision; repositories/projects; connection revisions and local session bindings; Runs/Tasks/dependencies/attempts/artifacts/evidence; conversations/messages/work links; idempotency commands and sequenced events. Foreign keys preserve ownership, a partial unique index permits at most one unresolved attempt per session, and triggers reject changes/deletions of committed events, command results and evidence. |
| 2 | Shared-capacity groups; saved-conversation drafts; document/diagnostic records checkpoint; immutable attempt context/configuration/diagnostic snapshot bindings; relational projections of versioned definitions, their references and diagnostic observations. Definition/diagnostic update/delete triggers preserve their histories. |
| 3 | Immutable worktree intents linked to registered attempts, ordered operation observations/state, and immutable exact artifact manifests/content/patch hashes. Earlier migration definitions remain unchanged. External Git effects use separate short intent and confirmation transactions. |
| 4 | Immutable manual-worker intents, exact policy/context/check bindings and append-only worker observations. Atomic reservation links definitions/diagnostics/project/connection/session/Run/Task/attempt; process/Git/check effects stay outside SQLite transactions. See [manual worker](./manual-worker.md). |
| 5 | Stable daemon identity/generation/admission state, immutable command journal/completions and append-only API events. Domain event references enter this feed in the same transaction; snapshot/cursor and command replay are defined by the [local daemon contract](./local-daemon.md). |
| 6 | Independent-review intents and append-only observations, retaining exact source artifact, independent session, structured findings and bounded correction provenance. |
| 7 | Serial-integration/application intents and append-only observations, preserving candidate/base/check/approval/effect confirmation separately. |
| 8 | Runtime-managed connection lifecycle revisions, diagnostic intents/observations and idempotent local commands. Nonsecret runtime bindings stay in the same Core store; no provider credential is imported. Local edit revision is distinct from the binding revision used by work. Historical records are not assigned authentication/identity during migration. |

Run/conversation checkpoints retain the full validated aggregate; relational rows provide identity, ownership and lookup constraints. Loading must validate the checkpoint and its relationships rather than trusting either JSON or SQL projections in isolation. Audit/event schema version is currently **1**, while database schema version is **8**; these numbers have different compatibility meanings.

Data values use SQL parameters. Fixed migration/DDL text may use SQL batches; user content never becomes an SQL identifier or concatenated query fragment. Foreign keys, unique identity constraints and necessary lookup indexes complement validated domain loading. Database constraints remain necessary even though the application checks identities first.

## Diagnostic retention — schema 10

Migration 10 adds an indexed operational/archive projection and storage/reservation accounting over the unchanged migration-8 diagnostic rows and migration-9 validity envelopes. Original IDs, ordinals, order, bodies and digests remain in place with their immutability triggers. The projection permits only monotonic archiving of completed rounds; evidence is never replaced by its projection. Startup validates original hashes, transitions, projection equivalence and accounting with a streaming pass rather than materializing the complete history.

Normal connection reports use bounded/indexed first-identity, latest-round, last-success, pending-identity and exact-reference queries over both recent and archived evidence. Cached observations use narrow cohort validity aggregation, including archived envelopes and all subsequent stricter settings, without deserializing every historical catalog. Read-time history never renews observations or grants admission. Existing preparation, receipt and attempt references remain exact diagnostic IDs, not aliases for the latest result.

Admission, bounded archive maintenance, terminal reservation and the original command receipt commit together in a short transaction before a runtime read. Terminal completion releases only its own reservation in the same transaction as evidence insertion. Rollback/crash cannot publish half a rotation or a terminal row without its accounting. Recovery records interruption without replaying metadata. No writer is held while waiting for a runtime, UI or process.

The operational bound is 512 transitions, maintenance starts at 384 and touches at most 64 completed rounds per admission. Legacy terminal recovery may perform that same bounded batch to make room for previously unreserved pending starts. Accounted history budgets are 256 MiB per connection/1 GiB globally, with a maximum terminal body reserved before admission. Exhaustion blocks new reads explicitly and preserves historical access and local cancellation/recovery. These logical accounting bounds are distinct from SQLite/WAL file allocation and other retained product data. Page/detail byte budgets and the cursor contract are specified in [Connection Diagnostics](../design/connection-diagnostics.md#queryable-diagnostic-archive--schema-10).

## Capacity observation extension — schema 11

Schema 11 adds `connection_capacity_denied`, a partial index over the original immutable diagnostic bodies. No historical body, hash, ID, ordinal or validity envelope is rewritten. Schema-1 diagnostics omit capacity exactly as before; schema-2 diagnostics carry bounded typed `CapacityObservation` version 1. Negative observations remain queryable after archive and form a cross-directory fence for the same binding; expiration/failure/reset cannot erase a denial. A newer explicit positive read may supersede it. Cache and identity fingerprints do not acquire volatile consumption fields. Memberships reference existing CapacityGroups only.

Bucket bodies are omitted from compact snapshots/list/history pages; explicit reports and exact details retain them within the existing evidence/frame/storage budgets. Overflow remains an explicit terminal failure using the reserved capacity, never silent truncation. Local storage accounting is not subscription capacity. No second history/database or provider query is introduced by inspection.

## Durable command contract

Opaque-base-v2 is a versioned extension of the existing immutable Git JSON
records, not another table/content store. `preservation` is absent for legacy
records and explicitly versioned when present; null/future versions are rejected.
Legacy serialization/revision hashes retain their original meaning. New Git
intent reads allow the separate 1 MiB manifest budget in addition to the prior
65,536-byte intent envelope; artifact limits still count metadata plus text.
Restoration binds artifact/intent manifests, worker allowlists and integration
snapshots exactly. Metadata contains native object IDs, modes, lengths and
SHA-256, never opaque bodies. Existing base/ownership refs retain reconstruction
objects. No trigger/migration is rewritten and no SQLite writer spans Git or
runtime work. Older readers must reject the unknown version/field rather than
reinterpreting a v2 record as textual history.

A command contains a validated idempotency scope/key, its typed business content and an expected store revision. The initial optimistic concurrency boundary is the **whole store**: a successful business transition advances its revision once. This is intentionally conservative for serial V1 operation and does not create a scheduler.

The persistence boundary computes a SHA-256 fingerprint from the canonical typed command representation, command schema and expected revision. Input is checked against bounds before hashing; the pre-redaction representation is ephemeral and is never stored as a command payload. Runtime frame content is represented by its digest, not raw bytes. The scope/key uniqueness and stored fingerprint/result live in SQLite, not a process-local set. Equivalent replay returns the originally stored result, including its original revision, without another attempt, message, artifact, evidence or business event. Reusing a **committed** scope/key for different content or expected revision returns a conflict.

For a new key, `StaleRevision` aborts without reserving the key or storing a command result. A caller may explicitly submit an updated request under that unused key; this does not retry an external execution or permit overwriting a newer state. After a key has committed, replay must retain the original expected revision even if the store has since advanced.

Within one write transaction, processing performs these steps:

1. Acquire the writer lock and look up the command's scope/key. Replay an identical persisted command before rejecting its now-old expected revision.
2. For a new command, compare the expected revision with the current store revision and load validated, detached state.
3. Execute the permitted domain operation against that detached state and classify its outcome explicitly.
4. Validate the resulting state, enforce persisted identity/session bindings and write the business state, evidence, audit event and command result belonging to the operation.
5. Commit before publishing success or the changed state to the caller.

Two connections therefore observe a unique command result or a conflict, rather than both accepting the same operation. A technical error before commit drops all staged changes. The store does not mutate caller-owned state while the transaction is pending; rollback cannot leave a live object pretending the database accepted a transition. A response lost **after** commit is handled by replaying the same key/content and recovering the committed result.

This reserves durable intent/state only. Neither a unique Attempt ID nor a committed command proves exactly one external runtime execution. The separate [worker/runtime infrastructure](./manual-worker.md) owns dispatch and process evidence; [daemon recovery](./local-daemon.md) reconciles those facts without automatic redispatch.

## Business outcomes versus storage errors

| Outcome | Durable meaning |
| --- | --- |
| Applied | The guarded operation succeeded. Commit its resulting state, evidence, audit event and command result atomically. |
| Invalid without change | The operation was rejected without a business transition. Preserve current state and record its stable command result for deterministic replay; do not manufacture a business event. |
| Observed failure | A specific supported operation recorded a failure/block/invalidation before reporting a domain error. Commit that state and evidence, its failure event and the command result. |
| Technical failure | SQL, serialization, validation of stored data, an injected write failure or another persistence fault prevented the transaction. Roll back; return no apparent committed business result. |

`Err` alone does not select a row in this table. Classification is bound to the operation and its actual allowed transition. For example, a failed integration can retain its negative integration evidence, clear delivery approval and block the Run while returning `VerificationFailed`; discarding it would alter the domain's meaning. An invalid integration reference must not receive the same treatment. Unexpected partial mutation is a contract violation, not permission to save arbitrary errored state.

Stale candidate/application operations can invalidate approval and block a Run. A failed application must retain the supplied negative validation evidence rather than dropping it because the command returned an error. Observer protocol/budget faults retain their quarantine state; sealed-attempt rejection does not reopen the observer. Successful-return negative checks/reviews are also real transitions and must be persisted.

## Restoration and identities

Stored checkpoints are untrusted input to validated restoration, not public setters or unrestricted domain deserialization. Checkpoint format versions are explicit. Invalid IDs, ownership, dependencies, evidence versions, impossible histories and incompatible snapshots produce errors rather than repaired/defaulted entities.

Entities expose `checkpoint()` and validated `restore_checkpoint()`; private restoration code reconstructs constructors and guards inside the domain module. Observer restoration replays a bounded journal of normalized events with original frame sizes and fixed fault/loss/close facts, then requires its recomputed snapshot to match the saved snapshot exactly. No rejected raw frame is needed for this reconstruction. The journal is limited to **20,000 records**, independently of the observer's original frame/event/byte limits.

Restoration preserves:

- Project/repository/connection identities, connection revisions and immutable local runtime-session bindings across **all** Runs in the store. One session cannot be rebound to another connection/revision/adapter or assigned to two unresolved attempts.
- Existing shared-capacity groups, their member connection IDs and original Unknown/Observed values with evidence. Membership requires known connections; storing an observation does not create independent quotas or a capacity reservation service.
- Run/Task states, dependency edges, all attempts, producer inputs, exact artifact `{id, revision, base}` values, checks, reviews, integrations, application/conflict evidence, delivery generation/candidate/approval and cancellation intent.
- Observer descriptor/request, limits, original accepted byte/event accounting, sequence, observed events, fault/fatal/closed state and attempt sealing. Serializing normalized events must not reset or recalculate the original transport byte budget.
- Versioned context/configuration and complete mode/profile snapshots. Revising a definition appends a greater version; old attempts retain their original definitions and references.

Restored execution is the **last known fact**, not proof that a process is still running. Loading does not start a worker, resume a provider thread, establish current authentication or grant billing/permission authority. Reconciliation requires evidence from the trusted runtime/supervisor boundary. Keeper receipts are read outside transactions; their stored format/binding/fence validation during restoration is pure. Process exit, an empty process tree and interrupt acknowledgement remain distinct from a remote terminal.

## Conversations, definitions and diagnostics

Projects may exist without conversations; conversations may exist without a project. HOME/New session and a composer that has not sent its first nonempty message create no persistent conversation. Saved-conversation drafts are durable, while history loading does not select the last conversation automatically. Attaching a saved conversation changes its current association without duplicating messages or rewriting their historical project/work snapshots.

Definitions are versioned values, not mutable records that rewrite existing attempts. Context/configuration references and their captured data remain distinct from the provider's internal transcript and compaction history. Storing these records does not implement routing, policy precedence or a Context Pack builder.

Diagnostic observations retain their connection revision, source, scope, validity/completeness and evidence references. A failed recheck is retained as history without renewing the last successful observation's time/validity. A response for another connection revision cannot replace the current record; it must be rejected or retained only as historical evidence under that revision. Loading a historical authentication observation does not establish authentication now. This increment stores records; it does not add periodic refresh, probes or provider discovery.

## Privacy and audit

OX-013 migration 6 adds retained independent-review intents and append-only, digested observations, without rewriting migrations 1–5 or historical worker bytes. Optional correction provenance is omitted on legacy intents. Loading first restores the complete worker/review graph, then checks cross-record ownership, exact artifact/check/output/observer evidence and correction budgets. No process/provider/filesystem inspection runs inside that read transaction. The native Desktop uses the same Core project/conversation/versioned configuration commands; its nonsecret executable bindings and exact pending-command journal are native-host records, not a second SQLite execution store or a credential vault.

OX-014 migration 7 adds immutable serial-integration intents and append-only observations without rewriting migrations 1–6. Exact integration snapshots retain the Git base, selected reviewed artifact, combined checks, candidate, target fingerprint, approval and application identity. Restoration validates these records against the Run/Task and requires reciprocal integration/application evidence before accepting delivery. The aggregate checkpoint contains intent/observation hashes rather than duplicating exact file bytes; exact records remain separately stored and validated. Git effects and checks run outside SQLite transactions, through intention → effect → confirmation. A lost reply reuses the original command identity; an uncertain partial Git effect blocks new admission and never implies permission to repeat application. Historical inspection uses a bounded projection; physical revalidation is a distinct journaled background operation. See the [application profile and recovery limits](./git-worktrees.md#serial-integration-and-explicit-application).

OX-006 [Git records](../../crates/orchestrix-core/src/git_records.rs) use separate bounded exact-content validation: at most 128 changed files and 128 exclusions, 1 MiB per file or patch, and 4 MiB of patch plus base/index/working content per artifact. Their canonical manifest revision and individual SHA-256 hashes are verified on restoration. Content that would require redaction is rejected; exact patch/evidence bytes are never sanitized into a different artifact. Command identity uses the complete Git command's digest, while stored receipts contain only bounded artifact references. `Store::command_result()` reads a validated receipt; `execute()` still enforces the original scope/key/content/expected revision. See the [Git contract](./git-worktrees.md) for file eligibility and exclusions.

Do not store credentials, provider-internal transcripts or raw rejected provider frames. Events/results use bounded, supported fields and fixed error classifications rather than exception strings, SQL errors or raw provider payloads. Sanitize supported user-facing output/text paths before any database write; synthetic known-secret tests must inspect persisted data, not only a returned UI projection.

The [privacy boundary](../../crates/orchestrix-core/src/persistence_privacy.rs) limits command/document/output JSON to depth **32**, **4,096 nodes** and **1 MiB** of aggregate UTF-8 text/key bytes. Validated aggregate checkpoints use separate bounds: depth **128**, **1,000,000 nodes** and **64 MiB** aggregate text/key bytes; each individual text still has a **1 MiB** limit. This permits full observer/history checkpoints without truncating evidence to a single-command budget. Exceeding a bound rejects the operation; these are policy bounds, not measurements of total heap memory or serialized SQLite file size.

Redaction of supported patterns/explicitly known synthetic secrets is not a universal secret detector. References/IDs must identify local records and must not embed credentials. A hash used for command identity is not encryption, credential storage or evidence that arbitrary user content is safe. Subscription Only remains unchanged and no persistence path permits a paid API fallback.

Audit events have an increasing sequence and event schema version, link to the corresponding command/revision and relevant Run/Task/Attempt references, and reject updates/deletes through the supported store boundary and schema constraints. Their evidence records facts supplied by the trusted host; it does not prove that Git, a check or a runtime actually ran. Persisted command results are validated on replay; a malformed result is not returned as a previously committed success.

## Required verification and limits

Use isolated temporary database files and fake inputs. Required acceptance covers close/reopen and domain continuation, another process opening the same test file, duplicate commands before/after reopen, payload conflicts, two connections and stale revisions, injected rollback between writes, commit-before-reply loss, retained negative integration/application evidence, stale artifact/approval rejection, older-schema migration, future-schema refusal, conversations/drafts/versioned definitions, diagnostic failed recheck and persisted synthetic-secret redaction.

Run Rust workspace check/test/example build, formatting and Clippy for affected files, plus the existing conformance check without regenerating expectations. If the shared runtime boundary changes, run its relevant JS regressions. Exact commands, totals and limitations belong to the increment evidence/current status after execution.

These tests establish behavior under their controlled file/transaction/process conditions. They do not certify recovery after power loss, network filesystems, hostile modification, production process supervision, provider capabilities, external exactly-once execution or simultaneous support for all operating systems. OX-006 adds real local Git effects outside database transactions; production supervisor/adapter, routing, daemon, Desktop and refresh retain their own tickets.

## Diagnostic cadence / policy transport — schema 9

Migration 9 preserves migration-8 connection and diagnostic bytes. It adds append-only connection_refresh_configurations (hashed body, independent revision) and connection_diagnostic_validity (immutable source cohort, refresh/policy revisions and effective class leases). Absent configuration means manual/no recurring consent. Configuration writes are CAS and do not rewrite observations or attempts. Observation restriction history prevents a later longer preference from rejuvenating old evidence.

Policy import uses the existing Settings mutation and command receipt transaction. Strict document/mapping validation is repeated against the expected revision and current local connection/observation state in that transaction. A changed binding, expired observation, stale preview or incompatible schema leaves Settings unchanged. Portability never serializes the database or imports runtime authority; see [portable policy format](../design/policy-portability.md).
