# Repository guide

## Start here

Check the branch, recent commits and local changes before editing; preserve unrelated work and never switch branches, reset, merge or force-push automatically. Read [docs/README.md](docs/README.md), then the opening **current** section of [DEVELOPMENT_STATUS.md](docs/DEVELOPMENT_STATUS.md), the relevant ticket in [DEVELOPMENT_BACKLOG.md](docs/DEVELOPMENT_BACKLOG.md) and its milestone/gates in [DEVELOPMENT_PLAN.md](docs/DEVELOPMENT_PLAN.md). These documents own development status; this guide does not maintain another status copy.

Read the applicable [contracts](docs/contracts/), [ADRs](docs/adr/) and design specifications before implementation. Accepted ADRs govern decisions; explicit supersession preserves earlier records. Historical reports describe their recorded revision, not necessarily HEAD. Verify documented claims against code and evidence.

## Responsibilities and boundaries

| Path | Responsibility |
| --- | --- |
| `crates/orchestrix-core/` | Portable Rust Core modules, integration tests and offline examples. Consult its [README](crates/orchestrix-core/README.md); domain logic must not depend on provider messages, Node, OS process APIs, Tauri or database types. |
| `tools/runtime-harness/` | Experimental Node protocol/adapter, replay, fake fixtures and Windows supervision proofs. It is not the production worker/Core. |
| `tools/provider-discovery/` | Provider-specific catalog discovery and bounded metadata tests; catalog results do not establish runtime entitlement. |
| `prototypes/desktop/` | HTML/CSS/JS interface studies, browser tests, locales and local assets. Not the production Tauri Desktop; do not promote/move it as though it were. |
| `docs/` | Current specifications, decisions, plan/backlog/status and dated research/evidence. Use the [index](docs/README.md). |
| `design-system/` | Approved visual foundations, components, motion, patterns and asset provenance. Preserve the approved shell and style. |
| `artifacts/`, `target/`, `node_modules/` | Ignored local outputs/caches, not deliverable source. Reference clones live outside this repository; see [catalog](docs/research/REFERENCE_REPOSITORIES.md). |

Desktop + Core, one account/serial execution, distinct connection/session/attempt identities, real-capability selection and manual control remain product constraints. Preserve Subscription Only without paid API fallback and [runtime vs Orchestrix context ownership](docs/CONTEXT_OWNERSHIP.md). Implement only the permitted ticket increment; examples/fakes/in-memory tests do not prove persistence, provider capabilities or production execution.

## Verification commands

Run the affected area's checks from the directory shown. Commands below are defined by the workspace/package scripts; browser checks require existing dependencies and Chrome. On Windows use `npm.cmd`; if Cargo is absent from the inherited PATH, use `& "$env:USERPROFILE\.cargo\bin\cargo.exe"`.

| Area / working directory | Commands |
| --- | --- |
| Rust / repository root | `cargo check --workspace --all-targets --locked --offline`; `cargo test --workspace --locked --offline`; `cargo build --workspace --examples --locked --offline` |
| Runtime boundary / `tools/runtime-harness` | `npm.cmd run check`; `npm.cmd test`; `npm.cmd run check:conformance`; `npm.cmd run test:rust-core` (build Rust examples first) |
| Windows transport / `tools/runtime-harness` | `npm.cmd run test:windows-transport`; `npm.cmd run test:windows-job` (own synthetic processes; Windows only) |
| Catalog discovery / `tools/provider-discovery` | `npm.cmd run check`; `npm.cmd test` |
| UI study / `prototypes/desktop` | `npm.cmd run check`; `npm.cmd test` (Playwright manages the local server); `npm.cmd run check:file`; `npm.cmd run test:discovery` |
| Edited documentation / repository root | `git diff --check`; verify relative link targets and relevant anchors. |

When already installed, use Rust formatting/Clippy for affected files/targets; do not mass-format unrelated code or silently install missing toolchain components. Offline Cargo requires cached locked dependencies. Report unavailable checks and pre-existing failures separately, without weakening acceptance criteria.

## Editing and evidence

- Preserve local changes, historical source reports, exact evidence bytes/hashes and unrelated files. Keep commits scoped to the increment.
- Check generated fixtures with `npm.cmd run check:conformance`. `export:conformance` regenerates expectations only after a reviewed semantic change, never to bless a failure. The portable input profile, LF attributes and source hash modes are intentional.
- UI locale/manifest generation is explicit through `npm.cmd run locales` / `npm.cmd run manifest`; regenerate only when relevant source changes require it. Do not rewrite historical manifests.
- Live inference, login, credential changes, active/paid probes and installations require explicit authorization. An earlier consumed live allowance is not renewable automatically. Do not execute downloaded reference projects' scripts or copy their code/assets without checking licensing and attribution.
- Name new files by responsibility: `snake_case` Rust modules, `kebab-case` ordinary documents, existing conventions for principal documents and numbered ADRs. Avoid mixed-purpose `utils`/`common` modules; do not rename existing files merely for visual uniformity or create empty future directories.
