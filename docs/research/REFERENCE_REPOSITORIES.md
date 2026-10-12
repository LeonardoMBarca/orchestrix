# Biblioteca de referências técnicas do Orchestrix

Catalogação inicial verificada em **12/10/2026**: nove clones superficiais e duas referências documentais. Não é nova etapa D0, auditoria completa ou evidência de execução dos runtimes do Orchestrix.

A biblioteca fica em `../orchestrix-references/`, fora do repositório. Clones em `repos/` não são dependências/submodules e não entram no Git do Orchestrix. Nenhum script, instalador, build ou teste upstream foi executado. As licenças foram consultadas antes do clone; os resumos abaixo não dispensam revisão de compatibilidade, avisos, assets e dependências por arquivo antes de reutilizar material.

| Projeto | Repositório oficial | Caminho na biblioteca | Commit de referência | Licença raiz |
| --- | --- | --- | --- | --- |
| Jackalope | [Jackalope-Dev/jackalope](https://github.com/Jackalope-Dev/jackalope) | `repos/jackalope` | `9bc0e9a3fa022f05fa1aac7a4bd4e78b64dabbba` | Apache-2.0 |
| Superset | [superset-sh/superset](https://github.com/superset-sh/superset) | `repos/superset` | `58b031f02b2f5c63e37541dd87fe14a93b84a2c9` | Elastic-2.0 |
| Vibe Kanban | [BloopAI/vibe-kanban](https://github.com/BloopAI/vibe-kanban) | `repos/vibe-kanban` | `d5cbb5380fa0b32e98ef9b8d987f63decce4be3a` | Apache-2.0 |
| OpenHands / Agent Canvas | [OpenHands/OpenHands](https://github.com/OpenHands/OpenHands) | `repos/openhands` | `26f590d1f710ff153ee22ec478348ddd1922716c` | MIT |
| Cline | [cline/cline](https://github.com/cline/cline) | `repos/cline` | `a48408909bf9b2ff7f620d420be8547fb509e8ed` | Apache-2.0 |
| Cline Kanban | [cline/kanban](https://github.com/cline/kanban) | `repos/cline-kanban` | `abd4912c27ce6b7f18b5a8106c145fd838e90cc4` | Apache-2.0 |
| OpenCode | [anomalyco/opencode](https://github.com/anomalyco/opencode) | `repos/opencode` | `c57c448d31c390880e06493f81b91a2c92536ad9` | MIT |
| cmux | [manaflow-ai/cmux](https://github.com/manaflow-ai/cmux) | `repos/cmux` | `c995e8337050e7dd4775e3927f4bba77b4282922` | GPL-3.0-or-later; BUSL-1.1 for listed server modules |
| Microsoft Conductor | [microsoft/conductor](https://github.com/microsoft/conductor) | `repos/microsoft-conductor` | `fd7428774fd955643158c1db27f4f5580c6e099a` | MIT |
| Conductor (conductor.build) | Código público oficial não localizado | `sem clone` | `não aplicável` | unknown |
| Cobalt (cobaltcode.ai) | Código público oficial não localizado | `sem clone` | `não aplicável` | unknown |

## Consulta prioritária para o plano

- **OX-001/OX-003/OX-007:** Jackalope (process ownership/Windows), Vibe Kanban (cliente Codex JSON-RPC) e Cline Kanban (adapters CLI/estados). No snapshot Vibe Kanban, follow-up usa `thread/fork`: isso não demonstra `thread/resume` da mesma identidade.
- **OX-006/OX-011:** worktrees, checkpoints e recuperação em Jackalope, OpenCode, Cline Kanban e Microsoft Conductor; validar os mesmos problemas com fixtures próprias do Orchestrix.
- **OX-010/OX-012:** separação Desktop/host/Core em Superset, Cline, Jackalope e OpenCode. A biblioteca não muda o ADR Desktop + Core Rust/Tauri.
- **OX-008/OX-016:** capabilities, modelo/thinking e contexto em OpenCode, Cline e Microsoft Conductor. Não inferir limites independentes a partir de múltiplas sessões.

Consultar o componente pertinente quando surgir um problema técnico; registrar pergunta, commit, arquivos examinados, decisão e limites em `notes/<slug>/`. Preservar Subscription Only, uma assinatura/execução serial e contexto interno do runtime separado da memória do Orchestrix. Não importar funcionalidades apenas porque aparecem nas referências.

## Atualização e anotações

O [README local](../../../orchestrix-references/README.md) e `tools/fetch-reference.py` descrevem o fetch explícito com profundidade 1. O comando preserva o checkout inicial e as anotações, verifica origin/HEAD e grava o commit/licença em `updates/`. Examinar a licença novamente e criar um worktree em `snapshots/` antes de promover outro snapshot. Não executar scripts de upstream, inicializar submodules ou usar reset/clean/merge automático. Catálogo inicial e registros anteriores permanecem disponíveis.

## Catálogo por projeto

### Jackalope

**Site oficial:** [https://jackalope.dev/](https://jackalope.dev/). **Disponibilidade:** public. **Versão:** 0.1.0 (apps/desktop/package.json; prerelease, not an assertion about an installer release).

**Stack identificada:** TypeScript; React 19; Zustand; Vite; Tailwind CSS; Tauri 2; Rust; Tokio; Axum; pnpm monorepo.

**Arquitetura inicial:** Desktop React frontend talks through typed clients/Tauri commands to a Rust coordinator and task runtime. Native modules separate queue admission, CLI execution, account profiles, attempt journals, process ownership, worktrees and reviewed integration receipts. Website, server and shared UI/knowledge packages are separate applications.

**Relação com o Orchestrix:** Coding-agent CLI adapters; Account profiles and quota-aware routing; Parallel worktree queues; Task event journals and recovery; Model/effort configuration; Human review and integration receipts; Project context and task-scoped tools.

**Licença/restrições:** Preserve the license, applicable attribution/NOTICE and modification notices when redistributing; the license includes patent conditions and does not grant trademark rights. Check separate notices and third-party dependency/asset licenses before any reuse.
Licença no commit: [LICENSE](https://github.com/Jackalope-Dev/jackalope/blob/9bc0e9a3fa022f05fa1aac7a4bd4e78b64dabbba/LICENSE). Clone `--depth 1 --no-tags`, branch `master`, HEAD `9bc0e9a3fa022f05fa1aac7a4bd4e78b64dabbba`; shallow e árvore limpa conferidos.

**Componentes para consulta:**

- [docs/ARCHITECTURE.md](https://github.com/Jackalope-Dev/jackalope/tree/9bc0e9a3fa022f05fa1aac7a4bd4e78b64dabbba/docs/ARCHITECTURE.md): Desktop/native boundaries, lock ordering, persistence and trust assumptions.
- [apps/desktop/src-tauri/src/commands/tasks/runtime.rs](https://github.com/Jackalope-Dev/jackalope/tree/9bc0e9a3fa022f05fa1aac7a4bd4e78b64dabbba/apps/desktop/src-tauri/src/commands/tasks/runtime.rs): Task runtime lifecycle; entry point for focused M0 integration comparison.
- [apps/desktop/src-tauri/src/commands/tasks/runners.rs](https://github.com/Jackalope-Dev/jackalope/tree/9bc0e9a3fa022f05fa1aac7a4bd4e78b64dabbba/apps/desktop/src-tauri/src/commands/tasks/runners.rs): Provider runner setup; study CLI launch policy alongside native task commands.
- [apps/desktop/src-tauri/src/commands/tasks/journal.rs](https://github.com/Jackalope-Dev/jackalope/tree/9bc0e9a3fa022f05fa1aac7a4bd4e78b64dabbba/apps/desktop/src-tauri/src/commands/tasks/journal.rs): Attempt event history; consult with storage.rs and journal_tests.rs for recovery.
- [apps/desktop/src-tauri/src/commands/process_control.rs](https://github.com/Jackalope-Dev/jackalope/tree/9bc0e9a3fa022f05fa1aac7a4bd4e78b64dabbba/apps/desktop/src-tauri/src/commands/process_control.rs): Bounded output and cancellation; Windows Job Objects own process trees.
- [apps/desktop/src-tauri/src/commands/integration.rs](https://github.com/Jackalope-Dev/jackalope/tree/9bc0e9a3fa022f05fa1aac7a4bd4e78b64dabbba/apps/desktop/src-tauri/src/commands/integration.rs): Snapshot/check-bound integration authorization and receipts.
- [apps/desktop/src-tauri/src/commands/agent_profiles.rs](https://github.com/Jackalope-Dev/jackalope/tree/9bc0e9a3fa022f05fa1aac7a4bd4e78b64dabbba/apps/desktop/src-tauri/src/commands/agent_profiles.rs): Account profile boundaries and sign-in configuration.
- [docs/LICENSING.md](https://github.com/Jackalope-Dev/jackalope/tree/9bc0e9a3fa022f05fa1aac7a4bd4e78b64dabbba/docs/LICENSING.md): Separate notices for fonts, bundled tools, branding and dependencies.

**Fontes da verificação:** [Official product website links to the Jackalope-Dev repository.](https://jackalope.dev/), [Public canonical repository and reciprocal official website link.](https://github.com/Jackalope-Dev/jackalope), [Apache-2.0 license read before cloning.](https://raw.githubusercontent.com/Jackalope-Dev/jackalope/master/LICENSE).

- Initial read-only catalog: no dependency installation, app build, provider login, runtime execution or upstream tests.
- README says native execution requires an approved Jackalope account; platform acceptance and signed installers are still in progress at this commit.
- Worktrees isolate Git changes, not OS privileges. The project's charging/account behavior is not a policy to import into Orchestrix.
- These modules are reference engineering, not evidence that Orchestrix runtime integration has passed OX-001.

### Superset

**Site oficial:** [https://superset.sh/](https://superset.sh/). **Disponibilidade:** public. **Versão:** 1.37.0 (apps/desktop/package.json).

**Stack identificada:** TypeScript; Electron; React; Bun; Turborepo; Vite; Tailwind CSS; tRPC; Drizzle; SQLite/local database; node-pty/xterm.

**Arquitetura inicial:** Electron main/renderer desktop shell fronts a shared host-service package used by other surfaces. Terminal-host processes and PTY sessions are separated from the UI; the host service owns workspace APIs, filesystem/Git operations, terminal transport and persistence. Shared packages contain agent launch/model metadata and interface contracts.

**Relação com o Orchestrix:** CLI agents in independent workspaces/worktrees; Persistent terminal sessions; Agent status and review workflows; Desktop-to-host service communication; Agent launch model/reasoning pickers; Workspace lifecycle and recovery; Shared APIs for desktop/CLI/MCP.

**Licença/restrições:** ELv2 restricts hosted/managed third-party access to a substantial set of the software's features, circumvention of license-key functionality, and removal/obscuring of notices. Redistribution must retain the terms and mark modifications. Do not treat it as a permissive open-source license or import implementation without a compatibility assessment.
Licença no commit: [LICENSE.md](https://github.com/superset-sh/superset/blob/58b031f02b2f5c63e37541dd87fe14a93b84a2c9/LICENSE.md). Clone `--depth 1 --no-tags`, branch `main`, HEAD `58b031f02b2f5c63e37541dd87fe14a93b84a2c9`; shallow e árvore limpa conferidos.

**Componentes para consulta:**

- [packages/host-service/src/index.ts](https://github.com/superset-sh/superset/tree/58b031f02b2f5c63e37541dd87fe14a93b84a2c9/packages/host-service/src/index.ts): Service exports and boundaries: app creation, authentication, Git providers and safety net.
- [apps/desktop/src/main/terminal-host/terminal-host.ts](https://github.com/superset-sh/superset/tree/58b031f02b2f5c63e37541dd87fe14a93b84a2c9/apps/desktop/src/main/terminal-host/terminal-host.ts): Owned terminal sessions, capped concurrent spawns, attach cancellation and kill timeouts.
- [apps/desktop/src/main/terminal-host/session.ts](https://github.com/superset-sh/superset/tree/58b031f02b2f5c63e37541dd87fe14a93b84a2c9/apps/desktop/src/main/terminal-host/session.ts): PTY session lifecycle; paired unit tests live beside implementation.
- [packages/host-service/src/runtime/git](https://github.com/superset-sh/superset/tree/58b031f02b2f5c63e37541dd87fe14a93b84a2c9/packages/host-service/src/runtime/git): Host-side Git abstractions and workspace operations.
- [packages/shared/src/agent-launch-request.ts](https://github.com/superset-sh/superset/tree/58b031f02b2f5c63e37541dd87fe14a93b84a2c9/packages/shared/src/agent-launch-request.ts): Cross-surface agent launch request contract and tests.
- [packages/shared/src/agent-models.ts](https://github.com/superset-sh/superset/tree/58b031f02b2f5c63e37541dd87fe14a93b84a2c9/packages/shared/src/agent-models.ts): Per-agent model/effort/launch catalog. Explicitly hand-maintained: compare catalog drift handling, not provider capability truth.
- [packages/host-service/test/integration/workspace-restore.integration.test.ts](https://github.com/superset-sh/superset/tree/58b031f02b2f5c63e37541dd87fe14a93b84a2c9/packages/host-service/test/integration/workspace-restore.integration.test.ts): Workspace restore scenarios without mistaking synthetic providers for live integration.
- [packages/host-service/test/integration/terminal.integration.test.ts](https://github.com/superset-sh/superset/tree/58b031f02b2f5c63e37541dd87fe14a93b84a2c9/packages/host-service/test/integration/terminal.integration.test.ts): Terminal/host-service integration test organization.

**Fontes da verificação:** [Official product website.](https://superset.sh/), [Public canonical project repository, official website link and matching package metadata.](https://github.com/superset-sh/superset), [Elastic License 2.0 read before cloning.](https://raw.githubusercontent.com/superset-sh/superset/main/LICENSE.md).

- Initial read-only catalog: no dependency installation, app build, provider login, runtime execution or upstream tests.
- Source is public under ELv2; its reuse restrictions require separate review from the Apache-2.0 references.
- README targets macOS, offers experimental Linux, and states Windows installers are not available; not a Windows acceptance reference by itself.
- The checkout is complete. core.longpaths=true is stored locally because deeply nested renderer paths exceed traditional Windows path lengths.
- Provider/model catalogs and hosted services in this product do not override Orchestrix capability discovery or Subscription Only policy.

### Vibe Kanban

**Site oficial:** [https://www.vibekanban.com/](https://www.vibekanban.com/). **Disponibilidade:** public. **Versão:** 0.1.45 (package.json and crates/db/Cargo.toml).

**Stack identificada:** Rust; Tokio; Axum; SQLx/SQLite; TypeScript; React; Vite; pnpm; Tauri desktop wrapper; Generated Rust-to-TypeScript types.

**Arquitetura inicial:** Rust workspace separates server/routes, executor adapters, services, session/process records, Git/worktree managers and desktop bridging. React frontends share web-core/UI packages. Executors normalize provider events and approvals; persisted sessions and execution processes are separate from workspace management.

**Relação com o Orchestrix:** CLI executor adapters and follow-up sessions; Codex app-server JSON-RPC; Model/reasoning options and discovery; Per-workspace branches/worktrees; Persisted execution logs and repository state; Diff review and feedback; Desktop/local service separation.

**Licença/restrições:** Preserve the license and applicable attribution/NOTICE, identify modifications when redistributing, and check patent conditions, trademarks and separate dependency/asset notices before any reuse.
Licença no commit: [LICENSE](https://github.com/BloopAI/vibe-kanban/blob/d5cbb5380fa0b32e98ef9b8d987f63decce4be3a/LICENSE). Clone `--depth 1 --no-tags`, branch `main`, HEAD `d5cbb5380fa0b32e98ef9b8d987f63decce4be3a`; shallow e árvore limpa conferidos.

**Componentes para consulta:**

- [crates/executors/src/executors/codex.rs](https://github.com/BloopAI/vibe-kanban/tree/d5cbb5380fa0b32e98ef9b8d987f63decce4be3a/crates/executors/src/executors/codex.rs): Codex app-server adapter, sandbox/approval configuration and follow-up thread forking.
- [crates/executors/src/executors/codex/client.rs](https://github.com/BloopAI/vibe-kanban/tree/d5cbb5380fa0b32e98ef9b8d987f63decce4be3a/crates/executors/src/executors/codex/client.rs): Typed JSON-RPC client, task-owned thread ID, turn events and cancellation token.
- [crates/executors/src/executor_discovery.rs](https://github.com/BloopAI/vibe-kanban/tree/d5cbb5380fa0b32e98ef9b8d987f63decce4be3a/crates/executors/src/executor_discovery.rs): Executor-specific option/capability discovery entry point.
- [crates/executors/src/model_selector.rs](https://github.com/BloopAI/vibe-kanban/tree/d5cbb5380fa0b32e98ef9b8d987f63decce4be3a/crates/executors/src/model_selector.rs): Model and reasoning option contracts; inspect per-provider evidence separately.
- [crates/services/src/services/execution_process.rs](https://github.com/BloopAI/vibe-kanban/tree/d5cbb5380fa0b32e98ef9b8d987f63decce4be3a/crates/services/src/services/execution_process.rs): Execution-process lifecycle above provider executors.
- [crates/worktree-manager/src/worktree_manager.rs](https://github.com/BloopAI/vibe-kanban/tree/d5cbb5380fa0b32e98ef9b8d987f63decce4be3a/crates/worktree-manager/src/worktree_manager.rs): Git worktree lifecycle and cleanup boundaries.
- [crates/db/src/models/session.rs](https://github.com/BloopAI/vibe-kanban/tree/d5cbb5380fa0b32e98ef9b8d987f63decce4be3a/crates/db/src/models/session.rs): Persisted application session model distinct from provider process records.
- [crates/desktop-bridge/src/lib.rs](https://github.com/BloopAI/vibe-kanban/tree/d5cbb5380fa0b32e98ef9b8d987f63decce4be3a/crates/desktop-bridge/src/lib.rs): Native desktop bridging module boundary.

**Fontes da verificação:** [Official website links to the BloopAI GitHub project.](https://www.vibekanban.com/), [Public canonical repository with reciprocal product website and maintained source.](https://github.com/BloopAI/vibe-kanban), [Apache-2.0 license read before cloning.](https://raw.githubusercontent.com/BloopAI/vibe-kanban/main/LICENSE).

- Initial read-only catalog: no dependency installation, app build, provider login, runtime execution or upstream tests.
- Official README/site announce sunsetting and community maintenance; record this state rather than assume an actively operated service.
- At this commit, the Codex follow-up path calls thread/fork: a new thread is not proof of strict same-thread resume. Use this distinction when validating Orchestrix OX-001.
- The code's protocol bindings are version-specific; derive Orchestrix behavior from the installed official runtime surface and independent live evidence.

### OpenHands / Agent Canvas

**Site oficial:** [https://www.openhands.dev/](https://www.openhands.dev/). **Disponibilidade:** public. **Versão:** @openhands/agent-canvas 1.27.0 (package.json).

**Stack identificada:** TypeScript; React 19; React Router 7; Vite; Tailwind CSS; Zustand; TanStack Query; Electron; Node.js launchers; Vitest; Playwright.

**Arquitetura inicial:** Frontend/control center separado do Agent Server. Adaptadores em src/api traduzem ações para APIs do backend escolhido; o SDK Python externo é responsável por executar agentes, ferramentas e isolamento. Launchers Node coordenam frontend, ingress e serviços locais; Electron empacota a interface. Há backends locais, remotos e cloud, com estado de UI separado do executor.

**Relação com o Orchestrix:** Escolha e validação de backend; Conversas e perfis de agentes; Integração de agentes ACP; Modelos de sessão anunciados pelo runtime; Files/editor e configurações; Retomada de streams e supervisão dos processos locais.

**Licença/restrições:** Manter aviso de copyright e a licença em cópias ou porções substanciais reutilizadas.; Sem garantia; conferir licenças de dependências, assets e componentes específicos antes de qualquer incorporação.
Licença no commit: [LICENSE](https://github.com/OpenHands/OpenHands/blob/26f590d1f710ff153ee22ec478348ddd1922716c/LICENSE). Clone `--depth 1 --no-tags`, branch `main`, HEAD `26f590d1f710ff153ee22ec478348ddd1922716c`; shallow e árvore limpa conferidos.
O snapshot atual deste repositório é Agent Canvas, não o antigo monolito Python.
O SDK/Agent Server, cliente TypeScript e automações vivem em outros repositórios oficiais e não foram baixados neste catálogo; a licença deste clone não deve ser presumida para serviços comerciais ou código externo.

**Componentes para consulta:**

- [docs/architecture.md](https://github.com/OpenHands/OpenHands/tree/26f590d1f710ff153ee22ec478348ddd1922716c/docs/architecture.md): Fronteiras entre controle visual, servidor de agentes e automação; serve à decisão Desktop + Core.
- [scripts/dev-process-utils.mjs](https://github.com/OpenHands/OpenHands/tree/26f590d1f710ff153ee22ec478348ddd1922716c/scripts/dev-process-utils.mjs): Spawn sem shell, resolução de binário Windows, sinalização de árvore de processos e registro de shutdown; conferir limites de cancelamento antes de adaptar ideias.
- [__tests__/scripts/dev-process-utils.test.ts](https://github.com/OpenHands/OpenHands/tree/26f590d1f710ff153ee22ec478348ddd1922716c/__tests__/scripts/dev-process-utils.test.ts): Testes correspondentes de supervisão e shutdown, sem depender de um agente live.
- [src/api/agent-server-compatibility.ts](https://github.com/OpenHands/OpenHands/tree/26f590d1f710ff153ee22ec478348ddd1922716c/src/api/agent-server-compatibility.ts): Probe de disponibilidade/versão, timeout e erros distintos para backend desconhecido, incompatível ou não autenticado.
- [src/api/conversation-service/agent-server-conversation-service.api.ts](https://github.com/OpenHands/OpenHands/tree/26f590d1f710ff153ee22ec478348ddd1922716c/src/api/conversation-service/agent-server-conversation-service.api.ts): Adaptação da conversa ao cliente oficial do Agent Server.
- [src/utils/session-seq-cursor.ts](https://github.com/OpenHands/OpenHands/tree/26f590d1f710ff153ee22ec478348ddd1922716c/src/utils/session-seq-cursor.ts): Cursor de replay contíguo, evitando pular eventos fora de ordem ao reconectar um stream.
- [src/hooks/query/use-acp-session-models.ts](https://github.com/OpenHands/OpenHands/tree/26f590d1f710ff153ee22ec478348ddd1922716c/src/hooks/query/use-acp-session-models.ts): Consulta de modelos expostos pela sessão ACP, sem tratar catálogo estático como capacidade comprovada.
- [electron/main.mjs](https://github.com/OpenHands/OpenHands/tree/26f590d1f710ff153ee22ec478348ddd1922716c/electron/main.mjs): Shell desktop e ciclo de vida dos serviços usados pela UI.

**Fontes da verificação:** [Official verification source](https://www.openhands.dev/), [Official verification source](https://github.com/OpenHands/OpenHands), [Official verification source](https://api.github.com/repos/OpenHands/OpenHands), [Official verification source](https://api.github.com/repos/OpenHands/OpenHands/license), [Official verification source](https://github.com/OpenHands/OpenHands/blob/26f590d1f710ff153ee22ec478348ddd1922716c/README.md), [Official verification source](https://github.com/OpenHands/OpenHands/blob/26f590d1f710ff153ee22ec478348ddd1922716c/docs/architecture.md).

- Verificado pelo link GitHub do site oficial e metadata do GitHub; o endereço solicitado difere apenas na capitalização canônica.
- Leitura inicial de README, arquitetura, manifests e alguns pontos de integração; nenhuma auditoria completa ou execução do projeto.
- Uma adaptação existente é referência de engenharia, não prova de que recursos, login, quotas ou Subscription Only sejam suportados pelo Orchestrix.

### Cline

**Site oficial:** [https://cline.bot/](https://cline.bot/). **Disponibilidade:** public. **Versão:** CLI 3.0.70; @cline/core 0.0.92 (apps/cli/package.json; sdk/packages/core/package.json).

**Stack identificada:** TypeScript; Bun workspaces; Node.js SDK; React; VS Code extension/webview; Tauri desktop shell; Bun desktop sidecar; Next.js desktop UI; SQLite session storage; Vitest.

**Arquitetura inicial:** Monorepo com SDK compartilhado (core/agents/llms e outros pacotes), CLI e clientes distintos. O Desktop Tauri usa um sidecar Bun que se conecta ao Hub canônico compartilhado; o Core mantém runtime e persistência das sessões. A UI comunica comandos/eventos via HTTP/WebSocket, com respostas correlacionadas e aprovação de ferramentas retornando ao cliente proprietário da sessão.

**Relação com o Orchestrix:** Separação Desktop/Core e Hub de sessões; Modo interativo e headless; Persistência, fork, checkpoint e retomada; Catálogo de provedores/modelos; Normalização de reasoning por modelo; Aprovação de ferramentas; Worktrees e equipes de agentes.

**Licença/restrições:** Redistribuição exige licença, preservação de avisos aplicáveis e identificação de arquivos modificados; incluir atribuições de NOTICE quando houver.; A concessão de patentes tem condições; a licença não concede uso de marcas além da atribuição usual.; Conferir dependências, assets, arquivos específicos e compatibilidade com a licença do Orchestrix antes de incorporar material.
Licença no commit: [LICENSE](https://github.com/cline/cline/blob/a48408909bf9b2ff7f620d420be8547fb509e8ed/LICENSE). Clone `--depth 1 --no-tags`, branch `main`, HEAD `a48408909bf9b2ff7f620d420be8547fb509e8ed`; shallow e árvore limpa conferidos.
A licença adicional de apps/vscode/LICENSE também é Apache-2.0 no snapshot observado.
O README declara que o plugin JetBrains não está publicado como código aberto; serviços hospedados e esse plugin não estão incluídos neste clone.

**Componentes para consulta:**

- [apps/examples/desktop-app/sidecar/ARCHITECTURE.md](https://github.com/cline/cline/tree/a48408909bf9b2ff7f620d420be8547fb509e8ed/apps/examples/desktop-app/sidecar/ARCHITECTURE.md): Contrato de comandos/respostas/eventos e fronteiras entre UI, sidecar e Hub; referência especialmente útil para Desktop + Core.
- [sdk/packages/core/src/hub](https://github.com/cline/cline/tree/a48408909bf9b2ff7f620d420be8547fb509e8ed/sdk/packages/core/src/hub): Descoberta/conexão, daemon e hosts do runtime compartilhado; consultar sob demanda, não pressupor que o Orchestrix precise replicar o Hub.
- [sdk/packages/core/src/session/services/persistence-service.ts](https://github.com/cline/cline/tree/a48408909bf9b2ff7f620d420be8547fb509e8ed/sdk/packages/core/src/session/services/persistence-service.ts): Persistência das sessões separada da apresentação da UI.
- [sdk/packages/core/src/session/checkpoint-restore.ts](https://github.com/cline/cline/tree/a48408909bf9b2ff7f620d420be8547fb509e8ed/sdk/packages/core/src/session/checkpoint-restore.ts): Recuperação/checkpoint de uma sessão e testes próximos; distinguir histórico do runtime da memória do Orchestrix.
- [sdk/packages/llms/src/providers/model-capabilities.ts](https://github.com/cline/cline/tree/a48408909bf9b2ff7f620d420be8547fb509e8ed/sdk/packages/llms/src/providers/model-capabilities.ts): Mapeamento tipado de capacidades do catálogo para capacidades do gateway, com tratamento de ausência de informação.
- [sdk/packages/llms/src/providers/routing/reasoning-options.ts](https://github.com/cline/cline/tree/a48408909bf9b2ff7f620d420be8547fb509e8ed/sdk/packages/llms/src/providers/routing/reasoning-options.ts): Normalização de intenção de reasoning para controles reais de provider/modelo; não é prova live de suporte.
- [apps/cli/src/runtime/run-agent.ts](https://github.com/cline/cline/tree/a48408909bf9b2ff7f620d420be8547fb509e8ed/apps/cli/src/runtime/run-agent.ts): Execução headless com eventos JSON, timeout, abort/stop e handlers de sinal.
- [apps/cli/src/utils/worktree.ts](https://github.com/cline/cline/tree/a48408909bf9b2ff7f620d420be8547fb509e8ed/apps/cli/src/utils/worktree.ts): Fluxo CLI de worktrees e testes adjacentes.
- [sdk/packages/llms/src/tests/provider-vcr/README.md](https://github.com/cline/cline/tree/a48408909bf9b2ff7f620d420be8547fb509e8ed/sdk/packages/llms/src/tests/provider-vcr/README.md): Organização de fixtures gravadas e distinção entre testes offline e testes live de provedores.

**Fontes da verificação:** [Official verification source](https://cline.bot/), [Official verification source](https://github.com/cline/cline), [Official verification source](https://api.github.com/repos/cline/cline), [Official verification source](https://api.github.com/repos/cline/cline/license), [Official verification source](https://github.com/cline/cline/blob/a48408909bf9b2ff7f620d420be8547fb509e8ed/README.md), [Official verification source](https://github.com/cline/cline/blob/a48408909bf9b2ff7f620d420be8547fb509e8ed/apps/examples/desktop-app/sidecar/ARCHITECTURE.md).

- Autenticidade cruzada com cline.bot, metadata oficial GitHub e manifests que apontam para cline/cline.
- No snapshot atual, o SDK já está em sdk/packages e os clientes em apps; não usar o layout antigo da extensão como mapa permanente.
- Catalogação inicial; não foram instaladas dependências, inicializados submodules nem executados testes/scripts do concorrente.
- Não assumir que a seleção de provider, políticas de cobrança ou integrações próprias do Cline atendam Subscription Only do Orchestrix.

### Cline Kanban

**Site oficial:** [https://github.com/cline/kanban](https://github.com/cline/kanban). **Disponibilidade:** public. **Versão:** kanban 0.1.70 (package.json).

**Stack identificada:** TypeScript; Node.js >=22; React 18; Vite; Tailwind CSS/Radix UI; tRPC; WebSocket; node-pty/xterm; Electron desktop shell; Vitest; Playwright.

**Arquitetura inicial:** Runtime Node local é a fonte de verdade para projetos, sessões, worktrees e Git. A UI React envia comandos tRPC e recebe atualizações por WebSocket. Há um caminho PTY para CLIs e outro SDK nativo para Cline, expostos por uma API de runtime comum. Cada tarefa pode usar worktree isolado; o shell Electron gerencia processos do runtime.

**Relação com o Orchestrix:** Registro e descoberta de CLIs; Sessões paralelas e retomada por runtime; Estados de execução/revisão/interrupção; Worktree por tarefa e recuperação de patches; Diffs, checkpoint e feedback; Comunicação UI/runtime e encerramento de processos.

**Licença/restrições:** Redistribuição exige licença, preservação de avisos aplicáveis e identificação de arquivos modificados; incluir atribuições de NOTICE quando houver.; A licença não concede uso das marcas; verificar dependências/assets e compatibilidade antes de eventual incorporação.
Licença no commit: [LICENSE](https://github.com/cline/kanban/blob/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/LICENSE). Clone `--depth 1 --no-tags`, branch `main`, HEAD `abd4912c27ce6b7f18b5a8106c145fd838e90cc4`; shallow e árvore limpa conferidos.
Catálogo inicial encontrou licença raiz Apache-2.0; não representa auditoria jurídica de todas as dependências ou assets.

**Componentes para consulta:**

- [docs/architecture.md](https://github.com/cline/kanban/tree/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/docs/architecture.md): Fronteira explícita UI/runtime e diferença entre agente SDK e agente CLI/PTY.
- [src/terminal/agent-registry.ts](https://github.com/cline/kanban/tree/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/src/terminal/agent-registry.ts): Descoberta de binários e construção de comandos por agente.
- [src/terminal/agent-session-adapters.ts](https://github.com/cline/kanban/tree/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/src/terminal/agent-session-adapters.ts): Adaptação de sessões e identificação/retomada específica de cada CLI.
- [src/terminal/session-state-machine.ts](https://github.com/cline/kanban/tree/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/src/terminal/session-state-machine.ts): Transições tipadas que distinguem processo encerrado, erro, interrupção e revisão.
- [src/terminal/session-manager.ts](https://github.com/cline/kanban/tree/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/src/terminal/session-manager.ts): Ciclo de vida e restart das sessões PTY, com testes adjacentes.
- [src/workspace/task-worktree.ts](https://github.com/cline/kanban/tree/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/src/workspace/task-worktree.ts): Criação isolada, preservação/reaplicação de patches, locking e recuperação de worktree; avaliar políticas destrutivas e symlinks antes de adaptar qualquer ideia.
- [src/trpc/runtime-api.ts](https://github.com/cline/kanban/tree/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/src/trpc/runtime-api.ts): Superfície comum de comandos consumida pela UI.
- [src/server/process-termination.ts](https://github.com/cline/kanban/tree/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/src/server/process-termination.ts): Tratamento de timeout e encerramento de árvore de processos no Windows.
- [packages/desktop/src/runtime-orchestrator.ts](https://github.com/cline/kanban/tree/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/packages/desktop/src/runtime-orchestrator.ts): Orquestração do processo Core no shell Electron.
- [test/integration/shutdown-coordinator.integration.test.ts](https://github.com/cline/kanban/tree/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/test/integration/shutdown-coordinator.integration.test.ts): Provas de encerramento e limpeza de processos; ponto útil para a evidência Windows exigida em OX-001.
- [test/integration/task-worktree.integration.test.ts](https://github.com/cline/kanban/tree/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/test/integration/task-worktree.integration.test.ts): Provas de isolamento e operações Git de worktree sem confundir com validação de um runtime live.

**Fontes da verificação:** [Official verification source](https://github.com/cline/kanban), [Official verification source](https://api.github.com/repos/cline/kanban), [Official verification source](https://api.github.com/repos/cline/kanban/license), [Official verification source](https://github.com/cline/kanban/blob/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/README.md), [Official verification source](https://github.com/cline/kanban/blob/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/docs/architecture.md), [Official verification source](https://github.com/cline/kanban/blob/abd4912c27ce6b7f18b5a8106c145fd838e90cc4/package.json).

- Repositório público da mesma organização oficial cline; manifests e README identificam Cline Bot Inc. e o próprio endereço do projeto.
- É um research preview: o README menciona bypass de permissões, hooks experimentais e automação de commit/PR; esses comportamentos não são decisões importadas para Orchestrix.
- Leitura somente de documentação, manifests e pontos selecionados de runtime; nenhum launcher, instalador ou teste foi executado.
- O clone não inicializa submodules. Referência técnica de adaptação CLI não comprova suporte oficial de nenhum recurso do Orchestrix.

### OpenCode

**Site oficial:** [https://opencode.ai/](https://opencode.ai/). **Disponibilidade:** public. **Versão:** 2.0.27 (root and core/desktop manifests; development snapshot, not inferred release tag).

**Stack identificada:** TypeScript; Bun workspaces / Turborepo; Effect; SolidJS / Vite; Electron desktop; SQLite / Drizzle; provider packages and model schemas.

**Arquitetura inicial:** Monorepo separates core services, HTTP server, typed protocol/client/schema, CLI, shared web app and Electron main/preload/renderer. Session services persist state and emit events; desktop manages a background service/sidecar. This snapshot differs from older Tauri descriptions.

**Relação com o Orchestrix:** Sessions and project-scoped persistence; Worktrees and file-access/permission boundaries; Provider/model variants and reasoning capabilities; Shared Desktop/Core protocol and event feed; PTY/process management and retry handling.

**Licença/restrições:** Preserve copyright and MIT license notices with copies or substantial portions. Inspect dependency and file-level notices before any reuse; branding is not automatically licensed.
Licença no commit: [LICENSE](https://github.com/anomalyco/opencode/blob/c57c448d31c390880e06493f81b91a2c92536ad9/LICENSE). Clone `--depth 1 --no-tags`, branch `dev`, HEAD `c57c448d31c390880e06493f81b91a2c92536ad9`; shallow e árvore limpa conferidos.

**Componentes para consulta:**

- [packages/core/src/session/runner](https://github.com/anomalyco/opencode/tree/c57c448d31c390880e06493f81b91a2c92536ad9/packages/core/src/session/runner): Session execution, model resolution and retry boundaries.
- [packages/core/src/session.ts](https://github.com/anomalyco/opencode/tree/c57c448d31c390880e06493f81b91a2c92536ad9/packages/core/src/session.ts): Session/store/execution service boundary and event integration.
- [packages/core/src/model-resolver.ts](https://github.com/anomalyco/opencode/tree/c57c448d31c390880e06493f81b91a2c92536ad9/packages/core/src/model-resolver.ts): Resolve supported model variants and explicit capability/configuration errors.
- [packages/core/src/model.ts](https://github.com/anomalyco/opencode/tree/c57c448d31c390880e06493f81b91a2c92536ad9/packages/core/src/model.ts): Provider-scoped model metadata, variants and reasoning fields.
- [packages/core/src/worktree.ts](https://github.com/anomalyco/opencode/tree/c57c448d31c390880e06493f81b91a2c92536ad9/packages/core/src/worktree.ts): Git-backed isolation and lifecycle with typed errors.
- [packages/core/test/worktree.test.ts](https://github.com/anomalyco/opencode/tree/c57c448d31c390880e06493f81b91a2c92536ad9/packages/core/test/worktree.test.ts): Isolation behavior tested against disposable Git fixtures.
- [packages/util/src/cross-spawn-spawner.ts](https://github.com/anomalyco/opencode/tree/c57c448d31c390880e06493f81b91a2c92536ad9/packages/util/src/cross-spawn-spawner.ts): Cross-platform child-process adapter; consult Windows behavior without assuming compatibility.
- [packages/core/src/pty.ts](https://github.com/anomalyco/opencode/tree/c57c448d31c390880e06493f81b91a2c92536ad9/packages/core/src/pty.ts): PTY output retention, bounded buffering and subscribers.
- [packages/protocol/src](https://github.com/anomalyco/opencode/tree/c57c448d31c390880e06493f81b91a2c92536ad9/packages/protocol/src): Typed API groups/client and authorization middleware.
- [packages/desktop/src/main/service](https://github.com/anomalyco/opencode/tree/c57c448d31c390880e06493f81b91a2c92536ad9/packages/desktop/src/main/service): Background service lifecycle and sidecar credential boundaries.
- [packages/app/src/new-session](https://github.com/anomalyco/opencode/tree/c57c448d31c390880e06493f81b91a2c92536ad9/packages/app/src/new-session): New-session composer and project/workspace UX boundaries.

**Fontes da verificação:** [Official verification source](https://opencode.ai/), [Official verification source](https://github.com/anomalyco/opencode), [Official verification source](https://api.github.com/repos/anomalyco/opencode), [Official verification source](https://raw.githubusercontent.com/anomalyco/opencode/dev/LICENSE).

- Initial catalog only, not a full source or security audit. No project scripts/installers or tests were executed. No provider authentication or calls were made. Existing support for subscriptions is a reference, not proof that Orchestrix has equivalent official access.

### cmux

**Site oficial:** [https://cmux.com/](https://cmux.com/). **Disponibilidade:** public. **Versão:** 0.65.1 / build 109 (Xcode project metadata; development snapshot, not inferred release tag).

**Stack identificada:** Swift / AppKit / SwiftUI; Xcode / modular Swift packages; Ghostty terminal rendering; Unix socket control API; WebKit; TypeScript / React / SolidJS webviews and ancillary services.

**Arquitetura inicial:** Native macOS terminal/workspace host and CLI use a control socket, terminal surfaces and agent hooks. Swift packages isolate session state, agent launch/journal, panes, notifications and transport. Server/relay modules have separate licenses. A terminal host is not equivalent to Orchestrix routing policies.

**Relação com o Orchestrix:** Coding-agent session/surface ownership; Workspace/pane UX and bounded session snapshots; Notifications and event routing; Retry and interrupted-agent recovery; Scoped process ownership and termination.

**Licença/restrições:** Default project license is GPL-3.0-or-later with copyleft obligations; inspect compatibility before incorporating code. Server modules use BUSL-1.1 with Additional Use Grant: None; production use including self-hosting requires commercial permission before the version-specific change date. Third-party and excluded/generated files retain their own notices. No reuse was performed.
Licença no commit: [LICENSE](https://github.com/manaflow-ai/cmux/blob/c995e8337050e7dd4775e3927f4bba77b4282922/LICENSE). Clone `--depth 1 --no-tags`, branch `main`, HEAD `c995e8337050e7dd4775e3927f4bba77b4282922`; shallow e árvore limpa conferidos.
Licença do módulo: [web/LICENSE](https://github.com/manaflow-ai/cmux/blob/c995e8337050e7dd4775e3927f4bba77b4282922/web/LICENSE) — BUSL-1.1.
Licença do módulo: [workers/ci-artifacts/LICENSE](https://github.com/manaflow-ai/cmux/blob/c995e8337050e7dd4775e3927f4bba77b4282922/workers/ci-artifacts/LICENSE) — BUSL-1.1.
Licença do módulo: [workers/iroh-v2/LICENSE](https://github.com/manaflow-ai/cmux/blob/c995e8337050e7dd4775e3927f4bba77b4282922/workers/iroh-v2/LICENSE) — BUSL-1.1.
Licença do módulo: [workers/presence/LICENSE](https://github.com/manaflow-ai/cmux/blob/c995e8337050e7dd4775e3927f4bba77b4282922/workers/presence/LICENSE) — BUSL-1.1.
Licença do módulo: [services/iroh-relay-minter/LICENSE](https://github.com/manaflow-ai/cmux/blob/c995e8337050e7dd4775e3927f4bba77b4282922/services/iroh-relay-minter/LICENSE) — BUSL-1.1.
Licença do módulo: [cmux-tui/relays/cloudflare-do/LICENSE](https://github.com/manaflow-ai/cmux/blob/c995e8337050e7dd4775e3927f4bba77b4282922/cmux-tui/relays/cloudflare-do/LICENSE) — BUSL-1.1.

**Componentes para consulta:**

- [Sources/SessionPersistence.swift](https://github.com/manaflow-ai/cmux/tree/c995e8337050e7dd4775e3927f4bba77b4282922/Sources/SessionPersistence.swift): Versioned, bounded window/workspace/session snapshots.
- [Sources/AgentAutoResumeCoordinator.swift](https://github.com/manaflow-ai/cmux/tree/c995e8337050e7dd4775e3927f4bba77b4282922/Sources/AgentAutoResumeCoordinator.swift): Retry coordination based on live events and cancellation when session activity changes.
- [Sources/App/AgentQuitProcessOwnership.swift](https://github.com/manaflow-ai/cmux/tree/c995e8337050e7dd4775e3927f4bba77b4282922/Sources/App/AgentQuitProcessOwnership.swift): Process identity/ancestry checks before termination; platform-specific implementation.
- [Sources/App/AgentHibernationController+ProcessSignaling.swift](https://github.com/manaflow-ai/cmux/tree/c995e8337050e7dd4775e3927f4bba77b4282922/Sources/App/AgentHibernationController+ProcessSignaling.swift): Scoped termination, process identity and race handling; Darwin-specific.
- [Sources/TerminalController.swift](https://github.com/manaflow-ai/cmux/tree/c995e8337050e7dd4775e3927f4bba77b4282922/Sources/TerminalController.swift): Terminal/control-socket integration entry point.
- [CLI](https://github.com/manaflow-ai/cmux/tree/c995e8337050e7dd4775e3927f4bba77b4282922/CLI): Programmable workspace, pane and agent-hook commands.
- [Packages/macOS/CmuxAgentSessionStore](https://github.com/manaflow-ai/cmux/tree/c995e8337050e7dd4775e3927f4bba77b4282922/Packages/macOS/CmuxAgentSessionStore): Agent-session state boundaries.
- [Packages/macOS/CmuxControlSocket](https://github.com/manaflow-ai/cmux/tree/c995e8337050e7dd4775e3927f4bba77b4282922/Packages/macOS/CmuxControlSocket): Desktop/CLI control transport.
- [docs/agent-recovery.md](https://github.com/manaflow-ai/cmux/tree/c995e8337050e7dd4775e3927f4bba77b4282922/docs/agent-recovery.md): Recovery design; invokes Subrouter and is not standalone Orchestrix-compatible code.
- [docs/agent-session-tracking-spec.md](https://github.com/manaflow-ai/cmux/tree/c995e8337050e7dd4775e3927f4bba77b4282922/docs/agent-session-tracking-spec.md): Draft design explicitly separates authoritative session binding from unreliable heuristics.
- [cmuxTests](https://github.com/manaflow-ai/cmux/tree/c995e8337050e7dd4775e3927f4bba77b4282922/cmuxTests): Native component tests for consultation only.

**Fontes da verificação:** [Official verification source](https://cmux.com/), [Official verification source](https://github.com/manaflow-ai/cmux), [Official verification source](https://api.github.com/repos/manaflow-ai/cmux), [Official verification source](https://raw.githubusercontent.com/manaflow-ai/cmux/main/LICENSE), [Official verification source](https://raw.githubusercontent.com/manaflow-ai/cmux/main/web/LICENSE).

- Official cmux.dev redirects to cmux.com. Existing repository submodules (ghostty, homebrew-cmux, vendor/bonsplit) were not initialized; clone is for reading and is not a complete build environment. macOS/Darwin process logic cannot establish Windows behavior. Some docs are drafts; catalog paths are consultation targets, not verified product claims. No code was executed.

### Microsoft Conductor

**Site oficial:** [https://github.com/microsoft/conductor](https://github.com/microsoft/conductor). **Disponibilidade:** public. **Versão:** 0.1.40 (pyproject.toml; development snapshot, not inferred release tag).

**Stack identificada:** Python >=3.12; Typer / Rich; Pydantic; YAML / Jinja2 / simpleeval; asyncio; GitHub Copilot SDK / Anthropic / optional Claude Agent SDK; FastAPI / WebSockets dashboard; pytest.

**Arquitetura inicial:** CLI loads validated YAML workflows, runs a deterministic routing engine and delegates agent execution to provider adapters. Separate context/checkpoint/event-log services, execution backends, human gates and fleet/web surfaces. Capability descriptors validate requests before SDK initialization.

**Relação com o Orchestrix:** Deterministic orchestration and conditional delegation; Provider capability validation and reasoning translation; Sequential and parallel execution boundaries; Checkpoints, context and failure recovery; Human gates, usage and billing modes; Local subprocess cancellation and stream draining.

**Licença/restrições:** Preserve copyright and MIT license notices with copies or substantial portions. Microsoft/third-party trademarks follow their separate brand policies. Inspect dependencies and per-file terms before any reuse; upstream contributions may require CLA.
Licença no commit: [LICENSE](https://github.com/microsoft/conductor/blob/fd7428774fd955643158c1db27f4f5580c6e099a/LICENSE). Clone `--depth 1 --no-tags`, branch `main`, HEAD `fd7428774fd955643158c1db27f4f5580c6e099a`; shallow e árvore limpa conferidos.

**Componentes para consulta:**

- [src/conductor/providers/base.py](https://github.com/microsoft/conductor/tree/fd7428774fd955643158c1db27f4f5580c6e099a/src/conductor/providers/base.py): Provider contract and structured events/results.
- [src/conductor/providers/capabilities.py](https://github.com/microsoft/conductor/tree/fd7428774fd955643158c1db27f4f5580c6e099a/src/conductor/providers/capabilities.py): Declarative capability descriptors without credentialed SDK initialization.
- [src/conductor/providers/reasoning.py](https://github.com/microsoft/conductor/tree/fd7428774fd955643158c1db27f4f5580c6e099a/src/conductor/providers/reasoning.py): Provider-specific effort/extended-thinking translation; values require independent current validation.
- [src/conductor/providers/copilot.py](https://github.com/microsoft/conductor/tree/fd7428774fd955643158c1db27f4f5580c6e099a/src/conductor/providers/copilot.py): SDK integration and session handling reference.
- [src/conductor/providers/claude_agent_sdk.py](https://github.com/microsoft/conductor/tree/fd7428774fd955643158c1db27f4f5580c6e099a/src/conductor/providers/claude_agent_sdk.py): Optional agent-runtime adapter and declared capability boundaries.
- [src/conductor/engine/router.py](https://github.com/microsoft/conductor/tree/fd7428774fd955643158c1db27f4f5580c6e099a/src/conductor/engine/router.py): Deterministic first-match workflow routing.
- [src/conductor/engine/checkpoint.py](https://github.com/microsoft/conductor/tree/fd7428774fd955643158c1db27f4f5580c6e099a/src/conductor/engine/checkpoint.py): Versioned failure checkpoints, session IDs and redaction.
- [src/conductor/execution/local.py](https://github.com/microsoft/conductor/tree/fd7428774fd955643158c1db27f4f5580c6e099a/src/conductor/execution/local.py): Owned child-process cancellation, draining and reaping.
- [src/conductor/interrupt/listener.py](https://github.com/microsoft/conductor/tree/fd7428774fd955643158c1db27f4f5580c6e099a/src/conductor/interrupt/listener.py): Interrupt signaling and terminal restoration boundaries.
- [tests/test_providers](https://github.com/microsoft/conductor/tree/fd7428774fd955643158c1db27f4f5580c6e099a/tests/test_providers): Provider contract and capability regression cases.
- [tests/test_execution](https://github.com/microsoft/conductor/tree/fd7428774fd955643158c1db27f4f5580c6e099a/tests/test_execution): Execution-backend regression cases.

**Fontes da verificação:** [Official verification source](https://github.com/microsoft/conductor), [Official verification source](https://api.github.com/repos/microsoft/conductor), [Official verification source](https://raw.githubusercontent.com/microsoft/conductor/main/LICENSE), [Official verification source](https://raw.githubusercontent.com/microsoft/conductor/main/pyproject.toml).

- Distinct project from Conductor at conductor.build; no relationship or shared source is implied. Provider/API/SDK behavior here does not authorize additional billing or fallback in Orchestrix Subscription Only. No SDK login, calls, installers, dependency installation or project tests were executed.

### Conductor (conductor.build)

**Site oficial:** [https://www.conductor.build/](https://www.conductor.build/). **Disponibilidade:** public-documentation-only. **Versão:** No source snapshot; documentation consulted 2026-10-12.

**Stack identificada:** Implementation stack not verified from official source code.

**Arquitetura inicial:** Documented product model: repository/project with isolated Git workspaces and branches, coding-agent chats, diff/check/PR flow; desktop and cloud contexts are distinct. This is product documentation, not source inspection.

**Relação com o Orchestrix:** Agent CLIs; Worktree ownership; Chats attached to workspace; Review and checks; Provider/subscription selection.

**Licença/restrições:** No source license established. Consult public documentation as references; do not infer permission to copy application code, branding or assets.

**Componentes para consulta:**

Sem componente de código catalogado; consultar somente as fontes oficiais abaixo. Stack interna e licença de código não estabelecidas.

**Fontes da verificação:** [Official product site; no application source repository identified.](https://www.conductor.build/), [Official agent/workspace product documentation.](https://www.conductor.build/docs), [Documents worktree isolation and its limits.](https://www.conductor.build/docs/concepts/git-worktrees).

- No verified official public application repository located in the initial search; absence is qualified, not a claim that private code cannot exist.
- Do not substitute Microsoft Conductor, Netflix/Orkes Conductor, Melty or unofficial forks for this product.

### Cobalt (cobaltcode.ai)

**Site oficial:** [https://cobaltcode.ai/](https://cobaltcode.ai/). **Disponibilidade:** public-product-site-only. **Versão:** No source snapshot; site consulted 2026-10-12.

**Stack identificada:** Implementation stack not verified from official source code.

**Arquitetura inicial:** The official site describes isolated cloud computers per task and a workflow binding prompt, sources, runtime, verification and delivery. Internal implementation is not established.

**Relação com o Orchestrix:** Task isolation; Agent/account/model choice; Parallel task lifecycle; Context and checks attached to task; Diff and delivery.

**Licença/restrições:** No source license established. Product descriptions are documentary references, not permission to reuse application code, graphics or branding.

**Componentes para consulta:**

Sem componente de código catalogado; consultar somente as fontes oficiais abaixo. Stack interna e licença de código não estabelecidas.

**Fontes da verificação:** [Official product presentation; no official application source repository identified.](https://cobaltcode.ai/).

- No verified official public application repository located; do not fabricate a GitHub URL or replace Cobalt with an unrelated Amaretto repository.
- Cloud resource and billing behavior are not policies to import into Subscription Only.
