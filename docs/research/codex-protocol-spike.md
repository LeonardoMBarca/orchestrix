# Spike de protocolo Codex app-server

Inspeção em **8 de outubro de 2026**. Este registro orienta o harness experimental; não certifica adapter, sandbox, isolamento de contas ou autorização comercial. A inspeção local executou somente `--version`, `--help` e geração de schema. Não executou servidor, login, leitura de conta, acesso a credenciais ou inferência.

## Evidência local e versão

- Binário observado: `codex-cli 0.162.0-alpha.2`.
- Ajuda: `codex app-server --help` e `codex app-server generate-json-schema --help`.
- Schema: `codex app-server generate-json-schema --out <TEMP>/orchestrix-codex-protocol-0.162.0-alpha.2`, sem `--experimental`.
- SHA-256 de `codex_app_server_protocol.schemas.json`: `4d1632bf3bd229f587fcfe975723844202540b67692a739520a4a65593a7396b`.

Os campos nas próximas tabelas vêm desse schema gerado pelo binário, e não de modelos presumidos. O bundle ficou no cache temporário, fora do repositório. A ajuda classifica app-server e geração de schema como experimentais. Registrar a versão suportada no harness e falhar com diagnóstico quando houver incompatibilidade.

A documentação oficial descreve JSON-RPC bidirecional, sem o campo `jsonrpc` no wire. Stdio usa um objeto JSON por linha. Requests têm `id`, `method`, `params`; responses devolvem `id` com `result` ou `error`; notifications não têm `id`. O handshake precede qualquer outro método. Schemas gerados correspondem à versão concreta do executável. [Codex App Server](https://learn.chatgpt.com/docs/app-server)

## Comandos mínimos e campos exatos

| Método | Params da versão local | Resposta/observação relevante |
| --- | --- | --- |
| `initialize` | `clientInfo: {name, version, title?}`; `capabilities` opcional | `userAgent`, `platformFamily`, `platformOs`, `codexHome`. Não publicar o path pessoal. |
| `initialized` | Notification com `{}` depois do sucesso de initialize | Não possui resposta. |
| `account/read` | `{refreshToken: false}` | `account` opcional/nulo; `requiresOpenaiAuth` obrigatório. Sanitizar imediatamente. |
| `account/rateLimits/read` | `{supportsLunaReserve: false, excludeResetCreditDetails: true}` ou params omitidos/nulos | `rateLimits` obrigatório; `ordinaryUsageAllowed` pode ser boolean/nulo. Não publicar IDs, plano, saldo ou payload bruto. |
| `model/list` | `{includeHidden: false, limit: 100}`; `cursor` para paginação | `data`; `nextCursor` opcional/nulo. Usar `data[].model` como identificador solicitado. |
| `thread/start` | `model`, `modelProvider`, `cwd`, `approvalPolicy`, `approvalsReviewer`, `sandbox`, `ephemeral`; todos opcionais no schema, explícitos no spike | `thread`, `model`, `modelProvider`, `cwd`, `approvalPolicy`, `approvalsReviewer`, `sandbox` obrigatórios; `reasoningEffort` opcional/nulo. |
| `thread/resume` | `threadId` obrigatório; admite overrides semelhantes aos de start | Exige vínculo original de sessão, conexão e ambiente. Não usar seleção da última sessão. |
| `turn/start` | `threadId`, `input` obrigatórios; `model`, `effort`, `approvalPolicy`, `approvalsReviewer`, `sandboxPolicy`, `cwd` opcionais | `{turn: {id, items, status, ...}}`; resposta inicial não confirma conclusão. |
| `turn/interrupt` | `{threadId, turnId}` | Resultado vazio; aguardar `turn/completed`. |

`initialize.capabilities.experimentalApi` deve permanecer falso no spike básico. Não anunciar filesystem, terminal, atestação ou outros recursos de cliente que não foram implementados.

Exemplo sintético de handshake:

```json
{"id":1,"method":"initialize","params":{"clientInfo":{"name":"orchestrix_runtime_harness","title":"Orchestrix Runtime Harness","version":"0.1.0"},"capabilities":{"experimentalApi":false}}}
```

Depois do ACK, enviar `{"method":"initialized","params":{}}`. Respostas e notifications podem chegar intercaladas; correlacionar por ID, sem pressupor posição no stream.

### Sandbox e raciocínio: diferenças de representação

No schema local, `thread/start.sandbox` é a string **`read-only`**, `workspace-write` ou `danger-full-access`. A resposta usa `SandboxPolicy`, por exemplo **`{type: "readOnly", networkAccess: false}`**. O override `turn/start.sandboxPolicy` também é um objeto. Não copiar o `workspaceWrite` de exemplos como valor da string de start.

`approvalPolicy` admite `on-request`, `never`, valor legado `untrusted` e uma configuração granular. Embora presente no schema, `untrusted` foi retirado da configuração atual e não deve ser escolhido. `approvalsReviewer` admite `user`, `auto_review`, `guardian_subagent`; usar `user` no spike. `never` com sandbox de leitura evita solicitar elevação; não representa bypass do sandbox. [Configuration Reference](https://learn.chatgpt.com/docs/config-file/config-reference)

Para o turno trivial:

```json
{"id":5,"method":"thread/start","params":{"model":"<model/list.data[].model>","modelProvider":"openai","cwd":"<diretorio-temporario-absoluto>","sandbox":"read-only","approvalPolicy":"never","approvalsReviewer":"user","ephemeral":true}}
```

Validar a configuração devolvida antes de começar o turno. O effort aparece em `turn/start.effort`; escolher uma string anunciada por `supportedReasoningEfforts[].reasoningEffort`, preferindo `low` quando disponível. Não assumir que todos os modelos admitem os mesmos níveis.

```json
{"id":6,"method":"turn/start","params":{"threadId":"<thread.id>","input":[{"type":"text","text":"Responda exatamente ORCHESTRIX_OK. Não use ferramentas, leia arquivos ou execute comandos."}],"model":"<modelo-escolhido>","effort":"<esforco-anunciado>","approvalPolicy":"never","approvalsReviewer":"user","sandboxPolicy":{"type":"readOnly","networkAccess":false}}}
```

A solicitação de não usar ferramentas é parte do cenário, não enforcement. A rede do sandbox das ferramentas e a rede usada pelo runtime para inferência são fronteiras distintas. O schema dessa alpha não oferece os novos campos de read-access presentes em exemplos atuais; não assumir suporte a eles.

`Thread.model` e `Thread.reasoningEffort` descrevem configuração atual/persistida; suas descrições locais dizem que não são telemetria de execução por turno. Registrar `requested` e `resolvedConfiguration`; não chamar a configuração de prova empírica do modelo que serviu a resposta. Catálogo também não comprova entitlement.

## Assinatura existente, sem fallback de API

A autenticação oficial diferencia ChatGPT, para acesso por assinatura, de API key, para cobrança por uso. CLI e extensão compartilham o cache de login. Portanto, um processo separado não significa conta ou armazenamento de autenticação separados. Não fazer login/logout no spike nem copiar arquivos pessoais de autenticação. [Authentication](https://learn.chatgpt.com/docs/auth)

Gate proposto para o harness real:

1. Iniciar processo próprio por stdio, em diretório temporário, sem `daemon`, `proxy`, socket do daemon, endpoint remoto ou `--code-mode-host` remoto.
2. Construir ambiente do filho sem API keys, tokens externos ou overrides de autenticação/endpoints. Não imprimir valores. Preservar somente o necessário à execução e à localização da autenticação existente.
3. Usar `account/read` com `refreshToken: false`. Admitir o cenário somente se `account.type === "chatgpt"` e o caminho esperado de autenticação OpenAI estiver ativo. Qualquer outro tipo/nulo interrompe antes de inferência.
4. Consultar disponibilidade. `ordinaryUsageAllowed === false` bloqueia o turno; nulo permanece desconhecido. Para o primeiro teste estrito de uso incluído, exigir confirmação positiva; não inferir permissão de percentual, reset ou presença de créditos. Manter `supportsLunaReserve: false`, sem consumir crédito de reset.
5. Usar provider builtin `openai` explicitamente e validar resposta de thread/start. Se uma política gerenciada resolver outro provider/permissão, parar; não reduzir a restrição para fazer o teste passar.
6. Executar um turno trivial somente após os gates. Auth failure, rate limit ou timeout não autorizam login automático, mudança de provider/modelo ou segunda tentativa por API.

No código oficial da **tag exata** instalada, o provider builtin OpenAI não tem `env_key`, exige autenticação OpenAI e escolhe a rota ChatGPT por modo de autenticação quando não há override. Uma `base_url` explícita substitui esse default. [ModelProviderInfo — 0.162.0-alpha.2](https://raw.githubusercontent.com/openai/codex/rust-v0.162.0-alpha.2/codex-rs/model-provider-info/src/lib.rs)

Na mesma tag, `openai_base_url` vazio vira ausência de override; `chatgpt_base_url` tem default `https://chatgpt.com/backend-api/`. O harness pode fixar esses valores por argumentos estruturados `-c`, sem editar config pessoal. Configuração gerenciada tem precedência no provider; a resposta resolvida deve ser conferida. Não considerar `modelProvider: "openai"` sozinho prova do destino HTTP. [Config — 0.162.0-alpha.2](https://raw.githubusercontent.com/openai/codex/rust-v0.162.0-alpha.2/codex-rs/core/src/config/mod.rs)

O auth manager dessa tag consulta `CODEX_API_KEY` com precedência quando esse caminho está habilitado. Remover essa variável do filho, além de `OPENAI_API_KEY` e `CODEX_ACCESS_TOKEN`; remover overrides de refresh/revoke/client ID. Sua recuperação de 401 recarrega a mesma conta e tenta refresh OAuth, sem troca para API nessa máquina de estados. `forced_login_method` incompatível pode efetuar logout: não introduzir essa opção sobre armazenamento compartilhado como substituto do gate. [Auth Manager — 0.162.0-alpha.2](https://raw.githubusercontent.com/openai/codex/rust-v0.162.0-alpha.2/codex-rs/login/src/auth/manager.rs)

Esses gates limitam o cenário ao caminho observado de assinatura. Não certificam condições de distribuição pública/comercial, identidade independente de duas contas ou ausência de mudanças normais no cache do runtime. Um turno com autenticação gerenciada pode renovar credenciais pelo próprio Codex.

## Inicialização de extensões na versão examinada

Esta seção registra **inspeção do código oficial**, sem concluir resultados de execução. Na tag `rust-v0.162.0-alpha.2`, tabelas de configuração são mescladas recursivamente: `mcp_servers: {}` conserva servidores herdados. O campo `mcp_servers.<nome>.enabled=false` impede a inicialização daquele servidor. Extrair apenas nomes em memória de `config/read`, construir overrides estruturados e reiniciar o subprocesso antes de criar a thread; conferir novamente que todos os servidores observados estão desativados. Não persistir nomes, comandos, URLs ou credenciais de MCP. [Merge](https://raw.githubusercontent.com/openai/codex/rust-v0.162.0-alpha.2/codex-rs/config/src/merge.rs), [MCP config](https://raw.githubusercontent.com/openai/codex/rust-v0.162.0-alpha.2/codex-rs/config/src/mcp_types.rs)

`Session::new` instala o runtime MCP e agenda prewarm durante a criação da sessão. O hook `SessionStart` fica pendente e é despachado no processamento dos hooks do turno. Assim, negar aprovações depois de `thread/start` não bloqueia toda atividade de inicialização. [Session startup](https://raw.githubusercontent.com/openai/codex/rust-v0.162.0-alpha.2/codex-rs/core/src/session/session.rs), [Hook runtime](https://raw.githubusercontent.com/openai/codex/rust-v0.162.0-alpha.2/codex-rs/core/src/hook_runtime.rs)

Fixar `features.hooks=false`, `features.plugins=false` e `features.apps=false` no subprocesso e exigir os respectivos booleans em `config/read`. O `notify` legado é construído independentemente do boolean dos hooks modernos; fixar também `notify=[]` e conferir a lista vazia. Esses bloqueios pertencem ao experimento, sem editar configuração pessoal. [Feature registry](https://raw.githubusercontent.com/openai/codex/rust-v0.162.0-alpha.2/codex-rs/features/src/lib.rs), [Hook registry](https://raw.githubusercontent.com/openai/codex/rust-v0.162.0-alpha.2/codex-rs/hooks/src/registry.rs)

O engine desativa hooks ordinários; se receber fontes de plugin, pode conservar handlers internos de limpeza. Não descrever o boolean isolado como garantia de ausência de qualquer execução interna do runtime. [Hook engine](https://raw.githubusercontent.com/openai/codex/rust-v0.162.0-alpha.2/codex-rs/hooks/src/engine/mod.rs)

No caminho examinado de startup stdio, o construtor de `McpManager` guarda managers e caches, sem iniciar conexões. Recuperação automática de threads exige transporte UnixSocket e `managed_daemon`; não se aplica ao stdio direto. Isso sustenta o bootstrap limitado a initialize/account/model/config antes de criar uma sessão, sem chamadas de descoberta, recursos ou ferramentas MCP. Não demonstra ausência de efeitos gerais no armazenamento do runtime. [MCP manager](https://raw.githubusercontent.com/openai/codex/rust-v0.162.0-alpha.2/codex-rs/core/src/mcp.rs), [App-server startup](https://raw.githubusercontent.com/openai/codex/rust-v0.162.0-alpha.2/codex-rs/app-server/src/lib.rs)

## Eventos, aprovação e cancelamento

| Evento/request | Campos obrigatórios do schema local | Tratamento |
| --- | --- | --- |
| `turn/started` e `turn/completed` | `threadId`, `turn` | Turn exige `id`, `items`, `status`. Status: `inProgress`, `completed`, `failed`, `interrupted`. |
| `item/agentMessage/delta` | `threadId`, `turnId`, `itemId`, `delta` | Correlacionar antes de acumular. Não imprimir mensagens brutas por padrão. |
| `item/commandExecution/requestApproval` | `threadId`, `turnId`, `itemId`, `startedAtMs`; callback pode ter `approvalId` | Responder ao ID do request exato com `{decision: "decline"}` ou `cancel`; não aceitar grants persistentes. |
| `item/fileChange/requestApproval` | `threadId`, `turnId`, `itemId`, `startedAtMs` | Mesmo formato `{decision: "decline"}`; `cancel` interrompe. |
| `item/permissions/requestApproval` | `threadId`, `turnId`, `itemId`, `startedAtMs`, `cwd`, `permissions` | Formato diferente: `{permissions: {}, scope: "turn"}` não concede permissões adicionais. |

`decline` permite que o agente continue; `cancel` solicita interrupção. Uma resposta a aprovação usa `{id: <id-recebido>, result: <payload>}`, sem method. Requests desconhecidos recebem erro de método não implementado e diagnóstico sanitizado, nunca aprovação genérica.

Cancelamento é uma operação observável: solicitar `turn/interrupt`, aguardar status terminal e então verificar encerramento do processo supervisionado. O ACK não confirma término. Timeout/perda de stdout mantém resultado desconhecido até reconciliação; não reenviar turn/start para recuperar um ACK perdido. Se o turno já tiver terminado, preservar seu resultado confirmado.

## Saída sanitizada e aceite do experimento

Persistir resumo por allowlist, por exemplo: versão, transporte, método, correlation local, configurações solicitadas/resolvidas, `auth.type`, `requiresOpenaiAuth`, boolean de disponibilidade, status terminal, aprovação rejeitada, contagem de eventos e `expectedTextMatched`. Account data deve conter somente tipo/booleans. Descartar e-mail, account/workspace IDs, plano, tokens, saldo, payload bruto e path do CODEX_HOME antes de qualquer logging.

Stderr deve ser drenado sem exposição direta: pode conter informações de conta, paths ou erros sensíveis. Logs e erros de diagnóstico do harness devem usar categorias locais. A mensagem esperada pode ser comparada em memória, conservando somente o resultado booleano; não gravar transcript pessoal.

O spike passa quando o handshake, os gates, a configuração resolvida e o término do turno forem observados, a resposta esperada corresponder e o processo próprio terminar sem alterar o repositório. Fake verifica parsing/estados; turno real verifica somente essa combinação de versão, autenticação e configuração. O cenário trivial não prova execução de código, recuperação persistente, cancelamento durante ferramenta, sandbox Windows ou multiaccount. Retomada real exige experimento separado, pois thread ephemeral não deve ser materializada para resume.
