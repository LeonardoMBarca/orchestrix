# ADR-0007 — Local daemon and user-restricted Windows transport

- **Status:** Proposed; implementation/acceptance tracked in current status
- **Date:** 2026-10-14
- **Owners:** Orchestrix Core
- **Related issues:** OX-010, OX-011, CONT-01/02

## Context

The accepted serial manual worker needs a durable owner that survives CLI/client disconnection and serves controls without inference. Subscription exhaustion must not prevent inspection, suspension or cancellation. The future Desktop uses the same application API; Tauri/provider messages remain outside the domain. Windows is the first validated platform.

## Decision

Run a dedicated local daemon process per canonical database/user scope. An OS lease excludes another daemon and competing legacy CLI hosts. Reuse the existing Rust worker/Store/supervisor; one background worker thread has its own short-lived SQLite operations while the control loop remains responsive. No writer transaction spans an external effect. This is serial execution, not a new scheduler.

Use a Win32 named byte pipe in `orchestrix-process`, with a protected explicit DACL granting the current user's SID, non-inherited handles, remote clients rejected and first-instance enforcement. The client checks the server's user identity. Do not use the default pipe security descriptor: its default access is broader than this scope. See [Microsoft named-pipe security](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights) and [CreateNamedPipe flags](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createnamedpipea).

Bound messages and I/O deadlines; overlapped read/write/connect operations keep ownership of their buffers until completion. Cancellation requests alone do not permit freeing a pending buffer; drain completion first, as required by [CancelIoEx](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelioex). OS FFI stays outside the Core's `forbid(unsafe_code)` boundary. Other systems return Unsupported until a separately tested transport exists.

For overlapped pipe reads/writes, pass null for the synchronous byte-count output and obtain confirmed counts through GetOverlappedResult, including immediate completion. A successful API return is not authority to use a potentially erroneous synchronous count. See the documented asynchronous parameters in [ReadFile](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-readfile) and [WriteFile](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-writefile).

Version commands/queries/responses and cursor envelopes. Journal a command before its external effect, retain an immutable completion, and replay results without dispatch. Store domain-change notifications in the same SQLite transaction as domain events. A consistent snapshot carries its high-water cursor. A wrong/future/discontinuous cursor requires explicit resynchronization, not silent omission.

Keep compact historical Snapshot/Workers reads separate from aggregate restoration: page selected worker histories and validate their local structure/ownership under the same cursor read transaction. These projections grant no eligibility or artifact certification; startup, writes and decisions keep full validation. Default diagnostics remain bounded in memory, so an undrained stderr pipe cannot block normal controls. An explicit diagnostic stderr opt-in requires a supervised/drained sink.

## Lifecycle and recovery

Startup suspends admissions and reconciles durable evidence before an explicit resume. Client disconnection does not terminate the daemon. Suspend applies to new admissions; cancellation targets only the active owned worker and its previously authorized process tree. Provider failure does not disable these deterministic controls.

Graceful shutdown suspends admissions, requests cancellation and waits within a bounded deadline. The dedicated daemon entry may terminate its own host on that hard deadline rather than leave a detached worker thread; the OS closes owned Job handles. This is not remote-terminal evidence. Recovery preserves unresolved facts and never automatically repeats accepted/uncertain work. `serve` is a dedicated-process boundary, not an embeddable service inside Desktop or another host.

## Alternatives considered

- Loopback TCP: viable with separately secured credentials/rendezvous, but adds local authentication material and port ownership without a current need for browser/network clients.
- In-process Desktop owner: couples execution/control lifetime to a window and leaves CLI-only operation dependent on Desktop.
- Node helper: useful in the historical harness; the production worker already has a Rust/OS boundary and needs no extra runtime.

## Consequences and validation

No new dependency or paid/provider probe is required. Same-user processes and administrators are not isolated by this endpoint; do not claim a sandbox against that principal, unvalidated macOS/Linux support, power-loss proof or a remote server. Filesystem access to the database remains governed by its authorized local directory; pipe ACLs are not database encryption.

Acceptance requires real multi-client IPC, command replay/reconnect, cursor recovery, controls while a synthetic provider is unavailable, bounded cancellation/shutdown and crash/restart cases with Git/SQLite/processes. Inspect DACL/remote flags and test a restricted token where supported rather than inferring denial from a same-user echo test. Results and remaining limits belong to the increment evidence before marking this ADR Accepted.

Revisit for another OS, a multi-user/network service, larger concurrency, transport threat-model changes or a supported native provider contract change. Preserve this decision's original Windows scope on supersession.
