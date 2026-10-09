# D1 — sistema visual do Studio

Data: 2026-10-08. Escopo: refinamento implementado em [styles.css](../../prototypes/desktop/styles.css) para o protótipo D1. Studio continua o padrão; Atelier, Horizon, Deep Black, Medieval, Forest e Dawn continuam selecionáveis pela galeria existente.

Este documento registra regras visuais e verificações técnicas. A avaliação com usuários permanece no [roteiro do piloto](d1-pilot.md) e na [matriz de cenários](d1-scenario-matrix.md); os checks abaixo não representam aceite do piloto nem uma implementação desktop funcional.

## Direção e hierarquia

O Studio mantém superfícies grafite, contraste claro, destaque lilás e cartões discretos. A navegação de projeto, objetivo, fila de tarefas e detalhe continua na mesma ordem. Cor destaca seleção e atenção; textos e ícones continuam identificando os estados.

A entrada apresenta projeto, ambiente Windows/WSL, caminho ilustrativo e conexão inicial antes do resultado. Na tela de trabalho, o objetivo precede os metadados do projeto, o controle da fila e o detalhe da tentativa. Contexto, origem da execução e verificações permanecem acessíveis pelo detalhe ou modal correspondente.

A galeria apresenta sete prévias, seleção por radio e indicação de Studio como padrão. Densidade e tamanho do texto são preferências visuais independentes da política de execução.

## Tokens

A paleta existente continua em variáveis CSS por tema. Os aliases semânticos tornam explícito o papel de cada valor e podem ser usados na futura implementação sem inventar outra biblioteca de componentes.

| Grupo | Contrato |
| --- | --- |
| Texto | `--text-body`, `--text-secondary`, `--text-caption` correspondem a texto principal, apoio e metadados legíveis. |
| Superfícies | `--surface-app`, `--surface-navigation`, `--surface-card`, `--surface-hover`, `--surface-selected` preservam a hierarquia do tema. |
| Bordas e foco | `--border-subtle` separa regiões; `--border-control` dá contraste a campos; `--focus-ring` identifica foco de teclado. |
| Estado | `--status-success`, `--status-attention`, `--status-error`, `--status-active` acompanham os fundos correspondentes e o rótulo explícito. |
| Espaçamento | `--space-1/2/3/4/5/6/8/10` representam 4/8/12/16/20/24/32/40px. |
| Densidade | `--control-height`, `--control-padding`, `--row-padding` alteram controles e linhas sem reduzir a fonte. |
| Leitura | `--reading-width:78ch` limita parágrafos; `--content-width:1600px` permite painéis largos com conteúdo legível. |

Os fundos e cores principais dos sete temas foram preservados. O texto de metadados `--faint` foi ajustado para permanecer legível nas superfícies elevadas; Dawn também recebeu um pequeno ajuste no texto secundário.

## Tipografia, controles e estados

A base é 14px com Segoe UI Variable/Segoe UI e fallback de sistema. Títulos mantêm a família de cada tema: Atelier e Medieval usam seus títulos serifados. O corpo usa 1rem; metadados têm piso equivalente a 12px; texto auxiliar e código usam normalmente 13px. O diff mantém Consolas/Cascadia Code e rolagem dentro do próprio componente.

A tipografia usa unidades relativas. `html[data-text-size="large"]` aplica escala 2 à base, produzindo corpo de 28px e ampliando também rótulos e controles textuais. Isso é uma preferência de fonte do protótipo, independente do zoom do navegador.

Controles têm altura mínima de 44px na densidade confortável e 36px na compacta. Texto ampliado, múltiplas linhas e mensagens longas podem aumentar a altura. Botões podem quebrar o rótulo; campos mantêm largura dentro do painel. Radio/checkbox têm controle nativo de 16px e um label ou cartão maior para interação.

O foco usa contorno de 2px com afastamento de 3px. Seleções têm combinação de fundo, borda/indicador e estado textual. A fila conserva indicador de tarefa selecionada; temas conservam radio marcado; fontes de contexto conservam `aria-pressed`. Aprovação, falha, bloqueio e ausência de confirmação continuam com seus rótulos específicos; o CSS não altera o significado dos estados.

Movimento é reduzido com `prefers-reduced-motion`. Em cores forçadas, contornos e bordas passam a usar cores de sistema. Esses dois modos possuem tratamento no CSS, sem validação prática nesta rodada.

## Componentes e reflow

| Região | Regra implementada |
| --- | --- |
| Entrada | `.entry-layout` usa formulário e percurso lado a lado em tela ampla; passa a uma coluna até 1150px ou com texto a 200%. |
| Projeto | `.project-meta` quebra caminhos e metadados longos. O seletor de projeto permanece visível no celular para abrir a preparação do projeto. |
| Trabalho | `.workspace` contém fila, separador e detalhe. A largura solicitada da fila vem de `--worklist-width`, limitada também pela largura disponível. |
| Separador | `.panel-resizer` ocupa 8px, indica arraste e tem foco próprio. O comportamento de ponteiro/teclado e a persistência pertencem ao JavaScript. |
| Fila recolhida | `.queue-collapsed` remove fila e separador, deixando o detalhe ocupar a região. |
| Telas menores | Até 900px, fila horizontal com rolagem própria e detalhe abaixo; separador oculto. Até 640px, navegação refluída no topo. |
| Texto a 200% | Navegação passa para o topo; entrada, revisão, configurações e formulário passam a uma coluna; fila horizontal e separador oculto. O modo de foco continua ocultando navegação e fila. |
| Contexto | Fontes em cartões com caminho, versão e motivo em linhas próprias. Prévia de código quebra linhas e tem rolagem local; seleção e opções futuras preservam labels legíveis. |
| Conexões | Linhas refluem de quatro para três e duas colunas conforme largura. Inspector, catálogo, sessões, consentimento e histórico quebram textos longos e agrupam ações com wrap. |
| Revisão | Diff preserva rolagem local. Inspector refluído abaixo do código até 1280px ou com texto ampliado. |
| Tela ampla | Conteúdo limitado a 1600px, parágrafos a 78ch e galeria com quatro colunas a partir de 1650px. |

Densidade compacta muda espaçamento e alturas; no desktop ela conserva a divisão de painéis e o redimensionamento. A fila horizontal corresponde à largura reduzida ou à ampliação do texto.

## Verificação executada

A verificação usou o preview local e Chrome headless no Windows, sem agente, autenticação ou leitura de repositório pela interface.

- Teste Playwright existente de legibilidade: passou nos sete temas e larguras 1920, 1280, 1024, 720, 390 e 320px, nas telas de trabalho e revisão. Inclui ausência de overflow da página, quatro pares de contraste e ausência de erros/requisições externas durante o teste.
- Matriz adicional de CSS: 144 combinações no Studio, com duas densidades, texto normal/200%, as seis larguras acima e telas de trabalho, revisão, atenção, conexões, preferências e entrada. Nenhuma apresentou overflow horizontal do documento ou erro de página; o corpo ampliado foi medido em 28px.
- Contraste adicional: 105 combinações nos sete temas, cobrindo texto principal/secundário/metadados, superfícies elevadas, fundos de diff, status, foco e bordas de campos. Os pares de texto passaram o piso de 4,5:1; foco e borda passaram o piso de 3:1. Valores foram resolvidos no navegador, inclusive o `color-mix()` da borda.
- Controles: o botão Nova tarefa mediu 44px confortável e 36px compacto com texto normal.
- Interação de layout: passaram teclado End/seta esquerda no separador, persistência da largura após reload, recolher/mostrar fila, persistência de densidade/texto, modo de foco em 200% e ausência de overflow nos modais de contexto/conexão a 320px.
- Inspeção visual: capturas Studio de trabalho, entrada, preferências, inspector de conexão, contexto e texto a 200% no desktop/celular. Capturas temporárias locais apoiaram a inspeção; não substituem um piloto.

O script adicional e suas imagens foram gerados no diretório temporário do sistema, sem adicionar outra ferramenta ao projeto. A cobertura de 200% em seis views se refere ao Studio; os outros temas receberam a cobertura existente de layout normal e a verificação adicional de contraste.

## Limites da rodada

Não foram executados: piloto com usuários, leitor de tela, zoom físico do navegador a 200%, modo de cores forçadas, movimento reduzido, embalagem Tauri ou avaliação visual do aplicativo nativo em Windows/WSL. O resize por teclado, a fonte ampliada manual e a persistência foram executados como descrito. A suíte posterior de [19 cenários D1](d1-delivery.md#verificação-técnica) também executou arraste do separador com ponteiro. Inspectors de contexto/conexões usam largura até 820px para leitura; demais diálogos conservam o limite original.

Os critérios de produto e as interações de domínio continuam na matriz D1. Esta folha de estilos não implementa autenticação, execução, seleção real de modelos ou integração Git.
