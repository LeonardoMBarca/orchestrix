# Workspace shell — sessões, painéis e Settings

Contrato de interface da revisão D1 atualizado em **12 de outubro de 2026**, com [barra fixa de atividades e arquivos](../docs/design/activity-shell-revision.md). A identidade visual aprovada é mantida. O responsável reportou resultado favorável do piloto, exceto no docking, cuja correção exige reteste humano; o gate permanece aberto. Este documento orienta HTML atual e futura migração para Desktop, sem aprovar gates ou criar outra stack.

## Entrada centrada na conversa

1. Marca transparente acima do título “What shall we create today?”.
2. Três cartões de início que apontam a conta, projeto e orientação.
3. Sugestões rápidas “Fix a problem”, “Create a feature” e “Understand my code”.
4. Composer com menu `+` de contexto, modelo orquestrador da sessão, modo de trabalho e envio.

Não acrescentar vários parágrafos de recepção ou um segundo seletor global de projeto/conversa. A pessoa deve conseguir começar com uma sessão independente e criar organização quando precisar. A aparência Studio usa navy e índigo sem o fundo de montanhas do site. A primeira entrada não preenche contas, projetos ou tarefas fictícios.

## Sessão e projeto

Sessions é a superfície principal para navegação entre conversas e começa à direita. Há um único comando principal **New session** nessa superfície, com **New project** textual ao lado; não duplicá-lo no cabeçalho, na navegação e em cartões. Cada projeto tem uma ação compacta de chat com `+` na mesma linha do nome, independente da expansão do grupo. Projetos são grupos expansíveis com contexto e diretório compartilhados; sessões independentes ficam acessíveis na mesma lista. Criar projeto e associar uma sessão existente precisam distinguir a intenção sem obrigar a pessoa a aprender dois dropdowns.

Selecionar uma sessão preserva o rascunho da anterior e mostra o destino/contexto ativo. Associar preserva identidade e histórico da sessão. Truncar nomes longos com acesso ao texto completo quando necessário; não permitir que um nome invada o ícone, indicador, borda ou ação da linha. Busca de sessões ocupa espaço legível, com label e foco; não prometer pesquisa em arquivos que não foram indexados.

O composer não mostra “Discussing”. **Attach to project**, documentos e diretório/contexto ficam no menu `+`, com fechamento externo/Escape e navegação por teclado. O modelo da sessão vem do catálogo de uma conexão permitida, mantendo sua identidade (IDs iguais em contas diferentes não são a mesma escolha); automático usa as preferências existentes. API é excluída em Subscription Only. Catálogo e preferência não comprovam autenticação ou execução. O modo começa em Development e pode registrar os seis propósitos existentes; isso não implementa os motores especializados. Modelo/modo e documentos explicitamente selecionados entram no snapshot das próximas tentativas, sem modificar as anteriores.

Documentos de texto UTF-8 selecionados pelo usuário ficam associados à sessão, com nome/remover, até 10 arquivos de 100 KB e 500 KB somados neste estudo. Cancelar ou falhar a leitura preserva o rascunho e as referências anteriores. Arquivos abertos no editor não entram automaticamente no contexto. PDFs/binários e extração de outros formatos não são implementados neste incremento. Persistência em navegador não substitui o armazenamento de produção.

Enviar por Enter ou pelo botão leva a área central ao limite inferior após renderizar. O registro de resposta e o evento futuro `orchestrix:response-complete`, com `detail.sessionId`, fazem o mesmo somente para a conversa ativa e visível, sem roubar foco ou rolar outra sessão/Files. O evento é um contrato de UI; não é evidência de integração de runtime.

## Barra fixa e menus

A barra de atividades fica na borda esquerda, independente dos painéis. A ordem inicial é **Chat, Files, Studio, Connections**; **Help, Settings e Profile** permanecem na base. Os quatro primeiros itens podem ser reordenados, mantendo nomes acessíveis e uma alternativa ao arraste. A barra não muda de lado, não se recolhe com os docks e permanece um caminho para reabrir uma atividade.

Chat abre diretamente a conversa, sem um painel lateral próprio com atalhos de sessão, diretório ou acompanhamento. A barra fixa permanece disponível e Sessions continua sendo um painel independente, que pode ficar aberto ou recolhido conforme a preferência da pessoa. Files mostra a árvore autorizada e abre leitura/edição leve no centro. Studio reúne as superfícies técnicas existentes, sem obrigá-las no primeiro uso. Connections conserva cadastro, recursos e diagnósticos. Settings continua modeless; Profile encaminha ao perfil, sem duplicar sua edição. Help abre a página externa.

A barra superior oferece **File, Edit, View e Help**, com ações pertinentes ao aplicativo e atalhos quando disponíveis. **File** reúne **Open file, Open folder e New file**; não recebe criação de sessão/projeto nem comandos globais de salvar/fechar arquivo. Sessões pertencem a Sessions; salvar e fechar pertencem ao arquivo aberto. A barra não acrescenta Run, debugger ou terminal de IDE. Ações indisponíveis devem explicar sua condição ou ficar desabilitadas; não apresentar comandos sem efeito. No cabeçalho da área central, a busca se expande para preencher o espaço disponível entre os demais controles, sem largura máxima que deixe uma faixa vazia. Em telas compactas, os controles mantêm sua legibilidade e não provocam overflow horizontal.

## Docking e abas

| Painel | Papel | Disposição inicial |
| --- | --- | --- |
| Navigation | Conteúdo lateral de Files, Studio ou Connections; não aparece para Chat | Esquerda, 224px, quando necessário |
| Sessions | Projetos e conversas | Direita, 280px |
| Work | Tarefas e próximos passos da sessão | Direita, quando há trabalho |

**Esquerda e direita são os destinos válidos nesta revisão.** O dock inferior da versão anterior foi retirado. Múltiplos painéis no mesmo lado viram abas; selecionar uma mantém os demais disponíveis. A conversa conserva a prioridade de espaço. Reflow deve reorganizar docks em telas menores, com rolagem vertical previsível e sem esconder ações essenciais fora de alcance.

- A alça/título e as abas permitem arraste com destinos laterais destacados. Durante a escolha do lado, o fundo recebe blur temporário para tornar os destinos claros; encerrar ou cancelar a escolha remove o efeito.
- Menu de posição oferece Move to left/right, com o mesmo escopo do arraste.
- Na alça, setas esquerda/direita movem o painel; Enter/Space abre o menu.
- Nas abas, esquerda/direita e Home/End mudam a seleção; Shift + setas esquerda/direita muda o dock.
- Separadores têm nome acessível, orientação, faixa de tamanho e alternativa por teclado.
- Arrastar a divisória reduz a largura até recolher o lado; a borda permanece disponível para expandi-lo novamente. Restaurar usa a última largura útil. Não exigir um botão permanente de minimizar nem ocultar a barra fixa.
- Escape cancela arraste/menu; Reset layout restaura a disposição inicial.

Mover ou redimensionar não recria o composer, não substitui a conversa e não altera configuração de uma execução. A posição e tamanho são preferências de UI, não estado de execução.

Files, Studio e Connections compartilham **um único Navigation/Workspace**. Depois de mover esse painel, os ícones fixos trocam/abrem o conteúdo no lado salvo. Uma atividade selecionada atrás de Sessions/Work não equivale a um painel visível: clicar deve ativá-la, sem recolher todo o lado. `aria-expanded` acompanha essa visibilidade. Em mobile, mover o painel visível transfere o overlay ao destino.

## Arquivos e edição leve

Files apresenta somente conteúdo de uma pasta concedida pelo usuário ou de arquivos importados. Um path salvo no projeto não concede leitura/escrita. Quando o navegador oferece handles de diretório e permissão de escrita, salvar pode alterar o arquivo autorizado; sem essa integração, importar permite ler e editar uma cópia, com **Save a copy** em vez de fingir gravação na origem. Cancelamento, permissão negada e pasta ausente conservam os rascunhos.

A entrada permite **Open file** para selecionar um arquivo avulso, sem obrigar a abrir uma pasta. **New file** pede nome com extensão e destino: a pasta autorizada atual ou uma subpasta elegível; sem pasta aberta, a pessoa precisa selecionar a pasta de destino antes de criar no disco. A criação exige uma ação explícita e não sobrescreve um arquivo existente. O navegador deve diferenciar criação efetiva com acesso de escrita de um rascunho/cópia quando essa integração não estiver disponível. Um buffer vazio ou editado em memória não é prova de que um arquivo foi gravado.

A árvore e o editor devem permitir abrir texto, fazer ajustes pequenos como um `.env` e visualizar Markdown produzido pelos agentes. A prévia não executa HTML/scripts do documento. O arquivo aberto e seus rascunhos não desaparecem ao trocar de atividade, sessão ou layout; mudanças não salvas precisam de uma saída explícita. Abrir um arquivo não o inclui automaticamente no contexto de um agente nem envia segredos ao provedor.

No Desktop de M2, uma bolinha no nome/aba identifica alterações não salvas; **Ctrl+S** salva o arquivo ativo. Fechar um arquivo alterado oferece **Save, Discard e Cancel**: salvar precisa terminar com sucesso antes de fechar, descartar abandona a edição e cancelar mantém o arquivo aberto. Esse ciclo completo pertence à implementação Desktop; os mecanismos existentes do navegador não o certificam. Escrita coordenada com o Core deve conferir versão/base e conflitos com workers; uma alteração manual invalida as evidências que dependiam da versão anterior. O editor leve não oferece execução, debugger, ambiente integrado de testes, language server ou gestão de extensões. Os checks do Core seguem separados como evidências de trabalho orquestrado.

## Settings modeless

Settings abre pelo botão de engrenagem, **centralizada, com tamanho inicial de até 940 × 720px**, limitado pela janela disponível. Continua flutuante, fechável, movível e redimensionável. Ela não navega para outra tela nem torna o chat inerte. Use semântica de diálogo não modal, foco inicial útil, foco visível e retorno a um elemento válido ao fechar. Não inserir um bloqueio de teclado que impeça voltar ao composer.

Clicar fora fecha Settings e permite usar a superfície escolhida. Fechar pelo botão ou pelo teclado também preserva o rascunho de nome/instruções para reabertura durante a sessão do app; isso não equivale a salvá-lo para próximas sessões ou reinícios. Perfil e instruções mantêm salvamento explícito. Tema, idioma, densidade e escala aplicam-se imediatamente. O blur da escolha de docking não é uma camada permanente das configurações.

| Seção | Conteúdo e comportamento |
| --- | --- |
| General | Nome de perfil, idioma, densidade e escala de texto. **Compact é o padrão**; Comfortable é opcional. Escala usa slider **80–200%, em passos de 5%, padrão 100%**, com valor visível e restauração a 100%. Perfil salva explicitamente; idioma/UI aplicam conforme os controles. |
| Accounts | Conexões, status, modalidade, percentual/renovação e crédito/gasto apenas quando observados. |
| Themes | Galeria, Studio padrão e restauração. Aplica imediatamente. Nomes de temas continuam em inglês. |
| Conversation | Personal instructions, com salvamento explícito para próximas tentativas. |
| Orchestration | Preferências de conexão, modelo, raciocínio e contexto dentro das capacidades reais. |
| Panel layout | Posição dos painéis e Reset layout, como alternativa ao arraste. |

English é padrão; Português e Español são opções do app. Idioma e Themes pertencem a Settings, sem controles duplicados permanentemente expostos na navegação. Conteúdo do usuário, código, paths e nomes não são traduzidos por troca de idioma.

## Capacidade observada e linguagem de produto

O texto normal apresenta o produto, sem rótulos de demo/exemplo/simulação. Isso não permite converter dados desconhecidos em sucesso. Uma configuração salva não equivale a autenticação; modelo solicitado não equivale a modelo efetivo; número de sessões não equivale a quota adicional.

Usage exige origem observada do provider/runtime e valores válidos. Mostrar **Not reported** quando quota, renovação, crédito ou gasto não forem expostos. Fixtures não podem gerar barras percentuais ou saldo como se fossem telemetria. O wizard começa por **Subscription ou API** e só depois mostra os provedores e campos da modalidade. Subscription Only exclui conexões API do roteamento/fallback, inclusive uma conexão cadastrada ou com crédito. Ver [contrato de API](../docs/design/api-connection-contract.md) e [pesquisa oficial](../docs/research/api-provider-discovery-2026-10-10.md).

A abertura normal não injeta cenários de estudo. Controles que fabricam limites, estados ou resultados ficam exclusivos do sinal de teste e não são funções de produção. A durabilidade do protótipo em `localStorage` ajuda a revisar o fluxo no mesmo navegador e não substitui M1.

## Ajuda e fontes

Help abre outra página no navegador, sem virar aba de conteúdo do app. Usar destino externo com `target="_blank"` e `rel="noopener"` no HTML; a integração Desktop deverá usar o mecanismo de abertura externa apropriado. O centro de ajuda público permanece em inglês, com busca, diagramas locais e vídeos apenas quando publicados.

Fontes atuais: [index.html](../prototypes/desktop/index.html), [app.js](../prototypes/desktop/app.js), [experience.js](../prototypes/desktop/experience.js), [docking.js](../prototypes/desktop/docking.js), [activity.js](../prototypes/desktop/activity.js), [activity.css](../prototypes/desktop/activity.css), [files.js](../prototypes/desktop/files.js), [files.css](../prototypes/desktop/files.css), [settings.js](../prototypes/desktop/settings.js), [styles.css](../prototypes/desktop/styles.css), [experience.css](../prototypes/desktop/experience.css), [settings.css](../prototypes/desktop/settings.css) e [help.html](../prototypes/desktop/help.html). O [registro da barra de atividades](../docs/design/activity-shell-revision.md) reúne o escopo da revisão de 12/10. Usar [tokens](tokens/README.md), [componentes](components/README.md) e [catálogo textual](../docs/design/visual-asset-catalog.md) como base para novas telas.

Verificar teclado, ordem da barra fixa, recolhimento/restauração e agrupamento dos painéis laterais, blur removido ao cancelar, foco e rascunho preservados, acesso e gravação explícitos em Files, Settings centralizada e fechamento externo, idiomas/temas, slider e nomes longos em telas compactas. Medidas e aprovações humanas devem ser registradas na [revisão atual](../docs/design/activity-shell-revision.md), no [piloto](../docs/design/d1-pilot.md) e no [status](../docs/DEVELOPMENT_STATUS.md). Evidências de composições anteriores continuam históricas; não certificam estes refinamentos.

## Proporções do shell

O refinamento de proporções de 11/10 reduz barras, espaçamento lateral, título/cartões e composer. A abertura mantém a ordem e as funções existentes; a caixa de mensagem começa com duas linhas, cresce proporcionalmente à escala de texto e conserva resize vertical. O label “Message” permanece acessível, sem aparecer na composição. A base de14px e as larguras/posições de painéis escolhidas pela pessoa são preservadas.

Na entrada desktop, o composer termina 16px acima do fim da área central. Os três cartões ficam mais acima, separados das sugestões por `clamp(28px,5vh,48px)`; em mobile, a separação é de 24px. As sugestões permanecem junto ao campo. A marca e o título ocupam o espaço restante com layout flexível, sem sobrepor controles. Mobile mantém fluxo natural; texto ampliado pode exigir rolagem. Conversas em andamento conservam seu layout.

## Descoberta ao cadastrar

Salvar uma API inicia discovery de catálogo pelo host, com feedback em Settings → Accounts e acesso a View resources. O resumo diferencia loading, complete/partial, erro e falta de permissão. Detalhes por modelo mostram recursos com provenance e unknown explícito, sem exigir configuração técnica. Usar controles nativos compartilhados e [o contrato de discovery](../docs/design/resource-discovery-increment.md). Lookup de catálogo não habilita execução ou quota automaticamente.

Cadastro e recheck também iniciam o [diagnóstico por conexão](../docs/design/connection-diagnostics.md). Connections apresenta o resumo mais recente; Settings → Accounts oferece View diagnostics. O relatório é uma janela fechável, conserva o chat e distingue suporte de disponibilidade e permissões. Cadastro a partir do chat termina com um aviso discreto que permite abrir o resultado sem mudar a conversa. Sem adapter/identidade de runtime, os checks ficam bloqueados ou desconhecidos. Cancelamento e troca de configuração invalidam respostas tardias; recheck não usa resultados antigos como fatos atuais.
