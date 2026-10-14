# OX-006 — Git worktrees and persisted provenance

Recorded **14 October 2026**, continuing `docs/development-plan` from the reviewed `54d1baa` baseline. OX-005 supplied SQLite commands/restoration; OX-006 adds actual Rust Git/filesystem operations integrated with that boundary. Ticket acceptance and the next permitted increment remain owned by [Development Status](../DEVELOPMENT_STATUS.md), [backlog](../DEVELOPMENT_BACKLOG.md) and [plan](../DEVELOPMENT_PLAN.md), not this historical report.

## Implemented path

The [Git service](../../crates/orchestrix-core/src/git/worktrees.rs) validates persisted Project/Repository/Run/Task/Attempt and path references, pins a full commit OID, commits intent, creates owned refs/worktree, observes the effect and confirms it durably. Git runs between short SQLite transactions. [Migration 3](../../crates/orchestrix-core/src/persistence/schema.rs) retains operations, append-only observations and immutable exact artifacts with existing foreign keys/idempotency/revision protection. No database transaction is claimed to undo Git.

Creation uses a locked `worktree add --no-checkout`, raw blob reads and exclusive writes, followed by `read-tree` without a working-tree checkout. It preserves the source branch/HEAD/index and pre-existing staged, unstaged and new files. Deterministic names alone cannot adopt a resource: exact intent/ownership refs, lock reason, base and registration must match. A retained completion ref distinguishes finished materialization from a partial directory. Canonical component checks exclude the source, common Git metadata and other registered checkouts, including a linked source whose common directory lies elsewhere.

If Git succeeds and SQLite confirmation fails, the same request inspects the retained resource and confirms it without duplicate creation. Partial/mismatched resources are preserved with a diagnostic rather than reset or deleted. Cleanup is deliberately narrower than capture: only an unchanged owned pinned base with no omissions can be removed. Unique commits and dirty/staged/untracked/ignored/excluded content block removal; refs retaining base/ownership/completion remain after the owned branch/worktree are removed. [The contract](../contracts/git-worktrees.md) specifies API, phases, recovery and limits.

[Capture](../../crates/orchestrix-core/src/git/capture.rs) retains exact base/index/working content separately, with explicit additions/deletions and untracked files. Rename handling is deletion plus addition. It produces a deterministic full-file unified patch, exact content/patch hashes and a complete artifact revision; index-only changes remain recorded even with an empty final patch. Three matching Git/filesystem observations reject the tested concurrent changes. Supported-pattern credential findings refuse the exact artifact instead of redacting bytes and misrepresenting their hash. Ignored/sensitive/dependency/build scopes are explicit omissions, not silently complete artifacts.

The [executor](../../crates/orchestrix-core/src/git/executor.rs) rebuilds the child environment, disables hooks/fsmonitor/external diff/textconv/config inheritance, blocks unsupported includes/filters/extensions/alternates and never exposes a network operation. Replace objects are disabled so a local replacement ref cannot silently change content behind the recorded base OID. Command output/deadline bounds and fixed errors are independent of future agent supervision.

## References consulted

The official Git manuals were consulted for [worktree lifecycle and `--no-checkout`](https://git-scm.com/docs/git-worktree), [transactional ref updates](https://git-scm.com/docs/git-update-ref), [raw object reads](https://git-scm.com/docs/git-cat-file), [index population](https://git-scm.com/docs/git-read-tree) and [environment/configuration controls](https://git-scm.com/docs/git). These sources explain Git mechanisms; owned fixtures establish what this Orchestrix implementation demonstrated.

The [existing Node worktree probe](../../tools/runtime-harness/worktree-probe.mjs) supplied concrete lessons about Windows canonical paths, a pinned base, a dirty source tree and binding artifact evidence to an attempt. Its former tracked-only diff was not treated as complete capture of new files. Node is not a dependency of this Rust boundary, and experimental/runtime evidence was not relabeled as production Git proof.

One narrow source question used the [local reference catalog](./REFERENCE_REPOSITORIES.md): how a Git worktree strategy handles repository discovery, canonical directory identity and removal. The read-only consultation examined OpenCode's [Git strategy](https://github.com/anomalyco/opencode/blob/c57c448d31c390880e06493f81b91a2c92536ad9/packages/core/src/worktree/git.ts) at **`c57c448d31c390880e06493f81b91a2c92536ad9`**, licensed [MIT](https://github.com/anomalyco/opencode/blob/c57c448d31c390880e06493f81b91a2c92536ad9/LICENSE). It discovers the repository, delegates Git operations and canonicalizes directory identities; removal forwards a force option. The useful comparison was canonical directory identity; Orchestrix does not adopt forced cleanup and retains its own Rust API and idempotent intent/effect/confirmation flow. No upstream implementation/assets were copied, and no reference-project script or installer was run. This was not a new competitor audit or product redesign.

## Verification evidence

Environment observed for this increment: **Windows, Git `2.53.0.windows.2`, Rust `1.99.0` and the existing MSVC toolchain**. No installation, provider login, inference, project check command or paid fallback was needed. Deterministic fixture edits are authored test operations, not simulated live AI responses.

The directed [capture suite](../../crates/orchestrix-core/tests/git_capture.rs) passed **7/7** tests in the observed run:

- Exact base/index/working bytes, modification/deletion/new file, hashes, repeated capture and explicit ignored/environment/dependency omissions.
- An index-only intermediate change with final working content equal to base.
- A deterministic concurrent edit to a previously unchanged file, rejected without a mixed artifact.
- Refusal of binary/known-secret content, nondefault index flags, filter attributes and bounded-size violations.
- Staged rename recorded as deletion plus addition.
- A generated patch including CRLF, missing final newline, accented/spaced paths and an empty new file accepted by `git apply --check`; the command did not apply it, and the source remained unchanged.

| Executed verification | Result |
| --- | --- |
| `cargo check --workspace --all-targets --locked --offline` | Passed. |
| `cargo build --workspace --examples --locked --offline` | Passed. |
| `cargo test --workspace --locked --offline` | 115 passed, zero failures. Four child helpers are ignored by the main runners and explicitly executed by successful parent tests. |
| `cargo test --package orchestrix-core --test git_worktrees clean --locked --offline` | Four passed after the final initial-cleanup reflog guard, including clean-base reset history and inactive-state checks. |
| `cargo test --package orchestrix-core --test git_worktrees removal_recovery_preserves_unique_branch_reflog_work_after_worktree_is_gone --locked --offline` | One passed after the final recovery guard: a disappeared worktree does not authorize deleting a branch whose reflog retains a unique commit. |
| Scoped rustfmt check; `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed after correcting the reported lint findings; no acceptance criterion disabled. |
| `npm.cmd run check:conformance` | 245 cases / 935 operations reproduced without regeneration or provider calls. |
| Documentation file links, `git diff --check`, original migrations 1/2 comparison | Passed; both original migration bodies remain byte-equivalent after checkout newline normalization. |

The workspace run included **9/9 integrated Git/SQLite parent tests**, both Git subprocess helpers and **7/7 capture tests**. It demonstrated two isolated attempts, exact preservation of dirty source HEAD/branch/index/staged/unstaged/new files, capture and persisted provenance in another process, original-key replay after edits/reopen, foreign branch/path/owner refusal, effect-before-confirmation recovery, capture races, safe/refused cleanup and adverse inherited Git environment variables. The additional reflog regression was then run with the three affected cleanup cases; these are overlapping executions, not four more independent workspace tests.

The final directed recovery test passed with a worktree already removed, a branch advanced and returned to base, and a unique commit retained only in its reflog. Retrying the original cleanup after SQLite reopen preserved that branch, exact log bytes, state and events. Coverage totals **117 distinct passed main-runner tests across these runs**; three cleanup cases repeat the workspace run. No shared JS runtime boundary changed, so the older JS/Windows runtime suite totals are not presented as newly executed here. The Git executor's own tests used actual Windows junctions and replacement refs, including refusal before an object-store junction could redirect a write.

## Limits and retained scope

The supported source/capture profile and numerical bounds are in [the contract](../contracts/git-worktrees.md). Executable modes, links, submodules, unsupported index/attribute/config states and changed binary/non-UTF-8 content are refused. Recognized-secret checks are conservative supported patterns, not universal detection. Excluded work is not implicitly archived. Very large repositories/files require a later bounded-profile extension, not silent partial success now.

Repeated observations demonstrate detection of the tested edits; they do not prove an atomic filesystem snapshot against a hostile writer, eliminate TOCTOU races, implement OS containment or certify power-loss recovery. Recovery completes the owned effect/confirmation cases; an interrupted partial materialization is retained for inspection rather than overwritten. Windows tests do not certify Linux/macOS support.

There is no merge/final application, project-test runner, agent supervisor, scheduler, daemon, Desktop, provider expansion or diagnostic refresh service here. Persisted artifacts do not approve Tasks or establish current runtime authentication. Subscription Only, one-account serial use, manual policy control and the separation of provider context from Orchestrix memory remain unchanged. OX-007/OX-008 and later tickets keep their existing dependencies and acceptance gates.
