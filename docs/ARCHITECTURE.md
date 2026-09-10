# Orchestrix — Architecture

## Status

This document describes the **initial target architecture**. It is intentionally more concrete than `PROJECT_VISION.md`, but it is still a design hypothesis and should evolve through ADRs and implementation feedback.

---

## 1. Architectural objective

Orchestrix coordinates heterogeneous coding-agent runtimes while keeping execution state deterministic, inspectable, recoverable, and interruptible.

The central architectural rule is:

> **Agents reason; the control plane owns state.**

Agents may propose plans, implementations, reviews, and adaptations. They should not become the authoritative owner of process state, repository state, task state, or integration state.

---

## 2. High-level architecture

```text
┌───────────────────────────────────────────────────────────┐
│                        Desktop UI                         │
│                                                           │
│ Overview | DAG | Tasks | Agents | Timeline | Git | Docs  │
│ Metrics | Approvals | Settings                            │
└──────────────────────────┬────────────────────────────────┘
                           │ IPC / local transport
                           ▼
┌───────────────────────────────────────────────────────────┐
│                    Orchestrator Core                      │
│                                                           │
│  Project Manager       Planner                            │
│  Task Store            Scheduler                          │
│  Router                Policy Engine                      │
│  Resource Locks        Context Compiler                   │
│  Event Store           Verification Engine                │
│  Git Manager           Integration Manager                │
│  Documentation Engine  Performance Registry               │
└──────────────┬──────────────────────────┬─────────────────┘
               │                          │
               ▼                          ▼
┌──────────────────────────┐   ┌────────────────────────────┐
│     Runtime Adapters     │   │    Deterministic Tools     │
│                          │   │                            │
│ ClaudeRuntime            │   │ Git                        │
│ CodexRuntime             │   │ filesystem                 │
│ AntigravityRuntime       │   │ process supervisor         │
│ FutureRuntime            │   │ build/test/lint runners    │
└──────────────┬───────────┘   │ sandbox/worktree manager   │
               │               └────────────────────────────┘
               ▼
┌──────────────────────────┐
│ External Coding Runtimes │
│                          │
│ claude                   │
│ codex                    │
│ antigravity/agy          │
└──────────────────────────┘
```

---

## 3. Architectural layers

### 3.1 Presentation layer

Responsible for human interaction and observability.

Responsibilities:

- project selection;
- objective creation;
- plan visualization;
- DAG visualization;
- task inspection;
- agent inspection;
- timeline;
- approval requests;
- manual overrides;
- logs;
- configuration;
- performance metrics.

The UI must not own orchestration state. It observes and sends commands to the core.

### 3.2 Application/orchestration layer

Responsible for coordinating domain workflows.

Major services:

- `ProjectService`
- `PlanningService`
- `SchedulingService`
- `RoutingService`
- `ExecutionService`
- `VerificationService`
- `IntegrationService`
- `ContextService`
- `DocumentationService`
- `ApprovalService`

### 3.3 Domain layer

Contains provider-independent concepts and invariants.

Primary entities/value objects:

```text
Project
Objective
Plan
PlanRevision
Task
TaskDependency
ResourceClaim
AgentProfile
AgentInstance
RuntimeDescriptor
ExecutionSession
Artifact
Evaluation
Review
ApprovalRequest
ContextPack
FailureRecord
PerformanceRecord
DomainEvent
```

### 3.4 Infrastructure layer

Responsible for external interactions:

- SQLite;
- Git CLI;
- worktrees;
- process spawning;
- stdout/stderr streams;
- runtime adapters;
- filesystem watchers;
- OS process state;
- build/test/lint commands;
- optional sandbox providers.

---

## 4. Control plane

The control plane is the authoritative coordinator.

It should answer:

```text
What work exists?
What work is ready?
What work is blocked?
Which agent can execute it?
Which resources are locked?
Which worktree belongs to which task?
Which execution is alive?
Which validations are required?
Which human approvals are pending?
Which plan revision is current?
```

No runtime adapter should make these decisions independently.

---

## 5. Runtime adapter boundary

All provider-specific behavior is isolated behind a normalized runtime interface.

Conceptual Rust-like contract:

```rust
#[async_trait]
pub trait AgentRuntime: Send + Sync {
    fn descriptor(&self) -> RuntimeDescriptor;

    async fn health_check(&self) -> Result<HealthState>;
    async fn usage_state(&self) -> Result<UsageState>;

    async fn start_session(
        &self,
        config: SessionConfig,
    ) -> Result<ExecutionSession>;

    async fn send_task(
        &self,
        session: &ExecutionSession,
        task: RuntimeTaskPayload,
    ) -> Result<RuntimeEventStream>;

    async fn interrupt(&self, session: &ExecutionSession) -> Result<()>;
    async fn terminate(&self, session: &ExecutionSession) -> Result<()>;
}
```

Provider-specific events are normalized into internal events.

Example:

```text
Claude stream event
        ↓
ClaudeRuntime adapter
        ↓
RuntimeEvent::ToolStarted
RuntimeEvent::Message
RuntimeEvent::Completed
RuntimeEvent::RateLimited
        ↓
Event Bus / Event Store
```

---

## 6. Process supervision

The orchestrator must supervise runtime processes directly.

Tracked process data may include:

```text
process id
runtime id
agent instance id
task id
project id
working directory
start timestamp
last event timestamp
exit code
stdout state
stderr state
cancellation state
```

A runtime process can transition through states such as:

```text
STARTING
RUNNING
INTERRUPTING
TERMINATING
EXITED
FAILED
ORPHANED
```

The OS process state, not an LLM message, determines whether the process exists.

---

## 7. Task state machine

Suggested initial state machine:

```text
CREATED
   │
   ▼
WAITING ───────────────┐
   │ dependencies met │
   ▼                  │
READY                  │
   │                   │
   ▼                   │
ASSIGNED               │
   │                   │
   ▼                   │
RUNNING                │
   │                   │
   ├──── failure ─────► FAILED
   │
   ├──── blocker ─────► BLOCKED
   │
   ▼
NEEDS_REVIEW
   │
   ├──── rejected ────► NEEDS_WORK
   │                        │
   │                        └──► READY / ASSIGNED
   ▼
APPROVED
   │
   ▼
COMPLETED
```

`CANCELLED` should be reachable from most non-terminal states.

Task transitions should be validated by the domain layer and emit events.

---

## 8. Task graph

Plans are represented as DAGs whenever possible.

Each task has:

- zero or more dependencies;
- zero or more dependents;
- optional semantic resource claims;
- priority;
- risk level;
- capability requirements;
- validation requirements.

A task is schedulable only if:

```text
state == READY
AND dependencies satisfied
AND required resource locks available
AND policy permits execution
AND at least one eligible runtime/profile exists
AND project is not paused
```

The scheduler should never use an LLM to determine simple graph reachability or dependency completion.

---

## 9. Scheduler

The scheduler decides **when** a task may run.

Inputs:

```text
ready task set
resource locks
runtime availability
project concurrency limits
runtime concurrency limits
priority
human pause state
retry backoff
policy constraints
```

Initial scheduler strategy can be deterministic and conservative.

Potential later strategies:

- priority queues;
- weighted fairness;
- capability pools;
- critical-path prioritization;
- quota-aware throttling;
- adaptive concurrency.

---

## 10. Router

The router decides **which runtime/profile** should receive an eligible task.

Initial score may use:

```text
explicit user preference
required capability match
runtime availability
profile permission match
```

Future scoring may include:

```text
historical success rate
average review score
average iterations
task similarity
context constraints
runtime quota state
risk
```

Routing decisions should emit a structured explanation suitable for audit/debugging.

Example:

```json
{
  "taskId": "AUTH-042",
  "selectedRuntime": "codex",
  "selectedProfile": "security-reviewer",
  "factors": [
    "required capability: security-review",
    "user preference: codex",
    "runtime available"
  ]
}
```

---

## 11. Worktree manager

The worktree manager owns task execution directories.

Suggested lifecycle:

```text
Task assigned
   ↓
select base commit
   ↓
create branch
   ↓
create worktree
   ↓
execute agent
   ↓
collect diff
   ↓
verification
   ↓
commit accepted output
   ↓
integration
   ↓
archive/remove worktree
```

Suggested naming:

```text
orchestrix/task/AUTH-042/claude-01
```

Physical path example:

```text
.orchestrix/worktrees/AUTH-042-claude-01/
```

Worktree metadata must be persisted so interrupted executions can be recovered.

---

## 12. Resource lock manager

Resource claims provide semantic concurrency control.

Initial lock modes:

```text
READ
WRITE
```

Rules:

```text
READ + READ   = allowed
READ + WRITE  = conflict
WRITE + READ  = conflict
WRITE + WRITE = conflict
```

Hierarchical resource identifiers may later support relationships such as:

```text
database
database.schema
database.schema.users
```

The first implementation can use exact resource names before introducing hierarchy.

---

## 13. Verification engine

The verification engine executes ordered evaluation pipelines.

Evaluator categories:

### Deterministic evaluators

```text
build
unit tests
integration tests
lint
format check
static analysis
type check
Git state checks
file policy checks
```

### Model-assisted evaluators

```text
code review
security review
architecture review
acceptance-criteria interpretation
adversarial review
```

Deterministic failures should generally outweigh model claims.

Example pipeline:

```text
BuildEvaluator
  ↓
TestEvaluator
  ↓
LintEvaluator
  ↓
CrossModelReviewEvaluator
  ↓
AcceptanceEvaluator
```

Each evaluator emits an `EvaluationResult` with evidence.

---

## 14. Integration manager

The integration manager owns the path from isolated task output to an integration branch or target branch.

Responsibilities:

- compare base and task commits;
- detect merge conflicts;
- verify task still applies to current integration state;
- optionally request integration-specific agent review;
- run post-integration validation;
- preserve provenance of integrated work;
- request human approval when policy requires it.

A task being valid in isolation does not guarantee it is valid after other tasks have been integrated.

---

## 15. Adaptive planning

Planning is versioned.

`PlanRevision` should contain:

```text
revision id
parent revision id
objective
created at
created by
trigger event
rationale
task additions
task removals
dependency changes
resource changes
approval state
```

Plan changes should not silently destroy prior history.

Adaptive planning can be triggered by:

```text
new requirement
review finding
failed verification
new dependency
integration conflict
security finding
runtime limitation
human intervention
```

---

## 16. Event architecture

All major state transitions should produce normalized domain events.

Potential event envelope:

```json
{
  "eventId": "evt_...",
  "eventType": "task.started",
  "projectId": "project_...",
  "taskId": "AUTH-042",
  "agentInstanceId": "agent_...",
  "occurredAt": "...",
  "causationId": "evt_...",
  "correlationId": "run_...",
  "payload": {}
}
```

Important metadata concepts:

- `correlationId` groups an execution flow;
- `causationId` links an event to what triggered it;
- immutable timestamps preserve ordering evidence.

The event stream can power both persistence/audit and real-time UI updates.

---

## 17. Persistence

SQLite is the initial persistence hypothesis.

Likely logical tables:

```text
projects
objectives
plans
plan_revisions
tasks
task_dependencies
resource_claims
agent_profiles
agent_instances
runtime_sessions
artifacts
evaluations
reviews
approval_requests
events
context_packs
failure_records
performance_records
worktrees
```

Not every table must exist on day one.

The schema should evolve from domain requirements rather than being generated prematurely.

---

## 18. Event sourcing strategy

The project intends to preserve a rich event log, but full strict event sourcing for every domain object is **not yet a mandatory decision**.

Two plausible approaches:

### State + append-only event log

```text
Normalized current-state tables
+
immutable audit/event table
```

Advantages:

- simpler implementation;
- easy queries;
- still supports observability and audit.

### Full event-sourced aggregates

```text
Events are authoritative
+
projections derive current state
```

Advantages:

- replay;
- time-travel;
- precise causality.

Costs:

- greater complexity;
- projection/versioning concerns;
- migration complexity.

This decision should receive an ADR before implementation.

---

## 19. Context Compiler

The Context Compiler constructs task-specific context from structured project state.

Inputs may include:

```text
task objective
acceptance criteria
permissions
current plan
project architecture
relevant ADRs
relevant source files
dependency artifacts
failure memory
recent related events
known issues
runtime context limits
```

Output:

```text
ContextPack
```

A Context Pack should be versioned and reproducible enough to answer:

> What information did this agent receive when it made this decision?

Potential pipeline:

```text
Task
 ↓
Relevance discovery
 ↓
Source selection
 ↓
Compression/summarization
 ↓
Constraint injection
 ↓
Context Pack
```

---

## 20. Memory separation

Do not merge all memory into one blob.

Suggested stores:

### Execution history

Immutable events.

### Current project knowledge

Current architecture and project facts.

### Decision memory

ADRs and explicit execution decisions.

### Failure memory

Validated failures and solutions.

### Performance memory

Historical agent/runtime outcomes.

### Session memory

Runtime-specific session references when resumable.

---

## 21. Documentation engine

Documentation generation consumes structured evidence rather than merely copying conversation text.

Inputs:

```text
current project state
plan revisions
ADRs
commits
diffs
evaluations
known issues
failure memory
events
```

Outputs may include:

```text
architecture summary
current-state summary
project map
internal changelog
known issues
agent re-contextualization files
```

Generated documentation should record provenance where practical.

---

## 22. Human approval engine

Policy should determine which operations require explicit approval.

Candidate approval categories:

```text
DESTRUCTIVE_FILESYSTEM
DANGEROUS_GIT
DATABASE_DESTRUCTIVE
PRODUCTION_INFRA
CREDENTIAL_ACCESS
OUT_OF_SCOPE_PATH
HIGH_RISK_PLAN_CHANGE
FINAL_INTEGRATION
```

Approval request states:

```text
PENDING
APPROVED
REJECTED
EXPIRED
CANCELLED
```

Agents cannot approve their own restricted actions.

---

## 23. Subscription-only mode

Subscription-only mode is a runtime execution policy, not a provider bypass mechanism.

Responsibilities:

- launch runtimes using officially supported local authentication modes;
- strip configured API billing credentials from child-process environments;
- detect unavailable/expired sessions when possible;
- surface rate limits explicitly;
- fail or reroute instead of silently enabling metered API access.

Invariants:

```text
No hidden API fallback.
No rate-limit bypassing.
No shared provider account pooling.
No credential extraction for redistribution.
```

---

## 24. Security architecture

Orchestrix coordinates agents capable of shell and filesystem access, so the threat model must be taken seriously.

Primary risks:

- destructive commands;
- credential exposure;
- prompt injection from repository content;
- supply-chain execution;
- malicious generated code;
- unintended network access;
- agent escaping allowed path boundaries;
- compromised runtime adapters;
- accidental secrets in logs/events.

Initial defenses should include:

- explicit project root;
- allowed-path policies;
- child-process environment sanitization;
- secret redaction in logs where possible;
- command audit trail;
- destructive-operation approvals;
- worktree isolation;
- conservative default permissions;
- clear kill controls.

Future defenses may include:

- sandbox profiles;
- container execution;
- network policies;
- filesystem namespaces;
- syscall restrictions;
- secret brokers;
- policy-as-code.

---

## 25. Failure handling

Failures should be categorized.

Examples:

```text
RUNTIME_START_FAILURE
RUNTIME_CRASH
RUNTIME_RATE_LIMIT
RUNTIME_AUTH_FAILURE
TASK_TIMEOUT
BUILD_FAILURE
TEST_FAILURE
REVIEW_REJECTION
LOCK_TIMEOUT
MERGE_CONFLICT
POLICY_VIOLATION
USER_CANCELLED
ORCHESTRATOR_FAILURE
```

Retry policy should depend on failure category.

A deterministic test failure should not be retried blindly with the same unchanged context forever.

The system should support bounded retries and escalation.

---

## 26. Recovery

A local crash should not make the project unknowable.

Persist enough information to recover:

```text
active project state
current plan revision
active tasks
worktree paths
branch names
runtime session identifiers where resumable
process metadata
last known events
pending approvals
```

On startup the orchestrator should reconcile persisted state with reality.

Example:

```text
Database says RUNNING
OS process no longer exists
        ↓
mark execution LOST/FAILED
        ↓
emit reconciliation event
        ↓
retry/reassign according to policy
```

---

## 27. Observability model

Use structured tracing throughout the core.

Useful trace dimensions:

```text
project_id
plan_revision_id
task_id
agent_instance_id
runtime_id
execution_id
correlation_id
worktree_id
```

The same identifiers should be visible in UI logs where practical.

Metrics may include:

```text
running agents
ready tasks
blocked tasks
average task duration
verification failure rate
review rejection rate
runtime availability
retry count
human intervention count
```

---

## 28. Candidate repository structure

A possible future structure:

```text
orchestrix/
├── README.md
├── LICENSE
├── docs/
│   ├── PROJECT_VISION.md
│   ├── ARCHITECTURE.md
│   ├── ROADMAP.md
│   └── adr/
├── apps/
│   └── desktop/
├── crates/
│   ├── orchestrix-domain/
│   ├── orchestrix-core/
│   ├── orchestrix-runtime/
│   ├── orchestrix-runtime-claude/
│   ├── orchestrix-runtime-codex/
│   ├── orchestrix-runtime-antigravity/
│   ├── orchestrix-git/
│   ├── orchestrix-storage/
│   └── orchestrix-observability/
├── packages/
│   └── ui/
└── tests/
    ├── integration/
    └── fixtures/
```

This structure is illustrative, not yet final.

---

## 29. Candidate stack

Current hypothesis:

```text
Tauri
React
TypeScript
Rust
Tokio
SQLite
petgraph
tracing
Git CLI
```

Technology selection should be validated through small technical spikes before permanent commitment.

Recommended early spikes:

1. Spawn and stream one Codex task.
2. Spawn and stream one Claude task.
3. Spawn and stream one Antigravity task.
4. Normalize all three into a common event contract.
5. Create isolated Git worktrees programmatically.
6. Cancel/kill a running agent safely.
7. Recover state after orchestrator restart.

---

## 30. Architectural invariants

Unless changed through an explicit ADR:

- Core scheduling logic is provider-independent.
- Worker agents do not directly mutate the primary branch.
- Deterministic evidence overrides conflicting agent claims.
- Recursive delegation is bounded and centrally authorized.
- Plans are versioned.
- State transitions are auditable.
- Every modifying execution has an isolated workspace by default.
- Dangerous operations can be gated by human approval.
- Subscription-only mode never silently enables usage-billed API fallback.
- System correctness does not depend on hidden chain-of-thought.
- Human operators can stop autonomous execution.

---

## 31. Open architectural decisions

The following decisions require ADRs or technical spikes:

- Rust vs alternative implementation language for the control plane;
- Tauri vs alternative desktop shell;
- state tables + audit log vs strict event sourcing;
- exact runtime adapter contracts;
- process-per-task vs persistent runtime sessions;
- filesystem watching strategy;
- initial sandbox boundary;
- resource lock granularity;
- plan revision approval rules;
- context retrieval/compression strategy;
- integration branch strategy;
- licensing strategy before stable release.

These should remain explicitly open rather than becoming accidental architecture.
