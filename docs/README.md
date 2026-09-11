# Orchestrix Documentation

This directory contains the evolving design and engineering documentation for Orchestrix.

## Core documents

### [Project Vision](./PROJECT_VISION.md)

Defines the problem, product thesis, goals, non-goals, domain concepts, execution philosophy, orchestration model, context strategy, memory model, open-source direction, and long-term vision.

### [Architecture](./ARCHITECTURE.md)

Describes the initial target architecture, component boundaries, control plane, runtime adapter model, task state machine, scheduler, router, worktree isolation, verification, persistence, recovery, security, and open architectural decisions.

### [Proposed Technology Stack](./TECH_STACK.md)

Records the current baseline stack: Tauri 2, React, TypeScript, Vite, Rust, Tokio, SQLite, petgraph, Git CLI, and structured local runtime adapters. It also identifies open choices such as SQLx vs rusqlite, frontend state management, and the code/diff viewer.

### [Context Ownership Model](./CONTEXT_OWNERSHIP.md)

Defines the boundary between provider-owned conversational context and Orchestrix-owned durable project context. Provider runtimes manage their own conversation history, context windows, resume semantics, and compaction, while Orchestrix persists tasks, plans, decisions, failures, reviews, events, project knowledge, and generates focused Context Packs.

The governing rule is:

> Providers own conversational context. Orchestrix owns durable project context.

### [Orchestration Control Center](./ORCHESTRATION_CONTROL_CENTER.md)

Defines single-runtime support as a first-class product requirement and specifies the proposed technical control panel for execution strategy, runtime pools, Agent Profiles, task routing, concurrency, review policy, delegation, retries, resource locks, context, quotas, integration, human approvals, and live policy changes.

It also defines the conceptual execution modes:

```text
Single Runtime
Multi Runtime
Hybrid
```

and the requirement that Orchestrix remain valuable even when the user has only one coding-agent subscription.

An illustrative policy document is available at [`examples/orchestration-policy.example.yaml`](./examples/orchestration-policy.example.yaml). It demonstrates how the technical UI can map to explicit, versionable orchestration policy rather than hidden UI state.

### [Model and Reasoning Policy](./REASONING_AND_MODEL_POLICY.md)

Defines model selection and reasoning/thinking effort as first-class orchestration controls. It specifies normalized reasoning intent, runtime capability discovery, provider-specific translation, per-role/per-risk/per-task overrides, requested-vs-effective configuration, reasoning-aware routing, observability, and the proposed `Nuclear` quality preset.

### [Desktop App and Project Workspace](./DESKTOP_APP_AND_PROJECT_WORKSPACE.md)

Defines the initial product shell as a standalone Desktop App backed by an editor-agnostic local Core/daemon. It specifies project creation/opening flows, Project Explorer behavior, file and diff visibility, execution-aware file metadata, external-editor integration, the distinction between Project and Repository, and why Orchestrix should not become a full IDE in its first version.

### [Roadmap](./ROADMAP.md)

Breaks the vision into implementation phases and milestones, from runtime feasibility spikes through multi-agent orchestration, adaptive planning, observability, benchmarking, open-source readiness, and eventual distributed execution.

## Architecture Decision Records

Important architectural decisions should be documented in [`adr/`](./adr).

Current ADRs:

- [`ADR-0001 — Single Runtime Is a First-Class Configuration`](./adr/0001-single-runtime-first-class.md)
- [`ADR-0002 — Model and Reasoning Controls Are First-Class Policy`](./adr/0002-model-and-reasoning-policy.md)
- [`ADR-0003 — Desktop App with an Editor-Agnostic Core`](./adr/0003-desktop-app-editor-agnostic-core.md)
- [`ADR-0004 — Providers Own Conversational Context; Orchestrix Owns Durable Project Context`](./adr/0004-context-ownership.md)

ADRs should be used when a decision materially affects one or more of:

- architecture;
- security;
- persistence;
- execution semantics;
- runtime/provider integration;
- concurrency;
- isolation;
- compatibility;
- deployment;
- licensing;
- long-term maintainability.

Start from [`adr/0000-template.md`](./adr/0000-template.md).

## Documentation principles

Orchestrix documentation should distinguish between:

```text
FACT
A behavior verified in implementation or provider/runtime behavior.

DECISION
A deliberate architecture choice recorded through an ADR.

HYPOTHESIS
A design idea that still requires technical validation.

FUTURE SCOPE
A direction intentionally excluded from the current implementation phase.
```

This distinction matters because Orchestrix integrates fast-changing external coding-agent runtimes. Documentation must not accidentally present an unverified runtime behavior or early design assumption as a stable invariant.

## Planned machine-oriented documentation

As the project evolves, Orchestrix may maintain a separate machine-oriented state tree for agent re-contextualization:

```text
.agents/
├── project-state.json
├── architecture-summary.md
├── recent-decisions.md
├── current-plan.json
├── dependency-graph.json
└── known-failures.json
```

These files are not intended to replace human documentation. They are compact, structured inputs for Orchestrix's future Context Compiler and Documentation Agent.
