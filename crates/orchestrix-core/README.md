# Orchestrix Core — runtime contract, domain and persistence

OX-003 provides the Rust consumer of portable runtime contract 1.1: validation, admission checks and observation of one owned attempt. The crate has no Node dependency, provider SDK/messages, OS process handles, Tauri, scheduler, production worker or daemon.

OX-004 adds the [pure domain](src/domain/mod.rs): typed identities, projects/repositories, connections/shared capacity, conversations/session references, versioned modes/profiles, attempts, tasks and Runs. It consumes the same runtime observer and keeps execution outcome, output production, artifact acceptance, internal integration and final application separate. [Domain contract/API rules](../../docs/contracts/domain-and-states.md) and [original acceptance evidence](../../docs/research/ox-004-domain-increment.md) describe guarded operations. The domain module remains independent of SQL and actual process/Git/check services.

OX-005 adds [SQLite persistence](src/persistence/mod.rs), outside the domain, using rusqlite 0.40.2 with bundled SQLite 3.53.2. It stores current state, evidence, append-only audit events and idempotent command results transactionally. Validated checkpoints preserve private invariants, observer budgets/quarantine, exact artifact/approval versions and historical definitions. [Persistence contract](../../docs/contracts/persistence.md) and [ADR-0006](../../docs/adr/0006-synchronous-sqlite-persistence.md) define the boundary; [current status](../../docs/DEVELOPMENT_STATUS.md) owns the ticket/milestone completion decision.

OX-003 is accepted for the first local single-account runtime under [portable input profile revision 1](../../docs/contracts/runtime-input-profile.md). This explicitly replaces the previous permissive experimental decoder; normalized v1.0/v1.1 data remains compatible, while ambiguous/nonportable inputs are rejected without timezone inference. Semantic wire version and codec revision are separate. [Acceptance and verification](../../docs/research/portable-runtime-input-increment.md) do not close M0 or production/provider gates.

`runtime_contract` exposes validated `RuntimeDescriptor`/`RuntimeRequest` wrappers, `RuntimeObserver`, `EventOrigin`, `Limits`, normalization, capability decisions and explicit admission validation. JSON internals are private and immutable through the public wrapper API; snapshots are owned copies. The host passes provenance separately, and must actually establish supervisor ownership. Validating a frame/configuration fact does not authorize dispatch or certify OS containment.

The consumer preserves requested/effective settings, model-specific levels/unknowns, local connection/revision/session/attempt/turn references, sequence/version/budgets, legacy strict and factual 1.1 transitions, typed errors/quarantine, interruption and independent process/helper/stream/tree evidence. It never derives a remote terminal from process termination. No hidden reasoning or runtime conversation memory is required.

## Run

Requires Rust/Cargo and the platform's linker/toolchain. The development Windows environment uses Rust **1.99.0**, x64/MSVC; no MSRV or macOS/Linux execution is certified by that result. Cargo.lock fixes dependency versions; it does not install providers or run inference.

```powershell
cargo check --workspace --all-targets --locked
cargo test --workspace --locked
cargo build --workspace --examples --locked
cd tools/runtime-harness
npm.cmd run check:conformance
npm.cmd run test:rust-core
```

After dependency download, these Cargo commands also work with `--offline`. The minimal rustup install used for this increment does not require an IDE or new system components; existing MSVC/SDK were reused.

## Durable commands and restoration

`Store::open(absolute_path)` opens an explicit local file whose parent directory already exists and runs versioned migrations; it refuses future/incompatible schemas without recreation. `load()` returns a detached read-only `SavedState`, not a resumed runtime or selected UI conversation. Short writes use WAL/FULL, foreign keys, a 250 ms busy timeout and an immediate writer transaction.

`Request` carries a `LocalRef` scope/key, `expected_revision` and typed `Command`. `Store::execute()` returns a `Receipt` with a persisted `CommandResult`, disposition and replay flag. A committed key binds the command content **and original expected revision**; identical replay returns the original result without additional effects. Stale revisions on unused keys abort without reserving them. The store stages changes separately and commits before reporting them; business failure evidence and technical rollback have distinct outcomes.

`Command::run(id, action)` and `Command::create_run(run)` are constructors for the owning domain operations. `RunAction::assign(task, attempt, snapshot)` reserves a local attempt with an `AttemptSnapshot` referencing existing immutable mode/profile/context/configuration definitions and diagnostic observations. Reservation does not dispatch or prove external exactly-once execution. Conversation Send/Reply/Attach/SaveDraft/LinkWork commands preserve saved history and first-nonempty-send semantics. `SaveCapacityGroup` stores the existing domain's membership and Unknown/Observed evidence without inferring independent quotas or adding a quota service.

The separate [document records](src/persistence_records.rs) module validates versioned definitions and scoped diagnostic histories; [privacy](src/persistence_privacy.rs) applies bounded sanitization before storage. A failed diagnostic refresh preserves the last successful observation and its original dates; no refresh service or new authentication check is implemented. Domain `checkpoint()` / `restore_checkpoint()` and observer equivalents are validated formats, not general-purpose entity deserialization.

[Workflow tests](tests/persistence_workflow.rs) exercise file-backed Run → Task → attempt → artifact/evidence → event → reopen → continued delivery, preserved negative integration/application evidence, stale references, global session bindings, quarantine and all three injected write boundaries. Eight parent tests pass; the separate-process parent explicitly invokes its ignored helper to reload an Approved Task and continue integration. The additional storage tests in [persistence/tests.rs](src/persistence/tests.rs) cover migrations, contention, command replay and commit-before-reply loss. Ignored child helpers are executed by their parent tests rather than counted as untested capability.

```powershell
cargo test --package orchestrix-core --test persistence_workflow --locked --offline
cargo test --package orchestrix-core --test persistence_boundary --locked --offline
cargo test --package orchestrix-core --test persistence_records --locked --offline
```

These tests use isolated temporary files and synthetic observations. Rollback injection is not a power-loss recovery test; no provider login, live inference, Git project operation or process reconciliation is performed.

## Conformance and adapter integration

[runtime-conformance-v1.json](tests/fixtures/runtime-conformance-v1.json) is a versioned, deterministic JSON corpus: **245 cases/935 operations** with construction results and each operation/event/snapshot/error checkpoint from the JavaScript executable specification. It includes seven fake scenarios, normalized existing Codex evidence and the three explicitly synthetic replay cases, plus adversarial identity, provenance, protocol, compatibility, limits, policy and output checks. The preceding 104 cases remain identical; new raw/timestamp/error-priority cases identify the codec profile explicitly. [Independent expected vectors](../../tools/runtime-harness/fixtures/portable-input-v1.json) prevent relying solely on the oracle. The future refresh policy is not implemented by this corpus.

[export-conformance.mjs](../../tools/runtime-harness/export-conformance.mjs) rebuilds the corpus offline from the existing contract/fake/replay and input hashes. `--check` checks committed bytes without overwriting; `export:conformance` is an explicit regeneration. Review semantic changes to expectations instead of using regeneration to bless a regression. Rust tests load the committed JSON directly and need no Node. They compare complete normalized data and snapshots, including events and byte budgets; JSON object ordering is immaterial.

The corpus and Cargo/Rust sources use LF through repository attributes. Input hashes explicitly use `lf-normalized-input-bytes`; CRLF/LF variants produce identical golden data without rewriting historical reports. This prevents a Windows checkout from making the generated corpus stale while keeping raw historical hashes separate.

[runtime_observe](examples/runtime_observe.rs) is an offline example reading a bounded **4 MiB trusted host bundle** from stdin: `{inputProfile: {id: "orchestrix-runtime-json", revision: 1}, descriptor, request, frames: [{origin, frame}]}`. Missing/unsupported profiles return `version`; semantic wire minors cannot downgrade decoding. It sends frames to the same library consumer, returns normalized output or a fixed typed error, and performs no provider/process/storage effects. It is a test/diagnostic bridge, not the future local daemon or an endpoint granting provider-supplied supervisor assertions.

The separate [adapter → Rust tests](../../tools/runtime-harness/tests/rust-core.spec.mjs) require that example to be built; they launch only the own Node fake app-server and this Rust consumer. They are opt-in from the Node harness because its default suite does not require a Rust toolchain. `success`, cooperative interruption, rate limit and malformed transport reports are normalized by the actual adapter, then checked against the Rust result. The provider-neutral fake uses the same executable. A successful historical replay is translation evidence, not new authentication or live containment.

## Remaining boundaries

This is a portable Core module, not a production adapter/supervisor. The actual runtime transport and private native mappings remain in the experimental Node harness; moving launcher/transport to Rust belongs to OX-007. Credential identity/entitlement, active execution and filesystem checks are not established by this parser. Catalog pagination/capability refresh are adapter/application responsibilities, not per-message behavior in this crate.

The [input profile](../../docs/contracts/runtime-input-profile.md) defines strict Gregorian/offset timestamps, finite binary64 JSON numbers and Unicode scalar strings. Normalized UTC milliseconds are idempotent; invalid/ambiguous legacy strings never become evidence. This is scoped conformance, not equivalence with every permissive `Date.parse` or arbitrary JavaScript input. Lexical validation includes ignored/overwritten fields; projection validates/skips compatible deep unknown data, including the existing 500-level regressions. Typed invalid-field priorities and numeric rounding match the compared inputs. The example's outer host-bundle decoder remains bounded and is not a production transport. Frame/collection bounds do not certify global memory usage, durability or cross-platform runtime support.

See [contract](../../docs/contracts/runtime-contract-v1.md), [portable input acceptance](../../docs/research/portable-runtime-input-increment.md), [preceding Rust evidence](../../docs/research/rust-runtime-contract-increment.md) and the [backlog](../../docs/DEVELOPMENT_BACKLOG.md). Git/worktree services, production supervision, routing, daemon and Desktop retain their own dependencies; durable state alone does not complete M0.
