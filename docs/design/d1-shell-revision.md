# D1 — revisão da navegação, sessões e configurações

Registro em **10 de outubro de 2026**, após os doze pontos iniciais e os sete ajustes seguintes de revisão do responsável. A identidade navy/índigo e a Direção 01 continuam como base. A estrutura da interface foi refinada para começar com uma conversa simples e revelar ferramentas conforme a necessidade.

**O piloto está adiado por solicitação explícita do responsável até que a interface corresponda à direção desejada.** Esta revisão não registra aprovação das novas interações nem resultados de uso humano. D1 continua aberto; os milestones e gates de M0 em diante são preservados.

## Os doze pontos e a resposta da interface

| Pedido | Implementação desta revisão |
| --- | --- |
| 1. Remover o destaque de “local” do site e app | Apresentação pública usa linguagem de produto, sem “Local-first” ou “Local project · demo”. Isso muda a apresentação; a arquitetura local e os limites de integração continuam documentados. |
| 2. Corrigir a marca/seleção de projeto com texto quebrado | A marca fixa é Orchestrix. Projeto e sessão saem do antigo card/seletor sobrecarregado e ficam na lista lateral; nomes longos devem caber na estrutura, sem cobrir bordas ou outros controles. |
| 3. Arrastar painéis | Navigation, Sessions e Work têm posições persistidas, alças de arraste e controles equivalentes por teclado/menu. A revisão seguinte retira o destino inferior; esquerda e direita ficam destacadas com o workspace desfocado. |
| 4. Simplificar projetos e sessões | A primeira entrada é uma sessão independente. O usuário pode criar uma sessão em um projeto, definir diretório para uma independente ou associá-la a um projeto posteriormente. Cada sessão conserva mensagens, rascunho e trabalho próprios. |
| 5. Ajuda externa com explicações e imagens | Help abre uma nova página, fora da navegação do app. O centro de ajuda em inglês tem busca, nove capítulos, cinco diagramas locais e espaços de vídeo que aparecem apenas quando há uma fonte real. |
| 6. Botão de configurações com engrenagem | O controle abre a janela flutuante Settings, sem substituir a conversa. |
| 7. Melhorar a busca | Busca ganha espaço, identidade de campo/ação e foco legível. Sessions permite encontrar conversas e projetos pelos nomes, sem prometer indexação de todos os arquivos do computador. |
| 8. Substituir os dois dropdowns pela lateral de sessões/projetos | Sessions apresenta projetos expansíveis, conversas e ações de criação/associação. Painéis no mesmo dock compartilham abas. |
| 9. Idioma e aparência dentro de configuração | General contém Interface language; Themes reúne a galeria e restauração de Studio. English permanece padrão, com Português e Español no app. |
| 10. Linguagem de produção | Referências visíveis a demo, exemplo e simulação saem do fluxo normal. Dados de estudo e controles de transição artificial permanecem internos ou exclusivos do sinal de teste. |
| 11. Reordenar a entrada | Logo acima de “What shall we create today?”, três cartões de início abaixo, sugestões rápidas depois e composer em seguida. Retirados os textos introdutórios redundantes. |
| 12. Settings flutuante e completa | General, Accounts, Themes, Conversation, Orchestration e Panel layout organizam perfil, uso observado, preferências de resposta, roteamento e disposição dos painéis. A janela fecha e permite continuar usando o chat. |

## Contrato do primeiro uso

Uma abertura normal, sem estado salvo, começa sem projeto ou conta fictícia conectada e sem tarefas preenchidas. Há uma sessão independente pronta para receber um rascunho. As entradas rápidas ajudam a conectar uma conta, organizar um projeto ou encontrar orientação; nenhuma delas exige configurar toda a plataforma antes de conversar.

Projetos agrupam sessões e contexto compartilhado. A sessão é a unidade de conversa. Associar uma sessão a um projeto preserva sua identidade, histórico e rascunho; o diretório e o contexto do destino devem ficar explícitos. Registrar um caminho no HTML não abre, lê ou concede acesso à pasta real.

A persistência do protótipo usa `localStorage` para projetos, sessões/rascunhos e preferências. Ela facilita retomada no mesmo navegador, com validação dos dados e aviso se não puder salvar. **Não é o banco transacional, a recuperação de execução nem a durabilidade de M1.** Estado de navegador pode ser apagado ou indisponível; o contrato de armazenamento real continua no plano.

## Painéis e Settings

Sessions começa à esquerda, com um único botão principal New session; Navigation começa à direita. Work aparece quando há trabalho na sessão e pode compartilhar as abas desse dock. Cada painel pode ir para esquerda ou direita. O destino inferior foi retirado e layouts antigos são migrados. Durante a escolha de lado, o restante do workspace recebe blur; Escape cancela e restaura o foco. Agrupar painéis cria abas em vez de comprimir várias interfaces simultaneamente. Redimensionamento, menus de posição, teclado e reset oferecem alternativas ao arraste.

Settings é uma janela **modeless**: sem camada que torne o restante do app inerte e sem prender o teclado dentro dela. Ela abre centralizada, com até 940 × 720 px, limitada ao viewport. Clicar fora fecha a janela e permite que a ação no chat receba foco normalmente; o rascunho permanece. Fechar pelo botão ou por Escape com foco na janela restaura um foco válido; mover a janela não muda a sessão. Perfil e instruções têm salvamento explícito; tema e idioma aplicam-se imediatamente. Preferências de resposta são incorporadas às novas tentativas e não alteram retroativamente snapshots existentes.

Accounts mantém assinatura e API como origens de cobrança distintas. A UI só apresenta percentual, renovação ou crédito/gasto quando há telemetria observada com origem de provider/runtime e valores válidos. Uma fixture nunca comprova quota ou crédito. Valores desconhecidos continuam **Not reported**, e uma configuração manual não comprova login ou acesso ao modelo. Subscription Only não autoriza fallback pago.

## Sete ajustes de refinamento

| Pedido | Contrato atual |
| --- | --- |
| Entrada sem scroll | Textarea inicialmente de70px, reduzido no [refinamento de11/10](compact-composer-refinement.md) a duas linhas com altura proporcional à fonte; primeira tela inteira em 1440×900, 1280×720 e 1024×768, a 100%, nos três idiomas. Mobile e texto ampliado usam reflow/rolagem para preservar os controles. |
| Um criador de sessão e lateral invertida | Sessions à esquerda por padrão; New session principal somente nesse painel. A ação específica de um projeto cria uma sessão naquele projeto. |
| Sem dock inferior; foco nos destinos | Apenas esquerda/direita, com blur durante arraste ou menu de posição. Cancelamento/teclado e persistência preservam a conversa. |
| Settings centralizada e fechamento externo | Abre centralizada; clique externo fecha sem consumir o foco do workspace. Rascunhos de perfil/instruções e conversa são preservados. |
| Compact e porcentagem | Compact é padrão em novas preferências; escolhas antigas explícitas são preservadas. Range 80–200%, passos de 5%, com valor ao vivo, teclado, reset 100% e persistência. |
| Origem de conexão antes do provider | Subscription/API primeiro; depois runtime ou provedor. OpenAI, Anthropic, Gemini, Azure AI, AWS Bedrock e OpenAI-compatible têm campos condicionais. |
| Configurações maiores e estilizadas | Janela até 940×720 com abas em categorias, ícones e indicador de seleção; reflow em telas estreitas e texto ampliado. |

A [pesquisa oficial de APIs](../research/api-provider-discovery-2026-10-10.md) e o [contrato de conexão](api-connection-contract.md) definem os detalhes de descoberta e limites. No HTML, chaves ficam apenas na memória privada da sessão; não são salvas no estado, histórico ou localStorage. Não há adapter de rede de produção neste estudo: o seam de descoberta é exercitado com adapters de teste, timeout, cancelamento, escopo e descarte de respostas tardias. Configuração salva permanece pendente e inelegível para execução. Catálogo não prova autenticação, quota ou inferência.

## Apresentação e capacidade real

A retirada de rótulos de demonstração não declara implementados OAuth, adapters, execução, Git real, instaladores, cotas ou cobrança. O fluxo de conexão registra configuração e, no caminho API com host, consulta catálogos de leitura; autorização de execução do provedor continua não verificada. Os controles que fabricam eventos de limite, descoberta, expiração e revogação ficam restritos a `window.__ORCHESTRIX_TEST_SCENARIO === 'seed'`, usado pelos testes.

O site continua em inglês e mantém downloads não publicados como **Coming soon**. O app oferece EN/PT-BR/ES. A ajuda é uma página pública em inglês e abre fora do app, por HTTP ou HTML direto. Vídeos só ganham player com mídia configurada; nenhum vídeo inexistente é apresentado como disponível.

## Verificação e próxima avaliação

**Incremento histórico de recursos:** [dropdowns e descoberta de recursos](resource-discovery-increment.md) adicionam consultas API de leitura ao cadastro, detalhes em Accounts e estilos nativos compartilhados. Check daquele incremento:1.356 chaves; 44 contratos Node e74 casos browser por cobertura consolidada. O [manifesto histórico](builds/2026-10-10-before-connection-diagnostics.json) tem **59 arquivos**, SHA-256 `f05ab28458b1b78afe3bdf9a33fda6c0a32f95c9607bec77c843b4a8eaa02800`. A verificação abaixo descreve a versão Compact/API anterior e continua como histórico.

**Verificação histórica do refinamento Compact/API:** check de sintaxe e catálogo aprovado, **1.303 chaves EN/PT-BR/ES**. A suíte completa aprovou **128/130 casos em 5,3 minutos**; dois testes antigos de configuração do vídeo excederam 20s e passaram no reteste isolado, **2/2 em 1,2s e 1,8s**. Assim, os **130 casos foram validados por cobertura consolidada**, sem afirmar uma rodada única de 130 aprovações. O HTML direto passou **13 composições**, incluindo ausência de rolagem inicial nos notebooks a 100%, sessões/associação/rascunhos, docks laterais/blur, Settings/fechamento externo, slider e idiomas persistidos e ajuda ilustrada; não houve erros, chaves ausentes, imagens quebradas ou requisições HTTP/HTTPS. Entrada, Settings e formulários foram examinados e [catalogados](visual-asset-catalog.md). O [manifesto daquela revisão](builds/2026-10-10-before-resource-discovery.json) tem **54 arquivos**, SHA-256 `cd630bca81ddd735c677e059cd73bf4c2b17d3a455e377f247ab3e308424caf0`; a [versão anterior](builds/2026-10-10-before-compact-settings.json) está preservada. Estas verificações não fecham o piloto humano ou os gates de runtime.

**Evidência histórica anterior aos sete ajustes atuais:** `npm.cmd run check` passou com catálogo de **1.240 chaves**. A suíte completa passou **101/101 casos em uma rodada de 5,8 minutos**, sem falhas ou casos ignorados. Os ajustes finais de distribuição no dock inferior e foco com texto a 200% passaram **2/2 casos focados em 7,1 segundos**, separadamente. O check de HTML direto passou sete composições e a jornada de sessões/associação/rascunhos, docking, Settings modeless, idiomas após reload e ajuda com imagens locais, sem erros, imagens quebradas, chaves ausentes ou requisições HTTP/HTTPS. Capturas desktop, mobile, abas à esquerda, dock inferior e Settings foram examinadas e catalogadas. O manifesto daquela revisão reúne **52 arquivos**, SHA-256 `c7480d96183a977eb515b852c932f0a3103ec0da6e8a8cee38ef4f022d5380ba`; o anterior está arquivado em `builds/2026-10-10-before-session-panels.json`. Os ajustes corrigiram conflitos entre teclados de abas, sobreposição da navegação a 200%, foco após fechar criação de projeto, clique nos radios de temas e texto da busca oculto em mobile. Fixtures de execução continuam restritas aos testes; esta evidência não fecha o piloto humano ou os gates de runtime.

O responsável primeiro avaliará a interface e decidirá quando retomar o [piloto](d1-pilot.md). A avaliação inicial da nova navegação pode conferir criação/associação de sessões, rascunhos, busca, docking, Settings e ajuda sem depender de runtime. Os cenários de resultado/correção/validação/aplicação e os demais casos P0 continuam necessários para a decisão de passagem, com a condição e a evidência de cada um explícitas.

Referências: [contrato de estilo do shell](../../design-system/workspace-shell.md), [catálogo de imagens](visual-asset-catalog.md), [guia externo](../../prototypes/desktop/help.html), [plano](../DEVELOPMENT_PLAN.md), [backlog](../DEVELOPMENT_BACKLOG.md) e [status](../DEVELOPMENT_STATUS.md).
