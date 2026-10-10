# APIs de modelos: descoberta, autenticação e capacidades

Pesquisa consultada e revalidada em **10 de outubro de 2026**, em documentação oficial e no código/schema local do runtime Codex. Foram examinados os contratos de OpenAI, Anthropic, Gemini Developer API, Google Cloud/Vertex AI, Microsoft Foundry/Azure OpenAI, Amazon Bedrock e um exemplo de API compatível com OpenAI. Nesta pesquisa não houve conexão a conta, leitura de segredo, autenticação ou inferência: do executável Codex foi consultado apenas `--version`, além dos schemas já gerados. As decisões propostas estão no [contrato de conexão](../design/api-connection-contract.md); este levantamento não comprova adapters implementados nem altera os milestones e gates existentes.

## Resultado para o produto

O primeiro passo deve perguntar **Subscription ou API**, antes de escolher o provedor. Conta de assinatura/runtime, credencial de API, modelo e perfil de agente são entidades diferentes. O caminho de assinatura segue a [pesquisa de onboarding](subscription-account-ux.md) e as capacidades efetivamente expostas pelo runtime. A Gemini API é uma conexão de API Google; não é uma sessão do Antigravity e não demonstra que uma assinatura desse produto autoriza chamadas à API.

**Decisão Orchestrix:** registrar modalidade de autorização e origem de cobrança explicitamente. Não deduzi-las pelo nome do modelo, domínio ou campo `api_key`. Uma opção “Google” sem distinguir Gemini Developer API e Google Cloud ocultaria requisitos importantes. APIs compatíveis também precisam identificar backend, protocolo e endpoint reais.

O catálogo ajuda a descobrir identificadores; seu conteúdo varia por provedor. **Não existe uma resposta universal de `list models` que comprove ferramentas, thinking, pesquisa web, quota e autorização de execução.** Algumas APIs já fornecem parte dessas informações, que deve ser aproveitada com sua origem. Campos ausentes permanecem desconhecidos, e acesso ao catálogo não equivale a uma inferência bem-sucedida.

## OpenAI API

A autenticação de aplicação aceita credenciais bearer; organização/projeto podem ser especificados em situações descritas pelo provedor. Segredos não devem ser expostos no código do cliente. **Consequência:** conexão de API com referência segura à credencial e escopo de organização/projeto quando necessário, independente do caminho de uso de assinatura. [API Overview](https://developers.openai.com/api/reference/overview).

`GET /v1/models` informa modelos disponíveis e metadata básica, como identificador e proprietário. Esse contrato não oferece uma matriz completa de ferramentas, limites e reasoning. **Consequência:** enriquecer o catálogo com documentação oficial por modelo/API e observações do adapter; não inventar capacidades a partir do ID. [List models](https://developers.openai.com/api/reference/resources/models/methods/list).

Níveis e valores padrão de `reasoning.effort` dependem do modelo. A superfície Responses e Chat Completions também pode alterar o suporte a ferramentas. **Consequência:** validar a combinação modelo + superfície + parâmetros, guardando intenção e configuração efetiva. [Reasoning models](https://developers.openai.com/api/docs/guides/reasoning).

Pesquisa web é uma ferramenta com contrato próprio: o caminho de web search de Responses difere dos modelos de busca especializados de Chat Completions. **Consequência:** `function calling` não deve implicar `web search`; o adapter declara a ferramenta concreta e suas restrições. [Web search](https://developers.openai.com/api/docs/guides/tools-web-search).

## Anthropic API

O catálogo `GET /v1/models` usa a autenticação e a versão da API próprias da Anthropic. A referência atual inclui limites `max_input_tokens`/`max_tokens`, lifecycle e `capabilities`, com detalhes de thinking, effort e ferramentas de servidor, inclusive web search. Alguns campos podem ser `null`. **Consequência:** consumir esses campos quando retornados; respeitar paginação e valores ausentes. Exemplos da referência não são resultados observados para uma conta do Orchestrix. [List Models](https://platform.claude.com/docs/en/api/models/list).

A documentação distingue capacidades do modelo e ferramentas de servidor; a plataforma de hospedagem pode disponibilizar recursos diferentes. **Consequência:** não copiar a ficha da Anthropic direta para a mesma família de modelo hospedada em Azure, Google Cloud ou Bedrock. O catálogo válido pertence à conexão efetiva. [Models overview](https://platform.claude.com/docs/en/models/overview).

## Gemini Developer API / Google AI

A Gemini Developer API utiliza uma chave própria; cada chave está associada a um projeto Google Cloud. O provedor recomenda manter segredos fora do cliente e do versionamento. **Consequência:** apresentar “Gemini API” como opção específica, com referência à credencial; o projeto associado não deve ser confundido com login em um runtime de desenvolvimento. [Using Gemini API keys](https://ai.google.dev/gemini-api/docs/api-key).

`models.list`/`models.get`, em `generativelanguage.googleapis.com`, expõem métodos de geração, limites de entrada/saída, `thinking` e informações de sampling. Por exemplo, a ausência de `topK` pode significar que esse parâmetro não é permitido. **Consequência:** filtrar modelos adequados à operação e aplicar apenas os parâmetros suportados. O booleano `thinking` não estabelece sozinho todos os níveis/budgets ou combinações com ferramentas. [Models API](https://ai.google.dev/api/models).

Grounding com Google Search exige uma ferramenta específica e suporte do modelo. Há regras próprias de cobrança; ativar function calling não ativa automaticamente uma busca. **Consequência:** expor web grounding separadamente, preservar as citações retornadas e respeitar a autorização para API/ferramentas. [Grounding with Google Search](https://ai.google.dev/gemini-api/docs/google-search/).

## Google Cloud / Vertex AI

As URLs de documentação de Vertex AI consultadas redirecionaram para **Gemini Enterprise Agent Platform** nesta data. O contrato continua identificando o backend Google Cloud concreto, sem transformar a nova nomenclatura em um runtime Antigravity.

A documentação oferece ADC/identidade e chave de API, recomendando ADC; o caminho completo requer projeto e location. **Consequência:** campos essenciais condicionais ao modo de autenticação e ao endpoint, incluindo projeto/localidade no modo Standard. [Get started](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/start), [Google Cloud API key](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/start/api-keys).

Express Mode utiliza chave e um endpoint global sem projeto/location na URL, com um subconjunto das funcionalidades. **Consequência:** separar Standard e Express no adapter; não exigir campos artificiais no Express ou prometer todas as funções do backend completo. [Express Mode overview](https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/start/express-mode/overview).

Model Garden oferece `publishers.models.list`, com publisher, paginação, endpoint e versão próprios. **Inferência de produto:** essa lista de catálogo não é prova de que um modelo esteja habilitado para inferência em determinado projeto/região, nem de que a credencial Express autorize a mesma operação de controle. O adapter precisa distinguir catálogo, deployments/endpoints próprios e disponibilidade observada. [Publisher models — list](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/rest/v1beta1/publishers.models/list).

## Microsoft Foundry / Azure OpenAI

O acesso utiliza o **nome do deployment**, que associa modelo, versão e configurações. A documentação aceita chave do recurso ou Microsoft Entra ID e recomenda identidade sem chave em produção. A rota OpenAI v1 recebe o deployment no campo `model`; nem todo deployment suporta Responses. **Consequência:** perguntar endpoint, modalidade de autenticação e deployment, preservando a superfície/API efetiva. Não tratar o nome do deployment como nome universal do modelo. [Foundry endpoints](https://learn.microsoft.com/en-us/azure/foundry/foundry-models/concepts/endpoints).

A referência atual também oferece **`GET {endpoint}/openai/v1/models`**, aceitando `api-key` ou identidade com escopo de Cognitive Services. Esse catálogo básico não exige autenticação ARM e não substitui a lista de deployments. **Consequência:** iniciar a descoberta do catálogo com os dados essenciais do recurso, sem exigir subscription/resource group para uma conexão apenas com chave. A configuração de inferência continua distinguindo ID do catálogo e nome do deployment. [Azure OpenAI — Models v1](https://learn.microsoft.com/en-us/rest/api/microsoft-foundry/azureopenai/models).

A listagem documentada de deployments usa a API de gerenciamento ARM com subscription, resource group, account e versão. **Inferência de produto:** permissão de inferência não comprova acesso a esse plano de gerenciamento. **Consequência:** descoberta via identidade de gerenciamento quando autorizada; oferecer deployment informado pelo usuário quando a listagem não estiver disponível, sem declará-lo verificado. Os campos ARM são necessários para essa descoberta, não para toda requisição de inferência. [Deployments — List](https://learn.microsoft.com/en-us/rest/api/microsoftfoundry/accountmanagement/deployments/list?view=rest-microsoftfoundry-accountmanagement-2025-06-01).

## Amazon Bedrock

Bedrock oferece chaves próprias além da autenticação AWS. Chaves de curto prazo herdam permissões do principal IAM, têm duração limitada e são regionais; certas operações não aceitam essas chaves. A AWS recomenda credenciais de curto prazo para requisitos maiores de segurança. **Consequência:** modo API key ou credenciais AWS/profile/SSO, região e família de endpoint explícitos; não solicitar um segredo IAM como único caminho. [API keys reference](https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys-reference.html).

Para novas aplicações a documentação recomenda `bedrock-runtime`; a descoberta usa `ListFoundationModels` e `ListInferenceProfiles`. O catálogo OpenAI-compatible pertence ao caminho `bedrock-mantle`, e seus objetos podem não conter capacidades Anthropic completas. **Consequência:** selecionar a descoberta do backend, sem tentar `/models` universalmente. Identificar modelo, profile de inferência e região sem intercambiá-los. [Get list of models](https://docs.aws.amazon.com/bedrock/latest/userguide/models-get-info.html).

`ListFoundationModels` retorna modalidades, suporte a streaming, tipos de inferência e lifecycle. O adapter ainda precisa das restrições de ferramentas/parâmetros para a API efetiva. **Consequência:** usar os campos presentes e manter outros desconhecidos; rejeição de acesso não é prova de ausência do modelo. [ListFoundationModels](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_ListFoundationModels.html).

## APIs compatíveis com OpenAI e outros backends

“OpenAI-compatible” descreve uma superfície de transporte; não garante igualdade de parâmetros, modelos ou ferramentas. A Groq documenta uma base URL própria e campos OpenAI que não suporta. **Consequência:** um conector genérico solicita base URL, autenticação e protocolo; utiliza a listagem apenas se documentada pelo backend e não herda a matriz OpenAI. [Groq OpenAI compatibility](https://console.groq.com/docs/openai).

**Decisão Orchestrix:** backend sem adapter especializado pode permitir ID de modelo manual e capabilities desconhecidas. Não habilitar Responses, reasoning, web search, MCP ou tools por suposição. A entrada manual configura um candidato; não comprova compatibilidade ou acesso.

## Uso, quota, créditos e custo

Telemetria de uma resposta e faturamento são fontes distintas. O Converse do Bedrock inclui `usage`, mas isso não fornece saldo da conta. **Decisão Orchestrix:** separar tokens observados por tentativa, rate limits, quota de assinatura, custo estimado, gasto reportado e saldo/crédito; nunca calcular “percentual restante” sem numerador, denominador e janela confiáveis. [Converse](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_runtime_Converse.html).

OpenAI oferece Costs na superfície de organização/Admin, com credenciais administrativas específicas. Anthropic também documenta uma Usage and Cost Admin API com regras próprias de autenticação e disponibilidade. **Consequência:** uma chave de inferência não garante leitura de gasto e menos ainda de crédito. Telemetria financeira deve ser opcional e solicitar sua autorização separadamente; indisponibilidade aparece como **Not reported**, sem impedir o cadastro. [OpenAI Costs](https://developers.openai.com/api/reference/resources/admin/subresources/organization/subresources/usage/methods/costs), [OpenAI Admin APIs](https://developers.openai.com/api/docs/guides/admin-apis), [Anthropic Usage and Cost API](https://platform.claude.com/docs/en/manage-claude/usage-cost-api).

## Decisões registradas e continuidade

| Decisão | Implicação |
| --- | --- |
| API-01 — Modalidade antes do provedor | Subscription e API seguem autorização/cobrança próprias; Gemini API não é Antigravity. |
| API-02 — Descoberta específica | Cada adapter informa campos essenciais, plano de controle, paginação e limitações. |
| API-03 — Capabilities com evidência | Metadata retornada, documentação versionada e observação são fontes distintas; ausência não vira suporte. |
| API-04 — Configuração antes de verificação | Salvar formulário não autentica; catálogo não comprova inferência, quota ou saldo. |
| API-05 — Segredo no host seguro | Renderer/HTML/localStorage não armazenam credenciais reais. |
| API-06 — Consentimento de API | Subscription Only bloqueia qualquer fallback para API, mesmo já cadastrada; permitir API exige escolha explícita. |
| API-07 — Política determinística primeiro | Efficiency, Balanced, Performance e Custom usam capacidades verificadas e regras explicáveis; qualidade não é inferida apenas por preço ou nome. |

Os provedores acima constituem o alcance de pesquisa e do contrato, **não um compromisso de implementar todos antes do primeiro gate**. D1 valida compreensão e layout; M0 comprova os adapters escolhidos e seus limites; persistência, execução e expansão seguem os milestones existentes. Nenhuma capacidade de API real passa a estar implementada por constar no wizard. Revalidar fontes e metadata antes de implementar cada adapter, porque modelos, autenticação e disponibilidade mudam.

Referências internas: [contrato de API](../design/api-connection-contract.md), [decisões de integração](integration-decisions.md), [ADR-0002](../adr/0002-model-and-reasoning-policy.md), [política de modelo e reasoning](../REASONING_AND_MODEL_POLICY.md) e [plano](../DEVELOPMENT_PLAN.md).

## Continuação: descoberta automática no cadastro

O pedido posterior do usuário estabelece o gatilho: **ao registrar uma API ou uma conta/runtime, começar imediatamente a descoberta dos recursos compatíveis que essa conexão expõe oficialmente**. A intenção é evitar um cadastro seguido de configuração manual de cada modelo. Esta seção especifica uma descoberta somente de leitura; não altera os gates de execução real nem autoriza inferência de teste, criação de deployment, adesão a acordo de marketplace ou instalação de runtime.

**Decisão Orchestrix:** descobrir todas as páginas dentro de limites operacionais explícitos. Uma resposta interrompida por timeout, paginação inconsistente, limite de itens ou permissão insuficiente é **parcial**, nunca “todos os recursos encontrados”. Guardar motivo e cursor seguro de continuidade; preservar os itens válidos já recebidos. Cadastro e descoberta têm estados separados: cadastro aceito, descoberta em andamento, completa, parcial ou indisponível. Uma lista vazia completa difere de um erro de acesso.

O alcance pertence à conexão: conta/runtime, backend, endpoint, versão, projeto/workspace e região quando aplicáveis. Duas contas com o mesmo modelo conservam registros e disponibilidade próprios. Uma resposta atrasada de uma conexão removida ou editada não deve preencher outra. Não importar automaticamente outras contas, trocar endpoint ou expandir a região para “achar mais modelos”. A descoberta com chave da modalidade API resulta da escolha explícita desse cadastro; não habilita execução paga nem fallback no **Subscription Only Mode**.

### Operações e paginação para os adapters de API

| Backend | Operação de leitura e autenticação | Continuidade e alcance |
| --- | --- | --- |
| OpenAI | `GET https://api.openai.com/v1/models`, bearer; headers de projeto/organização quando configurados. | `data[]`; a referência não documenta cursor nesta operação. Não inventar paginação ou sondar endpoints de inferência. [List models](https://developers.openai.com/api/reference/resources/models/methods/list), [autenticação](https://developers.openai.com/api/reference/overview). |
| Anthropic direta | `GET https://api.anthropic.com/v1/models`, `x-api-key`, `anthropic-version: 2023-06-01`; workspace quando a credencial exigir. | `limit` de 1 a 1000, default 20; enquanto `has_more`, usar `after_id=last_id`. `data[]`, `first_id`, `last_id`. A lista padrão cobre ativos/deprecated; retired é histórico, não candidato executável. [List Models](https://platform.claude.com/docs/en/api/models/list). |
| Gemini Developer API | `GET https://generativelanguage.googleapis.com/v1beta/models`, `x-goog-api-key`. | `models[]`, `nextPageToken`; `pageSize` default 50, máximo 1000, `pageToken` e demais parâmetros estáveis entre páginas. [Models](https://ai.google.dev/api/models), [chaves](https://ai.google.dev/gemini-api/docs/api-key). |
| Google Cloud Model Garden | `GET https://{service-endpoint}/v1beta1/publishers/{publisher}/models`, identidade do backend Standard. | `publisherModels[]`, `nextPageToken`, `pageSize`/`pageToken`; `view` e `listAllVersions` conforme contrato. Publisher/versão não demonstram endpoint de inferência disponível. Express precisa de contrato próprio; não presumir a mesma listagem. [Publisher list](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/rest/v1beta1/publishers.models/list). |
| Azure OpenAI v1 | `GET {endpoint}/openai/v1/models`, `api-key` ou bearer com escopo `https://cognitiveservices.azure.com/.default`. | Resposta básica `OpenAI.ListModelsResponse`; não há cursor listado. Montar a URL sem duplicar `/openai/v1`. Este é o catálogo do recurso, não a lista ARM de deployments. [Models v1](https://learn.microsoft.com/en-us/rest/api/microsoft-foundry/azureopenai/models). |
| Azure deployments | `GET https://management.azure.com/subscriptions/{subscriptionId}/resourceGroups/{resourceGroupName}/providers/Microsoft.CognitiveServices/accounts/{accountName}/deployments?api-version=2025-06-01`. Identidade/permissão de gerenciamento distinta da chave do recurso. | `value[]`, `nextLink`; seguir somente links validados do gerenciamento no mesmo alcance. Nome do deployment, modelo e versão são campos distintos. [Deployments — List](https://learn.microsoft.com/en-us/rest/api/microsoftfoundry/accountmanagement/deployments/list?view=rest-microsoftfoundry-accountmanagement-2025-06-01). |
| Bedrock, modelos | `GET https://bedrock.{region}.amazonaws.com/foundation-models`; AWS SigV4 ou Bedrock bearer key com permissão da operação. | `modelSummaries[]`, sem paginação nesta operação. O endpoint é de controle, separado de `bedrock-runtime`. [ListFoundationModels](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_ListFoundationModels.html). |
| Bedrock, profiles | Mesmo host de controle, `GET /inference-profiles?maxResults=…&nextToken=…`. | `inferenceProfileSummaries[]`, `nextToken`; `maxResults` de 1 a 1000. Manter profile, modelos associados e regiões como recursos relacionados. [ListInferenceProfiles](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_ListInferenceProfiles.html). |
| Compatível com OpenAI | `GET {baseURL}/models` somente quando previsto pelo backend/adapter configurado; autenticação declarada pela conexão. | Não aplicar cabeçalhos ou capacidades OpenAI universalmente. Uma operação não suportada torna essa descoberta indisponível; não dispara prompts para verificar o ID manual. [Exemplo Groq](https://console.groq.com/docs/api-reference). |

As chaves Bedrock são aceitas para ações **Bedrock e Bedrock Runtime**, com exceções documentadas; `ListFoundationModels` não consta dessas exclusões. Portanto o adapter pode tentar a leitura com `Authorization: Bearer …`, condicionada às permissões do principal e à região da chave. Não é necessário pedir credenciais IAM adicionais só por ser uma operação de controle. Isso é uma conclusão do contrato documentado, **não uma autenticação validada nesta pesquisa**. [Escopo das chaves](https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys-reference.html), [header bearer](https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys-use.html).

Bedrock também oferece leitura de disponibilidade sem executar prompts: `GET /foundation-model-availability/{modelId}` retorna `authorizationStatus`, `entitlementAvailability`, `regionAvailability` e `agreementAvailability`. O adapter pode enriquecê-la em chamadas limitadas, se autorizado. Não aceitar acordos nem habilitar acesso durante a descoberta; falta de permissão para consultar esse status permanece desconhecida. [GetFoundationModelAvailability](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_GetFoundationModelAvailability.html).

### Metadata utilizável e limites das conclusões

| Fonte | Campos concretos aproveitáveis | Informação que continua dependendo de outra evidência |
| --- | --- | --- |
| Anthropic | `max_input_tokens`, `max_tokens`, `lifecycle`, `line`; `capabilities.thinking.supported`/`types`, `effort.supported`/níveis, `image_input`, `pdf_input`, `server_tools.web_search`, `server_tools.code_execution`, `structured_outputs`, `citations`. Cada suporte usa `{supported: boolean}`. | Não há nesse objeto confirmação genérica de function calling, modalidades de saída ou streaming. `server_tools` não deve preencher todas as ferramentas. `capabilities: null` é desconhecido. [Schema Models](https://platform.claude.com/docs/en/api/models/list). |
| Gemini | `inputTokenLimit`, `outputTokenLimit`, `supportedGenerationMethods`, `thinking`, `temperature`, `maxTemperature`, `topP`, `topK`; identificar por `name`, preservando versão/base ID. | `thinking` não define budgets/níveis. A lista não contém uma matriz de modalidades, function calling ou Google Search. Ausência de `topK` tem a semântica específica documentada de parâmetro não permitido. [Model](https://ai.google.dev/api/models). |
| Bedrock | `inputModalities`, `outputModalities`, `responseStreamingSupported`, `inferenceTypesSupported`, `customizationsSupported`, `modelLifecycle`, IDs/ARN/provedor. | Limites de tokens, reasoning, ferramentas e web não constam desse resumo. Streaming não demonstra streaming bidirecional, e catálogo não demonstra permissão de invocação. [FoundationModelSummary](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_FoundationModelSummary.html). |
| Azure ARM | `name`, `properties.model.{format,name,version}`, `properties.provisioningState`, SKU/capacidade quando presentes. | Deployment provisionado não confirma todas as APIs/ferramentas nem saldo. Não reutilizar `properties.capabilities` ou outros campos sem contrato versionado. [Deployment](https://learn.microsoft.com/en-us/rest/api/microsoftfoundry/accountmanagement/deployments/list?view=rest-microsoftfoundry-accountmanagement-2025-06-01). |
| Vertex PublisherModel | `name`, `versionId`, `launchStage`, `versionState`, `predictSchemata`, `supportedActions`. | `supportedActions` é **CallToAction** de Model Garden, como abrir documentação/notebook/deploy; não é `supportedGenerationMethods` da Gemini nem suporte a function calling. [PublisherModel](https://docs.cloud.google.com/gemini-enterprise-agent-platform/reference/rest/v1beta1/publishers.models). |
| Groq especializado | O catálogo documenta `active`/`context_window`; `GET /models/{id}` acrescenta metadata como `max_completion_tokens`. | Esses campos pertencem ao adapter Groq, não ao protocolo genérico. Parametrização de reasoning continua por modelo/superfície. [API Reference](https://console.groq.com/docs/api-reference). |

**Regra de normalização Orchestrix:** booleano explícito `true` vira suporte reportado, `false` vira não suportado; campo ausente ou `null` vira desconhecido, salvo uma semântica oficial específica e versionada, como `topK`. Limites numéricos exigem valor finito positivo; exemplos com zero não estabelecem limite utilizável. Guardar campo de origem, contrato/API, data, endpoint/região e escopo da afirmação. Documentação estática complementa metadata; não transforma um exemplo em observação de conta.

O catálogo inclui modelos para tarefas diferentes. Embedding, áudio, imagem ou um modelo sem função conversacional não devem ser classificados como agente de chat pela aparência do ID. Compatibilidade usa métodos/modalidades reportados e a matriz versionada do adapter. Um recurso desconhecido pode continuar catalogado com motivo de elegibilidade pendente; não ativar automaticamente tool use, grounding, reasoning ou saída estruturada. Os perfis Efficiency/Balanced/Performance/Custom selecionam entre candidatos aptos segundo essas evidências; descoberta não mede inteligência, preço, latência ou qualidade.

### Runtime Codex: contrato local antes da documentação web

A inspeção local confirmou **`codex-cli 0.162.0-alpha.2`**, mesma versão do [spike](codex-protocol-spike.md). Foram lidos os schemas gerados dessa versão e [CodexSession](../../tools/runtime-harness/src/codex-session.mjs). Não se iniciou app-server, thread, login ou turno nesta pesquisa.

O fluxo de metadata começa com `initialize`/`initialized` e `account/read` sem refresh de token. **`model/list`** aceita `cursor`, `limit` e `includeHidden`; devolve `data` e `nextCursor`. Para catálogo completo, listar todas as páginas com `includeHidden: true`, preservando o campo hidden para filtro da interface. O identificador de execução é **`model`**, conservando `id` e `displayName` separadamente. [Referência app-server](https://learn.chatgpt.com/docs/app-server).

O schema local exige `supportedReasoningEfforts[]`/`defaultReasoningEffort` e aceita metadata de `inputModalities`, `serviceTiers`/`defaultServiceTier`, `modelSpecialty`, `multiAgentVersion` e upgrade. Preservar esforços como strings retornadas, sem limitar a um enum antigo. Campos opcionais ausentes não se tornam observados por terem defaults no schema. A documentação oficial prevê text/image como compatibilidade para catálogos antigos sem `inputModalities`; caso o adapter adote esse fallback, marcar sua origem documental/versionada.

**`modelProvider/capabilities/read {}`** existe no schema instalado e retorna `webSearch`, `imageGeneration`, `namespaceTools`. Como o request não seleciona um modelo, conservar esses dados no escopo do provider/configuração; um `true` não confirma suporte em cada item do catálogo. A chamada não foi executada nesta pesquisa. `account/rateLimits/read` é uma leitura distinta, sem implicar quota por modelo ou crédito de API. O estado de autenticação deve ser saneado antes de chegar ao renderer, sem armazenar email/tokens brutos.

O método atual `CodexSession.discover()` do harness lê apenas uma página de 100 itens e reduz os campos para o ensaio de assinatura. **Não representa descoberta completa**. A bridge deve reutilizar a disciplina de isolamento/encerramento do harness com um contrato próprio de leitura, sem chamar `thread/start` ou `turn/start`. Catálogo obtido não ativa a execução produtiva nem substitui a validação M0.

### Outros runtimes e recursos do agente

O Claude Agent SDK documenta leituras `supportedModels()`, `initializationResult()`, `supportedAgents()`, `supportedCommands()`, `mcpServerStatus()` e `accountInfo()`. `ModelInfo` inclui identificação/alias e suporte opcional a effort, adaptive thinking, fast/auto mode; esses campos têm requisitos de versão. Metadata do SDK não equivale ao catálogo de API direta. **Não usar `getContextUsage()` para este cadastro**, pois sua documentação informa requisições de token counting. [TypeScript SDK](https://code.claude.com/docs/en/agent-sdk/typescript).

Claude/Antigravity não estão disponíveis no PATH registrado pelo [harness](runtime-harness-results.md); não se instalaram ou iniciaram nesta rodada. Para Claude, manter as decisões vigentes de modalidade de autenticação e integração, revalidando o contrato oficial antes de implantar esse adapter. Para Antigravity, a documentação do SDK apresenta Gemini API key/Google Cloud, enquanto a lista pública por plano descreve o produto interativo; nenhuma delas comprova automaticamente um endpoint de catálogo da conta de assinatura. [Antigravity SDK](https://www.antigravity.google/docs/sdk/overview/), [modelos do aplicativo](https://www.antigravity.google/docs/models/).

**Decisão Orchestrix:** modelos, profiles/deployments, comandos, subagentes, skills e ferramentas são tipos de recurso distintos. Inventariar o que a interface oficial do adapter permite listar sem executar; não inicializar MCPs, chamar ferramentas, instalar plugins ou rodar hooks só para enriquecer o cadastro. Recursos de projeto têm outro alcance e podem depender de um diretório escolhido posteriormente. Se o runtime não oferece descoberta oficial segura, informar a limitação e conservar campos desconhecidos; não usar scraping de seletores/login nem atribuir o catálogo Gemini à conta Antigravity.

### Critérios de implementação e revisão da bridge

1. Gatilho automático após cadastro, com cancelamento ao remover/editar a conexão; credencial e resultado associados ao mesmo alcance/revisão. O usuário não precisa lançar uma tarefa para descobrir modelos.
2. Apenas operações allowlisted de metadata. Sem mensagens de “hello”, respostas artificiais, inferência paga, token-counting de contexto, mutações no provider ou expansão automática para outro backend.
3. Paginação concluída ou status parcial explícito, com guarda para cursor repetido, respostas grandes e tempo total. Respostas externas passam por validação de tipos antes da normalização.
4. Segredos usados pelo host/bridge e mantidos fora de renderer, storage, logs e artefatos de teste; redirects/nextLink não recebem credencial sem validar destino e alcance. Endpoint personalizado não pode virar acesso arbitrário ao host. A integração produtiva com vault continua uma entrega própria.
5. Proveniência por capacidade e escopo de provider/modelo/recurso. Não elevar suportes desconhecidos pelo nome nem preencher matriz atual a partir de exemplos da documentação.
6. Autenticação da leitura, catálogo, elegibilidade, quota e execução são estados separados. Falha parcial de enriquecimento não apaga catálogo válido; erro de acesso não equivale a ausência de modelo.
7. Testes com transportes injetados cobrem paginação, metadata incompleta, rejeição de autenticação, regiões/escopos, cancelamento e segredo saneado. Uma resposta de fixture é evidência de contrato/teste, sem selo de verificação real. Validação de contas reais será registrada separadamente quando houver credenciais e adapter autorizado.

Os critérios acima tornam o pedido de descoberta imediata concreto, preservando a estratégia determinística inicial, **Subscription Only**, configuração manual e explicação de escolhas. D1 continua validando experiência; este avanço de metadata não conclui o piloto humano, M0 de execução nem M1 de persistência.
