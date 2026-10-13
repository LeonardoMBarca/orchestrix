# ADR-0005 — Desktop Product Scope and Platform Boundaries

- **Status:** Accepted
- **Date:** 2026-10-13
- **Owners:** Product owner, explicit alignment of 13 October 2026
- **Related issues:** OX-003, OX-007, OX-012–OX-017; M5 distribution
- **Supersedes:** ADR-0003 only in IDE/editor product scope; its Desktop + editor-agnostic Core decision remains accepted

## Context

ADR-0003 selected Desktop + Core and rejected an IDE for initial development, while leaving a dedicated Orchestrix IDE as possible future product scope. The owner now explicitly excludes that presumed evolution and confirms a supporting editor. Windows is the first validation environment, while Windows, macOS and Linux remain intended distribution platforms. Current Codex/Job evidence is Windows-specific and does not establish support for other systems.

## Decision

Orchestrix is an open-source Desktop application for conversing with a selectable orchestrator agent over user-provided connections. The Core owns policy, canonical state and authorized execution. One account, one runtime and serial execution remain first-class configurations. Selecting an orchestrator never grants permissions or billing authority.

The product includes a supporting editor: file/folder opening and tree navigation, lightweight text/code edits, language-aware syntax highlighting, Markdown preview, diffs, save/undo and unsaved-change handling. Writes must coordinate with agent changes, preserving user edits and invalidating evaluations/approvals tied to old artifact versions. [Workspace scope and acceptance](../DESKTOP_APP_AND_PROJECT_WORKSPACE.md#7-supporting-editor-and-diff-viewer) define the behavior without selecting an editor engine here.

Building an IDE is outside the current product direction, including as a presumed future milestone. Interactive project execution, debugger, test explorer, embedded interactive terminal and an IDE extension ecosystem are not product goals. Opening an external editor or system terminal is an optional convenience. Thin integrations with existing editors remain possible clients of the same Core.

Internal execution is preserved: the Core may start/supervise agents, operate Git and run authorized verification tools in the environment. Displaying logs, commands/results and evidence does not create an interactive execution console.

Windows, macOS and Linux are intended distribution targets. Validation/delivery is progressive, with Windows allowed first and no simultaneous release promise. Domain types and runtime wire contracts must remain OS-independent. Process containment, signals/handles, filesystem/path rules, credentials, watchers and OS integrations belong in infrastructure, host or adapter layers. Missing platform support must be diagnosed explicitly; a Windows Job proof is not a portable containment guarantee. [Per-system delivery criteria](../TECH_STACK.md#platform-boundaries-and-progressive-delivery) govern support claims.

## Alternatives considered

### Preserve an Orchestrix IDE as presumed future scope

Rejected by the owner's explicit product definition. Existing editor integrations do not require Orchestrix to own an IDE. The original discussion is retained in ADR-0003 as historical context.

### Remove all process execution because the app is not an IDE

Rejected because supervision, Git and deterministic verification are necessary orchestration responsibilities. Their authorization and evidence remain distinct from interactive user tooling.

## Consequences

The team can implement a useful editor and inspectable execution evidence without owning a debugger/terminal/extensions stack. Platform-specific infrastructure needs separate tests and support statements, while contract/domain conformance stays portable. Editor concurrency and artifact versioning remain necessary implementation work; documentation does not certify these features.

## Validation

OX-003 verifies that the normalized consumer needs no OS/provider/Tauri types. OX-007 proves containment/cleanup for each claimed system; OX-012/OX-015 verify editor save/undo/unsaved changes and concurrent writes; OX-013/OX-014 reject stale artifact approvals. OX-017/M5 apply the platform matrix before advertising a supported build. These are planned criteria, not results of this ADR.

## Revisit conditions

Changing the non-IDE scope requires a new explicit owner decision and a superseding/revised ADR. Changes in platform capabilities or runtime integration require new scoped validation; they do not silently redefine the product.
