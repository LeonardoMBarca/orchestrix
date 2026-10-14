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
→ rusqlite 0.40.2, bundled SQLite 3.53.2 (ADR-0006)

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

## Platform boundaries and progressive delivery

Windows, macOS and Linux are intended distribution platforms for the open-source Desktop application. Windows is the current development/runtime-validation environment, not a structural domain dependency. [ADR-0005](./adr/0005-desktop-product-scope-and-platform-boundaries.md) clarifies this scope without replacing Rust/Tauri or promising simultaneous launches.

Domain and runtime wire types must not require Windows handles, Job Objects, OS paths, credentials or Tauri types. Process supervision/containment, path resolution, filesystem permissions/watchers/atomic writes, credential storage and external app integration belong in platform infrastructure, hosts or adapters. Each platform must report the guarantees it actually implements; unsupported/unknown containment cannot silently inherit a Windows proof.

**Progressive gates using existing tickets:** OX-003 portable parser/state conformance; OX-007 OS-specific supervisor; OX-012/OX-015 file/editor/UX integration; OX-017 alpha and M5 packaging/update validation. Windows may be delivered first. macOS/Linux builds gain a support claim only after their own evidence; their absence does not prevent the Windows-first increment or require all providers everywhere.

| Intended system | Required validation before claiming support | Current evidence |
| --- | --- | --- |
| Windows | Portable Core conformance; owned process/descendant cancellation/crash/timeout and stream drain; Git/worktree paths with spaces/Unicode; file permissions/concurrent edits; credential host; reproducible install/upgrade/uninstall and desktop keyboard/scale behavior. | Experimental Codex app-server/CLI and native Job tests only; no production Rust Core/Desktop/package validated. |
| macOS | Same contract fixture expectations; owned process-group/descendant cleanup appropriate to the host; case-sensitive/insensitive paths and file permissions; authorized credential storage; editor/shortcuts; reproducible native package and required signing/install/update behavior. | Not yet validated. |
| Linux | Same contract fixture expectations; owned process-group/descendant cleanup appropriate to the host; paths, permissions/symlinks/watchers; authorized credential storage including unavailable-secret-store behavior; desktop integration; reproducible package/install/update for the explicitly supported distributions. | Not yet validated. |

Record tested OS/version/architecture and, for Linux, distribution; runtime/version/auth mode; build/dependencies; commands/results and known limitations. A compiling artifact alone is not runtime integration, filesystem safety or credential-store validation. No signing, credential backend or process-containment implementation is selected by this matrix. Offline CI runs shared conformance; live runtime tests remain separately opt-in per platform. This matrix defines future acceptance, not completed support.

## Why Tauri 2

Orchestrix is a Desktop application whose proposed Core benefits from Rust's process/concurrency guarantees. A production Rust Core is not implemented yet. Tauri provides a natural bridge between a web-based frontend and a native Rust backend without forcing the product to become an Electron application.

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

SQLite is the local persistence engine implemented in the Core's separate persistence module. This does not imply a production daemon/Desktop or automatic runtime recovery.

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

## SQLite access decision

[ADR-0006](./adr/0006-synchronous-sqlite-persistence.md) accepts **rusqlite 0.40.2** with default features disabled and `bundled` enabled; the linked SQLite version verified in the file-backed tests is **3.53.2**. SQLx 0.9.0 was compared through official versioned documentation, without building a second implementation or making comparative performance claims.

The existing synchronous Core uses a small direct SQLite boundary, explicit migrations and short atomic commands. Two connections, rollback and reopening populated files were tested. Domain entities remain independent of SQL/connection types. [Persistence contract](./contracts/persistence.md) defines WAL, FULL synchronization, finite busy waiting, idempotency and validated restoration.

A future Tokio host must move the whole short synchronous operation off executor tasks through an owned worker or bounded blocking operation. Transactions must not cross `.await` or runtime execution. This choice does not add Tokio, a connection pool or a database actor now.

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
      └── future remote client
```

The UI must not become the source of truth for orchestration state.

## Supporting editor and diff viewer

Orchestrix requires a supporting editor and code/diff inspection. Building an IDE is outside the product direction, including as a presumed future evolution. Internal Core execution of agents/Git/authorized checks remains separate from UI editing and execution evidence.

A future implementation decision is required between technologies such as:

- Monaco Editor;
- CodeMirror;
- specialized syntax/highlight viewers for inspection, with a separate lightweight edit surface if chosen.

V1 requirements are primarily:

- file/folder opening and tree navigation;
- lightweight text/code editing;
- syntax highlighting;
- safe Markdown preview;
- diff viewing;
- save/undo, unsaved-change handling and coordination with agent writes;
- navigation;
- task/agent/change metadata.

The [workspace editor contract](./DESKTOP_APP_AND_PROJECT_WORKSPACE.md#7-supporting-editor-and-diff-viewer) defines version checks, conflict handling and stale approval invalidation. Choosing an editor component does not expand scope to a debugger, test explorer, embedded interactive terminal, project-execution environment or IDE extensions. Optional actions may open an external editor/system terminal. Logs/results from Core verification are evidence, not an interactive console.

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
frontend state management
code/diff viewer implementation
exact Core↔Desktop transport abstraction
```

The stack should evolve through explicit ADRs when choices become architectural commitments.
