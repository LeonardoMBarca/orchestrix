# ADR-0003 — Desktop App with an Editor-Agnostic Core

- **Status:** Accepted
- **Date:** 2026-09-10

## Context

Orchestrix must supervise coding agents, repositories, Git worktrees, task execution, verification, event streams, context management, retries, and long-running workflows.

Three initial delivery options were considered:

1. VS Code extension as the primary product;
2. a dedicated IDE based on Code - OSS;
3. a standalone Desktop App backed by an editor-agnostic local Core/daemon.

Binding the orchestration lifecycle to an editor would make the system dependent on that editor being open and would couple core state to an integration surface that is not itself the product.

Building a dedicated IDE immediately would create a large amount of unrelated work before validating Orchestrix's core value.

## Decision

Orchestrix will initially be implemented as:

```text
Desktop App
   +
Editor-agnostic Core / local daemon
```

The Core owns canonical orchestration state and execution behavior.

The Desktop App is the first-class client and engineering cockpit.

VS Code, Cursor, CLI, web, or future IDE integrations may become additional clients of the same Core.

The initial Desktop App SHOULD expose project visibility through:

- Project Explorer/file tree;
- syntax-highlighted file viewer;
- diff viewer;
- Git/worktree state;
- task and agent attribution;
- timeline/events;
- test/build/log output;
- actions to open files or folders in an external editor.

The initial application will NOT attempt to provide a complete IDE feature set such as a full debugger, extension ecosystem, production-grade LSP integration, or advanced editor tooling.

## Architectural invariant

> Core orchestration functionality MUST NOT require VS Code, Cursor, or another specific editor.

The UI is a client of orchestration state, not its source of truth.

## Project model

The initial implementation MAY support exactly one Git repository per Orchestrix Project.

The domain model SHOULD nevertheless distinguish Project from Repository so that multi-repository projects can be supported in the future without redefining the fundamental model.

## Consequences

### Positive

- Orchestration can continue independently of editor lifecycle.
- The Core becomes reusable across multiple interfaces.
- VS Code integration can remain thin and focused.
- The team can focus development on orchestration rather than recreating an IDE.
- The Desktop App can expose rich multi-agent execution views that do not map naturally onto an editor extension.
- A future dedicated Orchestrix IDE remains possible.

### Negative

- A standalone desktop shell must be built and distributed.
- Opening/editing code may initially require context switching to an external editor.
- IPC/local protocol boundaries must be designed earlier.

## Alternatives considered

### VS Code extension first

Rejected as the primary architecture because it unnecessarily couples the orchestration runtime to an editor host and makes editor-specific concerns foundational.

A VS Code extension remains desirable as a future thin client.

### Dedicated IDE first

Rejected for initial development because maintaining a full IDE creates substantial complexity unrelated to validating the orchestration engine.

A Code-OSS-based Orchestrix IDE remains possible as future scope if deep editor integration proves strategically important.
