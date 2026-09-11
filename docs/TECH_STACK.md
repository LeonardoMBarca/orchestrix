# Orchestrix — Proposed Technology Stack

## Status

This document records the current proposed technology stack for Orchestrix. It is a design baseline, not an immutable commitment. Individual choices may be replaced if implementation spikes or runtime constraints reveal a better option.

## Core stack

```text
Desktop shell
→ Tauri 2

Frontend
→ React
→ TypeScript
→ Vite

Core runtime
→ Rust

Async runtime / process orchestration
→ Tokio

Local persistence
→ SQLite

SQLite access
→ SQLx or rusqlite (decision still open)

Task graph / DAG representation
→ petgraph

Git integration
→ Git CLI initially

Agent runtime integration
→ Local subprocesses
→ stdin / stdout / stderr
→ JSON / JSONL / NDJSON / JSON-RPC depending on runtime

Desktop IPC
→ Tauri commands/events initially
```

## Why Tauri 2

Orchestrix is a local-first desktop application whose core already benefits from being implemented in Rust. Tauri provides a natural bridge between a web-based frontend and a native Rust backend without forcing the product to become an Electron application.

The intended architecture is:

```text
React / TypeScript UI
        │
        ▼
      Tauri
        │
        ▼
   Orchestrix Core
        │
        ├── Tokio
        ├── SQLite
        ├── Git
        ├── Codex runtime adapter
        ├── Claude runtime adapter
        └── Antigravity runtime adapter
```

The desktop UI is a client of the Core rather than the owner of orchestration state.

## Why React + TypeScript

The desktop interface is expected to contain UI patterns such as:

- project explorer;
- DAG visualization;
- task inspector;
- agent/runtime panels;
- timeline;
- logs;
- diff viewer;
- policy editor;
- dashboards;
- forms and advanced configuration.

React and TypeScript provide a mature ecosystem for this type of stateful developer tooling UI.

Vite is the proposed frontend build system for the initial implementation.

## Why Rust for the Core

The Orchestrix Core is expected to handle substantial systems-oriented work:

- subprocess lifecycle management;
- asynchronous I/O;
- stdout/stderr streaming;
- structured event parsing;
- cancellation;
- timeouts;
- concurrent agent sessions;
- scheduler state;
- channels and message passing;
- filesystem operations;
- Git worktrees;
- locks;
- persistence;
- recovery;
- local IPC.

Rust provides strong type safety and explicit concurrency semantics for this class of software.

Rust should not be selected merely for portfolio value. Its continued use must be justified by implementation quality, maintainability, runtime reliability, and developer experience.

## Tokio

Tokio is the proposed async runtime for the Core.

It is expected to be used for:

- spawning and supervising child processes;
- asynchronous stdin/stdout/stderr handling;
- channels between orchestration components;
- task cancellation;
- timers and timeouts;
- graceful shutdown;
- concurrent runtime adapters;
- scheduler/executor coordination.

The system should avoid unnecessary async complexity where synchronous code is simpler and more reliable.

## SQLite

SQLite is the proposed local persistence engine.

It should store durable Orchestrix state such as:

```text
Projects
Repositories
Tasks
Task dependencies
Plans and plan revisions
Events
Agent sessions metadata
Runtime observations
Policies
Approvals
Reviews
Metrics
Failures
Context manifests
Architecture decisions references
Execution history
```

SQLite is not intended to become the conversational memory implementation of Claude, Codex, Antigravity, or another external runtime.

Those runtimes own their own internal session history and compaction behavior.

Orchestrix owns durable project-level state.

See `CONTEXT_OWNERSHIP.md` and ADR-0004 for the explicit context ownership model.

## SQLx vs rusqlite

This choice remains open.

### SQLx advantages

- async-friendly API;
- migrations ecosystem;
- compile-time query validation options;
- natural integration with Tokio-heavy code.

### rusqlite advantages

- direct SQLite integration;
- simpler mental model;
- smaller abstraction layer;
- mature and focused API.

The decision should be based on actual persistence access patterns rather than aesthetic preference.

A small technical spike should compare both before the schema becomes large.

## petgraph

`petgraph` is proposed for the in-memory representation and algorithms around the task DAG.

Typical uses may include:

- dependency traversal;
- ready-node discovery;
- cycle detection;
- topological ordering;
- scheduling analysis;
- visualization metadata preparation.

Important invariant:

> petgraph is an in-memory computational representation, not the durable source of truth for task state.

The persistent task graph should remain reconstructable from durable Orchestrix storage.

## Git integration

The initial integration should prefer the installed Git CLI rather than immediately adopting a Rust Git implementation.

Reasons:

- behavior matches developer expectations;
- worktree support is already mature;
- easier debugging;
- commands remain visible and auditable;
- reduces early implementation surface.

Possible future alternatives such as `git2` may be evaluated if CLI process overhead or portability becomes a material issue.

## Runtime communication

External coding-agent runtimes should be normalized through runtime adapters.

Depending on provider/runtime support, adapters may consume:

```text
JSON
JSONL
NDJSON
stream-json
JSON-RPC
plain stdout/stderr where no structured protocol exists
```

The Orchestrix Core should normalize these into internal `AgentEvent`-style domain events rather than leaking provider-specific event formats throughout the application.

## Desktop communication

The initial Desktop client can communicate with the Core through Tauri commands and events.

The internal architecture should nevertheless preserve the possibility of a future long-lived local daemon and multiple clients:

```text
Orchestrix Core / daemon
      │
      ├── Desktop App
      ├── CLI
      ├── VSCode extension
      └── future IDE / remote client
```

The UI must not become the source of truth for orchestration state.

## Code viewer and diff viewer

Orchestrix needs code inspection but does not initially aim to become a full IDE.

A future implementation decision is required between technologies such as:

- Monaco Editor;
- CodeMirror;
- specialized read-only syntax/highlight viewers.

V1 requirements are primarily:

- file viewing;
- syntax highlighting;
- diff viewing;
- navigation;
- task/agent/change metadata.

LSP, debugger, autocomplete, and full editor behavior are intentionally not required initially.

## Frontend state management

No library is fixed yet.

Candidates may include lightweight state management such as Zustand, but the decision should follow actual UI complexity.

Durable state belongs in the Core/SQLite. Frontend state should primarily represent presentation, selection, transient filters, and subscribed runtime state.

## Testing direction

The eventual stack should support at least:

### Rust/Core

- unit tests;
- integration tests;
- process-adapter contract tests;
- state-machine tests;
- scheduler tests;
- persistence migration tests;
- failure/recovery tests.

### Frontend

- component tests;
- orchestration-policy UI tests;
- end-to-end desktop flows where practical.

### Runtime adapters

Each provider adapter should have fixture/replay tests so parser behavior can be validated without consuming a live subscription on every CI run.

## Current confidence

High-confidence choices:

```text
Tauri 2
React
TypeScript
Rust
Tokio
SQLite
Git CLI initially
structured runtime adapters
```

Medium-confidence choices:

```text
petgraph
Vite
```

Open decisions:

```text
SQLx vs rusqlite
frontend state management
code/diff viewer implementation
exact Core↔Desktop transport abstraction
```

The stack should evolve through explicit ADRs when choices become architectural commitments.
