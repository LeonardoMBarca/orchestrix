# Orchestrix Core — runtime contract increment

OX-003 now has a Rust consumer of the portable runtime contract 1.1. This crate contains only validation, admission checks and observation of one owned attempt. It has no Node dependency, provider SDK/messages, OS process handles, Tauri, SQLite, scheduler, worker or daemon.

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

[runtime-conformance-v1.json](tests/fixtures/runtime-conformance-v1.json) is a versioned, deterministic JSON corpus: **104 cases** with construction results and each operation/event/snapshot/error checkpoint from the JavaScript executable specification. It includes seven fake scenarios, the normalized existing Codex evidence and the three explicitly synthetic replay cases, plus adversarial identity, provenance, protocol, compatibility, limits, policy and output checks. The future refresh policy is not implemented by this corpus.

[export-conformance.mjs](../../tools/runtime-harness/export-conformance.mjs) rebuilds the corpus offline from the existing contract/fake/replay and input hashes. `--check` checks committed bytes without overwriting; `export:conformance` is an explicit regeneration. Review semantic changes to expectations instead of using regeneration to bless a regression. Rust tests load the committed JSON directly and need no Node. They compare complete normalized data and snapshots, including events and byte budgets; JSON object ordering is immaterial.

The corpus and Cargo/Rust sources use LF through repository attributes. Input hashes explicitly use `lf-normalized-input-bytes`; CRLF/LF variants produce identical golden data without rewriting historical reports. This prevents a Windows checkout from making the generated corpus stale while keeping raw historical hashes separate.

[runtime_observe](examples/runtime_observe.rs) is an offline example reading a bounded **4 MiB trusted host bundle** from stdin: `{descriptor, request, frames: [{origin, frame}]}`. It sends these to the same library consumer, returns normalized output or a fixed typed error, and performs no provider/process/storage effects. It is a test/diagnostic bridge, not the future local daemon or an endpoint granting provider-supplied supervisor assertions.

The separate [adapter → Rust tests](../../tools/runtime-harness/tests/rust-core.spec.mjs) require that example to be built; they launch only the own Node fake app-server and this Rust consumer. They are opt-in from the Node harness because its default suite does not require a Rust toolchain. `success`, cooperative interruption, rate limit and malformed transport reports are normalized by the actual adapter, then checked against the Rust result. The provider-neutral fake uses the same executable. A successful historical replay is translation evidence, not new authentication or live containment.

## Remaining boundaries

This is a portable Core module, not a production adapter/supervisor. The actual runtime transport and private native mappings remain in the experimental Node harness; moving launcher/transport to Rust belongs to OX-007. Credential identity/entitlement, active execution and filesystem checks are not established by this parser. Catalog pagination/capability refresh are adapter/application responsibilities, not per-message behavior in this crate.

Timestamp inputs at this boundary are canonical RFC3339 observations emitted by the normalized adapters; invalid/ambiguous legacy date strings are not execution evidence. Conformance here demonstrates the committed corpus, not equivalence with every permissive `Date.parse` or non-Unicode JavaScript input. Legacy timestamp grammar still needs reconciliation before unrestricted raw-input compatibility is declared; this remains an OX-003 follow-up, not a reason to claim its full acceptance now. Event parsing projects whitelisted finite-depth fields and validates/skips deeply nested unknown data; tests cover 500-level valid payloads, malformed ignored JSON and wrong types in known fields. The example's outer host-bundle decoder remains bounded and is not a production transport. Frame/collection bounds do not certify global memory usage, durability or cross-platform runtime support.

See [contract](../../docs/contracts/runtime-contract-v1.md), [Rust increment evidence](../../docs/research/rust-runtime-contract-increment.md) and the [existing backlog](../../docs/DEVELOPMENT_BACKLOG.md). OX-004, persistence, routing and production Desktop retain their gates.
