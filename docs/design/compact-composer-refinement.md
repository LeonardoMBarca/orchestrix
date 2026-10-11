# Proporções do shell e composer — 11/10/2026

A pedido do responsável, a interface ficou mais compacta: título e marca menores, cartões/espaços reduzidos, barra superior de60px no desktop e itens laterais mais curtos. A entrada do chat passa a ter largura máxima de720px. A fonte base continua14px e as preferências de escala/painéis são preservadas.

O campo de mensagem começa com duas linhas, fonte `1rem`, altura `calc(3em + 12px)` e resize vertical. Seu rótulo “Message” fica apenas como label acessível `sr-only`; não há texto redundante ocupando espaço visual. A altura acompanha o texto ampliado em vez de manter uma caixa fixa que corta linhas. Comfortable conserva alvos de44px; traduções e mobile refluem naturalmente.

Fontes: [shell.css](../../prototypes/desktop/shell.css), [experience.js](../../prototypes/desktop/experience.js) e [guia de componentes](../../design-system/components/README.md). Este é um ajuste de proporções, sem mudança de marcos ou fechamento do piloto D1.

## Verificação

- **37/37 em2,0min**: suítes existentes de entrada, chat/projetos e localização.
- `npm run check` aprovado, **1.534 chaves**; `npm run check:file` aprovado em **13 composições**, com jornada de sessões/Settings/docks/idiomas/ajuda e nenhuma requisição externa, imagem quebrada ou erro.
- QA visual: desktop1440×900, notebook1024×768 e mobile390px com texto200%. Sem overflow horizontal, erro JavaScript ou imagem quebrada nas condições verificadas. Rótulo acessível “Message” único e presente, visualmente oculto.
- Desktop: composer **246 →181px**, textarea **70 →54px**, título **37,8 →30,1px**. A200%, textarea cresce para **96px**, acomodando duas linhas. Essas medidas descrevem as condições de QA; traduções/conteúdo e preferências podem alterar alturas.

## Posição do campo de mensagem

O ajuste seguinte aproxima o campo da parte inferior da entrada desktop: composer a 16px do fim da área central, cartões/sugestões acima e marca/título centralizados no espaço restante. O layout é flexível; mobile e conversas em andamento conservam seu fluxo.

A revisão de espaçamento sobe os três cartões ao aumentar sua margem inferior de 12px para `clamp(28px,5vh,48px)` no desktop e 24px no mobile. As sugestões e o composer mantêm sua posição junto ao rodapé, criando uma separação visual mais clara entre os atalhos de configuração e a conversa. Esta revisão passou os **10/10** testes existentes de entrada; QA em 1440×900 e 1280×720 confirmou margens de 45px e 36px, campo a 16px do fim, EN/PT/ES sem rolagem a 100% e reflow sem overflow horizontal em mobile/texto a 200%.

Verificação deste ajuste: **10/10** testes existentes de entrada, `npm run check` com **1.534 chaves** e `npm run check:file` em **13 composições**. Capturas 1440×900 e 1280×720 revisadas; EN/PT/ES em notebook a 100% sem rolagem. Textarea de 54px e distância inferior de 16px nos dois tamanhos. A 200%, textarea de 96px e rolagem natural, sem overflow horizontal em desktop/mobile. Nenhum erro JavaScript ou imagem quebrada nas condições verificadas.

Capturas e aliases estão no [catálogo textual](visual-asset-catalog.md). Build atual: [63 arquivos](d1-build-manifest.json), SHA-256 `8cdad562051d594ba8d6adf693a65226d79471ce4491fa9edc3c757c5a87d35c`. O [build anterior ao reposicionamento](builds/2026-10-11-before-lower-composer.json) conserva o digest `3c00f0faa78b047f79e0945f9afbd1dffaddf501e4d4eb7c61a244d8cbc997d4`; o [anterior ao ajuste de proporções](builds/2026-10-11-before-compact-composer.json), `b381d12d3e48a7d30bb675158b95f3896c27474fed3defbb574cb53977e294ee`.
