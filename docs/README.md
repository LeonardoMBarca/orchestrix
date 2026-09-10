# Orchestrix Documentation

This directory contains the evolving design and engineering documentation for Orchestrix.

## Core documents

### [Project Vision](./PROJECT_VISION.md)

Defines the problem, product thesis, goals, non-goals, domain concepts, execution philosophy, orchestration model, context strategy, memory model, open-source direction, and long-term vision.

### [Architecture](./ARCHITECTURE.md)

Describes the initial target architecture, component boundaries, control plane, runtime adapter model, task state machine, scheduler, router, worktree isolation, verification, persistence, recovery, security, and open architectural decisions.

### [Roadmap](./ROADMAP.md)

Breaks the vision into implementation phases and milestones, from runtime feasibility spikes through multi-agent orchestration, adaptive planning, observability, benchmarking, open-source readiness, and eventual distributed execution.

## Architecture Decision Records

Important architectural decisions should be documented in [`adr/`](./adr).

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
