# Exemplos e verificação visual

Os exemplos reutilizam as páginas e os assets existentes. Não há cópia paralela de imagens nesta pasta. As páginas versionáveis permitem recuperar a referência em um novo checkout; as capturas locais complementam a inspeção e estão mapeadas no [catálogo visual](../../docs/design/visual-asset-catalog.md).

## Referência atual

| Cena | Fonte versionável | O que comparar |
| --- | --- | --- |
| Abertura e capacidades do site | [website.html](../../prototypes/desktop/website.html) | Navy, montanhas, estrela/ponteiro, logo sem fundo, hero, navegação, modos e CTA. |
| Página do vídeo | [watch.html](../../prototypes/desktop/watch.html) | Identidade compartilhada e composição do player/estado sem vídeo. |
| Início e conversa | [index.html](../../prototypes/desktop/index.html), [experience.js](../../prototypes/desktop/experience.js) | Studio padrão; logo/pergunta, três cartões, sugestões e composer; primeira sessão independente sem fixtures preenchidas. Conversa ativa mantém histórico antes do campo. |
| Sessões e projetos | [app.js](../../prototypes/desktop/app.js) | Sessions à esquerda, comando principal New session único, grupos expansíveis, busca, criação/associação a projeto, rascunho e contexto preservados. |
| Docks e abas | [docking.js](../../prototypes/desktop/docking.js) | Navigation/Work à direita por padrão; somente destinos esquerda/direita, abas compartilhadas, blur temporário durante escolha do lado, menu/teclado, foco e redimensionamento. |
| Settings | [settings.js](../../prototypes/desktop/settings.js), [settings.css](../../prototypes/desktop/settings.css) | Abertura centralizada até 940 × 720px; janela modeless, fechamento ao clicar fora com rascunho preservado, Compact padrão, texto 80–200% por slider de 5%, padrão 100%, idiomas/Themes e uso apenas observado. |
| Conexão por modalidade | [contrato de API](../../docs/design/api-connection-contract.md), [pesquisa oficial](../../docs/research/api-provider-discovery-2026-10-10.md) | Subscription ou API antes do provedor; campos condicionais, cadastro sem falsa autenticação/capacidades e ausência de fallback API em Subscription Only. |
| Trabalho e revisão | [app.js](../../prototypes/desktop/app.js), [workspace.js](../../prototypes/desktop/workspace.js) | Superfícies técnicas, estados, hierarquia, diff e inspeção. Fixtures/controlos sintéticos são condição de teste, não execução real. |
| Ajuda externa | [help.html](../../prototypes/desktop/help.html) | Outra página, busca, nove capítulos e cinco diagramas locais; sem player de vídeo inexistente. |
| Variantes da marca | [galeria transparente](../../prototypes/desktop/assets/brand/transparent/index.html) | Indigo/White/Graphite sobre navy/claro e tamanho pequeno do favicon. |

Para abrir as páginas, usar o [procedimento do protótipo](../../prototypes/desktop/README.md), começando pelo HTML diretamente ou pela prévia HTTP em 4173, reaberta com autorização do responsável. Os servidores das revisões anteriores foram encerrados; 4180 identifica a sessão histórica de idiomas, e `npm.cmd start` usa 4173 por padrão. O [manifesto atual](../../docs/design/d1-build-manifest.json) identifica os arquivos estáticos do estudo; o código versionável é a referência permanente, independentemente da porta de uma sessão.

## Capturas e histórico

As capturas `d1-session-entry-1440-en`, `d1-session-entry-390-en` e `d1-session-entry-320-es` registram a revisão anterior da entrada; `d1-sessions-tabs-left`, `d1-settings-modeless` e `d1-settings-accounts-empty` registram seus docks e configurações. `settings-floating-desktop` e `settings-floating-mobile-200` complementam aquela verificação da janela. Quatro `help-center-*` registram busca/tópicos, mobile, docking e contas na ajuda. Esses arquivos são históricos até que o catálogo indique explicitamente sua substituição/revisão: não comprovam o novo padrão de lados, Settings centralizada ou slider. Seus nomes/cenas e estado de revisão visual estão no catálogo.

Quatro capturas `d1-entry-refinement-*` preservam o refinamento mobile anterior: `desktop-en`, `mobile-en`, `mobile-es` e `options-en`. Não representam o novo shell. As nove `localization-*` representam a revisão anterior de idiomas; as sete `app-identity-*`, a aplicação anterior da identidade ao app. Esses arquivos ficam em `prototypes/desktop/artifacts/`, ignorados pelo Git.

O [catálogo](../../docs/design/visual-asset-catalog.md) registra seus nomes, cenas e viewports, além das capturas históricas. Os screenshots anteriores à aplicação navy/índigo documentam a evolução e não substituem a base aprovada atual. A biblioteca de conceitos preserva alternativas disponíveis; conferir o status de uso antes de adotá-las.

## Conferir uma alteração visual

- Comparar as páginas/componentes afetados com o código de referência e as decisões desta pasta.
- Conferir hover, foco, seleção, disabled, estados de atenção/erro/sucesso e textos longos onde se aplicam.
- Conferir teclado, larguras compactas e amplas, texto ampliado, densidade, tema claro/escuro e EN/PT-BR/ES nas superfícies afetadas. Preservar movimento reduzido e cores forçadas.
- Na conversa, confirmar que eventos e troca de idioma preservam rascunho, cursor e conteúdo. Na revisão, preservar arquivo, versão e próxima ação.
- Registrar capturas novas no catálogo com viewport, cena e revisão. Identificar a diferença entre um exemplo de exploração e a referência em uso.

## Evidências existentes

[Sessões](../../prototypes/desktop/tests/session-revision.spec.mjs), [Settings](../../prototypes/desktop/tests/settings-revision.spec.mjs) e [Help](../../prototypes/desktop/tests/help-revision.spec.mjs) são os testes pertinentes ao shell. [D1 e acessibilidade](../../prototypes/desktop/tests/d1-accessibility.spec.mjs), [projetos/conversas](../../prototypes/desktop/tests/project-chat.spec.mjs), [entrada amigável](../../prototypes/desktop/tests/friendly-entry.spec.mjs), [site/movimento](../../prototypes/desktop/tests/site-modernization.spec.mjs) e [idiomas](../../prototypes/desktop/tests/localization.spec.mjs) mantêm as regressões relacionadas. Executar os casos pertinentes à mudança; ampliar quando houver novo comportamento ou preocupação concreta. A presença desses arquivos não comprova uma rodada completa dos sete refinamentos atuais.

**Histórico anterior ao shell:** a revisão mobile passou **77/77 casos em uma única rodada de 2,2 minutos**, após 27 focados, com **1.012 chaves** naquele catálogo. As duas tentativas anteriores de 76/77 e suas correções estão na [experiência progressiva](../../docs/design/d1-progressive-experience.md#refinamento-da-entrada-mobile--1010). A revisão histórica de idiomas validou **73 casos por cobertura consolidada**: 65 na execução completa e oito em reteste, sem uma rodada única de 73 aprovações. Esses números não certificam o novo shell; os resultados atuais ficam no [status](../../docs/DEVELOPMENT_STATUS.md) e na [revisão de sessões/Settings](../../docs/design/d1-shell-revision.md).

**Histórico da revisão de painéis anterior aos sete refinamentos:** `npm.cmd run check:file` passou sete composições via `file://`, criação de sessões/projeto, associação preservando rascunho, docking, Settings com chat utilizável, retomada e idiomas persistidos e ajuda com diagramas locais. Não houve imagens quebradas, chaves faltantes, erros ou requisições HTTP/HTTPS nesse percurso. Esse check é separado da suíte Playwright e não demonstra acesso à pasta, autenticação ou execução de runtime. Não certifica a rodada atual. O arranjo usa rolagem natural; não importar a promessa de primeira dobra de uma composição anterior.

Esta revisão mantém a identidade visual e altera a organização da experiência. O responsável adiou o piloto até considerar a interface adequada; resultados humanos continuam em branco.


**Fechamento Compact/API:** catálogo de 1.303 chaves; suíte completa 128/130 em 5,3m, seguida de 2/2 retestes isolados de vídeo. Cobertura consolidada de 130, não uma rodada única 130/130. HTML direto em 13 composições; entrada sem scroll nos viewports de notebook a 100%, preservando reflow em mobile e texto ampliado. [Evidência atual](../../docs/DEVELOPMENT_STATUS.md).
