# Codex CLI JSON e app-server — comparação delimitada de OX-001

Preparação em 12/10/2026 e diagnóstico atualizado em 13/10, para o Codex instalado `0.162.0-alpha.17.2`. Esta entrega acrescenta o experimento [exec-probe.mjs](../../tools/runtime-harness/exec-probe.mjs) e registra a primeira rodada comparativa real, sem promover um adapter de produção. O [registro live anterior](m0-live-validation-2026-10-12.md) conserva a evidência de edição, retomada e interrupção já obtida por app-server; o percurso CLI exige evidência própria.

## Superfícies comparadas

| Aspecto | App-server usado no harness | `codex exec --json` preparado nesta entrega |
| --- | --- | --- |
| Transporte | JSON-RPC bidirecional por stdio, com IDs de requests e notificações. | Um subprocesso por invocação, com eventos JSONL em stdout; stderr é drenado separadamente. |
| Sessão | `thread/start`, persistência não ephemeral, `thread/resume` por ID exato. | `thread.started` fornece o ID da primeira invocação; `exec resume <ID>` continua essa sessão. Nunca usar `--last`, seleção por nome ou fork no experimento. |
| Admissão | Validar configuração resolvida de start/resume antes de enviar o turno. | Fixar modelo, effort, cwd, sandbox e aprovação por argumentos estruturados. O stream CLI examinado não devolve um ACK equivalente da configuração resolvida; o relatório conserva essa ausência. |
| Início e resultado | ACK de `turn/start` não é início; aguardar notificação própria e terminal correlacionado. | Exigir `thread.started`, `turn.started`, `turn.completed` com usage válido, exit 0 e fechamento dos pipes. Exit 0 sozinho não passa. |
| Aprovação e cancelamento | Requests de aprovação exatos; `turn/interrupt` seguido de terminal `interrupted`. | Aprovação `never`. O experimento não dispõe de RPC de interrupção; encerramento pelo processo/Job é outra evidência e nunca é chamado de `turn.interrupted`. |
| Modelos, conta e disponibilidade | Consultas de metadados antes de admitir inferência. | Reutilizar as consultas app-server antes de cada invocação. Comparar binding opaco apenas em memória; isso não estabelece lock atômico do login compartilhado durante a execução CLI. |
| Isolamento Windows | Supervisão própria, independente do protocolo. | Mesmo adapter de Job próprio, com pipes de dados separados do controle do helper. Esperar runtime `ready` antes dos deadlines de turno. |

Essas diferenças seguem a [documentação oficial do app-server](https://learn.chatgpt.com/docs/app-server), a [execução não interativa e seus eventos](https://learn.chatgpt.com/docs/non-interactive-mode) e a [referência CLI](https://learn.chatgpt.com/docs/cli/reference). Também foram lidos localmente `codex exec --help` e `codex exec resume --help`. Na versão examinada, a ajuda de resume não anuncia opções próprias `-C` e `-s`; o harness coloca cwd e overrides de configuração antes de `exec`, sem inventar flags de resume.

## Percurso e limites

O modo padrão faz somente preflight de metadados. A opção explícita `--live` admite no máximo **duas** invocações: primeiro, corrigir `src/add.mjs` na worktree descartável existente; depois, retomar exatamente o ID observado e recuperar um marcador anterior sem repeti-lo no prompt. A segunda invocação preserva os pins workspace-write, modelo e effort e deve conservar o artefato da primeira; seu prompt proíbe ferramentas/escritas. O diff, os cinco checks, o snapshot e o checkout principal usam os mesmos helpers do experimento app-server.

Antes de cada invocação, exigir versão revisada, autenticação ChatGPT, ordinary usage positivamente disponível, endpoint oficial e extensões desativadas. Escolher modelo/effort anunciados pelo runtime; uma mudança na conexão, no modelo ou no effort bloqueia a continuação. O ambiente usa a allowlist existente, sem API keys ou fallback pago. Não adicionar `forced_login_method` sobre o cache compartilhado como substituto desses gates: a [inspeção anterior do auth manager](codex-protocol-spike.md#assinatura-existente-sem-fallback-de-api) registra seu possível efeito de logout.

Em Windows, live exige Job próprio. Um encerramento bem-sucedido requer runtime exit 0, árvore confirmada vazia, pipes drenados, helper exit 0 e ausência de falha nos streams do helper. O fechamento ou morte do helper, isoladamente, mantém o resultado desconhecido. `ExecJsonProcess.cancel()` espera início próprio antes de pedir stop e relata separadamente a prova da árvore; o orçamento de duas invocações não acrescenta um terceiro turno dedicado a cancelamento. Cleanup por timeout não passa a contar como interrupção bem-sucedida.

O parser impõe limites de frame, eventos, itens, texto acumulado e stderr; rejeita JSON/UTF-8 inválidos, identidades contraditórias, terminal fora de ordem, usage inválido e conclusão sem terminal. Texto, comandos, mensagens de erro, stderr e IDs reais de sessão não são persistidos no relatório. O marcador e o binding ficam somente em memória. Eventos futuros têm contagem genérica, sem payload bruto.

## Evidência disponível

O [preflight real de metadados](evidence/codex-m0-exec-metadata.json) passou em `0.162.0-alpha.17.2`, com MXC selecionado, assinatura ChatGPT, ordinary usage disponível, endpoint oficial e extensões desativadas. Os dois processos **app-server** desse preflight — bootstrap e consulta final — encerraram com runtime exit 0, helper exit 0, Job confirmado vazio e todos os pipes drenados sem falha. O checkout principal permaneceu igual. O registro tem `liveStarted: false` e `invocations: 0`: **não iniciou `codex exec`, não enviou turno e não consumiu inferência**.

Os testes offline em [exec-probe.test.mjs](../../tools/runtime-harness/tests/exec-probe.test.mjs) cobrem streams sintéticos, limites, retomada por ID exato, sanitização, opt-in e orçamento antes de criar a fixture e separação entre início, término do runtime, término do helper e interrupção de turno. Eles validam o observador e seus critérios; não demonstram edição, retomada ou cancelamento do Codex real.

A suíte CLI passou **29/29**, incluída inicialmente nos **124/124 offline em 37,72 s**; sete casos posteriores da prova forçada completaram **131 casos offline distintos**. Após a correção de ambiente, a regressão final passou **131/131 offline em 39,67 s**, **21/21 de transporte Windows em 40,75 s**, **9/9 do helper em 28,28 s** e o check do harness. Os 29 casos CLI são um subconjunto dos 131 offline. A rodada real descrita abaixo observou eventos e encerramento CLI, mas não aprovou edição ou retomada. Mesmo que diff e retomada passem posteriormente, a ausência de ACK CLI da configuração resolvida permanece uma diferença da superfície.

| Critério | Evidência disponível | Situação desta comparação |
| --- | --- | --- |
| Gates de versão, assinatura, disponibilidade, endpoint e extensões | Preflight real acima; bootstrap e app-server final supervisionados por Job. | **Validado sem inferência**; não certifica a execução CLI posterior. |
| Parser, limites, identidade de thread e cleanup | 29 testes CLI sintéticos, incluídos na regressão offline; primeira invocação real observou início, terminal e saída. | **Offline aprovado e stream real observado**; não implica êxito da tarefa. |
| Configuração resolvida antes do turno | App-server já possui validação e evidência real anterior; CLI recebe pins iguais nas duas invocações. | **Preparado para CLI**, com ACK equivalente **não observável** no stream examinado. |
| Edição, diff e cinco checks da mesma tentativa | App-server passou no percurso MXC anterior; na nova comparação, app-server e CLI retornaram completed com diff vazio. | **Nova rodada não aprovou edição**; checks da tentativa não executados. Correção determinística posterior não substitui edição pelo modelo. |
| Retomada do mesmo ID, contexto e artefato | App-server possui evidência anterior; CLI está preparado para recuperar o marcador e conservar o diff. | **Live CLI pendente**, mantendo workspace-write, modelo e effort da primeira invocação. |
| Árvore real durante inferência e cancelamento | As duas primeiras invocações reais encerraram graceful, com Job confirmado vazio, runtime/helper exit 0 e streams drenados. | **Cleanup graceful real observado**; retomada, stop forçado e cancelamento não foram executados nessa rodada. |

## Rodada autorizada e diagnóstico — 12–13/10

A autorização de cinco turnos foi consumida parcialmente: **dois turnos enviados**, um por [app-server](evidence/codex-m0-job-live-blocked.json) e um por [CLI](evidence/codex-m0-exec-live-blocked.json). Ambos terminaram `completed`, mas não alteraram `src/add.mjs`: diff vazio, baseline incorreta preservada e checks da tentativa `notRun`. Os checkouts principais permaneceram intactos. O harness parou sem retomada, stop forçado ou segunda invocação CLI, preservando os critérios de aceite e sem retry automático.

Os dois processos reais encerraram graceful com árvore própria confirmada vazia, runtime exit 0, helper exit 0 e pipes drenados. No relatório CLI, o objeto externo expressa a observação do processo pai do runtime e mantém `treeTermination: unknown`; `process.supervision` contém a prova específica do Job confirmado vazio. O runner já valida explicitamente essa prova aninhada, sem inferi-la da saída do pai ou do helper. Não reinterpretar esse cleanup como stop forçado ou `turn.interrupted`.

O diagnóstico consultou somente metadados das threads próprias, com correspondência exata de cwd. Em cada tentativa, duas ferramentas falharam com exit 1: o launcher MXC não localizou o diretório da plataforma, com `HRESULT 0x80070003`. Esse erro explica a falha operacional observada; `completed` do turno continuou insuficiente para aprovar o artefato.

Um [comando fixo, sem modelo](evidence/codex-m0-job-mxc-diagnostic.json), separou os caminhos: execução direta retornou exit 0 e cinco checks aprovados; a execução sob Job inicialmente retornou exit 1. O ambiente JavaScript e o lançamento nativo bruto foram corrigidos para conservar `SystemDrive`. Depois da correção, o mesmo comando determinístico retornou exit 0 e cinco checks aprovados tanto direto quanto sob Job. Nenhuma política, composição do Job ou deadline foi relaxada. Essa prova confirma o caminho determinístico corrigido; **edição e retomada CLI pelo modelo após a correção permanecem não comprovadas**.

Uma nova tentativa live exige escopo explícito, limitado a no máximo cinco turnos — três app-server e duas invocações CLI — e continua sem retry automático. **Nenhuma nova rodada após a correção foi autorizada nesta atualização.** Não tratar a correção de ambiente ou o saldo da rodada anterior como nova autorização.

**Recomendação provisória:** manter app-server como candidato ao primeiro adapter pela confirmação de política e interrupção cooperativa, com evidência real anterior de edição/retomada. A comparação atual acrescentou cleanup graceful real nas duas superfícies, mas a falha compartilhada do launcher impediu avaliar edição e retomada sob a integração nova. `exec --json` conserva a limitação de ACK da configuração e de interrupção por RPC. Não há vencedor confirmado da comparação live nem aprovação de OX-001; não adiantar o adapter Rust/OX-003.
