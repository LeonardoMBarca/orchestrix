# Orchestrix

> **Make your coding agents work as a team.**

Orchestrix is a **local-first, multi-model orchestration control plane for AI coding agents**.

Instead of manually moving tasks, reviews, context, and feedback between tools such as Codex, Claude Code, and Antigravity, Orchestrix aims to coordinate them as specialized workers inside a single observable and controllable engineering system.

The project is designed first as a personal engineering tool and portfolio project, with an open-source direction from the beginning.

> **Status:** early architecture and design phase. No stable release exists yet.

## Why Orchestrix?

Modern coding agents are individually powerful, but using several of them together still requires a human to act as the scheduler, reviewer, message broker, context manager, and integration layer.

A typical workflow today looks like this:

```text
Developer
   ↓
Claude implements
   ↓
Developer inspects
   ↓
Codex reviews
   ↓
Developer copies feedback
   ↓
Claude fixes
   ↓
Developer verifies
```

Orchestrix aims to turn that manual coordination into an engineering runtime:

```text
Developer
   ↓
Orchestrator
   ↓
Task Graph / DAG
   ↓
Scheduler + Router
   ↓
┌────────────┬────────────┬──────────────┐
│ Claude     │ Codex      │ Antigravity  │
│ worker     │ reviewer   │ specialist   │
└────────────┴────────────┴──────────────┘
   ↓
Deterministic verification
   ↓
Adaptive plan + integration
```

## Core idea

Orchestrix treats coding AIs as a **heterogeneous pool of engineering workers** that can be:

- scheduled;
- routed by capability;
- isolated;
- observed;
- reviewed by other models;
- retried or reassigned;
- provided with task-specific context;
- measured over time;
- coordinated through a central control plane.

The fundamental unit is not a loose prompt. It is a **structured task** with explicit objectives, dependencies, acceptance criteria, permissions, risk level, required capabilities, and validation rules.

## Design principles

### Local-first

Execution should happen primarily on the developer's machine, close to repositories, Git state, local tools, and locally authenticated coding-agent runtimes.

### Subscription-first

Where officially supported, Orchestrix is intended to use local coding-agent runtimes authenticated through the user's own subscriptions instead of silently falling back to usage-billed APIs.

A future **Subscription Only Mode** should explicitly prevent accidental API-key fallback.

### Centralized orchestration

Agents should not recursively control one another without limits. Delegation requests go through a central orchestrator that understands task state, dependencies, locks, availability, limits, policies, and risk.

### Deterministic truth

LLMs provide judgment. Deterministic software provides facts.

Git, the filesystem, process state, test runners, build results, task state, and the event store should be the source of truth whenever possible.

### Isolation by default

Parallel agents should operate in isolated Git worktrees or equivalent sandboxes rather than editing the same working directory.

### Observable execution

Every meaningful transition should be inspectable: task lifecycle, agent activity, file changes, commands, tests, reviews, retries, plan revisions, and integration decisions.

### Human override

Autonomy must not remove control. The user should always be able to pause, cancel, reassign, inspect, override, approve, or reject important operations.

## Planned architecture

```text
┌─────────────────────────────────────────────┐
│                 Desktop UI                  │
│                                             │
│ DAG | Tasks | Agents | Timeline | Git       │
│ Logs | Docs | Metrics | Configuration       │
└─────────────────────┬───────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────┐
│              Orchestrator Core              │
│                                             │
│ Planner                                     │
│ Scheduler                                   │
│ Router                                      │
│ Policy Engine                               │
│ Context Compiler                            │
│ Memory                                      │
│ Event Store                                 │
│ Git Manager                                 │
│ Resource Lock Manager                       │
│ Evaluator                                   │
│ Integration Manager                         │
└─────────────┬───────────────────────────────┘
              │
      ┌───────┴───────────────────────┐
      ▼                               ▼
Runtime Adapters               Deterministic Tools
      │                               │
┌─────┼──────────┐             Git / tests / build
▼     ▼          ▼             lint / filesystem
Claude Codex  Antigravity      process supervision
```

## Planned runtime model

The orchestration core should depend on a common runtime abstraction rather than on provider-specific logic.

Potential runtimes include:

- Claude Code;
- Codex CLI / Codex runtime interfaces;
- Google Antigravity;
- future local or remote coding agents.

The goal is to support additional adapters without rewriting the scheduler or task system.

## Planned subsystems

- **Runtime Layer** — process lifecycle, structured streams, sessions, authentication, cancellation, health checks.
- **Execution Layer** — tasks, worktrees, commits, artifacts, shell execution, isolation.
- **Orchestration Layer** — DAG scheduling, dependencies, priorities, retries, delegation, resource locks.
- **Intelligence Layer** — planning, routing, critique, adaptive replanning, context compilation.
- **Verification Layer** — build, tests, lint, static analysis, diff review, security review, acceptance checks.
- **Memory Layer** — event history, project knowledge, decisions, failure memory, task-specific context packs.
- **Observability Layer** — timeline, traces, logs, agent state, task state, performance metrics.
- **Human Control Layer** — approvals, overrides, pause, kill, reassignment, merge control.
- **Learning Layer** — historical performance by model, runtime, task type, and capability.
- **Documentation Layer** — human-facing docs, ADRs, machine-oriented project state, re-contextualization material.

## A task-oriented system

A future task may look conceptually like this:

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
  "allowedPaths": ["src/auth/**", "tests/auth/**"],
  "requiresReview": true,
  "requiredEvaluations": ["unit-tests", "security-review"]
}
```

## Multi-model review

One of the project's hypotheses is that model diversity can improve engineering reliability.

For example:

```text
Claude implementation
        ↓
Codex review
        ↓
Claude correction
        ↓
Antigravity adversarial review
        ↓
Deterministic validation
```

This should eventually be benchmarked rather than assumed.

## Context engineering

Orchestrix should not blindly dump an entire project history into every new agent session.

A planned **Context Compiler** will assemble task-specific context from architecture, relevant files, dependent task outputs, project decisions, known failures, current plans, and acceptance criteria.

The system should also maintain machine-oriented project state so new agent sessions can be re-contextualized efficiently.

## Event-driven execution

Execution is planned around an append-only event history describing changes such as:

```text
plan.created
plan.updated
task.created
task.assigned
task.started
agent.spawned
file.modified
test.started
test.completed
review.started
review.passed
task.completed
agent.rate_limited
integration.completed
```

This history should support auditability, timeline visualization, debugging, analytics, documentation generation, and eventually replay or state reconstruction.

## Failure memory

Orchestrix should remember validated engineering failures and their solutions. A future context pack may warn an agent about a previously observed migration failure, race condition, architectural incompatibility, or other relevant problem before it repeats the same mistake.

## Performance-aware routing

The initial router may use explicit user preferences. Over time, Orchestrix should be able to evaluate historical performance using signals such as:

- first-pass success rate;
- iterations required;
- regressions introduced;
- reviewer rejection rate;
- completion time;
- human interventions;
- build and test outcomes;
- task category and risk.

This can enable evidence-based routing without requiring machine learning in the first versions.

## Candidate technology stack

The current architectural hypothesis is:

- **Desktop:** Tauri + React + TypeScript
- **Core:** Rust
- **Async runtime:** Tokio
- **Persistence:** SQLite
- **Graph:** petgraph or equivalent
- **Validation:** JSON Schema / Zod where appropriate
- **Observability:** tracing
- **Isolation:** Git worktrees, with optional stronger sandboxing later
- **Runtime communication:** stdin/stdout, JSON, JSONL/NDJSON, or JSON-RPC depending on the adapter

These are hypotheses, not irreversible commitments. Technology choices should be justified by correctness, maintainability, observability, recoverability, testability, and development velocity.

## Long-term possibilities

Potential future directions include:

- distributed workers;
- remote executors;
- multiple machines;
- Docker or VM sandboxing;
- cross-project memory;
- local-model runtimes;
- quota-aware scheduling;
- adaptive routing;
- execution replay;
- time-travel debugging;
- execution snapshots;
- chaos testing;
- team collaboration;
- hosted synchronization and remote control-plane services.

## Project philosophy

Orchestrix is intentionally ambitious.

The goal is not to add complexity for its own sake, but to explore what a serious engineering control plane for AI coding agents could look like when agents are treated as workers inside a deterministic, observable, recoverable system.

A guiding distinction throughout the project will be:

> **Useful complexity vs. accidental complexity.**

## Documentation

Detailed project vision, architecture, domain model, execution model, invariants, and roadmap will live under [`docs/`](./docs) as the design evolves.

## Contributing

Contribution guidelines will be added once the initial architecture and repository structure stabilize.

## License

This project is currently licensed under the MIT License. The licensing strategy may be revisited before the first stable release if required by the project's long-term open-source and commercial direction.
