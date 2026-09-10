# ADR-0001 — Single Runtime Is a First-Class Configuration

- **Status:** Accepted
- **Date:** 2026-09-10

## Context

Orchestrix is designed to coordinate coding agents and may support multiple heterogeneous runtimes such as Codex, Claude Code, and Antigravity.

It would be easy for the architecture to implicitly assume that its primary value comes from cross-model delegation and review. That assumption would make the product less useful to developers who only maintain one coding-agent subscription and would unnecessarily couple core orchestration behavior to provider diversity.

However, many important Orchestrix capabilities do not require more than one runtime:

- task decomposition;
- DAG execution;
- specialized Agent Profiles;
- multiple isolated sessions;
- Git Worktrees;
- deterministic verification;
- retries;
- planning and replanning;
- context compilation;
- memory;
- observability;
- documentation;
- human approval;
- quota-aware scheduling.

Separate sessions of the same runtime can still serve different roles such as implementation, review, testing, planning, and debugging.

The main capability lost in a single-runtime topology is strong cross-model diversity. Same-model review may share correlated blind spots, but it can still be useful when performed in an independent session using deterministic evidence.

## Decision

Orchestrix will treat a single connected runtime as a **first-class supported configuration**.

The minimum valid execution topology is:

```text
1 project
1 runtime
1 agent instance
```

The architecture must scale naturally through:

```text
1 runtime / 1 agent
1 runtime / N agents
N runtimes / N agents
```

Core components must not require multiple runtimes to function.

Cross-model review, provider failover, heterogeneous specialization, and cross-runtime routing are optional enhancements unlocked when additional runtimes are connected.

When policy requests independent review and only one runtime exists, Orchestrix should prefer:

```text
implementer session
        ↓
deterministic verification
        ↓
fresh reviewer session
        ↓
correction / approval
```

A task should only become impossible because cross-model review is unavailable when the user has explicitly configured cross-model review as a hard requirement.

## Consequences

### Positive

- Orchestrix remains useful for developers with only one subscription.
- The potential user base is significantly broader.
- Core architecture remains provider-independent.
- Multi-model capabilities become progressive enhancements instead of hidden dependencies.
- Dogfooding can begin before every runtime adapter is implemented.
- Testing the orchestration core becomes simpler because a one-runtime fixture is sufficient for many scenarios.

### Negative

- Review quality may be weaker when implementation and review use the same model family.
- The product must explain the difference between session independence and model diversity.
- Routing logic needs graceful degradation rather than assuming preferred alternatives always exist.
- Some policies require explicit fallback semantics.

## Rejected alternative

### Require at least two runtimes

Rejected because it would confuse Orchestrix's purpose with cross-model review specifically and make orchestration, memory, isolation, planning, and observability unavailable to a large class of users who could benefit from them.

## Related documentation

See [`../ORCHESTRATION_CONTROL_CENTER.md`](../ORCHESTRATION_CONTROL_CENTER.md) for execution modes, routing behavior, same-model review guidance, and the proposed technical policy interface.
