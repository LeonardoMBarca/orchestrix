# Git worktrees and exact artifacts — OX-006

The [Rust Git boundary](../../crates/orchestrix-core/src/git/mod.rs) connects the guarded [domain](./domain-and-states.md) to [SQLite persistence](./persistence.md). It owns bounded local Git/filesystem operations, not agent execution, project checks or final integration. [Current status](../DEVELOPMENT_STATUS.md) owns ticket acceptance; [increment evidence](../research/ox-006-git-increment.md) records verification.

## API and ownership

`WorktreeService::create()` receives explicit Project/Repository/Run/Task/Attempt identities, repository and authorized-directory paths, opaque directory/worktree references, a full commit OID and the normal command scope/key/expected revision. Existing domain bindings must agree. A branch name, abbreviated object name or implicit `HEAD` cannot replace the pinned base.

`inspect()` checks retained ownership. `capture()` records an immutable artifact; `remove()` applies the conservative cleanup policy below. Readonly records are available through `SavedState::worktrees()` and `captured_artifacts()`. [Git records](../../crates/orchestrix-core/src/git_records.rs) are validated on write and reload, with SQL and relational checks in [persistence/git.rs](../../crates/orchestrix-core/src/persistence/git.rs). These records do not expose setters on domain entities or grant execution authority.

Names derive deterministically from the domain identities: one operation, `orchestrix/attempt-…` branch and `attempt-…` directory per attempt. A persisted token, exact intent blob, pinned-base ref and worktree lock reason identify the operation. An existing branch/path is a collision unless these bindings prove it belongs to that operation. A directory with the expected name alone proves nothing.

Paths are canonicalized and compared by components. Existing symlinks and Windows reparse points/junctions are refused. The target must be a strict descendant of the explicit authorized directory, outside the source checkout, common Git metadata and other registered checkouts. Linked source worktrees do not make their external common Git directory an authorized destination. Windows verbatim paths remain canonical for filesystem checks; only the Git argv boundary removes the unsupported verbatim drive prefix. UNC/device paths and unsupported filename aliases are refused by the current path profile.

## Intent, external effect and confirmation

SQLite never remains in a write transaction while Git runs:

1. Persist `Planned`, the immutable intent and command result through the existing idempotent transaction boundary.
2. Inspect the source/base and create the owned branch/base/ownership refs with a Git ref transaction. Create a locked worktree with `--no-checkout`, materialize supported raw blob bytes with exclusive file creation and populate its index using `read-tree` without updating working files.
3. Verify registration, branch, base, path and materialization. Retain a materialization-completion ref, then commit `Created` with its observation in another short SQLite transaction.

The ownership blob and refs are cross-checks of retained local facts, not a sandbox or proof against a party that can rewrite both the database and repository. Git's own [worktree](https://git-scm.com/docs/git-worktree), [ref transaction](https://git-scm.com/docs/git-update-ref) and [read-tree](https://git-scm.com/docs/git-read-tree) behavior underlies this sequence; the Orchestrix code adds ownership and persistence rules.

If confirmation rolls back after Git succeeds, the resources remain. Repeating the original request checks them and completes confirmation without a second branch/worktree. The completion ref distinguishes completed materialization from an incomplete directory; replay never overwrites subsequent edits. An incomplete or mismatched state returns a diagnostic and preserves resources for inspection instead of resetting, cleaning or deleting them. This is bounded recovery of these operations, not the process reconciler of OX-011 or exactly-once atomicity between Git and SQLite.

## Executor and supported source profile

The [executor](../../crates/orchestrix-core/src/git/executor.rs) launches the installed Git binary with separate arguments and explicit cwd. Each command has a **15-second deadline** and **8 MiB combined stdout/stderr limit**; errors expose fixed classifications, not raw command output. This is a command bound, not a total operation-time or process-tree supervision guarantee. Git versions below 2.36 are refused by admission; the tested version is recorded in the increment evidence.

The child environment is rebuilt from a small OS/path allowlist. Inherited Git repository/index/config/namespace variables are absent. System/global config and system attributes are disabled; hooks use a checked empty private directory, fsmonitor and pagers are disabled, diff requests disable external diff/textconv, maintenance is disabled and no network operation is exposed. `GIT_NO_REPLACE_OBJECTS=1` prevents replace refs from changing content behind a pinned OID. [Git environment semantics](https://git-scm.com/docs/git) document these controls.

Direct local configuration is inspected without following includes. Includes, filter drivers, extensions/sparse or partial-clone configurations, alternate object stores and other explicitly unsupported features are refused. Metadata link checks cover owned refs/reflogs and the object store, with at most 16,384 object entries and two directory levels; deeper layouts are refused. Creation reads raw [blobs](https://git-scm.com/docs/git-cat-file) and never runs checkout filters, hooks, project scripts, submodule initialization or LFS downloads. Source materialization currently accepts at most 4,096 regular `100644` UTF-8 text files, 1 MiB each and 64 MiB total, with no NUL bytes or recognized credential patterns. It does not repair unsupported repositories.

## Exact capture and coherent observations

[Capture](../../crates/orchestrix-core/src/git/capture.rs) records repository/Run/Task/Attempt, operation, original path and pinned base. Every changed path retains separate optional **base**, **index** and **working-file** content/hash/mode records. This distinguishes staged changes, unstaged changes, staged additions/deletions and relevant untracked additions, including an index-only change whose final working bytes equal the base. Renames are an explicit deletion plus addition; identity is not guessed.

The artifact contains exact UTF-8 bytes, SHA-256 hashes, a deterministic full-file unified patch from base to working files, its hash and a revision hash over the complete serialized artifact excluding its own revision. The index manifest remains evidence even when the final patch is empty. CRLF and missing final newlines are preserved. Capturing/persisting this fact does not publish/approve a Task, run its checks or apply its patch.

Three matching observations surround capture and blob/patch assembly. They compare HEAD/branch, base/index entries, raw index bytes and supported index flags, attributes, untracked/ignored inventory, included working bytes and filesystem metadata. Metadata and bytes are also checked around each file read. A mismatch rejects the capture; no mixed artifact is persisted. This detects the tested concurrent edits, including edits to previously unchanged files. It does not establish an atomic filesystem snapshot against a hostile writer that changes and restores state between observations.

| Bound | Current maximum |
| --- | --- |
| Included inventory | 4,096 paths |
| Individual working file/blob read | 1 MiB |
| Aggregate working reads per observation / unique blob reads | 64 MiB each |
| Captured changed paths / recorded exclusions | 128 each |
| Exact patch | 1 MiB |
| Stored content plus patch | 4 MiB |

Limits fail rather than truncate. Ignored files/directories and explicit sensitive/dependency/build paths appear as omissions without reading their contents. The policy includes `.env` variants, key/credential paths, `node_modules`, virtual environments, `vendor`, `target`, build/dist/cache/coverage outputs. Omission is visible and does not mean the omitted work was preserved. Links, submodules, executable modes, unmerged or nondefault index flags, filter/encoding/ident attributes and changed binary/non-UTF-8 content are unsupported.

Exact evidence is never passed through a redactor and then labeled as original. If the existing supported-pattern privacy policy would change a captured path/content/patch, capture refuses it. This is not a universal secret detector. Sanitized diagnostics and exact artifacts have different purposes; neither a hash nor successful capture proves that arbitrary content is safe.

Migration **3** appends Git operation/observation/artifact tables to the OX-005 database without rewriting migrations 1/2. Intent and artifact versions are immutable; observations remain append-only. A replayed capture resolves and validates its original persisted artifact before inspecting today's files, then reuses the normal command fingerprint/revision checks. A new capture requires a new key. Loading an artifact does not authenticate a provider or resume execution.

## Conservative removal and limits

Removal currently accepts only a proven owned, materialized worktree whose HEAD/branch still equal the pinned base and whose exact capture has **no changed paths and no omissions**. A persisted Admitted, Running or Uncertain attempt blocks removal; rechecking these facts does not replace the future supervisor's process ownership proof. Dirty/staged/untracked/ignored/excluded work or a unique commit prevents cleanup, even if some artifact was previously captured. There is no general archive-and-discard policy in this increment.

Before cleanup, bounded reads of the owned branch and available worktree HEAD reflogs reject references to any commit other than the base, including a unique commit followed by a reset to a clean base. The branch reflog is explicitly created and its absence/empty state blocks removal. Each inspected reflog has a 1 MiB limit. These are local retained-history checks, not a discovery/archive service for arbitrary unreachable Git objects or rewritten/deleted history.

The service persists `CleanupPlanned`, revalidates ownership and cleanliness immediately before removal, unlocks only its matching lock and calls nonforced `git worktree remove`. It compare-and-deletes only its direct branch ref at the expected base, retains ownership/base/completion anchors and confirms `Removed` separately in SQLite. A failed confirmation can be retried after inspecting the already removed resources. No reset/stash/clean, global prune, forced removal or arbitrary recursive filesystem deletion is used as recovery.

Static link/config/path checks and repeated observations do not prevent every TOCTOU replacement by another host process. Supported operation assumes the repository/configuration/directories are not maliciously replaced during these checks; this is not OS filesystem containment, a hostile-repository execution sandbox or power-loss certification. The evidence covers Windows with owned disposable Git/SQLite fixtures, not runtime execution or untested operating systems. OX-007 supervision, OX-009 project checks, OX-011 broader reconciliation and OX-014 application retain their scopes.

## Serial integration and explicit application

[`IntegrationService`](../../crates/orchestrix-core/src/integration.rs) implements the initial `single-code-task-clean-target-v1` profile: one existing manual Run/CodeTask, its exact currently verified and Accepted artifact, and its original clean checkout at the physical Git base. Other task combinations, changed bases, staged source changes, omitted content, dirty/untracked/ignored target files and unsupported Git profiles block explicitly. This does not claim general multi-task merging or dogfooding in an incompatible repository.

Preparation reserves an immutable integration intent in schema 7 and creates a separate Run-owned branch/worktree. It materializes the selected artifact, captures the combined version and runs only the explicitly authorized checks there. Checks bind command/hash, cwd, authorization, timeout, exit code and exact snapshot revision. A change during checks invalidates their authority. The result is a genuine Git tree/commit under its own result ref; a content hash is never used as a commit OID. The attempt worktree and immutable implementation/review artifacts remain preserved.

The candidate binds the integration result and source evidence. Internal integration completes the CodeTask; it does not deliver the Run or authorize changing the user's checkout. Its retained target binding additionally pins canonical checkout/common Git directory, symbolic branch, HEAD, logical index and working bytes. Index stat-cache bytes and file timestamps are excluded from this logical fingerprint; coherent reads still check metadata and bytes for concurrent changes.

Human approval pins the candidate revision, target and an explicitly requested validity of 1 second to 24 hours; Accepted code review alone grants no application authority. Approval expiry, clock rollback or a changed candidate/base/content/check blocks the corresponding operation. Approve and Apply revalidate independently. Apply checks the clock again immediately before the first working-file update, after any index refresh and coherent target reads. An invalidated candidate cannot reuse its old approval. Historical inspection grants neither permission nor current verification.

Git mutations and checks join the same private Windows keeper Job before running; each Git command retains its 15-second/8-MiB bounds. The existing keeper deadline is unchanged. No SQLite writer remains open while Git or checks execute. Final application journals intent, updates supported target files/index, compare-and-swaps the exact target ref, verifies the resulting target and records application confirmation before marking the Run Delivered. No reset, clean, stash, hook, filter, network operation or automatic conflict resolution is exposed.

Before `read-tree -m -u`, application verifies the approved logical target, runs only the fixed argv `update-index --refresh` in that same journaled Job, then verifies the logical target again. Git's [stat-cache refresh](https://git-scm.com/docs/git-update-index#_using_refresh) handles a file whose original bytes were restored after an edit without staging new content or modes. The executor rejects this command outside an owned Job and rejects extra arguments, including paths, `--add`, `--cacheinfo`, quiet/ignore-missing flags and `--really-refresh`. A refresh failure or failed final authorization/cancellation precondition prevents `read-tree` and target-ref CAS. Once the mutation phase has begun, its outcome may conservatively remain Uncertain even when only index metadata changed; no automatic retry is inferred from unchanged working bytes. After working files change, cancellation does not promise rollback: the already authorized CAS may finish, or the operation preserves its uncertain partial result.

Replays consult the original operation identity. A reservation interrupted before the journaled Job launch can terminate as Blocked without performing effects. Restart never repeats application or checks. Exact completed Git effects plus the fenced own-Job closure proof can complete missing database confirmation. A crash after file/index update but before target-ref CAS preserves the partial result as Uncertain and blocks admission for inspection; this profile does not automatically resume that ref operation or discard files. Missing/mismatched closure or Git evidence also stays blocked. Editing or removing the original attempt checkout after internal integration withdraws its physical verification without rewriting the independent integrated snapshot or a confirmed delivery.
