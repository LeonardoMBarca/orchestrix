# Pesquisa de modernização visual do site Orchestrix

**Data da pesquisa:** 9 de outubro de 2026. **Status atualizado em 10/10:** pesquisa documentada, cinco incrementos iniciais e revisões solicitadas pelo responsável incorporados ao site local; direção visual aprovada e consolidada em [design-system/](../../design-system/README.md). A rodada de logos transparentes e estrelas com inércia aprovou 15/15 cenários em 36,2 segundos na porta 4178 e permanece evidência histórica. A identidade foi aplicada ao app e a revisão posterior de idiomas está registrada na [experiência progressiva](../design/d1-progressive-experience.md), que distingue os resultados de cada rodada.

A modernização pode tornar a orquestração visível: um pedido entra pelo chat, agentes recebem trabalhos e um resultado chega à revisão da pessoa. A prioridade proposta é combinar uma abertura com assinatura da marca, uma demonstração curta do produto e movimento que explique essa coordenação. O site apresenta a experiência; o aplicativo mantém o foco no trabalho diário.

Esta pesquisa complementa o [registro da experiência progressiva](../design/d1-progressive-experience.md) e a [galeria da marca](../design/brand/direction-01-review/index.html). Não altera o plano de implementação, o estado do piloto D1 ou os limites dos adapters.

## Referências e evidência

Foram consultadas páginas oficiais e documentação de design. As primeiras dobras de Xtory, Anthropic, Vercel e Linear foram renderizadas em Chrome headless, sem autenticação, em 1440×960 px e examinadas visualmente. Xtory e Anthropic também foram capturadas em 390×844 px. Essas capturas mostram composição em um instante; não comprovam o comportamento de animações ou jornadas completas. A captura da Anthropic contém um aviso de cookies que cobre parte do conteúdo inferior.

As seis capturas e o relatório de renderização estão em `prototypes/desktop/artifacts/reference-*.png` e `reference-sites-report.json`, ignorados pelo Git. OpenAI/ChatGPT, Claude Code, Cursor e Raycast foram consultados pelo conteúdo público nesta rodada. O acesso HTTP direto à homepage da OpenAI retornou 403; o conteúdo das páginas oficiais estava disponível pela ferramenta web. Não foi feita medição de desempenho dos fornecedores. “Vertex” foi descartado após esclarecimento do responsável: a referência pretendida era Vercel.

| Referência oficial | O que foi observado ou documentado | Ideia que podemos adaptar |
| --- | --- | --- |
| [Xtory](https://xtory.ai/) | A captura apresenta abertura escura, título de grande escala, ambientação com pontos e CTA destacado. O conteúdo organiza a apresentação entre necessidade, agentes especializados e arquitetura em camadas. | Dar mais força à primeira dobra e explicar coordenação por uma história visual. Usar formas da marca Orchestrix para a ambientação. |
| [Anthropic](https://www.anthropic.com/) | A captura combina título sem serifa, texto de apoio com serifa, espaço amplo e composição editorial. | Ritmo, respiro e contraste tipográfico. A identidade do Orchestrix continua azul; a escolha de fontes será própria. |
| [Vercel](https://vercel.com/) | A captura coloca um símbolo triangular grande no centro, com proposta e ações ao redor. | Fazer o emblema Orchestrix participar da composição principal, com profundidade e luz localizadas. |
| [OpenAI](https://openai.com/) e [ChatGPT](https://chatgpt.com/overview/) | O conteúdo da homepage aproxima a entrada de uma conversa; a apresentação do ChatGPT organiza usos e caminhos para experimentar ou baixar. | Linguagem familiar, convite para começar e exemplos concretos de uso. |
| [Claude Code](https://claude.com/product/claude-code) | O conteúdo mostra conversas sobre problemas de código, atividade, arquivos alterados e formas de começar. | Mostrar um pedido e seu resultado, mantendo o chat como a experiência compreensível para quem chega. |
| [Linear](https://linear.app/) | A captura destaca o título e uma cena ampla do produto. O conteúdo progride por etapas de trabalho. | Mostrar a interface com espaço para leitura e construir uma progressão entre objetivo, trabalho e revisão. |
| [Cursor](https://cursor.com/) | O conteúdo identifica demonstrações interativas das interfaces e estados de trabalho em andamento ou pronto para revisão. | Permitir explorar pequenas cenas da experiência, com estados e próximos passos claros. |
| [Raycast](https://www.raycast.com/) | O conteúdo apresenta ferramentas por categoria e uma conversa com etapas de execução. | Exemplos familiares e interações curtas que mostrem o que a ferramenta faz. |

As [diretrizes da Vercel](https://vercel.com/design/guidelines) oferecem orientação documentada sobre movimento, foco e consistência de tonalidade. Sua [escala tipográfica](https://vercel.com/geist/typography) distingue títulos, texto corrido e rótulos. São referências de organização, sem exigir a fonte ou a tecnologia do fornecedor.

O [relato da atualização visual do Linear](https://linear.app/now/behind-the-latest-design-refresh), publicado em 12 de março de 2026, descreve menor competição da navegação, divisórias mais suaves e alinhamento dos tokens entre interface e Figma. Isso apoia nossa separação entre apresentação expressiva do site e conforto de uso do aplicativo.

## Direção recomendada para o Orchestrix

As recomendações abaixo registram a proposta original da pesquisa. CTA para experimentar o protótipo, avisos de simulação e botão de pausa da ambientação pertencem àquela proposta; a revisão autorizada após sua avaliação, registrada adiante, substitui essas escolhas na apresentação pública.

### Abertura com assinatura da marca

Manter a base navy aprovada e trabalhar uma composição mais marcante: título curto, emblema com presença e uma cena legível do produto. Luz índigo pode destacar a região que conduz o olhar; bordas, sombras e textos secundários seguem a família azul. A curva que atravessa o símbolo pode orientar linhas e transições recorrentes.

Uma frase proposta para teste foi **“Uma ideia. Seus agentes. Trabalho coordenado.”**, seguida de uma explicação direta sobre desenvolver pelo chat e revisar o resultado. Era uma proposta de texto, sujeita à avaliação junto da composição. Na proposta original, o botão principal era **Experimentar interface** enquanto o protótipo fosse o único caminho disponível; essa ação foi substituída na revisão posterior.

A geometria do símbolo precisa permanecer reconhecível. Para animação vetorial, preparar um SVG fiel e validado a partir da direção aprovada; os PNGs atuais continuam como referência. A presença do emblema, inspirada na composição observada da Vercel, deve conduzir ao produto e à ação principal.

### Demonstração que começa por um pedido

Trocar a ilustração estática por uma demonstração breve, identificada como **Fluxo ilustrativo**, com três pedidos selecionáveis: corrigir um problema, criar uma funcionalidade e entender código. Cada pedido abre uma sequência simples:

1. A pessoa descreve o objetivo no chat.
2. O Orchestrix organiza contexto e trabalho.
3. Agentes participam de desenvolvimento e revisão.
4. Um resultado aparece para a pessoa conferir e pedir ajustes.

A cena pode combinar mensagem, conexões entre agentes e um pequeno resultado/diff. Controles de avançar, voltar e repetir deixam o visitante explorar em seu ritmo. Isso é uma demonstração local da proposta; não conecta contas, não executa agentes nem apresenta resultados ilustrativos como evidência real.

### Movimento que torne a coordenação compreensível

Linhas índigo saem do pedido, passam pelos agentes envolvidos e convergem no resultado. Um pulso curto destaca a etapa selecionada. Os mesmos traços e curvas podem reaparecer em divisores de seção, dando uma assinatura recorrente ao site.

Preferir SVG/CSS para essa cena, com transformações e opacidade quando adequadas. Movimento reduzido deve conservar as etapas e informações em uma representação estática; loops contínuos terão pausa. Animações deixam de consumir trabalho quando a cena sai da tela. Essas escolhas seguem a orientação de movimento das [diretrizes da Vercel](https://vercel.com/design/guidelines), com execução própria para o Orchestrix.

### Uma transição visível entre chat e Studio

Uma seção **“Comece pelo chat. Aprofunde quando quiser.”** mostra a mesma conversa em dois níveis de detalhe. Na visão essencial, pedido e resultado dominam. Ao escolher Studio, aparecem contexto, decisões de execução e revisão. O objetivo é explicar que o aplicativo acompanha a pessoa sem exigir configuração de tudo para começar.

O controle deve funcionar por clique, toque e teclado. No celular, as cenas seguem leitura vertical normal. A rolagem pode revelar elementos discretamente, preservando a posição e o controle do navegador. A experiência diária do aplicativo conserva ações rápidas; as cenas expressivas pertencem à apresentação.

### Ritmo editorial e acabamento

Alternar uma cena ampla do produto, uma explicação curta e uma composição de agentes. Reservar cartões para conteúdos comparáveis, como plataformas ou capacidades; evitar que toda seção tenha a mesma estrutura de três caixas. A tipografia precisa dar mais espaço à leitura, incluindo textos secundários relevantes.

Como ponto inicial para experimentar, usar corpo de 16–18 px no site, títulos responsivos e rótulos menores apenas onde são auxiliares. No aplicativo, densidade e hierarquia seguem suas necessidades de uso. A divisão de papéis tipográficos é coerente com a [documentação Geist](https://vercel.com/geist/typography); os valores propostos são nossos, não uma exigência do fornecedor.

Estados de foco, hover e toque compartilham a mesma linguagem de borda e luz. Um pequeno destaque responde à ação da pessoa; a página mantém legibilidade durante as transições. Fontes, formas e componentes dos fornecedores não serão copiados.

## Sequência proposta para a página

**Abertura e promessa → pedido demonstrado → coordenação dos agentes → chat e Studio → revisão e controle → disponibilidade e GitHub.**

Essa ordem aproxima a pessoa da experiência antes de explicar a configuração avançada. Múltiplas contas, escolha de modelo, raciocínio e contexto aparecem em situações compreensíveis. Os estados de integração e os downloads continuam refletindo o que está efetivamente disponível. O efeito desejado é que o visitante entenda rapidamente o produto e queira experimentar o fluxo.

| Prioridade proposta | Incremento | O que avaliar |
| --- | --- | --- |
| Primeiro | Abertura com assinatura da marca e demonstração guiada | O visitante entende para que serve e encontra como experimentar? A demonstração permanece legível? |
| Segundo | Conexões animadas e comparação Chat/Studio | A coordenação fica clara? A pessoa percebe que os controles avançados são opcionais? |
| Terceiro | Ritmo das demais seções e acabamento das interações | A página mantém personalidade e conforto no celular, teclado e movimento reduzido? |
| Após validar | Consolidar o sistema visual | Site e aplicativo derivam valores comuns, com diferenças explícitas por superfície e tema? |

Essa tabela registra a ordem proposta na pesquisa. Após a autorização do responsável, os três grupos de incrementos visuais foram implementados no protótipo: abertura com emblema e ambiente de partículas, três exemplos navegáveis, conexões por etapa, comparação Chat/Studio e revisão editorial das seções. Os exemplos continuam ilustrativos; a implementação não entrega integrações reais nem altera a ordem dos marcos de desenvolvimento. A consolidação do sistema visual aguarda a avaliação dessa direção.

## Revisão autorizada após avaliar a implementação

**Registro histórico da revisão em 4177, seguido da direção atual de transparência.** O responsável refinou a direção com onze pontos. A nova apresentação manteve navy/índigo e a logo aprovada, acrescentando uma paisagem de voo noturno entre montanhas rochosas. A referência de outra direção de marca inspira apenas essa ambientação. A paisagem foi gerada para o site; estrelas, luz do ponteiro e conectores são camadas de código. [Asset e prompt da paisagem](../../prototypes/desktop/assets/visuals/README.md).

As ações públicas passam a ser **Baixar Orchestrix**, que leva à disponibilidade por plataforma, e **Veja o Orchestrix em ação**, que abre uma página preparada para o vídeo futuro. O convite para experimentar o protótipo e os avisos sobre a simulação foram retirados do site. **Em preparação** continua sendo o estado dos instaladores; não foram criados binários ou links fictícios. `watch.html` recebe mídia por `video-config.json`, com poster, legendas e transcrição opcionais; enquanto não há vídeo configurado, sua composição permanece legível.

A navegação recebe tratamento mais expressivo para Como funciona, A experiência, Download e GitHub. Os links ao GitHub usam a [marca oficial armazenada localmente](../../prototypes/desktop/assets/icons/README.md). A orientação daquela rodada conservava o fundo e a borda preta circular das logos central e do topo; o favicon usava transparência, sem bloco quadrado. Essa decisão foi substituída posteriormente por logos sem fundo, mantendo a geometria aprovada como referência.

O campo de estrelas conserva deriva lenta, reage discretamente à rolagem e responde ao ponteiro com profundidade e luz. Os conectores da abertura encontram a lateral vertical dos cartões e acompanham seu movimento. O controle de pausa da ambientação foi removido conforme a preferência do responsável; movimento reduzido e suspensão com aba oculta continuam previstos na implementação. A reprodução opcional da demonstração mantém controle próprio. A alteração é uma decisão de produto do Orchestrix; não muda o registro das orientações encontradas nas referências externas.

O conteúdo amplia os exemplos para seis modos de ação: **Development, Research, Documentation, Presentations, Refactoring e Improvement Review**. Uma seção própria explica a configuração dos agentes executores, distinguindo seus perfis da finalidade do modo. O site também apresenta **uma conta ou várias** e estratégias **Efficiency, Balanced, Performance e Custom**. [Modos/perfis](../design/action-modes-and-worker-profiles.md) e [routing](../design/routing-strategies.md) documentam requisitos e integração aos contratos existentes; a apresentação não comprova busca científica, geração PPTX/PDF, autenticação ou execução real pelo Core.

A rodada histórica agregada dessa revisão, no servidor 4177, aprovou **15/15 cenários em 36,5 segundos**; verificação de sintaxe e whitespace também passaram. O QA daquela rodada renderizou 12 capturas, verificou Chat/Studio em seis larguras sem overflow e não registrou erros de página/requisições falhas/HTTP ≥400. Dez pares selecionados de contraste ficaram entre 4,89:1 e 17,64:1, sem certificação completa de acessibilidade. O [registro histórico de implementação e validação](../design/d1-progressive-experience.md#revisão-do-site--onze-pontos) detalha os testes de movimento, conectores, favicon, vídeo e suas limitações. A suíte integral do app e a pesquisa original não foram repetidas nessa rodada. Não houve publicação do site ou entrega de instaladores; os marcos e o piloto D1 permanecem inalterados.

## Revisão atual: logos sem fundo e estrelas com inércia

A orientação seguinte do responsável remove o fundo das logos. A variante padrão [symbol-indigo.png](../../prototypes/desktop/assets/brand/transparent/symbol-indigo.png) usa o gradiente de `main-logo-dark.png`; variantes branca e grafite atendem às demais superfícies do app. Os três PNGs de 1254×1254 têm alpha zero nos quatro cantos e na região vazada amostrada, permitindo que o fundo da página apareça dentro da marca. Cabeçalho e emblema não usam círculo preto ou recorte ampliado em CSS. O favicon usa diretamente a variante índigo, com ocupação aproximada de 86%/83% da largura/altura da imagem, contra 59%/55% antes, considerando pixels com alpha ≥8. Site, app e vídeo têm título **Orchestrix**. [Galeria e variantes](../../prototypes/desktop/assets/brand/transparent/index.html) · [prompts registrados](../../prototypes/desktop/assets/brand/transparent/prompts.json).

A frase de abertura relaciona a direção do trabalho à experiência de um piloto em um voo noturno sobre as montanhas. As estrelas respondem ao ponteiro com deslocamento próprio, inércia e retorno por mola; a influência entra e sai gradualmente. O canvas tem alvos de 30 fps em repouso e 60 fps durante mouse/rolagem, enquanto os conectores mantêm alvo de 30 fps. Esses valores são escolhas de implementação e foram observados com relógio controlado, sem benchmark físico. Redimensionamento conserva o campo, e movimento reduzido e suspensão com aba oculta permanecem preservados.

A prévia atual usa [4178/website.html](http://127.0.0.1:4178/website.html), [4178/watch.html](http://127.0.0.1:4178/watch.html) e a [galeria transparente](http://127.0.0.1:4178/assets/brand/transparent/index.html). A nova rodada agregada aprovou **15/15 cenários em 36,2 segundos**. O QA renderizou 15 capturas, incluindo galeria e aplicativo em Studio/Institutional; as 12 combinações Chat/Studio em seis larguras continuaram sem overflow ou erros de página/requisição. A marca foi examinada nas composições atuais sem artefatos observados. [Evidências e limites da revisão atual](../design/d1-progressive-experience.md#revisão-atual--transparência-e-movimento). Os resultados anteriores seguem históricos, sem certificação completa de acessibilidade ou entrega de novas capacidades do Core.

## Pasta de padronização consolidada — 10/10

A direção foi aprovada pelo responsável. A pasta [design-system/](../../design-system/README.md), na raiz do repositório, agora reúne regras reutilizáveis e referências às fontes consumidas pelo código. O [catálogo dos assets](../design/visual-asset-catalog.md) continua o índice de imagens e capturas. A estrutura criada concretiza a proposta desta pesquisa:

```text
design-system/
  README.md
  brand/          # Variantes transparentes e aplicações da marca
  tokens/         # Referências às fontes de cor, tipo, espaço e raio
  components/     # Estados e contratos dos componentes existentes
  motion/         # Efeitos atuais, gatilhos e movimento reduzido
  patterns/       # Site, entrada pelo chat, revisão e Studio
  assets/         # Localização, procedência e registro de imagens
  examples/       # Páginas, capturas e verificações de referência
  decisions.md    # Direção aprovada e como evoluí-la
```

Os tokens de marca já são comuns em [identity.css](../../prototypes/desktop/identity.css), consumido pelo site e pelo Studio. Tokens semânticos descrevem fundo, superfície, texto, ação e estado; o site e os temas aplicam seus papéis. A pasta aponta para essas fontes, preservando uma fonte única de valores no código. Não foi criada uma segunda paleta ou um gerador novo nesta consolidação. Os exemplos permitem revisar uma alteração comum nas superfícies afetadas; a migração para produção segue o plano existente.

A adoção de tokens compartilhados foi informada pelo [processo descrito pelo Linear](https://linear.app/now/behind-the-latest-design-refresh) na pesquisa original. Para o Orchestrix, uma alteração de base deve atualizar sua fonte, guias e exemplos correspondentes, preservando as diferenças intencionais entre site e aplicativo.
