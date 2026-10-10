# Descoberta imediata de recursos e controles — 10/10/2026

Salvar uma conexão API inicia a consulta do catálogo no host de desenvolvimento. A interface acompanha o resultado em Connections e em **Settings → Accounts → View resources**, com atualização e cancelamento. Os modelos apresentam compatibilidade, contexto, reasoning, modalidades, streaming, chamadas de ferramentas, pesquisa web nativa e ferramentas do Orchestrix como propriedades independentes. Informação ausente permanece **Not reported**; a lista distingue resultados completos, parciais e indisponíveis.

## Caminhos integrados

| Conexão | Operação ao salvar | Limite conhecido |
| --- | --- | --- |
| OpenAI API | `GET /v1/models` com a chave fornecida | O catálogo básico não fornece todas as capacidades de cada modelo. Não inferir thinking/tools pelo nome. |
| Anthropic API | `GET /v1/models`, percorrendo cursores | Preservar somente campos de capacidades realmente retornados pela versão da API. |
| Gemini Developer API | Listagem de modelos em Google AI, com paginação | Métodos e token limits disponíveis não provam suporte de todas as ferramentas; uma chave não conecta Antigravity. |
| Azure AI | Catálogo `/openai/v1/models` com chave do recurso | Deployment declarado continua não verificado. Enumeração ARM exige autenticação de gerenciamento própria. |
| Bedrock com chave | Catálogo de foundation models no endpoint de controle regional e páginas de inference profiles | Permissões IAM/região podem limitar a listagem; listar não comprova entitlement de inferência. |
| API compatível | `/models` somente no endpoint informado | Protocolo declarado não transfere capacidades de OpenAI. HTTP é permitido apenas para backend loopback explicitamente escolhido. |
| AWS profile | Registrar a necessidade do signer desktop | O wizard não interpreta um nome de perfil como credencial verificada. |
| Assinatura/runtime | Registrar tentativa e ausência de catálogo vinculado | Login oficial e vínculo da identidade são necessários; não reutilizar o login global para uma conta de nome arbitrário. |

O [pacote de descoberta](../../tools/provider-discovery/README.md) também oferece leitura Azure ARM com token de gerenciamento próprio e um helper Codex para transporte RPC já inicializado e pertencente ao chamador. Esses caminhos de biblioteca não estão expostos pelo wizard/bridge. Vertex/ADC e discovery Claude/Antigravity continuam na pesquisa/backlog; não são conexões implementadas nesta entrega.

## Contrato e fronteira

O renderer guarda a chave apenas em uma closure privada durante a sessão e a envia ao host somente para a consulta. O host usa uma sessão efêmera, valida Origin/Host/CSRF e repassa a chave ao endpoint apropriado. Nenhum segredo entra em estado de UI, localStorage, arquivos, argumentos de processo ou logs. Limites de requisição/resposta, páginas, modelos, tempo e concorrência são explícitos. Redirects são recusados e a resolução DNS do transporte padrão é validada e fixada ao destino permitido. Cancelamento, troca de credencial, desconexão e pagehide descartam respostas tardias.

Resultados preservam origem, timestamp, escopo da conexão/backend/endpoint/região/resource ID e evidência por capacidade. Modelos e inference profiles permanecem recursos distintos. Falhas transitórias depois de páginas válidas preservam um catálogo parcial; resposta malformada, redirect inseguro ou eco de credencial invalidam a consulta. Capacidade do provedor/runtime não é automaticamente atribuída a todos os modelos.

Não há prompts de teste, probes de inferência, invocação de ferramentas, criação de acordos cloud ou fallback pago ao cadastrar. Catálogo não torna uma conexão elegível para execução, não confirma quota/saldo e não fecha o login de runtime. A produção ainda precisa do host desktop/cofre e dos gates existentes. Em `file://`, o app continua utilizável; consultas de API exigem executar `npm start` em `prototypes/desktop` e abrir o endereço do servidor.

## Padrão visual

[controls.css](../../prototypes/desktop/controls.css) padroniza os selects existentes, incluindo opções, chevron, separador, foco, hover, disabled/invalid e cores dos temas. São controles nativos com teclado preservado. [resources.css](../../prototypes/desktop/resources.css) apresenta resumo, estados e detalhes progressivos no estilo Studio; o usuário não precisa configurar capacidades para conhecer o catálogo.

Fontes: [transporte browser](../../prototypes/desktop/discovery-client.js), [host de leitura](../../prototypes/desktop/host-discovery.mjs), [normalização/wizard](../../prototypes/desktop/connections.js), [apresentação](../../prototypes/desktop/resource-view.js), [pesquisa oficial](../research/api-provider-discovery-2026-10-10.md) e [contrato de conexão](api-connection-contract.md).

## Verificação

- **20/20** contratos do pacote de discovery: provedores, paginação, metadata, falhas parciais, cancelamento, limites, transporte loopback e runtime RPC sem inicializar execução.
- **24/24** contratos host/browser: Origin/Host/CSRF, conteúdo, credenciais/erros sanitizados, cancelamento, concorrência, deadline de upload e transporte por escopo.
- Interface: rodada focada **64/65 em 2,5 minutos**; o caso restante encontrou uma corrida de evento `close` ao reabrir o wizard. Após a correção, **22/22** casos de API passaram em **51,8s**, cobrindo esse caso. Os 65 cenários estão validados por cobertura consolidada, sem alegar uma rodada única de 65 aprovações.
- **9/9** cenários novos do fluxo HTTP normal passaram em **16,3s**, com respostas interceptadas dos catálogos: descoberta automática, recursos ricos/parciais, erros/retry, credenciais, duas contas no mesmo endpoint, recursos de espécies distintas com mesmo ID e EN/PT/ES. Somados aos 65 anteriores, são **74 cenários de interface por cobertura consolidada**. Os cinco de dropdowns também passaram separadamente em **24,1s**, e já integram esses 65.
- HTML direto: **13 composições** e percurso de sessões, Settings, docks, idiomas e ajuda aprovados, sem erro, imagem quebrada ou requisição HTTP/HTTPS. Check de sintaxe e catálogo aprovado com **1.356 chaves**.
- Capturas desktop/mobile de dropdowns e recursos foram examinadas e estão no [catálogo textual](visual-asset-catalog.md). Credenciais usadas nos testes são sintéticas; nenhuma conta real foi consultada.

D1/piloto humano, M0 e milestones seguintes permanecem abertos. Esta entrega prepara OX-002/OX-003/OX-007/OX-016, sem alterar sua ordem ou critérios de passagem.

Build histórico deste incremento: [59 arquivos](builds/2026-10-10-before-connection-diagnostics.json), SHA-256 `f05ab28458b1b78afe3bdf9a33fda6c0a32f95c9607bec77c843b4a8eaa02800`. [Build anterior preservado](builds/2026-10-10-before-resource-discovery.json). O [incremento de diagnósticos](connection-diagnostics-increment.md) registra a revisão seguinte e sua verificação.
