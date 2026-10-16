# Orchestrix Desktop

Production Tauri/React client of the local daemon. The HTML study in `prototypes/desktop` remains the approved visual/interaction reference; it is not the runtime engine.

## Development

Windows x64/MSVC is the current validation target. Rust/Core and WebView2 must already be available. Dependency downloads and installations require explicit authorization under the repository guide.

From this directory, with the locked dependencies installed:

```powershell
npm.cmd run typecheck
npm.cmd run build
npm.cmd test
npm.cmd run test:browser
npm.cmd run test:worker-evidence
node tests/task-form.browser.mjs
node tests/integration-flow.browser.mjs
node tests/integration-invalidation.browser.mjs
npm.cmd run native:dev
```

`native:dev` builds/stages the Orchestrix-owned Core and supervisor, starts Vite at `127.0.0.1:1420`, and opens the native window. For the unsigned Windows package:

```powershell
npm.cmd run native:build
```

Before building or replacing sidecars on Windows, staging inspects process metadata for the exact owned executable paths under this checkout's `target/debug`, `target/release` and staged binaries directory. Loaded binaries stop staging with PID/path diagnostics; the script never terminates processes or requests administrator access. When a daemon command unambiguously identifies a native QA database within this checkout, the diagnostic prints its explicit graceful Shutdown command for inspection and manual execution. Other process command lines are not printed. Close the relevant window and finish or stop only its identified work before retrying.

Packaging targets current-user NSIS with the installed WebView2 runtime; it does not install a service, administrative startup task or another WebView2. Build/bundle output is under the workspace `target/` directory. Installer execution, signing and distribution retain their own validation gates.

When inspecting an extracted NSIS package, Core and supervisor must match their staged/build SHA-256 values exactly. Tauri CLI 2.12.3 replaces the Desktop's unique `__TAURI_BUNDLE_TYPE_VAR_UNK` token with `__TAURI_BUNDLE_TYPE_VAR_NSS` during packaging and then restores the original build executable ([official implementation](https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.12.3/crates/tauri-bundler/src/bundle.rs)). Verify that exact transformation and equality of all remaining bytes; do not ignore arbitrary binary differences or execute the installer merely to inspect its payload.

Versions are pinned in the manifests/lockfiles: Tauri/API 2.12.4, Tauri CLI 2.12.3, tauri-build 2.7.3, dialog 2.8.3, React 19.3.1, TypeScript 7.0.2 and Vite 8.3.4. Read the [current project status](../../docs/DEVELOPMENT_STATUS.md) for executed evidence and remaining acceptance.

The Desktop executable links the host as a Rust `rlib`. It does not build unused C/JNI/mobile `staticlib` or `cdylib` exports; the installer contains the executable and its two Core/supervisor sidecars.

## Boundaries

React → narrow native commands → typed local API client → daemon → Core. The renderer never opens SQLite, receives provider credentials or starts an agent. The Core and `orchestrix-supervisor` executables are staged as sidecars and resolved beside the Desktop binary. The daemon owns serial execution and remains independent of window lifetime.

Opening Home creates no conversation. Sending the first nonempty message persists one. Messages are instructions recorded for the user; execution requires preparing and explicitly authorizing a structured manual task. Projects, conversations, settings and worker evidence use Core persistence. A task currently requires a conversation attached to a project. The composer model preference is local to the open application session and is applied to the next prepared task; saved global defaults remain separate. No autonomous planner is presented as implemented.

For an exact verified and independently accepted result, contextual actions prepare a serial integration candidate in a separately granted directory, authorize its checks, approve the exact candidate/base, then explicitly authorize application to the original granted checkout. Only durable Core confirmation is shown as Delivered. This first profile requires one code task and a clean compatible target; conflicts or uncertain partial effects preserve work and block automatic replay. Inspection is historical, while a separate background revalidation checks physical state. Save/Create withdraw current candidate claims until that revalidation finishes. See the [Git/application contract](../../docs/contracts/git-worktrees.md#serial-integration-and-explicit-application) and current status for the tested scope.

Recovery also shows the daemon's blocking integration observations, including the source execution, artifact, base and target. These historical summaries do not authorize retry, application or SetAside, and never expose process ownership bindings. An absent detailed artifact response is presented as unavailable details rather than a claim that capture did not occur.

Conversation work cards distinguish chronological attempts of the same task, with exact-worker historical status from the compact snapshot. Expand their identity details to inspect Run/Task/Attempt/Worker and recorded revision; the card does not infer an implementation/correction role or claim current artifact validity. Inspecting evidence uses the linked worker identity, without restoring every worker just to render the conversation.

File grants originate in native dialogs, live only in the host, and do not follow from a saved project path. The supporting editor permits bounded UTF-8 text, explicit creation and version-checked writes. Windows handle sharing prevents other writers/renames during comparison and save. External changes are conflicts; the UI buffer remains available. Markdown renders text nodes without active HTML or remote resources. Opening `.env` never adds it to an agent Context Pack.

After a native grant, file labels distinguish a unique recorded project or worker relationship from an otherwise authorized file. These historical metadata labels confer no access; ambiguous or unavailable metadata stays unknown. Creating or saving in a worktree requests exact artifact revalidation and withdraws previous current-evidence claims, including failures after a possible disk mutation. Historical artifact snapshots remain read-only.

Host command records retain exact authorized operations and request hashes for lost replies/reopen, excluding credentials and rejected secret patterns. They are distinct from provider context and Core's authoritative journal. Subscription Only has no paid API fallback. The first validated subscription surface uses an explicitly selected, already authenticated Codex home; this is not OAuth or an independent account vault. Unknown quotas remain unknown. API adapters and unsupported connection modes are visibly unavailable.

## Explicit synthetic native QA

The nondefault Rust feature `test-fixtures` is reserved for controlled tests. Normal builds ignore `ORCHESTRIX_DESKTOP_FIXTURE_CONFIG` and expose no synthetic runtime registration action. A test build reads only an explicit fixture configuration prepared by `scripts/native-fixture.mjs`, uses separate app data, and labels the window as native synthetic validation.

The fixture exercises the real bridge, daemon, Git, SQLite, supervised processes, checks and review/correction pipeline. It never establishes provider authentication, model entitlement or a live Codex review proof. No fixture is injected into normal application opening, and no turn is sent until the normal authorization action.

To build the isolated release QA window, stage release sidecars with `node scripts/prepare-native.mjs --release`, then run `npm.cmd run tauri -- build --no-bundle --features test-fixtures`. Generate a fresh fixture with `node scripts/native-fixture.mjs --release` and pass its returned configuration path through `ORCHESTRIX_DESKTOP_FIXTURE_CONFIG` only to that QA process. A later normal package requires a fresh `npm.cmd run native:build` without the feature; do not distribute the QA executable as the normal application.

## Visual sources and icon provenance

Tokens/themes are imported from the approved `identity.css`, `styles.css` and `experience.css`; locales reuse the existing JSON catalogs. Imported assets are bundled by Vite. This keeps the approved Studio identity, eight themes, English default and Português/Español without copying the prototype's execution simulation.

The Windows `src-tauri/icons/icon.ico` and `icon.png` are format/size exports of `prototypes/desktop/assets/brand/transparent/symbol-indigo.png`, made with the pinned official Tauri CLI:

```powershell
npm.cmd run tauri -- icon ../../prototypes/desktop/assets/brand/transparent/symbol-indigo.png --output src-tauri/icons
```

Only the two referenced Windows/native formats are retained. Source identity and provenance are in the [visual asset catalog](../../docs/design/visual-asset-catalog.md); this conversion does not redesign the mark or validate other platforms.

The source icon SHA-256 is `84b66bf03100f1afcf8ae0578fb2b14d42227754f0af262f37463b8b62443679`.
