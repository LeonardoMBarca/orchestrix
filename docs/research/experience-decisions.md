# Orchestrix — Benchmark de experiência e decisões de workspace

**Data:** 8 de outubro de 2026. **Escopo:** material de OX-D02 e proposta de arquitetura de informação de OX-D03. **Status:** pesquisa documental e inspeção visual de imagens oficiais realizadas. Um [protótipo exploratório navegável](../../prototypes/desktop/README.md) já está disponível; avaliação com usuários e testes próprios dos aplicativos concorrentes não foram realizados nesta pesquisa.

Este documento transforma referências em decisões específicas para o Orchestrix. Complementa o [levantamento de produto](../PRODUCT_AND_UX_RESEARCH.md), o [plano de desenvolvimento](../DEVELOPMENT_PLAN.md) e o [backlog](../DEVELOPMENT_BACKLOG.md). As decisões abaixo são propostas para o protótipo e os gates de D1; não substituem ADRs aceitos nem comprovam usabilidade.

## 1 Evidência e limites do benchmark

Foram lidas fontes oficiais e inspecionadas visualmente, em memória, as três imagens vinculadas abaixo. As imagens retornaram HTTP 200 durante a consulta. São capturas publicadas pelos fornecedores, não capturas de uma instalação nossa. A captura do Linear pertence ao redesign de março de 2024; as versões das outras interfaces não são informadas nos materiais consultados.

| Referência | Fonte oficial e imagem inspecionada | Observação visual/documental | Limite |
| --- | --- | --- | --- |
| Linear | [Relato do redesign](https://linear.app/now/how-we-redesigned-the-linear-ui), [captura After](https://webassets.linear.app/images/ornj730p/production/9b91020243984487b4e0cbe72278dd1acd7f9c57-2352x1380.png) | A captura separa navegação, fila, conteúdo e propriedades. A seleção tem superfície própria; metadados têm menor ênfase que o título. O artigo explica trabalho de alinhamento, hierarquia e densidade. | Material histórico. Não demonstra tempo de uso, acessibilidade ou comportamento atual de toda a aplicação. |
| Raycast | [Action Panel no manual](https://manual.raycast.com/action-panel), [referência de API](https://developers.raycast.com/api-reference/user-interface/action-panel), [captura do painel](https://2922539984-files.gitbook.io/~/files/v0/b/gitbook-x-prod.appspot.com/o/spaces%2F-Me_8A39tFhZg3UaVoSN%2Fuploads%2Fgit-blob-a1bf254ceb4ad9679111270a8a123946d5538877%2Faction-panel.webp?alt=media) | A captura mostra item selecionado, ação principal destacada, grupos e atalhos. Manual documenta busca de ações e operação contextual pelo teclado. | Painel isolado; não comprova adequação a processos longos ou revisão de código. Atalhos do fornecedor não determinam os do Orchestrix. |
| GitHub Desktop | [Histórico de branch](https://docs.github.com/en/desktop/making-changes-in-a-branch/viewing-the-branch-history-in-github-desktop), [captura de commit/arquivo](https://docs.github.com/assets/cb-119758/images/help/desktop/branch-history-file.png) | A captura relaciona commit selecionado, hash, quantidade de arquivos e arquivo selecionado. A documentação descreve selecionar um arquivo para examinar suas alterações. | A imagem é recortada e contém marcação didática; não mostra o diff completo nem autoriza conclusões sobre todo o fluxo de revisão. |

As três referências cumprem papéis distintos: estrutura do workspace, acesso a ações e vinculação do resultado à versão. São referências de interação; não foram tratadas como concorrentes funcionalmente equivalentes ao Orchestrix. Não houve comparação de produtividade, execução de jornadas completas ou teste de usuário.

## 2 Decisões de adotar, adaptar e descartar

| Decisão | Necessidade do Orchestrix | Referência | Proposta e verificação futura |
| --- | --- | --- | --- |
| Adotar navegação previsível | Permanecer orientado durante uma execução longa | Linear | Projeto e seção permanecem identificáveis. Selecionar uma tarefa abre seu workspace sem perder fila, seleção ou filtros. Avaliar localização da tarefa e retorno à revisão. |
| Adaptar fila e detalhe | Distinguir execução normal de intervenção necessária | Linear | Criar a fila **Precisa de você**, com motivo, artefato e próxima ação. Só entra nela trabalho que exige decisão; uma falha com retry automático permitido não vira automaticamente um pedido humano. |
| Adotar ações contextuais descobríveis | Reduzir busca por controles sem exigir decorar atalhos | Raycast | Disponibilizar painel de ações para tarefa, arquivo e finding. Manter a ação principal também visível na tela. Medir descoberta por mouse e teclado. |
| Adaptar seleção por versão | Evitar revisar/aprovar outro resultado sem perceber | GitHub Desktop | Cabeçalho de revisão mostra tarefa, tentativa, snapshot/commit, base e resultado dos checks. Mudança de artefato não substitui silenciosamente a revisão aberta. |
| Descartar reprodução integral da UI de referência | Evitar carregar funções irrelevantes para o primeiro produto | As três | Não reproduzir todos os metadados de issue tracker, recursos de launcher ou operações de cliente Git. Cada elemento precisa servir à jornada assistida. |
| Descartar Enter global para integrar | Aprovação precisa corresponder ao artefato e destino exatos | Adaptação própria | Enter abre o item ou ação selecionada no contexto correto. Aplicação final passa por revisão explícita de destino/base/resultado; nenhuma atualização muda o alvo de um botão focado. |

Não atribuir notas de usabilidade aos fornecedores a partir de imagens. Na comparação das direções do Orchestrix, usar os mesmos dados e tarefas e registrar problemas de orientação, legibilidade, descoberta de ações, compreensão de estado e recuperação.

## 3 Modelo de trabalho visível ao usuário

O centro da experiência é o trabalho que o usuário quer concluir, com tarefas e artefatos vinculados. Conta, provider, modelo e sessão explicam a execução; não precisam se tornar a navegação principal.

```text
Projeto
  Trabalho / objetivo
    Tarefas
      Tentativas e sessões
      Código, diffs, findings e verificações
    Resultado do run
      Pronto para aplicar / aplicado / bloqueado / cancelado
```

No alpha M2, o usuário cria uma tarefa manual com objetivo e critérios. A arquitetura admite um objetivo com várias tarefas, mas a geração automática de plano pertence a M4. O protótipo precisa identificar funções futuras e dados simulados; mostrar um plano não prova que o planner foi implementado.

Uma tarefa concluída na branch interna do run não torna o objetivo entregue. A interface distingue **validado no run**, **pronto para aplicar** e **aplicado ao destino**, conforme o contrato de integração do Core. Indicadores de progresso devem explicar sua unidade e não inventar percentual de conclusão de um objetivo cujo plano ainda pode mudar.

## 4 Navegação e áreas do aplicativo

| Área | Conteúdo principal | Ações e limites |
| --- | --- | --- |
| Entrada / projetos recentes | Repositórios, disponibilidade de runtime e último trabalho | Abrir repositório existente; retomar supervisão. Cadastro sem alterar conteúdo do projeto. |
| Trabalho | Objetivos/runs e tarefas; estado, resumo e última evidência | Criar tarefa, selecionar trabalho, filtrar; visualizar plano/DAG quando disponível. Não listar transcripts como unidade principal. |
| Precisa de você | Aprovações, bloqueios e recuperação que exigem decisão | Abrir resultado, resolver conexão, revisar recuperação. Mostrar motivo e impacto; não apenas badge numérico. |
| Workspace do trabalho | Resumo, código/diff, verificações e atividade | Fornecer contexto, pedir alteração, revisar, corrigir e aplicar. Navegação para finding mantém o snapshot selecionado. |
| Histórico | Runs, decisões, tentativas e eventos consultáveis | Comparar tentativa anterior e atual; inspecionar por que houve retry ou fallback. Logs detalhados ficam sob demanda. |
| Configurações | Conexões, presets e preferências frequentes | Configurar conta/modelo/reasoning/revisão/contexto e limites. Editor avançado da política separado da entrada comum. |

Dentro do workspace, usar quatro destinos identificáveis: **Resumo**, **Código**, **Verificações** e **Atividade**. O plano é acessível a partir do resumo; DAG é uma vista especializada. Código inclui lista de arquivos, viewer e diff; findings abrem a localização da versão em revisão. Verificações mostram evidência configurada pelo Core, sem exigir test explorer ou debugger.

Um inspector contextual recolhível mostra conexão/conta, runtime, modelo e reasoning solicitados/efetivos, Context Pack e versão da política. O usuário pode verificar escolhas automáticas e acessar overrides autorizados. Não pedir seleção manual de todos esses campos a cada tarefa.

## 5 Jornadas próprias para o alpha e seus incrementos

### J1 Abrir projeto e iniciar trabalho — M2

1. Selecionar repositório e verificar Git/runtime/conexão.
2. Criar tarefa: objetivo; critérios de aceite; contexto/arquivos; checks do projeto. Risco e restrições avançadas ficam acessíveis sem sobrecarregar o formulário inicial.
3. Ver o preset efetivo e resolver requisitos que impedem execução. Parâmetro sem suporte é identificado; fallback depende da política.
4. Iniciar tentativa e acompanhar resumo de execução. A tela preserva acesso à ação de interromper e à evidência atual.

**Resultado observável:** o usuário entende o que iniciou, onde está isolado e qual conexão foi selecionada. Objetivo em linguagem natural com decomposição automática é incremento de M4.

### J2 Examinar código e pedir correção — M2

1. Abrir uma tarefa pronta para revisar a partir do trabalho ou da fila de atenção.
2. Ver critérios, tentativa/snapshot, diff e checks correspondentes.
3. Selecionar finding/arquivo e escrever pedido de correção ligado à versão e localização.
4. Acompanhar nova tentativa baseada no artefato rejeitado; acessar comparação e checks atualizados.

**Resultado observável:** completar o fluxo dentro do app, sem copiar prompts ou abrir editor externo. Edição manual leve é uma decisão adicional de D1, com coordenação de escrita e invalidação de verificações; não é condição para esse fluxo assistido.

### J3 Aprovar aplicação — M2

1. Examinar resultado validado no run.
2. Abrir a revisão de aplicação com destino, base, commit/diff e verificações.
3. Confirmar apenas esse resultado. Se a base mudar, apresentar revalidação; conflito preserva artefato e aponta resolução necessária.
4. Exibir confirmação de aplicação e resultado no destino. Aprovação sozinha não aparece como entrega confirmada.

### J4 Entender bloqueio e retomar — M2/M3

1. Ver motivo específico: autenticação expirada, requisito incompatível, falha verificada, limite de retry, rate limit ou recuperação incerta.
2. Abrir detalhes necessários; distinguir problema do runtime, da policy e do código.
3. Resolver conexão, autorizar nova tentativa ou ajustar preferência permitida. Falhas com retry automático mostram a ação do sistema.
4. Manter tentativa anterior e evidências. Em M3, fallback para outra conexão inicia sessão própria; não transplanta uma sessão autenticada.

### J5 Configurar o gosto do usuário — M2, ampliada em M3/M4

1. Escolher preset e preferências frequentes; mostrar valores concretos resultantes.
2. Abrir detalhes de modelo/reasoning/contexto quando necessário.
3. Inspecionar mudança, escopo e quando entra em vigor.
4. Preservar snapshots antigos. Parâmetro não confirmado permanece desconhecido; UI não promete ter aplicado uma opção unsupported.

## 6 Telas, estados e próxima ação

| Tela/estado | Informação indispensável | Ação principal proposta |
| --- | --- | --- |
| Entrada sem projeto | Como abrir trabalho existente e por que não há execução | Abrir repositório |
| Projeto sem conexão | Runtime disponível e autenticação necessária | Conectar/verificar runtime |
| Nova tarefa | Objetivo, critérios, contexto, checks e política efetiva | Iniciar tarefa quando elegível |
| Aguardando execução | Dependência/capacidade que falta; status observado | Inspecionar bloqueio; cancelar se desejado |
| Executando | Tarefa, tentativa, etapa e última evidência | Inspecionar trabalho; pause/cancel sempre acessíveis |
| Checks falhos | Comando, exit code e versão verificada; retry previsto | Pedir correção ou acompanhar retry autorizado |
| Revisão rejeitada | Findings e artefato rejeitado | Solicitar correção |
| Pronto para aplicar | Resultado, base/destino e evidências correspondentes | Revisar aplicação |
| Base alterada / conflito | Resultado preservado e motivo de bloqueio | Revalidar ou tratar conflito conforme contrato |
| Rate limit | Conexão afetada e cooldown confirmado, ou desconhecido | Aguardar/pausar; fallback elegível quando disponível |
| Reconectando / execução incerta | Último estado confirmado e motivo da incerteza | Reconectar/reconciliar; não apresentar sucesso antecipado |
| Cancelado | O que foi interrompido e quais artefatos continuam disponíveis | Inspecionar resultado; criar tentativa autorizada |
| Aplicado | Destino e resultado confirmado | Ver resultado ou criar próximo trabalho |

Pausa de novas admissões e cancelamento de tentativa são ações diferentes e precisam de rótulos distintos. Estado do processo não equivale a progresso semântico da tarefa. Cota desconhecida não vira barra percentual. Eventos novos não roubam foco, não selecionam outro arquivo nem reordenam uma revisão em andamento.

## 7 Direção visual e critérios para o protótipo

As regras abaixo são escolhas iniciais para comparar direções do Orchestrix, não medidas copiadas dos produtos nem tokens já aceitos em ADR.

### Hierarquia, espaçamento e densidade

- Priorizar título do trabalho, estado e próxima ação. Metadados e decisões técnicas ficam em segundo nível; transcript/logs em terceiro.
- Usar superfícies contidas, separadores discretos e alinhamento consistente. Evitar transformar cada métrica ou log em um card permanente.
- Explorar uma escala de espaço de 4, 8, 12, 16, 24 e 32 pixels CSS; aplicar tokens semânticos de layout em vez de valores dispersos.
- Partir de texto de interface de 14 pixels, títulos de 20/24 e código monoespaçado legível. Texto secundário menor não carrega informações críticas sem alternativa legível. Ajustar pelo teste de leitura e escala.
- Comparar densidade confortável e compacta nos mesmos dados. Como ponto inicial, linhas de 44 e 36 pixels respectivamente; os alvos finais dependem de teclado, pointer, zoom e legibilidade.
- Temas claro/escuro mantêm papel semântico de texto, seleção, foco, alerta e sucesso. A cor da marca não serve como único indicador de status.

### Teclado e descoberta de ações

- Proposta inicial: `Ctrl+K` abre busca/ações; `Esc` fecha o painel e devolve foco; `Tab` percorre controles; setas navegam listas quando o componente está focado.
- Atalhos são mostrados junto das ações e não interferem com digitação em texto/código. O mapa precisa ser testado no Windows e no futuro cliente de editor.
- Aprovação tem rótulo específico e alvo exato; não usar atalho global que aplique o resultado da seleção mais recente sem revisão.
- Modais têm foco inicial, saída e retorno definidos. Atualização ao vivo não muda o controle sob o foco do usuário.

### Adaptação ao espaço e preferência

- Em janela compacta, reduzir navegação e apresentar inspector como painel acessível sob demanda. A área do trabalho e a ação principal continuam disponíveis.
- Em janela ampla, permitir lista + trabalho + inspector se o usuário desejar. Largura extra não deve esticar linhas de texto sem limite nem abrir todos os painéis automaticamente.
- Persistir dimensões e preferências por usuário; modo foco concentra código/revisão. Mudanças de breakpoint não perdem filtros, seleção ou instrução em edição.
- Testar pelo menos 1024×768, 1280×800 e 1920×1080 em pixels CSS de viewport, além de escala Windows 125%, 150% e 200%. Tamanhos físicos e área lógica devem ser registrados separadamente.
- Testar texto ampliado a 200%. Conteúdo de controle reflowa; diff e DAG podem ter navegação própria quando sua estrutura bidimensional exigir.
- Respeitar redução de movimento. Transições não mantêm atividade animada onde o usuário precisa ler diff/logs.

### Acessibilidade de base

Especificar contraste mínimo de 4,5:1 para texto comum e 3:1 para elementos gráficos/controles aplicáveis; foco visível, labels acessíveis e estado reconhecível além da cor. Documentar ordem de foco e semântica dos componentes. A orientação oficial do [Fluent 2](https://fluent2.microsoft.design/accessibility) fundamenta esses critérios; utilizar um design system não comprova acessibilidade de toda a aplicação.

## 8 Piloto e avaliação de uso — roteiro, ainda não executado

O piloto com o responsável libera o gate de implementação quando o fluxo está navegável e não existem problemas críticos de entendimento, controle ou aprovação. A avaliação com três a cinco desenvolvedores representativos continua necessária antes de concluir o alpha, conforme OX-D05/OX-017. Não registrar participantes, resultados ou satisfação antes das sessões.

| Tarefa de avaliação | Evidência a registrar | Erro crítico |
| --- | --- | --- |
| Abrir projeto e criar trabalho | Conclusão; ajuda; campos compreendidos; conexão identificada | Iniciar no repositório/conta errado sem perceber |
| Identificar o que precisa de atenção | Estado/motivo/ação explicados pelo participante | Confundir espera normal com falha ou tarefa aplicada com apenas validada |
| Localizar uma falha e pedir correção | Encontrar check/finding, versão e arquivo; pedido contextual | Revisar ou corrigir outra tentativa silenciosamente |
| Ajustar preferência de modelo/thinking | Compreender solicitado/efetivo, suporte e escopo | Achar que parâmetro desconhecido foi confirmado ou alterar histórico |
| Interromper execução | Distinguir pause de admissões e cancel; localizar controle | Perder a possibilidade de controlar trabalho ativo |
| Aprovar aplicação | Explicar destino/base/artefato e confirmar o resultado correto | Aplicar outro diff ou destino sem aviso |

Registrar caminho seguido, tempo, hesitações, pedidos de ajuda, ações incorretas e comentários de desconforto. Ao final, perguntar qual informação faltou e onde o participante esperava encontrá-la. Comparar temas/densidade com os mesmos dados; evitar mostrar só a tela mais bonita.

Priorizar correção por severidade: crítico envolve alvo errado, perda de controle/trabalho ou impossibilidade de concluir; alto exige condução para tarefa essencial; médio reduz descoberta/clareza; baixo é refinamento. Cada problema tem cenário, decisão, mudança e reteste pendente. A meta inicial é concluir tarefas essenciais com pouca ajuda e zero problema crítico aberto; ela não representa prova estatística de usabilidade.

## 9 Handoff para protótipo e implementação

OX-D02 recebe três referências com evidência visual/documental, decisões de adotar/adaptar/descartar e critérios observáveis. OX-D03 recebe navegação, jornadas, telas e estados propostos. Wireframes navegáveis, direções alternativas e conciliação com os contratos definitivos do Core continuam parte de D1; este documento sozinho não conclui OX-D03/OX-D04/OX-D05.

Para comparar direções em OX-D04, preparar as mesmas quatro cenas: trabalho em execução; check falho com correção; resultado pronto para aplicação; configuração de modelo/reasoning com fallback visível. Usar nomes longos, mais de uma tarefa, artefatos de versões distintas e janela compacta. A direção escolhida deve funcionar nesses casos antes de receber refinamento decorativo.

O fluxo do aplicativo é desenvolvimento assistido: instruir, fornecer contexto, examinar código, solicitar mudanças, verificar e aplicar. IDE completo, LSP, debugger e ambiente integrado de testes permanecem fora do requisito inicial. A verificação determinística e sua evidência continuam parte essencial da orquestração.
