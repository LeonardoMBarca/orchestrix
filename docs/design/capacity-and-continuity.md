# Capacity, priorities and continuity

This complements the accepted [manual worker](../contracts/manual-worker.md), [routing strategies](routing-strategies.md), [diagnostics](connection-diagnostics.md) and [context ownership](../CONTEXT_OWNERSHIP.md). It does not reopen their historical acceptance. [Status](../DEVELOPMENT_STATUS.md) owns implementation claims.

## Capacity evidence (CAP-01)

Keep these dimensions separate: subscription usage/renewal windows, request frequency, concurrency, model/resource limits, session context budgets and API credits/billing. A subscription percentage is not an exact token count. A plan name does not establish a window or entitlement.

An authorized adapter observation, when officially exposed, identifies its connection/revision, existing **CapacityGroup**, resource and window, plus: used/remaining/limit where available, unit, window start/reset, source/evidence reference, observed time, expiry and confidence. Fields may be unknown. Preserve provider-reported values, local measurements and estimates as distinct sources; local consumption does not cover usage outside Orchestrix. Estimates include uncertainty and never silently become provider facts.

Membership in a group does not create independent capacity per connection, model or session. Evaluate every applicable window/group separately; do not add overlapping balances. Frequency/concurrency limits cannot be converted to subscription balances. Existing domain `SharedCapacity` evidence remains valid in its original single-unit scope; it does not acquire window/freshness semantics by deserialization.

Reads use the existing authorized bounded refresh/cache/coalescence/backoff boundary. No model probe, credential scraping or unsupported private surface. Partial/failed reads preserve the last successful observation and its original validity. A connection revision invalidates eligibility from earlier evidence. A predicted reset schedules revalidation; time alone cannot confirm restoration.

## Codex CAP-01 mapping — pinned alpha.17.2

The implementation reuses the existing `account/rateLimits/read` response, with unchanged `supportsLunaReserve:false` / `excludeResetCreditDetails:true`. No `account/usage/read`, reset-credit, administrative or inference call is added. The [official app-server documentation](https://learn.chatgpt.com/docs/app-server) describes current multi-bucket quota views; the executable-generated v2 schema pins the supported version. Static generation on 20 October with installed `0.162.0-alpha.17.2` reproduced `GetAccountRateLimitsResponse.json` SHA-256 `cb9655e68130116f634ed9353e45b336492e6069a6628f424d1cd52e54eda0e0`, identical to the retained onboarding schema. Generating that public schema reads no account. Current web documentation and the pinned schema are not interchangeable: `ordinaryUsageAllowed` is documented in the pinned generated schema, rather than the current web example.

| Official field/surface | Meaning and unit/scope | Normalized fact and evidence | Limitation |
| --- | --- | --- | --- |
| `ordinaryUsageAllowed` | Backend permission for ordinary included usage; boolean or unavailable/null | `ordinaryUsage`: reported true/false, missing, null or invalid; pinned schema and synthetic fixtures | False alone does not identify a window or cause. Only explicit true satisfies the existing execution gate. |
| `rateLimitsByLimitId`; legacy `rateLimits` | Opaque metered-resource map; backwards-compatible single view | Separate bounded buckets; legacy view retained separately; official docs/schema | Never add views or infer independent quotas. A display alias is not a selected-model entitlement. |
| `limitId`, `limitName`, `normalModelSlug` | Opaque ID, label and display alias | Sanitized reported/missing/null/invalid fields | No remote account ID, quota independence or cross-account matching is inferred. |
| `primary`, `secondary` ? `usedPercent` | Integer percent used of that resource window | Original reported percentage; `100-used` explicitly derived only in 0..100 | No token/currency/task conversion. Out-of-range int32 retained with partial diagnostic, without bar/complement. |
| `windowDurationMins`, `resetsAt` | Minutes; Unix epoch seconds | Explicit units, optional-field states, bounded browser-safe values | Crossing a reset expires an old observation; it never restores permission or saldo. A new read has its own date. |
| `rateLimitReachedType` | Pinned backend reason enum | Reported reason, distinct from permission and read failure | A generic denial is not labeled weekly exhaustion. Credit-related reason is not a monetary balance. |
| `credits`, `individualLimit`, `spendControlReached`, reset-credit details | Other plan/workspace/credit constructs; numeric strings lack the required documented unit in this mapping | Not mapped as a quantitative subscription balance | No invented money, tokens, unlimited quota or extra administrative call. |

Quantitative parsing is pinned to alpha.17.2 and explicit synthetic profiles. The older supported alpha.2 retains its existing qualitative permission gate, with quantitative schema unverified. Missing optional telemetry does not invalidate an otherwise permitted connection. Raw provider account IDs, upsell objects and credentials never enter the observation. Existing local CapacityGroup memberships are referenced where present, without rewriting their original single-unit evidence.

The observation has schema version 1, carried by a schema-2 diagnostic (schema-1 historical records remain byte-identical). Source, connection/binding, runtime version, observed date, synthetic provenance, reported-subset coverage and bounded issues are retained. Operational freshness is the minimum of the original source ceiling, captured/current stricter class policy and any reported future reset. Changes in consumption never enter the identity fingerprint. A negative reading is durable evidence from a technically successful partial discovery: authentication was observed, configuration/catalog not inspected remain unknown, and no executable adapter is returned.

Schema 11 adds only an indexed lookup over immutable negative permission bodies. A newer negative blocks older positive metadata across directories, including after archive/expiration/reopen; only a newer explicitly positive read can supersede it. Metadata cache invalidation preserves backoff. Worker diagnostic snapshots retain the exact typed observation in the existing immutable attempt evidence, without turning it into model context; their availability lease uses the same reset ceiling. Admission and the last moment before `turn/start` retain the mandatory permission/freshness guard. It does not interrupt an already authorized active turn or add a low-balance threshold.

Connections/Settings load full facts on demand; compact snapshots and historical pages omit bucket bodies, while exact inspection resolves original bytes/hashes. Preparation carries its exact project-directory diagnostic and binding, not registration evidence substituted as execution authority. Policy export/import and Context Packs do not transport capacity observations. UI distinguishes current permission, historical reading, failed update, null/missing/invalid, renewal not confirmed and local storage limits. Availability now is never a guarantee of finishing implementation, review or corrections. [Current status](../DEVELOPMENT_STATUS.md) separates schema/fixture/native proofs from authenticated observations. One explicitly authorized alpha.17.2 read confirmed permission and a percent/duration/reset window plus an equivalent legacy view; the secondary window and display alias were null. Monetary balances, independent quotas, all resources and execution entitlement were not certified by that read. Personal values remain in ignored local evidence, not the policy export or repository.

## Admission and reserves (CAP-02 / CAP-03)

Future configurable local reserves cover planning/replanning, model-assisted synthesis, mandatory review and corrections needed to finish. They are Orchestrix admission rules, not provider reservations or protection from external consumption. Keep per-window accounting and estimation uncertainty explicit; the same reserve cannot be counted twice through overlapping groups/windows.

Prioritization considers objective, risk, dependencies, phase, work already produced/active, required verification and remaining observable capacity. It is not a fixed ranking of roles. Under pressure, completing mandatory review can precede starting optional implementation. Efficiency/Performance/Balanced/Custom tune preferences and margins; none bypasses required checks, quality floors, permissions or billing restrictions. Performance preserves continuity.

Explain warning thresholds, restrictions on new work and controlled waiting. Use separate enter/release thresholds, minimum dwell/backoff and newer scoped evidence to avoid oscillating on small changes. Unknown means neither zero nor unlimited: apply a configurable conservative policy, show the limitation and keep single-account use viable. Confirmed exhaustion cannot be cleared by a failed refresh or the wall clock crossing a reported reset.

## Deterministic control and memory (CONT-01 / CONT-02)

The Core owns permissions, state, persistence, suspension and cancellation. Local status/inspection/recovery controls need no model or quota. Suspending admissions does not pause/cancel a turn; client disconnection does not stop the daemon. Continue observing and persisting active work under its previously authorized budgets even when new orchestrator decisions are unavailable. Do not indiscriminately cancel workers or repeat uncertain effects.

Persist at relevant transitions: objective, plan/revision, recorded decisions, tasks/dependencies, blockers, attempts, artifacts, evidence, approvals and known next steps. Never rely on a final model call after exhaustion. Mark hypotheses and missing information; do not claim to capture hidden reasoning or unobserved decisions.

The deterministic Context Compiler builds a bounded basic continuity pack from durable facts and authorized sources with versions and explicit omissions. An optional synthesis agent can enrich it, but is not its sole source. Stored authentication/quota observations remain historical after restart. Reconstruction grants no new execution permission.

## Orchestrator unavailability and transfer (CONT-03)

Suspend only new delegations needing unavailable decisions; retain local controls and evaluate active work by existing authorization. If an eligible authorized alternative exists, a future transfer supplies reconstructed context to a new session. Without one, wait with state intact. Account/provider changes do not transfer native internal context. Native resume requires a compatible identity and mechanism.

One revision/generation of orchestrator authority may issue new decisions at a time. Revoke the previous generation before handing over; reject delayed decisions from it. Respect shared capacity, quality, permissions and Subscription Only. Never rotate accounts to circumvent provider restrictions or enable a paid API fallback.

## Delivery boundaries and acceptance

| Requirement | Owning delivery | Evidence required |
| --- | --- | --- |
| CAP-01 observation shape/freshness | OX-007 diagnostics/adapters; OX-016 inspection | Absent/stale/partial/transient/exhausted observations stay distinct; revision/reset requires revalidation; no fabricated percentages. Provider mapping needs official exposure. |
| CAP-02 margins/reserves | OX-008 policy extension; M3 pool | One account/serial remains useful; no double counting across windows/groups; reservations preserve mandatory completion work and declare uncertain cost. |
| CAP-03 contextual priority | M3 pool / M4 orchestration | Mandatory review of existing work outranks optional new work under the configured constraints; hysteresis/waiting is explainable; strategy never removes a mandatory check. |
| CONT-01 local controls | OX-010 / OX-011 | Same daemon serves two clients; suspend/cancel/status/recovery work without a provider; reconnect/replay never duplicates execution. |
| CONT-02 durable basic context | OX-005 records / OX-011 reconstruction | Crash/reopen reconstructs facts, sources, versions, budgets and omissions without synthesis/inference, preserving artifacts/approvals and uncertainty. |
| CONT-03 role transfer | M4 orchestration | Quota loss with/without alternative, single-authority fencing, late-response rejection, identity-safe native resume versus fresh session. No claim of actual handoff from component tests. |
| Product communication | Website / OX-016 Control Center | Explain capacities, contextual priorities and continuity without fictional balances, independent-account assumptions or exact exhaustion forecasts. |

**Existing:** CapacityGroup persistence, scoped diagnostic histories, deterministic policy/Context Pack, runtime rate-limit classification, bounded refresh and durable manual worker in the accepted Windows/Codex scope. **Delivered in this batch:** daemon controls and safe reconciliation/basic reconstruction in the Windows/serial offline scope; [evidence](../research/local-daemon-recovery-increment.md) separates component/crash/integrated tests from the earlier live provider proof. **Still planned:** provider-specific quota observations beyond exposed metadata, preventive reservations, predictive admission, contextual multi-agent scheduling and actual orchestrator transfer. No autonomous quota-testing agent or full scheduler is introduced.
