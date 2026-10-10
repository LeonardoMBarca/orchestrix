# Workspace shell — sessões, painéis e Settings

Contrato de interface da revisão D1 de **10 de outubro de 2026**, incluindo os sete refinamentos seguintes de navegação e configurações. A identidade visual aprovada é mantida; a aprovação da nova organização e o piloto de uso ainda estão pendentes. Este documento orienta HTML atual e futura migração para Desktop, sem aprovar gates ou criar outra stack.

## Entrada centrada na conversa

1. Marca transparente acima do título “What shall we create today?”.
2. Três cartões de início que apontam a conta, projeto e orientação.
3. Sugestões rápidas “Fix a problem”, “Create a feature” e “Understand my code”.
4. Composer, destino/contexto da sessão e envio.

Não acrescentar vários parágrafos de recepção ou um segundo seletor global de projeto/conversa. A pessoa deve conseguir começar com uma sessão independente e criar organização quando precisar. A aparência Studio usa navy e índigo sem o fundo de montanhas do site. A primeira entrada não preenche contas, projetos ou tarefas fictícios.

## Sessão e projeto

Sessions é a superfície principal para navegação entre conversas e começa à esquerda. Há um único comando principal **New session** nessa superfície; não duplicá-lo no cabeçalho, na navegação e em cartões. Ações contextuais de um projeto podem indicar explicitamente a criação dentro daquele projeto. Projetos são grupos expansíveis com contexto e diretório compartilhados; sessões independentes ficam acessíveis na mesma lista. Criar projeto e associar uma sessão existente precisam distinguir a intenção sem obrigar a pessoa a aprender dois dropdowns.

Selecionar uma sessão preserva o rascunho da anterior e mostra o destino/contexto ativo. Associar preserva identidade e histórico da sessão. Truncar nomes longos com acesso ao texto completo quando necessário; não permitir que um nome invada o ícone, indicador, borda ou ação da linha. Busca de sessões ocupa espaço legível, com label e foco; não prometer pesquisa em arquivos que não foram indexados.

## Docking e abas

| Painel | Papel | Disposição inicial |
| --- | --- | --- |
| Navigation | Entradas do workspace, Studio e utilitários | Direita |
| Sessions | Projetos e conversas | Esquerda |
| Work | Tarefas e próximos passos da sessão | Direita, quando há trabalho |

**Esquerda e direita são os destinos válidos nesta revisão.** O dock inferior da versão anterior foi retirado. Múltiplos painéis no mesmo lado viram abas; selecionar uma mantém os demais disponíveis. A conversa conserva a prioridade de espaço. Reflow deve reorganizar docks em telas menores, com rolagem vertical previsível e sem esconder ações essenciais fora de alcance.

- A alça/título e as abas permitem arraste com destinos laterais destacados. Durante a escolha do lado, o fundo recebe blur temporário para tornar os destinos claros; encerrar ou cancelar a escolha remove o efeito.
- Menu de posição oferece Move to left/right, com o mesmo escopo do arraste.
- Na alça, setas esquerda/direita movem o painel; Enter/Space abre o menu.
- Nas abas, esquerda/direita e Home/End mudam a seleção; Shift + setas esquerda/direita muda o dock.
- Separadores têm nome acessível, orientação, faixa de tamanho e alternativa por teclado.
- Escape cancela arraste/menu; Reset layout restaura a disposição inicial.

Mover ou redimensionar não recria o composer, não substitui a conversa e não altera configuração de uma execução. A posição e tamanho são preferências de UI, não estado de execução.

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

Fontes atuais: [index.html](../prototypes/desktop/index.html), [app.js](../prototypes/desktop/app.js), [experience.js](../prototypes/desktop/experience.js), [docking.js](../prototypes/desktop/docking.js), [settings.js](../prototypes/desktop/settings.js), [styles.css](../prototypes/desktop/styles.css), [experience.css](../prototypes/desktop/experience.css), [settings.css](../prototypes/desktop/settings.css) e [help.html](../prototypes/desktop/help.html). Usar [tokens](tokens/README.md), [componentes](components/README.md) e [catálogo textual](../docs/design/visual-asset-catalog.md) como base para novas telas.

Verificar teclado, mover/agrupamento dos painéis laterais, blur removido ao cancelar, foco e rascunho preservados, Settings centralizada e fechamento externo, idiomas/temas, slider e nomes longos em telas compactas. Medidas e aprovações humanas devem ser registradas na [revisão D1](../docs/design/d1-shell-revision.md), no [piloto](../docs/design/d1-pilot.md) e no [status](../docs/DEVELOPMENT_STATUS.md). Evidências da composição anterior, que tinha Sessions à direita e dock inferior, continuam históricas; não certificam estes refinamentos.

## Descoberta ao cadastrar

Salvar uma API inicia discovery de catálogo pelo host, com feedback em Settings → Accounts e acesso a View resources. O resumo diferencia loading, complete/partial, erro e falta de permissão. Detalhes por modelo mostram recursos com provenance e unknown explícito, sem exigir configuração técnica. Usar controles nativos compartilhados e [o contrato de discovery](../docs/design/resource-discovery-increment.md). Lookup de catálogo não habilita execução ou quota automaticamente.

Cadastro e recheck também iniciam o [diagnóstico por conexão](../docs/design/connection-diagnostics.md). Connections apresenta o resumo mais recente; Settings → Accounts oferece View diagnostics. O relatório é uma janela fechável, conserva o chat e distingue suporte de disponibilidade e permissões. Cadastro a partir do chat termina com um aviso discreto que permite abrir o resultado sem mudar a conversa. Sem adapter/identidade de runtime, os checks ficam bloqueados ou desconhecidos. Cancelamento e troca de configuração invalidam respostas tardias; recheck não usa resultados antigos como fatos atuais.
