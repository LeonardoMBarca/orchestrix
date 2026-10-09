# Orchestrix runtime harness — M0

Experimento isolado de protocolo, executável com **Node 24+ e Git**, sem dependências de pacote. Não é o Core de produção nem muda a stack proposta Rust/Tauri. O protótipo visual em `prototypes/desktop` continua simulado.

Foi uma antecipação técnica. O foco atual do plano é D0 → D1 com piloto inicial → retomada de M0. Preservar este experimento não encerra os gates de pesquisa/design ou promove o adapter a produção.

## Executar

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

Aprovações de comando/arquivo recebem `decline`; pedidos de permissões recebem `{permissions:{},scope:'turn'}`. Requests desconhecidos recebem erro; não existe aceite genérico. O parser permite notifications desconhecidas bem formadas e limita frames a 1 MiB, conteúdo de resposta a 64 KiB e eventos a 4096.

## Limites para produção

Este supervisor confirma apenas o término do **processo pai**; `treeTermination` permanece `unknown`. Ainda faltam Job Objects/árvore no Windows, retomada real, crash/reconciliação durável, execução de código em worktree, checks/revisão/aplicação, matriz CLI versus app-server e isolamento de duas contas. Dois processos fake testam correlação; não comprovam cotas independentes.

Os testes deste diretório são offline. Não tornam live parte do CI e não instalam providers ou toolchains. O contrato Rust final de OX-003 deve incorporar os resultados, preservando as separações de estado estabelecidas no [backlog](../../docs/DEVELOPMENT_BACKLOG.md).
