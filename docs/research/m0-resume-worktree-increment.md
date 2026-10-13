# M0 — retomada e tentativa em worktree

Registro em **12/10/2026**, após o [reteste humano de D1](../design/d1-pilot.md#piloto-reportado-pelo-responsável--1210). O recorte é OX-001: preparar e verificar o próximo percurso do primeiro runtime, aproveitando o harness existente. A preparação abaixo foi seguida do [live autorizado, correções e segunda prova MXC aprovada](m0-live-validation-2026-10-12.md); esse registro posterior é a referência atual. Não implementa o Core Rust, o Desktop ou todos os provedores.

## Referências consultadas

A [biblioteca local](REFERENCE_REPOSITORIES.md) foi concluída antes da implementação: nove clones superficiais, onze entradas e 85 componentes com caminhos conferidos. Nenhum código, asset, script ou instalador upstream foi incorporado/executado.

- Jackalope `9bc0e9a3fa022f05fa1aac7a4bd4e78b64dabbba`: `apps/desktop/src-tauri/src/commands/process_control.rs` separa saída limitada e controle de processos. Essa leitura orienta a atenção à propriedade do processo; não comprova contenção do runtime do Orchestrix.
- Vibe Kanban `d5cbb5380fa0b32e98ef9b8d987f63decce4be3a`: `crates/executors/src/executors/codex/client.rs` e `codex.rs` separam protocolo e execução. O follow-up observado usa `thread/fork`; para o critério atual do Orchestrix, isso não substitui retomada da mesma identidade por `thread/resume`.
- O contrato de retomada e sandbox foi conferido na [documentação oficial OpenAI](https://learn.chatgpt.com/docs/app-server) e no schema da instalação local. A implementação usa código próprio e os ADRs existentes.

## Entrega

O [worktree-probe.mjs](../../tools/runtime-harness/worktree-probe.mjs) cria um repositório temporário próprio, faz um commit base e abre um worktree detached para uma tentativa. Não faz checkout, merge ou escrita no Orchestrix. O ambiente Git exclui overrides herdados, configuração global/sistema e hooks; os comandos do fixture têm diretório e identidade próprios.

Por padrão, só consulta metadados. `--live` é opt-in e exige assinatura ChatGPT, disponibilidade ordinary, provider/endpoint oficiais e extensões desativadas, incluindo delegação nativa. Não há fallback API, login/logout ou cópia de autenticação. O modelo default e um esforço baixo são escolhidos apenas dentre os anunciados; sem confirmação de capacidade ou identidade observável, a prova bloqueia.

O percurso live preparado tem três turnos limitados:

1. Uma função aritmética defeituosa no worktree deve ser corrigida, sem alterar os checks, instruções ou Git. O baseline falha. A evidência vincula base, arquivos, SHA-256 do diff, patch e cinco casos determinísticos à mesma tentativa. O checker só avalia uma expressão aritmética limitada; código JavaScript arbitrário é recusado. Alterar o checker, produzir arquivo extra ou mudar o checkout principal bloqueia a verificação.
2. O app-server próprio é encerrado e reiniciado. O harness reconfirma os gates e compara em memória a identidade observável da conta. `thread/resume` deve devolver a mesma thread, persistida e idle, com cwd/modelo/effort/política confirmados. Um marcador do primeiro turno, não repetido no segundo prompt, permite conferir a continuidade do contexto interno do runtime. IDs, marcador, paths e binding não entram no relatório sanitizado.
3. Um turno é iniciado para interrupção; o harness corrigido espera sua notificação própria de início antes do pedido. ACK sozinho não basta. O terminal deve ser `interrupted` e o artefato/checks devem permanecer iguais. O término do processo pai é observado separadamente da árvore Windows.

`CodexSession` mantém read-only/ephemeral como defaults do primeiro probe. Persistência/workspace-write são escolhas explícitas. Retomada só admite uma sessão local nova e ociosa; timeout, ACK perdido ou configuração contraditória conservam estado incerto. Os turnos usam a política confirmada e congelada, sem mudar esforço silenciosamente. Binding de metadados da conta fica apenas em memória, não é isolamento de múltiplas contas.

## Versão e evidência de leitura

A instalação passou de `0.162.0-alpha.2` para **`0.162.0-alpha.17.2`**. O primeiro preflight recusou a versão desconhecida antes de criar thread/turno. Após inspecionar o schema local, a allowlist passou a conter essas duas versões explícitas; versões futuras continuam bloqueadas.

- Bundle `codex_app_server_protocol.schemas.json`: SHA-256 `c7338bc0830b61fb07ba35325d4fbeb8207a919e53468c664dda5803aacb6c8e`.
- `ThreadResumeParams.json`: SHA-256 `948779cace55dda4384689a1627cd6675aa4782744bb5820cca16bfc0118586c`.
- `ThreadResumeResponse.json`: SHA-256 `de584907214ffd2210b2a6be7072bb63f41e3c31e086b2665af90e385c8a6098`.
- Campos pertinentes conferidos: `threadId`, overrides de cwd/modelo/approval/sandbox/config; respostas com thread/modelProvider/approval/reviewer/sandbox e effort; `TurnStartParams.sandboxPolicy`/effort. O bundle completo permanece em artifacts local ignorado, sem copiar schemas volumosos ao projeto.

O [preflight sanitizado](evidence/codex-m0-worktree-metadata.json) passou, **sem inferência**: assinatura autenticada, ordinary positivo, endpoint oficial, hooks/plugins/apps/notify/MCP/delegação nativa desativados e processo pai encerrado com código 0. O baseline da fixture falhou como esperado e o checkout principal temporário permaneceu igual. Isso não testa workspace-write nem retomada real, pois o modo de leitura não cria uma thread.

## Verificação e pendências

- **75/75 testes offline** passaram em rodada serial de **26,02 s**; incluem os 58 contratos anteriores, onze casos de sessão/retomada e seis de worktree/sanitização/ambiente/delegação. Os últimos casos testam fixtures próprias, sem inferência. A rodada anterior de 73 teve três timeouts em cenários antigos sob carga concorrente; a rodada final manteve os deadlines e todos os critérios.
- **9/9 testes nativos Windows** passaram em **33,97 s**, incluindo watchdog com stdout bloqueado. Essa prova usa o helper/descendentes sintéticos existentes; não integra Job Object ao transporte Codex.
- Check de sintaxe do harness passou. Biblioteca: HEAD/origin/shallow/árvore limpa/licenças e caminhos dos nove clones conferidos; utilitário de fetch passou nove verificações em Git fixture exclusivamente local, preservando checkout/anotações e rastreando mudança/ausência de licenças por módulo.
- **Continuação live:** a primeira autorização teve edição bloqueada, retomada read-only e cancelamento não comprovado. Após correções e 95/95 offline, uma segunda autorização de três turnos passou o percurso MXC: edição pelo modelo/cinco checks, retomada com a mesma política de escrita e cancelamento do turno confirmados em 36,27 s. O [registro posterior](m0-live-validation-2026-10-12.md) mantém ambos os resultados e os limites; árvore real e comparação CLI permanecem pendentes.

**OX-001 continua aberto.** A [continuação de supervisão](windows-process-containment.md#integração-do-transporte--1210) implementou a bridge, testou fake/árvore própria e passou metadados reais sem inferência. A rodada autorizada usou dois turnos (app-server/CLI), bloqueados sem diff por falha MXC; a correção de `SYSTEMDRIVE` passou diagnóstico determinístico, 131 offline, 21 de transporte Windows e nove do helper. Próximo: autorização específica para demonstrar trabalho real após a correção e concluir a [comparação CLI](cli-app-server-comparison.md), sem retry automático. O percurso MXC anterior não certifica término da árvore. Só promover OX-003 após os critérios do primeiro runtime; M1/M2 conservam domínio, persistência, recuperação, revisão e aplicação reais. Avaliação externa de D1 permanece em OX-017.
