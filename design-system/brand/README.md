# Marca Orchestrix

Referência de aplicação aprovada em **10 de outubro de 2026**. A identidade segue a **Direção 01**: símbolo circular aberto, atravessado por uma faixa ondulada índigo, com o nome Orchestrix em tipografia sans. Navy, índigo, clareza e coordenação orientam o conjunto. A inspiração de voo noturno entre montanhas acrescenta ambientação ao site; não altera o símbolo nem adota a marca da Direção 03.

As variantes atuais estão na [coleção transparente](../../prototypes/desktop/assets/brand/transparent/README.md). Elas revelam o fundo da própria página no entorno, no centro e nas aberturas do símbolo. A apresentação antiga com disco, centro ou borda preta foi substituída pela revisão seguinte do responsável.

## Escolha da variante

| Aplicação | Arquivo e regra |
| --- | --- |
| Site, página de vídeo, header e emblema central | [Indigo — `symbol-indigo.png`](../../prototypes/desktop/assets/brand/transparent/symbol-indigo.png). Arco superior lavanda/índigo com gradiente até branco, inferior branco e faixa índigo. Usar o símbolo sem fundo, com o nome **Orchestrix** como texto HTML quando a composição pedir uma assinatura. |
| Aplicativo nos temas escuros | A mesma variante Indigo. É a marca atual do Studio padrão, Horizon, Deep Black, Medieval e Forest. |
| Aplicativo em Institutional, Atelier e Dawn | [Graphite — `symbol-graphite.png`](../../prototypes/desktop/assets/brand/transparent/symbol-graphite.png). Arcos grafite com faixa índigo sobre a superfície clara do tema. O grafite pertence ao desenho do símbolo; não é um disco de fundo. |
| Alternativa disponível para superfícies escuras | [White — `symbol-white.png`](../../prototypes/desktop/assets/brand/transparent/symbol-white.png). Arcos brancos e faixa índigo. Está na galeria para comparação; não substituir automaticamente a variante Indigo aplicada. |
| Favicon | A variante Indigo transparente nas três páginas principais, com título da aba **Orchestrix**. Aproveitar a área útil do ícone, sem acrescentar um quadrado, cartão ou círculo preto. |

## Composição e preservação

- Preservar proporções, gradientes, aberturas e transparência. Aplicar o PNG com `object-fit: contain`, como nas interfaces atuais; não forçar recorte circular, fundo opaco, blend ou filtro para simular outra variante.
- Manter o nome como texto separado do símbolo. Os [lockups raster originais](../../prototypes/desktop/assets/brand/README.md) contêm a apresentação e a assinatura em português da referência; são fontes preservadas, sem substituição automática do header atual ou de textos localizáveis.
- Conferir a marca sobre a superfície real, no header, no centro e em tamanho de favicon. A [galeria comparativa](../../prototypes/desktop/assets/brand/transparent/index.html) mostra as três variantes sobre navy e claro. Não inferir uma margem universal a partir do canvas raster.
- Compartilhar os valores de identidade de [identity.css](../../prototypes/desktop/identity.css). O [sistema visual D1](../../docs/design/d1-design-system.md) descreve a aplicação às superfícies e controles; os demais temas mantêm suas escolhas próprias.
- Reservar [montanhas](../../prototypes/desktop/assets/visuals/README.md), estrelas e efeitos de fundo para o site e sua página de vídeo. O aplicativo usa a identidade navy/índigo com superfícies funcionais, sem a paisagem ou o campo de partículas.

Os três símbolos são PNGs de 1254 × 1254 preparados com `image_gen` a partir das fontes existentes. Os [prompts e invariantes](../../prototypes/desktop/assets/brand/transparent/prompts.json) registram a derivação. As verificações de alpha e enquadramento constam no README da coleção; não equivalem a certificação geométrica ou a uma marca vetorial final.

Antes de introduzir outra aplicação ou variante, consultar o [guia de ativos](../assets/README.md) e registrar fonte, finalidade e estado no [catálogo textual](../../docs/design/visual-asset-catalog.md). Uma imagem candidata ou um exemplo histórico não redefine a identidade aprovada.
