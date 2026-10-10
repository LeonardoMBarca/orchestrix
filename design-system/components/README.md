# Componentes e composição

Referência da identidade aprovada em **10 de outubro de 2026**, atualizada com os refinamentos de navegação e Settings. Use estes padrões nas próximas telas do Orchestrix. A organização revisada ainda depende de avaliação; os detalhes de domínio permanecem nos [contratos D1](../../docs/design/d1-scenario-matrix.md), e o código vivo determina as medidas finais.

## Começar pela conversa

O percurso é **site → aplicativo acolhedor → conversa → detalhes conforme a necessidade**. A página inicial apresenta marca, pergunta curta, três cartões de início, sugestões rápidas e composer, nessa ordem. Conectar conta e organizar projeto ficam acessíveis sem exigir que a pessoa configure toda a orquestração. A primeira entrada oferece uma sessão independente, sem contas/projetos/tarefas fictícios preenchidos. A navegação técnica do Studio começa recolhida.

- Sugestões preenchem o composer e devolvem o foco; não enviam o pedido nem iniciam trabalho.
- O envio revela o trabalho relacionado e sua próxima ação. Abrir o detalhe permite inspecionar o Studio; revelar uma tela não executa uma tarefa.
- O composer continua sendo uma entrada útil depois que os cartões de trabalho aparecem. Mensagens, objetivo e próximo passo precedem modelo, reasoning, contexto e detalhes da tentativa.
- Studio é o **tema padrão** e também o nome da área de controles técnicos. Selecionar um tema não muda o nível de detalhe da experiência.

Fontes: [experience.js](../../prototypes/desktop/experience.js), [experience.css](../../prototypes/desktop/experience.css), [contrato do shell](../workspace-shell.md), [revisão D1](../../docs/design/d1-shell-revision.md) e [testes de sessões](../../prototypes/desktop/tests/session-revision.spec.mjs).

## Componentes a reutilizar

| Componente | Padrão a preservar | Fonte |
| --- | --- | --- |
| Marca e cabeçalho | Símbolo transparente e texto “Orchestrix”; cores e superfícies compartilhadas, escala adequada ao contexto. O app mantém seu cabeçalho funcional; a composição editorial ampla pertence ao site. | [index.html](../../prototypes/desktop/index.html), [website.html](../../prototypes/desktop/website.html), [experience.css](../../prototypes/desktop/experience.css) |
| Navegação | Conversa e conexões acessíveis na entrada; Studio expansível para trabalho, revisão e preferências. Indicar a página atual além da cor. Preservar a ação de voltar ao início sem apagar o trabalho. | [index.html](../../prototypes/desktop/index.html), [app.js](../../prototypes/desktop/app.js) |
| Settings e ajuda | Engrenagem abre Settings centralizada, até 940 × 720px, flutuante/modeless. Clique fora fecha, preservando rascunhos para reabertura; perfil/instruções continuam com salvamento explícito. Idioma em General e aparência em Themes. Help abre página externa com busca e tutoriais; não ocupa uma aba de conteúdo do app. | [settings.js](../../prototypes/desktop/settings.js), [settings.css](../../prototypes/desktop/settings.css), [help.html](../../prototypes/desktop/help.html) |
| Lista de projetos e sessões | Sessions começa à esquerda e reúne projetos expansíveis, conversas independentes, busca e um único comando principal New session. Ações contextuais podem criar dentro de um projeto; associar preserva histórico e rascunho. Não voltar aos dois dropdowns globais ou duplicar a criação na navegação. | [app.js](../../prototypes/desktop/app.js), [experience.js](../../prototypes/desktop/experience.js), [testes de sessões](../../prototypes/desktop/tests/session-revision.spec.mjs) |
| Docks e abas | Somente esquerda/direita; Navigation e Work começam à direita. Painéis no mesmo lado compartilham abas. A escolha de lado destaca os destinos com blur temporário do fundo. Preservar foco/composer ao mover e oferecer menu/teclado, redimensionamento e reset. | [docking.js](../../prototypes/desktop/docking.js), [contrato do shell](../workspace-shell.md) |
| Conexão | Modalidade Subscription ou API primeiro; provedor/backend depois, com campos essenciais condicionais. Cadastro não declara autenticação, execução ou uso observado. API não é fallback automático de assinatura. | [contrato de API](../../docs/design/api-connection-contract.md), [pesquisa oficial](../../docs/research/api-provider-discovery-2026-10-10.md) |
| Composer e transcript | Campo rotulado, hint de teclado, envio explícito e autoria visível. Enter envia; Shift+Enter cria linha; composição de texto não envia. Preservar rascunho, cursor e foco por conversa. | [experience.js](../../prototypes/desktop/experience.js), [testes de chat](../../prototypes/desktop/tests/project-chat.spec.mjs) |
| Cartão de trabalho | Objetivo, estado por texto e próxima ação. Acesso ao detalhe/revisão permanece ligado à tarefa correta. Não transformar todo cartão em um botão sem identificar sua ação. | [workspace.js](../../prototypes/desktop/workspace.js) |
| Botões | Primário para a ação principal; secundários/links para inspeção e navegação. Ícones acompanham rótulo ou nome acessível. Desabilitado, selecionado e foco são estados distintos. | [styles.css](../../prototypes/desktop/styles.css), [experience.css](../../prototypes/desktop/experience.css), [website.css](../../prototypes/desktop/website.css) |
| Cards e superfícies | Navy em camadas no Studio e no site, bordas discretas, texto claro e acento índigo. Usar os tokens compartilhados e seus aliases semânticos; evitar introduzir uma paleta independente por tela. | [identity.css](../../prototypes/desktop/identity.css), [sistema visual D1](../../docs/design/d1-design-system.md) |
| Formulários e diálogos | Labels explícitos, hints associados ao campo, conteúdo longo com wrap e ações agrupadas. Diálogos mantêm rolagem local e devolvem o foco a uma ação útil ao fechar. | [app.js](../../prototypes/desktop/app.js), [styles.css](../../prototypes/desktop/styles.css), [testes de foco](../../prototypes/desktop/tests/d1-accessibility.spec.mjs) |
| Preferências de aparência | Settings → Themes: galeria com prévia e radio. Studio padrão; Institutional, Atelier, Horizon, Deep Black, Medieval, Forest e Dawn opcionais. General reúne idioma, densidade Compact por padrão e slider de texto 80–200%, em passos de 5%, padrão 100%, como escolhas independentes. | [settings.js](../../prototypes/desktop/settings.js), [styles.css](../../prototypes/desktop/styles.css), [idiomas](../../docs/design/interface-languages.md) |
| Detalhes técnicos | Fila e detalhe redimensionáveis em tela ampla; inspectores, contexto, diff e checks acessíveis conforme a necessidade. Código preserva fonte própria e rolagem dentro do componente. | [workspace.js](../../prototypes/desktop/workspace.js), [styles.css](../../prototypes/desktop/styles.css), [testes de workspace](../../prototypes/desktop/tests/workspace.spec.mjs) |

Projeto, conversa do app e sessão nativa de um agente são conceitos separados. Não rotular uma conversa como se fosse a sessão privada de um provider, nem usar o nome exibido como identidade do trabalho. Revisar/aceitar uma tarefa e aplicar um Run são ações separadas, com os bloqueios e confirmações próprios. O acabamento visual não pode sugerir que uma aprovação já alterou o destino.

## Leitura, teclado e reflow

O app usa base de 14px a 100% e unidades relativas; texto principal é 1rem e metadados têm piso equivalente a 12px nessa escala. Controles comuns seguem `--control-height`: **36px Compact, padrão / 44px Comfortable**; controles da entrada podem conservar 44px. A pessoa escolhe a escala por slider de **80–200%, em passos de 5%, padrão 100%**. A 200%, a base dobra e a altura pode crescer; densidade não reduz a fonte. Não reduzir fonte para fazer uma tradução ou um caminho caber.

O foco do app tem contorno de 2px e afastamento de 3px; o site usa contorno de 3px e afastamento de 5px. Seleção, erro, bloqueio e aprovação combinam texto/ícone com cor. Preserve os ajustes de cores forçadas e movimento reduzido.

Chat e entrada passam a uma coluna conforme a largura; texto ampliado também favorece essa organização. Em telas pequenas, os painéis laterais refluem e o conteúdo ganha rolagem própria; isso não cria um terceiro destino de docking. O separador desaparece quando não há espaço para a divisão. Nomes, caminhos, mensagens e rótulos devem quebrar dentro da sua região; diff e filas podem ter rolagem local, sem alargar o documento.

Na entrada atual, marca/pergunta, cartões e sugestões precedem o composer na ordem do DOM. A conversa ativa mantém seu histórico antes do campo. Redimensionar ou mover painéis preserva o campo, foco, cursor e rascunho. Texto ampliado e telas compactas usam reflow/rolagem natural, sem reduzir fontes nem exigir que todos os controles caibam na primeira dobra. Preservar alvos de pelo menos 44px na entrada confortável e a transferência de foco quando um controle recolhe ou muda de apresentação. As medidas de primeira dobra do incremento mobile anterior são históricas e não certificam a nova composição.

O [sistema visual D1](../../docs/design/d1-design-system.md) registra os breakpoints atuais. Consulte [styles.css](../../prototypes/desktop/styles.css) e [experience.css](../../prototypes/desktop/experience.css) antes de ajustar medidas; este guia não é uma segunda implementação da folha de estilos.

## Texto e idioma

Site, vídeo e centro de ajuda ficam em **inglês**. O app oferece **English (`en`, padrão), Português (`pt-BR`) e Español (`es`)**, com preferência global salva e seleção em **Settings → General → Interface language**. Os temas ficam em **Settings → Themes**. Textos, nomes acessíveis e hints de novos componentes entram nos [catálogos locais](../../prototypes/desktop/locales/README.md).

A redação pública conecta as ideias com naturalidade. Evitar sequências de frases curtas separadas por pontos quando formam uma única ideia; títulos podem quebrar em linhas sem receber ponto final em cada trecho. Parágrafos usam conectores e mantêm pausas onde ajudam a leitura, preservando clareza e significado. A abertura segue “Your idea and your agents in sync”.

Mensagens, rascunhos, nomes de projetos/contas, caminhos, código e resultados existentes permanecem como foram escritos. Nunca traduzir o conteúdo da pessoa para fazer a tela parecer consistente. A preferência da interface não define por si só o idioma de saída de um agente. O [contrato de idiomas](../../docs/design/interface-languages.md) e os [testes de localização](../../prototypes/desktop/tests/localization.spec.mjs) detalham essa separação.

## Ao acrescentar uma tela

1. Reutilize tokens, componentes e hierarquia existentes; identifique a ação principal e mantenha detalhes técnicos acessíveis sem exigir sua configuração na entrada.
2. Confira teclado, foco/cursor, nomes acessíveis e preservação de rascunhos nos percursos alterados.
3. Revise o reflow com nomes/caminhos longos, 320px, tela ampla e texto a 200%, nos idiomas afetados. Verifique os temas quando mudar estilos compartilhados.
4. Use os testes existentes da jornada alterada e registre a evidência visual no [catálogo](../../docs/design/visual-asset-catalog.md). Uma mudança documental não exige repetir a suíte do protótipo; resultados históricos não certificam uma tela nova.

## Selects e recursos de conexões

Manter selects nativos e reutilizar [controls.css](../../prototypes/desktop/controls.css), em vez de estilizar cada tela isoladamente. Cores vêm dos tokens do tema; estados de foco, erro, disabled e opções seguem o mesmo padrão. Em discovery, usar resumo de status e lista de detalhes expansíveis de [resource-view.js](../../prototypes/desktop/resource-view.js)/[resources.css](../../prototypes/desktop/resources.css); supported/unsupported/unknown, origem e parcialidade são parte do conteúdo. Não confundir catálogo com autenticação, execução ou quota. Ver [entrega e evidências](../../docs/design/resource-discovery-increment.md).

O diagnóstico usa [diagnostics-view.js](../../prototypes/desktop/diagnostics-view.js) e [diagnostics.css](../../prototypes/desktop/diagnostics.css): resumo no cadastro, relatório em janela fechável e botão de rechecagem por conexão. Contagem de recursos disponíveis é independente do progresso das verificações. Mostrar suporte, configuração/política e disponibilidade em campos separados, com evidência e timestamp. Não abrir a janela automaticamente nem interromper o rascunho do chat; oferecer um aviso discreto com ação. Detalhes de modelos, escopo e checklist são expansíveis. Reutilizar tokens, idiomas, foco e reflow do Studio, inclusive a 200%. Ver [contrato de diagnósticos](../../docs/design/connection-diagnostics.md).
