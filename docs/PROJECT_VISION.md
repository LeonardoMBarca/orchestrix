# Orchestrix — Project Vision

## 1. Purpose

Orchestrix is a **local-first, multi-model orchestration control plane for AI coding agents**.

Its purpose is to transform multiple coding AIs from isolated interactive tools into a coordinated engineering system capable of planning work, assigning tasks, executing in parallel, reviewing results, adapting plans, preserving context, documenting decisions, and exposing the entire execution lifecycle to the developer.

The project begins as a personal engineering tool and portfolio project, but its architecture should remain open to broader open-source and commercial use.

The central idea is simple:

> The developer should define goals and constraints. Orchestrix should coordinate the agents, deterministic tools, execution state, and verification loops required to move the repository toward those goals.

---

## 2. Problem statement

Modern coding agents are individually powerful, but multi-agent usage is still largely manual.

A developer commonly acts as:

- planner;
- scheduler;
- router;
- reviewer;
- message broker;
- context manager;
- integration manager;
- test operator;
- failure recovery mechanism.

Typical manual workflow:

```text
Developer
   ↓
Claude implements a feature
   ↓
Developer inspects the result
   ↓
Codex reviews it
   ↓
Developer copies the feedback
   ↓
Claude applies corrections
   ↓
Developer runs tests
   ↓
Another model performs a final review
```

This coordination does not scale well when a developer already has access to multiple capable coding-agent runtimes.

Orchestrix intends to automate the coordination layer while keeping deterministic state and human control explicit.

---

## 3. Product thesis

Orchestrix does **not** attempt to build a better foundation model.

It attempts to build a better **system around foundation-model coding agents**.

The core thesis is that a heterogeneous group of coding agents can become significantly more useful when a deterministic control plane provides:

- structured work decomposition;
- capability-based routing;
- parallelism;
- isolation;
- cross-model review;
- validation;
- memory;
- context engineering;
- adaptive replanning;
- observability;
- recovery;
- human override.

Conceptually:

```text
                     USER
                      │
                      ▼
                  OBJECTIVE
                      │
                      ▼
                    PLAN
                      │
                      ▼
                TASK GRAPH / DAG
                      │
                      ▼
             SCHEDULER + ROUTER
              /        |        \
             /         |         \
       Claude        Codex      Antigravity
       Worker        Reviewer    Specialist
             \         |         /
              \        |        /
                  VERIFICATION
                      │
                      ▼
              ADAPTIVE PLANNING
                      │
                      ▼
                  INTEGRATION
```

---

## 4. Primary goals

Orchestrix should eventually allow a developer to:

1. Open or register a local Git repository.
2. Select or configure one or more available coding-agent runtimes.
3. Define a high-level engineering objective.
4. Select or configure a master planning/orchestration strategy.
5. Convert the objective into a structured execution plan.
6. Represent that plan as tasks and dependencies.
7. Execute independent tasks in parallel.
8. Route tasks according to capabilities, preferences, historical results, limits, and risk.
9. Isolate concurrent changes using Git worktrees or stronger sandboxes.
10. Validate agent claims using deterministic tools.
11. Send implementations through appropriate review loops.
12. Reassign or retry failed work.
13. Adapt the plan when discoveries invalidate prior assumptions.
14. Preserve architecture decisions, failures, and execution history.
15. Generate efficient context packs for future agent sessions.
16. Continuously document project state and architectural evolution.
17. Expose all important activity in a real-time UI.
18. Preserve human authority over risky or destructive actions.

---

## 5. Non-goals

At least initially, Orchestrix is **not** intended to:

- replace Git;
- replace test frameworks;
- replace CI/CD systems;
- provide its own foundation model;
- hide provider limitations or bypass provider rate limits;
- share a user's AI account with other users;
- depend on raw hidden chain-of-thought;
- consider an agent's self-reported success as authoritative;
- give arbitrary agents unrestricted recursive control over other agents;
- become a hosted multi-tenant SaaS before the local execution model is safe and mature.

---

## 6. Core principles

### 6.1 Local-first

The initial architecture should execute locally because the system needs close access to:

- repositories;
- Git;
- coding-agent CLIs/runtimes;
- local authentication state;
- shell tools;
- test runners;
- build systems;
- files;
- developer credentials and configuration.

Local-first reduces infrastructure complexity and limits unnecessary exposure of sensitive code and agent credentials.

### 6.2 Subscription-first

The developer may already pay for consumer or professional subscriptions that include access to coding-agent runtimes.

Where a provider officially supports this usage mode, Orchestrix should prefer the user's already-authenticated local runtime rather than requiring separate usage-billed API calls.

A planned **Subscription Only Mode** should ensure that child processes do not silently fall back to API billing.

In that mode Orchestrix should be able to strip or block environment variables such as:

```text
OPENAI_API_KEY
ANTHROPIC_API_KEY
GEMINI_API_KEY
```

when appropriate.

This mode must never attempt to bypass rate limits, account restrictions, or provider terms.

### 6.3 Provider independence

Provider-specific details belong inside runtime adapters.

The orchestration core should operate on normalized concepts such as:

- task;
- session;
- agent event;
- runtime capability;
- runtime availability;
- usage state;
- interruption;
- completion;
- failure.

### 6.4 Central orchestration

Agents may request delegation, review, or additional expertise, but they should not freely create uncontrolled recursive agent hierarchies.

Delegation should flow through a central control plane.

The control plane can evaluate:

- task graph state;
- dependencies;
- resource locks;
- available runtimes;
- configured limits;
- priority;
- risk;
- retry budget;
- user policy.

### 6.5 Deterministic truth

A central invariant of Orchestrix is:

> Use language models for judgment. Use deterministic software for facts.

Examples:

- Process alive? Inspect process state.
- Files modified? Ask Git/filesystem.
- Tests passed? Run tests.
- Build succeeded? Run the build.
- Commit exists? Ask Git.
- Task dependency complete? Read task state.
- Lock available? Ask the lock manager.

Agent statements are observations, not unquestionable truth.

### 6.6 Isolation by default

Parallel tasks should not modify the same working tree by default.

Git worktrees are the initial isolation primitive.

Possible future isolation levels:

- worktree only;
- worktree + restricted process environment;
- Docker container;
- VM/microVM;
- remote isolated worker.

### 6.7 Observable execution

The system should expose enough information to answer:

- What is running?
- Why is it running?
- Which task owns it?
- Which agent was selected?
- What changed?
- Which commands ran?
- Which tests failed?
- Why was the plan modified?
- Why was a task retried?
- What is blocked?
- What requires human approval?

### 6.8 Human authority

Automation must remain interruptible.

The developer should be able to:

- pause a project;
- pause a task;
- cancel a task;
- kill an agent process;
- reassign a task;
- override routing;
- inject new information;
- change priority;
- reject a plan change;
- reject an implementation;
- approve or reject integration;
- require manual approval for sensitive actions.

---

## 7. Domain model

The architecture should distinguish the following concepts.

### Provider

Company or platform behind one or more models/runtimes.

Examples:

```text
OpenAI
Anthropic
Google
```

### Model

Underlying reasoning/coding model when the runtime exposes this information.

### Runtime

Executable integration surface used by Orchestrix.

Examples:

```text
Codex CLI
Claude Code
Antigravity CLI
```

### Agent Profile

A reusable role configuration.

Examples:

```text
Backend Engineer
Frontend Engineer
Security Reviewer
Database Reviewer
Debugger
Planner
Architect
Documentation Agent
Context Compiler
Integration Agent
```

### Agent Instance

A concrete execution/session of an Agent Profile using a Runtime.

### Task

Structured unit of work with objective, dependencies, permissions, capabilities, acceptance criteria, and execution state.

### Artifact

A result produced by a task or agent, such as:

- commit;
- diff;
- patch;
- test report;
- review report;
- documentation;
- plan revision;
- context pack.

### Evaluation

A deterministic or model-assisted check performed against task output.

### Plan

Versioned representation of the intended execution strategy.

### Event

Immutable statement that something relevant occurred in the system.

---

## 8. Structured tasks

Prompts are transport. Tasks are domain objects.

A task should eventually support data similar to:

```json
{
  "id": "AUTH-042",
  "objective": "Implement refresh-token rotation",
  "acceptanceCriteria": [
    "old refresh token becomes invalid",
    "concurrent refresh is safe",
    "all tests pass"
  ],
  "dependencies": ["AUTH-038"],
  "capabilities": ["backend", "security"],
  "risk": "high",
  "allowedPaths": [
    "src/auth/**",
    "tests/auth/**"
  ],
  "requiresReview": true,
  "requiredEvaluations": [
    "unit-tests",
    "security-review"
  ]
}
```

Candidate lifecycle:

```text
CREATED
  ↓
READY
  ↓
ASSIGNED
  ↓
RUNNING
  ↓
NEEDS_REVIEW
  ├───────────────┐
  ↓               │
APPROVED       NEEDS_WORK
  ↓               │
COMPLETED     RUNNING/REASSIGNED
```

Additional states may include:

```text
WAITING
BLOCKED
FAILED
CANCELLED
```

Task state transitions should be explicit and auditable.

---

## 9. Agent profiles and specialization

A profile should describe the engineering role, not merely the provider.

Example:

```yaml
name: Security Reviewer
preferredRuntimes:
  - codex
  - claude
capabilities:
  - security-review
  - backend-review
canModifyFiles: false
canDelegate: true
requiresReview: false
filesystem: read
shell: restricted
```

Another profile:

```yaml
name: Implementation Engineer
preferredRuntimes:
  - claude
  - codex
canModifyFiles: true
canDelegate: true
requiresReview: true
filesystem: read-write
shell: allowed
```

The same runtime can therefore serve different roles.

---

## 10. Routing

Routing should begin with explicit user preferences and simple deterministic heuristics.

Example configuration:

```yaml
frontend:
  preferred: antigravity
backend:
  preferred: claude
debugging:
  preferred: codex
security:
  preferred: codex
architecture:
  preferred: claude
documentation:
  preferred: claude
```

Long-term routing may consider:

```text
user preference
+
required capabilities
+
runtime availability
+
rate-limit/quota state
+
historical performance
+
task complexity
+
risk
+
context requirements
+
retry history
```

The initial implementation should not require machine learning.

---

## 11. Delegation model

Agents should be able to request additional work without directly spawning arbitrary recursive trees of agents.

Example request:

```json
{
  "type": "delegation_request",
  "capability": "database_review",
  "task": "Review migration safety",
  "reason": "Migration affects existing production rows"
}
```

The Orchestrator decides whether the delegation is allowed and which runtime/profile receives it.

This avoids uncontrolled recursion such as:

```text
Claude
 └─ Codex
     └─ Antigravity
         └─ Claude
             └─ ...
```

---

## 12. Worktree isolation

Each independently executed modifying task should normally receive its own Git worktree.

```text
main
 │
 ├── worktrees/AUTH-042-claude
 ├── worktrees/POST-019-codex
 └── worktrees/UI-088-antigravity
```

This provides:

- branch isolation;
- independent diffs;
- easy rollback;
- safer retries;
- parallel experimentation;
- explicit integration boundaries.

The main branch should not be directly mutated by normal worker execution.

---

## 13. Semantic resource locking

Worktrees prevent direct filesystem collisions but not logical conflicts.

Example:

```text
Task A changes database schema.
Task B generates ORM code using the old schema.
```

These tasks may never touch the same physical working tree but are still incompatible in parallel.

Tasks should eventually be able to declare semantic resources:

```yaml
resources:
  write:
    - database.schema.users
  read:
    - auth.contracts
```

A resource lock manager can prevent unsafe concurrent execution.

Potential resources include:

```text
database.schema
API contracts
shared types
auth/session model
configuration
infrastructure
specific modules
```

---

## 14. Planning and DAG execution

A Planner converts a high-level goal into tasks and dependencies.

```text
                 DATABASE
                 /      \
                ↓        ↓
             AUTH       POSTS
               │          │
               ↓          ↓
            SESSION      FEED
                \        /
                 \      /
                   API
                    │
                    ↓
                    UI
                    │
                    ↓
                 E2E TESTS
```

The scheduler should only release tasks when required dependencies are satisfied and required resources are available.

The plan should be versioned.

Example:

```text
PLAN v1
1. schema
2. auth
3. API
4. frontend
5. tests
```

After a discovery:

```text
Current schema does not support multiple identities.
```

The planner may propose:

```text
PLAN v2
1. schema
2. identity migration
3. auth
4. API
5. frontend
6. migration tests
7. integration tests
```

Plan revisions should preserve:

- previous version;
- reason for change;
- triggering evidence/event;
- tasks added/removed/modified;
- human approval when required.

---

## 15. Planner, scheduler, router, executor

These responsibilities should remain conceptually separate.

```text
Planner
  decides WHAT should be done

Scheduler
  decides WHEN work can run

Router
  decides WHO should do it

Executor
  manages HOW the selected runtime is executed

Verifier
  determines WHETHER output satisfies requirements

Orchestrator
  coordinates the entire system
```

An implementation may colocate some components initially, but the conceptual boundaries should remain explicit.

---

## 16. Verification model

Agent completion messages must not automatically complete tasks.

A task can be configured with a verification pipeline:

```text
IMPLEMENTATION
      ↓
BUILD
      ↓
TEST
      ↓
LINT
      ↓
STATIC ANALYSIS
      ↓
DIFF REVIEW
      ↓
ACCEPTANCE CHECK
      ↓
APPROVED
```

High-risk tasks may require additional stages:

```text
Security Review
Database Review
Architecture Review
Adversarial Cross-Model Review
```

The verification engine should combine deterministic evaluators and model-based evaluators without confusing their authority.

---

## 17. Multi-model review hypothesis

One important hypothesis is that different model families may catch different classes of mistakes.

Example:

```text
Claude implementation
        ↓
Codex review
        ↓
Claude correction
        ↓
Antigravity adversarial review
        ↓
Deterministic verification
```

This must eventually be measured rather than treated as a fact.

Possible benchmark questions:

- Does cross-model review reduce regressions?
- Does it reduce human intervention?
- Does it reduce total iterations?
- Does it justify additional runtime consumption?
- Which task categories benefit most?

---

## 18. Runtime abstraction

The orchestration core should not embed provider-specific behavior throughout the system.

Conceptual interface:

```ts
interface AgentRuntime {
  id: string;
  capabilities: Capability[];

  start(config: SessionConfig): Promise<Session>;

  send(
    session: Session,
    task: AgentTask
  ): AsyncIterable<AgentEvent>;

  interrupt(session: Session): Promise<void>;
  resume(sessionId: string): Promise<Session>;
  terminate(session: Session): Promise<void>;
  getUsage(): Promise<UsageState>;
  healthCheck(): Promise<HealthState>;
}
```

Initial implementations may include:

```text
ClaudeRuntime
CodexRuntime
AntigravityRuntime
```

Future adapters might include:

```text
Gemini CLI
OpenCode
Aider
Copilot CLI
Kiro
Cursor Agent
Ollama
llama.cpp
other local models
```

---

## 19. Runtime communication

Runtime adapters should prefer structured machine-readable execution surfaces whenever officially available.

Possible transports/formats:

```text
stdin/stdout
JSON
JSONL
NDJSON
stream-json
JSON-RPC
```

Adapters may normalize events such as:

```text
session.started
message.received
tool.requested
tool.completed
command.started
command.completed
runtime.warning
runtime.rate_limited
runtime.completed
runtime.failed
```

Runtime streams provide observability but should not become the sole source of truth.

---

## 20. Source of truth

Orchestrix should prefer objective system state from:

```text
Git
filesystem
OS process state
task database
test output
build output
event store
orchestrator state
```

An agent claiming that it modified three files does not replace `git diff`.

An agent claiming tests passed does not replace running the tests.

---

## 21. Reasoning and structured rationale

Orchestrix should not depend on hidden raw chain-of-thought.

Instead, agent contracts should request explicit useful reasoning artifacts such as:

```text
decisions
assumptions
hypotheses
risks
blockers
observations
delegation requests
concise rationale
```

Example:

```json
{
  "summary": "Implemented refresh-token rotation",
  "decisions": [
    {
      "decision": "Use optimistic locking",
      "reason": "Avoid duplicate concurrent refresh"
    }
  ],
  "assumptions": [],
  "risks": [],
  "blockers": [],
  "delegationRequests": []
}
```

---

## 22. Event sourcing and execution history

Every meaningful transition should emit a normalized event.

Example:

```json
{
  "event": "task.completed",
  "taskId": "AUTH-014",
  "agent": "claude",
  "baseCommit": "123abc",
  "resultCommit": "456def",
  "filesChanged": [
    "src/auth/session.ts",
    "tests/auth/session.test.ts"
  ],
  "tests": {
    "passed": 93,
    "failed": 0
  }
}
```

Candidate event families:

```text
project.*
plan.*
task.*
agent.*
runtime.*
process.*
git.*
file.*
test.*
build.*
review.*
integration.*
approval.*
context.*
documentation.*
```

The event history should support:

- auditability;
- real-time UI updates;
- debugging;
- analytics;
- documentation generation;
- state reconstruction;
- future replay/time-travel capabilities.

---

## 23. Memory architecture

Orchestrix should maintain multiple forms of memory rather than one enormous conversation transcript.

### Event Store

Immutable execution history.

### Project Knowledge

Consolidated current state:

```text
architecture
modules
interfaces
database schema
important files
decisions
known issues
conventions
dependencies
```

### Context Packs

Task-specific context generated for a particular agent execution.

### Failure Memory

Validated prior failures and proven solutions.

### Performance Memory

Historical task outcomes associated with runtimes and capabilities.

---

## 24. Context Compiler

One of Orchestrix's major components should be the **Context Compiler**.

Its goal is not to maximize context size.

Its goal is to maximize **relevant information density**.

For task `AUTH-042`, a context pack may contain:

```text
current project goal
task objective
acceptance criteria
relevant architecture summary
related ADRs
relevant source files
outputs from dependency tasks
known auth failures
recent related decisions
constraints and permissions
```

Instead of sending every historical conversation to every agent, Orchestrix should select or summarize only relevant state.

---

## 25. Failure Memory

A validated failure record may resemble:

```json
{
  "pattern": "required column added to populated table",
  "context": "database migration",
  "failure": "migration failed for existing rows",
  "solution": "use a two-stage nullable migration",
  "validated": true
}
```

A future context pack can surface this before a new agent repeats the same class of mistake.

The system therefore accumulates operational experience without requiring model fine-tuning.

---

## 26. Documentation system

Documentation should serve both humans and agents.

Human-oriented documentation may include:

```text
README.md
docs/ARCHITECTURE.md
docs/PROJECT_VISION.md
docs/ROADMAP.md
docs/adr/
```

Agent-oriented state may eventually include:

```text
.agents/project-state.json
.agents/architecture-summary.md
.agents/recent-decisions.md
.agents/current-plan.json
.agents/dependency-graph.json
.agents/known-failures.json
```

A Documentation Agent may continuously or periodically synthesize:

- events;
- commits;
- diffs;
- decisions;
- test results;
- plan revisions;
- architectural changes.

The output should not merely be verbose logs. It should be useful for future re-contextualization.

---

## 27. Architecture Decision Records

Major technical decisions should be documented through ADRs.

Potential early ADRs:

```text
ADR-001 — Local-First Architecture
ADR-002 — Runtime Adapter Boundary
ADR-003 — Centralized Delegation
ADR-004 — Git Worktrees for Agent Isolation
ADR-005 — Event Store as Execution Audit Log
ADR-006 — Deterministic State as Source of Truth
ADR-007 — Subscription-Only Execution Mode
ADR-008 — Context Compiler
```

Each ADR should contain:

```text
Status
Date
Context
Decision
Alternatives
Consequences
```

---

## 28. Adaptive planning

Execution discoveries should be able to modify future work.

A Plan Adapter/Planner may react to:

- failed tests;
- discovered architectural constraints;
- unexpected dependencies;
- review findings;
- security findings;
- runtime failure;
- new user requirements;
- integration conflicts.

Plan adaptation must remain controlled.

Not every agent observation should rewrite the entire DAG.

Changes should have explicit rationale and, when risk warrants it, human approval.

---

## 29. Observability

The UI should eventually provide several perspectives on the same execution state.

### Project overview

High-level project goal, plan version, progress, active agents, blockers, and recent failures.

### DAG view

Graph of tasks and dependencies with states such as:

```text
WAITING
READY
RUNNING
REVIEW
FAILED
COMPLETED
```

### Task inspector

For a selected task:

```text
objective
acceptance criteria
assigned runtime/profile
worktree
dependencies
dependents
context pack
events
logs
commands
diff
commits
tests
review results
retries
decisions
```

### Agent view

Agent/session status, assigned task, runtime, recent events, process state, and availability.

### Timeline

Example:

```text
19:31:04  MASTER
           Created PLAN v13

19:31:09  CLAUDE-01
           Started AUTH-42

19:31:11  CODEX-02
           Started DB-18 review

19:31:32  CLAUDE-01
           modified src/auth/session.ts

19:31:51  CLAUDE-01
           npm test

19:32:08  TEST
           91 PASS / 2 FAIL

19:32:09  ORCHESTRATOR
           AUTH-42 → NEEDS_WORK

19:33:17  CODEX-03
           probable race condition discovered

19:33:21  PLANNER
           PLAN v13 → v14

19:34:02  TEST
           93 PASS

19:34:04  REVIEWER
           APPROVED

19:34:05  AUTH-42
           COMPLETE
```

---

## 30. Human control and safety boundaries

The system must make important automation reversible and inspectable.

Potential human controls:

```text
pause project
resume project
pause task
cancel task
kill agent
reassign task
override route
change priority
inject instruction
approve plan
reject plan
approve integration
reject integration
inspect commands
```

Sensitive actions may require explicit approval, including:

- deleting large file sets;
- destructive Git operations;
- force push;
- database drops;
- production infrastructure changes;
- credential access;
- dangerous shell commands;
- changes outside allowed paths.

---

## 31. Performance registry

Orchestrix should eventually learn from outcomes.

Potential dimensions:

```text
provider
model
runtime
agent profile
capability
task category
project
risk level
```

Potential metrics:

```text
task count
first-pass success
average iterations
completion time
human interventions
review rejection rate
regressions introduced
failed tests introduced
build failures
reverted changes
```

Example:

```text
Claude / backend
  tasks: 132
  first-pass success: 83%
  average iterations: 1.31

Codex / backend
  first-pass success: 89%

Antigravity / frontend
  first-pass success: 91%
```

These metrics may later influence routing.

---

## 32. Benchmarking strategy

Because Orchestrix is also a portfolio and research-style engineering project, its claims should be tested.

Candidate comparisons:

```text
single Claude
single Codex
single Antigravity
manual multi-agent workflow
naive automated multi-agent workflow
Orchestrix
```

Candidate measurements:

```text
completion time
first-pass success
accepted tests
regressions
iterations
human interventions
review rejection rate
rework
parallel throughput
runtime consumption
```

The objective is not necessarily to prove Orchestrix wins on every metric.

The objective is to understand when orchestration adds value and what it costs.

---

## 33. Candidate technology stack

Current hypothesis:

```text
Desktop        Tauri + React + TypeScript
Core           Rust
Async runtime  Tokio
Persistence    SQLite
Graph          petgraph or equivalent
Validation     JSON Schema / Zod where appropriate
Observability  tracing
Isolation      Git worktrees
Optional       Docker/VM sandboxing later
```

Runtime communication will depend on adapter capabilities:

```text
stdin/stdout
JSON
JSONL/NDJSON
stream-json
JSON-RPC
```

The stack is not considered final until documented through explicit architectural decisions.

---

## 34. Why Rust is being considered

Rust is a candidate for the core because Orchestrix is expected to contain significant amounts of:

- subprocess supervision;
- async I/O;
- streaming;
- concurrency;
- shared state;
- cancellation;
- filesystem coordination;
- scheduling;
- event processing;
- process lifecycle management.

Rust also provides strong compile-time guarantees valuable for a system coordinating potentially destructive autonomous processes.

However, Rust must be selected because its engineering benefits justify the development cost, not merely because it looks impressive in a portfolio.

---

## 35. Open-source direction

The project is intended to be openly developed.

The repository currently uses the MIT License, but licensing strategy may be reconsidered before a stable release if needed.

Possible future monetization should not depend on pretending the underlying AI access is provided by Orchestrix.

Users should retain their own relationships with providers.

Potential value-added commercial directions could include:

- signed desktop binaries;
- automatic updates;
- hosted synchronization;
- remote workers;
- team collaboration;
- enterprise policy controls;
- managed services;
- commercial support.

Any commercial design must remain compatible with provider terms and the project's chosen open-source license.

---

## 36. Long-term possibilities

Potential extensions include:

```text
distributed workers
multiple machines
remote execution
Docker sandboxes
VM/microVM isolation
multi-repository task graphs
cross-project memory
semantic memory
local models
quota-aware scheduling
adaptive routing
agent capability discovery
execution replay
time-travel debugging
execution snapshots
chaos testing
team collaboration
plugin ecosystem
```

An eventual distributed model could resemble:

```text
                 Control Plane
                 /     |      \
                /      |       \
          Machine A Machine B Machine C
           Claude     Codex    Antigravity
```

This is intentionally future scope, not a requirement for the first usable release.

---

## 37. Key invariants

The following invariants should guide implementation unless deliberately changed through an ADR:

1. **Normal worker agents do not directly mutate the primary branch.**
2. **Agent self-reported completion is insufficient to complete a task.**
3. **Deterministic state wins over agent claims when deterministic evidence exists.**
4. **Unbounded recursive agent delegation is forbidden.**
5. **Every state-changing task has an identifiable owner and execution context.**
6. **Important task transitions are auditable.**
7. **Parallel modifying tasks are isolated by default.**
8. **Sensitive/destructive operations can require human approval.**
9. **The runtime/provider implementation is abstracted away from core scheduling logic.**
10. **Subscription-only mode must never silently enable usage-billed API fallback.**
11. **Hidden chain-of-thought is not required for system correctness.**
12. **Plans are versioned rather than silently overwritten.**
13. **Human operators retain the ability to stop autonomous execution.**

---

## 38. Engineering learning objective

Orchestrix is intentionally designed to be educational as well as useful.

Building the system should create opportunities to learn and apply:

```text
processes
threads
async I/O
IPC
concurrency
scheduling
state machines
event sourcing
Git internals
resource locking
fault tolerance
observability
testing
security
message passing
DAG execution
software architecture
LLM orchestration
```

The project should not avoid legitimate complexity solely because AI agents can write the implementation.

At the same time, every complex mechanism should answer a real engineering problem.

The guiding distinction is:

> **Useful complexity versus accidental complexity.**

---

## 39. Definition of success

Orchestrix is successful if it can eventually take a meaningful software-engineering objective and reduce the amount of manual coordination required from a developer while preserving or improving reliability, visibility, recoverability, and control.

A mature execution should look less like manually chatting with multiple agents and more like supervising an engineering system:

```text
Human defines objective
        ↓
Orchestrix plans and schedules work
        ↓
Multiple agents execute specialized tasks
        ↓
Deterministic tools validate reality
        ↓
Agents review one another where useful
        ↓
Failures cause controlled retries/replanning
        ↓
Human intervenes only where valuable
        ↓
Integrated result + complete execution history
```

That is the long-term vision of Orchestrix.
