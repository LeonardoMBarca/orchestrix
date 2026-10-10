# D1 — entrada amigável e experiência progressiva

Registro iniciado em **9 de outubro de 2026**, atualizado em **10 de outubro** com a identidade aprovada, idiomas e refinamento da entrada mobile. As rodadas **15/15 na porta 4178**, **18/18 na porta 4179** e **73 casos por cobertura consolidada na revisão de idiomas** permanecem resultados históricos, com suas capturas e limites. O refinamento atual passou 27 casos focados, a suíte completa **77/77 em uma única rodada** e a verificação direta do HTML; sua evidência está abaixo. O piloto humano e D1/OX-D05 continuam abertos.

### Refinamento da entrada mobile — 10/10

A verificação direta anterior encontrou o campo do chat abaixo da primeira dobra, com topo em 949px em 390×844 e 1.029px em 320×900. A continuação reduz o espaço ocupado antes do campo, conservando a identidade navy/índigo, a marca e os controles:

- Em até 640px, **Options** (**Opções / Opciones**) reúne idioma, aparência e link do produto num grupo expansível ao lado de Studio. Desktop conserva esses utilitários abertos; suas ações continuam acessíveis por teclado.
- As ações de contexto ficam em linha com wrap. A entrada reduz espaçamentos, mantendo texto legível e alvos de pelo menos 44px.
- Apenas na entrada mobile, a introdução da conversa passa após o composer na ordem real do DOM. Redimensionar move esse log, sem recriar o campo ou perder foco, rascunho e cursor. Depois de enviar o primeiro pedido, o histórico conserva a ordem habitual antes do composer. O foco passa a um controle válido quando um grupo deixa de estar disponível ao redimensionar.

`npm.cmd run locales` compilou **1.012 chaves** e `npm.cmd run check` passou. Os [12 casos de entrada](../../prototypes/desktop/tests/friendly-entry.spec.mjs), incluindo quatro regressões novas, e os [15 de idiomas](../../prototypes/desktop/tests/localization.spec.mjs) passaram juntos: **27/27 em 53,5 segundos**. A suíte completa final passou **77/77 em uma única rodada de 2,2 minutos**, sem falhas ou casos ignorados.

As duas tentativas completas anteriores tiveram **76/77**. Na primeira, o helper de escolha de tema precisava abrir Options no mobile antes de acionar um botão oculto; foi adaptada a facilitação do teste, preservando suas assertions. Na segunda, o teste novo de resize encontrou perda intermitente de foco ao passar do summary mobile ao desktop: CSS ocultava o summary antes da sincronização do foco. A implementação passou a ocultá-lo pelo atributo `hidden` somente depois de transferir o foco, mantendo as assertions. Esse caso passou **cinco repetições em 9,6 segundos** antes da rodada completa final. O check direto foi repetido após essa correção, no manifesto atual abaixo.

O comando `npm.cmd run check:file` passou **sete composições** via `file://`, com o campo inteiro dentro da primeira dobra em tamanho normal:

| Viewport | Idioma | Campo do chat, topo–base |
| --- | --- | --- |
| 1440×1000 | English | 560–672px |
| 390×844 | English / Português | 661–773px |
| 390×844 | Español | 722–834px |
| 320×900 | English / Português | 741–853px |
| 320×900 | Español | 754–866px |

O **botão Send pode exigir rolagem**; essas medidas não afirmam que o formulário inteiro cabe na primeira dobra. Texto a 200% passou reflow sem overflow horizontal nas seis composições mobile, com rolagem natural. Ciclo com correção/aplicação, isolamento de rascunhos e persistência EN/PT-BR/ES também passaram via HTML direto, sem assets quebrados, chaves faltantes, erros ou requisições HTTP/HTTPS. O QA examinou quatro capturas `d1-entry-refinement-*` — desktop EN, mobile EN/ES e opções EN — mapeadas no [catálogo](visual-asset-catalog.md).

O [manifesto atual](d1-build-manifest.json) identifica **36 arquivos**, SHA-256 agregado **`896778a77a828b86ed4d12e0e77c82e4154951c10e6fc718d015180901cb727c`**; o [manifesto anterior ao refinamento](builds/2026-10-10-before-mobile.json) preserva `f5077169474953e30c0511ac60d5e561cd5e7115d4fb2d1cfb967bf720c4e965`. A prévia HTTP foi reaberta em **4173**, autorizada pelo responsável; abrir o HTML diretamente continua possível. Essa continuação não registra um novo piloto humano nem fecha D1/M0.

### Consolidação da referência de estilo — 10/10

O responsável aprovou os refinamentos e solicitou que o projeto siga esta direção. A pasta [design-system/](../../design-system/README.md) passa a reunir decisões, marca, tokens, componentes, movimento, padrões de experiência, ativos e exemplos. O guia aponta para `identity.css`, os componentes existentes e o catálogo visual, sem duplicar paletas ou imagens. A referência serve às novas telas e à migração para o Desktop dentro dos milestones existentes. A aprovação estética não substitui a realização das tarefas do piloto ou a viabilidade dos runtimes.

### Revisão de fluidez dos textos — 10/10

O responsável pediu reduzir o excesso de pontos finais e conectar melhor as frases da apresentação. Títulos e slogans passam a ter leitura contínua, como “Your idea and your agents in sync”; abertura, experiência, modos, perfis, estratégias, exemplos dinâmicos e página do vídeo receberam redação mais fluida. Inglês, proposta de produto e metáfora do voo noturno permanecem. O [guia de componentes](../../design-system/components/README.md#texto-e-idioma) registra a orientação para novos textos. A revisão altera a redação e conserva os fluxos e efeitos visuais.

`npm.cmd run check` passou; quatro casos existentes passaram em **7,8 segundos**, cobrindo fluxo manual, alternância Chat/Studio, reflow em seis larguras e site/vídeo em inglês com o app em português. As nove capturas `localization-*` foram atualizadas sem erros de página/requisição, overflow ou chaves faltantes; abertura desktop/mobile e modos foram examinados visualmente. É uma rodada focada na revisão de textos, separada da cobertura consolidada de 73 casos dos idiomas.

## Direção do produto

O Orchestrix deve atender quem deseja conectar suas contas e desenvolver pelo chat, fazendo apenas as escolhas essenciais, e quem deseja personalizar a orquestração com mais profundidade. A interface inicial precisa acolher ambos: começar com objetivo, conta e projeto; apresentar informação técnica conforme o trabalho avança ou a pessoa decide inspecioná-lo.

A identidade parte da referência **Direção 01 — Minimalista e institucional**, separada na [galeria da marca](brand/direction-01-review/index.html). Grafite, carvão, índigo, cinza neutro e branco técnico orientaram a primeira aplicação; em 10/10, o Studio recebeu a família navy/índigo aprovada no site. Os símbolos raster gerados foram incorporados ao protótipo; os textos da interface e do site são HTML legível. A prancha não identifica uma família tipográfica exata.

O percurso de produto passa a ser:

**Site de apresentação → aplicativo acolhedor e conversa → detalhes e Studio conforme a necessidade.**

Isso preserva a [organização por projetos e várias conversas](d1-chat-direction.md), os controles de revisão e a direção de orquestração automática/configurável. A proposta continua sendo um aplicativo de agentes de código. Edição leve de código pode integrar o produto posteriormente; este incremento não amplia seu escopo para uma IDE.

## Site público e disponibilidade

A [página de apresentação local](../../prototypes/desktop/website.html) explica a solução, a entrada pelo chat, projetos, uma conta ou várias e controle progressivo. Os CTAs **Download Orchestrix** encaminham à disponibilidade por plataforma; **See Orchestrix in action** abre a [página preparada para o vídeo](../../prototypes/desktop/watch.html). O CTA para experimentar o protótipo foi removido conforme a revisão do responsável. Coordenação, modos de ação, perfis e estratégias comunicam a direção do produto; sua apresentação visual não constitui implementação de integrações reais.

Windows, Linux e macOS aparecem como plataformas previstas, com estado **Coming soon**. **Não existem binários, releases ou URLs de download neste incremento.** A página não contém botões de download fictícios. O link GitHub aponta ao repositório do Orchestrix; o site permanece local, sem publicação.

### Idiomas da apresentação e do aplicativo — 10/10

O [contrato de idiomas](interface-languages.md) registra **site e página de vídeo integralmente em inglês**: textos, metadados, controles, anúncios acessíveis, exemplos e legendas do player. A descrição dos modos termina com: “We call them coding agents, but no single name can capture everything they can help you create.” O idioma público permanece independente da preferência escolhida no aplicativo.

O app começa em **English (`en`)**, com **Português (`pt-BR`)** e **Español (`es`)** no seletor **Language** da navegação — **Options → Language** no mobile — e em **Settings → Appearance → Interface language**. A preferência é global e persiste no `localStorage` (`orchestrix-prototype-language`), separada de projetos, conversas, temas e runtimes. Mudar a interface preserva nomes, mensagens, rascunhos, caminhos, contas, código/diffs e resultados já criados; não traduz o histórico nem inicia trabalho. Novos exemplos podem usar o idioma selecionado quando são criados. Os [catálogos locais](../../prototypes/desktop/locales/README.md) mantêm texto da interface separado dos valores interpolados.

**Verificação histórica da revisão de idiomas em 4180:** `npm.cmd run locales` compilou **1.010 chaves** e `npm.cmd run check` passou. O Playwright totalizou **73 casos**, com **15 novos de idiomas**. A execução inicial aprovou **65/73 em aproximadamente três minutos**; oito falhas de harness, assertions ou timeout foram corrigidas nos testes e passaram no reteste focado, **8/8 em 16,9 segundos**. Os 73 casos foram validados por cobertura consolidada; esse resultado **não equivale a uma única execução com 73 aprovações**.

O QA renderizou e examinou **nove capturas novas**, sem erros JavaScript/requisições ou overflow nos percursos e com `missing=[]` nas nove telas. O [catálogo visual](visual-asset-catalog.md) tinha naquela revisão **53 imagens versionáveis e 73 capturas locais**, conservando as referências anteriores. Os registros abaixo preservam os labels e resultados de suas versões históricas. A revisão técnica não certifica acessibilidade completa ou entendimento humano; D1/OX-D05, M0, publicação e instaladores continuam pendentes.

### Revisão da identidade do site — azul profundo

**Registro da revisão anterior da identidade.** O responsável esclareceu que a página pública deve usar uma identidade própria em azul escuro moderno, ligada à logo. O site passou a ter base navy `#0B1426`, superfícies `#101D34` e `#152641`, bordas azuladas e acentos no índigo da marca `#6366F1`. Cabeçalho, ilustração do chat, cartões, seção de orquestração, downloads e rodapé seguem a mesma família visual, com variação de profundidade entre superfícies. Essa é a apresentação fixa do site; as preferências de tema do aplicativo continuam independentes.

A revisão foi renderizada e examinada em desktop e celular (`artifacts/site-navy-desktop.png` e `site-navy-mobile.png`, ignorados pelo Git). O percurso existente do site passou **1/1 em 3,4 segundos**, incluindo CTA para o chat e reflow em seis larguras de 320 a 1920 px. O QA adicional não encontrou overflow, erros de página, HTTP ≥400 ou requisições falhas. Oito pares de texto/superfície foram conferidos, com mínimo de **4,89:1**; o CTA usa variação do índigo para manter contraste também ao passar o ponteiro. Esses pares não constituem auditoria completa de acessibilidade. As capturas e rodadas anteriores abaixo permanecem como evidência histórica.

### Modernização do site — cinco incrementos

**Registro histórico da primeira modernização, anterior às onze revisões abaixo.** Após a [pesquisa de referências oficiais](../research/site-modernization-2026-10-09.md), o responsável autorizou os cinco pontos e o uso de imagens/animações conforme a necessidade. Aquela implementação local preservou a identidade navy e usou os PNGs aprovados, com linhas auxiliares em SVG/CSS e partículas em canvas. Não foi gerada outra marca; o emblema mantém a geometria da referência. CTA para o aplicativo, controle de pausa e avisos descritos nessa versão foram revistos posteriormente.

| Incremento | Implementação disponível |
| --- | --- |
| Abertura com assinatura da marca | Título de maior escala, emblema central, luz índigo, curvas de conexão e ambiente discreto de partículas. CTA direto para o aplicativo e outro para conhecer o fluxo. |
| Demonstração interativa | **Corrigir um problema**, **Criar uma funcionalidade** e **Entender código** selecionam pedidos distintos. Quatro etapas — pedido, contexto, agentes e revisão — permitem avançar, voltar, escolher uma etapa e reiniciar. Entender código apresenta uma explicação, sem inventar um diff de alteração. |
| Coordenação em movimento | O papel relacionado à etapa recebe destaque; planejamento e desenvolvimento anteriores ficam marcados como concluídos. Conexões com pulsos acompanham a etapa. A ambientação da abertura usa até 44 pontos, limite de atualização de aproximadamente 30 fps e densidade de pixels limitada a 1,5. Esses limites são escolhas de implementação, sem benchmark de desempenho. |
| Chat essencial e Studio | Um controle mostra a mesma conversa com ou sem contexto, decisões de execução e revisão. A cena ilustra um projeto com várias conversas e uma conversa avulsa. O composer dessa seção é uma ilustração; o CTA abre o chat do aplicativo. |
| Ritmo editorial e acabamento | Seções alternam cena ampla, explicação e sequência de decisões. Corpo principal usa 16–17 px no desktop, com tamanhos responsivos e controles de ao menos 44 px de altura. Foco, hover e toque seguem a linguagem navy/índigo. |

**Reproduzir fluxo** é opt-in, avança a cada 4,3 segundos e termina na revisão. Trocar cenário, navegar manualmente ou reiniciar encerra a reprodução. A ambientação e os timers suspendem fora da tela ou com a aba oculta. **Pausar animações** pausa o movimento e encerra a reprodução; a preferência é guardada no navegador. Com `prefers-reduced-motion: reduce`, o site começa estático, desabilita reprodução automática e conserva as etapas manuais e a comparação Chat/Studio. Sem JavaScript, marca, explicações, exemplo inicial, plataformas e CTA continuam disponíveis; controles dependentes do script permanecem ocultos.

Os novos arquivos são `website.html`, `website.css` e `website.js`, disponibilizados pelo servidor estático. O check de sintaxe agora inclui o script do site. Contas, agentes, código e resultados da demonstração são fixtures locais: não há autenticação, inferência, leitura de projeto ou aplicação de alterações. Os instaladores permanecem **Em preparação**.

O primeiro teste da modernização aprovou **9/9 cenários em 23,7 segundos**. O percurso anterior do site foi aprovado separadamente, **1/1 em 4,8 segundos**. A revisão visual encontrou e corrigiu um fade que deixava o resultado parcialmente transparente quando o movimento estava pausado. Após acrescentar regressão para a legibilidade do resultado pausado e suspensão/retomada fora da tela, a rodada final aprovou **10/10 cenários em 13,4 segundos**, com dois workers no servidor 4175. Não houve execução integral de todos os testes do aplicativo neste incremento; os registros de 36/44 casos abaixo são históricos.

O QA adicional verificou seis larguras (320, 390, 720, 1024, 1440 e 1920 px), tanto na cena Chat quanto Studio: **12 combinações sem overflow horizontal**, nenhum erro de página, requisição falha ou HTTP ≥400. Dez pares de texto/superfície tiveram mínimo de **4,89:1**; são pares selecionados, sem auditoria completa de acessibilidade. Na emulação de movimento reduzido, não havia animações CSS em execução e o avanço manual permaneceu disponível.

Oito capturas `artifacts/site-modern-*.png` foram renderizadas e examinadas em desktop e celular, incluindo abertura, coordenação, revisão e Chat/Studio. O relatório fica em `artifacts/site-modernization-qa.json`, ignorado pelo Git. A captura de seção alta do Chromium trouxe um artefato do skip-link; a posição e ausência de foco foram verificadas no viewport real. O helper de captura oculta somente esse link quando sem foco durante o screenshot. O controle de acessibilidade da interface permanece intacto.

À época desta rodada, o incremento era um candidato visual e a pasta comum de padronização estava prevista após validar a direção. Em 10/10, a aprovação do responsável foi consolidada em [design-system/](../../design-system/README.md). O piloto humano D1/OX-D05 e as entregas do Core/adapters continuam abertos.

### Revisão do site — onze pontos

**Registro histórico da rodada em 4177. A decisão de manter o círculo preto das logos foi substituída na revisão seguinte.** O responsável pediu mais acabamento da marca e da navegação, uma ambientação de voo noturno entre montanhas rochosas e uma apresentação orientada ao futuro download do aplicativo. A revisão usou a mesma identidade navy/índigo, os símbolos existentes e uma [paisagem raster gerada para essa composição](../../prototypes/desktop/assets/visuals/README.md). Os controles e dados do aplicativo de estudo permanecem distintos da apresentação pública.

O [catálogo visual](./visual-asset-catalog.md) reúne a referência dos assets e suas aplicações na marca, no favicon e na ambientação desta revisão.

| Ponto revisado | Resultado na apresentação |
| --- | --- |
| Logo | A orientação dessa rodada conservava fundo e borda preta circular na logo central e no topo. O tratamento de recorte evitava um bloco quadrado no emblema; o favicon tinha fundo transparente. Essa decisão foi substituída posteriormente por logos sem fundo. |
| Ações principais | **Baixar Orchestrix** conduz à seção de plataformas; o convite para experimentar o protótipo foi retirado. O controle de pausa da ambientação também foi removido. |
| Texto público | Removidos avisos sobre papéis ilustrativos, dados simulados e ausência de acesso ao código/contas. Os limites do estudo ficam documentados aqui; o estado **Em preparação** dos instaladores continua visível. |
| GitHub | Navegação e demais links ao repositório usam o [ícone oficial baixado e armazenado localmente](../../prototypes/desktop/assets/icons/README.md), como imagem. |
| Estrelas e ponteiro | O campo canvas tem deriva lenta em repouso, aceleração discreta e temporária com a rolagem e resposta de luz/proximidade ao ponteiro em dispositivos compatíveis. |
| Conectores | Linhas e pontos encontram a lateral vertical dos cartões Sua ideia, Contexto, Código e Revisão. A atualização acompanha o pequeno movimento dos cartões. |
| Vídeo | **Veja o Orchestrix em ação** abre `watch.html`, preparado para uma gravação futura via `video-config.json`, com poster, legendas e transcrição opcionais. |
| Navegação | Como funciona, A experiência, Download e GitHub recebem tratamento visual consistente, ícones e estados de interação. |
| Modos de ação | O site apresenta Development, Research, Documentation, Presentations, Refactoring e Improvement Review, além de personalização dos modos nativos e criação de modos especializados. |
| Perfis executores | Uma seção distingue a finalidade do modo das instruções e padrões dos agentes que executam o trabalho. A personalização conserva a essência e os critérios do modo. |
| Paisagem | Montanhas rochosas em camadas, luz fria e céu navy criam a sensação de um voo noturno, com texto e superfícies sobrepostos para leitura. A referência alternativa forneceu essa ambientação, sem substituir a logo aprovada. |

O movimento não exige um botão de pausa na página. `prefers-reduced-motion: reduce` mantém a apresentação estática e as etapas manuais disponíveis; ocultar a aba suspende o trabalho de animação. A reprodução guiada da demonstração continua opt-in e possui seu controle próprio. A atualização conjunta de estrelas e conectores e seus limites locais são escolhas de implementação, sem medição comparativa de desempenho.

A página de vídeo usa controles nativos, sem autoplay. Enquanto `src` está vazio, mostra **O filme está a caminho.**; não há vídeo gravado neste incremento. O servidor local permite arquivos MP4, WebM e VTT em `assets/video/`, além de URLs HTTP/HTTPS configuradas. [Instruções para preparar a mídia](../../prototypes/desktop/README.md#preparar-o-vídeo-do-produto).

O site também explicita **uma conta ou várias** e as estratégias **Efficiency, Balanced, Performance e Custom**. [Modos e perfis](./action-modes-and-worker-profiles.md) e [estratégias de routing](./routing-strategies.md) registram requisitos, critérios e relação com o plano. Seleção de modelo/raciocínio na mesma conexão, execução serial, capacidades, snapshots e Subscription Only seguem os contratos existentes; o texto do site não entrega o router ou os adapters.

**Validação histórica dessa revisão: 15/15 cenários aprovados em uma única rodada agregada, em 36,5 segundos, no servidor 4177.** O comando selecionou `tests/site-modernization.spec.mjs` e `tests/friendly-entry.spec.mjs` com `--grep 'site-modernization|site explica'`. Cobriu CTAs sem convite ao protótipo, GitHub com imagem local, seis modos, fallback sem JavaScript, configuração de vídeo com legendas/transcrição e ausência de autoplay, movimento/redimensionamento, seis larguras e quatro conectores em 1440 e 390 px. O teste do favicon conferiu a rota do PNG e alpha zero nos quatro cantos. `npm.cmd run check` e `git diff --check` passaram. A suíte completa do aplicativo não foi repetida nesta rodada; os registros de 36/44 cenários continuam históricos.

O helper de QA renderizou **12 capturas locais** e verificou **12 combinações de layout**: Chat e Studio em 320, 390, 720, 1024, 1440 e 1920 px, sem overflow horizontal. Não foram registrados erros de página, requisições falhas ou HTTP ≥400 no percurso exercitado. Com movimento reduzido emulado, havia zero animações CSS ativas e as etapas manuais permaneceram funcionais. Dez pares específicos de contraste variaram de **4,89:1 a 17,64:1**; esses pares não constituem auditoria completa de acessibilidade. Foram examinadas visualmente a abertura em desktop/mobile, a seção de perfis em desktop e a página de vídeo em mobile.

Os testes de vídeo exercitaram configuração e apresentação do player; não demonstram a produção da gravação futura. Nenhum instalador, publicação ou commit é produzido por este incremento. Os gates e marcos de D1/Core permanecem inalterados.

### Revisão atual — transparência e movimento

**Registro da revisão do site em 09/10, na porta 4178; preservado como histórico após a aplicação visual no app.** O responsável alterou a orientação anterior e pediu as logos sem fundo. O símbolo padrão passa a ser [symbol-indigo.png](../../prototypes/desktop/assets/brand/transparent/symbol-indigo.png), com o gradiente de `main-logo-dark.png`; as alternativas [symbol-white.png](../../prototypes/desktop/assets/brand/transparent/symbol-white.png) e [symbol-graphite.png](../../prototypes/desktop/assets/brand/transparent/symbol-graphite.png) atendem às demais superfícies da interface. Os três arquivos têm 1254×1254 pixels e alpha zero nos quatro cantos e no ponto normalizado 50%/32% da região vazada. A transparência permite que o fundo da página apareça por dentro da marca. Os assets selecionados, prompts e aplicações estão na [galeria transparente](../../prototypes/desktop/assets/brand/transparent/index.html), no [registro de prompts](../../prototypes/desktop/assets/brand/transparent/prompts.json) e no [catálogo visual](./visual-asset-catalog.md).

Cabeçalho e emblema deixam de usar fundo/círculo preto, mistura de cores ou recorte ampliado em CSS. O favicon usa diretamente a variante índigo transparente. Considerando pixels com alpha ≥8, o conteúdo ocupa aproximadamente **86% da largura e 83% da altura**, contra 59%/55% do favicon anterior; isso aumenta a presença do símbolo na aba. Os testes exigem ao menos 85%/80% de ocupação. Site, página de vídeo e aplicativo usam o título **Orchestrix**.

A abertura reforça a direção do voo noturno com o texto:

> Um espaço para desenvolver pelo chat. Com uma conta ou várias, seus agentes trabalham na mesma direção. Você conduz a ideia e acompanha a rota, como um piloto em um voo noturno sobre as montanhas.

Cada estrela agora mantém deslocamento e velocidade próprios, com inércia e retorno por mola ao sair da influência do ponteiro. A suavização do ponteiro usa 70 ms; a entrada/saída da influência usa 180/400 ms. O halo tem raio de 260 px e alpha máximo de 0,085; a repulsão chega a 34 px dentro de 220 px. Esses parâmetros são escolhas locais de animação, não medidas de percepção ou desempenho físico.

O canvas tem alvos de atualização de **30 fps em repouso e 60 fps durante interação de mouse/rolagem**; conectores permanecem com alvo de 30 fps. Um teste com relógio controlado observou 12 frames em repouso e 24 durante interação em intervalos de 400 ms, com 65 estrelas. Em uma estrela amostrada, o deslocamento de 22,49 px ao sair do ponteiro continuou em 22,86 px após 32 ms e retornou a 6,45 px após 632 ms e 0,33 px após 1832 ms. Essa observação verifica continuidade e retorno no cenário controlado; não é benchmark do navegador ou monitor real. Redimensionar conserva as posições, evitando reiniciar o campo. Movimento reduzido, suspensão com aba oculta e navegação manual continuam preservados.

**Validação histórica dessa revisão: 15/15 cenários aprovados em uma única rodada agregada, em 36,2 segundos, no servidor 4178.** Os testes conferem os títulos das três páginas, cantos e região vazada dos PNGs com alpha zero, ocupação mínima do favicon e imagens ajustadas sem fundo, moldura ou blend CSS. Os demais percursos de site/vídeo da rodada anterior continuam cobertos. A suíte completa do aplicativo não foi repetida. `npm.cmd run check` e `git diff --check` passaram naquela revisão.

O QA renderizou **15 capturas**: as 12 cenas anteriores, atualizadas, mais a galeria transparente e o aplicativo em Studio e Institutional. As 12 combinações Chat/Studio nas seis larguras continuaram sem overflow e sem erros de página, requisições falhas ou HTTP ≥400. A abertura, a galeria e as duas superfícies do app foram examinadas visualmente com os novos assets; a região vazada acompanha o fundo sem artefatos observados nessas composições. Os registros anteriores de contraste e os testes não representam certificação completa de acessibilidade. Publicação, instaladores, Core e gates D1 não são alterados por esta revisão.

### Aplicação da identidade do site no aplicativo — 10/10

O responsável solicitou aplicar ao app o design aprovado no site, com uma superfície apropriada ao trabalho. O tema padrão **Studio** recebe navy/índigo, texto claro, superfícies em camadas, bordas azuladas e ações índigo. A ambientação de montanhas e o campo de partículas permanecem no site; o app usa fundos e componentes calmos para leitura e edição de pedidos.

[identity.css](../../prototypes/desktop/identity.css) reúne tokens de marca compartilhados pelo site e pelo Studio: base `#0B1426`, navegação `#0C172A`, superfície `#101D34`, superfície elevada `#152641`, overlay `#1B2E4B`, texto `#FAFAFC`, apoio `#AAB8D1`, metadados `#9CAEC9`, borda `#2A3B5A`, borda de controle `#596A92`, índigo `#6366F1`, acento `#A5ACFF`, seleção `#252F58`, ação principal `#555BDC` e hover `#5A60DA`. O [sistema visual](./d1-design-system.md#aplicação-da-identidade-compartilhada--1010) registra os papéis dos valores e sua tradução para as variáveis dos componentes.

O acabamento alcança welcome, composer, sugestões de pedido, atalhos de preparação, navegação, seletores de projeto/conversa e cartões/painéis técnicos. Preserva **Conversa primeiro**, projetos com várias conversas, conversa avulsa, início guiado opcional e Studio progressivo. Atualizar a aparência não inicia execução, envia mensagens, muda conexões ou modifica snapshots. Institutional e os outros seis temas alternativos mantêm suas paletas e continuam na galeria: **oito temas**, com Studio padrão.

**Validação histórica da aplicação visual de 10/10, anterior aos idiomas: 18/18 cenários aprovados em uma única rodada de 40,8 segundos**, na prévia [4179/index.html](http://127.0.0.1:4179/index.html): 17 casos do app e um percurso do site. Foram verificados teclado/persistência nos oito temas em seis larguras, foco e cursor, diff, texto ampliado, caminhos Windows/WSL, Enter/composição de texto, movimento reduzido, cores forçadas e Unicode a 320px. O botão Nova tarefa e quatro duplas principais tiveram suas cores efetivas resolvidas no navegador e passaram o piso de contraste de 4,5:1 nos oito temas. `npm.cmd run check` e `git diff --check` passaram.

O QA renderizou sete capturas locais `app-identity-*`, ignoradas pelo Git: welcome, conversa, trabalho, conexões, aparência e welcome Institutional em 1440×1000, além do welcome Studio em 390×844. O percurso não apresentou overflow, erros de página ou requisições/respostas falhas. As sete telas foram examinadas sem falhas materiais; o app não contém canvas ou paisagem de montanhas. Naquela versão, navegação e introdução deixavam o composer abaixo da primeira dobra em 390px; o refinamento posterior descrito no início deste registro resolve essa altura nas composições verificadas. Essa rodada focada não representa uma execução integral de todos os cenários anteriores do app nem uma certificação de acessibilidade; a rodada 15/15 em 4178 acima permanece um resultado histórico próprio do site. O piloto humano D1/OX-D05, Core/adapters, instaladores e publicação não são concluídos por este refinamento visual.

## Contrato da experiência implementada

### Registro anterior ao refinamento mobile: preparação do piloto e abertura direta — 10/10

O responsável pediu continuar o plano após aprovar a identidade e encerrar os processos de prévia. O [roteiro atual](d1-pilot.md#primeiro-percurso-atual--seis-passos-pelo-chat) apresenta seis passos pelo chat em inglês, usando o projeto de exemplo e `index.html` diretamente. Não exige configurar contas ou Studio; o convite foi enviado e as observações humanas continuam pendentes. O [manifesto](d1-build-manifest.json) identifica 36 arquivos do estudo, incluindo identidade, idiomas e assets, com o gerador/algoritmo documentados na [entrega](d1-delivery.md#revisão-atual--1010).

`npm.cmd run check:file` passou em Chrome headless sem servidor: pedido → início/conclusão → correção → nova conclusão → validação → aplicação, troca entre duas conversas, isolamento de trabalhos e restauração do rascunho. English/Português/Español persistiram após reload. As entradas em 1440×1000, 390×844 e 320×900 não apresentaram overflow horizontal, imagens quebradas, chaves faltantes ou erros de página/requisição; não houve requisição HTTP/HTTPS no percurso. O relatório local `artifacts/direct-file-qa.json` fica ignorado pelo Git. Não foram geradas capturas novas; as contagens do catálogo permanecem inalteradas. O check de sintaxe/catálogos passou.

| Viewport CSS | Topo do campo de mensagem na entrada |
| --- | --- |
| 1440×1000 | 560px |
| 390×844 | 949px |
| 320×900 | 1029px |

Aquela verificação reproduziu a observação mobile: navegação, contexto e duas introduções empurravam o composer abaixo da primeira dobra. A proposta era avaliar utilitários secundários expansíveis, ações de contexto lado a lado e introdução redundante após o composer, preservando texto, alvos e ordem de foco. Ela foi aplicada e retestada posteriormente, como registra o [refinamento atual](#refinamento-da-entrada-mobile--1010). Esta rodada anterior não alterou o estilo ou os fluxos do app; conforto e compreensão continuam precisando de observação humana.

Essa verificação separada não aumentou a contagem histórica dos 73 casos da suíte nem confirmou um piloto humano. O percurso curto avalia parte de quatro P0; os demais casos e gates continuam abertos. Os servidores anteriores permaneceram encerrados naquela etapa; a prévia 4173 foi reaberta posteriormente com autorização do responsável.

### Comportamentos

| Momento | Comportamento do protótipo | Decisão de experiência |
| --- | --- | --- |
| Primeira visita | Saudação **O que vamos criar hoje?**, composer visível e três entradas: **Conectar conta**, **Abrir projeto** e **Explorar demonstração**. | A pessoa encontra uma ação clara sem percorrer primeiro a fila técnica ou políticas de execução. |
| Sugestões iniciais | **Corrigir um problema**, **Criar uma funcionalidade** e **Entender meu código** preenchem o composer e posicionam o foco. | Sugestão não envia mensagem, não prepara tarefa e não inicia execução. O envio é explícito. |
| Conta | **Conectar conta** abre o registro de conexão da demonstração. Modalidade, identidade e autorização continuam inspecionáveis. | Uma conta pode ser a entrada inicial; a pessoa não precisa configurar uma frota. Este protótipo não solicita credenciais nem acessa providers. |
| Projeto | Preparação pede nome, caminho de exemplo e conexão inicial. **Começar pelo chat** é o trabalho inicial padrão; ambiente e templates ficam em **Opções do projeto**. **Novo projeto** permanece disponível na barra de contexto após enviar um pedido e em janela mobile. **Detalhes do projeto** expande o caminho completo do repositório de exemplo. | O começo por projeto continua disponível, com poucas escolhas visíveis e um contexto inspecionável. |
| Projeto pelo chat | Cria projeto/conversa sem tarefas ou fontes de contexto do exemplo de autenticação. A inspeção da conexão pode ser concluída na simulação ou fechada para voltar ao chat. | O primeiro trabalho vem do pedido da pessoa. Preparar projeto não cria automaticamente a fixture de refresh tokens. |
| Templates opcionais | **Correção curta** e **Feature guiada** permanecem selecionáveis e usam os exemplos de autenticação para explorar revisão e orquestração. | O percurso técnico existente continua disponível de forma intencional. |
| Conversas | Um projeto reúne várias conversas. Seletores de projeto/espaço e conversa permanecem disponíveis; **Nova conversa** e **Conversa avulsa** conservam identidades próprias. A avulsa mostra **Sem repositório associado**, explicitando o destino pendente. | Projeto e conversa continuam entidades distintas; troca de espaço não mistura rascunhos ou resultados. |
| Primeiro envio | O pedido prepara um trabalho simulado e revela seu cartão com estado e próximo passo. **Iniciar demonstração** é uma ação separada. Enviar um pedido não revela o exemplo pronto OX-24; esse exemplo aparece ao explorar a demonstração ou escolher um template explicitamente. | Mensagem, preparação, início de execução e exploração de exemplos têm efeitos distintos. |
| Aprofundar | **Studio** começa recolhido. Pode ser aberto diretamente; **Ver tarefa**, revisão ou navegação técnica revelam os controles correspondentes. | Trabalho, atenção, alterações, histórico e preferências são acessíveis quando a pessoa precisa deles. |
| Demonstração pronta | **Explorar demonstração** mostra o exemplo existente no mesmo projeto sem enviar uma mensagem em nome da pessoa. | É uma entrada para conhecer o percurso técnico, separada do pedido livre. |
| Revisar e aplicar | Diff, evidências, tentativas, validação da tarefa e confirmação da aplicação preservam o fluxo anterior. | A entrada simples mantém os controles de revisão. Uma conversa avulsa continua sem aplicação enquanto faltar destino. |

**Studio** nomeia tanto o tema padrão quanto a área de controles técnicos. Abrir essa área não altera o tema. A preferência visual e o nível de detalhe são escolhas independentes.

## Identidade e temas

- **Studio permanece o tema padrão**, agora com base navy `#0B1426` e superfícies azuladas/índigo compartilhadas com o site. A versão grafite `#0B0F14`/carvão `#1A1F29` permanece nos registros anteriores.
- **Institutional** acrescenta uma opção clara com branco técnico `#FAFAFC`, cinza neutro `#E5E7EB` e índigo.
- Atelier, Horizon, Deep Black, Medieval, Forest e Dawn continuam disponíveis: **oito temas**, com os mesmos dados e controles.
- **Personalizar aparência →** continua abrindo a galeria; **Restaurar Studio** retorna ao padrão.
- Cores de texto e ações usam variações do índigo adequadas ao contraste das superfícies claras ou escuras. Estados semânticos conservam seus significados.

Tema, densidade, texto ampliado e layout continuam persistindo no navegador. Projetos, conversas, contas registradas, mensagens, rascunhos e resultados da simulação permanecem em memória até recarregar/reiniciar.

## Limites e regras preservadas

Contas, autorizações, catálogos, modelos, raciocínio, contexto, scheduler, respostas, resultados e integração Git continuam **simulados**. Não há autenticação real, chamada de inferência, leitura de diretórios, execução de testes do projeto do usuário ou modificação de seu repositório. **Orquestração automática** no composer comunica a direção da experiência; não representa um scheduler conectado neste estudo.

Mudar preferências continua orientando tentativas futuras sem reescrever snapshots anteriores. Validar uma tarefa no Run continua distinto de aplicar um candidato no destino. Base alterada, aprovação desatualizada, destino ausente e estado incerto conservam seus bloqueios. Associação posterior de conversa avulsa a um projeto e persistência durável seguem no desenvolvimento futuro.

## Avaliar este incremento

Abra diretamente as páginas versionáveis:

- [Aplicativo e entrada pelo chat](../../prototypes/desktop/index.html).
- [Site de apresentação](../../prototypes/desktop/website.html).
- [Página preparada para o vídeo](../../prototypes/desktop/watch.html).
- [Galeria das logos transparentes](../../prototypes/desktop/assets/brand/transparent/index.html).

A prévia HTTP atual está em [4173/index.html](http://127.0.0.1:4173/index.html), reaberta com autorização do responsável; o servidor usa **4173 por padrão** com `npm.cmd start`. A porta 4180 identifica a revisão histórica de idiomas. As sessões 4175, 4177, 4178 e 4179 também correspondem às revisões anteriores. Abrir o HTML diretamente continua possível; a prévia não cria um serviço público.

Percurso curto para avaliação:

1. Abra o site, confira a navegação, o movimento ao rolar/passar o ponteiro e os conectores da abertura. Compare também com movimento reduzido.
2. Explore as etapas da demonstração, alterne Chat/Studio e examine modos, perfis e disponibilidade por plataforma.
3. Siga **See Orchestrix in action** para conferir a composição que receberá o vídeo.
4. Para avaliar o aplicativo de estudo, abra seu link direto acima; o site público não oferece mais um CTA para o protótipo.
5. No app, clique em uma sugestão, revise o texto e envie quando quiser preparar o pedido. Explore **Conectar conta** ou **Abrir projeto**, conferindo os avisos próprios da simulação.
6. Crie outra conversa, volte à primeira, abra **Studio** ou **Ver tarefa** e compare o nível de informação. Em **Personalizar aparência →**, compare Studio e Institutional. Selecione English, Português ou Español pela navegação/Aparência e confira a preservação do rascunho e do histórico; os labels anteriores deste roteiro usam Português.

### Histórico de verificação da entrada do aplicativo

A suíte histórica de projetos/conversas teve **36/36 testes aprovados** antes da entrada progressiva. Esse incremento acrescentou [oito casos de entrada amigável](../../prototypes/desktop/tests/friendly-entry.spec.mjs), totalizando **44 cenários verificados por cobertura consolidada**. A rodada integral terminou com **38 aprovados e seis falhas**, em 12,1 minutos. Após ajustar seletores de navegação e carregamento, os seis casos afetados e dois casos alterados passaram no reteste: **8/8 em 17,7 segundos**. Esse registro não equivale a uma única rodada integral com 44 aprovações. O comando `npm.cmd run check` passou e passou a incluir `experience.js`; `git diff --check` também passou.

Os casos extensos de captura e ciclos completos de chat receberam prazo de até 60 segundos, sem remover assertions; os fluxos curtos continuam com 20 segundos. O botão do logotipo foi verificado como navegação ao chat que preserva mensagem e trabalho, sem recarregar a página. Os oito temas passaram nas verificações dos pares principais de contraste e reflow em seis viewports; isso não certifica acessibilidade completa.

O QA visual renderizou e examinou sete capturas próprias: início Studio e Institutional, conversa, controles técnicos, site em desktop e celular e entrada mobile. As capturas ficam em `prototypes/desktop/artifacts/friendly-*.png`, ignoradas pelo Git. Foi corrigido um overflow de 1–2 px causado pelo brilho decorativo do site; o teste existente passou novamente (**1/1 em 2,2 segundos**) com reflow em 320, 390, 720, 1024, 1440 e 1920 px. A captura final em 390 px não apresentou overflow no site ou no app, nem erros de página ou requisições falhas no percurso exercitado.

Os novos casos verificam entrada simples e reflow, sugestões sem efeitos de envio, revelação do Studio sem início implícito, conexão simulada sem credenciais, projeto pelo chat vazio, início explícito do pedido da pessoa, demonstração sem mensagem automática e disponibilidade honesta no site. Resultado técnico e conforto/entendimento humano serão registrados separadamente.

O [README do protótipo](../../prototypes/desktop/README.md), a [entrega D1](d1-delivery.md) e o [piloto](d1-pilot.md) continuam como referências de execução e aceite. Este ajuste não conclui D1/OX-D05 nem substitui o trabalho do Core/adapters.
