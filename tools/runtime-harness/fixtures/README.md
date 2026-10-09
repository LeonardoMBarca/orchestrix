# Fixtures offline do app-server

Também há uma [fixture de processos](process-tree.mjs) para a preparação offline de 09/10. Ela cria um descendente Node próprio `detached`, mantendo pipes herdados depois da saída do pai. O descendente se encerra sozinho em 2,5s; o pai tem timer de segurança. Não há autenticação, rede ou leitura de projeto. Os [testes de ciclo de processo](../tests/process-lifecycle.test.mjs) conferem pai/transporte/árvore separadamente, sem kill por PID do descendente. [Resultados e limites](../../../docs/research/offline-lifecycle-validation.md).

`fake-app-server.mjs` é um processo Node 24 que fala envelopes JSON-RPC delimitados por linha em stdin/stdout. Não chama Codex, modelos ou redes; não lê autenticação, não executa comandos e não modifica repositórios ou arquivos. stderr permanece separado do protocolo. A execução usa somente módulos da stdlib e não exige instalar dependências.

```powershell
node .\tools\runtime-harness\fixtures\fake-app-server.mjs --scenario success
```

O processo aguarda requisições JSONL. `--scenario=success` também é aceito; cenário padrão é `success`. `--help` escreve ajuda em stderr. Argumento/cenário inválido termina com exit code 2. stdout contém apenas envelopes de protocolo, exceto a linha inválida intencional do cenário `malformed`.

## Contrato mínimo

Enviar `initialize` com um ID e, depois, a notificação `initialized`. O fixture aceita o campo opcional `jsonrpc: "2.0"` no input; suas respostas usam o envelope do app-server, sem esse campo.

```jsonl
{"id":1,"method":"initialize","params":{"clientInfo":{"name":"orchestrix-fixture-client","version":"0"}}}
{"method":"initialized","params":{}}
{"id":2,"method":"account/read","params":{"refreshToken":false}}
{"id":3,"method":"model/list","params":{}}
{"id":4,"method":"thread/start","params":{"sandbox":"read-only","approvalPolicy":"never","ephemeral":true,"config":{"model_reasoning_effort":"low"}}}
{"id":5,"method":"turn/start","params":{"threadId":"thread-fake-001","input":[{"type":"text","text":"Synthetic task"}]}}
```

IDs de requisição são preservados. Thread/turn/item/approval IDs usam sequência determinística por processo: `thread-fake-001`, `turn-fake-001`, `item-fake-001` e `approval-fake-001`. `thread/resume` aceita apenas threads criadas no mesmo processo; não há persistência entre reinícios. `turn/interrupt` exige a identidade correspondente de thread/turn, responde `{}` e emite conclusão `interrupted` sem duplicar conclusões já emitidas.

`account/read` retorna uma conta `chatgpt` sintética, com `email: null`, plano fictício `pro`, `requiresOpenaiAuth: true` e sem credenciais. `model/list` retorna apenas `fake-model`, com esforços `low`, `medium` e `high` fictícios; `low` é o padrão. `account/rateLimits/read` retorna limites nulos e `ordinaryUsageAllowed: true` sintético na raiz, inclusive no cenário `rate-limit`, que falha no início do turno. `config/read` retorna rotas fictícias de OpenAI/ChatGPT, sem consultar configurações reais nem acessar os endpoints.

`thread/start` conserva `ephemeral` e reflete modelo, esforço de `config.model_reasoning_effort`, `approvalPolicy` e `approvalsReviewer` recebidos no descriptor retornado; `thread/resume` devolve esse descriptor no mesmo processo. Sandbox permanece o objeto sintético `readOnly` com rede desativada. Strings de plataforma/home, provider, sandbox, catálogo, endpoints e políticas são dados do fixture; não são capacidades observadas ou autenticação confirmada do ambiente real. O caminho sintético de home não é criado.

As notificações principais são `turn/started`, `item/agentMessage/delta` e `turn/completed`. O turno contém `id`, `items: []`, `status` e `error`. Deltas são resultados sintéticos identificados; não representam arquivos, commits ou checks executados. Campos mínimos foram alinhados à inspeção do schema local de Codex **0.162.0-alpha.2**; o fixture não é uma implementação completa desse schema e precisa acompanhar a superfície escolhida pelo harness.

## Cenários

| `--scenario` | Comportamento observável |
| --- | --- |
| `success` | Inicia turno, emite mensagem sintética e conclui após um timer curto. |
| `permission` | Emite server request `item/commandExecution/requestApproval` e aguarda resposta do cliente. `decline` continua e conclui sem comando; `cancel` interrompe. Outros resultados falham explicitamente, sem execução. |
| `auth-failure` | `account/read` tem `account: null`; `turn/start` retorna erro RPC sintético de autenticação, sem iniciar turno. |
| `rate-limit` | `turn/start` retorna erro RPC sintético de limite, com `retryAfterSeconds: 1`, sem iniciar turno. Não mede ou consome cota. |
| `malformed` | Depois de iniciar turno, escreve uma linha JSON inválida, seguida de eventos válidos. Permite verificar o diagnóstico do transporte; o harness decide se interrompe ou continua. |
| `crash` | Depois de resposta de início e `turn/started`, escreve diagnóstico em stderr e termina com exit code 23. |
| `hang` | Emite início e delta, conserva processo vivo e nunca conclui sozinho. Pode ser interrompido pelo cliente ou encerrado pelo supervisor. |
| `split-utf8` | Usa CRLF; fragmenta envelopes e separa bytes dentro do emoji `🧩`. Um delta contém Unicode e newline escapado. Escritas são espaçadas; o SO ainda pode reagrupar chunks. |
| `noisy-stderr` | Emite aproximadamente 1 MiB de ruído sintético em stderr antes da conclusão. stdout continua exclusivamente JSONL. |
| `unknown-notification` | Insere `fixture/unknownNotification` antes da conclusão normal. |
| `slow` | Emite início e delta, concluindo após 10 segundos se não houver `turn/interrupt`. Não bloqueia processamento de requisições. |

Resposta esperada para o pedido sintético de permissão:

```jsonl
{"id":"approval-fake-001","result":{"decision":"decline"}}
```

Interrupção do primeiro turno:

```jsonl
{"id":6,"method":"turn/interrupt","params":{"threadId":"thread-fake-001","turnId":"turn-fake-001"}}
```

Os códigos RPC `-32001` e `-32029` e os dados `kind` de auth/limite são convenções sintéticas para estes cenários, não um contrato de classificação de erros do provedor. Requisição desconhecida usa `-32601`; thread/turn inválidos usam `-32602`; input JSON inválido recebe `-32700`.

## Limitações e uso no harness

- Uma thread aceita um turno ativo; threads diferentes podem manter turnos separados. Repetir `initialize` não reinicia o estado. A notificação `initialized` é tolerada, mas seu envio não é exigido pelo fake.
- Não há validação completa de parâmetros, descoberta real de modelo, sandbox efetivo, aplicação de permissões, ferramentas, compaction, contexto, billing ou telemetria. `items` fica vazio mesmo quando há deltas de mensagem.
- `permission` exercita somente aprovação de comando negada/cancelada. Aprovação de filesystem e grants de permissions possuem contratos diferentes e não estão simulados aqui.
- Somente Node timers e pipes são reais. Nenhum cenário fornece evidência de comportamento, quota ou isolamento de conta de um provider.
- `hang` e aprovação pendente conservam um timer ativo, inclusive após EOF do input, para que timeout/encerramento fiquem sob controle do harness. Fechar stdin sozinho não os cancela.
- O fake não implementa retry, scheduler, domínio de tarefas ou testes do supervisor. O harness deve iniciar/encerrar o processo, drenar os dois streams, interpretar eventos e verificar suas próprias garantias.

Referência do protocolo a refinar nos spikes: [Codex app-server](https://learn.chatgpt.com/docs/app-server). Fixtures reais devem registrar sua própria versão e excluir credenciais antes de serem persistidas.
