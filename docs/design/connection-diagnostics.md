# Diagnósticos de conexões e recursos

Contrato de **10 de outubro de 2026** para o diagnóstico automático no cadastro de uma conexão e sua repetição em Settings. Complementa o [contrato de conexão](api-connection-contract.md) e a [descoberta de recursos](resource-discovery-increment.md). A interface deve explicar o resultado de várias verificações, específicas para a modalidade e o provedor, sem exigir que a pessoa conheça o protocolo do runtime.

**Alinhamento de 13/10:** a atualização periódica e orientada a eventos abaixo é um **requisito novo planejado**, além do cadastro e Recheck já previstos. Não está implementada no protótipo ou no harness; não muda o contrato wire 1.1 nem autoriza probes. Esta seção é a especificação central de validade/atualização, consumida pelos tickets existentes.

O incremento usa uma lógica de diagnóstico separada da execução. A bridge de desenvolvimento pode ler catálogos API; o relatório reúne essas respostas, configuração e observações autorizadas de adapters. O cadastro de um runtime sem adapter autenticado produz verificações bloqueadas ou desconhecidas, em vez de simular login. Não há inferência, execução de ferramentas ou alteração de permissões durante esse diagnóstico. Esse registro descreve o estado histórico de 10/10. O caminho Codex gerido pelo runtime e o worker receberam provas posteriores, registradas no status; OAuth/cofre próprios e modalidades adicionais conservam seus gates.

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

## Recorte implementado de cadastro — 17/10/2026

O Desktop usa a extensão `connections-v1` do [daemon](../contracts/local-daemon.md#runtime-managed-connections-extension): cadastro explícito do Codex instalado, diagnóstico após consentimento, Recheck, cancelamento da rodada exata e escopo separado para preparação de tarefa. As janelas compartilham o serviço existente de metadata; não mantêm outro catálogo autoritativo. CAS de cadastro e revisão de vinculação são distintos: renomear não troca a identidade das tentativas; desabilitar/desvincular/reconfirmar retira elegibilidade futura e preserva histórico.

A instalação é candidata até sua verificação; o login continua gerido pelo runtime. O perfil instalado expõe apenas detalhes limitados da conta, sem ID remoto estável de conta/workspace. A interface não importa tokens, não promete contas/cotas independentes e não executa login. Uma rodada agrupa as leituras disponíveis; seu resultado distingue fonte sintética/oficial, idade, completude, falha, cancelamento e fatos desconhecidos. `model/list` não comprova entitlement, e permissões declaradas/configuradas não são observações de um turno executado.

O recorte entrega cadastro/Recheck e revalidação necessária à preparação/admissão, com cache/limites/backoff já existentes. Não implementa outro scheduler nem configura uma cadência periódica completa pelo Control Center. O requisito ampliado abaixo permanece válido; resultados efetivamente executados, inclusive provas nativas/autenticadas, pertencem ao [status](../DEVELOPMENT_STATUS.md).

## Atualização periódica e orientada a eventos — requisito planejado

A política deve definir um intervalo automático configurável e prazo de validade por classe de observação/adapter. Suporte relativamente estável, configuração, identidade e disponibilidade transitória podem exigir validades distintas; um TTL global não certifica todos os campos. Registrar os valores efetivos, unidades, origem e limites, junto dos budgets de duração/páginas/bytes/frequência/concorrência e espera progressiva. Defaults numéricos serão definidos/validados em OX-007/OX-008 conforme as leituras disponíveis, sem assumir a mesma frequência para todos os provedores.

O objetivo é manter o que as interfaces oficiais permitem observar naquele escopo, sem prometer uma matriz perfeita de todas as funções do fornecedor. Recurso não exposto permanece desconhecido, mesmo após uma rodada concluída.

| Gatilho | Comportamento planejado |
| --- | --- |
| Cadastro/Recheck manual | Manter a rodada no escopo/revisão selecionados; Recheck não ignora rate limit, autorização ou budgets. |
| Intervalo periódico | Atualizar observações elegíveis para a conexão habilitada/autorizada, reutilizando dados válidos. Sem credencial disponível, manter bloqueio/ação de login explícita, sem prompt ou login automático inventado. |
| Reconexão | Revalidar identidade/configuração/versão e capacidades necessárias antes da próxima execução; dados válidos só são reutilizáveis se esse escopo continuar confirmado. |
| Mudança de identidade, endpoint/região/deployment, configuração ou versão | Invalidar as observações afetadas; descartar resultados tardios da revisão antiga, sem apagar o histórico. Mudança de conta não retoma uma sessão de outra identidade. |
| Erro indicativo de recurso indisponível | Agendar revalidação direcionada do modelo/recurso/escopo envolvido, com código/fonte e próxima tentativa permitida. Não declarar modelo descontinuado a partir de timeout, catálogo parcial ou rate limit. |
| Nova decisão/admissão de execução | Consultar validade das capacidades **necessárias**. Usar cache válido; revalidar somente dados críticos vencidos/ausentes ou bloqueá-los. Não fazer inventário completo por mensagem. |

Agrupar leituras quando uma resposta satisfizer várias verificações. Usar uma rodada em voo por chave de escopo/revisão, agrupar pedidos simultâneos e limitar concorrência por conexão/provedor e globalmente; requests coalescidos não disparam mais chamadas. Priorizar a revalidação necessária à admissão, dentro dos mesmos limites, sem executar loops de consulta ou retries de inferência. Falhas repetidas aplicam espera progressiva limitada e respeitam `Retry-After` ou equivalente quando exposto. Ausência de rede/autorização gera estado e próxima ação, não tentativas imediatas contínuas. Cancelamento/desconexão encerra rodadas próprias e rejeita respostas antigas.

Cada observação mantém conexão/revisão, modelo e backend/versão aplicáveis, fonte, escopo, instante da última leitura bem-sucedida, validade e completude. Não inventar data para evidência não datada. Renovar a validade exige outra observação bem-sucedida pertinente; uma falha de recheck não altera a data original. O planejamento de validade/eventos deve ser versionado na camada diagnóstica futura de OX-003/OX-005, sem tratar o `observedAt` opcional do wire atual como um TTL já implementado.

| Condição exibida/consumida | Significado |
| --- | --- |
| Atual | Observação datada ainda válida, no escopo/revisão/versão confirmados. Completeness e o eixo de suporte/disponibilidade continuam visíveis. |
| Desatualizada | Última observação conhecida está vencida ou seu escopo deixou de ser válido. Pode ser mostrada como histórica, com data; não como confirmação atual. |
| Desconhecida | Não há evidência suficiente ou a fonte não expõe o recurso. Uma leitura bem-sucedida pode legitimamente manter `unknown`. |
| Indisponibilidade confirmada | Evidência específica do acesso/modelo naquele escopo, com fonte/data/validade. Não significa necessariamente suporte negado ou descontinuação. |
| Falha temporária de consulta | Timeout/transporte/rate limit da leitura, com código e espera/ação indicada. Mantém a evidência histórica, sem convertê-la em ausência do recurso. |

Validade, estado da leitura, completude, suporte, configuração e disponibilidade são eixos separados, não um novo enum que substitui os existentes. Catálogo parcial não invalida fatos positivos efetivamente observados, mas impede inferir ausência dos itens não cobertos. Cache vencido de uma restrição obrigatória (cobrança, identidade, permissão ou capacidade exigida) requer revalidação ou bloqueio explicado; não admite execução usando esse cache. Para dados não críticos, a política conservadora pode exibir o último valor datado, mas o routing trata-o como desconhecido para ampliar elegibilidade ou garante apenas uma alternativa com evidência válida. Não afirmar novo acesso/capacidade com base em informação antiga.

Consultas automáticas usam somente metadata/configuração autorizadas expostas pelo adapter, sem alterar permissões, aceitar acordos, iniciar inferência, executar ferramentas ou preencher lacunas com probes potencialmente cobrados. Uma API explicitamente cadastrada pode ter leituras autorizadas; isso não libera execução API sob Subscription Only. Provas ativas continuam separadas, com objetivo, efeitos, custo/limites e autorização.

Mudanças confirmadas geram nova versão de observação e explicação visível do efeito na elegibilidade para **novas decisões**. Guardar a evidência usada por cada decisão/tentativa; não trocar modelo, thinking, configuração ou conexão de trabalho ativo silenciosamente. Erro na tentativa ativa segue supervisão/reconciliação e comandos explícitos, sem ser resolvido por redispatch ou mutação do snapshot.

**Aceite planejado:** usando relógio/respostas controlados, verificar reutilização antes do vencimento, revalidação/bloqueio de requisito crítico vencido, reconexão/versão/identidade, resultados tardios, consulta direcionada após erro, coalescência e concorrência, budgets/backoff, datas históricas intactas após timeout, catálogo parcial sem remoção inventada, explicação das mudanças e tentativa ativa inalterada. Recheck manual e gatilhos automáticos não iniciam inferência nem fallback pago. Integração e persistência pertencem a OX-003/OX-005/OX-007/OX-008; configuração/apresentação a OX-012/OX-016. O serviço de atualização não é pré-requisito novo para terminar a conformance do primeiro contrato single-account.

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

## Implemented cadence and validity — OX-016, 19 October 2026

The existing daemon owns one ConnectionService and MetadataRefreshCoordinator. Connections/Settings read status; their visual polling never schedules metadata. ConfigureRefresh stores a version-1 setting with its own expected refresh revision plus the Connection CAS revision. Migration 9 adds append-only configurations and original effective-validity envelopes beside connections-v1/migration 8; legacy connections default to manual, without recurring consent.

Automatic reads require explicit, revocable consent bound to the current binding revision and registration scope: runtime/executable hash/version, managed home and authorized registration directory. A rename does not create a new account. Disable/unlink/binding changes prevent new reads and cancel the corresponding owned metadata flight; changed identity/authentication pauses recurrence for explicit action. Importing settings cannot grant this consent.

Requested interval is 1–86,400 seconds, clamped to the existing adapter floor of 60 seconds. Effective metadata concurrency is one globally and per connection, independent of worker concurrency. A new configuration/restart schedules a future opportunity; no missed intervals are replayed after sleep/restart. Backoff is bounded to five minutes; reads retain the existing 15-second metadata deadline and finite stream/page budgets. The existing diagnostic history bound is 512 rows per connection; exhaustion suspends recurrence with a visible reason, rather than growing history without limit.

There are three authorities for freshness. The metadata source retains its original 60-second lease. Policy observation TTLs remain mandatory maxima for identity/billing, availability and capability/configuration. The per-connection class preferences may shorten those maxima. Effective validity is the minimum of source, policy and requested class TTL; the whole diagnostic is current only while all critical classes remain valid in the same daemon generation. The UI shows source and effective deadlines separately. All restrictions introduced since an observation was made are retained for that observation: changing a preference/policy never moves its date or revives a previously shortened lease. Cached metadata keeps the original date and lease cohort. A new read can establish new evidence; a status read cannot.

Registration and task-preparation directories remain distinct scopes. Foreground preparation takes priority by cancelling an owned periodic read and waiting for closure within the existing preparation budget, using the same command identity. No SQLite writer remains open while waiting for a provider. Suspending task admissions does not suspend metadata consent; Disable recurring reads and Cancel diagnostic are separate actions. Periodic reads cannot create threads, turns, tools, login or forced credential refresh, nor confirm a changed identity, retry work or alter active attempts. Missing quota/entitlement stays unknown.

Diagnostic refresh and the worker's pre-reservation discovery share the same process-local metadata service/cache/read pool. Changing class validity or policy TTLs invalidates affected cached facts and pending tickets; changing only cadence reuses valid cache. A cancelled physical read retains its slot until shutdown returns, even after its logical ticket was invalidated. The read-pool bound describes these discovery rounds, not every protocol message: the executing worker still performs its mandatory in-band handshake in its own supervised process and checks the actual worktree scope before a turn. Worker concurrency remains one.
