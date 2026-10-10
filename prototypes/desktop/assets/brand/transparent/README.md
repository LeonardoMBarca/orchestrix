# Símbolos Orchestrix sem fundo

Preparados em **9 de outubro de 2026** com a ferramenta integrada `image_gen`, a partir dos rasters de marca existentes. Os três PNGs preservam o alpha gerado, sem edição raster posterior, e têm dimensões de **1254 × 1254**. Os originais permanecem na pasta pai.

| Arquivo | Aparência e origem | Uso atual |
| --- | --- | --- |
| [symbol-indigo.png](symbol-indigo.png) | Extração do emblema de `main-logo-dark.png`: arco superior índigo/lavanda com gradiente até branco, inferior branco e faixa índigo. | Site e vídeo: header, centro e marcas auxiliares. App: temas escuros. Favicon nas três páginas principais. |
| [symbol-white.png](symbol-white.png) | Extração de `symbol-dark.png`: dois arcos claros e faixa índigo. | Alternativa disponível para superfícies escuras. |
| [symbol-graphite.png](symbol-graphite.png) | Extração de `symbol-light.png`: dois arcos grafite e faixa índigo. | Marca do app em Institutional, Atelier e Dawn. |

Nenhum PNG contém disco, cartão ou moldura como fundo. O centro e as aberturas do emblema revelam a superfície sobre a qual ele é colocado. A interface apresenta o nome **Orchestrix** como texto HTML junto do símbolo; não há slogan embutido nesses arquivos.

## Conferência e enquadramento

A [galeria](index.html) mostra todas as variantes sobre navy e claro, e uma amostra do favicon em 16 px. No servidor local desta revisão: [abrir a galeria](http://127.0.0.1:4178/assets/brand/transparent/index.html).

O QA leu os PNGs em canvas: alpha **0 nos quatro cantos** e no ponto interno `(50%, 32%)` para as três variantes. O bounding box dos pixels com alpha ≥ 8 ocupa:

| Variante | Largura da tela | Altura da tela |
| --- | --- | --- |
| Indigo | 86,04% | 83,01% |
| White | 84,45% | 82,22% |
| Graphite | 82,78% | 79,51% |
| Favicon anterior, apenas para comparação | 58,77% | 55,10% |

A imagem Indigo assim aproveita mais o espaço fixo de ícone oferecido pela aba. O header e o centro agora exibem o PNG a 100% do container, sem ampliação, corte circular, fundo preto ou blend. A revisão visual verificou a composição no site, a galeria e a marca do app em Studio e Institutional. Essas verificações não equivalem à preparação vetorial definitiva da marca.

## Prompts e organização

O [conjunto de prompts](prompts.json) registra a extração de cada alvo, os invariantes e a solicitação de transparência/enquadramento. O [catálogo visual](../../../../../docs/design/visual-asset-catalog.md) descreve todos os ativos, incluindo as variantes antigas e os conceitos ainda disponíveis.
