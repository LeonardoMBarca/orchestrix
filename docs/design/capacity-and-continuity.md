# Capacity, priorities and continuity

This complements the accepted [manual worker](../contracts/manual-worker.md), [routing strategies](routing-strategies.md), [diagnostics](connection-diagnostics.md) and [context ownership](../CONTEXT_OWNERSHIP.md). It does not reopen their historical acceptance. [Status](../DEVELOPMENT_STATUS.md) owns implementation claims.

## Capacity evidence (CAP-01)

Keep these dimensions separate: subscription usage/renewal windows, request frequency, concurrency, model/resource limits, session context budgets and API credits/billing. A subscription percentage is not an exact token count. A plan name does not establish a window or entitlement.

An authorized adapter observation, when officially exposed, identifies its connection/revision, existing **CapacityGroup**, resource and window, plus: used/remaining/limit where available, unit, window start/reset, source/evidence reference, observed time, expiry and confidence. Fields may be unknown. Preserve provider-reported values, local measurements and estimates as distinct sources; local consumption does not cover usage outside Orchestrix. Estimates include uncertainty and never silently become provider facts.

Membership in a group does not create independent capacity per connection, model or session. Evaluate every applicable window/group separately; do not add overlapping balances. Frequency/concurrency limits cannot be converted to subscription balances. Existing domain `SharedCapacity` evidence remains valid in its original single-unit scope; it does not acquire window/freshness semantics by deserialization.

Reads use the existing authorized bounded refresh/cache/coalescence/backoff boundary. No model probe, credential scraping or unsupported private surface. Partial/failed reads preserve the last successful observation and its original validity. A connection revision invalidates eligibility from earlier evidence. A predicted reset schedules revalidation; time alone cannot confirm restoration.

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
