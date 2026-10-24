# Runtime contract v1 — OX-003 initial increment

Status: executable contract 1.1 with experimental Codex adapter and Rust Core consumer, **13 October 2026**. The mandatory [portable input profile revision 1](runtime-input-profile.md) reconciles timestamps/JSON decoding with explicit compatibility limits. [Conformance and acceptance](../research/portable-runtime-input-increment.md) close OX-003 for the first **local, single-account app-server surface**, with 245 cases/935 operations and an offline adapter → Rust pipeline. This does not close OX-001/OX-002/M0 or connect a production worker. The provider's own OAuth flow, complete discovery and further adapters retain their existing gates.

The implementation is [runtime-contract.mjs](../../tools/runtime-harness/src/runtime-contract.mjs), with an [in-memory fake](../../tools/runtime-harness/fixtures/fake-runtime.mjs) and [offline tests](../../tools/runtime-harness/tests/runtime-contract.test.mjs). It imports no provider SDK, Node filesystem/process module or network client and creates no timers. It specifies data validation and observation; orchestration policy, native parameter translation, persistence and the supervisor remain separate components. This preserves the architectural adapter boundary in [integration decisions](../research/integration-decisions.md), [ADR-0002](../adr/0002-model-and-reasoning-policy.md) and [ADR-0004](../adr/0004-context-ownership.md).

## Version and ownership

Every descriptor, request and event carries `version: {major: 1, minor: n}`. The baseline below is v1.0; current emitters use the [additive v1.1 revision](#additive-v11--factual-adapter-observations). A compatible future minor version can add fields/events: unknown fields are discarded, and an unknown event becomes `runtime.diagnostic` with code `unknown-event`, without its original type or payload. Another major version is rejected with a typed `version` error. Unknown values in a known enum are errors; they cannot silently broaden permissions.

The semantic wire version is distinct from `inputProfile: {id: "orchestrix-runtime-json", revision: 1}` at the trusted host boundary. Both consumers implement the [same decoding domain](runtime-input-profile.md): strict offset timestamps, Unicode scalar strings and finite binary64 numbers. This profile intentionally replaces the earlier permissive preproduction decoder; it is not an additive minor change promising acceptance of every historical raw JavaScript input. Existing normalized v1.0/v1.1 cases remain identical, while ambiguous dates/invalid Unicode/nonfinite numbers are rejected without timezone inference or downgrade.

The host allocates `connectionId`, `sessionId`, `attemptId` and a positive `connectionRevision`. These are bounded **local references**, not native thread IDs, e-mails, auth tokens or paths. Adapters keep the native mapping separately. A request belongs to exactly one connection revision; every event must match all four references. The host assigns a contiguous sequence starting at 1 across runtime and supervisor observations. A duplicate or gap is rejected and cannot apply the same transition twice. This is validation without external effects, not durable replay/recovery for the future Core.

## Capacity observation adjunct (CAP-01)

The normalized execution descriptor/events retain v1.1 and the existing conformance bytes. `RuntimeMetadata.capacity` is a separate bounded typed observation from the same authorized metadata read, not a provider message exposed to the domain or an execution event. Diagnostic schema 2 retains it; old schema-1 evidence omits it. The [pinned Codex mapping](../design/capacity-and-continuity.md#codex-cap-01-mapping--pinned-alpha172) documents fields, units and exclusions.

Discovery can return metadata-only negative/unknown permission observations, after successful account identification but before inspecting configuration/catalog. Their descriptor marks uninspected billing/capabilities/catalog unknown/partial; no executable adapter exists for that result. `CodexAdapter::connect` still requires explicit ordinary usage permission plus all existing authentication, subscription routing, permissions and version guards. A lease/reset guard also runs immediately before `turn/start`. Optional malformed telemetry is an explicit partial observation, not a fabricated balance or a new execution threshold. Consumption never enters the authentication fingerprint. This extension does not regenerate normalized conformance expectations.

The exact Codex `0.164.0-alpha.11.1` stable schema preserves the consumed capacity fields/units. Its [offline qualification](../research/codex-0164-offline-qualification.md) enables the same mapping without another RPC, normalized wire revision, migration or rehashing of old diagnostics. Static schemas and synthetic observations do not prove values exposed by a real account on that version.

## Descriptor and capabilities

`normalizeRuntimeDescriptor()` retains:

- Contract version, adapter ID and observed runtime version.
- Connection identity/revision, authentication status, billing surface and availability.
- Catalog completeness: `complete`, `partial` or `unknown`. A partial catalog never implies that every model or native feature was discovered.
- Connection capabilities and models with their own availability, capabilities and optional `reasoningLevels`.
- Context ownership: conversation belongs to the runtime; project memory belongs to Orchestrix.

Each capability has independent axes:

| Axis | Values | Meaning |
| --- | --- | --- |
| `support` | `supported`, `unsupported`, `unknown`, `runtime-managed` | Declared support or runtime ownership of the control. |
| `availability` | `available`, `unavailable`, `unknown` | Observed access in this scope. |
| `configuration` | `enabled`, `disabled`, `unknown` | Observed configuration, independent of support and access. |
| `evidence` | Source, scope, optional local reference/date | Provenance of the claim; not an implicit successful execution. |

The initial keys are `modelSelection`, `reasoning`, `resume`, `quota`, `tools`, `webSearch`, `streaming`, `sandbox`, `approvals`, `nativeDelegation` and `contextManagement`. Missing keys normalize to unknown on every axis. `resume` and `quota` are optional and their absence does not prevent a single-account serial run. Connection capabilities are not inherited by every model. Diagnostic `not-applicable` remains a separate check state in the [connection diagnostics contract](../design/connection-diagnostics.md), rather than being converted into unsupported runtime functionality.

`capabilityDecision()` permits a requested control only with supported, available and enabled observations. Configuration unknown is denied as unknown, not treated as disabled. Runtime-managed controls are not manual controls. Evidence remains separately visible even when the operation is denied.

`models[].reasoningLevels` contains only advertised **neutral** levels: `minimal`, `low`, `medium`, `high`, `maximum`. Null or omission means no observed list; `[]` explicitly advertises no selectable level. Explicit effort requires a supported, enabled, available reasoning control and the level in that model's list. Missing evidence produces `evidence-missing`; a known list without the requested level produces `unsupported`, without rewriting the descriptor. Model A's levels never grant them to model B. The adapter must translate officially exposed native values to these neutral levels; this increment does not invent that mapping.

## Request and effective configuration

`normalizeRuntimeRequest()` captures policy intent: `efficiency`, `performance`, `balanced` or `custom`, optional specialized preset reference, model policy (`auto`, `specific`, `runtime-default`), reasoning intent, sandbox, network and approval. It accepts a local Context Pack reference, without copying conversation history into project memory. It neither routes models nor executes tools.

`assertRuntimeAdmission()` requires the matching connection revision, observed authentication, available access and the requested billing surface. Unknown authentication or availability has its own typed error and is not fabricated into authentication failure or rate limiting. Explicit model selection requires that model's observed control. Explicit effort with automatic model selection waits for a known effective model before checking that model's advertised levels. Auto/default intent can retain unknown levels for a runtime-managed choice, but cannot bypass a known unavailable effective model. Unknown model availability remains unknown; it is not a claim of entitlement.

Billing defaults to `subscription-only`, with `paidFallback: forbidden`. An API surface cannot satisfy that request. An explicitly chosen API request must identify its own local billing authorization reference and match an API connection; that reference records consent supplied by the trusted host, without authenticating an API or authorizing a fallback. The contract never changes connection, provider or billing surface on an error.

Effective model, reasoning, sandbox, network and approval are separate observations, each with `state: known | unknown | runtime-managed`, value and evidence. Unknown values stay null and are not copied from requested values. A known value requires configuration/execution evidence or explicitly synthetic fixture evidence. A known mismatch of an explicitly selected model/effort or the execution settings is a `policy` error: v1 has no silent degradation rule. Auto/default intent may resolve to a concrete value; a known effective effort must still belong to the effective selected model's advertised list.

Unknown effective configuration remains representable for surfaces such as CLI JSON. A terminal event with unknown settings records that terminal, **not proof those settings were enforced**. The worker's policy gate must decide whether the necessary enforcement evidence exists; its implementation belongs to the later integration. Catalog presence and schema validation are not entitlement, sandbox enforcement or empirical proof of the model that served a response.

## Events, lifecycle and trust boundary

`RuntimeContractObserver.accept(frame, {origin})` accepts a bounded JSON string plus provenance supplied **separately by the trusted host**. `origin` is `runtime` by default. A provider's JSON field named origin is ignored and cannot elevate it to `supervisor`. Adapters normalize native events before this boundary; provider blobs, error messages, commands, prompts, credentials and native IDs are not part of the normalized event schema.

| Event | Observation retained |
| --- | --- |
| `configuration.observed` | Effective configuration, before admission. |
| `attempt.accepted` | Admission ACK; neither start nor completion. |
| `attempt.started` | Owned runtime start after admission. |
| `attempt.interrupt-requested` / `attempt.interrupt-acknowledged` | Request and ACK, independently of terminal outcome. |
| `attempt.terminal` | Explicit `completed`, `failed` or `interrupted` from the runtime. |
| `runtime.error` | Typed code, phase, dispatch knowledge and retryability; no raw message. |
| `process.exited` | Runtime or helper exit, separately. |
| `process.streams-closed` | Per-role stdin/stdout/stderr drain and stream fault status. |
| `process.tree-observed` | Explicit owned-supervisor observation, independent of parent exit. |

Attempt/configuration events require runtime origin. Process events require supervisor origin. The host must grant that provenance only to its own launcher/supervisor, never an arbitrary provider stream. A confirmed tree additionally requires an observed runtime exit, confirmed ownership, an owned-Job/process-group source, zero active processes and a local evidence reference. The reference itself is **not OS attestation**: this validator checks the structure and consistency of a supervisor observation; the real supervisor must establish it. Fake evidence is synthetic and cannot establish ownership of an OS process.

Completion requires observed start; explicit failed/interrupted terminals may arrive after admission without start. A parent exit, helper exit, ACK, closed pipe or confirmed empty tree never generates a runtime terminal. Forced cleanup without a terminal leaves outcome unknown. Terminal success is separate from artifact checks, acceptance and integration; none is provided by this module.

`transportLost(timeout | transport-loss | crash)` preserves an unknown outcome until a late owned terminal is observed. A terminal already confirmed is retained after later transport loss. Invalid protocol, ownership, revision, sequence, version or budget latches a fatal runtime quarantine: later runtime frames cannot manufacture completion. Independently trusted process observations can still record cleanup within the remaining budgets; they cannot create a terminal. `close()` is irreversible. No uncertainty triggers automatic retry, resend or another fake invocation.

## Bounds and retained data

Defaults are 64 KiB per UTF-8 frame, 256 accepted events and 1 MiB aggregate accepted frame bytes. Configurable upper bounds are 1 MiB/frame, 4096 events and 16 MiB total. An early string-length guard rejects a grossly oversized frame before UTF-8 encoding. Descriptors contain at most 64 models; IDs, enums and lists have explicit bounds. Budget exhaustion is fail closed, including supervisor observations when no budget remains.

Only whitelisted normalized fields survive in frozen descriptors, requests, events and snapshots. Unknown fields and raw malformed frames are not retained. Typed exceptions have fixed messages and codes. Credentials must never be supplied in a known ID/evidence field: the schema cannot detect a secret deliberately mislabeled as a local reference. Input creation, framing, OS buffers, global memory, durable storage and redaction of native logs remain responsibilities of the host/adapter.

## Executable fake and verification

`createFakeRuntime()` returns an immutable descriptor and a serial async generator `run(request)`. Each item has `{origin, frame}` for the trusted fixture driver to feed into the observer. Scenarios are `success`, `event-invalid`, `auth-invalid`, `rate-limit`, `interruption`, `timeout` and `crash`; options include unsupported optional resume/quota and unknown effective configuration. Timeout is an immediate synthetic observation, with no long timer. Parallel iteration is rejected as busy; closing an iterator releases it. The fake never launches a process, authenticates, contacts a provider or performs paid inference.

```js
import {createFakeRuntime, fakeRuntimeRequest} from './tools/runtime-harness/fixtures/fake-runtime.mjs';
import {RuntimeContractObserver} from './tools/runtime-harness/src/runtime-contract.mjs';

const runtime = createFakeRuntime({optionalUnsupported: true});
const request = fakeRuntimeRequest();
const observer = new RuntimeContractObserver({descriptor: runtime.descriptor, request});
for await (const {origin, frame} of runtime.run(request)) {
  observer.accept(frame, {origin});
}
const result = observer.snapshot();
```

Run the focused offline contract checks with `node --test --test-concurrency=1 tools/runtime-harness/tests/runtime-contract.test.mjs`. They exercise lifecycle, ownership, replay rejection, terminal uncertainty, provenance, close/quarantine, capability axes, per-model levels, requested/effective settings, billing boundaries, compatibility, budgets and secret-bearing unknown fields. The [Rust consumer](../../crates/orchestrix-core/README.md) and [portable decoding tests](runtime-input-profile.md#evidence) provide current OX-003 acceptance; the module remains separate from a production worker/daemon.

## Additive v1.1 — factual adapter observations

The current emitter version is `{major: 1, minor: 1}`. This additive revision keeps the v1.0 events and their strict guards above. It addresses actual adapter/replay ordering: a runtime may finish without an observed start, send start/terminal before its admission ACK, or complete interruption before the interrupt RPC result arrives. Historical evidence may lack ACKs or a model catalog. Recording those facts must not invent missing events, entitlement or advertised reasoning levels.

The seven new events require a v1.1-or-later envelope. The first six require runtime provenance; stream observations require supervisor provenance supplied by the host, outside the JSON:

| Event | Payload | Observation and constraints |
| --- | --- | --- |
| `configuration.observation-recorded` | `effective` | Normalize configuration states/evidence and require known model IDs to exist in the local descriptor. Preserve known requested/effective differences and unknown values. This factual record does not run capability, entitlement, reasoning-level or requested-policy preflight. |
| `attempt.admission-observed` | None | Record a real ACK already observed by the adapter. Set `admissionObserved`; move an unstarted attempt to accepted. A later ACK cannot regress running or terminal phase. No call to `assertRuntimeAdmission()` is made. |
| `attempt.start-observed` | None | Record a real owned start, including before ACK. Set `startObserved` and running phase while leaving an absent ACK absent. Duplicate or post-terminal start is rejected. |
| `attempt.terminal-observed` | `outcome`, optional typed `error` and optional `startObserved` assertion | Record explicit completed/failed/interrupted even without start or ACK. The normalized event always includes `startObserved` derived from start observations already recorded; a supplied assertion must match. It never synthesizes a start. |
| `attempt.interrupt-result` | `acknowledged: boolean` | Record the actual RPC result even when the terminal is already known, preserving that terminal. Older sanitized recordings retain the ACK without a separate send observation: keep `interrupt.requested=false` in that case. A confirmed ACK cannot later be erased by false. ACK alone still does not confirm interruption. |
| `output.observed` | `channel: assistant / tool / other`, nonnegative integer `bytes` | Record bounded safe-integer counters by channel and total. Preserve neither text nor commands, native objects or provider error messages. Output never implies start, admission or success. |
| `process.streams-observed` | `role`, `streams` | Merge partial and later stream-closure observations monotonically for the runtime or helper. Closure and fault flags cannot be erased; a fault prevents confirmed complete drain. This supervisor event never creates a remote terminal. Legacy `process.streams-closed` retains its single-observation guard. |

`snapshot()` adds `admissionObserved`, `startObserved` and `output: {assistant, tool, other, bytes, events}`. The legacy admission/start events also set the appropriate observation flags. Legacy `attempt.accepted` continues to run host preflight, and legacy completed `attempt.terminal` continues to require start. A v1.0 envelope using a new event name becomes an unknown-event diagnostic in the current observer; an older v1.0 consumer likewise diagnoses unfamiliar v1.1 events and retains an unknown result rather than inventing a terminal.

Requests, envelopes and `snapshot().scope` now support an optional **local** `turnId`. Omission or null normalizes to null, so old fixtures remain valid. When provided, the turn reference must match on every event, together with connection/session/attempt/revision. The adapter retains native turn IDs outside this contract. Local sequence, ownership, version, provenance and byte/event bounds remain enforced for all new events.

The observational path is **not dispatch authorization**. The host performs policy/billing admission before execution; the native adapter preserves its own confirmed configuration/enforcement requirements. Schema validation of a real ACK or terminal cannot authorize another dispatch. For a replay lacking a catalog, an observed model can have a local entry with catalog completeness/entitlement/levels unknown. Do not fill missing effort lists or ACKs from assumptions. A control can become available based on a configuration ACK without granting per-model execution entitlement; those axes remain separate. Manual dispatch still uses strict advertised levels, while unmapped native levels remain unknown rather than being ordered into a fabricated neutral scale.

`runtime.error` additionally supports top-level `fatal: true` for typed `protocol`, `ownership`, `sequence`, `version` or `budget` failures reported by the adapter. These quarantine later runtime events while permitting independently owned `process.*` cleanup observations within the remaining budgets. Other errors stay nonfatal unless the frame itself violates the schema; a late factual terminal may reconcile ordinary signal loss. A terminal already known is never overwritten by the error or interrupt result. Close remains irreversible, and supervisor provenance cannot create any attempt/configuration/output observation.

The [v1.1 unit tests](../../tools/runtime-harness/tests/runtime-contract-v11.test.mjs) cover pre-ACK/startless terminal ordering, missing entitlement, factual configuration, late interrupt result, output sanitization, local turn binding, fatal quarantine, legacy compatibility and strict source boundaries. The [Codex adapter](../../tools/runtime-harness/src/codex-contract.mjs) and [portable replay fixtures](../../tools/runtime-harness/fixtures/codex-contract-replay.json) exercise this same contract through the generic observer. JavaScript integration evidence is preserved in the [preceding increment](../research/codex-contract-integration.md); the subsequent [Rust consumer/pipeline evidence](../research/rust-runtime-contract-increment.md) records 104 cases and the outstanding timestamp grammar difference. No router or provider SDK is added to the Core.

Normalized terminal frames use `error: null` when no typed error was observed. Both legacy and factual terminal input accept omitted or null error, so stored normalized events can be replayed through the same generic observer without provider-specific conversion. The roundtrip regression preserves events, terminal, observation flags, output counters and supervisor facts; serialized byte budgets measure each actual input representation separately.
