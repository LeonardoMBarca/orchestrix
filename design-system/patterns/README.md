# Padrões de experiência

A organização acompanha a direção aprovada: **site de apresentação → entrada pelo chat → informação técnica conforme a necessidade**. O [contrato do shell](../workspace-shell.md) e a [revisão de sessões/Settings](../../docs/design/d1-shell-revision.md) registram a organização atual; [experiência progressiva](../../docs/design/d1-progressive-experience.md) e [direção de projetos/conversas](../../docs/design/d1-chat-direction.md) preservam sua evolução.

## Site de apresentação

O site tem identidade navy/índigo fixa, logo transparente e ambientação noturna com montanhas. A abertura apresenta a proposta em linguagem acessível, com Download Orchestrix e See Orchestrix in action. A navegação reúne How it works, The experience, Download e GitHub.

As seções explicam coordenação, conversa/Studio, seis modos de ação, perfis dos agentes e estratégias de recursos. Os agentes de código são apresentados como capazes de ajudar em mais tipos de criação; uma conta também é um uso válido. A frase aprovada é: “We call them coding agents, but no single name can capture everything they can help you create.”

O vídeo tem página própria e configuração preparada. Download corresponde à disponibilidade real por plataforma; enquanto não houver instaladores, a página apresenta Coming soon. Textos, exemplos, controles e metadados públicos são em inglês. A apresentação mantém seus CTAs atuais e respeita movimento reduzido.

Fonte: [website.html](../../prototypes/desktop/website.html), [website.js](../../prototypes/desktop/website.js), [watch.html](../../prototypes/desktop/watch.html) e [configuração do vídeo](../../prototypes/desktop/assets/video/README.md).

## Entrada do aplicativo

O app acolhe quem quer conectar uma conta e começar pelo chat. Logo, pergunta curta, três cartões de início, sugestões rápidas e composer têm prioridade, nessa ordem. Uma abertura sem estado salvo começa com sessão independente, sem contas/projetos/tarefas fictícios. Criar projeto é uma alternativa, e preferências detalhadas ficam acessíveis conforme a pessoa precisa delas.

Uma conta ou várias atendem ao mesmo percurso. Tema, idioma, densidade e escala de texto são preferências de apresentação, independentes de estratégias, quotas, conexão ou modelo. Studio é o tema padrão; **Compact** é a densidade inicial e **100%** a escala de texto. A pessoa escolhe outros temas em Settings → Themes e ajusta General pelo slider **80–200%, em passos de 5%**.

A ambientação de montanhas/estrelas permanece no site. No app, a mesma identidade de cor, marca e interação serve à leitura, à conversa e ao acompanhamento prolongado do trabalho. Sessions começa à esquerda, com um único comando principal **New session**, substituindo seletores separados de projeto/conversa. Navigation e Work começam à direita. Os painéis podem trocar entre esses dois lados ou compartilhar abas; não há destino inferior. O blur durante a escolha de lado destaca os destinos e desaparece ao concluir/cancelar.

Settings mantém idioma/temas organizados, sem expor utilitários redundantes na entrada. A janela abre centralizada, até **940 × 720px**, adaptada ao viewport, e permanece movível/modeless. Clicar fora fecha e permite voltar ao chat. Rascunhos de perfil/instruções sobrevivem à reabertura durante a sessão do app; salvar continua sendo uma ação explícita. Fechar não descarta preferências visuais já aplicadas.

Preservar fontes, alvos de interação e ordem coerente do DOM nas composições compactas. A nova entrada usa rolagem/reflow natural, sem promessa de campo ou todas as ações caberem no primeiro viewport. O [refinamento mobile anterior](../../docs/design/d1-progressive-experience.md#refinamento-da-entrada-mobile--1010) conserva suas medidas como histórico. Conforto/compreensão da revisão atual continuam pendentes; o responsável adiou o piloto até considerar a interface adequada.

## Projeto, conversa e trabalho

Projeto e sessão têm identidades próprias e aparecem na lista Sessions. Um projeto pode ter várias sessões; uma independente pode começar sem repositório, receber um diretório ou ser associada a projeto posteriormente. Uma sessão nativa de um agente é um conceito técnico distinto e aparece no detalhe da execução quando relevante.

Pedidos e mensagens ficam associados ao trabalho na conversa. Contexto essencial e próxima ação devem ser legíveis ali. Conta, modelo, reasoning solicitado/efetivo, contexto versionado, tentativas e evidências podem ser inspecionados em Studio e no detalhe correspondente.

Revisar, pedir correção, aceitar tarefa e aplicar resultado têm ações e destinos explícitos. Uma alteração de aparência/idioma não pode disparar trabalho nem mudar a identidade de uma tentativa. A definição desses contratos continua nos documentos do domínio e no plano de implementação.

## Divulgação progressiva

| Nível de informação | Conteúdo prioritário |
| --- | --- |
| Começar | Objetivo/pedido, projeto ou conversa, conexão essencial e composer. |
| Acompanhar | Mensagens, trabalho ligado ao pedido, estado atual, bloqueio e próxima ação. |
| Revisar | Artefato/diff, critérios, verificações, correção, aceite e aplicação. |
| Inspecionar/personalizar | Tentativas, contexto, modelo/reasoning, políticas, perfis, sessões e histórico técnico. |

A pessoa pode acessar Studio e preferências quando desejar. Crescer em detalhe significa ampliar a informação disponível preservando a conversa como caminho utilizável; a interface não exige que todo usuário configure cada opção.

## Idioma e conteúdo

English é padrão no app, com Português e Español em **Settings → General → Interface language**. Themes reúne aparência. A preferência persiste independentemente dos projetos e runtimes. Rótulos da interface acompanham a seleção; nomes, mensagens, código, rascunhos, caminhos e histórico permanecem como foram escritos. Ver [contrato de idiomas](../../docs/design/interface-languages.md) e [guia dos catálogos](../../prototypes/desktop/locales/README.md).

## Referência para a implementação

Adicionar uma conexão começa por **Subscription ou API**, seguido do provedor/backend. Pedir apenas campos essenciais da escolha atual. O [contrato de API](../../docs/design/api-connection-contract.md) e a [pesquisa oficial](../../docs/research/api-provider-discovery-2026-10-10.md) distinguem chaves, identidades, endpoints, regiões e deployments. Salvar configuração não autentica; dados de uso desconhecidos continuam Not reported. Subscription Only exclui APIs do roteamento/fallback, mesmo quando já cadastradas.

O protótipo usa `localStorage` para sessões/rascunhos e preferências no mesmo navegador; segredos reais não pertencem a esse armazenamento. A apresentação normal usa linguagem de produto, mantendo valores desconhecidos como desconhecidos e controles de fixtures restritos aos testes. Isso não implementa OAuth, execução ou durabilidade de M1. A implementação Desktop deve conectar os padrões ao Core e ao armazenamento previstos, seguindo [OX-012/OX-015/OX-016](../../docs/DEVELOPMENT_BACKLOG.md). A aprovação do estilo não muda a ordem nem substitui os gates de uso e de viabilidade de runtime; verificações da composição anterior são históricas.
