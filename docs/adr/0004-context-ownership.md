# ADR-0004 — Providers Own Conversational Context; Orchestrix Owns Durable Project Context

- Status: Accepted
- Date: 2026-09-10

## Context

Orchestrix integrates coding-agent runtimes that already maintain their own session history, context windows, resume semantics, and in some cases provider-specific compaction or summarization behavior.

At the same time, Orchestrix needs durable knowledge that survives across runtime sessions and providers.

Without an explicit boundary, the system risks either:

1. duplicating provider-specific conversation infrastructure unnecessarily; or
2. depending too heavily on opaque runtime sessions for knowledge that should belong to the project.

## Decision

Orchestrix adopts the following ownership rule:

> Providers own conversational context. Orchestrix owns durable project context.

Provider/runtime responsibilities include, where supported:

- conversation history;
- context-window management;
- runtime-specific compaction/summarization;
- session/thread continuation;
- provider-specific conversational persistence.

Orchestrix responsibilities include:

- tasks and task outcomes;
- plans and revisions;
- architecture decisions;
- project knowledge;
- failure memory;
- execution events;
- reviews and deterministic verification results;
- Git artifacts and references;
- agent/runtime performance observations;
- Context Pack generation.

SQLite is the initial durable store for Orchestrix-owned state.

The Context Compiler is responsible for projecting relevant durable project state into focused context for new or resumed agent sessions.

Orchestrix may choose to terminate an old runtime session and start a fresh one when that produces a cleaner or more independent execution environment.

## Consequences

### Positive

- Runtime sessions become disposable.
- Knowledge can move between Claude, Codex, Antigravity, and future runtimes.
- Orchestrix does not need to reverse-engineer provider compaction behavior.
- Cross-model reviews and retries become easier to isolate.
- Project knowledge survives provider/session failure.
- Context can be purpose-built for each task instead of replaying entire transcripts.

### Negative

- Orchestrix must implement a real project-memory model.
- Context extraction and relevance selection become important engineering problems.
- The system must distinguish transient conversational information from durable project knowledge.
- Context Pack quality directly affects agent performance.

## Alternatives considered

### Persist every conversation turn as the primary memory

Rejected as the core model because raw transcripts are large, provider-specific, noisy, and poorly suited to cross-runtime handoff.

Raw observable events may still be retained for audit/debugging where useful.

### Rely entirely on runtime session persistence

Rejected because project state must survive session expiration, provider changes, reassignment, and cross-model collaboration.

### Reimplement context compaction inside Orchestrix

Rejected as a general strategy. Provider runtimes already own internal conversational context behavior, and Orchestrix should not depend on undocumented internal mechanisms.

Orchestrix may perform project-level summarization and Context Pack construction, but this is distinct from provider-internal conversation compaction.

## Related documentation

- `docs/CONTEXT_OWNERSHIP.md`
- `docs/TECH_STACK.md`
- `docs/PROJECT_VISION.md`
