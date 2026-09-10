# Orchestrix — Roadmap

## Purpose

This roadmap translates the project vision into an implementation sequence without pretending that every architectural decision is already final.

The project is intentionally ambitious. The goal is not to minimize scope at all costs, but to build complexity in an order that keeps the system testable and understandable.

The sequence is based on one principle:

> **Establish deterministic execution and observability before adding autonomous intelligence.**

---

## Phase 0 — Architecture foundation

### Objectives

Define the rules of the system before coding the control plane.

### Deliverables

- project vision;
- architecture document;
- initial domain model;
- task state machine;
- initial event vocabulary;
- runtime abstraction proposal;
- security/threat model;
- ADR process;
- initial repository structure;
- development conventions.

### Key ADRs

Potential early ADRs:

```text
ADR-001 Local-first architecture
ADR-002 Control-plane implementation language
ADR-003 Runtime adapter boundary
ADR-004 Centralized delegation
ADR-005 Git worktree isolation
ADR-006 Persistence/event strategy
ADR-007 Subscription-only mode
ADR-008 Desktop technology
```

### Exit criteria

- core concepts have explicit names and boundaries;
- unresolved decisions are documented rather than hidden;
- runtime technical spikes are clearly defined.

---

## Phase 1 — Runtime feasibility spikes

### Objective

Prove that the selected coding-agent runtimes can be reliably controlled programmatically from a local process.

### Experiments

#### Codex spike

Verify:

- authentication through supported local user flow;
- non-interactive task execution;
- structured output/events;
- cancellation;
- exit-code handling;
- working-directory control;
- runtime-limit/error detection.

#### Claude Code spike

Verify the same properties.

#### Antigravity spike

Verify the same properties.

### Deliverable

A small harness for each runtime plus captured sample event streams under `tests/fixtures/runtime-events/`.

### Exit criteria

Each runtime can execute a trivial repository task and produce events that can be normalized without screen automation.

---

## Phase 2 — Runtime abstraction

### Objectives

Build the first provider-independent runtime layer.

### Deliverables

- `AgentRuntime` interface/trait;
- runtime registry;
- normalized `RuntimeEvent` model;
- health checks;
- process supervisor;
- cancellation/termination;
- environment sanitization;
- subscription-only policy hook;
- adapters for initial runtimes.

### Tests

- fake runtime adapter;
- deterministic replay of runtime fixture events;
- crashed process handling;
- timeout handling;
- cancellation handling;
- invalid event handling.

### Exit criteria

The rest of Orchestrix can execute a task without knowing whether the underlying runtime is Claude, Codex, or Antigravity.

---

## Phase 3 — Project and task domain

### Objectives

Create the deterministic core domain.

### Deliverables

- `Project`;
- `Objective`;
- `Plan`;
- `Task`;
- task state machine;
- dependencies;
- capability requirements;
- risk levels;
- execution policy;
- acceptance criteria;
- persistence schema.

### Exit criteria

A manually constructed DAG can be persisted, loaded, validated, and transitioned through legal task states without running an LLM.

---

## Phase 4 — Git execution isolation

### Objectives

Make modifying agents safe enough to run in parallel.

### Deliverables

- repository manager;
- base-commit selection;
- task branch creation;
- Git worktree creation/removal;
- diff collection;
- commit provenance;
- worktree recovery metadata;
- integration staging branch.

### Failure cases to test

- worktree already exists;
- branch already exists;
- dirty repository;
- agent crashes after editing;
- orchestrator restarts with active worktree;
- user manually changes Git state;
- merge conflict.

### Exit criteria

Two agents can modify independent worktrees concurrently without mutating the user's primary working tree.

---

## Phase 5 — Event store and observability foundation

### Objectives

Make every important execution step inspectable.

### Deliverables

- normalized event envelope;
- append-only event persistence;
- correlation and causation IDs;
- structured tracing;
- project execution timeline;
- task event feed;
- agent event feed;
- process event feed.

### Exit criteria

A complete execution can be reconstructed well enough to answer what ran, what changed, what failed, and why the current task state exists.

---

## Phase 6 — Deterministic verification

### Objectives

Stop trusting `"done"` as task completion.

### Deliverables

- evaluator abstraction;
- build evaluator;
- test evaluator;
- lint evaluator;
- type-check evaluator;
- configurable verification pipeline;
- evaluation artifacts;
- task failure/retry integration.

### Exit criteria

A worker implementation cannot be marked complete when configured deterministic checks fail.

---

## Phase 7 — First orchestrated workflow

### Objective

Deliver the first complete multi-agent loop.

### Target workflow

```text
User objective
      ↓
Manually defined task
      ↓
Implementation Agent
      ↓
Deterministic verification
      ↓
Review Agent using different runtime
      ↓
Correction loop if required
      ↓
Final verification
      ↓
Human merge approval
```

### Exit criteria

Orchestrix can coordinate an implementation + cross-model review + correction cycle without the user manually copying prompts between agents.

This is the first major usability milestone.

---

## Phase 8 — Scheduler and parallel DAG execution

### Objectives

Execute multiple independent tasks concurrently.

### Deliverables

- ready-task discovery;
- scheduler;
- project concurrency limits;
- runtime concurrency limits;
- priority;
- retries/backoff;
- blocked task handling;
- runtime-unavailable handling;
- rate-limit-aware pausing when detectable.

### Exit criteria

A DAG containing independent branches runs concurrently and releases dependent tasks only after prerequisites complete.

---

## Phase 9 — Semantic resource locks

### Objectives

Prevent logical conflicts that Git worktrees cannot detect.

### Deliverables

- resource claims;
- read locks;
- write locks;
- scheduler integration;
- lock visualization;
- lock timeout/error handling.

### Initial strategy

Use exact resource names before attempting hierarchical lock semantics.

### Exit criteria

Tasks with conflicting semantic resource claims cannot run concurrently.

---

## Phase 10 — Planner

### Objectives

Convert a high-level goal into structured tasks.

### Deliverables

- planner profile;
- structured plan output schema;
- plan validator;
- DAG cycle detection;
- task normalization;
- human plan review UI;
- plan versioning.

### Important rule

The Planner proposes a plan. Deterministic validation verifies graph/schema correctness before the plan becomes executable.

### Exit criteria

A natural-language objective can produce a valid, reviewable, versioned task graph.

---

## Phase 11 — Router and agent profiles

### Objectives

Assign work based on capabilities rather than hard-coded providers.

### Deliverables

- Agent Profiles;
- capability registry;
- user routing preferences;
- eligible-runtime calculation;
- route scoring;
- route-decision explanation;
- fallback rules.

### Initial scoring

```text
capability compatibility
+
user preference
+
runtime availability
+
permission compatibility
```

### Exit criteria

The same task can be automatically assigned to different runtimes based on profile/capability configuration.

---

## Phase 12 — Context Compiler

### Objectives

Provide agents with the right project context without blindly replaying full history.

### Deliverables

- Context Pack model;
- context source registry;
- relevant file selection;
- relevant ADR selection;
- dependency artifact inclusion;
- failure-memory inclusion;
- project-state summary;
- context-pack provenance;
- context-size budgeting.

### Exit criteria

A new agent session can receive a compact, task-specific package that explains the relevant project state without manual re-contextualization.

---

## Phase 13 — Project memory

### Objectives

Create durable operational knowledge.

### Deliverables

- current project knowledge;
- decision memory;
- failure memory;
- known-issues registry;
- runtime/session references;
- relationship to event history.

### Exit criteria

Validated knowledge discovered in earlier executions can influence later context generation.

---

## Phase 14 — Adaptive planning

### Objectives

Allow the execution plan to evolve safely.

### Deliverables

- plan-revision proposal model;
- triggers;
- diff between plan revisions;
- validation;
- human approval policy;
- affected-task analysis;
- scheduler reconciliation.

### Exit criteria

A discovered dependency or failed assumption can generate a new plan revision without losing execution history or corrupting active work.

---

## Phase 15 — Documentation agent

### Objectives

Continuously synthesize project state into useful documentation.

### Inputs

```text
events
commits
diffs
ADRs
plan revisions
evaluations
decisions
known failures
```

### Outputs

Potential machine-oriented outputs:

```text
.agents/project-state.json
.agents/architecture-summary.md
.agents/current-plan.json
.agents/recent-decisions.md
.agents/known-failures.json
```

Potential human-oriented outputs:

```text
CURRENT_STATE.md
PROJECT_MAP.md
KNOWN_ISSUES.md
CHANGELOG_INTERNAL.md
```

### Exit criteria

A developer or new agent can understand the current state of a non-trivial project without replaying the complete execution timeline.

---

## Phase 16 — Desktop UI

### Objectives

Turn the orchestration engine into an operational control plane rather than a CLI experiment.

### Views

- project overview;
- DAG;
- tasks;
- agent processes;
- runtime availability;
- timeline;
- Git/worktrees;
- evaluations;
- approvals;
- documentation;
- metrics;
- settings.

### Core controls

```text
start
pause
resume
cancel
kill agent
reassign task
override route
inject instruction
approve/reject plan
approve/reject integration
```

### Exit criteria

The majority of normal orchestration operations can be supervised from the UI.

---

## Phase 17 — Security hardening

Security work should happen continuously, but this phase focuses on systematic hardening.

### Deliverables

- formal threat model;
- command policy engine;
- environment sanitization audit;
- secret redaction;
- allowed-path enforcement;
- destructive-operation approvals;
- sandbox proof of concept;
- dependency/supply-chain review;
- recovery testing.

### Exit criteria

The project has documented security assumptions and tested controls for its highest-risk local execution paths.

---

## Phase 18 — Performance registry

### Objectives

Measure how agents actually perform.

### Metrics

```text
first-pass success
iterations
completion time
review rejection rate
regressions
build failures
test failures
human interventions
reverted work
```

### Dimensions

```text
provider
model
runtime
agent profile
capability
task category
risk
project
```

### Exit criteria

Routing decisions can reference real historical statistics rather than anecdotes.

---

## Phase 19 — Adaptive routing

### Objectives

Use performance history as one input to routing.

### Potential scoring

```text
user preference
+
capability fit
+
availability
+
historical performance
+
risk fit
+
context suitability
+
quota state
```

### Exit criteria

The router can explain why one runtime was preferred based partly on observed outcomes.

---

## Phase 20 — Benchmark suite

### Objective

Test the project's central claims.

### Comparisons

```text
single Claude
single Codex
single Antigravity
manual multi-agent
naive automatic multi-agent
Orchestrix
```

### Measures

```text
completion time
first-pass success
regressions
iterations
human interventions
review rejection rate
rework
throughput
runtime usage
```

### Important requirement

Benchmark tasks should be reproducible enough to compare strategies fairly.

### Exit criteria

The README/portfolio can present measured evidence of where Orchestrix helps and where it does not.

---

## Phase 21 — Open-source readiness

### Deliverables

- stable installation instructions;
- contribution guide;
- code of conduct if community requires it;
- issue templates;
- architecture contribution guide;
- adapter contribution guide;
- release process;
- security reporting process;
- confirmed licensing strategy.

### Exit criteria

An external developer can understand, run, modify, and contribute to Orchestrix without private context from the original author.

---

## Phase 22 — Advanced isolation

Potential work:

- Docker execution profiles;
- restricted network access;
- filesystem isolation;
- container lifecycle management;
- sandbox policy profiles;
- potentially microVM support later.

This should be introduced based on actual threat-model requirements rather than novelty.

---

## Phase 23 — Distributed execution

Future scope.

Potential architecture:

```text
                 Control Plane
                 /     |      \
                /      |       \
          Worker A   Worker B   Worker C
           Claude     Codex     Antigravity
```

Required concepts:

- worker registration;
- capability discovery;
- authenticated transport;
- remote process supervision;
- artifact transfer;
- repository synchronization;
- distributed locks;
- network failure handling;
- remote secret management.

This phase intentionally comes late because it converts local concurrency problems into distributed-systems problems.

---

## Milestone summary

### Milestone A — Runtime Harness

```text
Runtime adapters + normalized events + process supervision
```

### Milestone B — Safe Worker

```text
Task + worktree + runtime + deterministic verification
```

### Milestone C — Multi-Agent Loop

```text
Implementation + cross-model review + correction
```

### Milestone D — Parallel Orchestrator

```text
DAG + scheduler + parallel worktrees + resource locks
```

### Milestone E — Intelligent Control Plane

```text
Planner + Router + Context Compiler + adaptive plan
```

### Milestone F — Persistent Engineering Memory

```text
Event history + project knowledge + failures + docs
```

### Milestone G — Operational Product

```text
Desktop UI + observability + approvals + security hardening
```

### Milestone H — Evidence

```text
Benchmarks + performance-aware routing + portfolio results
```

### Milestone I — Ecosystem

```text
Open-source contribution model + adapters + optional distributed workers
```

---

## Immediate next actions

Before major implementation begins:

1. Write the first ADRs.
2. Decide whether Rust/Tauri remains the preferred stack after explicit trade-off analysis.
3. Create runtime feasibility spikes for Codex, Claude Code, and Antigravity.
4. Capture real structured runtime events.
5. Define the normalized runtime event schema from evidence rather than assumptions.
6. Implement a fake runtime before relying on external agents in tests.
7. Build the first deterministic Task state machine.

The first major engineering objective should be:

> **Execute one structured task through one runtime in an isolated Git worktree, capture every event, run deterministic verification, and recover correctly from failure.**

Everything else becomes substantially safer once that foundation is reliable.
