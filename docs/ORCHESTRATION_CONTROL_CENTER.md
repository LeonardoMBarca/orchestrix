# Orchestrix — Orchestration Control Center

## 1. Purpose

This document defines two related product requirements:

1. **Orchestrix must remain useful with only one coding-agent runtime/subscription.**
2. Orchestrix should expose a technical **Orchestration Control Center** where users can configure how work is decomposed, scheduled, routed, reviewed, retried, and constrained.

Multi-model orchestration is an important capability, but it must be an enhancement rather than a prerequisite.

A user with only Codex, only Claude Code, only Antigravity, or another single supported runtime should still receive substantial value from Orchestrix.

---

## 2. Architectural invariant: one runtime is enough

The orchestration core MUST NOT assume that two or more runtimes are connected.

The minimum supported topology is:

```text
1 project
1 runtime
1 agent instance
```

The architecture should naturally scale through:

```text
1 runtime / 1 agent
        ↓
1 runtime / N agents
        ↓
N runtimes / N agents
```

Core components such as the Planner, Scheduler, Task Graph, Event Store, Context Compiler, Verification Engine, Git Manager, and Human Control Layer must remain functional in every topology.

This means Orchestrix is fundamentally an **agent orchestration system**, not a Claude↔Codex bridge.

---

## 3. Why single-runtime orchestration still matters

A single model/runtime can still be organized into specialized, isolated sessions.

Example using only one Claude subscription/runtime:

```text
                         ORCHESTRIX
                             │
                    ┌────────┴────────┐
                    │                 │
                 Planner          Scheduler
                    │                 │
          ┌─────────┼─────────┐       │
          ▼         ▼         ▼       ▼
      Claude-01  Claude-02  Claude-03 ...
      Backend     Reviewer    Tests
```

The provider/model may be the same, but each Agent Instance can differ in:

- role;
- context;
- permissions;
- task objective;
- allowed paths;
- tools;
- verification responsibilities;
- retry policy;
- worktree;
- session history.

For example:

```text
Claude A
ROLE: Backend Engineer
WRITE: src/backend/**, tests/backend/**
CONTEXT: backend architecture + task + relevant source

Claude B
ROLE: Security Reviewer
WRITE: none
CONTEXT: implementation diff + threat model + acceptance criteria

Claude C
ROLE: Test Engineer
WRITE: tests/**
CONTEXT: specification + public interfaces + edge cases
```

This produces useful separation of responsibilities even without model diversity.

---

## 4. Capabilities that remain valuable with one subscription

The following features must not require multiple providers:

- structured Tasks;
- DAG planning;
- dependency management;
- parallel execution when the runtime/provider allows it;
- Agent Profiles;
- isolated sessions;
- Git Worktrees;
- semantic resource locks;
- Context Compiler;
- project memory;
- Failure Memory;
- deterministic build/test/lint verification;
- model-assisted review;
- retries and reassignment;
- adaptive planning;
- event sourcing;
- timeline and observability;
- automatic documentation;
- human approvals and intervention;
- rate-limit awareness;
- execution policies;
- metrics and historical performance tracking.

Cross-model routing and cross-model review are additional capabilities, not foundational requirements.

---

## 5. Limitation of same-model review

When the same model family both implements and reviews a task, review independence is weaker because the two sessions may share correlated tendencies or blind spots.

Example:

```text
Claude Implementer
        ↓
Claude Reviewer
```

is not equivalent to:

```text
Claude Implementer
        ↓
Codex Reviewer
```

Orchestrix should communicate this honestly rather than pretending that separate sessions guarantee epistemic independence.

However, same-model review remains useful when combined with independent context and deterministic evidence.

A preferred single-runtime pipeline is:

```text
IMPLEMENTER
    ↓
DETERMINISTIC VERIFICATION
    ↓
INDEPENDENT REVIEW SESSION
    ↓
CORRECTION / APPROVAL
```

The reviewer should receive evidence such as:

```text
Task objective
Acceptance criteria
Git diff
Build result
Test result
Lint/static analysis result
Known failures
Relevant architecture constraints
```

instead of being asked to blindly confirm the implementer's conclusion.

---

## 6. Execution modes

The product should expose three conceptual execution modes.

### 6.1 Single Runtime

One connected runtime/provider is sufficient.

Example:

```text
Codex only
```

Orchestrix may create multiple Agent Instances and roles from that runtime.

Features include:

- specialized roles;
- multiple sessions;
- planner/implementer/reviewer separation;
- deterministic verification;
- orchestration and memory;
- task parallelism when supported;
- quota-aware scheduling.

### 6.2 Multi Runtime

Multiple coding-agent runtimes are connected.

Example:

```text
Claude Code
+
Codex
+
Antigravity
```

Additional capabilities include:

- capability-based cross-runtime routing;
- cross-model review;
- provider failover;
- heterogeneous execution pools;
- specialization by runtime;
- empirical runtime comparison.

### 6.3 Hybrid

Subscriptions/runtimes may coexist with API-backed or local-model runtimes.

Future example:

```text
Claude Code subscription
+
Codex subscription
+
Antigravity
+
local Ollama worker
+
optional API runtime
```

This can eventually enable cheap/local agents for low-risk tasks while stronger runtimes handle high-value work.

---

# 7. Orchestration Control Center

The Orchestration Control Center is the technical interface for configuring how Orchestrix behaves.

It should expose configuration without requiring users to edit internal files manually, while still allowing the underlying policy to be represented as versionable structured data.

The Control Center should follow two principles:

1. **Progressive disclosure** — common strategies should be easy to configure.
2. **Full technical control** — advanced users should be able to inspect and override the real policies used by the system.

The UI should therefore have two levels:

```text
Strategy / Presets
        ↓
Advanced Policy Editor
```

---

## 8. Strategy presets

A first-run user should not need to understand scheduler internals.

Possible presets:

### Balanced

Prioritizes good review quality without excessive runtime consumption.

### Quality First

Uses stronger review requirements, more independent verification, and lower tolerance for unreviewed integration.

### Throughput First

Maximizes safe parallelism and reduces optional review stages for low-risk work.

### Conservative

Low concurrency, strict approvals, strong isolation, and additional review gates.

### Subscription Saver

Minimizes concurrent/duplicate model work and avoids optional model-based review when deterministic checks are sufficient.

### Custom

All orchestration policies are user-controlled.

Presets should resolve into explicit policy values. They must not hide magical behavior that cannot be inspected.

---

## 9. Control Center sections

A possible technical UI structure:

```text
Orchestration Control Center
│
├── Execution Strategy
├── Runtime Pool
├── Agent Roles
├── Task Routing
├── Concurrency
├── Review & Verification
├── Delegation
├── Retries & Recovery
├── Resource Locks
├── Context & Memory
├── Rate Limits / Quotas
├── Integration Policy
├── Human Approval
└── Advanced Policy
```

---

## 10. Execution Strategy

This section defines the high-level operating mode.

Example UI:

```text
Execution mode

● Single Runtime
○ Multi Runtime
○ Hybrid

Strategy

● Balanced
○ Quality First
○ Throughput First
○ Conservative
○ Subscription Saver
○ Custom
```

The mode may be detected automatically from connected runtimes, but users should still be able to inspect the resulting behavior.

---

## 11. Runtime Pool

Shows available execution capacity.

Example:

```text
CODEX
Status: Ready
Enabled: Yes
Max concurrent sessions: 3
Current sessions: 1
Roles: Implementer, Debugger, Reviewer
Priority: 90

CLAUDE CODE
Status: Rate Limited
Enabled: Yes
Max concurrent sessions: 2
Current sessions: 0
Roles: Planner, Architect, Reviewer
Priority: 100

ANTIGRAVITY
Status: Ready
Enabled: Yes
Max concurrent sessions: 2
Current sessions: 1
Roles: Frontend, Reviewer
Priority: 80
```

The user should be able to:

- enable/disable a runtime;
- cap concurrency;
- restrict allowed roles;
- configure relative routing priority;
- mark a runtime as preferred or fallback-only;
- inspect availability and observed usage state.

---

## 12. Agent Roles

Users should be able to configure which roles the system may create.

Example:

```text
Planner
Architect
Implementation Engineer
Backend Engineer
Frontend Engineer
Database Engineer
Debugger
Security Reviewer
General Reviewer
Test Engineer
Documentation Agent
Integration Agent
Context Compiler
```

A role can define:

- preferred runtime order;
- allowed runtimes;
- read/write permissions;
- allowed filesystem paths;
- shell access;
- network access;
- whether it may request delegation;
- whether its work requires review;
- default context policy;
- risk ceiling.

---

## 13. Task Routing

Routing should support both explicit rules and scoring.

Example technical configuration:

```text
Backend implementation
1. Codex
2. Claude
3. Antigravity

Architecture
1. Claude
2. Codex

Frontend
1. Antigravity
2. Claude
3. Codex

Security review
1. Codex
2. Claude
```

A routing score may eventually consider:

```text
user preference
+ capability match
+ availability
+ historical success
+ task risk
+ context requirements
+ current load
+ quota pressure
+ retry history
```

Users should be able to tune the weight of these factors in Advanced mode.

For single-runtime operation, the router simply chooses among Agent Profiles/instances on the only available runtime rather than failing because no alternative provider exists.

---

## 14. Concurrency

Users should have explicit control over parallelism.

Possible controls:

```text
Global maximum concurrent tasks: 4
Maximum modifying tasks: 3
Maximum review tasks: 2
Maximum sessions per runtime: provider-specific
Allow parallel tasks sharing dependency ancestors: yes/no
Allow speculative parallel execution: yes/no
```

Concurrency should be bounded by:

- runtime capabilities;
- configured limits;
- dependency state;
- resource locks;
- provider limits;
- machine resources;
- risk policy.

More parallelism is not automatically better.

---

## 15. Review & Verification

Users should control review depth by task risk.

Example:

```text
LOW RISK
Build + tests
Optional model review

MEDIUM RISK
Build + tests + lint
Independent reviewer session required

HIGH RISK
Build + tests + static analysis
Independent reviewer required
Cross-model reviewer preferred
Human approval before integration

CRITICAL
Multiple reviewers
Security/architecture gate
Human approval mandatory
```

Possible toggles:

```text
Require reviewer different from implementer session: true
Prefer reviewer from different runtime: true
Require different runtime for high risk: false/true
Fallback to same runtime when no alternative exists: true
Require deterministic checks before model review: true
```

The important behavior is graceful degradation:

```text
Different runtime available?
       │
      yes → use cross-model review when policy requests it
       │
       no → use independent same-runtime review + deterministic evidence
```

A single-subscription user must never be blocked merely because cross-model review is unavailable unless they explicitly configure such a hard requirement.

---

## 16. Delegation Policy

Agents may request delegation, but the Control Center defines boundaries.

Controls may include:

```text
Allow delegation requests: yes
Maximum delegation depth: 2
Maximum child tasks per task: 5
Allow reviewer to create corrective task: yes
Allow worker to request specialist review: yes
Require approval for new high-risk task: yes
```

The central Orchestrator remains authoritative.

---

## 17. Retry & Recovery Policy

Retries should be bounded and observable.

Example:

```text
Max retries per task: 3
Retry same agent first: yes
After first failure: fresh context/session
After second failure: alternate role/runtime if available
After third failure: human escalation
```

For a single runtime:

```text
failure
  ↓
fresh session
  ↓
alternate Agent Profile
  ↓
reduced/replanned task
  ↓
human escalation
```

For multiple runtimes:

```text
failure
  ↓
fresh session
  ↓
alternate runtime
  ↓
specialist review
  ↓
human escalation
```

---

## 18. Resource Lock Policy

Advanced users should be able to inspect active semantic locks and configure lock behavior.

Example:

```text
database.schema.users
Writer: TASK-42
Readers: none
Queued: TASK-51, TASK-58
```

Controls:

- strict vs advisory semantic locks;
- automatic lock inference;
- manual resource declarations;
- lock timeout;
- deadlock detection policy;
- human override.

---

## 19. Context & Memory Policy

The user should control context construction.

Possible settings:

```text
Context strategy: focused
Include architecture docs: relevant only
Include recent task history: 5 tasks
Include Failure Memory: yes
Include full Git diff: when under threshold
Include unrelated project history: no
Maximum context budget: runtime-aware
```

The Context Compiler should produce auditable context manifests so users can inspect what each agent actually received.

---

## 20. Quota and rate-limit policy

Orchestrix should treat runtime capacity as a schedulable resource.

Possible controls:

```text
Reserve Claude capacity for architecture/review: 20%
Prefer Codex while Claude is under quota pressure: yes
Stop assigning new tasks at quota warning: configurable
Allow runtime fallback: yes
Allow API fallback: NO in Subscription Only Mode
```

Quota data may be incomplete depending on the runtime. The UI must distinguish:

```text
MEASURED
REPORTED BY RUNTIME
ESTIMATED
UNKNOWN
```

It must never fabricate exact remaining capacity.

---

## 21. Integration policy

Controls may include:

```text
Auto-merge low-risk approved tasks: yes/no
Require integration agent: yes/no
Require all deterministic checks green: yes
Human approval before main branch: yes/no
Preferred merge strategy: merge/rebase/squash
Conflict handling: agent-assisted/manual
```

Normal worker agents should not mutate protected integration branches directly.

---

## 22. Human approval policy

The Control Center should expose approval gates by action category.

Example:

```text
Delete many files           REQUIRE APPROVAL
Database destructive op     REQUIRE APPROVAL
Production command          REQUIRE APPROVAL
Force push                  BLOCK
Modify secrets              BLOCK
Install dependency          AUTO / ASK / BLOCK
Network access              AUTO / ASK / BLOCK
```

Policy should be configurable globally and overridable per project.

---

## 23. Policy scopes

Configuration should support layered scopes.

```text
System defaults
      ↓
User defaults
      ↓
Project policy
      ↓
Execution/run override
      ↓
Task-specific override
```

Lower levels may override higher levels only where policy allows it.

Security-critical system policies may be non-overridable by agents.

---

## 24. Policy as data

The UI should not be the source of truth for orchestration behavior.

The Control Center should edit a structured policy model that can be:

- validated;
- versioned;
- diffed;
- exported;
- imported;
- attached to execution records;
- referenced by event history.

Illustrative configuration only:

```yaml
version: 1
mode: multi-runtime
preset: custom

scheduler:
  maxConcurrentTasks: 4
  maxModifyingTasks: 3

routing:
  backend:
    preferred: [codex, claude, antigravity]
  architecture:
    preferred: [claude, codex]
  frontend:
    preferred: [antigravity, claude, codex]

review:
  requireIndependentSession: true
  preferDifferentRuntime: true
  requireDifferentRuntimeForHighRisk: false
  fallbackToSameRuntime: true

retry:
  maxAttempts: 3
  freshSessionAfterFailure: true
  alternateRuntimeWhenAvailable: true

integration:
  requireGreenDeterministicChecks: true
  requireHumanApprovalForHighRisk: true

billing:
  subscriptionOnly: true
  allowApiFallback: false
```

This schema is intentionally illustrative. Its final shape must be validated against implementation needs before becoming a stable public contract.

---

## 25. Technical / expert mode

The advanced panel should expose the actual decision inputs behind orchestration.

For a selected task, users should be able to inspect something similar to:

```text
TASK AUTH-042

Required capabilities
- backend
- security

Eligible runtimes
Codex        score 0.91
Claude       score 0.86
Antigravity  score 0.62

Routing explanation
+0.30 capability match
+0.25 user preference
+0.18 historical success
+0.12 available capacity
+0.06 context compatibility

Rejected candidates
Claude-02: concurrency cap reached
Codex-03: semantic resource conflict
```

This is important for observability and trust.

The scheduler/router must be explainable enough that a developer can answer:

> Why did Orchestrix assign this task to this agent at this time?

---

## 26. Live control

The Control Center should not only configure future runs. It should support bounded live intervention.

Examples:

```text
Pause new scheduling
Reduce concurrency from 5 → 2
Disable Claude temporarily
Mark Codex as reviewer-only
Drain a runtime
Increase task priority
Force re-route waiting task
Require human review for current plan
```

Changes should generate events and should not silently rewrite historical execution configuration.

A running execution should preserve which policy version governed each decision.

---

## 27. Suggested UI concept

```text
┌──────────────────────────────────────────────────────────────┐
│ Orchestration Control Center                                │
├──────────────────────────────────────────────────────────────┤
│ Mode: Multi Runtime       Strategy: Quality First           │
│ Global concurrency: 4     Subscription Only: ON             │
├───────────────────────┬──────────────────────────────────────┤
│ Runtime Pool          │ Routing                             │
│                       │                                     │
│ ● Codex        2/3    │ Backend        Codex > Claude       │
│ ● Claude       1/2    │ Architecture   Claude > Codex       │
│ ● Antigravity  1/2    │ Frontend       AGY > Claude         │
│                       │ Security       Codex > Claude       │
├───────────────────────┼──────────────────────────────────────┤
│ Review Policy         │ Current Constraints                 │
│                       │                                     │
│ Independent: YES      │ auth.contract       LOCKED          │
│ Cross-model: PREFER   │ database.schema     FREE            │
│ High risk: HUMAN      │ Claude quota        PRESSURE        │
├───────────────────────┴──────────────────────────────────────┤
│ [Presets] [Advanced Policy] [Export] [Apply to Run]        │
└──────────────────────────────────────────────────────────────┘
```

A Single Runtime version should remain equally coherent:

```text
┌──────────────────────────────────────────────────────────────┐
│ Orchestration Control Center                                │
├──────────────────────────────────────────────────────────────┤
│ Mode: Single Runtime      Strategy: Balanced                │
│ Runtime: Codex            Max sessions: 3                   │
├──────────────────────────────────────────────────────────────┤
│ Agent Roles                                                  │
│ Implementer       up to 2                                   │
│ Reviewer          up to 1                                   │
│ Planner           shared                                    │
├──────────────────────────────────────────────────────────────┤
│ Review                                                      │
│ Separate session required: YES                              │
│ Deterministic checks first: YES                             │
│ Cross-model review: unavailable (optional enhancement)      │
└──────────────────────────────────────────────────────────────┘
```

The UI should never shame or artificially degrade single-runtime users.

---

## 28. Product implications

Single-runtime support substantially broadens the potential user base.

Orchestrix should be useful to someone who says:

```text
I only use Codex.
```

or:

```text
I only pay for Claude Max.
```

Connecting additional runtimes should progressively unlock capabilities such as:

- cross-model review;
- runtime specialization;
- provider failover;
- heterogeneous routing;
- empirical provider competition.

A useful UX pattern is:

```text
✓ Codex connected

Claude Code
Connect another runtime
→ Enables cross-model review and additional routing capacity

Antigravity
Connect
→ Adds another execution pool
```

The product's value proposition therefore becomes:

> Orchestrate the coding agents you already have — whether that means one agent runtime or many.

---

## 29. Future experimental capability: routing performance

As execution history grows, the Control Center may expose empirical routing data.

Example:

```text
Backend tasks — last 90 days

Codex
First-pass success     89%
Median iterations      1.2

Claude
First-pass success     84%
Median iterations      1.4

Antigravity
First-pass success     72%
Median iterations      1.8
```

Users could choose between:

```text
Manual routing
Preference-weighted routing
Performance-weighted routing
Adaptive routing
```

Adaptive routing should be introduced only after metrics are trustworthy enough to justify it.

---

## 30. Required invariants

The following should be treated as requirements unless superseded by an ADR:

1. A single connected runtime must be a first-class supported configuration.
2. Core orchestration must not require cross-model availability.
3. Cross-model review is an enhancement and may be configured as a hard requirement only by explicit user policy.
4. Deterministic verification should remain independent of runtime count.
5. Routing must degrade gracefully when preferred runtimes are unavailable.
6. Subscription-only mode must never silently activate API billing.
7. Policy decisions should be inspectable and auditable.
8. Policy changes during execution must generate events.
9. Historical execution must retain the effective policy/version used for decisions.
10. Agents may request policy-relevant actions, but the central control plane owns scheduling and routing authority.
11. The UI is an editor/observer of policy; structured policy state is the source of truth.
12. Single-runtime users must not receive an intentionally crippled orchestration experience.

---

## 31. Open design questions

These questions should be resolved during architecture/implementation work rather than guessed prematurely:

- How should provider concurrency limits be discovered versus manually configured?
- Which quota information can be obtained reliably from each runtime?
- Should routing use additive weighted scoring, ordered rules, constraint solving, or a hybrid?
- How should semantic resources be inferred safely?
- Which policy settings are safe to change during an active run?
- How should policy migrations work after schema evolution?
- Should project policy live inside the target repository, Orchestrix state, or both?
- Which settings belong to Agent Profiles versus global orchestration policy?
- How should performance metrics account for task difficulty and avoid misleading rankings?
- What constitutes an independent review when only one model/runtime exists?

These remain explicit design questions, not hidden assumptions.
