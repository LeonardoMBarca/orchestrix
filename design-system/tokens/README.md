# Tokens e fundamentos

Referência da base aprovada em **10/10/2026**. A fonte dos valores compartilhados é [identity.css](../../prototypes/desktop/identity.css). O app aplica os aliases em [experience.css](../../prototypes/desktop/experience.css), sobre os componentes/temas de [styles.css](../../prototypes/desktop/styles.css); o site aplica seus aliases em [website.css](../../prototypes/desktop/website.css).

Esta pasta documenta os valores existentes. Não mantém outro CSS ou JSON com uma cópia independente da paleta.

## Marca compartilhada

| Papel | Token | Valor atual |
| --- | --- | --- |
| Fundo principal | `--orx-navy` | `#0B1426` |
| Navegação | `--orx-sidebar` | `#0C172A` |
| Cartão/painel | `--orx-surface` | `#101D34` |
| Superfície elevada | `--orx-surface-raised` | `#152641` |
| Camada acima da superfície | `--orx-overlay` | `#1B2E4B` |
| Texto principal | `--orx-text` | `#FAFAFC` |
| Texto de apoio | `--orx-muted` | `#AAB8D1` |
| Metadados | `--orx-subtle` | `#9CAEC9` |
| Separação de regiões | `--orx-line` | `#2A3B5A` |
| Borda de campos no Studio | `--orx-control-border` | `#596A92` |
| Índigo da marca | `--orx-indigo` | `#6366F1` |
| Acento legível/seleção | `--orx-accent` | `#A5ACFF` |
| Fundo da seleção | `--orx-accent-surface` | `#252F58` |
| Botão primário | `--orx-primary` | `#555BDC` |
| Hover do botão primário | `--orx-primary-hover` | `#5A60DA` |

Índigo de marca, botão primário e acento legível são papéis diferentes. Usar o valor do papel existente em vez de aplicar `#6366F1` a qualquer texto ou botão.

## Aliases e temas do aplicativo

| Grupo | Aliases atuais |
| --- | --- |
| Texto | `--text-body`, `--text-secondary`, `--text-caption` |
| Superfícies | `--surface-app`, `--surface-navigation`, `--surface-card`, `--surface-hover`, `--surface-selected` |
| Bordas/foco | `--border-subtle`, `--border-control`, `--focus-ring` |
| Estado | `--status-success`, `--status-attention`, `--status-error`, `--status-active`, com os fundos de estado correspondentes |
| Ação primária | `--primary`, `--primary-hover`, `--on-primary` |

Studio resolve sua identidade pelo CSS compartilhado. Os outros sete temas conservam as paletas e famílias existentes; componente novo deve responder ao tema pelos aliases. Significado de estado vem também de texto e ícone. Preservar foco, selecionado, disabled, atenção, erro e sucesso nas superfícies em que aparecem.

## Tipografia e medidas

- A família compartilhada `--orx-font` é Inter, Segoe UI, system-ui e fallbacks locais. Não há download de fonte obrigatório. A referência raster não determina outra família exata.
- O site tem corpo de 16px; o app usa `--font-base:14px` e tamanhos relativos. A preferência de texto ampliado aplica escala 2. Fontes dos temas alternativos e a fonte monoespaçada do código permanecem próprias dos seus papéis.
- A escala de espaçamento do app é `--space-1/2/3/4/5/6/8/10`: **4, 8, 12, 16, 20, 24, 32 e 40px**. Densidade altera espaçamento/altura, preservando o tamanho da fonte.
- Controles comuns usam `--control-height`: **44px** confortável e **36px** compacto. Componentes com tratamento próprio, como composer e sugestões, têm suas medidas no CSS de experiência; manter espaço suficiente para conteúdo em mais de uma linha.
- O Studio tem `--radius:14px`; componentes específicos usam seus próprios raios, como botão 10px e composer 18px. Uma nova variante deve partir do componente equivalente.
- `--reading-width:78ch` limita prosa e `--content-width:1600px` limita a região técnica do app. O site usa `--page-width:1240px`; conversa e welcome têm limites próprios no CSS de experiência.
- Foco do app: contorno de 2px com afastamento de 3px; foco dos links/botões do site: 3px com afastamento de 5px. Preservar o contorno e sua variante para cores forçadas.

## Manutenção

Ao mudar uma base, conferir seu uso no site, no Studio e nos temas/estados afetados. Atualizar a fonte de código e este guia conjuntamente. Na implementação de produção, importar/reutilizar a fonte ou portar os tokens mantendo uma fonte única; não criar uma paleta de produção desvinculada desta referência.

As verificações de contraste e reflow já executadas estão no [sistema visual D1](../../docs/design/d1-design-system.md). Seus resultados se referem às combinações e rodadas registradas, não a qualquer valor novo introduzido depois.
