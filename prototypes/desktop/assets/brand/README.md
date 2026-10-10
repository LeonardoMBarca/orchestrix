# Ativos de marca do protótipo

Os sete PNGs originais são cópias dos conceitos da [Direção 01](../../../../docs/design/brand/direction-01-review/README.md). Os nomes descrevem a aplicação: `main-logo-*` (assinatura completa), `symbol-*` (símbolo), `app-icon-*` (ícone com superfície) e `orchestration-banner` (composição de agentes). Light/Dark indicam o fundo de apresentação, não temas obrigatórios do aplicativo.

As logos atuais no centro e no topo do site usam `transparent/symbol-indigo.png`: o gradiente do arco superior da assinatura principal e o arco inferior branco, sem disco preto ou fundo. O aplicativo usa essa variante nos temas escuros e `transparent/symbol-graphite.png` nos temas claros. A [coleção transparente](transparent/README.md), a [galeria](transparent/index.html) e os [prompts](transparent/prompts.json) registram três aplicações do símbolo. Os originais foram preservados. A apresentação anterior com recorte circular preto foi substituída por orientação posterior do usuário.

## Favicon transparente anterior

`favicon-transparent.png` é a primeira variante com alpha, usada anteriormente nas três abas do navegador. Foi substituída por `transparent/symbol-indigo.png`, cujo enquadramento aproveita mais a área útil da aba. O arquivo anterior permanece disponível como referência. Foi produzido pela ferramenta integrada `image_gen` em 9 de outubro de 2026, usando `symbol-dark.png` como alvo de edição, e copiado para esta pasta sem edição raster posterior. Dimensões: 1254 × 1254; alpha do canto: 0. O PNG preserva as margens da referência original.

### Prompt final

> Use case: background-extraction. Input image: EDIT TARGET, the existing approved Orchestrix emblem. Remove only the dark charcoal background, including the dark empty centre of the circular mark and the gaps between the white arcs and indigo band, and replace all those background areas with true alpha transparency. Keep the existing white upper and lower arcs and the indigo flowing horizontal band unchanged: identical proportions, curves, openings, position, colours and subtle gradients. Do not redesign, rotate, crop, round the canvas, add a surrounding circle, border, glow, shadow, text, or another symbol. Preserve the original square canvas and margins, with the existing mark centred exactly as shown. This must be a clean transparent PNG asset suitable over a navy website background. Only background removal.

O [catálogo visual](../../../../docs/design/visual-asset-catalog.md) reúne descrições, usos e fontes dos ativos e dos conceitos ainda não usados.
