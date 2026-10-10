# Diagnósticos de conexões e recursos

Contrato de **10 de outubro de 2026** para o diagnóstico automático no cadastro de uma conexão e sua repetição em Settings. Complementa o [contrato de conexão](api-connection-contract.md) e a [descoberta de recursos](resource-discovery-increment.md). A interface deve explicar o resultado de várias verificações, específicas para a modalidade e o provedor, sem exigir que a pessoa conheça o protocolo do runtime.

O incremento usa uma lógica de diagnóstico separada da execução. A bridge de desenvolvimento pode ler catálogos API; o relatório reúne essas respostas, configuração e observações autorizadas de adapters. O cadastro de um runtime sem adapter autenticado produz verificações bloqueadas ou desconhecidas, em vez de simular login. Não há inferência, execução de ferramentas ou alteração de permissões durante esse diagnóstico. Autenticação de produção, cofre, worker confiável e gates existentes continuam pendentes.

## Fluxo automático e nova verificação

Ao registrar uma conexão válida, iniciar o diagnóstico para **aquela identidade e revisão**. A escolha Subscription/API precede a escolha do provedor; determina os campos, a autorização e o conjunto de operações possíveis. Para APIs, reaproveitar a leitura específica de catálogo da bridge, com credencial transitória, limites, cancelamento e paginação. Não fazer uma chamada por linha da interface se uma mesma resposta já contém a evidência necessária.

A sequência é:

1. Validar os campos não secretos, o backend, endpoint, versão, região e deployment/profile aplicáveis.
2. Verificar a identidade/autorização somente pela operação oficial que o adapter realmente expõe. Configuração preenchida não é identidade autenticada; acesso a catálogo não confirma inferência.
3. Ler o catálogo e preservar recursos distintos, origem, escopo, data e eventual parcialidade.
4. Derivar verificações independentes para capacidades, configuração e política do Orchestrix. Dados ausentes permanecem desconhecidos.
5. Mostrar resumo, verificações e detalhes por recurso, com a próxima ação apropriada ao bloqueio observado.

**Recheck** repete o diagnóstico no mesmo escopo. Não troca conta, região, endpoint, origem de cobrança ou permissões para obter um resultado melhor. Se a credencial transitória já não está disponível, a interface informa que a leitura depende de fornecê-la novamente; isso não revoga a conexão no provedor. Edição de identidade, desconexão ou remoção invalida a revisão em andamento. Uma resposta tardia não pode preencher outra conexão nem restaurar um recurso removido.

O relatório anterior pode ser mantido como evidência histórica, com sua data e alcance. Falha no recheck não transforma informação antiga em observação atual. Cancelamento encerra a rodada sem declarar que a capacidade foi reprovada. Limites de concorrência, duração, páginas e tamanho de resposta continuam sob responsabilidade do host/adapter.

## Três informações que a interface conserva separadas

| Informação | Pergunta respondida | Exemplo |
| --- | --- | --- |
| Suporte | O recurso é declarado para este modelo/runtime/backend? | Um modelo lista os esforços de reasoning aceitos. |
| Configuração e política | O recurso está selecionado ou permitido neste escopo? | O runtime possui delegação habilitada, mas a política do worker Orchestrix a mantém desativada. |
| Observação | Qual leitura ou execução forneceu a evidência? | Uma resposta de catálogo foi observada; nenhum turno foi executado. |

Suporte usa `supported`, `unsupported`, `unknown` e, para verificações que não pertencem à modalidade, `not-applicable`. Disponibilidade de um recurso usa `available`, `unavailable` ou `unknown`, conforme evidência de acesso. Um modelo listado pode ter suporte conhecido a reasoning e acesso de execução ainda desconhecido. A união de capacidades no resumo indica cobertura de **alguns** recursos; não concede o recurso a todos os modelos.

O estado de cada verificação usa outro eixo:

| Estado | Significado |
| --- | --- |
| `passed` | A verificação encontrou a evidência esperada. Pode ter confirmado que um recurso **não** é suportado. |
| `failed` | Houve falha observada de validação/leitura, com código tipado. Não é sinônimo de recurso ausente. |
| `pending` | A rodada está aguardando ou lendo os dados. |
| `blocked` | Falta um requisito, como host, adapter ou identidade de runtime vinculada. |
| `skipped` | A verificação não se aplica ou foi cancelada antes de concluir. |
| `unknown` | A fonte não informa a propriedade ou a evidência disponível não resolve a pergunta. |

Não exibir uma pontuação de “qualidade da conta” baseada no total verde. Uma conta adequada pode ter recursos não aplicáveis e telemetria desconhecida. A conclusão do relatório significa que as leituras terminaram, não que todos os modelos, ferramentas e permissões funcionaram em execução.

## Verificações comuns e específicas

O conjunto comum cobre configuração, identidade, catálogo, acesso, reasoning/thinking, contexto, function calling, streaming, pesquisa web nativa, saída estruturada, velocidade, multi-agent nativo, sandbox, aprovação, acesso completo, uso/créditos e política de delegação. Acrescentar verificações específicas por adapter; uma API de texto não herda os recursos do aplicativo de assinatura do mesmo fornecedor.

| Modalidade/backend | Leituras e verificações específicas | Limite da conclusão |
| --- | --- | --- |
| Codex por assinatura | Identidade vinculada, `model/list`, capacidades do provider e configuração sanitizada da versão instalada. Esforço, service tier, native multi-agent, sandbox e aprovação são eixos separados. | Sem adapter autenticado, os campos ficam bloqueados/desconhecidos. Catálogo e configuração não demonstram um turno. [App-server](https://learn.chatgpt.com/docs/app-server). |
| Claude Code por assinatura | Identidade e metadata oficialmente expostas pelo adapter/SDK, inclusive suporte a effort, thinking, velocidade e recursos nativos quando disponíveis na versão. | Não reutilizar o catálogo da API direta. Inicializar ferramentas, MCPs, hooks ou executar token counting não faz parte da leitura automática. [Claude Agent SDK](https://code.claude.com/docs/en/agent-sdk/typescript). |
| Antigravity por assinatura | Identidade e inventário que uma integração oficial e segura realmente permita obter. | A chave e o catálogo Gemini não autenticam o aplicativo Antigravity. Sem superfície validada de leitura, registrar a limitação. [Antigravity SDK](https://www.antigravity.google/docs/sdk/overview/). |
| OpenAI API | Catálogo do endpoint autorizado e metadata básica; enriquecer capacidades apenas com fontes específicas por modelo/API. | `GET /v1/models` não fornece uma matriz completa de reasoning, ferramentas, contexto e velocidade. [List models](https://developers.openai.com/api/reference/resources/models/methods/list). |
| Anthropic API | Catálogo paginado, contexto/lifecycle e campos de thinking, effort e ferramentas de servidor quando retornados pelo schema daquela versão. | Cada tipo de thinking e ferramenta possui seu próprio suporte. Não inferir todas as ferramentas a partir de um booleano genérico. [Models](https://platform.claude.com/docs/en/api/http/models). |
| Gemini Developer API | `models.list`, limites de entrada/saída, `supportedGenerationMethods` e `thinking`. | `thinking` não define todos os níveis/budgets; a lista não confirma grounding ou function calling. Não representa Antigravity. [Models](https://ai.google.dev/api/models). |
| Azure / Microsoft Foundry | Catálogo do recurso em `/openai/v1/models`; configuração do deployment; lista ARM de deployments somente com identidade e autorização de gerenciamento próprias. | Chave do recurso e permissão ARM são distintos. Deployment manual permanece declaração do usuário; deployment provisionado não confirma toda inferência. [Models v1](https://learn.microsoft.com/en-us/rest/api/microsoft-foundry/azureopenai/models), [Deployments — List](https://learn.microsoft.com/en-us/rest/api/microsoftfoundry/accountmanagement/deployments/list?view=rest-microsoftfoundry-accountmanagement-2025-06-01). |
| Amazon Bedrock | Catálogo regional de foundation models e inference profiles; modalidade de autenticação; modalidades e streaming reportados. Uma futura leitura limitada de disponibilidade pode separar autorização, entitlement, região e acordo. | API key Bedrock e identidade/profile AWS seguem contratos próprios. Não aceitar acordos, ampliar região nem invocar um modelo para testar acesso. [API keys](https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys-reference.html), [Model summary](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_FoundationModelSummary.html), [Availability](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_GetFoundationModelAvailability.html). |
| Vertex / Google Cloud | Contrato do backend Standard/Express, identidade, projeto/location e recursos de gerenciamento quando autorizados. | Ações do Model Garden não são ferramentas executáveis do modelo. ADC/SSO e esse adapter não são considerados entregues pela existência de um campo no wizard. [PublisherModel](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/rest/v1beta1/publishers.models). |
| API compatível / custom | Endpoint e protocolo explicitamente configurados; catálogo somente quando o backend o suporta. | Compatibilidade de formato e nome do modelo não confirmam reasoning, web, ferramentas ou contexto. Preservar ID manual como não verificado. [Pesquisa de adapters](../research/api-provider-discovery-2026-10-10.md). |

Uso observado, limites de assinatura, saldo, gasto e créditos continuam verificações independentes. Ausência de uma superfície administrativa não é falha da conexão de inferência. Não calcular percentuais sem denominador e janela correspondentes.

## Codex: Ultra, velocidade e delegação

A inspeção começou pelo [harness do repositório](../../tools/runtime-harness/src/codex-session.mjs) e pelos schemas gerados da versão instalada **0.162.0-alpha.2**, antes de consultar a documentação web. Foram lidos `ModelListResponse`, `ConfigReadResponse`, `ExperimentalFeatureListParams/Response`, `ThreadStartParams` e `TurnStartParams`. Esta pesquisa não iniciou app-server, login, thread ou turno.

O schema de `model/list` separa `supportedReasoningEfforts`, `defaultReasoningEffort`, `serviceTiers`, `defaultServiceTier` e `multiAgentVersion`. `serviceTiers` contém objetos `{id, name, description}`; o identificador define o tier, enquanto o nome serve à apresentação. `multiAgentVersion` pode ser `disabled`, `v1`, `v2` ou ausente/null. Os esforços são strings retornadas pelo runtime, sem enum fixo no Orchestrix. Defaults do schema e campos ausentes não se tornam observações.

A documentação atual descreve **Ultra como um modo que utiliza subagentes** e permite delegação proativa para trabalho divisível. Isso é relevante para diagnosticar o comportamento de um cliente, mas não justifica concluir que qualquer campo `reasoningEffort = ultra` confirma delegação efetiva em qualquer versão, conta ou backend. Preservar esforço, suporte nativo, configuração e política separadamente. [Models](https://learn.chatgpt.com/docs/models?surface=app), [Subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents).

Fast e Ultrafast são **service tiers**, com disponibilidade e consumo próprios; Ultrafast não é o esforço Ultra. O comando `/fast` depende do tier anunciado no catálogo. Mostrar somente IDs/tipos reportados ou um contrato documental versionado; não deduzir rapidez pelo nome do modelo. Uma preferência solicitada também não comprova qual tier foi aplicado a uma execução. [Speed](https://learn.chatgpt.com/docs/agent-configuration/speed), [Developer commands](https://learn.chatgpt.com/docs/developer-commands).

O registro de features e a configuração resolvida podem complementar o catálogo. `experimentalFeature/list` fornece `name`, `enabled`, `defaultEnabled` e `stage`, com paginação; pode receber um `threadId` já existente para o alcance de sua configuração. Não criar um thread para completar o cadastro. A configuração atual documenta `agents.enabled`; o adapter precisa entender a versão antes de traduzir nomes ou assumir defaults. Uma feature habilitada continua sendo evidência de configuração, sem provar que um subagente foi executado. [App-server](https://learn.chatgpt.com/docs/app-server), [Subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents).

**Decisão Orchestrix:** quando o Orchestrix coordena workers, a delegação nativa aninhada permanece desativada por padrão na política desses workers. O relatório pode mostrar que o runtime externo a suporta ou a tem habilitada, sem alterar sua configuração pessoal. A política declarada neste incremento ainda não comprova sua aplicação a um processo real. OX-007/OX-008 devem aplicar e verificar a restrição por tentativa no adapter, respeitando o mecanismo oficial e bloqueando uma execução incompatível quando a restrição for obrigatória. Não depender apenas de uma frase no prompt para declarar contenção comprovada.

Uma futura opção de delegação nativa pode ser útil em uma subtarefa limitada. Exigirá escolha de escopo, orçamento de tokens/tempo/concorrência, responsabilidades, cancelamento e evidência de aplicação. Ela não substitui o scheduler do Orchestrix nem autoriza expansão recursiva sem limites.

## Permissões do worker e ferramentas de nuvem

No Codex, sandbox e aprovação são controles diferentes. `approval_policy = never` impede prompts de aprovação, mas não amplia sozinho o sandbox. A combinação documentada para Full access é `sandbox_mode = danger-full-access` com `approval_policy = never`; o reviewer de aprovação é outro campo. A disponibilidade dos modos também depende do ambiente e das restrições geridas. [Sandbox](https://learn.chatgpt.com/docs/sandboxing), [Permissions](https://learn.chatgpt.com/docs/permission-modes).

O diagnóstico lê somente uma whitelist sanitizada dos campos pertinentes; não devolve configuração completa, diretórios pessoais, instruções, tokens ou endpoints secretos. Nunca troca sandbox, reviewer, writable roots ou rede para validar um recurso. Em APIs de modelo, essas permissões nativas do aplicativo são **não aplicáveis**. A eventual ferramenta de arquivos/Git/shell do Orchestrix depende de executor, autorização e isolamento próprios. Pesquisa web ou code execution hospedados pelo provedor não concedem acesso ao computador do usuário.

## Contrato implementado e limites deste incremento

O módulo puro [connection-diagnostics.js](../../prototypes/desktop/connection-diagnostics.js) expõe `plan`, `buildReport`, `isCurrent` e `summarize`. O relatório `schemaVersion: 1` conserva `scope`, `scopeKey`, revisão, `runId`, fase, datas, checks, features, modelos, política e resumo. Os IDs das verificações são estáveis; labels pertencem à camada de idioma. O módulo não faz chamadas de rede: consome o resultado do catálogo e observações vinculadas à identidade. A bridge continua responsável pelas leituras API reais permitidas.

Cada evidência inclui origem, alcance e data. O diagnóstico aceita metadata somente no escopo válido; a seam de runtime exige identidade vinculada e revisão correspondente. Resumos de suporte por modelo não transformam uma capacidade do provider em fato de todos os itens. Uma lista parcial permanece parcial. Cadastro manual, fixture de teste e texto de documentação não viram autenticação ou execução observadas.

A política distingue delegação nativa recomendada/desativada de configuração do runtime observada, com aplicação ao runtime ainda não demonstrada. Multi-agent do Orchestrix continua planejado nos milestones vigentes. A integração real de configuração, autorização e telemetria de runtimes precisa de adapters aprovados; o relatório não preenche essas lacunas. Nenhuma credencial de produção foi utilizada para validar provedores nesta pesquisa. Resultados dos testes e da revisão visual são registrados pelo [status do desenvolvimento](../DEVELOPMENT_STATUS.md), sem conclusão antecipada do piloto.

O relatório e o registro de conexões deste estudo são mantidos em memória na sessão atual; não há histórico durável de diagnósticos nem restauração de credenciais após reload. Telemetria fornecida por uma observação vinculada pode exibir percentuais finitos entre 0–100, saldo não negativo, moeda e data válidos. Esses campos permanecem desconhecidos quando não informados; o host atual consulta catálogos, sem implementar uma fonte administrativa de quota/crédito. Persistência e adapters de telemetria seguem o plano de produção.

## Validação ativa posterior

Este incremento atende à tela e à lógica de diagnósticos por leitura. Probes de inferência, reasoning, tier, ferramenta ou delegação são uma evolução dentro dos tickets existentes. Antes de executá-los, o contrato deverá definir modelo/endpoint, parâmetros, limite de tokens, duração, concorrência, eventual custo e autorização de efeitos. Ferramentas locais precisarão de diretório e permissões pertinentes; testes de escrita usarão recursos controlados, com recuperação definida.

O cadastro e **Recheck** não disparam esses probes. Uma ação ativa futura deve explicar o que verifica e seus limites; uma resposta trivial não certifica arquitetura, segurança ou qualidade geral do modelo. Uma observação de execução vale para a combinação testada, preservando requested/effective. Subscription Only continua impedindo execução API e fallback cobrado, inclusive quando uma API cadastrada tem créditos ou free tier.

## Critérios de aceite e continuidade

- Cadastro inicia a rodada adequada à modalidade, com progresso e verificações específicas; recheck conserva a identidade e invalida resultados atrasados.
- Suporte, disponibilidade, configuração, política e resultado da verificação permanecem distintos. Recurso não suportado, dado ausente, falta de autenticação e leitura falha têm apresentações diferentes.
- Catálogos e capabilities mantêm proveniência e cobertura; nenhum nome de modelo libera web, ferramentas, reasoning, velocidade ou acesso completo.
- Diagnóstico é somente leitura, cancela com limites e não expõe segredos ou muda configuração pessoal. Nenhum prompt ou ferramenta potencialmente cobrado é executado automaticamente.
- Native multi-agent do runtime e coordenação Orchestrix aparecem separados. A restrição de delegação é política declarada até sua aplicação ser demonstrada pelo adapter.
- Testes utilizam respostas controladas e cobrem erros, desconhecido, não aplicável, parcialidade, isolamento de revisão, cancelamento e recheck. Validação live futura registra escopo e autorização próprios.

O trabalho complementa OX-001/OX-002/OX-003, prepara aplicação real em OX-007/OX-008 e apresentação persistente em OX-012/OX-016. Não cria um milestone paralelo, não conclui M0 e não encerra o piloto humano D1. Referências: [plano](../DEVELOPMENT_PLAN.md), [backlog](../DEVELOPMENT_BACKLOG.md), [pesquisa oficial de provedores](../research/api-provider-discovery-2026-10-10.md), [ADR de modelo/reasoning](../adr/0002-model-and-reasoning-policy.md).
