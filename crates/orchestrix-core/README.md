# Orchestrix Core — runtime contract and domain

OX-003 now has a Rust consumer of the portable runtime contract 1.1. Its runtime module contains validation, admission checks and observation of one owned attempt. The crate has no Node dependency, provider SDK/messages, OS process handles, Tauri, SQLite, scheduler, worker or daemon.

OX-004 adds the [pure in-memory domain](src/domain/mod.rs): typed identities, projects/repositories, connections/shared capacity, conversations/session references, versioned modes/profiles, attempts, tasks and Runs. It consumes the same runtime observer and keeps execution outcome, output production, artifact acceptance, internal integration and final application separate. [Domain contract/API rules](../../docs/contracts/domain-and-states.md) and [acceptance evidence](../../docs/research/ox-004-domain-increment.md) describe guarded operations and the 29 new domain tests. SQLite and actual process/Git/check services are not implemented by this module; OX-005 is the next increment.

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

## Conformance and adapter integration

[runtime-conformance-v1.json](tests/fixtures/runtime-conformance-v1.json) is a versioned, deterministic JSON corpus: **245 cases/935 operations** with construction results and each operation/event/snapshot/error checkpoint from the JavaScript executable specification. It includes seven fake scenarios, normalized existing Codex evidence and the three explicitly synthetic replay cases, plus adversarial identity, provenance, protocol, compatibility, limits, policy and output checks. The preceding 104 cases remain identical; new raw/timestamp/error-priority cases identify the codec profile explicitly. [Independent expected vectors](../../tools/runtime-harness/fixtures/portable-input-v1.json) prevent relying solely on the oracle. The future refresh policy is not implemented by this corpus.

[export-conformance.mjs](../../tools/runtime-harness/export-conformance.mjs) rebuilds the corpus offline from the existing contract/fake/replay and input hashes. `--check` checks committed bytes without overwriting; `export:conformance` is an explicit regeneration. Review semantic changes to expectations instead of using regeneration to bless a regression. Rust tests load the committed JSON directly and need no Node. They compare complete normalized data and snapshots, including events and byte budgets; JSON object ordering is immaterial.

The corpus and Cargo/Rust sources use LF through repository attributes. Input hashes explicitly use `lf-normalized-input-bytes`; CRLF/LF variants produce identical golden data without rewriting historical reports. This prevents a Windows checkout from making the generated corpus stale while keeping raw historical hashes separate.

[runtime_observe](examples/runtime_observe.rs) is an offline example reading a bounded **4 MiB trusted host bundle** from stdin: `{inputProfile: {id: "orchestrix-runtime-json", revision: 1}, descriptor, request, frames: [{origin, frame}]}`. Missing/unsupported profiles return `version`; semantic wire minors cannot downgrade decoding. It sends frames to the same library consumer, returns normalized output or a fixed typed error, and performs no provider/process/storage effects. It is a test/diagnostic bridge, not the future local daemon or an endpoint granting provider-supplied supervisor assertions.

The separate [adapter → Rust tests](../../tools/runtime-harness/tests/rust-core.spec.mjs) require that example to be built; they launch only the own Node fake app-server and this Rust consumer. They are opt-in from the Node harness because its default suite does not require a Rust toolchain. `success`, cooperative interruption, rate limit and malformed transport reports are normalized by the actual adapter, then checked against the Rust result. The provider-neutral fake uses the same executable. A successful historical replay is translation evidence, not new authentication or live containment.

## Remaining boundaries

This is a portable Core module, not a production adapter/supervisor. The actual runtime transport and private native mappings remain in the experimental Node harness; moving launcher/transport to Rust belongs to OX-007. Credential identity/entitlement, active execution and filesystem checks are not established by this parser. Catalog pagination/capability refresh are adapter/application responsibilities, not per-message behavior in this crate.

The [input profile](../../docs/contracts/runtime-input-profile.md) defines strict Gregorian/offset timestamps, finite binary64 JSON numbers and Unicode scalar strings. Normalized UTC milliseconds are idempotent; invalid/ambiguous legacy strings never become evidence. This is scoped conformance, not equivalence with every permissive `Date.parse` or arbitrary JavaScript input. Lexical validation includes ignored/overwritten fields; projection validates/skips compatible deep unknown data, including the existing 500-level regressions. Typed invalid-field priorities and numeric rounding match the compared inputs. The example's outer host-bundle decoder remains bounded and is not a production transport. Frame/collection bounds do not certify global memory usage, durability or cross-platform runtime support.

See [contract](../../docs/contracts/runtime-contract-v1.md), [current acceptance](../../docs/research/portable-runtime-input-increment.md), [preceding Rust evidence](../../docs/research/rust-runtime-contract-increment.md) and the [backlog](../../docs/DEVELOPMENT_BACKLOG.md). OX-004 domain/states is the next permitted increment; persistence, routing and production Desktop retain their dependencies.
