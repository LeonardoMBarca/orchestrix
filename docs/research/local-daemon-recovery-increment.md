# Local daemon and recovery increment — OX-010/011

This record continues `cb13888` on `docs/development-plan`. Requirements and the website capacity section were committed separately as `a992768`; the restricted IPC/recoverable Windows Job boundary is `b6f9447`. [Current status](../DEVELOPMENT_STATUS.md) owns acceptance; this report records source, results and boundaries without converting synthetic execution into a new live Codex proof.

## Delivered interfaces

- API v1 and a CLI client share typed commands, snapshots, paged worker references, status, events/cursor, deterministic controls and recovery. One dedicated daemon per canonical database/user owns operational access and one serial worker. Reconnection never starts another scheduler.
- Schema 5 retains migrations 1–4 and adds stable daemon identity, admission state/generation, immutable command/completion journal and append-only API events. Domain-change references are committed in the same transaction as their domain events. Replays return the original record; changed content conflicts; incomplete commands are inspected after restart, never dispatched automatically.
- The restricted Windows named pipe verifies identity, rejects remote/second-instance access, limits frames and uses bounded overlapped I/O with completed cancellation. A read acknowledgment prevents disconnect from discarding an unread response. Same-user/admin processes remain inside the trust boundary.
- Startup reconciles before binding and starts suspended. Suspend blocks only new admissions; cancel targets the daemon's owned active worker. Provider failure never disables local controls. Shutdown has an independent 20-second own-host deadline: normal exit 0, deadline exit 3 with uncertainty retained.
- Recovery distinguishes never-sent, intent-only, confirmed dispatch and observed terminal outcome. Durable Preparing/keeper binding, SupervisorReady, dispatch intent and Checking bracket the critical launch windows. PID absence or name lookup cannot establish a remote terminal or safe local closure.
- A trusted sibling Rust keeper holds the exact restricted Job before any runtime launch. Runtime and authorized checks share it. Host identity is pinned by handle/creation identity. Before publishing a private atomic closure receipt, the keeper seals process admissions, terminates members and confirms active count zero. A receipt proves local scope closure only, not root exit, streams, provider result or task success.
- Exact-revision intervention records inspection or sets an eligible incident aside without changing its native result or artifacts. Explicit recheck can import a later keeper receipt. Missing/corrupt receipt, helper loss or legacy ownership gap remains blocked. Original command/task replay cannot retry the attempt.
- Basic continuity uses the existing deterministic Context Compiler over durable objective/criteria, policy/context sources and versioned artifact/check/review/approval facts. Sources, byte budget and omissions are explicit; provider transcript and hidden reasoning are unavailable. It grants no execution or session-transfer authority.

[API contract](../contracts/local-daemon.md), [transport ADR](../adr/0007-local-daemon-windows-transport.md) and [ownership ADR](../adr/0008-recoverable-process-ownership.md) specify the mechanisms and limits.

## Capacity versus future scheduling

[CAP-01/02/03 and CONT-01/02/03](../design/capacity-and-continuity.md) are incorporated into the existing milestones. This increment implements deterministic control/recovery/basic context, not role reservations, contextual multiagent scheduling or actual orchestrator transfer. Missing subscription telemetry remains unknown; a rate-limit error is not a remaining percentage. Historical connection observations retain their revision/validity and reset time alone cannot grant admission. Subscription Only never enables API billing/fallback.

Coverage is intentionally narrower than CAP-01 telemetry: daemon scenarios cover unknown quota and local controls after rate-limit/unavailability; existing policy/refresh tests cover critical metadata TTLs, connection revision invalidation, late replies and timeout/backoff. These are not tests of quantitative quota freshness. The current adapter does not consume a reported reset/window/balance, so no reset event or clock crossing can release admissions. Reported balances/windows, quota-specific stale/transient refresh and confirmed-exhaustion recovery remain future adapter/policy work; this batch must not claim those scenarios were implemented or tested.

The English website presents observable capacity, completion margins/priorities and continuity in the approved style. Its copy explicitly describes the direction under development. The initial documentation/site commit records 14/14 browser checks, direct-HTML desktop/mobile inspection and the regenerated 72-file manifest. No app redesign or extra assets were introduced.

## Verification and source attribution

Windows x64/MSVC, Rust/Cargo 1.99.0, Git 2.53.0.windows.2 and Node 24.19.0 are the observed tools. Cargo uses the existing lock/cache with `--locked --offline`; no dependency installation, login, inference or paid probe is part of this batch. Test helpers own their fixtures/processes; no personal database or competitor script is executed.

The first combined source inventory, S3, covers 83 files, including workspace metadata, SHA-256 `3f2849e54ad45afcc2437363108ca4978775a6e8d9ff9e99292ad35127e6c980`. Its precise input is all `rg --files crates` paths ending in `.rs`, `.toml` or `.json`, plus root `Cargo.toml`, `Cargo.lock` and `.gitattributes`, sorted with slash separators. Each line is `path + NUL + SHA256(LF-normalized UTF-8 bytes) + LF`; the inventory hash is SHA-256 of their concatenation. Ignored local logs/binaries/databases are not deliverable source.

The recovery parent ran with its worker/recovery/guardian/runtime/evaluator/process paths frozen. Later guardian additions are exclusively `#[cfg(test)]`; hashing the production prefix confirms identity with the matrix's source. The final daemon's collapsed condition and test observer are covered by the combined daemon regressions. This separates module-specific matrix evidence from a claim that every check ran on one earlier whole-workspace revision.

| Combined check on S3 | Observed result |
| --- | --- |
| `cargo check --workspace --all-targets --locked --offline` | Passed, 3.55 s after final lint corrections. |
| `cargo build --workspace --locked --offline` | Passed, 7.53 s; CLI and trusted helper built together. |
| `cargo build --workspace --examples --locked --offline` | Passed, 1.45 s. |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` | Passed, 0.85 s after correcting both conditions. |
| Rustfmt check | Passed on all 25 Rust files affected by this batch; no unrelated mass formatting. |
| Actual CLI client smoke | Passed: startup suspended, snapshot/resume/suspend/reconnect, immutable replay, four consecutive cursor events through `api --request`, shutdown exit 0. Empty private SQLite fixture, no runtime/Git/provider. |
| Runtime harness syntax/conformance check | Passed without export/regeneration: 245 cases, zero provider calls. |

The first combined workspace command was `cargo test --workspace --locked --offline -- --test-threads=1 --skip crash_windows_reconcile_without_pid_adoption_or_automatic_dispatch_and_restore_context`. The separately passed 917.26-second recovery parent was its only explicit name filter. It **failed**, with 110 selected tests passed and one failed before Cargo stopped: Core library 36, conformance 3, domain 45, evaluators 4, Git capture 7, worktrees 11, daemon 4 passed/1 failed. Three child helpers were ignored by their outer runners and explicitly invoked by their parents. Test build took 44.93 s; command wall time was 1,446.24 s. Worktrees alone took 849.02 s. This is not a green workspace result, and the later worker/runtime/persistence targets had not run in that command.

Focused implementation evidence, not a substitute for the final combined revision:

| Check | Observed result |
| --- | --- |
| Migration-focused library filter | 4 passed, 1.29 s; build 29.78 s. Populated V4 migration/event tests also require the final library run. |
| Pipe ACL/overlapped I/O tests | 2 passed, 0.11 s: protected SID DACL, restricted-token access denied, malformed/oversize/stalled peer and listener reuse. |
| Final daemon library tests | 4 passed, 0.29 s; build 59.65 s: journal/replay, lease scope and worker/watchdog spawn failures. |
| Five daemon scenarios during implementation | Four passed in a 436.64 s suite; its shutdown assertion failed. The corrected deterministic shutdown scenario then passed alone in 61.86 s, build 21.19 s. These are consolidated focused results, not a single green final suite. |
| Job admission fence | 1 passed, 0.25 s; build 1.38 s. Same executable/spec runs twice in an open Job, then association to the sealed Job fails before the child marker. |
| Private-file inherited ACL | 1 passed, 0.01 s; build 4.68 s. Restricted token cannot read the file; normal identity can after reverting. |
| Combined Windows process library | 5 passed, 1 helper ignored by the top-level runner and explicitly invoked by its parent, 1.67 s; build 0.10 s. Process Clippy all-targets with `-D warnings` passed in 5.16 s. This is the source committed as `b6f9447`. |
| Final recovery matrix | 1 parent passed, 917.26 s; build 28.25 s. Its ignored helper ran explicitly at 14 checkpoints: 13 Core crashes plus simultaneous keeper/Core loss. Real connected runtime/check processes supplied valid fenced closure receipts; dual loss preserved uncertainty/blocking. The production sources remained unchanged throughout this run. |
| Guardian negative tests added after that matrix | 3 passed, 1.14 s; build 9.66 s. Binding/generation/scope/authority/host/helper/directory mismatch, closed schema, invented streams/remote result, missing/malformed/8,193-byte receipt and a real junction are rejected. Only test code was appended to the guardian after the matrix. |

## Failures found and corrections

The first daemon run failed four scenarios with response-not-confirmed: pipe disconnection discarded bytes before the client read them. The bounded read acknowledgment fixes the transport, while durable command identity remains independent of that acknowledgment. A subsequent run terminated with `0xc0000409`; the original first assertion was not captured, so no specific FFI/double-panic cause is claimed. Fixture cleanup now preserves failed evidence rather than panicking again.

A test observer's arbitrary 30-second wait expired while the actual worker was still preparing Git. It now follows factual progress/owner state within the existing operation budgets; runtime/production deadlines and safety assertions were not relaxed. A cancellation assertion was corrected to inspect the original runtime evidence rather than an unrelated final summary, retaining Job-zero and closed-stream requirements.

The first recovery matrix failed with command-rejected after 192.75 s (build 46.57 s): the observer was supplied a contextual recovery reason where the normalized transport-loss classification was required. The normalized input is corrected; the contextual reason remains a separate durable observation.

The first shutdown case incorrectly assumed that post-cancel capture would always finish before the 20-second host deadline. Its nonzero exit code was not captured, so that old run is not asserted to prove code 3. A labelled synthetic-only stall now deterministically exercises the hard deadline, asserts exit 3 within 19–23 seconds, preserves the prior local/native observations and verifies uncertain recovery without redispatch. Codex has no injected stall and production limits are unchanged.

Ownership review rejected a missing Job name as closure proof. It also found that an empty receipt must prevent later process admission, checks must share the recoverable scope, readiness files must publish atomically, private file DACLs must inherit explicitly and SQLite restoration must validate receipts without filesystem I/O. The kernel fence, shared scope, durable Checking barrier, atomic publication, inheritable private DACL and pure validator address those specific gaps. DB authority is rejected inside repository/worktree roots before worker reservation.

The first final Clippy passes found two nested conditions in the daemon shutdown branch and its test observer. Both were collapsed as suggested, without changing conditions, deadlines or assertions; no warning was suppressed. Final daemon regressions cover that source, while the recovery matrix's production path is unchanged. Read-only review of the combined journal/cursor/admission/freshness logic found no remaining concrete issue; that review is distinct from execution evidence.

S3 failed the combined cancellation/quota scenario at `local_daemon.rs:193`: Client returned `response-not-confirmed-query-before-retry`. Cancellation had completed correctly with Job zero/closed streams. The next worker was Reserved at durable `git.plan`, with no second turn dispatched, no completion and empty daemon stderr. Four other daemon scenarios passed in that 445.97-second run. The first failure did not identify the native I/O class; its source and preserved fixture are not overwritten or retroactively called successful.

Review found a concrete Win32 contract violation: overlapped ReadFile/WriteFile used synchronous byte-count outputs and trusted them on immediate completion. Microsoft warns those counts can be erroneous; asynchronous completion counts must be confirmed. This defect requires correction, but discovery alone does not establish it as the cause of S3. Native completion coverage and the affected daemon scenarios must be revalidated, without increasing deadlines or weakening assertions; subsequent results retain their own source inventory.

S4 contains that correction plus private stage/native diagnostics, with source inventory SHA-256 `90e74ef64fb8ae5eadd296d90b6d88802639b5183b8997d88805b04a01bd9b22` (83 files under the same rule). Immediate and pending read/write completions passed four native IPC tests in 0.13 s (build 1.88 s). The cancellation/quota scenario passed alone in 174.12 s (build 29.37 s), but the five-scenario daemon suite then **failed two cases**, with three passed in 443.89 s (build 11.81 s). Both failures now report `read-pending`, Deadline, native error 995, wait result 258, 1,999 ms and zero transferred bytes. Thus the byte-count defect was real but did not resolve the observed deadline failure. No failed/uncertain turn was automatically retried by the daemon; these separately labelled offline tests use fresh synthetic fixtures.

Inspection found full state restoration/validation on every snapshot, another full restore during error completion before accepting a client, and synchronous diagnostic stderr writes before sending a response. Snapshot handler timings alone exclude that queue/completion/logging time. At S4 these paths required measurement and correction, without attributing the two-second failure to a particular path before evidence established it. S3/S4 remain failed revisions; their retained fixtures and logs are not overwritten.

A read-only profiler used the S4 library and consistent backup copies of the retained databases, preserving the originals and making no runtime/Git call. On the two-worker quota fixture, five full loads took 247/480/1,024/700/1,188 ms and five snapshots took 351/521/756/**2,341**/444 ms. Thus an actual snapshot exceeded the existing two-second response budget; this does not identify the exact call that caused either S4 failure. The measurements had concurrent machine activity and are not a general benchmark. Caching only unchanged revisions would not bound the first full restore after a write.

The correction reads compact historical worker pages in the cursor's read transaction, checking identity/ownership joins, bounded typed history, digests, ordinals, transitions and structural recovery proof. It preserves aggregate validation for startup, writes, decisions and detailed queries; compact status never grants eligibility or artifact certification. Error completion and duplicate-worker inspection use narrow existence queries. Default transport diagnostics stay memory-only; tests can explicitly opt into a regular file. Negative SetAside/history/projection cases and an undrained-stderr scenario accompany the change. Acceptance depends on the new tests, not this description.

## Remaining regression targets

The remaining-target command compiled on S4 before those three daemon-only files changed and **passed 88 tests**. The two subsequent source inventories differ only in `daemon.rs`, `persistence/daemon.rs` and `tests/local_daemon.rs`; worker/recovery/runtime/Git/evaluator/policy production sources remain identical. This allows the affected daemon tests to be rerun without claiming a new monolithic green workspace or unnecessarily repeating the separately passed recovery matrix and Git suites.

| Target | Result on the S4 build |
| --- | --- |
| `manual_worker` | 12 passed, 1,106.46 s. |
| `manual_worker_live` | Offline arithmetic fixture passed, 1.43 s; provider inference test ignored for lack of fresh opt-in. |
| `mechanical_queries` | 2 passed, 0.15 s. |
| `persistence_boundary` / `persistence_records` / `persistence_workflow` | 9 / 12 / 9 passed, 0.58 / 0.40 / 32.91 s. Workflow child helper explicitly invoked by its parent. |
| `policy_context` | 17 passed, 0.28 s. |
| `portable_input` | 3 passed, 0.41 s. |
| `runtime_production` | 23 passed, 11.02 s; separate real metadata opt-in not executed. |
| `worker_recovery` | Recovery parent explicitly filtered because its frozen source already passed the 917.26-second matrix; no new matrix claimed. |

Compilation for this command took 23.81 s. All tests ran serially; the large manual/Git regression time is observed test cost, not an inferred implementation-time estimate.

## Final revision S5 and acceptance

Source commit `1ccc0b78fe36a78d9236d0a7da24e25bb91626be`, inventory SHA-256 `afd351ce4a720eaf7963752407ffbb8734c25cec6709812b834f80a82721370e` (83 files, same normalized rule). Compared with S4, only the daemon implementation, compact persistence queries and daemon integration tests changed. The transport correction is committed separately as `291f5a7`; all production recovery/worker/runtime/Git/evaluator/policy sources retain the earlier test attribution.

| S5 check | Observed result |
| --- | --- |
| Core library | 39 passed, 21.43 s; build 34.16 s. New compact corruption/pagination/resync and default diagnostic cases included. |
| Process library | 7 passed, 0.41 s; build 0.05 s. Four IPC cases plus lease, private file DACL and kernel admission fence. |
| Workspace binary build | Passed, 17.89 s. |
| Strict workspace/all-targets Clippy | Passed with `-D warnings`, 28.48 s, without corrections or warning suppression. |
| Integrated daemon | **6 passed**, 441.82 s; build 8.93 s. Both previously failing cases passed, plus undrained stderr, exact shutdown deadline, invalid/stalled peers and two-client state/events/result consistency. |
| Final all-targets check / example build | Passed, 18.78 / 3.99 s. |
| Final Rustfmt / doc-tests | Formatting check passed on all 25 affected Rust files; doc-test command passed with zero runnable examples (0.26 s build). |
| Actual S5 CLI client | Passed in an empty private SQLite fixture: snapshot, resume/suspend, reconnect, immutable replay preserving suspension, four consecutive cursor events via `api`, shutdown exit 0. No runtime/Git/provider call. |
| Final conformance / JS → Rust bridge | 245 cases reproduced without export; 11/11 bridge tests passed in 7.51 s, zero provider dispatch. |

The Core/process library runners each list one auxiliary child as ignored; their owning tests invoke those helpers explicitly. The six integrated daemon scenarios ran after this source commit, without concurrent heavy suites or source changes. Failed S3/S4 results remain historical evidence, not retroactive passes.

At the end of S5 checks, before the legacy review below, inventory comparison found zero changes among all 83 sealed source files. The later compatibility correction is a distinct source revision; S5 evidence is not relabelled as its execution.

Profiling S5 on consistent copies of the same two retained fixtures, without Git/inference, measured Snapshot at 63/68/74 ms (quota fixture) and 35/55/68 ms (recovery fixture). Detailed Status retained full validation at 336/619/491 ms and 323/310/500 ms; Recovery at 331/813/508 ms and 311/303/1,056 ms. These observed samples stayed within the existing deadline and explain why the projection correction matters. They are not a general performance guarantee or a controlled comparative benchmark, and do not turn the failed S3/S4 suites green.

**S5 scope:** the six scenarios passed, but subsequent inspection found a legacy V4 eligibility gap; final acceptance therefore required S6 below. The daemon connects the existing manual-task policy/context/reservation/worktree/runtime/capture/check path, retains a durable result pending independent review, serves deterministic controls without provider quota and reconciles without automatic redispatch. Explicit local intervention must preserve uncertain remote outcome/artifacts. The basic continuity pack requires no synthesis agent. This is not a live daemon/Codex proof, quota-management engine, orchestrator handoff or final artifact application. OX-012 was not started here.

## Final review: legacy launch evidence

Inspection of `cb13888:worker.rs` found that the V4 worker connected before writing DispatchPlanned and ran evaluators after RuntimeObserved without the new Checking barrier. Therefore an old reservation without Preparing is not proof of no process launch; an old runtime Job-zero/closed-stream observation alone is not proof that no later evaluator started. The S5 predicate's `all(Preparing)` was vacuously true for the former, and its absence-of-Checking test also admitted the latter. The current-generation crash matrix did not cover those historical cases. This is a concrete eligibility bug, not a reason to reinterpret its passed current-generation results as legacy proof.

The correction introduces an optional typed launch-contract marker on new intents, omitted completely when absent so legacy serialized bytes/immutable rows remain unchanged. It binds the Ready-before-runtime and Checking-before-evaluator contract. Existing S5 Preparing evidence is recognized only with its explicit guard and validated keeper binding. Both pre-launch and original-runtime closure eligibility require that contract; a valid independent keeper closure receipt remains a separate proof. Missing legacy markers do not acquire authority by loading, checking a free PID or acknowledging inspection. The focused compatibility and follow-up regressions below close this correction's acceptance.

The raw legacy serialization/closed-enum unit test passed (1/1, build 16.95 s). A new file-backed focused parent passed all six scenarios in 82.68 s (build 1.86 s): legacy Reserved, legacy worktree, new marker, explicit early-S5 Preparing guard, unguarded Preparing, and legacy RuntimeObserved while its separately owned evaluator was demonstrably still active. Ineligible SetAside was rejected without killing/adopting that process; the test then terminated only its own check handle. Intent bytes and snapshots remained identical through inspection/reconciliation/reopen.

This fixture reconstructs valid historical-contract reservations through public APIs in new SQLite files, using a real child stopped at Reserved as its source. It does not claim execution by an old V4 binary. It never updates/drops immutable history or disables triggers. The first focused attempt failed before the last runtime case after 50.73 s (build 25.74 s): two fixture databases used the same Git operation identity in one repository. Ownership correctly rejected reassignment. The corrected test uses an explicit local `--no-hardlinks` clone and separate authorized worktree registry for that case; no production protection/assertion was weakened.

### S6 source and checks

The correction is committed as `e828a7fddfdda90ab6d7b98fcc511b952a483d2e`, source inventory SHA-256 `90995ef2af2f647f8e78cb09010051f044e40b55f6f364bdb01a5aca966014d6` (83 files, same rule). Changes from S5 are the typed intent field, its producer/eligibility helper, compact unit-fixture constructor and compatibility tests; no native process/guardian/evaluator/runtime/Git implementation changed. The earlier 14-point matrix remains attributed to its original source, while the new eligibility branches receive their six focused cases.

S6 Core library passed **40 tests** in 15.09 s (build 0.49 s), with its one separately invoked helper ignored by the outer runner. Strict workspace/all-targets Clippy passed with `-D warnings` in 16.69 s; rustfmt/diff-check passed. Workspace binary build passed in 0.19 s. Follow-up integrated worker/API results are recorded separately below rather than relabelling S5's six scenarios as a new full run.

| S6 follow-up | Observed result |
| --- | --- |
| Seven-model worker/reservation/reopen | 1 passed, 108.68 s; build 2.17 s. Full descriptor, policy candidates, diagnostics and historical snapshots retained with the new marker. |
| API lost reply/crash/recovery/new task | 1 passed, 115.96 s; build 2.30 s. No redispatch, explicit recheck/SetAside and a distinct new authorized task with Verified/NeedsReview outcome. |
| Final all-targets check / example build | Passed, 6.84 / 1.92 s. |
| Final formatting / doc-tests | All 25 affected Rust files passed rustfmt; doc-test command passed with zero runnable examples (0.16 s build). |
| Final conformance / JS → Rust bridge | 245 cases reproduced without export; 11/11 passed in 4.64 s, zero provider dispatch. |

**Final acceptance:** OX-010/011 are complete in the documented Windows/serial offline scope, including conservative legacy handling. The source-specific evidence above covers daemon control, cursor/replay, bounded shutdown, current crash windows, legacy gaps and deterministic continuity, without claiming that every historical suite was rerun on S6. S3/S4 and the first legacy-fixture failure remain recorded. The next released increment is OX-012; Desktop was not started. The provider/platform/capacity/application limits below remain open.

Final comparison found zero changes among S6's 83 sealed source files after the follow-up tests/checks. Documentation validation covered 15 changed documents, 431 relative targets and 36 Markdown anchors, with no failures; `git diff --check` passed. Acceptance/status commits following `e828a7f` contain documentation only.

## Remaining limits

Windows is the only validated transport/containment environment here. The API is not a network/multi-user service, database encryption or protection against the same user/administrator. Canonical paths define the scope; physical aliases/hard-link deployments and hostile metadata replacement remain outside the initial profile. Initial Store restoration still validates global state; bounded frames/pages do not establish unlimited-database responsiveness.

Keeper packaging is required. The combined runtime/check scope is bounded to 300 seconds. A lost/invalid receipt or simultaneous keeper/host loss remains conservative, with inspection and no automatic retry. Local cleanup never certifies remote provider completion. Git retains OX-006 root-process/profile limits and is not covered by the runtime/check receipt. Reconciliation exposes ownership warnings on closed historical work without licensing adoption/cleanup/retry; new independent tasks still need explicit authorization and complete validation.

Status is historical and does not synchronously run Git in the control loop; startup reconciliation revalidates required artifacts. No power-loss/disk-durability fault campaign, new live Codex run, second provider, macOS/Linux certification, full-repository dogfooding, autonomous planner, Tauri Desktop or final review/application was performed. OX-013/OX-014 and M0's remaining provider gates retain their criteria.
