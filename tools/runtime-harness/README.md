# Orchestrix runtime harness — M0

Experimento isolado de protocolo, executável com **Node 24+ e Git**, sem dependências de pacote. Não é o Core de produção nem muda a stack proposta Rust/Tauri. O protótipo visual em `prototypes/desktop` continua simulado.

Foi uma antecipação técnica. O foco atual do plano é D0 → D1 com piloto inicial → retomada de M0. Preservar este experimento não encerra os gates de pesquisa/design ou promove o adapter a produção.

Em 09/10/2026, a [preparação offline](../../docs/research/offline-lifecycle-validation.md) corrigiu admissão de turnos e acompanhamento/interrupção concorrentes, e verificou descendente sintético sobrevivendo ao pai no Windows. **48 testes offline passaram**; nenhum runtime real foi chamado nessa continuação. O piloto D1 permanece pendente.

Em 10/10/2026, o mesmo registro acrescenta limites de saída JSONL e da fila de escrita: **58/58 testes offline passaram**, incluindo dez novos casos de fronteiras UTF-8, rejeição local, backpressure e admissão. Essa rodada não executou os oito testes nativos Windows novamente, nem chamou agentes reais ou mudou os gates de D1/M0.

## Executar

Há também uma [prova nativa separada de contenção Windows](../../docs/research/windows-process-containment.md): **oito casos passaram**, além dos 48 anteriores. Requer Windows 10+ x64, Windows PowerShell/Add-Type e Node instalados; compila o [helper C#](windows/README.md) localmente, sem dependência de pacote ou instalação. Executar com `npm.cmd run test:windows-job`. A prova lança somente a fixture própria e permanece separada de `JsonRpcProcess`/Codex; não promove o adapter a produção.

```powershell
cd tools/runtime-harness
npm test
npm run probe:fake
node probe.mjs --fake --scenario permission
node probe.mjs --fake --scenario hang
npm run probe:codex
```

`probe:codex` consulta metadados e **não inicia inferência**. O Codex precisa estar no PATH como executável nativo; também é possível informar `--binary 'C:\caminho\codex.exe'`. A versão validada nesta rodada é **0.162.0-alpha.2**. Outra versão produz diagnóstico, sem tentar adaptar silenciosamente o contrato.

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

Orçamento de fila excedido produz `backpressure`: o cliente falha, rejeita pedidos pendentes e pede término somente ao pai próprio ainda ativo. O turno fica incerto e uma segunda admissão permanece bloqueada. Não há fila de retry. Retorno `false` de `stdin.write` significa que o frame foi aceito pelo Writable; esse frame nunca é reenviado. Falha síncrona de escrita é `transport` com `dispatch: unknown`, sem afirmar que o frame chegou ao runtime. Fechamento reentrante durante a serialização também impede uma escrita posterior.

Esses limites cobrem frames e a fila local de escrita, **não a memória global do processo**: `JSON.stringify` ainda cria uma representação temporária antes de medir; buffers do sistema operacional, quantidade de RPCs simultâneos e outras coleções têm limites distintos ou permanecem trabalho futuro. Backpressure neste cliente não resolve o watchdog independente do helper Windows.

Saída do pai e fechamento dos pipes são observações distintas. Depois de `exit`, drena por até 250ms; se pipes herdados permanecerem abertos, desliga seus endpoints e marca `observationDetached=true`, `stdioClosed=false`, mantendo `treeTermination=unknown`. Não envia kill a um pai já encerrado. `stop()` compartilha uma operação; falha de lançamento não inventa um processo pai terminado.

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

Este supervisor confirma apenas o término do **processo pai**; `treeTermination` permanece `unknown`, inclusive quando um descendente sintético foi observado ativo. A [prova de Job Objects](../../docs/research/windows-process-containment.md) confirma processos associados somente no helper sintético separado; ainda faltam integração no supervisor e término comprovado com runtime real, retomada, crash/reconciliação durável, execução de código em worktree, checks/revisão/aplicação, matriz CLI versus app-server e isolamento de duas contas. Dois processos fake testam correlação; não comprovam cotas independentes.

Os testes deste diretório são offline. Não tornam live parte do CI e não instalam providers ou toolchains. O contrato Rust final de OX-003 deve incorporar os resultados, preservando as separações de estado estabelecidas no [backlog](../../docs/DEVELOPMENT_BACKLOG.md).
