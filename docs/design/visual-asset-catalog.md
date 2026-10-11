# Catálogo textual de imagens do Orchestrix

Atualizado em **11 de outubro de 2026**. Este é o índice para encontrar imagens pelo assunto, aparência e uso, sem abrir cada arquivo. A coleção mapeada reúne **58 arquivos de imagem versionáveis** e **120 capturas locais de QA e diagnóstico**, incluindo conceitos, variantes transparentes e cinco diagramas de ajuda. A revisão de sessões acrescentou treze capturas e o refinamento Compact/API acrescentou dezoito; o histórico anterior tinha 53 imagens e 77 capturas. Caminhos são relativos a este documento; nomes e aliases podem ser pesquisados com `rg`.

## Como encontrar e interpretar

Procure por termos como **montanhas / mountains**, **voo noturno / night flight**, **logo escura / dark logo**, **sem fundo / transparent**, **nós / nodes**, **carvão / charcoal**, **chat**, **revisão / review**, **ajuda / help**, **sessões / sessions**, **painéis / docking**, **configurações / settings** ou **Medieval**. Os identificadores `B`, `V`, `I`, `D`, `S` e `H` são estáveis dentro deste catálogo.

- **Protótipo:** arquivo selecionado para o estudo local. Sua presença em `assets/` não equivale a exportação final para produção.
- **Conceito/fonte:** imagem guardada para avaliação ou reutilização; a referência da Direção 01 foi aprovada como direção visual, mas os rasters ainda precisam da preparação final de marca.
- **Evidência histórica:** screenshot de uma interface e fixtures em uma etapa específica; não é arte gerada nem recurso para compor o produto.
- **Evidência local:** screenshot regenerável em `artifacts/`, ignorado pelo Git. Pode ser atualizado ou não existir em outro checkout.
- **Ícone oficial:** imagem externa com procedência registrada; o uso respeita a identidade de seu proprietário.

A escolha atual usa **símbolos sem fundo**, com transparência real no entorno e nos vazios. **Indigo** é a principal no site, vídeo, app escuro e favicon; **Graphite** é usada no app claro; **White** fica disponível como alternativa. A preferência anterior por centro/borda pretos e recorte circular é histórica: foi substituída pela revisão seguinte do responsável. Os rasters de referência permanecem como fontes, sem serem sobrescritos.

## Ativos disponíveis para a interface

Os sete PNGs originais de marca são cópias idênticas dos conceitos indicados na coluna Fonte, verificadas por SHA-256. Sua origem é geração raster com `image_gen` a partir da prancha enviada pelo usuário. Os [prompts da Direção 01](brand/direction-01-review/prompts.json) e o [README dos ativos de marca](../../prototypes/desktop/assets/brand/README.md) registram procedência e limitações.

| ID / arquivo | Descrição visual e aplicação | Aliases de busca | Estado / fonte |
| --- | --- | --- | --- |
| B01 · [main-logo-light.png](../../prototypes/desktop/assets/brand/main-logo-light.png) | Assinatura horizontal: símbolo grafite com faixa índigo, nome Orchestrix e assinatura pequena sobre superfície clara. | logo principal clara; light wordmark; full lockup | Disponível no protótipo; fonte D01. |
| B02 · [main-logo-dark.png](../../prototypes/desktop/assets/brand/main-logo-dark.png) | Assinatura horizontal em fundo grafite, arcos claros/lavanda, faixa índigo e nome branco. | logo principal escura; dark wordmark; full lockup | Disponível no protótipo; fonte D02. |
| B03 · [symbol-light.png](../../prototypes/desktop/assets/brand/symbol-light.png) | Emblema circular grafite atravessado por uma faixa índigo ondulada; superfície clara e margens. | símbolo claro; light emblem; circular mark | Fonte raster preservada; uso anterior no app claro substituído por B11; fonte D03. |
| B04 · [symbol-dark.png](../../prototypes/desktop/assets/brand/symbol-dark.png) | Emblema de arcos claros e faixa índigo sobre grafite. A apresentação anterior no site usava recorte circular e centro/borda pretos. | símbolo escuro; dark emblem; black edge; borda preta; previous hero logo | Fonte raster preservada; uso anterior em site/vídeo/app substituído por B09; fonte D04. |
| B05 · [app-icon-light.png](../../prototypes/desktop/assets/brand/app-icon-light.png) | Símbolo escuro sobre um cartão claro de cantos arredondados, com sombra. | ícone aplicativo claro; light app tile | Variante disponível; não é o favicon atual; fonte D05. |
| B06 · [app-icon-dark.png](../../prototypes/desktop/assets/brand/app-icon-dark.png) | Símbolo claro/índigo sobre cartão carvão arredondado, com gradiente discreto e sombra. | ícone aplicativo escuro; dark app tile | Variante disponível; favicon histórico, depois B08 e atualmente B09; fonte D06. |
| B07 · [orchestration-banner.png](../../prototypes/desktop/assets/brand/orchestration-banner.png) | Banner panorâmico grafite: papéis Coder, Reviewer, Tester e Documenter convergem para o emblema; texto à esquerda e código ao fundo. | múltiplos agentes; orchestration banner; converging roles | Composição disponível, sem uso atual nas páginas; fonte D23. |
| B08 · [favicon-transparent.png](../../prototypes/desktop/assets/brand/favicon-transparent.png) | Primeira variante clara/índigo com alpha no fundo e vazios; mantém margens amplas da referência. PNG de 1254 × 1254. | previous favicon; favicon anterior; sem fundo; transparent symbol; alpha | Variante histórica, substituída no favicon por B09 para maior ocupação em tamanho pequeno. Edição de B04 com `image_gen`; [prompt e procedência](../../prototypes/desktop/assets/brand/README.md). |
| B09 · [transparent/symbol-indigo.png](../../prototypes/desktop/assets/brand/transparent/symbol-indigo.png) | Emblema sem fundo: arco superior lavanda/índigo em gradiente, inferior branco e faixa ondulada índigo. Enquadramento próximo do símbolo. | principal transparente; indigo gradient; transparent logo; hero emblem; favicon; sem fundo | Atual no topo, centro e outras marcas do site/vídeo, app escuro e favicon das três páginas. Extração de B02 com `image_gen`; [prompt](../../prototypes/desktop/assets/brand/transparent/prompts.json). |
| B10 · [transparent/symbol-white.png](../../prototypes/desktop/assets/brand/transparent/symbol-white.png) | Emblema sem fundo com arcos superior e inferior brancos e faixa índigo. Enquadramento próximo do símbolo. | white transparent; símbolo branco; light arcs; dark surface alternative | Alternativa disponível, exibida na galeria; sem uso nas páginas principais. Extração de B04 com `image_gen`; [prompt](../../prototypes/desktop/assets/brand/transparent/prompts.json). |
| B11 · [transparent/symbol-graphite.png](../../prototypes/desktop/assets/brand/transparent/symbol-graphite.png) | Emblema sem fundo com arcos grafite e faixa índigo. O grafite faz parte do símbolo, sem disco ou borda externa adicionada. | graphite transparent; símbolo grafite; light theme; clear surface; sem fundo | Atual no app em Institutional, Atelier e Dawn. Extração de B03 com `image_gen`; [prompt](../../prototypes/desktop/assets/brand/transparent/prompts.json). |
| V01 · [night-flight.png](../../prototypes/desktop/assets/visuals/night-flight.png) | Vale de montanhas rochosas em azul profundo, visto do ar; cumes facetados, neblina discreta e céu aberto escuro. PNG de 1672 × 941. | voo noturno; night flight; rocky mountains; navy valley; fundo; background | Geração original com `image_gen`; fundo do site e página de vídeo, além de poster padrão. [Prompt e procedência](../../prototypes/desktop/assets/visuals/README.md). |
| I01 · [github-mark-white.svg](../../prototypes/desktop/assets/icons/github-mark-white.svg) | Silhueta branca oficial do Invertocat, sem fundo, para links identificados como GitHub. | GitHub logo; ícone GitHub; white Invertocat; repository | Ícone oficial baixado, sem alteração; usado no site e vídeo. [Fonte e orientação](../../prototypes/desktop/assets/icons/README.md). |

As variantes B09–B11 foram geradas pela ferramenta integrada `image_gen`, têm **1254 × 1254**, alpha 0 nos cantos e em uma amostra do vazio interno, e foram incorporadas sem edição raster posterior. A largura visível medida com alpha ≥8 é de **86,04% em Indigo**, **84,45% em White** e **82,78% em Graphite**, contra **58,77% no favicon anterior B08**. São medidas de enquadramento, não certificação de fidelidade geométrica. A [galeria transparente](../../prototypes/desktop/assets/brand/transparent/index.html) permite comparar as três em superfícies navy e claras, além do favicon em 16 px; os [prompts](../../prototypes/desktop/assets/brand/transparent/prompts.json) registram cada alvo de edição.

Estrelas, brilho, conectores, bolinhas e formas da interface atual são desenhados por **CSS, SVG embutido e canvas**, não por outros PNGs. A animação do fundo não é incorporada a V01. O vídeo do produto ainda não foi fornecido; [configuração e pasta de mídia](../../prototypes/desktop/assets/video/README.md) não representam uma imagem ou filme existente.

## Biblioteca de conceitos da Direção 01

Todos os 23 arquivos abaixo são **conceitos raster gerados** com `image_gen`, não recortes pixel-exatos da prancha. Estão preservados em `docs/design/brand/direction-01-review/images/`. A [galeria](brand/direction-01-review/index.html), o [registro da direção](brand/direction-01-review/README.md) e os [prompts individuais](brand/direction-01-review/prompts.json) acompanham a coleção. Arquivos sem cópia em `assets/` continuam encontráveis para uso futuro; sua presença não instrui a colocá-los automaticamente na interface.

| ID / arquivo | Conteúdo e uso possível | Aliases | Estado |
| --- | --- | --- | --- |
| D01 · [01-main-logo-light.png](brand/direction-01-review/images/01-main-logo-light.png) | Logo completa sobre claro, com assinatura “ORQUESTRE · DESENVOLVA · MAIS LONGE”. | light logo; logo clara; wordmark | Conceito/fonte de B01. |
| D02 · [02-main-logo-dark.png](brand/direction-01-review/images/02-main-logo-dark.png) | Logo completa branca/lavanda e índigo sobre grafite. | dark logo; logo escura; wordmark | Conceito/fonte de B02; origem de B09. |
| D03 · [03-symbol-light.png](brand/direction-01-review/images/03-symbol-light.png) | Símbolo grafite/índigo isolado em fundo claro. | light symbol; emblema claro | Conceito/fonte de B03; origem de B11. |
| D04 · [04-symbol-dark.png](brand/direction-01-review/images/04-symbol-dark.png) | Símbolo claro/índigo isolado em fundo escuro. | dark symbol; emblema escuro | Conceito/fonte de B04; origem de B08 e B10. |
| D05 · [05-app-icon-light.png](brand/direction-01-review/images/05-app-icon-light.png) | Cartão claro arredondado com emblema e sombra. | light app icon; tile; ícone claro | Conceito/fonte de B05. |
| D06 · [06-app-icon-dark.png](brand/direction-01-review/images/06-app-icon-dark.png) | Cartão carvão arredondado com emblema claro. | dark app icon; tile; ícone escuro | Conceito/fonte de B06. |
| D07 · [07-color-deep-graphite.png](brand/direction-01-review/images/07-color-deep-graphite.png) | Cartão de cor grafite profundo com código e função. Valor de referência `#0B0F14`. | deep graphite; grafite profundo; base | Conceito de paleta. |
| D08 · [08-color-charcoal.png](brand/direction-01-review/images/08-color-charcoal.png) | Cartão de cor carvão para superfície e hierarquia. Valor `#1A1F29`. | charcoal; cinza carvão; surface | Conceito de paleta. |
| D09 · [09-color-electric-indigo.png](brand/direction-01-review/images/09-color-electric-indigo.png) | Cartão índigo para acento e interação. Valor `#6366F1`. | electric indigo; índigo elétrico; accent | Conceito de paleta. |
| D10 · [10-color-neutral-gray.png](brand/direction-01-review/images/10-color-neutral-gray.png) | Cartão cinza neutro para texto e bordas sutis. Valor `#E5E7EB`. | neutral gray; cinza neutro; border | Conceito de paleta. |
| D11 · [11-color-technical-white.png](brand/direction-01-review/images/11-color-technical-white.png) | Cartão branco técnico com contorno leve. Valor `#FAFAFC`. | technical white; branco técnico; clarity | Conceito de paleta. |
| D12 · [12-typography.png](brand/direction-01-review/images/12-typography.png) | Amostra “Aa” e orientação de sans geométrica, moderna e técnica. Não identifica uma família final. | typography; tipografia; geometric sans | Conceito tipográfico. |
| D13 · [13-background-pattern.png](brand/direction-01-review/images/13-background-pattern.png) | Grade regular de pequenos pontos cinza sobre claro; estrutura discreta. | dot grid; padrão de fundo; pontilhado; background | Conceito reutilizável de textura. |
| D14 · [14-modular-nodes.png](brand/direction-01-review/images/14-modular-nodes.png) | Nós circulares ligados por caminhos finos ortogonais; centro índigo e contornos claros/pretos. | modular nodes; módulos; nós; coordination | Conceito reutilizável de coordenação. |
| D15 · [15-flow-lines.png](brand/direction-01-review/images/15-flow-lines.png) | Linhas paralelas finas e curvas suaves com terminais circulares; acentos índigo e cinza. | flow lines; linhas de fluxo; connectors | Conceito reutilizável de fluxo. |
| D16 · [16-interface-shapes.png](brand/direction-01-review/images/16-interface-shapes.png) | Quadrado arredondado, círculo e cápsula sobre claro. | interface shapes; formas; circle; pill | Conceito reutilizável de formas. |
| D17 · [17-loading-empty-state.png](brand/direction-01-review/images/17-loading-empty-state.png) | Anel de progresso cinza claro com arco curto índigo/lavanda. | loading; estado vazio; progress ring; empty state | Conceito reutilizável de estado. |
| D18 · [18-watermark-emblem.png](brand/direction-01-review/images/18-watermark-emblem.png) | Emblema em cinza muito claro e baixo contraste. | watermark; marca d’água; subtle emblem | Conceito reutilizável de presença sutil. |
| D19 · [19-brand-concept.png](brand/direction-01-review/images/19-brand-concept.png) | Cartão textual sobre plataforma desktop local-first, coordenação, precisão e controle. | brand concept; conceito da marca; positioning | Conceito editorial; texto deve ser HTML nas páginas. |
| D20 · [20-application-light.png](brand/direction-01-review/images/20-application-light.png) | Aplicação compacta da assinatura em cartão claro com contorno discreto. | light application; aplicação clara; compact lockup | Conceito de aplicação. |
| D21 · [21-application-dark.png](brand/direction-01-review/images/21-application-dark.png) | Aplicação compacta da assinatura branca/índigo em cartão escuro. | dark application; aplicação escura; compact lockup | Conceito de aplicação. |
| D22 · [22-application-monochrome.png](brand/direction-01-review/images/22-application-monochrome.png) | Assinatura em preto e tons neutros, sem índigo. | monochrome; monocromática; grayscale | Conceito para aplicações sem cor. |
| D23 · [23-orchestration-banner.png](brand/direction-01-review/images/23-orchestration-banner.png) | Banner de quatro papéis convergindo para a marca; “Múltiplos agentes. Um mesmo propósito.” | banner; multiple agents; Coder; Reviewer; Tester; Documenter | Conceito/fonte de B07. |

Os hexadecimais acima vêm da referência documentada; amostras raster têm pequenas variações de geração e não substituem tokens de cor.

## Screenshots versionáveis: evidência de interface

São **17 capturas históricas**, obtidas do protótipo com dados de estudo. O [registro D1](d1-delivery.md), a [direção de chat](d1-chat-direction.md) e a [documentação do protótipo](../../prototypes/desktop/README.md) contextualizam os fluxos. Estas capturas não mostram agentes reais sendo executados e não fixam a aparência atual do site.

| ID / arquivo | Assunto para localizar | Aliases / etapa |
| --- | --- | --- |
| S01 · [d1-entry.png](d1-entry.png) | Entrada guiada e preparação de projeto. | onboarding; entrada; project setup; D1 |
| S02 · [d1-context.png](d1-context.png) | Inspeção de contexto do trabalho. | context; contexto; manifest; D1 |
| S03 · [d1-connection.png](d1-connection.png) | Conexão e autorização no fluxo inicial. | account; connection; conexão; assinatura; D1 |
| S04 · [d1-chat.png](d1-chat.png) | Projetos, seleção de conversa e pedido livre. | project chats; projetos; conversa; D1 |
| S05 · [d1-chat-review.png](d1-chat-review.png) | Revisão do resultado de um pedido pela conversa. | chat review; revisão; resultado; D1 |
| S06 · [d1-chat-compact.png](d1-chat-compact.png) | Conversa em janela compacta. | compact chat; chat compacto; responsive; D1 |
| S07 · [d1-chat-standalone.png](d1-chat-standalone.png) | Entrada de conversa avulsa, sem repositório. | standalone; avulsa; no repository; D1 |
| S08 · [studio.png](../research/previews/studio.png) | Workspace Studio histórico, grafite e índigo; tema padrão. | Studio; default theme; workspace; tema padrão |
| S09 · [atelier.png](../research/previews/atelier.png) | Tema claro de papel, terracota e títulos editoriais. | Atelier; paper; terracotta; tema claro |
| S10 · [horizon.png](../research/previews/horizon.png) | Tema de azul profundo, acento azul claro e superfícies amplas. | Horizon; blue; azul; rounded |
| S11 · [deep-black.png](../research/previews/deep-black.png) | Tema de preto profundo e acento neutro. | Deep Black; preto profundo; neutral |
| S12 · [medieval.png](../research/previews/medieval.png) | Tema de madeira escura, ouro antigo e títulos clássicos. | Medieval; gold; ouro; wood; madeira |
| S13 · [forest.png](../research/previews/forest.png) | Tema de verde profundo e menta. | Forest; green; verde; mint |
| S14 · [dawn.png](../research/previews/dawn.png) | Tema de luz quente e coral. | Dawn; warm; coral; amanhecer |
| S15 · [review.png](../research/previews/review.png) | Tela de revisão de trabalho no workspace. | review; revisão; artifact |
| S16 · [compact.png](../research/previews/compact.png) | Revisão em janela compacta. | compact review; revisão compacta; reflow |
| S17 · [appearance.png](../research/previews/appearance.png) | Galeria de seleção em Preferências → Aparência. | appearance; themes; personalizar aparência; settings |

Institutional está disponível no protótipo e tem captura local no grupo abaixo; não há PNG histórico correspondente em `docs/research/previews/`.

## Capturas locais de pesquisa e QA

Os arquivos ficam em [prototypes/desktop/artifacts/](../../prototypes/desktop/artifacts/), **ignorado pelo Git**. Este mapa registra os nomes existentes nesta revisão; não transforma capturas em biblioteca de fundos, em aprovação final ou em arquivos garantidos para novos checkouts. Relatórios e helpers ficam nessa mesma pasta. Quando uma evidência precisar permanecer com uma entrega, selecione a captura, registre data/build no documento responsável e mantenha uma cópia versionável apropriada.

| Família / aliases | Nomes existentes e conteúdo |
| --- | --- |
| Temas e revisão · themes, appearance, workspace | `studio.png`, `atelier.png`, `horizon.png`, `deep-black.png`, `medieval.png`, `forest.png`, `dawn.png`, `institutional.png`: workspaces por tema. `appearance.png`: galeria. `review.png`, `compact.png`: revisão ampla/compacta. |
| Entrada D1 · onboarding, context, connection | `d1-entry.png`, `d1-context.png`, `d1-connection.png`: entrada, manifesto de contexto e conexão. |
| Chat D1 · conversations, projects, request, result, review | `chat-projects-wide.png`: seleção de projetos/conversas. `chat-request-wide.png`, `chat-request-compact.png`: pedido. `chat-result-wide.png`, `chat-result-compact.png`: resultado. `chat-review-wide.png`, `chat-review-compact.png`: revisão. `chat-standalone-compact.png`: conversa avulsa compacta. |
| Galeria da marca · brand gallery, auxiliary elements | `brand-direction-01-gallery-desktop-light.png`, `brand-direction-01-gallery-desktop-dark.png`, `brand-direction-01-gallery-mobile-light.png`, `brand-direction-01-gallery-mobile-dark.png`: galeria em duas superfícies e larguras. `brand-direction-01-auxiliary-elements.png`: elementos auxiliares da coleção. |
| Entrada amigável · friendly entry, chat first | `friendly-home-studio.png`, `friendly-home-institutional.png`, `friendly-home-mobile.png`: entrada por chat. `friendly-conversation.png`: conversa. `friendly-studio.png`: controles técnicos. `friendly-website-desktop.png`, `friendly-website-mobile.png`: versão anterior do site. |
| Entrada mobile refinada · mobile first fold, composer, chat field, campo inicial, opções recolhidas, Options, idiomas | Quatro screenshots de página inteira de 10/10, gerados por [check-file-entry.mjs](../../prototypes/desktop/tools/check-file-entry.mjs) pelo HTML direto, no build `896778a77a828b86ed4d12e0e77c82e4154951c10e6fc718d015180901cb727c`: `d1-entry-refinement-desktop-en.png` (PNG 1440×1219, viewport 1440×1000): início Studio em inglês, utilitários abertos; `d1-entry-refinement-mobile-en.png` (PNG 390×1576, viewport 390×844): entrada inglesa, Options recolhido e campo na primeira tela; `d1-entry-refinement-mobile-es.png` (PNG 320×1864, viewport 320×900): mesma entrada em espanhol na menor largura; `d1-entry-refinement-options-en.png` (PNG 390×1732, viewport 390×844): Options aberto, com aparência, idioma e utilitários. Evidência local regenerável, sem geração de arte ou uso como fundo; revisão visual e medidas no [registro D1](./d1-progressive-experience.md). |
| Site navy anterior · blue website, navy baseline | `site-navy-desktop.png`, `site-navy-mobile.png`: revisão azul anterior à modernização. |
| Site modernizado · hero, orchestration demo, Chat, Studio | `site-modern-hero-desktop.png`, `site-modern-hero-mobile.png`: abertura. `site-modern-demo-desktop.png`: fluxo. `site-modern-review-desktop.png`, `site-modern-review-mobile.png`: revisão. `site-modern-chat-desktop.png`: experiência Chat. `site-modern-studio-desktop.png`, `site-modern-studio-mobile.png`: apresentação do Studio. Esses nomes podem ser regenerados com as revisões mais recentes. |
| Revisão noturna · modes, worker profiles, video | `site-revised-modes-desktop.png`: seis modos de ação. `site-revised-profiles-desktop.png`: perfis de agentes executores. `site-revised-video-desktop.png`, `site-revised-video-mobile.png`: página preparada para vídeo. |
| Marca transparente · transparent brand, Indigo, Graphite, gallery | `transparent-brand-gallery.png`: galeria das três variantes sobre navy/claro. `transparent-brand-app-studio.png`: aplicação Indigo em Studio. `transparent-brand-app-institutional.png`: aplicação Graphite em Institutional. |
| Identidade do aplicativo · app identity, navy, indigo, sem montanhas, welcome, conversation, work, connections, appearance | Capturas de 10/10 da aplicação da identidade navy/índigo ao app, sem paisagem: `app-identity-studio-welcome-desktop.png`: início Studio; `app-identity-studio-conversation-desktop.png`: conversa; `app-identity-studio-work-desktop.png`: trabalho; `app-identity-studio-connections-desktop.png`: conexões; `app-identity-studio-appearance-desktop.png`: Aparência; `app-identity-institutional-welcome-desktop.png`: início Institutional para comparação; `app-identity-studio-welcome-mobile.png`: início Studio mobile. Capturas de página inteira, com viewport desktop de 1440 × 1000 e mobile de 390 × 844. |
| Idiomas do aplicativo · localization, i18n, language, English, português brasileiro, español, settings | `localization-app-en-desktop.png`: início em inglês, idioma padrão; `localization-app-pt-BR-desktop.png`: início em português brasileiro; `localization-app-es-desktop.png`: início em espanhol; `localization-app-es-work.png`: tela de trabalho com interface em espanhol; `localization-app-en-settings.png`: configurações em inglês. Capturas de página inteira com viewport de 1440 × 1000. O idioma da interface muda; conteúdo original dos exemplos permanece preservado. |
| Idiomas do site · localization, English website, hero, capabilities, modes, video, mobile | `localization-site-en-hero-desktop.png`: abertura em inglês no viewport de 1440 × 1000; `localization-site-en-modes.png`: seção de capacidades/modos em inglês, capturada pelo elemento `#capabilities` com largura de viewport de 1440; `localization-site-en-hero-mobile.png`: abertura em inglês no viewport de 390 × 844; `localization-site-en-video-mobile.png`: página de vídeo em inglês, captura de página inteira com viewport de 390 × 844. Contexto e medições desta rodada estão em [localization-qa.json](../../prototypes/desktop/artifacts/localization-qa.json). |
| Referências externas · reference websites, visual research | `reference-xtory-desktop.png`, `reference-xtory-mobile.png`, `reference-anthropic-desktop.png`, `reference-anthropic-mobile.png`, `reference-vercel-desktop.png`, `reference-linear-desktop.png`: homepages públicas examinadas na [pesquisa de modernização](../research/site-modernization-2026-10-09.md). São evidências de terceiros, não ativos Orchestrix para reutilizar. |

## Ilustrações da ajuda — revisão de sessões em 10/10

Os cinco SVGs seguintes são diagramas escritos em código, com texto, title/description e paleta navy/índigo. Não são screenshots, dados de contas, nem imagens geradas com IA; não requerem prompt ou procedência externa. A [página de ajuda](../../prototypes/desktop/help.html) os usa em inglês, com alternativas textuais contextualizadas e orientação escrita. O [mapa local](../../prototypes/desktop/assets/help/README.md) reúne manutenção e publicação de vídeos.

| ID / arquivo | Assunto e aliases para busca | Uso / estado | Dimensões |
| --- | --- | --- | --- |
| H01 · [session-tree.svg](../../prototypes/desktop/assets/help/session-tree.svg) | Projeto expansível, contexto compartilhado, duas sessões com histórico próprio e sessão independente associável. Project tree; shared context; independent session; attach to project; histórico; associação. | Capítulo Sessions & projects; ativo no centro de ajuda. | 920 × 440 |
| H02 · [workspace-docking.svg](../../prototypes/desktop/assets/help/workspace-docking.svg) | Três composições: painéis separados, dock comum com abas e escolha de lado sobre o workspace desfocado. Left/right; docking; tabs; Sessions; Work; blur; abas; painéis móveis. | Capítulo Arrange your workspace; ativo. É esquema de opções, não captura do layout padrão. | 920 × 410 |
| H03 · [settings-window.svg](../../prototypes/desktop/assets/help/settings-window.svg) | Settings centralizada sobre conversa, botão de fechar, abas, densidade Compact e slider percentual. Centered settings; floating window; outside click; text scale; Compact; configurações; tamanho do texto. | Capítulo Settings & personal style; ativo. Esquema com General selecionada. | 920 × 440 |
| H04 · [account-usage.svg](../../prototypes/desktop/assets/help/account-usage.svg) | Assinatura e API em cartões separados; quota/saldo desconhecidos; Subscription Only. Quota; API credit; billing; unavailable usage; créditos; limite; cobrança. | Capítulo Accounts & usage; ativo. Não exibe percentuais ou saldo inventados. | 920 × 370 |
| H05 · [review-path.svg](../../prototypes/desktop/assets/help/review-path.svg) | Pedido → trabalho → revisão → confirmação de aplicação, com correção voltando ao fluxo. Review; correction; validate; apply; target; revisão; validação; destino. | Capítulo Progress & review; ativo. Explica decisões distintas sem alegar execução real. | 920 × 330 |

### Capturas locais da ajuda

Capturas históricas de QA ignoradas pelo Git, capturadas antes do refinamento Compact/API. Os SVGs H02/H03 foram atualizados depois; as imagens antigas preservam a composição anterior. Confirmam a composição local; não substituem avaliação humana nem as capturas de sessão/Settings do app.

| Arquivo em `prototypes/desktop/artifacts/` | Assunto / aliases | Viewport |
| --- | --- | --- |
| `help-center-desktop.png` | Cabeçalho, busca, tópicos e instruções da primeira sessão. Help center; search guides; tutorials; ajuda desktop. | 1440 × 1000 |
| `help-center-mobile.png` | Cabeçalho compacto, busca e tópicos distribuídos em linhas. Mobile help; responsive topics; ajuda mobile. | 390 × 844 |
| `help-center-docking.png` | Capítulo de docking e diagrama totalmente carregado das três composições. Docking guide; panel tabs; ajuda painéis. | 1440 × 1000 |
| `help-center-accounts.png` | Uso de assinatura/API não disponível e explicação dos limites de cobrança. Usage guide; unknown quota; subscription only; ajuda contas. | 1440 × 1000 |

### Capturas históricas da navegação anterior ao refinamento Compact/API

Família preparada por [check-file-entry.mjs](../../prototypes/desktop/tools/check-file-entry.mjs), com contexto na [revisão do shell](d1-shell-revision.md). O check via `file://` passou sete composições e a jornada de sessões/docks/Settings/idiomas/ajuda, sem erros ou requisições externas. Entrada 1440/390 e Settings modeless foram examinadas; revisão visual dos demais estados e regeneração após o refinamento final da busca ainda estão em andamento. Os viewports são os do script, não necessariamente a altura do PNG de página inteira. Não atribuir a estas imagens o manifesto ou resultados da revisão mobile anterior.

| Arquivo em `prototypes/desktop/artifacts/` | Assunto / aliases | Viewport previsto |
| --- | --- | --- |
| `d1-session-entry-1440-en.png` | Primeira sessão independente, marca, cartões de início, sugestões e composer sem fixtures preenchidas. Session entry; English desktop; empty workspace; entrada simples. | 1440 × 1000 |
| `d1-session-entry-390-en.png` | Mesma entrada em inglês com composição compacta. Mobile session entry; compact chat; EN; entrada mobile. | 390 × 844 |
| `d1-session-entry-320-es.png` | Entrada compacta em espanhol na menor largura. Spanish; Español; 320; responsive session; reflow. | 320 × 900 |
| `d1-sessions-bottom.png` | Sessions no dock inferior, com projetos e sessões recentes em colunas paralelas; composer/rascunho preservados. Bottom dock; horizontal tree; projects and recents; painel inferior. | 1440 × 1000 |
| `d1-sessions-tabs-left.png` | Sessions movida para esquerda e agrupada com Navigation em abas; sessão associada a projeto com rascunho preservado. Left dock; shared tabs; attach project; abas laterais. | 1440 × 1000 |
| `d1-settings-modeless.png` | Settings General flutuante, perfil salvo e composer utilizável; Sessions no dock inferior. Floating settings; modeless; profile; bottom dock; configurações sem bloquear chat. | 1440 × 1000 |
| `d1-settings-accounts-empty.png` | Accounts sem contas conectadas, sem barra de quota ou valores fictícios. Empty accounts; unconnected; unknown usage; sem contas. | 1440 × 1000 |

### Capturas históricas da janela Settings

O responsável pela implementação de Settings examinou as duas capturas abaixo, separadamente da suite consolidada. Ambas são evidências locais ignoradas pelo Git, disponíveis em `prototypes/desktop/artifacts/`.

| Arquivo | Assunto / aliases | Dimensões do PNG |
| --- | --- | --- |
| `settings-floating-desktop.png` | Janela flutuante, categorias, contexto da conversa preservado, foco/fechamento. Desktop settings; modeless; configuração flutuante. | 1440 × 1000 |
| `settings-floating-mobile-200.png` | Settings em janela compacta com texto a 200%, categorias e rolagem adaptadas. Mobile settings; 200% text; accessible reflow; configuração ampliada. | 390 × 844 |

### Capturas do refinamento Compact/API — 10/10

Evidência local, ignorada pelo Git. As oito capturas de HTML direto vêm de [check-file-entry.mjs](../../prototypes/desktop/tools/check-file-entry.mjs); o percurso passou treze composições, preservação de sessão/rascunho/layout, Settings com fechamento externo e slider persistido, idiomas após reload e ajuda ilustrada, sem chaves ausentes, erros, imagens quebradas ou requisições HTTP/HTTPS. Capturas de janela e formulários complementam a inspeção. As quatro imagens scout são diagnósticos de espaçamento durante o ajuste, não a evidência final nem recursos do app.

| Arquivo em `prototypes/desktop/artifacts/` | Assunto / aliases | Viewport |
| --- | --- | --- |
| `d1-compact-entry-1440-en.png` | Entrada final com Sessions à esquerda, Navigation à direita, cartões e composer reduzidos; nenhum scroll a 100%. Compact entry; desktop; first fold; entrada. | 1440 × 900 |
| `d1-compact-entry-390-en.png` | Entrada estreita, conteúdo e controles com reflow natural. Mobile; compact; English; entrada. | 390 × 844 |
| `d1-compact-entry-320-es.png` | Menor largura em espanhol, sugestões e composer sem overflow horizontal. Spanish; Español; 320; reflow. | 320 × 900 |
| `d1-compact-tabs-left.png` | Navigation e Sessions agrupadas à esquerda, projeto associado e rascunho preservado. Shared dock; left tabs; projects. | 1440 × 900 |
| `d1-compact-side-chooser.png` | Só esquerda/direita em evidência, restante do workspace em blur. Placement; side chooser; blur; destinos. | 1440 × 900 |
| `d1-compact-sessions-right.png` | Sessões movidas à direita, navegação à esquerda, com o mesmo draft. Right dock; rearrange; rascunho. | 1440 × 900 |
| `d1-settings-centered-file.png` | Settings centralizada sobre o HTML direto; perfil salvo e chat preservado. Centered settings; file entry; perfil. | 1440 × 900 |
| `d1-settings-api-empty.png` | Accounts sem conexões, quotas ou saldo inventados. Empty accounts; unknown usage. | 1440 × 900 |
| `settings-centered-desktop.png` | Janela 940×720 centralizada em x250/y140, abas com ícones e slider100%. Centered; categories; percent slider. | 1440 × 1000 |
| `settings-scale-mobile-200.png` | Janela limitada ao viewport, texto200%, abas horizontais e fechamento acessível. Mobile; 200%; settings reflow. | 390 × 844 |
| `api-connection-kind.png` | Primeiro passo escolhe origem Subscription/API antes do provider. Billing source; two-stage wizard. | 1440 × 900 |
| `api-connection-gemini.png` | Gemini API com nome e chave; separado do runtime Antigravity. Google AI; API provider; key form. | 1440 × 900 |
| `api-connection-azure.png` | Azure AI com endpoint e deployment, chave e escolha explícita da cobrança. Foundry; Azure; deployment. | 1440 × 900 |
| `api-connection-bedrock.png` | AWS Bedrock com região e autenticação por API key/profile. Amazon; region; AWS credentials. | 1440 × 900 |
| `compact-scout-1440.png` | Diagnóstico desktop durante ajuste da altura, antes da prova consolidada. Scout; spacing; layout. | 1440 × 900 |
| `compact-scout-1280.png` | Diagnóstico da dobra em notebook. Laptop; spacing; fold. | 1280 × 720 |
| `compact-scout-1024.png` | Diagnóstico de cartões na largura1024; regra de colunas corrigida. Card wrapping; tablet landscape; diagnostic. | 1024 × 768 |
| `compact-scout-pt.png` | Diagnóstico português em notebook, usado na comparação dos idiomas. Portuguese; pt-BR; spacing. | 1280 × 720 |

## Organização e manutenção da biblioteca

A estrutura atual é preservada. Para novos conjuntos, usar:

| Pasta | Papel |
| --- | --- |
| `docs/design/brand/direction-01-review/` | Fontes e conceitos desta direção, com galeria e prompts. |
| `docs/design/visual-experiments/<collection>/` | Destino para futuras coleções de fundos, atmosferas, texturas ou aplicações ainda candidatas. Criar a coleção quando houver arquivos, com `README.md`, `images/` e prompts. Esta convenção não implica imagens já criadas. |
| `prototypes/desktop/assets/brand/` | Marca selecionada para uso pelo estudo local. |
| `prototypes/desktop/assets/brand/transparent/` | Três variantes sem fundo, galeria comparativa e prompts de extração; originais mantidos no nível acima. |
| `prototypes/desktop/assets/visuals/` | Ambientações selecionadas para o site, com procedência. |
| `prototypes/desktop/assets/icons/` | Ícones externos identificados, com fonte e orientação de uso. |
| `prototypes/desktop/assets/help/` | Diagramas code-native de tutoriais, com mapa textual e alternativas no centro de ajuda. |
| `docs/design/` e `docs/research/previews/` | Evidência visual versionável vinculada a decisões e entregas. |
| `prototypes/desktop/artifacts/` | Capturas, relatórios e arquivos temporários de pesquisa/QA; não é a pasta de padronização. |

Cada nova imagem ou variante deve ganhar uma entrada neste catálogo **no mesmo incremento**, mesmo se permanecer sem uso. Registrar nome descritivo, assunto, aliases em português e inglês, finalidade, estado, origem, arquivo de referência, prompt quando gerada, dimensões/transparência relevantes e local onde é usada. Fontes externas devem ter procedência e condições de uso; screenshots devem indicar fluxo e versão/data no registro da evidência. Não usar nomes genéricos como `image-final-2.png`.

Para promover um candidato à interface: confirmar coerência com a Direção 01 e a atmosfera navy, conferir recorte/transparência/legibilidade nas superfícies necessárias, preservar a fonte conceitual, copiar apenas a variante escolhida para a pasta de ativos adequada e atualizar o catálogo com sua relação fonte → uso. Uma variante específica para favicon não substitui automaticamente a logo principal. Remover um uso pode tornar o arquivo um candidato disponível novamente; registrar esse estado antes de descartá-lo.

A biblioteca pode acumular opções sem uso imediato. Novas imagens de montanhas, fundos, estilo e detalhes devem ser catalogadas como **conceitos/candidatos** até serem selecionadas para uma aplicação concreta. Esta revisão organiza o acervo existente; não gera imagens adicionais nem declara concluída a futura padronização completa do produto.


## Controles e recursos de conexões — 10/10/2026

Evidência local regenerável, não arte do produto; capturas ignoradas pelo Git e provenientes de QA no Chrome headless. Catálogos usam dados sintéticos/interceptados, sem conta real.

| Arquivo em `prototypes/desktop/artifacts/` | Assunto, finalidade e aliases | Dimensões |
| --- | --- | --- |
| `dropdown-controls-desktop.png` | Selects em Settings e Studio; estilos navy, chevron, foco; dropdown, select, controles, native menu. | 1440 × 1000 |
| `dropdown-controls-mobile-200.png` | Selects em mobile com texto a200%; reflow e legibilidade; mobile, scale, acessibilidade. | 390 × 844 |
| `dropdown-menu-studio.png` | Menu nativo aberto com opções escuras e highlight do navegador; open menu, options, keyboard. | 1440 × 1000 |
| `api-resources-observed-desktop.png` | Inspector de catálogo com reasoning, contexto, tools e campos unknown; resources, capabilities, discovery, metadados, catálogo. | 1440 × 960 |
| `api-resources-accounts-mobile-200.png` | Accounts com resumo de discovery e quota/reset/crédito desconhecidos a200%; usage, settings, accounts, unknown. | 390 × 844 |

## Diagnósticos de conexões — 10/10/2026

Capturas locais regeneráveis em `prototypes/desktop/artifacts/`, ignoradas pelo Git e verificadas visualmente no Chrome headless. Dados sintéticos/interceptados, sem consulta a contas reais. Fontes: [interface](../../prototypes/desktop/diagnostics-view.js), [contrato](connection-diagnostics.md) e [entrega](connection-diagnostics-increment.md).

| Arquivo | Assunto, finalidade e aliases | Dimensões |
| --- | --- | --- |
| `connection-diagnostics-running-desktop.png` | Relatório em andamento; 19/20 verificações concluídas e catálogo pendente. Auto checks; progress; pending catalog; cancel diagnostic. | 1440 × 960 |
| `connection-diagnostics-completed-desktop.png` | Conclusão com recurso de acesso não confirmado; suporte e disponibilidade separados, recheck e fontes/checklist expansíveis. Report; unknown access; capabilities; diagnostics. | 1440 × 960 |
| `connection-diagnostics-model-details-desktop.png` | Detalhes por modelo, níveis de reasoning, contexto, ferramentas, provenance e timestamp. Model matrix; thinking; source; observed resources. | 1440 × 960 |
| `connection-diagnostics-mobile-200.png` | Relatório em mobile com texto a200%, contagens empilhadas, janela fechável e reflow sem overflow horizontal. Responsive report; narrow; large text; accessibility. | 390 × 844 |

## Proporções e composer compacto — 11/10/2026

Capturas locais em `prototypes/desktop/artifacts/`, ignoradas pelo Git e revisadas visualmente. [Registro do refinamento](compact-composer-refinement.md).

| Arquivo | Assunto, finalidade e aliases | Dimensões |
| --- | --- | --- |
| `compact-composer-desktop.png` | Entrada com título/cartões/barras menores, chat de duas linhas e sem label visual Message. Compact sizing; composer; desktop; proportions. | 1440 × 900 |
| `compact-composer-notebook.png` | Composição em notebook, mantendo primeira dobra, cartões flexíveis e controles do chat. Laptop; initial fold; compact layout. | 1024 × 768 |
| `compact-composer-mobile-200.png` | Página inteira mobile com texto200%, textarea de96px, reflow e nome acessível preservado. Large text; mobile; scaled textarea. | 390 × 2218 |
