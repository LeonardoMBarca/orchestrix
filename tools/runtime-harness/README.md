# Orchestrix runtime harness — M0

Experimento isolado de protocolo, executável com **Node 24+ e Git**, sem dependências de pacote. Não é o Core de produção nem muda a stack proposta Rust/Tauri. O protótipo visual em `prototypes/desktop` continua simulado.

Foi uma antecipação técnica. Em **12/10/2026**, o reteste humano de D1 liberou o gate piloto e **M0 foi retomado em OX-001**. A avaliação externa e os critérios de integração continuam nos tickets existentes; nenhum experimento promove automaticamente o adapter a produção.

## Incremento atual — worktree e retomada

O [registro de 12/10](../../docs/research/m0-resume-worktree-increment.md) acrescenta `worktree-probe.mjs` e `CodexSession.resumeThread`: **75/75 testes offline**, **9/9 Windows sintéticos** e check passaram. O preflight real **sem inferência** passou na versão `0.162.0-alpha.17.2`, após inspeção de schema e admissão explícita; a versão histórica `0.162.0-alpha.2` continua na allowlist, sem aceitar versões futuras silenciosamente. O [registro live posterior](../../docs/research/m0-live-validation-2026-10-12.md) preserva a primeira tentativa bloqueada em unelevated e registra a **segunda autorização de três turnos MXC aprovada** em 36,27 s: edição pelo modelo/cinco checks, retomada com mesma thread/conta observável/política de escrita e cancelamento do turno confirmados. O mesmo código passou **95/95 offline** e check na revisão anterior; a nova rodada não repetiu essas suítes. Árvore Windows permanece unknown.

```powershell
node worktree-probe.mjs                   # metadados apenas
node worktree-probe.mjs --windows-sandbox mxc         # preflight explícito Windows
node worktree-probe.mjs --windows-sandbox mxc --live  # exige novo opt-in
```

Live usa uma assinatura existente e três turnos limitados: corrigir uma função aritmética em worktree descartável, reiniciar o app-server e retomar a mesma thread com um marcador do contexto anterior, depois interromper um turno. Não copia login, muda a conta, usa fallback API ou executa código dos clones de referência. Cria uma thread persistida no runtime, necessária à prova de retomada. O relatório sanitizado não inclui thread/turn IDs reais, marker, binding, paths pessoais, transcript ou stdout/stderr bruto.

O fixture tem commit base próprio, baseline falho e cinco casos determinísticos. Modificação do checker, arquivo extra, código fora da gramática aritmética ou alteração no checkout principal temporário bloqueiam a prova. Diff/hash/checks permanecem vinculados ao snapshot. O ambiente Git exclui overrides e configuração global/sistema; hooks são desativados. Repositório/worktree e patch são mantidos para inspeção, sem cleanup recursivo automático.

Assinatura/ordinary/endpoint, hooks/plugins/apps/notify/MCP e delegação nativa precisam estar confirmados antes de live e novamente após reinício. Modelo/effort só usam capacidades anunciadas; o binding de metadados da conta fica em memória e não prova isolamento/cotas de múltiplas contas. Política read-only/ephemeral continua padrão do probe original; workspace-write/persistência são explícitos e confirmação contraditória mantém admissão incerta.

**OX-001 aberto:** o percurso live MXC anterior de edição/retomada/cancelamento com política de escrita passou. A bridge experimental Windows já está integrada ao transporte, com provas sintéticas e dois preflights reais sem inferência, descritos abaixo. Faltam a prova integrada durante trabalho do modelo e a comparação CLI `exec --json`. `--windows-sandbox mxc` é subprocess-only, confirmado em cada conexão, sem setup/configuração pessoal ou fallback automático. O cancelamento cooperativo espera início próprio antes do RPC; ACK e terminal são registrados separadamente. Timeout do baseline não conta como falha aritmética validada. Job Object e sandbox são mecanismos distintos; saída do pai não certifica término da árvore. Não é o Core Rust/SQLite ou o ciclo completo de revisão/aplicação no Desktop.

## Bridge Windows integrada — 12/10

`JsonRpcProcess({windowsJob:true})` usa o [adapter Windows](src/windows-job-process.mjs) e a [bridge nativa](windows/README.md), mantendo JSON-RPC nas três pipes binárias de runtime e eventos/comandos de supervisão em um canal separado. A opção também aceita `{lifetimeMs,startupTimeoutMs}`. O caminho sem bridge permanece disponível e mantém `treeTermination=unknown`. O helper sintético original conserva seus nove casos e limites próprios.

Os probes de worktree e CLI aceitam `--windows-job`. Em `worktree-probe.mjs`, `--process-stop` substitui a interrupção cooperativa do terceiro turno por `JsonRpcProcess.terminateOwnedJob()`: força no Job próprio, sem enviar EOF antes. O percurso exige início observado, ausência de terminal anterior/durante a força, saída **137**, helper com saída zero, streams fechados/drenados e Job vazio. Encerrar o processo não comprova `turn.interrupted`; o resultado remoto permanece desconhecido quando não foi observado.

```powershell
# Preflights: nenhuma inferência; ambos consultam metadata pelo app-server.
node worktree-probe.mjs --windows-job --windows-sandbox mxc
node exec-probe.mjs --windows-job --windows-sandbox mxc

# Exemplos para autorização específica pós-correção; não repetir automaticamente.
node worktree-probe.mjs --windows-job --windows-sandbox mxc --process-stop --live
node exec-probe.mjs --windows-job --windows-sandbox mxc --max-invocations 2 --live
```

Os dois preflights reais em `0.162.0-alpha.17.2`/MXC passaram sem inferência: [worktree](../../docs/research/evidence/codex-m0-job-metadata.json) e [preparação CLI](../../docs/research/evidence/codex-m0-exec-metadata.json). Incluindo bootstraps para desativar MCP, **quatro app-servers** tiveram saída zero da raiz/helper, stdio fechado/drenado e Job vazio. O preflight CLI usa o app-server e não comprova inferência por `exec`, seleção efetiva reconhecida pela CLI ou retomada CLI.

Na rodada anterior à correção de ambiente, **131 casos offline distintos** passaram em execuções separadas: **124/124 em 37,72 s** e **7/7 em 0,40 s**. A integração Windows com fake runtime passou **20/20 em 44,43 s**; o helper nativo foi retestado com **9/9 em 31,86 s**. Esses resultados são históricos. As falhas intermediárias e as correções de fixture/admissão, backpressure e drenagem estão preservadas no [registro de contenção](../../docs/research/windows-process-containment.md#integração-do-transporte--1210). Provas sintéticas não substituem integração real do modelo.

O startup da bridge tem orçamento de **10 s**; o watchdog independente admite **1–300 s**, com **300 s por Job** nos percursos preparados e **90 s por turno**. Stop comum concede **3 s** para EOF gracioso, depois até **4,5 s** para resposta do helper e **1 s** adicional após encerrar somente esse helper próprio. O watchdog conserva o fallback independente quando controle/stdout bloqueia. Fechamento de pipes, drenagem Node limitada a **250 ms**, saída da raiz, saída do helper e contabilização do Job continuam observações distintas. `treeTermination=confirmed` pode coexistir com stdio incompleto; isso não recebe sucesso de transporte. Perda do helper sem registro de término conserva `unknown`.

### Tentativas live e correção de ambiente — 13/10

Foram usados **2/5 turnos autorizados**, um de [app-server](../../docs/research/evidence/codex-m0-job-live-blocked.json) e outro de [CLI](../../docs/research/evidence/codex-m0-exec-live-blocked.json). Ambos terminaram `completed`, mas sem diff e com checks `notRun`; os percursos pararam antes de resume ou força. Consulta às threads dos fixtures próprios, por cwd exato e `sourceKinds` `appServer`/`exec`, identificou duas operações `commandExecution` por thread com saída **1** e erro MXC `cannot locate required Windows platform directory`, HRESULT **`0x80070003`**. Nenhum transcript foi salvo.

O [diagnóstico sem inferência](../../docs/research/evidence/codex-m0-job-mxc-diagnostic.json) comparou o mesmo comando fixo: direto teve saída **0** e cinco checks; via Job teve saída **1** com o erro MXC. A correção acrescentou **somente `SYSTEMDRIVE`** à allowlist JavaScript e ao ambiente raw C#. O [libuv do Node 24.19.0](https://github.com/nodejs/node/blob/v24.19.0/deps/uv/src/win/process.c) complementa `SystemDrive` e outras variáveis em `required_vars`/`make_program_env`; a bridge nativa precisa fornecer seu bloco mínimo explicitamente. A correção não herda o ambiente integral nem muda Job, sandbox, pins ou deadlines.

Após a correção, o comando fixo passou direto e via Job, com saída **0** e **cinco checks cada**; o Job encerrou `confirmed`/`graceful`. Check passou, assim como **131/131 offline em 39,67 s**, **21/21 de transporte em 40,75 s** e **9/9 do helper em 28,28 s**, sem falhas ou skips. A prova do helper com stdout bloqueado conservou deadline de **4 s**, membros encerrados em **3580 ms**, helper em **4599 ms**, **19 frames**, **82837 bytes**, frame final parcial e nenhuma mensagem `stopped` inventada.

Nenhuma prova live pós-correção foi executada e não há retry automático. A autorização original deixou três turnos sem uso; não inicia outra sequência nem redistribui esse orçamento automaticamente. O próximo passo é uma prova live com autorização específica, mantendo **90 s por turno**, **300 s por Job**, execução serial e nenhum fallback pago. A [comparação CLI/app-server](../../docs/research/cli-app-server-comparison.md) conserva as limitações de configuração reconhecida e vínculo observável da conta. Edição/retomada/força integradas continuam pendentes; OX-001/M0 permanecem abertos.

Em 09/10/2026, a [preparação offline](../../docs/research/offline-lifecycle-validation.md) corrigiu admissão de turnos e acompanhamento/interrupção concorrentes, e verificou descendente sintético sobrevivendo ao pai no Windows. **48 testes offline passaram**; nenhum runtime real foi chamado nessa continuação. Naquela data, o piloto D1 permanecia pendente; o reteste de 12/10 liberou seu gate.

Em 10/10/2026, o mesmo registro acrescenta limites de saída JSONL e da fila de escrita: **58/58 testes offline passaram**, incluindo dez novos casos de fronteiras UTF-8, rejeição local, backpressure e admissão. Essa rodada não executou os oito testes nativos Windows novamente, nem chamou agentes reais ou mudou os gates de D1/M0.

## Executar

Há também uma [prova nativa separada de contenção Windows](../../docs/research/windows-process-containment.md): os oito casos históricos cresceram para nove e permanecem separados da integração. Requer Windows 10+ x64, Windows PowerShell/Add-Type e Node instalados; compila o [helper C#](windows/README.md) localmente, sem dependência de pacote ou instalação. `npm.cmd run test:windows-job` lança somente a fixture própria; `npm.cmd run test:windows-transport` testa a bridge integrada com fake runtime. Nenhuma dessas suítes chama providers ou promove o adapter a produção.

```powershell
cd tools/runtime-harness
npm test
npm run probe:fake
node probe.mjs --fake --scenario permission
node probe.mjs --fake --scenario hang
npm run probe:codex
```

`probe:codex` consulta metadados e **não inicia inferência**. O Codex precisa estar no PATH como executável nativo; também é possível informar `--binary 'C:\caminho\codex.exe'`. A versão histórica validada foi **0.162.0-alpha.2**; a continuação de 12/10 admite **0.162.0-alpha.17.2** após schema/metadados. Outra versão produz diagnóstico, sem tentar adaptar silenciosamente o contrato.

O teste live é opt-in e usa a autenticação existente pelo fluxo oficial do runtime:

```powershell
node probe.mjs --codex --live
```

Antes dos dois turnos, exige `account.type=chatgpt`, `requiresOpenaiAuth=true`, `ordinaryUsageAllowed=true`, provider/endpoint oficiais resolvidos e limites de execução confirmados. Desconhecido ou incompatível bloqueia o teste. Não faz login/logout, copia autenticação, cria outra conta ou usa API como fallback. O próprio Codex pode renovar sua autenticação normalmente. O teste consome uso da assinatura.

O filho recebe um ambiente com allowlist de variáveis de sistema/localização. Hooks, plugins, apps e notify legado são desativados por overrides do processo. Servidores MCP configurados são desativados individualmente em um reinício anterior à criação da thread, com nova consulta de configuração; nomes incompatíveis ou ausência de confirmação bloqueiam live. Não modifica a configuração pessoal. `--no-daemon` evita conectar ao daemon compartilhado.

O harness cria um repositório temporário descartável; o primeiro turno pede somente uma resposta constante e o segundo solicita uma resposta longa e imediatamente pede interrupção. A política resolvida exige leitura, sem rede de ferramentas, aprovação `never`, reviewer `user`, cwd esperado e thread ephemeral. Instruções para não usar ferramentas não são enforcement adicional. Rede do sandbox e rede do runtime para inferência são distintas.

## Artefatos e resultados

Relatórios JSON ficam em `artifacts/`, ignorado pelo Git; `--output <path>` escolhe outro destino. Contêm campos de uma allowlist: categorias/booleans, configurações, eventos normalizados, contagens e confirmação de término. IDs de thread/turn/request viram referências locais. Não gravam e-mail, plano, account IDs, tokens, conteúdo de mensagens, stdout bruto, stderr ou caminhos pessoais de autenticação. Erros são categorias locais, sem mensagens brutas do provedor.

O repositório temporário é mantido para inspeção, sem limpeza recursiva automática. Somente seu conteúdo é comparado antes/depois; o runtime pode atualizar seu próprio cache. Não se assume isolamento de autenticação pelo simples fato de criar subprocessos.

O [resultado registrado](../../docs/research/runtime-harness-results.md) distingue fake, metadados e live. As [fixtures](fixtures/README.md) descrevem os 11 cenários. Auth/rate limit sintéticos não comprovam os códigos usados pelo serviço real. Cenários de falha devolvem exit code não zero propositalmente.

## Contrato experimental

`JsonRpcProcess` cuida de JSONL/UTF-8, correlação, requests bidirecionais, limites, timeouts e subprocesso próprio. Drena stderr sem registrar seu conteúdo. Respostas atrasadas não assumem ownership de outro request. Erro de framing encerra o processo próprio; não repete turn/start.

Os limites de bytes precisam ser inteiros positivos seguros; a validação precede o lançamento do filho. `maxMessageBytes` limita a linha recebida, sem LF, em 1 MiB por padrão. `maxOutboundMessageBytes` limita o frame enviado em 1 MiB **incluindo LF**, medido em bytes UTF-8. `maxPendingWriteBytes` limita o orçamento de escrita pendente do Writable a 2 MiB. Antes de cada escrita, a soma de `stdin.writableLength` e bytes do novo frame precisa caber nesse orçamento.

Frame excessivo produz `outbound-limit`; JSON não serializável produz `arguments`. Ambos trazem `dispatch: not-written`, sem enviar o frame ou encerrar um transporte saudável. O pedido e seu timer são removidos. Uma resposta recusada localmente preserva o request ID do servidor para uma resposta explícita válida. Um `turn/start` rejeitado antes de escrever não registra turno/terminal e permite somente outra ação explícita, desde que não haja falha ou notificações contraditórias.

Orçamento de fila excedido produz `backpressure`: o cliente falha, rejeita pedidos pendentes e solicita cleanup próprio. Sem bridge, isso pede término somente ao pai próprio ainda ativo; com `windowsJob`, o cleanup permanece restrito ao Job próprio. O turno fica incerto e uma segunda admissão permanece bloqueada. Não há fila de retry. Retorno `false` de `stdin.write` significa que o frame foi aceito pelo Writable; esse frame nunca é reenviado. Falha síncrona de escrita é `transport` com `dispatch: unknown`, sem afirmar que o frame chegou ao runtime. Fechamento reentrante durante a serialização também impede uma escrita posterior.

Esses limites cobrem frames e a fila local de escrita, **não a memória global do processo**: `JSON.stringify` ainda cria uma representação temporária antes de medir; buffers do sistema operacional, quantidade de RPCs simultâneos e outras coleções têm limites distintos ou permanecem trabalho futuro. Backpressure neste cliente não resolve o watchdog independente do helper Windows.

Saída do pai e fechamento dos pipes são observações distintas. No caminho sem Job, depois de `exit`, drena por até **250 ms**; se pipes herdados permanecerem abertos, desliga seus endpoints e marca `observationDetached=true`, `stdioClosed=false`, mantendo `treeTermination=unknown`. Não envia kill a um pai já encerrado. Com Job, membros restantes continuam sujeitos à supervisão do Job próprio mesmo após saída da raiz, preservando a distinção entre término da árvore e drenagem de I/O. `stop()` compartilha uma operação; falha de lançamento não inventa um processo pai terminado.

`CodexSession` traduz somente a parte necessária ao spike:

| Evento normalizado | Significado |
| --- | --- |
| `runtime.initialized` | Handshake confirmado. |
| `connection.observed` | Tipo/boolean de autenticação observado. |
| `session.started` | Configuração resolvida e vínculo da sessão observados. |
| `attempt.turn.started` | Início de turno da sessão/tentativa correspondente. |
| `response.delta` | Quantidade de bytes, sem persistir conteúdo. |
| `permission.observed/denied` | Request específico identificado e rejeitado. |
| `attempt.interrupt.requested` | Pedido de interrupção reconhecido; ainda não confirma fim. |
| `attempt.turn.ended` | Status terminal confirmado pelo runtime. |
| `attempt.observation.unknown` | Timeout sem confirmação de resultado. |

Envelope: `schemaVersion`, `sequence`, `attemptId`, `type`, campos específicos. Eventos de provedor não normalizados conservam somente o nome do método como diagnóstico. Modelo/effort resolvidos não são telemetria de qual execução serviu o turno. Sessão/turno não representam conclusão ou aplicação de uma Task/Run no Git.

Um turno ativo/incerto bloqueia outro início; ACK perdido não autoriza retry. Acompanhamento e interrupção compartilham o terminal com deadlines independentes; interrupções concorrentes enviam um RPC. Uma confirmação terminal recebida permanece preservada mesmo quando o ACK de interrupção falha. Perda do ACK de início sem identidade comprovada ainda exige recuperação futura, não um reset silencioso.

Aprovações de comando/arquivo recebem `decline`; pedidos de permissões recebem `{permissions:{},scope:'turn'}`. Requests desconhecidos recebem erro; não existe aceite genérico. O parser permite notifications desconhecidas bem formadas e limita frames a 1 MiB, conteúdo de resposta a 64 KiB e eventos a 4096.

## Limites para produção

Sem `windowsJob`, este supervisor confirma apenas o término do **processo pai** e mantém `treeTermination=unknown`, inclusive quando um descendente sintético foi observado ativo. A [bridge integrada de Job Objects](../../docs/research/windows-process-containment.md#integração-do-transporte--1210) confirma membros associados nos casos sintéticos e cleanup de app-servers reais sem inferência; a prova durante trabalho do modelo continua pendente. Não certifica sandbox de arquivos/rede, processos delegados a serviços fora do Job ou todas as combinações de Jobs ambientais. Permanecem pendentes crash/reconciliação durável, revisão/aplicação de produção, matriz live CLI versus app-server e isolamento de duas contas. Dois processos fake testam correlação; não comprovam cotas independentes.

Os testes deste diretório são offline. Não tornam live parte do CI e não instalam providers ou toolchains. O contrato Rust final de OX-003 deve incorporar os resultados, preservando as separações de estado estabelecidas no [backlog](../../docs/DEVELOPMENT_BACKLOG.md).
