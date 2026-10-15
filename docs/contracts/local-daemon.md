# Local daemon API and deterministic recovery

OX-010/011 reuse the accepted [manual worker](manual-worker.md), [persistence](persistence.md), [Git ownership](git-worktrees.md) and [context boundary](../CONTEXT_OWNERSHIP.md). [ADR-0007](../adr/0007-local-daemon-windows-transport.md) defines the Windows transport/lifecycle decision; [current status](../DEVELOPMENT_STATUS.md) owns acceptance.

## Scope and clients

One dedicated daemon per canonical database/user scope owns operational access and at most one worker. A separate daemon lease excludes another daemon/legacy CLI host; the existing worker lease still guards worker effects. Native handles and IPC APIs stay in `orchestrix-process`, outside the pure domain and policy. No Node/Tauri runtime or new execution engine is required.

API v1 uses bounded JSON request/reply frames over a user-restricted local named pipe. `Request { api_version: 1, operation: { type, ... } }` and `Response { api_version, ok, error, result }` are shared Rust application types. Provider frames, credentials, arbitrary shell commands and client-supplied supervisor assertions are not API commands.

Operations: Snapshot, Workers, Status, Events, Submit, Suspend, Resume, Cancel, Shutdown, Recovery, RecheckRecovery, Continuity, Intervene and historical deterministic queries. Submit includes a unique command ID, explicit authorization reference, the existing typed manual-task manifest and an explicit runtime choice. Codex inference requires `allow_live:true`; synthetic fixtures remain labelled and establish no provider capabilities. Project checks retain individual manifest authorization; submission does not authorize discovered scripts.

The daemon CLI starts in the foreground for a service host to supervise. A background launcher must use a hidden window on Windows. Clients only connect; they do not start a scheduler on reconnection or open SQLite to bypass daemon ownership.

```powershell
orchestrix daemon --database C:/orchestrix/state.sqlite
# Separate client process:
orchestrix snapshot --database C:/orchestrix/state.sqlite
orchestrix resume --database C:/orchestrix/state.sqlite --command-id resume-1
orchestrix api --database C:/orchestrix/state.sqlite --request C:/orchestrix/authorized-request.json
orchestrix suspend --database C:/orchestrix/state.sqlite --command-id suspend-1
orchestrix cancel --database C:/orchestrix/state.sqlite --command-id cancel-1 --id task-1
orchestrix shutdown --database C:/orchestrix/state.sqlite --command-id shutdown-1
```

## Commands, events and disconnection

Schema 5 adds daemon identity/generation/admission state, immutable command journal/completions and append-only local API events. Migrations 1–4 remain unchanged. Domain events insert a sanitized change reference into the daemon stream within the same transaction. No SQL transaction stays open during worker effects or IPC waits.

Bind a command ID to the complete operation fingerprint before effects. Identical replay returns its recorded receipt/completion and never reexecutes the worker. Changed content is a conflict. A missing response is not permission for a new command ID; inspect/replay the original. Startup records incomplete commands as interrupted, preserving uncertainty and the original journal instead of dispatching them.

A snapshot reads compact worker references/control state and the event high-water cursor in one SQLite read snapshot. Additional worker pages bind to the same high-water cursor; changed state requires explicit resynchronization. Detailed outcomes remain available through Status. Cursor binds database identity and sequence. Events are paged and retained in V1; wrong database, future or discontinuous positions return explicit resync-required. Old domain history before migration is represented by the baseline snapshot, not invented API events.

Status and snapshots label verification/process observations historical, not freshly revalidated; the control loop performs no synchronous Git inspection. Slow filesystem/Git queries stay outside that loop. Check/process queries return durable facts without a model. Startup reconciliation revalidates the required artifacts before admitting new work.

Compact Snapshot/Workers pages read only the selected worker histories with identity/ownership joins, digest/ordinal/transition checks and structural recovery validation. They do not rerun aggregate policy resolution, certify an artifact or authorize SetAside/admission. Full Store restoration remains mandatory at startup, writes and decisions; detailed Status/Recovery/Continuity retain their aggregate validation. Message/page bounds do not establish unlimited-database responsiveness. Transport diagnostics stay bounded in memory by default; explicit host diagnostics may opt into stderr and require a drained sink or regular file.

The client acknowledges reading a response before the server disconnects its pipe instance, with bounded waiting on both sides. A successful pipe write alone does not mean the peer read the response. Command effects depend on the durable journal, never on receipt of this transport acknowledgment.

Suspend blocks new admissions and preserves observation/persistence of active work. Resume never resumes an old model turn and must pass recovery gates. Cancel targets only the daemon's current owned worker ID; the cancellation receipt is a request, not a terminal confirmation. Client disconnection leaves the daemon running. Shutdown suspends/cancels; an independent watchdog bounds host shutdown to 20 seconds. Normal exit is 0; expiry exits only this dedicated host with code 3 and preserves uncertainty for startup inspection. This is not a deadline for remote provider completion or proof of a captured artifact.

## Recovery and intervention

Before new admissions, acquire worker ownership and inspect durable reservations, dispatch intent/admission/terminal facts, worktrees and artifacts. Never adopt/kill a process by PID or infer a provider terminal from host exit, stream closure or absence of a process. Persist Preparing with the keeper binding before starting that helper; persist SupervisorReady before any target runtime process can launch. Legacy Preparing records lack that enforced barrier and do not prove a launch-free window. Persist Checking before launching an evaluator: the runtime's earlier empty-Job/stream observation cannot certify later check processes.

Classify never-sent, dispatch intent only, confirmed dispatch and known outcome separately. Missing confirmation stays uncertain, without retry. Startup preserves approvals/versions and historical observations; it grants no fresh authentication/capacity. Reinspect Verified artifacts through the existing exact capture guard. Worktree ownership/inspection failures remain visible and block unsafe continuation of that work. A warning on a closed Failed/Cancelled/SetAside record does not globally forbid a new independent authorized task; it never authorizes adopting, cleaning or retrying the old work. This batch has no automatic dependency continuation or application of those artifacts.

Intervene requires exact current revision, authorization reference and a constrained action. RecordInspection records acknowledgment while preserving the block. SetAside can release a local admission barrier only with a proven pre-launch never-sent window, trusted owned-tree/stream closure evidence, or a separately validated local Job-closure receipt from the [independent keeper](../adr/0008-recoverable-process-ownership.md). The latter does not establish stream closure or native terminal. It preserves the old attempt/session/outcome and artifacts; replay of the original task ID never dispatches it again. Preparing alone, a missing Job name and generic acknowledgment cannot supply closure evidence. Cases without sufficient proof remain blocked with explicit next actions; the API accepts no arbitrary PID or claimed stop boolean.

RecheckRecovery explicitly reads and validates a bounded private keeper receipt, then records it in a short transaction. It allows a receipt published after startup to be inspected without reopening the daemon. It performs no provider call, Git command, process adoption or dispatch, and does not itself authorize SetAside. Receipt validation during SQLite restoration is pure; filesystem/process queries never run inside that transaction.

The trusted `orchestrix-supervisor` executable must accompany the CLI/host. It holds the exact Job handle before the runtime starts, pins host identity with a process handle and creation time, and has a maximum scope lifetime of 300 seconds for runtime and authorized checks together. Before publishing any receipt, it applies a kernel admission fence, terminates members and confirms an empty Job. Missing helper, invalid receipt or simultaneous helper/host loss remains fail-closed. Git retains its separate process profile; this receipt does not certify all Git subprocesses or power-loss recovery.

Continuity reconstructs a bounded basic Context Pack from durable objective/criteria, policy/context sources and observed artifacts/checks/reviews/approvals. It records revisions, omissions and historical file bytes, without synthesis/inference, native transcript or hidden reasoning. Reconstruction grants no retry permission or native-session transfer.

## Capacity and remaining boundaries

Local controls, snapshots, recovery and continuity require no model. Missing subscription quota stays unknown; API credits are unused in Subscription Only. Rate-limit/auth/provider failure does not manufacture a remaining percentage or enable fallback. [CAP-01/02/03 and CONT-01/02/03](../design/capacity-and-continuity.md) distinguish current local controls from future provider telemetry, reserves, contextual scheduling and orchestrator transfer.

Windows only; other OS transports explicitly return Unsupported. The scope is a canonical authorized database path, not a cross-user/network service or a sandbox against the same user/administrator. Physical database aliases/hard-link deployments and hostile filesystem writers are outside the initial profile. Git retains its regular bounded UTF-8 profile; compatible fixtures do not establish full-repository dogfooding, OX-014 integration/application or M0 completion.
