# Codex CLI JSON e app-server — comparação delimitada de OX-001

Preparação em 12/10/2026 e comparação atualizada em 13/10, para Codex `0.162.0-alpha.17.2`, MXC, `gpt-6.1-sol` / `low`. Após corrigir `SYSTEMDRIVE`, uma nova autorização de **cinco turnos foi usada integralmente**: três app-server e dois CLI passaram. App-server foi escolhido como primeiro runtime experimental no recorte local single-account/serial e libera o início de OX-003. Não é aprovação de adapter de produção nem conclusão de OX-001/OX-002/M0. A primeira rodada bloqueada e o [live anterior](m0-live-validation-2026-10-12.md) permanecem registros separados.

## Superfícies comparadas

| Aspecto | App-server usado no harness | `codex exec --json` usado no harness |
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

A suíte CLI passou **29/29**, incluída inicialmente nos **124/124 offline em 37,72 s**; sete casos posteriores da prova forçada completaram **131 casos offline distintos**. Após a correção de ambiente, a regressão passou **131/131 offline em 39,67 s**, **21/21 de transporte Windows em 40,75 s**, **9/9 do helper em 28,28 s** e o check do harness. Esses resultados são da **rodada anterior à nova entrega live**, sem repetição nesta entrega; os 29 casos CLI são um subconjunto dos 131 offline. A primeira rodada real não aprovou edição/retomada, mas a nova autorização abaixo demonstrou ambas. A ausência de ACK CLI da configuração resolvida permanece uma diferença da superfície.

| Critério | Evidência disponível | Situação desta comparação |
| --- | --- | --- |
| Gates de versão, assinatura, disponibilidade, endpoint e extensões | Preflight real acima; novamente verificados no percurso live, com metadata supervisionada por Job. | **Observados no recorte testado**; não tornam a autenticação compartilhada um lock atômico. |
| Parser, limites, identidade de thread e cleanup | 29 casos CLI sintéticos da regressão anterior e duas invocações live pós-correção. | **Offline e live aprovados no recorte**; catálogo completo permanece parcial. |
| Configuração resolvida antes do turno | App-server valida ACK em start/resume; CLI recebe pins iguais nas duas invocações. | **Observada por app-server**; ACK equivalente CLI continua **não observável** no stream examinado. |
| Edição, diff e cinco checks da mesma tentativa | Nova autorização pós-correção: app-server e CLI produziram diffs próprios e cinco checks cada. | **Comprovado nas duas superfícies** com versão/modelo/effort fixados. |
| Retomada do mesmo ID, contexto e artefato | Reinício/resume app-server e `exec resume` por ID exato; marcador anterior e artefato conservados. | **Comprovado nas duas superfícies**; nenhuma seleção por `--last` ou fork. |
| Árvore real durante inferência e término | App-server forçado durante terceiro turno ativo: runtime 137, helper 0, Job vazio e streams completos. CLI: seis processos, incluindo metadata, encerrados graceful com runtime/helper 0. | **Força local app-server e cleanup graceful CLI comprovados**; força não produz terminal remoto; CLI não teve turno dedicado a cancelamento. |

## Rodada autorizada e diagnóstico — 12–13/10

A autorização de cinco turnos foi consumida parcialmente: **dois turnos enviados**, um por [app-server](evidence/codex-m0-job-live-blocked.json) e um por [CLI](evidence/codex-m0-exec-live-blocked.json). Ambos terminaram `completed`, mas não alteraram `src/add.mjs`: diff vazio, baseline incorreta preservada e checks da tentativa `notRun`. Os checkouts principais permaneceram intactos. O harness parou sem retomada, stop forçado ou segunda invocação CLI, preservando os critérios de aceite e sem retry automático.

Os dois processos reais encerraram graceful com árvore própria confirmada vazia, runtime exit 0, helper exit 0 e pipes drenados. No relatório CLI, o objeto externo expressa a observação do processo pai do runtime e mantém `treeTermination: unknown`; `process.supervision` contém a prova específica do Job confirmado vazio. O runner já valida explicitamente essa prova aninhada, sem inferi-la da saída do pai ou do helper. Não reinterpretar esse cleanup como stop forçado ou `turn.interrupted`.

O diagnóstico consultou somente metadados das threads próprias, com correspondência exata de cwd. Em cada tentativa, duas ferramentas falharam com exit 1: o launcher MXC não localizou o diretório da plataforma, com `HRESULT 0x80070003`. Esse erro explica a falha operacional observada; `completed` do turno continuou insuficiente para aprovar o artefato.

Um [comando fixo, sem modelo](evidence/codex-m0-job-mxc-diagnostic.json), separou os caminhos: execução direta retornou exit 0 e cinco checks aprovados; a execução sob Job inicialmente retornou exit 1. O ambiente JavaScript e o lançamento nativo bruto foram corrigidos para conservar somente `SystemDrive`. Depois da correção, o mesmo comando determinístico retornou exit 0 e cinco checks aprovados tanto direto quanto sob Job. Nenhuma política, composição do Job ou deadline foi relaxada. Essa prova confirmou o caminho determinístico corrigido; naquela etapa, ainda não demonstrava edição/retomada pelo modelo.

Na conclusão desse diagnóstico, nenhuma nova rodada pós-correção havia sido autorizada. Os três turnos não utilizados não foram tratados como autorização implícita, nem houve retry automático. A prova seguinte decorreu de **nova autorização explícita**, descrita abaixo.

## Nova autorização pós-correção — prova aprovada

Foram usados **5/5 turnos da nova autorização**, serialmente, sem retry ou fallback pago: três app-server e duas invocações CLI. HEAD de execução: **`f93dae9`**, com correção de runtime **`b6015e3`**; Codex **`0.162.0-alpha.17.2`**, MXC, **`gpt-6.1-sol`** / **`low`**, limites de **90 s por turno** e **300 s por Job**. As autorizações anterior e atual permanecem separadas.

O [app-server passou em 48,985 s](evidence/codex-m0-job-live.json): [edição/diff](evidence/codex-m0-job-live.patch) com cinco checks; reinício e retomada da mesma thread, identidade observável da conta, política, marcador e artefato; força no Job próprio após início observado do terceiro turno, sem ferramentas. A raiz final terminou com **137**, helper com **0**, árvore confirmada vazia, `activeCount=0` e todos os streams completos. `processTerminationConfirmed` e `forcedConfirmed` foram verdadeiros; `terminalStatus=null` e `turnInterruptedConfirmed=false` conservam a ausência de terminal remoto. A interrupção cooperativa por RPC tem evidência real anterior, distinta da prova de força atual.

A [CLI passou as duas invocações](evidence/codex-m0-exec-live.json): [edição/diff](evidence/codex-m0-exec-live.patch) com cinco checks e resume por ID exato, recuperando marcador anterior e conservando identidade observável de conta e artefato. Os **seis processos próprios** do percurso — quatro app-servers de metadata/bootstrap e dois `exec` — encerraram `graceful`, com raiz/helper **0**, Job confirmado vazio, `activeCount=0` e stdio fechado/drenado. O escopo não incluiu uma terceira invocação CLI dedicada à força/cancelamento, nem comprovou lock atômico da autenticação ou ACK da configuração CLI.

## Escolha do primeiro runtime experimental e lacunas

**Decisão:** escolher **app-server** como superfície do primeiro adapter, aprovada somente para o recorte local **single-account e serial** validado acima. Start/resume reconhecem configuração efetiva; requests de aprovação e `turn/interrupt` têm comunicação bidirecional. A CLI também demonstrou edição/resume, mas não expõe ACK equivalente da configuração nem RPC de interrupção no stream examinado. A decisão não transforma nenhuma das superfícies em adapter de produção ou suporte universal do Codex.

O aceite desse primeiro runtime libera o **início de OX-003**: contrato wire/provider-neutral versionado, fake executável, validação/replay e fixtures nos tools experimentais. O futuro Core Rust consumirá esse contrato; não há crate/Core implementado, e Cargo/rustc não estão no PATH nem foram instalados nesta entrega. Não importar eventos específicos Codex para o domínio nem implementar SQLite/worker/Desktop por associação.

**OX-001 continua aberto:** `CodexSession.discover()` lê apenas a primeira página do catálogo e conserva modelos/esforços limitados; paginação completa, service tiers e registro de features nativas permanecem parciais ou `unknown`. OAuth próprio **INT-10** também continua pendente. Não considerar a configuração desativada de delegação uma certificação de todos os recursos nativos. Multiaccount, sandbox completo, serviços externos fora do Job, recuperação durável, Claude/Antigravity e gates de OX-002/M0 permanecem abertos.

Os **5/5 turnos da nova autorização foram usados**. Não há nova inferência autorizada por este registro e não há retry automático. Os resultados 131/21/9 são da regressão anterior à fase live. A continuação posterior de [OX-003](../contracts/runtime-contract-v1.md) reexecutou os 131 dentro de **162/162 offline em 34,89 s**, com 31 novos casos do contrato e check aprovado. As suítes nativas 21/21 e 9/9 não foram repetidas; o código de transporte/helper permaneceu igual.
