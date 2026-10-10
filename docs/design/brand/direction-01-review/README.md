# Orchestrix — Direção 01: validação visual

Esta pasta separa a direção **“Minimalista e institucional”** da prancha fornecida em **23 imagens PNG**, agrupadas por conceito.

Abra [a galeria](index.html) no navegador. Ela funciona como página estática local, sem instalação nem dependências externas. Os botões **Claro** e **Escuro** mudam a superfície da galeria para facilitar a avaliação; não alteram as imagens. Cada cartão oferece abertura do arquivo original e download individual.

## Origem e finalidade

Os arquivos em `images/` são **reconstruções raster geradas com image_gen a partir da prancha enviada pelo usuário**. Servem para validar a direção visual antes de preparar os ativos finais. Não são recortes pixel-exatos da imagem original: forma, espaçamento, composição ou texto podem apresentar diferenças de geração. Os [prompts](prompts.json) registram a preparação das imagens.

A referência descreve uma **sans geométrica e moderna**, mas não indica o nome da família tipográfica. A amostra gerada não define uma fonte final. A fonte usada na própria galeria também é apenas para navegação, não uma especificação da marca.

Depois da validação, a preparação para produção deverá definir a tipografia, vetorizar o símbolo e o wordmark, normalizar dimensões e margens, preparar transparência quando necessária e exportar as versões para os usos reais do aplicativo.

## Paleta impressa na referência

| Cor | Hexadecimal | Papel indicado |
|---|---|---|
| Grafite profundo | `#0B0F14` | Base, profundidade e foco |
| Cinza carvão | `#1A1F29` | Superfícies e hierarquia |
| Índigo elétrico | `#6366F1` | Acento principal e interação |
| Cinza neutro | `#E5E7EB` | Fontes e bordas sutis |
| Branco técnico | `#FAFAFC` | Espaço, clareza e equilíbrio |

Esses valores são os códigos declarados na prancha. Gradientes, sombras e variações de raster presentes nos PNGs não substituem os hexadecimais de referência.

Uma amostragem dos cartões gerados encontrou pequenos desvios RGB, de até 7 níveis por canal. Ao implementar o tema, use os códigos da tabela como valores exatos; não extraia as cores dos PNGs com um conta-gotas.

## Inventário

### Marca principal

- [Logo principal · Light](images/01-main-logo-light.png) — Símbolo, nome e assinatura em fundo claro.
- [Logo principal · Dark](images/02-main-logo-dark.png) — Símbolo, nome e assinatura em fundo escuro.
- [Símbolo · Light](images/03-symbol-light.png) — Marca gráfica isolada em fundo claro.
- [Símbolo · Dark](images/04-symbol-dark.png) — Marca gráfica isolada em fundo escuro.
- [App icon · Light](images/05-app-icon-light.png) — Ícone com superfície clara e sombra.
- [App icon · Dark](images/06-app-icon-dark.png) — Ícone com superfície escura e sombra.

### Paleta

- [Grafite profundo](images/07-color-deep-graphite.png) — Base, profundidade e foco. Valor da referência: `#0B0F14`.
- [Cinza carvão](images/08-color-charcoal.png) — Superfícies e hierarquia. Valor da referência: `#1A1F29`.
- [Índigo elétrico](images/09-color-electric-indigo.png) — Acento principal e interação. Valor da referência: `#6366F1`.
- [Cinza neutro](images/10-color-neutral-gray.png) — Fontes e bordas sutis. Valor da referência: `#E5E7EB`.
- [Branco técnico](images/11-color-technical-white.png) — Espaço, clareza e equilíbrio. Valor da referência: `#FAFAFC`.

### Tipografia

- [Direção tipográfica](images/12-typography.png) — Clareza, legibilidade e aparência técnica. A fonte final ainda precisa ser definida.

### Elementos auxiliares

- [Padrão de fundo](images/13-background-pattern.png) — Grade de pontos: estrutura e profundidade.
- [Módulos / nós](images/14-modular-nodes.png) — Nós conectados: coordenação modular.
- [Linhas de fluxo](images/15-flow-lines.png) — Orquestração e controle.
- [Formas de interface](images/16-interface-shapes.png) — Quadrado, círculo e cápsula: simplicidade e sistema.
- [Loading / estado vazio](images/17-loading-empty-state.png) — Anel de progresso: progresso contínuo.
- [Marca d’água / emblema](images/18-watermark-emblem.png) — Símbolo em baixa intensidade: presença sutil.

### Conceito

- [Conceito da marca](images/19-brand-concept.png) — Desktop, local-first e open source; coordenação, precisão e controle.

### Aplicações da marca

- [Aplicação · Light](images/20-application-light.png) — Composição em fundo claro.
- [Aplicação · Dark](images/21-application-dark.png) — Composição em fundo escuro.
- [Aplicação · Monochrome](images/22-application-monochrome.png) — Composição monocromática.

### Banner

- [Múltiplos agentes. Um mesmo propósito.](images/23-orchestration-banner.png) — Coder, Reviewer, Tester e Documenter convergem para o Orchestrix.

## Como avaliar

Entrega verificada: 23 PNGs válidos, revisão visual dos 23 conceitos e conferência dos textos. A galeria carregou todas as imagens, abriu referências locais existentes e alternou entre Claro e Escuro; também foi conferida em largura de 390 px, sem transbordamento horizontal ou erros de JavaScript.

Compare a forma do símbolo entre as variantes, a legibilidade do nome e da assinatura, o contraste nas superfícies clara e escura e a coerência dos elementos auxiliares. Abra o PNG original para avaliar detalhes que fiquem pequenos no cartão.

A assinatura da referência é **“ORQUESTRE · DESENVOLVA · MAIS LONGE”**. O banner apresenta **“Múltiplos agentes. Um mesmo propósito.”** com os papéis Coder, Reviewer, Tester e Documenter.

Esta pasta registra a direção para aprovação visual. Sua criação não aplica automaticamente o tema ao protótipo ou ao aplicativo.

Os conceitos posteriormente selecionados para o protótipo e os usos atuais estão mapeados no [catálogo textual de imagens](../../visual-asset-catalog.md), junto de fundos, ícones, variantes e capturas de interface. O catálogo permite encontrar também os conceitos ainda sem uso por descrição e aliases, e registra a regra de preservar esta coleção como fonte.
