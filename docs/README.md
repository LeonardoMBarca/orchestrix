# Orchestrix Documentation

Start with current status and the permitted ticket, then the specifications and accepted decisions relevant to that work. Implementation and verification commands are mapped in the root [repository guide](../AGENTS.md). Specifications describe intended behavior; dated evidence records what a particular revision demonstrated.

## Current development state

| Document | Use |
| --- | --- |
| [Development Status](./DEVELOPMENT_STATUS.md) | Authoritative current implementation, evidence, remaining gates and next increment. Read the opening current section before historical entries. |
| [Development Backlog](./DEVELOPMENT_BACKLOG.md) | Tickets, dependencies, deliverables and acceptance criteria. |
| [Development Plan](./DEVELOPMENT_PLAN.md) | Milestones and gates; follows consolidated D0/D1 work into technical implementation. |
| [Core implementation](../crates/orchestrix-core/README.md) | Actual Rust module/API, verification commands and boundaries. |
| [Runtime harness](../tools/runtime-harness/README.md) | Experimental adapter, fake/replay/conformance tooling and scoped runtime proofs. |
| [Desktop study](../prototypes/desktop/README.md) | Existing UI study and browser/direct-HTML workflows; not the production Desktop. |
| [Native Desktop](../apps/desktop/README.md) | Tauri/React application, restricted native bridge, local daemon and Windows validation/remaining gates. |
| [Local technical reference library](./research/REFERENCE_REPOSITORIES.md) | Official sources, licenses, pinned commits and component paths. Clones are outside Orchestrix in `../orchestrix-references/`; no competitor code is incorporated. |

Current status takes precedence over older reports' “next” or “pending” statements. [Portable runtime input acceptance](./research/portable-runtime-input-increment.md) records scoped OX-003 completion after the earlier Rust/adapter reports; [the pilot decision](./design/d1-pilot.md#piloto-reportado-pelo-responsável--1210) records the human D1 retest. Remaining production/provider gates are tracked in status and backlog rather than inferred from historical test totals.

## Current specifications and decisions

### Product and architecture

| Document | Responsibility |
| --- | --- |
| [Project Vision](./PROJECT_VISION.md) | Product thesis, goals, non-goals and execution philosophy. |
| [Architecture](./ARCHITECTURE.md) | Core/adapter boundaries, entities, task states, execution, verification and recovery. |
| [Technology Stack](./TECH_STACK.md) | Rust Core/Desktop baseline, unresolved choices and progressive platform validation. Proposed components are not all implemented. |
| [Desktop App and Project Workspace](./DESKTOP_APP_AND_PROJECT_WORKSPACE.md) | Desktop + Core, projects/repositories, chat entry and supporting file/editor/diff behavior; no presumed IDE evolution. |
| [Context Ownership](./CONTEXT_OWNERSHIP.md) | Providers own conversational context; Orchestrix owns durable project context and focused Context Packs. |
| [Capacity and Continuity](./design/capacity-and-continuity.md) | Preventive capacity evidence, local reserves, contextual priorities, deterministic controls and continuity; requirement IDs distinguish the current daemon/recovery batch from future scheduling/transfer. |
| [Orchestration Control Center](./ORCHESTRATION_CONTROL_CENTER.md) | Single-runtime support, configurable execution, profiles, routing, concurrency, review and approvals. [Policy example](./examples/orchestration-policy.example.yaml) illustrates versionable controls. |
| [Model and Reasoning Policy](./REASONING_AND_MODEL_POLICY.md) | Intent, capabilities, requested/effective settings, overrides and explanations. |
| [Routing Strategies](./design/routing-strategies.md) | Efficiency/Performance/Balanced/Custom, specialized presets and deterministic V1 selection for one or multiple accounts. |
| [Action Modes and Worker Profiles](./design/action-modes-and-worker-profiles.md) | Versioned objectives and executor profiles, distinct from resource strategies; local customization without hosted sharing. |
| [Roadmap](./ROADMAP.md) | Subsystem outlook; the Development Plan/backlog govern executable increments. |

### Integration contracts

| Document | Responsibility |
| --- | --- |
| [Runtime Contract v1](./contracts/runtime-contract-v1.md) | Provider-neutral wire/lifecycle 1.x, capabilities, configuration, identity, budgets, errors and separate runtime/supervisor facts. JS reference and Rust consumption are implemented in scoped OX-003 acceptance; this is not a production supervisor. |
| [Runtime Input Profile](./contracts/runtime-input-profile.md) | Revision 1 portable decoding, timestamp/Unicode/number grammar and compatibility. Codec revision is distinct from semantic wire version. |
| [Domain and States](./contracts/domain-and-states.md) | Implemented OX-004 identities, conversations, versioned snapshots, guarded task/Run completion and exact application evidence; in-memory domain, not persistence or execution infrastructure. [Acceptance evidence](./research/ox-004-domain-increment.md). |
| [Persistence Boundary](./contracts/persistence.md) | OX-005 transaction, idempotency, restoration, schema and privacy contract; implementation validation remains separate from the specification. |
| [Git Worktrees and Capture](./contracts/git-worktrees.md) | OX-006 owned worktrees, pinned bases, exact artifact provenance, recoverable Git/SQL effects and conservative cleanup. [Increment evidence](./research/ox-006-git-increment.md). |
| [Manual Worker, Policy and Runtime](./contracts/manual-worker.md) | OX-007/008/009 interfaces, deterministic policy/context, authorized metadata refresh, Windows supervision, serial durable worker, evaluators and CLI. Current status distinguishes synthetic evidence from live provider acceptance. |
| [Local Daemon and Recovery](./contracts/local-daemon.md) | OX-010/011 versioned API, user-restricted transport, command/event recovery, deterministic controls, scoped closure evidence, intervention and basic Context Pack. |
| [API Connection Contract](./design/api-connection-contract.md) | Two-stage subscription/API connection flow, provider-specific credentials and host boundary. |
| [Connection Diagnostics](./design/connection-diagnostics.md) | Available/unavailable/unknown evidence, scoped checks and recheck. Periodic/event-driven refresh is specified future work; active probes/production authentication retain their gates. |

### Desktop study and visual standards

| Document | Responsibility |
| --- | --- |
| [Activity Shell and Files](./design/activity-shell-revision.md) | Fixed activity rail, movable workspace, independent Sessions dock and supporting file/editor behavior. |
| [Shell Revision](./design/d1-shell-revision.md) | Session/project navigation, Settings, panels and progressive entry; dated verification sections retain their build context. |
| [Project and Conversation Direction](./design/d1-chat-direction.md) | Projects, UI conversations and native worker sessions are distinct. |
| [Friendly Entry and Progressive Experience](./design/d1-progressive-experience.md) | English public site, chat-first app entry and optional technical controls. Earlier test totals/pilot statements are historical; consult status and shell revisions. |
| [Interface Languages](./design/interface-languages.md) | English public pages; English default, Portuguese and Spanish app UI. Current controls: Settings → General for language and Settings → Themes for appearance. |
| [Approved Style Reference](../design-system/README.md) | Tokens, logos, components, motion, patterns and provenance. Preserves the night-flight website and calm Studio app. |
| [Visual Asset Catalog](./design/visual-asset-catalog.md) | Searchable image roles, aliases, source and approval status, including historical captures and unused concepts. |
| [D1 Design System](./design/d1-design-system.md) | Prototype design/interaction foundations, complemented by the approved shared style reference. |
| [Pilot Guide](./design/d1-pilot.md) / [Scenario Matrix](./design/d1-scenario-matrix.md) | Human evaluation route, feedback and remaining criteria; automation does not substitute for human acceptance. |
| [D1 Build Manifest](./design/d1-build-manifest.json) | Static study bytes/hashes, generated by [create-manifest.mjs](../prototypes/desktop/tools/create-manifest.mjs). Historical versions remain in `design/builds/`; direct HTML entry is checked separately. |

### Architecture Decision Records

Accepted ADRs govern architectural choices. Read supersession notices rather than treating an earlier hypothesis as current. Create future records from the [template](./adr/0000-template.md).

- [ADR-0001 — Single Runtime Is a First-Class Configuration](./adr/0001-single-runtime-first-class.md)
- [ADR-0002 — Model and Reasoning Policy](./adr/0002-model-and-reasoning-policy.md)
- [ADR-0003 — Desktop App and Editor-Agnostic Core](./adr/0003-desktop-app-editor-agnostic-core.md), with IDE/editor scope partially superseded by ADR-0005.
- [ADR-0004 — Context Ownership](./adr/0004-context-ownership.md)
- [ADR-0005 — Desktop Product Scope and Platform Boundaries](./adr/0005-desktop-product-scope-and-platform-boundaries.md)
- [ADR-0006 — Synchronous SQLite Persistence](./adr/0006-synchronous-sqlite-persistence.md)
- [ADR-0007 — Local Daemon and Windows Transport](./adr/0007-local-daemon-windows-transport.md)
- [ADR-0008 — Recoverable Owned-Process Closure](./adr/0008-recoverable-process-ownership.md)

## Historical research and implementation evidence

Sources retain their original results, failures, dates and limits. Offline replay is translation evidence, not a new live run; a passed fixture does not prove provider capability, durable storage or production execution. Later acceptance records point to the evolution without rewriting older reports.

### Runtime/Core increments

| Record | What it demonstrated at that revision / later update |
| --- | --- |
| [Local Daemon and Recovery — OX-010/011](./research/local-daemon-recovery-increment.md) | Versioned API/CLI, deterministic local controls, command/event recovery, owned closure receipts, explicit intervention and basic continuity. Windows processes/Git/SQLite with synthetic runtime; no new live provider proof. |
| [Integrated Manual Worker — OX-007/008/009](./research/manual-worker-increment.md) | Deterministic policy/context, Windows supervisor, Codex adapter and durable worker/check path. R9 records live edit/check, restart/context continuity and cancellation; previous offline and uncertain results retain their source revisions. Verified remains pending artifact review/application. |
| [SQLite Persistence — OX-005 Acceptance](./research/ox-005-persistence-increment.md) | Real file-backed durable domain, migrations, idempotency, rollback/lost reply, concurrent callers and subprocess continuation. Synthetic runtime facts do not establish production execution. |
| [Domain — OX-004 Acceptance](./research/ox-004-domain-increment.md) | Guarded identities/states and conversations in memory; OX-005 adds the separate persistence boundary without turning that older record into a storage proof. |
| [Portable Runtime Input — OX-003 Acceptance](./research/portable-runtime-input-increment.md) | Profile revision 1 and scoped acceptance: 245 cases/935 operations, six Rust, eleven bridge and 320 JS checks. Totals belong to that increment; status identifies subsequent work. |
| [Rust Runtime Contract Increment](./research/rust-runtime-contract-increment.md) | Authorized Rust setup and initial 104-case/309-operation conformance. The timestamp difference recorded there was resolved by subsequent portable input acceptance. |
| [Codex Contract Integration](./research/codex-contract-integration.md) | Experimental normalization, private mappings, bounded discovery and historical/synthetic replay. Rust consumption was pending there; later Rust/input records supply acceptance. |
| [Runtime Harness Results](./research/runtime-harness-results.md) | Initial experiment and later evidence, with failures preserved and subsequent proof linked. |
| [CLI/App-Server Comparison](./research/cli-app-server-comparison.md) | Authorized edit/check/resume comparisons and selection of the local single-account app-server adapter. |
| [Windows Process Containment](./research/windows-process-containment.md) | Synthetic helper/transport tests, failure diagnosis and authorized owned-Job force proof. Process termination remains separate from a remote runtime terminal. |
| [M0 Worktree and Resume Increment](./research/m0-resume-worktree-increment.md) / [Live Validation — 2026-10-12](./research/m0-live-validation-2026-10-12.md) | Blocked attempt and subsequent MXC edit/resume/interruption proof. Later Windows/comparison records address tree supervision and surface selection; new inference requires opt-in. |
| [Offline Lifecycle Validation](./research/offline-lifecycle-validation.md) | Earlier uncertain/terminal/process/backpressure preparation. Later proofs do not convert original synthetic results into live evidence. |

### Product alignment and interface increments

| Record | Scope / later update |
| --- | --- |
| [Product Alignment — 2026-10-13](./design/product-alignment-2026-10-13.md) | Owner clarification of Desktop/editor/platform scope and planned refresh. Current specifications/ADR-0005 govern; later Rust records resolved the toolchain/conformance preparation mentioned historically. |
| [D1 Prototype Delivery](./design/d1-delivery.md) | Initial Studio candidate, historical 36-case validation and simulation limits. Later shell revisions/pilot decision identify the subsequent build and human outcome. |
| [Website Modernization Research](./research/site-modernization-2026-10-09.md) | Official visual references, observations and approved site increments; shared design-system is now the implementation reference. |
| [API Provider Discovery Research](./research/api-provider-discovery-2026-10-10.md) | Official catalogs, credentials, capabilities and telemetry limitations; not a live entitlement proof. |
| [Antigravity Runtime Assessment](./research/antigravity-runtime-assessment.md) | OX-002: official CLI surface, subscription eligibility/billing gates and scoped local metadata; no adapter or inference proof. |
| [Resource Discovery Increment](./design/resource-discovery-increment.md) | Registration/catalog study, observed/unknown resources, host boundary and technical evidence. |
| [Connection Diagnostics Increment](./design/connection-diagnostics-increment.md) | Provider-specific diagnostics UI/logical flow and offline validation; production gates stay separate. |

### D0 and competitive research

- [Product and UX Research](./PRODUCT_AND_UX_RESEARCH.md): initial sourced survey and evaluation methodology.
- [D0 Conclusion](./research/d0-conclusion.md): consolidated decisions, methodology revision and D1 handoff.
- [Product Positioning](./research/product-positioning.md), [Attention and Review Benchmark](./research/attention-and-review-benchmark.md) and [Workflow/Risk Patterns](./research/workflow-risk-patterns.md): compared families, UX priorities and observable scenarios.
- [Landscape and Provenance](./research/landscape-and-provenance.md), [Conductor Dynamic Evidence](./research/conductor-dynamic-evidence.md), [Workflow Demonstrations](./research/workflow-demonstrations.md), [OpenCode Media Observation](./research/opencode-media-observation.md) and [Subscription/Account UX](./research/subscription-account-ux.md): source versions/licenses, observations and explicit gaps.
- [Competitive Update — 2026-10-09](./research/competitive-update-2026-10-09.md): Jackalope/Cobalt and multi-account comparisons; does not introduce implementation scope.

## Documentation conventions

Distinguish verified **facts**, accepted **decisions**, unvalidated **hypotheses** and **future scope**. Link implementation claims to code/build/evidence and keep results attached to their original revision. New documents use descriptive kebab-case names; principal documents and numbered ADRs retain established conventions. The proposed machine-oriented state tree belongs to future context/documentation work, not an existing implementation.
