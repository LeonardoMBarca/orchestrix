# Orchestrix — Desktop App and Project Workspace

## 1. Decision summary

Orchestrix should initially be delivered as:

```text
Desktop App
   +
Editor-agnostic Core / Local Daemon
```

It should not begin life as a VS Code extension and should not attempt to become a full IDE in the first version.

The Desktop App is the engineering cockpit. The Core is the product engine.

A future VS Code extension, CLI, web client, or even a dedicated Code-OSS-based IDE may connect to the same Core without changing the orchestration model.

---

## 2. Why Desktop + Core

Orchestrix needs to manage work that should not depend on a text editor being open:

- coding-agent processes;
- task execution;
- Git worktrees;
- repositories;
- event streams;
- scheduling;
- context compilation;
- verification pipelines;
- rate-limit state;
- retries;
- documentation;
- long-running multi-agent workflows.

Therefore the lifecycle of orchestration must not be coupled to VS Code or another editor.

The Core should remain available as a local process/service while one or more clients connect to it.

Conceptually:

```text
                   ORCHESTRIX CORE
                  local daemon/runtime
                         │
       ┌─────────────────┼─────────────────┐
       │                 │                 │
     Codex             Claude         Antigravity
       │
       ├── Scheduler / Router
       ├── Git / Worktrees
       ├── Event Store
       ├── Verification
       ├── Context / Memory
       └── Runtime adapters
                         │
                Local protocol / IPC
             ┌───────────┼───────────┐
             ▼           ▼           ▼
         Desktop App   CLI      VS Code Extension
                                (future)
```

---

## 3. The Desktop App is not initially a full IDE

The first Desktop App should provide enough code awareness to understand and supervise the project without reimplementing a complete development environment.

V1 should include:

- project/repository explorer;
- file tree;
- read-only or lightweight code viewer;
- syntax highlighting;
- diff viewer;
- Git state;
- task-to-file attribution;
- agent-to-file attribution;
- timeline/events;
- test/build output;
- terminal/log views where useful;
- links/actions to open files in an external editor.

V1 does not need to include:

- full LSP integration;
- production-grade autocomplete;
- debugger implementation;
- extension ecosystem;
- full IDE keybinding compatibility;
- advanced refactoring tooling;
- integrated language-specific development features already provided by existing IDEs.

This avoids spending the first development cycle recreating VS Code instead of building the orchestration system.

---

## 4. Workspace entry flows

The application should support two primary ways to begin work.

### 4.1 Create New Project

Example welcome flow:

```text
Welcome to Orchestrix

[ Create New Project ]
[ Open Existing Project ]
[ Recent Projects ]
```

Creating a project may eventually allow:

- choosing a target directory;
- initializing Git;
- creating a minimal project structure;
- selecting a basic project template;
- enabling Orchestrix metadata/configuration;
- connecting runtimes;
- creating an initial project goal/plan.

Initial templates should remain simple. Possible examples:

```text
Empty
Rust
Node / TypeScript
Python
Custom
```

Orchestrix should not attempt to become a general-purpose project scaffolding framework.

### 4.2 Open Existing Project

The user can select an existing folder/repository.

Example:

```text
C:\dev\fragmented-id-pro
```

Orchestrix can inspect the project and detect information such as:

- whether Git is initialized;
- primary languages;
- package/build systems;
- test frameworks;
- existing documentation;
- known agent instruction files;
- repository status;
- relevant scripts/commands;
- project structure.

This discovery process should produce explicit, inspectable project metadata rather than hidden assumptions.

---

## 5. Project Explorer

The Desktop App should expose the actual project/repository filesystem through a Project Explorer.

Example layout:

```text
┌──────────────────────────────────────────────────────────┐
│ Orchestrix                                               │
├───────────────┬────────────────────────┬─────────────────┤
│ PROJECT       │ MAIN VIEW              │ AGENTS          │
│               │                        │                 │
│ src/          │ Task / DAG / Diff      │ Claude          │
│ ├── auth/     │ Code / File Viewer     │ Codex           │
│ ├── api/      │ Review / History       │ Antigravity     │
│ tests/        │                        │                 │
│ docs/         │                        │                 │
├───────────────┴────────────────────────┴─────────────────┤
│ Timeline / Events / Tests / Terminal / Logs             │
└──────────────────────────────────────────────────────────┘
```

The file tree is not merely navigation. It is part of orchestration observability.

---

## 6. Files as execution-aware entities

A major differentiator from a traditional editor is that Orchestrix should understand a file in relation to agent execution.

When the user selects a file such as:

```text
src/auth/session.rs
```

Orchestrix should eventually be able to show information such as:

```text
Current state

Modified by:
TASK AUTH-042

Agent:
Codex-03

Runtime:
Codex

Execution state:
Awaiting review

Related tasks:
AUTH-038
AUTH-042
AUTH-045

Related events:
14

Review findings:
2
```

Possible file-level metadata includes:

- current repository version;
- uncommitted changes;
- branch/worktree;
- related task(s);
- creating/modifying agent;
- runtime/session;
- related commits;
- execution events;
- review findings;
- test failures related to the task;
- integration status;
- relevant architecture decisions.

The conceptual distinction is:

```text
Traditional IDE:
"This is a file."

Orchestrix:
"This is a file inside an observable multi-agent execution history."
```

---

## 7. File viewer and diff viewer

The user should be able to inspect agent-produced code without leaving Orchestrix.

V1 should support:

- syntax-highlighted file viewing;
- side-by-side or unified diffs;
- worktree vs base comparison;
- task-specific diff filtering;
- changed-files list;
- links from an event to the relevant diff/file;
- links from a review finding to the relevant location.

A complete code editor is optional and can be evaluated later.

For manual editing, Orchestrix may provide actions such as:

```text
Open in VS Code
Open in Cursor
Open in configured editor
Open folder in terminal
```

---

## 8. Project and Repository are distinct concepts

Initially, a Project may map to exactly one Git repository.

However, the domain model should avoid permanently assuming:

```text
Project == Repository
```

Future use cases may include:

```text
Project: Product X

Repositories:
- frontend
- backend
- infrastructure
```

Therefore the conceptual model should allow:

```text
Project
  └── one or more Repositories
```

while implementation may intentionally support only one repository per project in the initial release.

---

## 9. Editor integrations are clients, not foundations

Future VS Code/Cursor/editor integrations should act as thin clients of the Orchestrix Core.

Examples of editor integration features:

- display current Orchestrix task;
- submit selection/file as task context;
- inspect agent status;
- open review findings;
- apply approved patches;
- navigate to Orchestrix tasks/events;
- pause or resume work;
- open the full Desktop App.

The editor extension should not own canonical scheduler, task, event, or project state.

---

## 10. Dedicated Orchestrix IDE is future scope

A future Orchestrix IDE based on Code - OSS or another editor foundation is technically possible.

That decision should only be considered if deep IDE integration becomes a major product advantage that cannot be delivered effectively through the Desktop App plus editor extensions.

It is intentionally excluded from initial development because maintaining an IDE introduces a large unrelated surface area:

- editor engine;
- debugging;
- language integration;
- terminal integration;
- extension compatibility;
- upstream synchronization;
- workbench behavior.

The Orchestrix Core should be reusable by such an IDE if this direction is pursued later.

---

## 11. Initial product invariant

> The Orchestrix Core MUST remain editor-agnostic.

No core orchestration feature should require VS Code, Cursor, or any specific editor to be installed.

The Desktop App is the initial first-class client, not the owner of orchestration truth.

---

## 12. Initial UI scope

A reasonable first functional Desktop shell should target:

```text
Project selector / welcome
        ↓
Project Explorer
        ↓
File Viewer + Diff Viewer
        ↓
Tasks / DAG
        ↓
Agents / Runtime state
        ↓
Timeline / Events
        ↓
Tests / Logs / Terminal output
        ↓
Orchestration Control Center
```

This provides enough project visibility to supervise real work while keeping focus on Orchestrix's unique value: orchestration rather than text editing.
