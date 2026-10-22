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

Direct local configuration is inspected without following includes. Includes, filter drivers, extensions/sparse or partial-clone configurations, alternate object stores and other explicitly unsupported features are refused. Metadata link checks cover owned refs/reflogs and the object store, with at most 16,384 object entries and two directory levels. The sole additional level is Git's `objects/info/commit-graphs`: its immediate children must be regular files and consume that same entry budget; links, arbitrary directories and further nesting remain refused. This admits the existing repository layout without writing/rebuilding its graphs. Creation reads raw [blobs](https://git-scm.com/docs/git-cat-file) and never runs checkout filters, hooks, project scripts, submodule initialization or LFS downloads. Source materialization currently accepts at most 4,096 regular `100644` UTF-8 text files, 1 MiB each and 64 MiB total, with no NUL bytes or recognized credential patterns. It does not repair unsupported repositories.

### Complete-repository preflight

#### Exact opaque-base preservation — profile v2

Decision for the OX-017 increment: retain native Git objects and the existing
owned base/owner/completion anchors. Do not create another content store or turn
every baseline file into textual artifact content. `WorktreeIntent` and
`CapturedArtifact` opt into `preservation.schema_version = 2`; absence retains
the original textual representation, validation and revision hash. Explicit null
is rejected. Historical bodies/hashes are not migrated or recomputed as v2.

The immutable manifest binds the original commit/tree and a sorted inventory of
path, regular mode, size, Git object ID and SHA-256, plus a sorted explicit
editable allowlist. The complete base, editable set, authorized Context Pack and
textual review evidence are separate sets. A changed/new/deleted path outside
the editable set blocks capture, including an opaque same-size modification;
unsupported content is not silently omitted or restored. Correction inherits
the exact allowlist and original manifest, materializing the preceding textual
result over that original base. Artifact revisions are not commit IDs.

The v2 reader verifies each raw blob against its native Git ID, including the
`blob <length>\0` header, before pinning or accepting its SHA-256. It similarly
verifies the bounded pinned commit body and reconstructs the tree ID from the
complete inventory with Git's directory ordering. A valid compressed object
stored under another object's ID is inconsistent evidence, not a replacement
base. This uses RustCrypto SHA-1 for native SHA-1 repositories and the existing
SHA-256 implementation for native SHA-256 IDs; retained SHA-256 content hashes
remain independent. Tree projection permits at most 32 directory levels and
1 MiB of encoded entries. Only the pinned commit/tree and bounded blob batches
are checked; no history-wide fsck, network recovery or HEAD substitution occurs.

Independent ceilings: 4,096 base files, 64 MiB base bytes, 6 MiB per preserved
blob, 1 MiB serialized preservation metadata, 128 editable paths. Editable files
remain at 1 MiB each, patch at 1 MiB, total captured text/patch/manifest at 4 MiB;
the Context Pack retains its existing independent limits. The measured original
Orchestrix base (54,332,380 bytes, largest 4,685,242) motivates these preservation
limits without increasing `MAX_FILE_BYTES`. Blob batches stay below 7 MiB and
the executor's existing 8 MiB/15 s bounds. Materialization writes exclusive files
from bounded batches; opaque snapshot checks stream SHA-256 rather than keeping
multiple copies of the full tree in memory.

Scoped local Git reads use the supervisor's explicit backpressured delivery
mode for bulk blob output: the existing 128-event/8 KiB-chunk queue and process
byte/lifetime limits do not grow. A reader waits for queue space while retaining
only its pending chunk; termination/Drop/the independent watchdog release it.
Runtime protocol callers retain the original fail-fast delivery. Git drains
bounded bursts before Job queries and preserves I/O errors during terminal
draining; process exit, stream closure, complete output and tree termination
remain separate observations. A single bounded input-write ticket is polled
while draining output, rather than waiting for all input before reading output:
`cat-file --batch` may emit a large object before reading its next request. This
keeps both pipe directions progressing without increasing their budgets or
repeating writes.

The complete serialized ownership intent must also fit the existing 1 MiB
Git-input envelope, checked before `Planned` is persisted. The manifest ceiling
does not grant additional space for paths/IDs in that envelope. Typed manifests
validate every path/mode/size/OID/hash and their own byte/item bounds before a
small privacy-validation projection substitutes the manifest digest. V2 command
fingerprints hash the full original typed body; v1 fingerprints stay unchanged.
The original immutable bodies remain in SQLite. Integration snapshots use the
same narrowly typed projection, retaining the existing 1 MiB intent/4 MiB
observation envelopes and privacy checks on every other field. This does not
increase the generic JSON-node budget or redact stored evidence.

Only regular mode 100644 is supported here. Path aliases, links/junctions,
submodules, executable modes, unsupported attributes/configuration, external
filters/hooks/network and excluded sensitive/generated paths retain their
guards. Preserving a privacy-pattern match locally is neither confirmation of
a credential nor permission to disclose or edit it. Selected editable/context
files still undergo textual/privacy checks before provider metadata.

Review receives the exact authorized full-file textual patch, checks and
base/profile/inventory digest/allowlist summary, not binary or oversized
baseline bodies. The v2 evidence projection identifies the original artifact
revision and version presence/hash/size/mode, without repeating base, index and
working bodies already represented in the patch. It never recalculates the
original artifact revision from that reduced view. Runtime
tools **can read materialized workspace files** under the actual runtime
permissions. The manifest and Context Pack are not an OS read sandbox. A policy
requiring unproven read confinement must block rather than treating omission
from the prompt as enforcement.

The initial integration profile refuses staged/index changes. V2 review refuses
them before metadata as well, preserving their captured content rather than
accepting index-only code absent from the base-to-working patch. It never resets
the index automatically; a new explicit compatible attempt is required.

For v2 integration materialization, the completion operation returns a fresh
verification only when that same call has checked the complete initial base and
written its completion anchor. This replaces the immediately repeated empty
artifact capture only for that fresh result. Existing/replayed completed work
retains the original empty-artifact inspection. Every edited file is still
compared with its exact base before writing, and the mandatory final complete
capture must match the source, including protected files. Unexpected changes
are preserved and block the result.

Approval and explicit inspection retain their full read-only verification. V2
application performs commit, candidate-tree and complete artifact verification
inside the owned Job, followed by both target fences around index-stat refresh
and the clock/cancellation check at the byte-write edge. It does not duplicate
that same verification outside the Job first. The legacy profile, approval TTL,
Job lifetime, checks and post-effect reconciliation are unchanged.

Capture, checks, review, correction, integration, target approval/application
and recovery use the same profile. Protected content is revalidated, including
after checks and before authority-bearing effects. Combined trees reuse exact
opaque Git object IDs and write only authorized textual changes. The source
artifact revision binds the profile/allowlist; candidate and approval bind that
revision and the exact target/base/result. A changed protected file invalidates
the applicable evidence. Objects must resolve through the original repository
and owned anchors; missing, corrupt or mismatched objects block. There is no
fallback to current HEAD, destructive restoration or automatic replay.

`repository-inspection-v1` accepts an explicit editable selection and returns
schema-2 `opaque-base-v2`, preservation counts/bytes and selection-required
state. Queries without that selection keep schema-1 textual behavior. Both are
observations only; preparation and final operations revalidate. The normal
single-CodeTask/clean-target integration restriction is unchanged. Git/SQLite
still use intent → effect → confirmation and never share an atomic transaction.

#### Textual preflight compatibility

`inspect_repository()` returns the advisory schema-1 `regular-utf8-v1` report over a full pinned commit, with HEAD, file/mode/size totals, bounded blocker examples, checkout state, skipped content and separate worker/application eligibility. No repository, ref, index, worktree, database or provider effect is requested. An explicit standalone mechanical query or the daemon's `repository-inspection-v1` read surface uses this same implementation. It never grants execution or application authority.

The inventory comes from bounded `ls-tree`; raw `cat-file --batch` reads stay below 7 MiB per batch and 64 MiB total. File/path/content rules and limits are shared with worktree creation and capture. Oversized content is not read or classified; `complete` means the bounded inventory/classification pass finished, not that skipped bytes were decoded. Content counters therefore describe only inspected files. A conservative privacy-pattern match is a profile blocker, not a finding of actual credentials. Responses contain no file content and are limited to 64 KiB, with three sanitized examples per blocker and 8,192 status characters.

Attributes are inspected both at the pinned base and in the working checkout. [`check-attr --source`](https://git-scm.com/docs/git-check-attr/2.43.0) supplies the read-only base observation; if unavailable, preflight blocks explicitly rather than trusting modified local attributes. Existing executor admission still refuses Git below 2.36; this preflight additionally needs the source-attribute operation documented in Git 2.43. A compatible inventory undergoes capture's repeated coherent checks of working content, index/flags, links and attributes. Base content hashes, final status/untracked/ignored observations and HEAD fence the advisory clean/application claim. Dirty/ignored data may prevent application without prohibiting otherwise supported worker preparation. Final application still invokes its full existing validation.

Preflight has a 90-second total Git budget, the existing 15-second/8-MiB per-command bounds and cancellation between commands. It does not enlarge worker/runtime/check budgets. Unsupported metadata can reject before inventory; no empty report is presented as a successful scan. The schema-1 textual limits and historical hashes remain unchanged; the opt-in v2 limits above apply only to that separate profile.

That observation deadline is checked at operation boundaries, rather than a
hard preemption of Windows filesystem calls. A bounded metadata walk can cross
it before the next check reports Deadline; entry/path/byte limits still apply.
Object-store layout and filesystem latency can therefore block an otherwise
compatible inventory. Tests may use an independent local transport clone with
packfiles while preserving the exact pinned tree and its reachable history;
this does not remove tracked files, repack the user's repository, skip guards
or certify arbitrary loose-object repositories within the deadline.

Equivalent metadata queries may share a bounded Git process: the two administrative directory paths from [rev-parse](https://git-scm.com/docs/git-rev-parse), stage/flag records from [ls-files](https://git-scm.com/docs/git-ls-files), and exact generated refs from [for-each-ref](https://git-scm.com/docs/git-for-each-ref). Each parser rejects malformed, duplicate or unexpected fields; only default `H`/stage-zero index entries are accepted. Ref observations are repeated around the exact ownership blob. Because enumeration can omit broken refs, an optional missing ref still requires the direct resolver and absence of a loose entry; dangling symbolic or corrupt refs are not authority to treat a resource as absent. Grouping does not cache support/ownership across operations, remove coherent snapshots or increase command/keeper budgets.

Attribute reads use documented [`check-attr --stdin -z --all`](https://git-scm.com/docs/git-check-attr): at most 256 paths/64 KiB input per process, with the existing 8 MiB output ceiling. Path/attribute framing, requested-path membership and duplicate pairs are checked; forbidden attributes remain forbidden. Source and working observations remain separate and are repeated at their original fences. This avoids Windows argv limits and the process multiplication of 16-path batches; it does not cache mutable attributes or increase deadlines.

Within each fresh repository-support observation, administrative and common
metadata roles are both canonicalized and path-checked. If they resolve to the
same directory, that current object store is walked once after those checks;
distinct linked-worktree directories are still inspected separately. No layout
observation survives the call. A subsequent call walks again and rejects a newly
unsupported entry. This groups aliased roles within one observation rather than
caching mutable object-store state across captures or authority decisions.

Capture combines the administrative directories, absolute index path and verified HEAD in one exact typed frame; the symbolic branch still comes from native `symbolic-ref`, preserving its recursive and detached/unborn behavior. At the final edge the index path is resolved again and both path and raw bytes must match. Index path identity also participates in equality of the three snapshots; it is not a new field in historical artifact revisions. Accepting a 64-character OID in the frame grammar does not enable repositories with an unsupported object-format extension.

Ownership checks can consume the canonical administrative/common directories returned by that same fresh support observation, rather than launching another discovery process. Each integration instance shares only its bounded executor, empty hook directory and keeper scope with its worktree service; configuration, object stores, refs, registration and snapshots are not cached between calls.

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

An explicitly opted-in host can set `ORCHESTRIX_INTEGRATION_DIAGNOSTICS=1` to measure Prepared validation. It emits at most five short stderr records per validation, containing only a fixed phase, static Git error code and duration. The sink must be drained or a regular file. No paths, record contents, command arguments, provider output or keeper bindings are logged; public errors and validation authority remain unchanged. The ignored Windows read-only diagnostic additionally requires an exact Prepared export and its SHA-256, confines its roots to that private fixture and verifies physical fingerprints before/after. It never approves, applies, opens the Store or reuses a historical keeper. A later successful inspection does not explain an earlier failure or renew its evidence.

The separate ignored `scoped_inspect_from_readonly_export` diagnostic accepts an exact immutable integration intent export and its SHA-256. It opens one new owned Job, performs one scoped read-only inspection without opening the Store, then confirms Job closure and unchanged physical fingerprints. Its watchdog is 120 seconds, independent of the production Job's unchanged 300-second lifetime. The fingerprint covers the union of the target, owned Git store and integration worktree, bounded to 16,384 entries and 160 MiB in total. A complete 54,451,461-byte source fixture measured 1,401 entries/138,787,246 bytes in that union; the old 64 MiB diagnostic bound failed before opening a Job. This test-only bound does not enlarge the production 64 MiB base, 6 MiB preserved-file, textual delta or process budgets. A successful later inspection neither reconciles the integration nor authorizes another preparation/application.

Git mutations and checks join the same private Windows keeper Job before running; each Git command retains its 15-second/8-MiB bounds. The existing keeper deadline is unchanged. No SQLite writer remains open while Git or checks execute. Final application journals intent, updates supported target files/index, compare-and-swaps the exact target ref, verifies the resulting target and records application confirmation before marking the Run Delivered. No reset, clean, stash, hook, filter, network operation or automatic conflict resolution is exposed.

Before `read-tree -m -u`, application verifies the approved logical target, runs only the fixed argv `update-index --refresh` in that same journaled Job, then verifies the logical target again. Git's [stat-cache refresh](https://git-scm.com/docs/git-update-index#_using_refresh) handles a file whose original bytes were restored after an edit without staging new content or modes. The executor rejects this command outside an owned Job and rejects extra arguments, including paths, `--add`, `--cacheinfo`, quiet/ignore-missing flags and `--really-refresh`. A refresh failure or failed final authorization/cancellation precondition prevents `read-tree` and target-ref CAS. Once the mutation phase has begun, its outcome may conservatively remain Uncertain even when only index metadata changed; no automatic retry is inferred from unchanged working bytes. After working files change, cancellation does not promise rollback: the already authorized CAS may finish, or the operation preserves its uncertain partial result.

Replays consult the original operation identity. A reservation interrupted before the journaled Job launch can terminate as Blocked without performing effects. Restart never repeats application or checks. Exact completed Git effects plus the fenced own-Job closure proof can complete missing database confirmation. A crash after file/index update but before target-ref CAS preserves the partial result as Uncertain and blocks admission for inspection; this profile does not automatically resume that ref operation or discard files. Missing/mismatched closure or Git evidence also stays blocked. Editing or removing the original attempt checkout after internal integration withdraws its physical verification without rewriting the independent integrated snapshot or a confirmed delivery.
