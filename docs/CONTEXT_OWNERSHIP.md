# Orchestrix — Context Ownership Model

## Principle

Orchestrix separates conversational runtime context from durable project context.

The core rule is:

> Providers own conversational context. Orchestrix owns durable project context.

This separation prevents the Orchestrix Core from reimplementing provider-specific conversation management while still allowing project knowledge to survive across agents, sessions, models, and providers.

## Runtime-owned conversational context

Coding-agent runtimes such as Codex, Claude Code, Antigravity, and future adapters may maintain their own internal session state.

Examples include:

- conversation history;
- tool-call history;
- provider-specific thread/session identifiers;
- internal summarization;
- compaction;
- context-window management;
- provider-specific resume mechanisms.

Orchestrix should use officially exposed session/resume capabilities where available rather than reconstructing every conversation turn itself.

The Core should not assume that every provider exposes the same compaction mechanism or internal storage model.

## Orchestrix-owned durable context

Orchestrix must persist information that belongs to the engineering project rather than to a single conversation.

Examples:

```text
Tasks
Task outcomes
Plans and revisions
Architecture decisions
Important implementation decisions
Known failures
Reviews
Test/build results
Runtime performance observations
Agent assignments
Git commits and diffs
Project structure
Policies
Approvals
Dependency graph
Semantic locks
Execution events
```

This durable state should remain available even if:

- a provider session expires;
- a runtime process crashes;
- an agent is replaced;
- a task moves from Claude to Codex;
- a project is closed and reopened;
- the original conversation can no longer be resumed.

SQLite is the initial persistence layer for this durable Orchestrix state.

## Context Compiler

The Context Compiler bridges durable project context and provider-owned conversational context.

Instead of blindly replaying the complete project history into every new session, it should construct task-specific context packages.

Example:

```text
Task AUTH-051
      │
      ▼
Context Compiler
      │
      ├── objective and acceptance criteria
      ├── relevant architecture decisions
      ├── relevant project files
      ├── output from prerequisite tasks
      ├── relevant known failures
      ├── current Git state
      └── policy / permissions
      │
      ▼
New agent session
```

The goal is not maximal context size.

The goal is maximal useful information per unit of context.

## Session continuation vs fresh session

Orchestrix should support both strategies.

### Continue existing session

Appropriate when:

- conversational continuity is valuable;
- the runtime still has healthy usable context;
- the task remains within the same focused problem;
- provider resume semantics are reliable.

### Start a fresh session

Appropriate when:

- context has become noisy;
- repeated compaction has reduced clarity;
- the role changes substantially;
- a reviewer should be independent from the implementer;
- a retry should avoid carrying previous assumptions;
- the task is reassigned to another runtime.

Before starting a fresh session, Orchestrix should make sure important durable knowledge has been projected into project state and/or the next Context Pack.

## Do not duplicate provider internals unnecessarily

Orchestrix does not need to store a full duplicate of every hidden provider conversation simply to preserve continuity.

It may persist observable/runtime-provided information when useful for audit and debugging, such as:

- normalized agent events;
- assistant-visible summaries;
- tool executions;
- task results;
- session identifiers;
- runtime status;
- explicit decisions and findings.

The durable source of truth should prioritize project-relevant outcomes rather than raw conversational volume.

## Compaction responsibility

Provider runtimes remain responsible for their own internal context-window and compaction mechanisms.

Orchestrix may observe symptoms such as:

- context pressure;
- runtime warnings;
- degraded task performance;
- repeated compaction;
- unusually long sessions;

when such signals are exposed.

It can then decide at the orchestration level to:

- continue the session;
- request a provider-supported compaction action;
- checkpoint project state;
- terminate the old session;
- spawn a fresh session;
- regenerate a focused Context Pack.

Orchestrix should not attempt to imitate undocumented provider-internal compaction logic.

## Memory layers

The intended memory architecture is approximately:

```text
Layer 1 — Provider session memory
Owned by Claude/Codex/Antigravity/etc.
Short/medium-lived conversational context.

Layer 2 — Orchestrix Event Store
Durable record of what occurred.

Layer 3 — Project Knowledge
Consolidated current understanding of the project.

Layer 4 — Failure Memory
Reusable validated lessons from prior failures.

Layer 5 — Context Packs
Ephemeral task-specific projections generated from durable state.
```

This architecture allows sessions to remain disposable while project knowledge remains persistent.

## Example lifecycle

```text
Codex session A
implements AUTH-042
      │
      ▼
Git diff + tests + findings
      │
      ▼
Orchestrix durable state
      │
      ├── task result
      ├── known failure discovered
      ├── decision recorded
      └── relevant events
      │
      ▼
Claude reviewer session B
receives focused Context Pack
      │
      ▼
review finding
      │
      ▼
Orchestrix durable state
      │
      ▼
Codex session C
receives correction-focused Context Pack
```

None of these sessions must share one giant conversational transcript.

## Consequences

### Benefits

- provider independence;
- cleaner fresh-session workflows;
- cross-model handoff;
- less dependence on giant conversation histories;
- persistent project knowledge;
- better auditability;
- easier retry/reassignment;
- lower context pollution;
- future support for local and remote runtimes.

### Costs

Orchestrix must build good mechanisms for:

- knowledge extraction;
- event persistence;
- project-state consolidation;
- Context Pack construction;
- relevance selection;
- freshness tracking;
- conflicting knowledge resolution.

This is intentional. Context engineering is a core Orchestrix responsibility, even though raw conversational compaction is primarily a runtime responsibility.
