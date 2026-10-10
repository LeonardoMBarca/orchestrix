# Contrato de conexão por assinatura e API

Atualizado em **10 de outubro de 2026**, com base na [pesquisa oficial de provedores](../research/api-provider-discovery-2026-10-10.md). O cadastro API agora inicia uma consulta real de catálogo pelo host de desenvolvimento, com credencial transitória, endpoints específicos, paginação e resultados sanitizados. O HTML conserva a chave apenas em memória privada e a transmite ao host no momento da consulta; não a persiste. A [entrega de descoberta imediata](resource-discovery-increment.md) distingue caminhos integrados, adapters de leitura e recursos ainda dependentes de autenticação. Listagem não habilita inferência ou confirma quota/entitlement. Host desktop de produção, cofre e login oficial continuam nos milestones existentes; ADRs e gates permanecem.

## Entrada em duas etapas

O wizard começa pela origem da conexão:

1. **Subscription:** usar uma conta/assinatura por uma modalidade oficialmente suportada. Depois selecionar o runtime/provedor, a forma de autorização e a identidade correspondente.
2. **API:** usar uma API própria do usuário, com origem de autorização/cobrança independente. Depois selecionar provedor/backend e método de autenticação.

Uma opção de runtime não implica API; uma chave Gemini não conecta Antigravity. A mesma família de modelo pode aparecer em diferentes backends com capacidades diferentes. Nome da conexão, perfil de worker e sessão de chat continuam independentes. A [pesquisa de contas de assinatura](../research/subscription-account-ux.md) continua válida para esse caminho.

O wizard pede somente os campos essenciais à escolha atual e abre detalhes adicionais quando necessários. Não começar mostrando todos os provedores, regiões e configurações avançadas. Retornar à primeira etapa preserva rascunhos não secretos, sem reaproveitar uma credencial no backend errado.

## Campos condicionais do caminho API

| Backend | Entrada essencial | Descoberta / detalhes adicionais |
| --- | --- | --- |
| OpenAI API | Rótulo, credencial segura e endpoint do adapter | Organização/projeto quando necessários; catálogo da conexão e superfície suportada. |
| Anthropic API | Rótulo e credencial segura | Versão da API e metadata de modelos geridas pelo adapter; não transportar capabilities para outro host. |
| Gemini Developer API | Rótulo e chave segura | Endpoint Google AI e versão do adapter; projeto associado como metadata quando conhecido. Não exigir login Antigravity. |
| Google Cloud / Vertex Standard | Identidade/ADC, projeto e location | Endpoint e catálogo do publisher/deployments conforme adapter; permissões de controle separadas das de inferência. |
| Google Cloud / Vertex Express | Chave segura e modo Express | Endpoint próprio sem project/location artificiais; subconjunto de catálogo e capacidades documentado. |
| Azure / Microsoft Foundry | Endpoint, autenticação e deployment | Tipo de API/versão; subscription/resource group/resource account apenas para descoberta ARM autorizada. Deployment manual continua não verificado. |
| Amazon Bedrock | Região, família de endpoint e modo API key ou credenciais AWS | Referência de chave ou profile/SSO/identidade; catálogo nativo, inference profiles ou compatibilidade conforme backend. |
| OpenAI-compatible / Custom | Base URL, protocolo e método de autenticação | Catálogo se suportado; ID manual quando indisponível. Capacidades permanecem desconhecidas até evidência. |

Essa tabela é o desenho do **contrato**, não a afirmação de que todo método já existe no Orchestrix. A justificativa e as URLs oficiais para cada campo estão junto aos achados na [pesquisa](../research/api-provider-discovery-2026-10-10.md). Usar credenciais temporárias/identidade quando suportadas; o renderer não implementa fluxos ADC/SSO ou armazena chaves de longa duração por conta própria.

## Estado da conexão e descoberta real

Separar estados que a UI costuma confundir:

| Estado | Evidência necessária |
| --- | --- |
| Configured | Campos não secretos válidos e referência de autenticação registrada. Não implica login. |
| Authorization pending | Fluxo oficial iniciado pelo adapter, aguardando conclusão. |
| Authentication verified | Resposta válida de verificação oficial para o escopo indicado. |
| Catalog discovered | Resposta do catálogo concreto, com origem, versão, data e paginação completas ou parcialidade indicada. |
| Execution observed | Operação autorizada respondeu para aquele modelo/endpoint/configuração; não comprova todas as combinações futuras. |
| Unavailable / expired / denied | Falha tipada, com causa observada e ação de recuperação adequada. |

Autenticação, acesso a catálogo, entitlement de modelo, capacidade de inferência e telemetria são observações separadas. Um `401`, `403`, `404` ou `429` recebe a interpretação do adapter e do corpo de erro; não vira uma conclusão genérica de que o provedor não existe, a senha está errada ou a conta consumiu 100%. Falha de listagem permite o caminho manual quando o backend o suporta, sem falsa aprovação.

A descoberta deve utilizar a operação documentada do backend, nunca sondar indiscriminadamente rotas `/models`. Respeitar paginação, permissões, região/projeto e eventual diferença entre plano de gerenciamento e inferência. Cache é versionado e atualizável; uma troca de conta, endpoint, região, deployment ou credencial invalida observações incompatíveis. Uma lista parcial deve ser apresentada como parcial.

Não executar prompts de teste, ferramentas ou probes potencialmente cobrados ao salvar o formulário. Verificação de execução é uma ação explícita, com origem API e parâmetros limitados compreensíveis. A ausência de um probe pago não impede salvar uma configuração não verificada.

O cadastro inicia também os [diagnósticos específicos da conexão](connection-diagnostics.md), consumindo leituras oficiais de metadata/configuração e o catálogo daquele escopo. A rodada apresenta várias verificações e permite recheck em Settings. Suporte, disponibilidade, configuração/política e estado da verificação são eixos distintos: confirmar que uma capability não é suportada pode concluir uma leitura com sucesso, enquanto ausência de autenticação bloqueia a leitura e campo omitido permanece desconhecido. Runtime sem adapter autenticado não recebe prova fictícia de recursos.

Modelo, reasoning/thinking, service tier de velocidade, multi-agent nativo e permissões são diagnosticados separadamente. Ultra pode envolver delegação no produto do fornecedor, mas seu nome não confirma a configuração efetiva de um runtime. Quando o Orchestrix coordena workers, delegação nativa aninhada fica desativada por padrão na política de execução; o diagnóstico não modifica configuração pessoal nem declara essa política aplicada a um processo ainda não validado. Sandbox/aprovação/Full access nativos não se aplicam automaticamente a uma API de modelo.

Um recheck conserva conta, backend, endpoint, região e revisão; troca de identidade ou desconexão invalida a rodada. Respostas tardias não atualizam outra conexão. Probes ativos futuros continuam separados do cadastro/recheck, com modelo, efeitos, limites e eventual cobrança explicitamente definidos; não são requisito para salvar uma configuração pendente. O contrato de diagnósticos registra a matriz por provedor e sua continuidade nos tickets existentes.

## Identidade do catálogo e proveniência

Uma entrada de modelo é identificada por **conexão + backend + endpoint + região/projeto quando aplicáveis + deployment/profile/model ID + versão da API/adapter**. Alias e snapshot devem ser separados quando conhecidos. O registro nunca usa apenas o nome exibido para reutilizar capacidades entre contas ou provedores.

Cada propriedade mantém valor, escopo, origem e momento da evidência:

- `provider-metadata`: campo devolvido pelo endpoint, com resposta sanitizada ou referência de observação.
- `official-documentation`: URL, data de consulta e versão à qual a afirmação se aplica.
- `runtime-observation`: operação concreta e combinação efetiva que produziu a evidência.
- `user-declaration`: preferência/configuração manual, sem promover a informação a fato do provedor.

O suporte normalizado segue `supported`, `unsupported`, `unknown` ou `runtime-managed`, consistente com as [decisões de integração](../research/integration-decisions.md). **Fonte e suporte são eixos diferentes:** informação documentada não é observação daquela conta; uma falha de rede não estabelece `unsupported`. Conflitos ficam explícitos e restringem escolhas automáticas até resolução. O cache não vence um erro atual de acesso ao modelo.

## Modelo de capacidades do adapter

| Grupo | Propriedades a representar |
| --- | --- |
| Operação | Superfícies disponíveis, geração de texto, streaming, modalidades de entrada/saída, structured output e restrições por versão. |
| Parâmetros | Schema permitido: nomes, tipos, ranges/enums, defaults conhecidos, parâmetros incompatíveis e regras condicionais. Campo ausente não vira default inventado. |
| Tokens/contexto | Limites de entrada, saída e janela; tokenizer/counting quando disponível; reserva de saída/reasoning e limite efetivo do deployment. |
| Reasoning/thinking | Suporte, níveis/budgets/modos aceitos, possibilidade de desativar, dependências e tradução da intenção neutra para parâmetros nativos. |
| Ferramentas | Function calling, chamadas paralelas, ferramentas nativas hospedadas, ferramentas executadas pelo Orchestrix e MCP, cada uma separadamente. |
| Pesquisa web | Suporte da ferramenta específica, condições de API/modelo/effort, grounding/citações, permissões e possível cobrança adicional. |
| Lifecycle/acesso | IDs/aliases, versão, estado de depreciação, disponibilidade conhecida, origem de autorização e últimos erros de acesso. |
| Telemetria | Tokens reais retornados, cache/reasoning quando informados, rate limits, quota, janela/reset, gasto, saldo e crédito como propriedades independentes. |

Metadata rica dos provedores deve ser utilizada; o contrato não força todos a um catálogo apenas de IDs. Do mesmo modo, o conector não cria metadados que um backend não retorna. Nome contendo “pro”, “reasoning”, “code” ou “search” não determina qualidade, ferramentas, limites ou preço.

**Function calling é solicitação de execução, não a execução em si.** Uma API de modelo não é automaticamente um agente que lê arquivos, usa Git ou executa comandos. Ferramentas fornecidas pelo Orchestrix continuam dependentes de executor, permissões, isolamento e evidência dos gates existentes. Uma ferramenta de servidor também não concede acesso irrestrito à máquina do usuário.

## Roteamento e classificação

Efficiency, Balanced, Performance e Custom continuam intenções neutras. O motor V1 utiliza regras determinísticas e explicáveis sobre tipo de tarefa, risco, importância, orçamento/contexto e combinações efetivamente elegíveis. A seleção funciona com uma única conta e runtime. Presets especializados refinam essas regras; não liberam APIs ou ferramentas vedadas.

A classificação da tarefa e a classificação das capacidades do modelo são distintas. Um adapter pode fornecer tags de modalidade/aptidão com proveniência; preferências de qualidade ou rankings precisam de documentação/eval identificado, não somente nome, preço ou tamanho. Otimização por histórico e classificadores sofisticados permanecem evolução posterior.

Antes de despachar, validar ferramentas obrigatórias, token limits e schema de parâmetros. Uma intenção de “mais thinking” é traduzida apenas para valores nativos aceitos. Quando a preferência não puder ser atendida, falhar ou oferecer ajuste explícito conforme política; registrar **requested** e **effective**, sem fingir equivalência perfeita entre provedores. O [ADR-0002](../adr/0002-model-and-reasoning-policy.md) permanece a referência.

Verificações determinísticas, como status do Git, devem usar a ferramenta autorizada apropriada, sem pedir reasoning elevado para produzir um fato observável. Se essa ferramenta ainda não estiver implementada, a interface não apresenta um resultado fictício.

## Assinatura, API e autorização de gasto

Subscription Only exclui conexões API da execução, inclusive APIs cadastradas, com free tier ou crédito conhecido. Um limite de assinatura não habilita automaticamente API, não troca a origem de cobrança e não aceita fallback pago silencioso.

Permitir API requer opção explícita e escopo visível; cadastro de uma conexão não libera seu uso global. O fluxo identifica a origem antes da primeira execução, respeita limites/políticas estabelecidos pelo usuário e conserva a escolha no snapshot da tentativa. Defaults mantêm o princípio de ausência de cobrança adicional no modo de assinatura.

Tokens usados no Orchestrix não medem todo o consumo de uma conta. Gasto observado não é saldo; saldo não é limite da assinatura. Percentuais só aparecem quando há observação válida, denominador e janela correspondentes. Custo estimado guarda modelo de preço, moeda, fonte e data; não aparece como valor faturado. Telemetria administrativa exige consentimento/credencial própria quando necessário; dados indisponíveis continuam **Not reported**. A [pesquisa de uso/custo](../research/api-provider-discovery-2026-10-10.md#uso-quota-créditos-e-custo) registra os limites oficiais consultados.

## Segredos e fronteira do host

Credenciais reais ficam no processo/host seguro, com broker e cofre do sistema conforme a implementação desktop aprovada. O domínio e a persistência recebem um `credentialRef` opaco, metadata não secreta e estados observados. A escolha técnica do cofre deve ser validada no milestone correspondente; este contrato não declara Credential Manager/Keychain/Secret Service implementados.

Chaves, refresh tokens, access tokens e secrets IAM não entram em `localStorage`, projeto/workspace, histórico de chat, arquivos de exemplo, manifestos, capturas, argumentos de processo ou logs. Renderer recebe apenas resultados sanitizados. O [contrato arquitetural de segurança](../ARCHITECTURE.md) continua aplicável.

Endpoint custom é destino de dados e autorização, portanto deve ser reconhecido explicitamente. Validar formato/protocolo no host; impedir envio de credencial por redirecionamento para outro host ou reutilização automática em provider diferente. Endpoint HTTP local exige uma escolha apropriada ao backend local; não degradar HTTPS de uma API remota silenciosamente. Revogar/remover uma configuração não deve ser anunciado como revogação no provedor sem resposta que a comprove.

## Critérios para as etapas existentes

- **D1:** a pessoa distingue assinatura de API, consegue escolher provider/método sem campos irrelevantes, entende configuração pendente e encontra uso desconhecido. Interface sem fixtures conectadas por padrão; nenhuma chave real ou discovery simulada promovida a prova.
- **M0 / adapter selecionado:** demonstrar autenticação e isolamento no caminho oficial, descoberta ou limitação documentada, capacidades com proveniência, validação de parâmetros, erros e cancelamento; confirmar ausência de fallback API sob Subscription Only. Novos providers seguem o mesmo contrato conforme priorização existente.
- **Persistência e execução:** validar referências de segredo, snapshots de configuração e retomada conforme os milestones existentes. Estado do HTML não substitui banco/recuperação de M1.
- **Expansão:** adicionar adapters e telemetria somente com evidência do backend, preservando compatibilidade e histórico. Publicação do nome de um provider no wizard não fecha essa entrega.

Referências: [pesquisa oficial](../research/api-provider-discovery-2026-10-10.md), [pesquisa de assinatura](../research/subscription-account-ux.md), [política de modelo/reasoning](../REASONING_AND_MODEL_POLICY.md), [plano](../DEVELOPMENT_PLAN.md) e [backlog](../DEVELOPMENT_BACKLOG.md).
