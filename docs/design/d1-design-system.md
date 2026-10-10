# D1 — sistema visual do Studio

Data: 2026-10-08, revisado em 2026-10-09 e **2026-10-10**. Escopo: refinamento do protótipo D1 em [styles.css](../../prototypes/desktop/styles.css), [experience.css](../../prototypes/desktop/experience.css) e [identity.css](../../prototypes/desktop/identity.css). Studio continua o padrão, agora em navy/índigo compartilhado com o site; Institutional, Atelier, Horizon, Deep Black, Medieval, Forest e Dawn continuam selecionáveis: **oito temas**.

**Referência permanente:** após a aprovação visual do responsável, [design-system/](../../design-system/README.md) reúne os guias vigentes para site, app e futuras telas. O [contrato do shell](../../design-system/workspace-shell.md) já incorpora os sete refinamentos de navegação e Settings. Este documento conserva também o registro técnico das rodadas anteriores, incluindo o [refinamento mobile histórico](d1-progressive-experience.md#refinamento-da-entrada-mobile--1010). Resultados anteriores não certificam automaticamente a nova composição; aprovação estética e verificações técnicas não encerram o piloto.

Este documento registra regras visuais e verificações técnicas. A avaliação com usuários permanece no [roteiro do piloto](d1-pilot.md) e na [matriz de cenários](d1-scenario-matrix.md); o responsável adiou o piloto até considerar a interface adequada. Os checks não representam aceite humano ou implementação de produção. As passagens técnicas e capturas registradas abaixo são históricas; a evidência dos refinamentos atuais está consolidada no [status](../DEVELOPMENT_STATUS.md).

## Direção e hierarquia

As conexões acrescentam o [diagnóstico automático](connection-diagnostics-increment.md), com cartão de progresso, relatório fechável, recheck e detalhes expansíveis por recurso. Suporte, disponibilidade e política são apresentados separadamente, usando tokens semânticos e idiomas existentes. O resumo não exige conhecimento do protocolo; escopo, fontes e datas ficam acessíveis. As fontes visuais são [diagnostics.css](../../prototypes/desktop/diagnostics.css) e [diagnostics-view.js](../../prototypes/desktop/diagnostics-view.js); o [guia de componentes](../../design-system/components/README.md) mantém o padrão para novas telas.

O Studio usa superfícies navy em camadas, texto claro, acentos índigo e cartões discretos, seguindo a identidade aprovada no site. O aplicativo recebe a linguagem de cor, tipografia, borda e interação; montanhas e partículas permanecem na apresentação pública. A entrada prioriza conversa e composer, com os controles técnicos do Studio acessíveis conforme a necessidade. Cor destaca seleção e atenção; textos e ícones continuam identificando os estados.

A abertura normal começa com uma sessão independente, sem conexão, projeto ou trabalho fictício preenchidos. Logo e pergunta curta precedem os cartões de início, sugestões e composer. Projetos aparecem na lista Sessions, com associação opcional e contexto/diretório explícitos. Na tela de trabalho, o objetivo precede os metadados do projeto, o controle da fila e o detalhe da tentativa. Contexto, origem da execução e verificações permanecem acessíveis pelo detalhe ou janela correspondente.

A galeria apresenta oito prévias, seleção por radio e indicação de Studio como padrão, em **Settings → Themes**. General contém idioma, densidade **Compact por padrão** e slider de texto **80–200%, em passos de 5%, padrão 100%**. Essas escolhas são independentes da política de execução.

Sessions começa **à esquerda**, com um único comando principal **New session**; Navigation e Work começam **à direita**. Docking oferece apenas esses dois lados, com abas quando painéis compartilham um deles. Durante a escolha de lado, o blur temporário do fundo destaca os destinos; cancelamento e conclusão removem o efeito. Menu e teclado oferecem alternativas ao arraste. O dock inferior e o grupo Options da composição anterior deixam de ser referências atuais.

Settings abre **centralizada, até 940 × 720px**, adaptada à área visível, e continua flutuante/modeless, movível e redimensionável. Clicar fora fecha sem descartar o rascunho de perfil/instruções para reabertura durante a sessão do app; salvamento continua explícito. O chat permanece utilizável. Mudanças de layout conservam rascunho/cursor e devolvem o foco a um controle válido. Reflow e rolagem natural atendem telas menores e texto ampliado; as medições de primeira dobra da versão anterior são históricas.

Em Accounts, escolher **Subscription ou API antes do provedor/backend**. Cadastro é configuração, não autenticação comprovada. A [pesquisa oficial](../research/api-provider-discovery-2026-10-10.md) e o [contrato de conexão](api-connection-contract.md) especificam campos condicionais, catálogo/capabilities com proveniência, segredos fora do HTML e telemetria desconhecida. Subscription Only não autoriza fallback API.

### Aplicação da identidade compartilhada — 10/10

`identity.css` centraliza os valores de marca consumidos pelo site e pelo Studio. `experience.css` traduz esses valores para as variáveis de superfície e controle do aplicativo. A aplicação ao tema padrão conserva os overrides dos outros sete temas e os significados das cores de estado. Preferência de tema e nível de detalhe do Studio continuam independentes.

| Papel | Token compartilhado | Valor |
| --- | --- | --- |
| Base e navegação | `--orx-navy`, `--orx-sidebar` | `#0B1426`, `#0C172A` |
| Superfícies | `--orx-surface`, `--orx-surface-raised`, `--orx-overlay` | `#101D34`, `#152641`, `#1B2E4B` |
| Texto | `--orx-text`, `--orx-muted`, `--orx-subtle` | `#FAFAFC`, `#AAB8D1`, `#9CAEC9` |
| Bordas | `--orx-line`, `--orx-control-border` | `#2A3B5A`, `#596A92` |
| Marca e acentos | `--orx-indigo`, `--orx-accent`, `--orx-accent-surface` | `#6366F1`, `#A5ACFF`, `#252F58` |
| Ação principal | `--orx-primary`, `--orx-primary-hover` | `#555BDC`, `#5A60DA` |

Ação principal e acento têm papéis distintos: no Studio, botões primários usam texto claro sobre índigo, enquanto seleções e indicadores usam o acento claro. A família compartilhada é Inter/Segoe UI/system-ui, com fallback local; não depende de baixar fontes. Bordas, hierarquia do welcome/composer, sugestões, navegação, seletores e superfícies técnicas recebem acabamento consistente. Os contratos de ações e estados, projetos com várias conversas, conversa avulsa, painéis, densidade e ampliação do texto permanecem preservados.

### Histórico da direção de conversa — feedback real de 09/10

Os parágrafos seguintes registram o incremento de 09/10 e seus limites naquela versão, anterior à lista Sessions, associação persistida no navegador e Settings atual. Os nomes de controles e resultados não são instruções de navegação da revisão vigente.

O responsável relatou gostar da interface, considerá-la pouco intuitiva e sentir falta de chat. Pediu uma experiência de agente de código preservando a interface existente. Em refinamento ainda de 09/10, esclareceu: manter o início por projetos e o fluxo guiado; cada projeto pode reunir várias conversas e uma conversa avulsa pode ser o ponto de partida. **Project != Conversation**, com vínculo opcional. Esse relato fundamenta a [direção de projetos/conversas](d1-chat-direction.md); não equivale a conclusão registrada das tarefas do piloto.

**Incremento implementado:** seleção/preparação de projetos e percurso guiado preservados, com conversas do projeto, novos chats e entrada avulsa. Na conversa selecionada, composer e mensagens ligadas ao trabalho organizam pedidos e acompanhamento. Runs/tarefas, atenção, diff/checks e inspectors permanecem acessíveis. Projeto, conversa e trabalho têm rótulos próprios; conta e sessões nativas continuam nos detalhes da tentativa.

O contrato usa **Projeto ou espaço**, **Conversa**, **Nova conversa**, **Conversa avulsa** e **Abrir projeto**, além do composer/**Enviar** e cartões ligados ao trabalho. O primeiro pedido prepara tarefa/Run sem iniciar execução; mensagens registram orientação, e correção passa pelo envio explícito na revisão. Trocas preservam mensagens, rascunhos e referências em memória; IDs distintos evitam mistura mesmo com nomes iguais. Temas são globais. Persistência durável e restauração após reload/restart não entram neste incremento. A avulsa não tem repositório/destino e bloqueia aplicação; associação posterior não está entregue.

Preservar temas e tokens existentes. Navegação de projetos/conversas, mensagens, resumo de trabalho e próxima ação devem ter hierarquia legível; conta/modelo/contexto entram em divulgação progressiva. Correção deve indicar tarefa/artefato de destino. Aceitar tarefa e aplicar Run conservam ações e confirmações separadas. Em layouts compactos e com texto ampliado, seletor de conversa, composer, mensagens e ações de revisão devem continuar operáveis por teclado, sem roubar foco ou posição ao receber eventos.

Os componentes de conversa usam os mesmos tokens e superfícies Studio. A passagem técnica passou **36/36 testes**, incluindo [12 de conversa](../../prototypes/desktop/tests/project-chat.spec.mjs), e capturas próprias foram examinadas na [entrega](d1-delivery.md). Os números anteriores de 144 combinações/105 pares não se estendem automaticamente ao chat.

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

Na auditoria original, os fundos e cores principais dos sete temas então existentes foram preservados. O texto de metadados `--faint` foi ajustado para permanecer legível nas superfícies elevadas; Dawn também recebeu um pequeno ajuste no texto secundário. Em 10/10, apenas a identidade cromática do Studio migra para navy/índigo compartilhado; Institutional e os seis temas alternativos conservam suas paletas.

## Tipografia, controles e estados

A base é 14px. Studio usa a família compartilhada Inter/Segoe UI/system-ui; os demais temas conservam suas famílias anteriores, incluindo os títulos serifados de Atelier e Medieval. O corpo usa 1rem; metadados têm piso equivalente a 12px; texto auxiliar e código usam normalmente 13px. O diff mantém Consolas/Cascadia Code e rolagem dentro do próprio componente.

A tipografia usa unidades relativas. O slider de General permite **80–200%, em passos de 5%**, com valor exibido e ação **Reset to 100%**. A escala inicial é 100%; a 200%, a base de 14px produz corpo de 28px e amplia rótulos/controles. Essa é uma preferência de fonte da interface, independente do zoom do navegador e da escala física do Windows. A antiga escolha binária normal/ampliado pertence ao histórico.

Controles comuns têm altura mínima de **36px em Compact, padrão**, e 44px em Comfortable; a entrada pode conservar alvos de 44px. Texto ampliado, múltiplas linhas e mensagens longas podem aumentar a altura. Botões podem quebrar o rótulo; campos mantêm largura dentro do painel. Radio/checkbox têm controle nativo de 16px e um label ou cartão maior para interação. Densidade altera espaço, não reduz a fonte.

O foco usa contorno de 2px com afastamento de 3px. Seleções têm combinação de fundo, borda/indicador e estado textual. A fila conserva indicador de tarefa selecionada; temas conservam radio marcado; fontes de contexto conservam `aria-pressed`. Aprovação, falha, bloqueio e ausência de confirmação continuam com seus rótulos específicos; o CSS não altera o significado dos estados.

Movimento é reduzido com `prefers-reduced-motion`. Em cores forçadas, contornos e bordas passam a usar cores de sistema. Os modos foram ativados e conferidos no Chrome/Playwright em 09/10; a configuração real do Windows permanece pendente.

## Componentes e reflow

| Região | Contrato da revisão vigente |
| --- | --- |
| Conversas | Contexto/destino da sessão ficam explícitos e quebram texto dentro do painel. Histórico e rascunho pertencem à sessão; a conversa ativa conserva histórico antes do composer. Não reintroduzir os dois seletores globais de projeto/conversa. |
| Composer | `#chat-message` mantém rascunho/seleção por conversa. Envio e ações explícitas focam/trazem o campo à vista; eventos passivos preservam foco/cursor/posição. Enter envia, Shift+Enter quebra linha e composição não envia. |
| Entrada | Logo, pergunta, três cartões de início, sugestões rápidas e composer, nessa ordem. A 100%, cabe em 1440×900, 1280×720 e 1024×768 nos três idiomas; mobile e ampliação conservam reflow/rolagem natural. |
| Projeto/sessões | Sessions começa à esquerda; projetos expansíveis, sessões independentes, busca e comando principal New session único. Criar/associar preserva rascunho, identidade e contexto da sessão. |
| Docks/abas | Somente esquerda/direita; Navigation e Work começam à direita. Painéis no mesmo lado compartilham abas. Blur destaca a escolha dos lados e desaparece ao encerrar/cancelar. Menu/teclado e reset mantêm alternativa ao arraste. |
| Settings | Janela modeless centralizada até 940 × 720px, limitada ao viewport; pode ser movida/redimensionada. Clique externo fecha preservando rascunhos para reabertura; salvamento de perfil/instruções permanece explícito. |
| Trabalho | `.workspace` contém fila, separador e detalhe. A largura solicitada da fila vem de `--worklist-width`, limitada também pela largura disponível. |
| Separador | `.panel-resizer` ocupa 8px, indica arraste e tem foco próprio. O comportamento de ponteiro/teclado e a persistência pertencem ao JavaScript. |
| Fila recolhida | `.queue-collapsed` remove fila e separador, deixando o detalhe ocupar a região. |
| Telas menores | Docks laterais e conteúdo refluem conforme largura, sem criar destino inferior. Configurações continuam acessíveis pela engrenagem; idioma fica em General e aparência em Themes. Rolagem local/natural mantém ações alcançáveis. |
| Escala de texto | Slider 80–200%, passo 5%, padrão 100%, em General. Texto ampliado reorganiza regiões e conserva modo de foco; limites específicos devem ser verificados na composição atual. |
| Contexto | Fontes em cartões com caminho, versão e motivo em linhas próprias. Prévia de código quebra linhas e tem rolagem local; seleção e opções futuras preservam labels legíveis. |
| Conexões | Subscription ou API antes do provedor; campos condicionais ao backend. Inspector, catálogo, consentimento e histórico quebram textos longos e agrupam ações. Cadastro/descoberta/execução são estados distintos. |
| Revisão | Diff preserva rolagem local. Inspector refluído abaixo do código até 1280px ou com texto ampliado. |
| Tela ampla | Conteúdo limitado a 1600px, parágrafos a 78ch e galeria com quatro colunas a partir de 1650px. |

Densidade Compact muda espaçamento e alturas; no desktop conserva a divisão lateral e o redimensionamento. As antigas composições de fila e docking inferior pertencem às verificações históricas abaixo, não definem destinos válidos da revisão atual.

## Verificações históricas

As rodadas a seguir antecedem os sete refinamentos atuais. Preservam contagens, cenários e limites daquela versão; não afirmam que o slider, Settings centralizada ou novo padrão de docking já passaram a rodada final. Consulte o [status](../DEVELOPMENT_STATUS.md) e a [revisão do shell](d1-shell-revision.md) para a evidência vigente quando consolidada.

**Refinamento mobile de 10/10:** catálogo de **1.012 chaves** e check aprovado; **27/27 casos focados** cobriram entrada e idiomas, incluindo quatro regressões novas. O check direto passou sete composições: campo inteiro na primeira dobra de 390×844 e 320×900 em EN/PT-BR/ES, tamanho normal; reflow a 200% sem overflow horizontal, com rolagem natural. Quatro capturas `d1-entry-refinement-*` foram examinadas. A suíte completa final passou **77/77 em uma única rodada de 2,2 minutos**, sem falhas ou casos ignorados; resultados, tentativas anteriores, manifesto e limites ficam no [registro progressivo](d1-progressive-experience.md#refinamento-da-entrada-mobile--1010). O botão Send pode exigir rolagem; não se afirma que o formulário inteiro cabe no viewport ou que conforto humano foi avaliado.

**Aplicação visual de 10/10:** 18/18 cenários aprovados em uma única rodada de 40,8 segundos, na prévia [4179/index.html](http://127.0.0.1:4179/index.html): 17 casos do app e um percurso do site. Incluem teclado e persistência em oito temas/seis larguras, foco/cursor, diff, texto ampliado, Windows/WSL, Enter/composição, movimento reduzido, cores forçadas e Unicode a 320px. O contraste real do botão Nova tarefa e quatro duplas principais, resolvidos no navegador, passou o piso de 4,5:1 nos oito temas. `npm.cmd run check` e `git diff --check` passaram. A rodada focada não equivale a repetição integral dos testes do aplicativo ou certificação de acessibilidade.

O QA daquela aplicação visual renderizou sete capturas `app-identity-*`, ignoradas pelo Git: welcome, conversa, trabalho, conexões, aparência e welcome Institutional em 1440×1000; welcome Studio em 390×844. Não houve overflow, erros de página ou requisições/respostas falhas no percurso de captura. As sete telas foram revisadas sem falhas materiais. O aplicativo não contém canvas ou cenário de montanhas; essas camadas continuam exclusivas do site. Naquela versão, navegação e introdução colocavam o composer abaixo da primeira dobra em 390px; o refinamento mobile posterior descrito acima resolve essa altura nas composições verificadas, mantendo o piloto humano pendente.

**Histórico das rodadas de 08–09/10:** a verificação usou o preview local e Chrome headless no Windows, sem agente, autenticação ou leitura de repositório pela interface.

- Teste Playwright existente de legibilidade: passou nos sete temas e larguras 1920, 1280, 1024, 720, 390 e 320px, nas telas de trabalho e revisão. Inclui ausência de overflow da página, quatro pares de contraste e ausência de erros/requisições externas durante o teste.
- Matriz adicional de CSS: 144 combinações no Studio, com duas densidades, texto normal/200%, as seis larguras acima e telas de trabalho, revisão, atenção, conexões, preferências e entrada. Nenhuma apresentou overflow horizontal do documento ou erro de página; o corpo ampliado foi medido em 28px.
- Contraste adicional: 105 combinações nos sete temas, cobrindo texto principal/secundário/metadados, superfícies elevadas, fundos de diff, status, foco e bordas de campos. Os pares de texto passaram o piso de 4,5:1; foco e borda passaram o piso de 3:1. Valores foram resolvidos no navegador, inclusive o `color-mix()` da borda.
- Controles: o botão Nova tarefa mediu 44px confortável e 36px compacto com texto normal.
- Interação de layout: passaram teclado End/seta esquerda no separador, persistência da largura após reload, recolher/mostrar fila, persistência de densidade/texto, modo de foco em 200% e ausência de overflow nos modais de contexto/conexão a 320px.
- Inspeção visual: capturas Studio de trabalho, entrada, preferências, inspector de conexão, contexto e texto a 200% no desktop/celular. Capturas temporárias locais apoiaram a inspeção; não substituem um piloto.
- Continuação em 09/10: cinco [testes reproduzíveis](../../prototypes/desktop/tests/d1-accessibility.spec.mjs) passaram, com foco após entrada/cadastro, movimento reduzido, cores forçadas e textos Unicode nos limites dos campos/paths Windows e WSL a 320px. O toast ficou sem transição/animação; foco e seleção conservaram contorno de 2px; campos/texto conservaram distinção do fundo. Chrome 154.0.8037.98 headless, Playwright 1.64.0, Node 24.19.0 no Windows. A suíte completa passou 24 cenários.
- Incremento por conversa em 09/10: os 24 cenários foram retestados junto aos 12 novos; **36/36 passaram**. Chat com texto de 2000 caracteres, texto CSS 200%, 320px/tela ampla, foco, rascunho e teclado exercitados. Capturas em 1440×960, 390×844 e 320×720 foram examinadas; revisão adicional entre 320–1920px não mostrou overflow nas condições usadas. Media queries emuladas conservam foco/controles; leitor de tela, zoom e configurações Windows continuam pendentes.

O script adicional e suas imagens foram gerados no diretório temporário do sistema, sem adicionar outra ferramenta ao projeto. A cobertura de 200% em seis views se refere ao Studio; os outros temas receberam a cobertura existente de layout normal e a verificação adicional de contraste.

## Limites das rodadas históricas e da etapa atual

Nos registros acima não foram executados: piloto com usuários, leitor de tela, zoom físico do navegador a 200%, escalas/contraste configurados no Windows, logs/diffs extensos, embalagem Tauri ou avaliação visual do aplicativo nativo em Windows/WSL. As media queries de cores forçadas e movimento reduzido foram exercitadas pelo navegador, conforme descrito. Resize por teclado/ponteiro, fonte ampliada manual e persistência passaram naquela [suíte D1](d1-delivery.md#verificação-técnica); não estender esses resultados à rodada atual. O limite histórico de 820px dos inspectores não define a nova janela Settings, cujo tamanho inicial máximo é 940 × 720px.

Os critérios de produto e as interações de domínio continuam na matriz D1. Esta folha de estilos não implementa autenticação, execução, seleção real de modelos ou integração Git.
