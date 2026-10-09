# Orchestrix Documentation

This directory contains the evolving design and engineering documentation for Orchestrix.

## Core documents

### [Development Status](./DEVELOPMENT_STATUS.md)

Records completed D0 research, its explicit methodology revision, solution journeys, integration recommendations and visual benchmarking. Also tracks the [Desktop study](../prototypes/desktop/README.md) with Studio as default and the [runtime harness](../tools/runtime-harness/README.md) with a bounded Codex subscription probe. D1 pilot and runtime/production gates remain pending.

### [D1 Prototype Delivery](./design/d1-delivery.md)

Records the Studio prototype candidate, onboarding with one connection, four-task Run, versioned implementation/review context, account states and adaptive panels. Includes 19 passing technical scenarios, a reproducible build identifier and current limitations. The [pilot guide](./design/d1-pilot.md), [scenario matrix](./design/d1-scenario-matrix.md) and [visual system](./design/d1-design-system.md) keep human validation and later runtime proofs separate. D1 remains open pending the initial pilot.

### [D0 Research Conclusion](./research/d0-conclusion.md)

Consolidates nine solution families, three interaction references, 14 prioritized decisions and the handoff to D1. Records the replacement of the earlier full-journey criterion, observed/documented/unknown steps, and validation owners. Links [version/license provenance](./research/landscape-and-provenance.md), [Conductor workflow](./research/conductor-dynamic-evidence.md), [Cline/Superset demos](./research/workflow-demonstrations.md), [OpenCode media](./research/opencode-media-observation.md) and [subscription/account UX](./research/subscription-account-ux.md).

### [Runtime Harness Results](./research/runtime-harness-results.md)

Records the local Codex app-server experiment, normalized offline fixtures, supported/unknown capabilities and sanitized live evidence. Confirms a trivial response and interrupted turn via existing ChatGPT authentication; leaves process-tree supervision, real resume, code execution and independent accounts pending. Includes a [version-specific protocol inspection](./research/codex-protocol-spike.md) and a [discovery handoff](./research/discovery-handoff.md).

### [Development Plan](./DEVELOPMENT_PLAN.md)

Starts with D0 product/UX research and D1 design validation, followed by delivery milestones M0–M6. Covers multi-account connections, configurable model/reasoning/context policies, recoverable execution, an adaptive Desktop workspace, provider feasibility, and future VS Code integration. Written in Portuguese for the project owner. Accepted ADRs remain authoritative.

### [Product and UX Research](./PRODUCT_AND_UX_RESEARCH.md)

Records an initial internet survey of comparable products, integration approaches, and interaction references, with official sources. Defines deeper comparative journeys, annotated visual references, design exploration, prototyping, usability evaluation, and experience acceptance criteria. Distinguishes sourced product claims from proposed Orchestrix decisions and future validation.

The consolidated D0 research includes [product positioning](./research/product-positioning.md) across nine families, an [attention/recovery/review benchmark](./research/attention-and-review-benchmark.md), and [workflow/context patterns](./research/workflow-risk-patterns.md). These deliver priorities and observable D1 scenarios while keeping incomplete competitor journeys explicit.

### [Initial Development Backlog](./DEVELOPMENT_BACKLOG.md)

Defines discovery/design tickets OX-D01–OX-D05 and implementation tickets OX-001–OX-017, with dependencies and acceptance criteria. D0 is closed as research under the explicitly consolidated method. D1 and its initial pilot are next, then M0 resumes using the early experiments. Independent tasks may progress in parallel within a milestone.

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

Lists the envisioned subsystem phases. The Development Plan groups them into usable delivery increments, bringing basic policy, context, observability, recovery, and Desktop supervision into the initial workflow.

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
