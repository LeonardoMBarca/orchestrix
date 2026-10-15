# ADR-0008 — Recoverable owned-process closure evidence

- **Status:** Proposed; acceptance depends on the integrated crash tests
- **Date:** 2026-10-14
- **Owners:** Orchestrix Core / Windows process boundary
- **Related issues:** OX-011, CONT-01/02

## Context

The accepted supervisor safely contains runtime processes in an owned Windows Job, but its anonymous handles cannot be reopened after host death. A durable dispatch may then remain uncertain despite local cleanup. A PID alone cannot establish ownership, and acknowledging uncertainty is not evidence of process closure.

A named Job alone is insufficient: a temporary object's name can disappear when its handle count reaches zero while internal references remain. Thus OpenJobObject not-found is not a proof of empty process membership. [Microsoft Object Manager lifecycle](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/life-cycle-of-an-object) distinguishes handle/name lifetime from final deletion. An internal boot GUID or an estimated boot timestamp is not used to bypass that gap.

## Decision

Provide a small trusted Rust supervisor helper, built alongside the CLI, that holds a handle to the exact restricted Job before the runtime root is created. The Core keeps its existing process I/O and Codex adapter. The helper keeps ownership evidence alive independently; it does not interpret provider protocol, choose a model, delegate or duplicate the worker engine.

Persist a scope/receipt reference before entering the process-launch window. Use a private user-owned scope outside repository/worktree access. Bind helper readiness to that Job's identity/security and the host process handle plus creation identity. No runtime process may start before readiness. Preserve atomic Job association through the existing process-creation Job-list attribute, which assigns membership before the first thread runs. See [Microsoft's explanation of PROC_THREAD_ATTRIBUTE_JOB_LIST](https://devblogs.microsoft.com/oldnewthing/20230209-00/?p=107812).

On host death, explicit finish or bounded lifetime, the helper first seals admissions to that exact kernel Job with a verified zero-active-process limit, then terminates its members and confirms zero active processes. Identified process creation never resets that limit. This fence matters even when the original host remains alive: a delayed host cannot put a new process into a scope after its closure was certified. A Windows test uses the same executable/specification successfully in an open Job before proving that association to the sealed Job fails before child code runs.

The helper publishes a bounded scope-bound closure receipt only after that fence and empty-membership observation. Publication must be durable and atomic; incomplete/missing/invalid receipts are not positive evidence. The helper is outside the runtime Job, has no arbitrary-shell entry and uses only the installed sibling executable or an explicit trusted fixture path. Private files inherit the restricted user DACL, rather than the creator token's default file DACL.

The receipt is a **local scope-closure fact** covering the worker's runtime and explicitly authorized check processes, which share this Job. Git keeps the earlier OX-006 process limitations. The receipt cannot invent root exit code, drained streams, provider terminal, task success or a check result. Recovery may consume it for an explicitly authorized SetAside while retaining the old attempt/session and uncertain remote outcome. New work still requires normal policy/authentication/permission/billing checks; the old command/task ID is never dispatched again.

## Alternatives and consequences

- Retain uncertainty with no reconciliation mechanism: safe, but leaves common host crashes without an operational release path.
- Infer closure from a missing Job name, free PID or available host lease: rejected because none proves the old tree stopped.
- Persist a boot timestamp/uptime: not a general same-boot recovery mechanism; no undocumented boot identity is adopted.
- Proxy every provider stream through another runtime engine: unnecessary; a keeper handle and separate local receipt preserve the current adapter boundary.

The helper adds a binary that must accompany the CLI/Core host. Missing helper support blocks the corresponding launch; it does not silently downgrade supervision. If helper/host die before a valid receipt, recovery preserves the incident and stays blocked. Same-user/administrator tampering, power loss and copied/hostile database deployments do not gain guarantees from this scoped Windows design.

## Validation and reconsideration

Use real Windows processes/Git/SQLite and a synthetic runtime: stop the host before/after keeper readiness, before dispatch and after confirmed dispatch; prove bounded owned cleanup and scope-bound receipt; reopen without retry; authorize only eligible SetAside; retain unrelated processes. Kill the helper or corrupt/mismatch a receipt and require conservative blocking. Record exact source revisions, failures and limits before acceptance.

Revisit for another OS, packaging, an independently managed supervisor service or provider work continuing remotely after local termination. That last condition remains a distinct native-runtime fact, not something a local Job receipt can settle.
