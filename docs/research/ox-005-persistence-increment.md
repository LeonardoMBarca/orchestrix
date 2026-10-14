# OX-005 — SQLite persistence and durable commands

Recorded **14 October 2026**, continuing `docs/development-plan` at `d1a307a`. The working tree was clean at entry. This increment implements the existing OX-005 ticket; it does not reopen D0/D1, provider gates or the production worker.

## Decision and implementation

[ADR-0006](../adr/0006-synchronous-sqlite-persistence.md) selects **rusqlite 0.40.2** with only `bundled`; the compiled SQLite reports **3.53.2**. SQLx 0.9.0 received an official-document comparison, not a second implementation. Cargo dependency download was explicitly authorized; the existing Rust/MSVC/SDK were reused. Serde derive and SHA-256 (`sha2` 0.10.9) are locked project dependencies. No global component, provider, login or inference was installed or invoked.

[Persistence](../../crates/orchestrix-core/src/persistence/mod.rs) is separate from the pure domain. It uses an explicit absolute file path, versioned migrations 1 → 2, application identity, foreign keys, WAL, `synchronous=FULL`, a 250 ms busy timeout and `BEGIN IMMEDIATE`. Unknown/future schemas and invalid checkpoints/projections are refused without deleting or recreating data. Migration failure retains the previous schema version and existing data.

Current aggregate checkpoints coexist with relational identities, ownership constraints and indexed projections. Append-only events have sequence/schema version and optional Run/Task/Attempt references; command results and evidence are also protected against update/delete. This is transactional state plus audit, not full event sourcing. Definitions and diagnostics have immutable versioned rows; attempt snapshots retain exact context/configuration/mode/profile and diagnostic references.

The command boundary loads detached state, verifies the expected global revision, applies a guarded operation and atomically saves the state, evidence, event and result. It explicitly distinguishes:

- `Applied`: a successful guarded command; unchanged operations need no business event.
- `InvalidNoChange`: a stable rejected result, with no business transition/event.
- `ObservedFailureCommitted`: a supported failure that changed domain state, including negative integration/application validation, stale candidate invalidation and observer quarantine.
- Technical storage/validation failure: rollback with no published persisted success.

The allowlist is operation-specific and also requires an actual state change; a `DomainError` alone is insufficient. Unexpected partial mutation rolls back. Negative application evidence now has a domain history rather than being discarded. Run restoration validates its initial revision and chronological integration references, including failures that do not advance the result.

Idempotency is keyed by `(scope, key)` in SQLite and binds the typed command content, original frame digest and expected revision to the stored result through SHA-256. Only a digest is retained, never the raw command/frame. Replay precedes expected-revision rejection and adds no effect. Different content conflicts. The database's writer transaction, uniqueness and a partial unique index on unresolved session reservations protect two callers; no process-local Set is relied upon. Durable reservation does not prove exactly one external execution.

## Demonstrated durable path

[File workflow tests](../../crates/orchestrix-core/tests/persistence_workflow.rs) create a project, connection/session, definitions and Run/Task, reserve an attempt with exact snapshots, consume synthetic portable runtime observations, publish/check/review an artifact, close/reopen and continue integration/application/delivery. Replaying the reservation before/after reopen does not duplicate the attempt or audit event.

A separate child test process opens a database left by its parent with an approved artifact and complete attempt/check/review history. It validates the restored facts and continues the guarded integration; the parent opens it again and verifies the committed evidence/revision. A smaller unit subprocess path independently demonstrates project → Run continuation.

Other file tests cover negative integration and application evidence with a blocked Run; rollback at state/event/result seams; commit followed by lost reply; two connections racing on a command and stale revisions; finite busy waiting; old-schema migration, failed migration and future-schema refusal; exact artifact versions and invalidated approvals; global session binding and unresolved reservations across Runs; saved drafts/conversations/attachments/work links; immutable definition versions; capacity-group membership/unknown versus observed quota; failed diagnostic refresh and old connection revisions.

The observer checkpoint stores projected accepted facts, original accepted byte accounting, bounds, faults, uncertainty and closed/sealed state. It is restored between every operation of the existing 245-case/935-operation corpus without changing its expected snapshots. Reopening does not start, resume or authenticate a provider. A 256-event observer journal passes through the larger aggregate privacy envelope without truncating history.

Synthetic-secret checks inspect saved text, raw database/sidecar bytes and failure paths, including malformed runtime input. Rejected raw frames and provider transcripts are absent. Credential-shaped command scope/key values are rejected before SQL. Supported-pattern redaction is bounded and deterministic, not a universal secret detector; [the contract](../contracts/persistence.md) documents command versus aggregate limits.

## Verification

Commands ran on Windows/MSVC with the locked cached dependencies. The complete Rust workspace passed **92 tests**, plus two child helpers marked ignored in the main runner and explicitly executed successfully by their parents in other processes. Final restoration/counter hardening then passed the affected checkpoint/boundary/workflow suites again (16 + 9 + 8 parent tests) and check/build/Clippy. The preceding 35 Rust tests were actually rerun here, not copied from historical totals.

| Check | Result |
| --- | --- |
| `cargo check --workspace --all-targets --locked --offline` | Passed |
| `cargo test --workspace --locked --offline` | Passed: 92 main-runner tests; two child helpers explicitly exercised by parents |
| `cargo build --workspace --examples --locked --offline` | Passed |
| Rustfmt on affected domain/new persistence sources; `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed; compact pre-existing runtime file style preserved |
| Runtime harness syntax check | Passed |
| Existing conformance reproduction | Passed: 245 cases / 935 operations, no regeneration |
| Default offline JavaScript regression | Passed: 320/320, no skips or provider calls |
| `npm.cmd run test:rust-core` | Passed: 11/11, zero provider calls |
| Documentation links / `git diff --check` | Passed |

## Limits and next dependency

These are real SQLite-file tests with synthetic domain/runtime facts. Fault injection proves transaction rollback under the tested conditions; it is not proof of power-loss recovery. Parent/child process tests prove reopening durable domain state, not restarting a runtime or recovering a killed database writer. No production adapter/supervisor, scheduler, Git project operation, daemon, Desktop, refresh or live capability validation was implemented. Windows/MSVC execution does not certify macOS/Linux runtime support.

The initial synchronous implementation loads/validates a bounded whole-store view and uses a conservative global expected revision. It is an explicit serial V1 baseline, not a throughput benchmark or unrestricted concurrent store. Future async hosting must offload complete short operations and never hold a transaction across inference or `.await`. Checkpoint metadata is local historical evidence, not cryptographic proof against a coherently forged database.

Subscription Only, one-account serial use, manual control, strategies/presets and runtime-versus-Orchestrix context ownership remain intact. OX-006 worktrees/provenance is the next sequential increment; OX-007/OX-008 retain their own dependencies. OX-005 acceptance and broader milestone status remain owned by [the backlog](../DEVELOPMENT_BACKLOG.md) and [current status](../DEVELOPMENT_STATUS.md).
