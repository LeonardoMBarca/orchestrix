# Pesquisa de produto e experiência do Orchestrix

O Orchestrix precisa combinar coordenação confiável de agentes com um espaço agradável para desenvolver: formular trabalho, acompanhar execução, entender escolhas, examinar código, solicitar correções e aplicar resultados. A pesquisa inicial deve orientar tanto a solução quanto a experiência visual antes de consolidar a implementação.

**Atualização em 8 de outubro de 2026 — D0 concluído como pesquisa:** a [conclusão](./research/d0-conclusion.md) registra evidências, 14 decisões priorizadas, lacunas e a revisão explícita do aceite metodológico. O pacote reúne nove famílias, [manutenção/licenças](./research/landscape-and-provenance.md), [jornadas](./research/solution-journeys.md), [benchmark visual](./research/experience-decisions.md), [posicionamento](./research/product-positioning.md) e [contas/assinatura](./research/subscription-account-ux.md). Demos [Conductor](./research/conductor-dynamic-evidence.md) e [Cline/Superset](./research/workflow-demonstrations.md) aprofundam as capturas anteriores; Vibe/OpenCode complementam a observação. Passos apenas documentados permanecem identificados. O [protótipo exploratório](../prototypes/desktop/README.md) usa Studio como padrão escolhido. D1 e seu piloto são a próxima etapa; depois retomamos M0, conforme o [status](./DEVELOPMENT_STATUS.md).

**Complemento em 09/10/2026:** [pesquisa competitiva atualizada](./research/competitive-update-2026-10-09.md) acrescenta Jackalope e Cobalt e registra múltiplas contas no Superset. O levantamento acumulado passa a onze famílias; demos e auditoria de licenças anteriores não foram ampliadas por essa consulta documental. A diferenciação passa por hipóteses mensuráveis, com protocolo futuro vinculado ao piloto/alpha e a M3/M4.

## 1 Perguntas que a pesquisa precisa responder

1. Que parte da coordenação os produtos semelhantes automatizam e que parte ainda exige trabalho manual?
2. Como representam objetivo, tarefa, workspace, agente, conta, sessão e resultado?
3. Como o usuário percebe paralelismo, bloqueios, pedidos de atenção e trabalho pronto para revisar?
4. Como contexto, revisão, correção e integração são encaminhados entre agentes?
5. Que interfaces oficiais podem ser reutilizadas sem comprometer políticas, identidade ou modo de cobrança?
6. Que estrutura permite trabalhar confortavelmente durante uma sessão longa, em diferentes tamanhos de janela?
7. O que o Orchestrix deve adotar, adaptar ou deixar fora do escopo inicial?

Comparar separadamente coordenação funcional, integração técnica e experiência de uso. Um produto pode ser uma ótima referência de navegação sem possuir o controle de execução necessário ao Orchestrix.

## 2 Produtos próximos da proposta

As capacidades nesta tabela são descritas pelas fontes oficiais. A coluna de investigação contém perguntas e inferências para o Orchestrix, sem atribuir desempenho comprovado ou superioridade a nenhum produto.

| Referência | Capacidades documentadas | Investigação para o Orchestrix |
| --- | --- | --- |
| Conductor | Harnesses para Claude Code, Codex, Cursor e OpenCode; workspaces com branches/worktrees; revisão com diff e feedback encaminhado ao agente. Chats de um workspace compartilham código. [Harnesses](https://www.conductor.build/docs/reference/harnesses), [workspaces](https://www.conductor.build/docs/concepts/workspaces-and-branches), [workflow](https://www.conductor.build/docs/concepts/workflow). | Estudar o vínculo trabalho → workspace → resultado. Adaptar comentários no diff para pedidos de correção; investigar conflitos de escrita quando mais de uma sessão usa o mesmo workspace. |
| Codex no aplicativo desktop | Chats paralelos em worktrees, revisão e controles Git. Comentários no diff orientam o agente quando o usuário envia uma mensagem de acompanhamento. [Worktrees](https://learn.chatgpt.com/docs/environments/git-worktrees), [ambiente local](https://learn.chatgpt.com/docs/environments/local-environment), [revisão e feedback](https://learn.chatgpt.com/docs/code-review). | Estudar continuidade entre conversa, alteração e revisão. Comparar organização por chats com a organização por objetivos/tarefas proposta para o Orchestrix, sem inferir pooling de contas ou provedores dessa experiência. |
| Superset | Workspaces locais, agentes CLI, diff e sessões persistentes; Desktop e CLI usam host server separado. Também documenta coordenação Claude/Codex com workers mistos, mensagens, worktrees e verificação antes de integrar. [Modelo](https://docs.superset.sh/superset-model), [host server](https://docs.superset.sh/cli/host-server), [orchestration](https://docs.superset.sh/orchestration). | Comparar o ciclo de coordenação completo e os controles mantidos pelo app. Avaliar quanto editor, terminais e browser ajudam na experiência pretendida. Orquestração de workers mistos já é uma sobreposição documentada. |
| Vibe Kanban | Organização do trabalho, execução de agentes em worktrees e revisão de alterações; código público inclui executors, Git e review. A empresa encerrou atividades em abril de 2026; o projeto continua comunitário e sem os antigos serviços remotos. [Produto](https://www.vibekanban.com/), [código](https://github.com/BloopAI/vibe-kanban), [encerramento](https://www.vibekanban.com/blog/shutdown). | Estudar a passagem entre planejar, executar e revisar. Verificar se board/lista explicam dependências e decisões ou se exigem detalhes complementares. Registrar a continuidade comunitária ao avaliar dependências. |
| cmux | Terminal com workspaces, splits, metadados e notificações; API de controle. Sua filosofia deixa worktrees a scripts, e restauração de workspace não equivale a restaurar qualquer processo vivo. [Projeto](https://github.com/manaflow-ai/cmux), [worktrees](https://cmux.com/blog/cmux-home), [restauração](https://cmux.com/docs/getting-started). | Estudar navegação por necessidade de atenção. Preservar a distinção entre restaurar a interface e recuperar execução; avaliar que configurações o Orchestrix deve assumir para o usuário. |
| Antigravity | O anúncio oficial apresenta o Antigravity 2.0 como app independente do IDE, com Windows/macOS/Linux, tarefas assíncronas e subagentes; a família também anuncia CLI/SDK/API. [Anúncio](https://www.antigravity.google/blog/introducing-google-antigravity-2), [artefatos/revisão](https://www.antigravity.google/docs/artifact-review/). | Referência direta de coordenação e revisão, além de provider desejado. Comparar as superfícies concretas; integração de terceiros, conta/cota e contrato do adapter ainda não foram validados. [Análise complementar](./research/product-positioning.md#antigravity-como-referência-adicional). |

Multissessão e suporte a vários agentes não comprovam suporte a múltiplas contas independentes. No comparativo, conta, autenticação, quota e grupos de capacidade precisam de evidência própria. Recursos não encontrados nas fontes consultadas entram como “não verificado”, sem virar afirmações de inexistência.

O [complemento competitivo](./research/competitive-update-2026-10-09.md#comparação-documental) registra evidência específica de contas para Superset, Jackalope e Cobalt, além de atualizar Cline Kanban e OpenHands Canvas. Suporte documentado permanece distinto de isolamento/cota comprovados em teste próprio.

## 3 Soluções e contratos técnicos a estudar

| Referência | Evidência oficial | Aplicação proposta |
| --- | --- | --- |
| OpenHands | A versão examinada também apresenta Agent Canvas como control center de agentes; produto, SDK/Agent Server e backends são superfícies distintas. SDK documenta ACP, roteamento e implementação/crítica/correção. [Canvas v1.26.0](https://github.com/OpenHands/OpenHands/blob/v1.26.0/README.md), [ACP Agent](https://docs.openhands.dev/sdk/guides/agent-acp), [routing](https://docs.openhands.dev/sdk/guides/llm-routing), [refinement](https://docs.openhands.dev/sdk/guides/iterative-refinement). | Comparar aplicativo e contratos separadamente. Wrapper ACP documenta autoaprovação e fallback API; conferir aprovação/cobrança antes de reutilizar. Exemplos SDK não comprovam a UX do Canvas. [Proveniência](./research/landscape-and-provenance.md). |
| Cline | Família com CLI, Desktop Windows beta, Kanban e SDK; documenta Teams com coordenador/especialistas e pipelines por modelo/thinking. [Desktop](https://docs.cline.bot/usage/cline-desktop), [Teams](https://docs.cline.bot/sdk/guides/multi-agent-teams), [pipeline](https://docs.cline.bot/cli/samples/model-orchestration). | Comparar coordenação além do agente individual; distinguir qual superfície oferece cada recurso. Testar identidade independentemente de config/estado e importação de conversas. |
| OpenCode | SDK com servidor local, sessões, abort, resposta a permissões, eventos SSE e saída estruturada. [SDK](https://docs.opencode.ai/docs/sdk/). | Estudar contrato tipado e clientes separados do servidor. Saída válida em schema confirma formato, não correção do código produzido. |

Roo Code pode ser estudado como referência histórica de papéis especializados. Seu repositório informa encerramento da extensão e arquivamento em maio de 2026; não deve ser tratado como dependência ativa. [Repositório oficial](https://github.com/RooCodeInc/Roo-Code).

ACP, MCP e APIs de runtime devem ser avaliados por responsabilidade: protocolo de execução do agente, ferramentas/contexto e controle da aplicação são contratos diferentes. A pesquisa deve comparar adapter específico e integração por protocolo, registrando eventuais perdas em autenticação, cancelamento, capabilities, eventos, permissões e quota.

Não reutilizar código ou componentes apenas porque estão disponíveis no GitHub. Registrar licença e condições da versão estudada antes de qualquer incorporação; referência conceitual e reutilização de implementação são decisões distintas.

## 4 Referências de interação e identidade visual

### Linear

O relato oficial de redesign descreve ajustes de hierarquia, alinhamento, densidade e ruído visual, com testes de diferentes vistas e temas. As imagens consultadas mostram navegação lateral consistente, lista e detalhe, superfícies contidas e cor usada para seleção/status. Esse material é de 2024 e serve como estudo do processo e da linguagem visual, não como prova do estado atual inteiro do produto. [Redesign do Linear](https://linear.app/now/how-we-redesigned-the-linear-ui).

A documentação atual permite alternar layouts, escolher propriedades visíveis e persistir preferências de exibição. Para o Orchestrix, investigar lista, board e DAG como vistas do mesmo trabalho, com divulgação progressiva dos metadados. [Display options](https://linear.app/docs/display-options).

### Raycast

O Action Panel agrupa ações contextuais, oferece busca e apresenta atalhos. A adaptação proposta é um mecanismo consistente para agir sobre tarefa, arquivo, finding e execução, preservando uma ação principal visível. Ações de aprovação ou integração precisam de semântica própria, sem disparo ambíguo ao pressionar Enter. [Action Panel](https://manual.raycast.com/action-panel).

### Fluent 2

As orientações abordam estrutura previsível, foco, contraste e adaptação ao zoom. Usá-las como base para leitura e operação confortável no Windows, sem impor ao Orchestrix a identidade visual da Microsoft nem escolher uma biblioteca de componentes antecipadamente. [Acessibilidade](https://fluent2.microsoft.design/accessibility).

### GitHub Desktop

O histórico permite selecionar commits e arquivos para examinar seus diffs. Investigar a clareza do vínculo versão → arquivos → alterações como referência para revisão de artefatos, sem transportar toda a experiência de um cliente Git para a tela principal do Orchestrix. [Histórico e diff](https://docs.github.com/en/desktop/making-changes-in-a-branch/viewing-the-branch-history-in-github-desktop).

O benchmark visual deve identificar o problema resolvido por cada referência: hierarquia, leitura de diff, configuração, navegação, feedback ou atenção. Um painel de imagens sem anotações e uma escolha de tema isolada não encerram essa pesquisa.

## 5 Hipóteses de posicionamento a validar

O foco proposto é um workspace de desenvolvimento assistido centrado no objetivo e no resultado. O usuário organiza trabalho e toma decisões; o orquestrador distribui execução e prepara evidências. Conta, modelo, thinking e contexto são escolhas explicáveis, com ajustes simples e controles avançados acessíveis.

As oportunidades a investigar são:

- Reduzir a coordenação manual entre planejamento, execução, revisão e correção.
- Mostrar o que precisa da atenção do usuário antes de expor todo o fluxo de eventos.
- Relacionar cada alteração a tarefa, contexto, política e verificação.
- Fazer o mesmo fluxo funcionar com uma conta, várias contas próprias ou vários provedores.
- Preservar continuidade do trabalho e clareza de estado após interrupções.

Essas oportunidades são hipóteses de produto. A pesquisa comparativa e os testes de uso precisam verificar sua relevância; o plano não pressupõe que nenhum concorrente já as resolva.

A [rodada de posicionamento](./research/product-positioning.md) encontrou sobreposição direta em coordenação, modelo/thinking, review iterativo e Windows, inclusive no Antigravity 2.0. A proposta a validar fica mais precisa: **um ciclo compreensível e rastreável, útil com uma única assinatura, com escolhas configuráveis e possibilidade de crescer para outras conexões**. O documento contém a matriz de adotar/adaptar/descartar e 16 cenários verificáveis para D1; não anuncia exclusividade ou superioridade.

## 6 Processo de pesquisa aprofundada

### D0 Pesquisa de solução e benchmark

Completar o levantamento com pelo menos seis soluções relevantes e três referências de interação/visual. Priorizar documentação oficial, repositórios, releases e demonstrações. Registrar data, versão quando disponível e tipo de evidência: documentado, visível em demo, observado em teste próprio ou hipótese.

Para cada solução, comparar: público e problema; organização do trabalho; runtimes e contas; contexto; modelo/thinking; isolamento; permissões; concorrência; revisão; integração; recuperação; local/cloud; plataformas; extensibilidade; manutenção e licença. Usar “não verificado” quando a informação não estiver estabelecida.

Percorrer pelo menos três referências diretas em instalação ou demonstração acessível, documentando estas jornadas:

1. Abrir projeto e conectar agente/conta.
2. Iniciar trabalho e fornecer contexto.
3. Acompanhar duas tarefas ou sessões e identificar pedidos de atenção.
4. Entender bloqueio, falha ou limite e executar a ação apropriada.
5. Revisar diff, pedir correção e aplicar o resultado.

Capturar ou vincular telas/demos pertinentes e anotar decisões de navegação, densidade e feedback. Se uma jornada não puder ser executada, indicar exatamente quais passos foram apenas documentados. Não substituir essa distinção por uma avaliação fictícia.

**Entregáveis:** matriz comparativa, mapa de jornadas, referências visuais anotadas, lacunas e adotar/adaptar/descartar, consolidados em [D0 — conclusão](./research/d0-conclusion.md). Cada proposta prioritária aponta necessidade, referência e efeito esperado. A sequência seguinte é D1 com piloto inicial → retomada de M0; experimentos antecipados não substituem os gates.

### D1 Design e validação do workspace

Usar as decisões de D0 para criar arquitetura de informação, wireframes e duas ou três direções visuais coerentes. Comparar as direções nas mesmas telas e estados; selecionar uma com o responsável pelo produto e desenvolver um protótipo de alta fidelidade navegável.

O protótipo precisa cobrir abrir projeto, configurar conexão, formular trabalho, acompanhar execução, revisar código, solicitar correção e aprovar aplicação. Incluir estados de falha, espera, conflito e reconexão, além do caminho de sucesso. Definir tokens, componentes, conteúdo, responsividade, foco, temas e movimento.

Avaliar o protótipo com o responsável pelo produto e desenvolvedores representativos, buscando inicialmente três a cinco participantes externos. Registrar conclusões de tarefas, ajuda necessária, enganos de estado, ações incorretas e desconforto relatado. Se a primeira avaliação tiver apenas o responsável, identificá-la como piloto e manter avaliação externa pendente.

**Condição de saída inicial de D1 para retomar M0:** direção visual escolhida, fluxo principal navegável, avaliação piloto registrada e nenhum problema crítico aberto que permita aprovar o artefato errado, perder controle da execução ou impedir a conclusão do fluxo. A experiência validada orientará o Desktop de M2. A avaliação externa amplia a evidência antes da conclusão do alpha; uma amostra pequena não comprova satisfação de todo o público.

## 7 Direção proposta para a experiência

| Área | Comportamento a projetar e validar |
| --- | --- |
| Entrada | Projeto recente, conexão disponível e próxima ação; onboarding gradual conforme a necessidade. |
| Workspace | Área principal para o trabalho selecionado, navegação consistente e painel contextual recolhível. |
| Atenção | Separar “em execução” de “precisa de você”, “bloqueado” e “pronto para revisar”, com motivo e ação. |
| Revisão | Resumo, diff, findings e evidências vinculados à mesma versão do artefato. |
| Configuração | Presets e preferências frequentes primeiro; detalhes de política/modelo/contexto sob demanda. |
| Adaptação | Painéis redimensionáveis/recolhíveis, modo foco e preferências persistentes; preservar seleção e posição ao atualizar. |
| Código | Leitura, diff e pedidos de alteração dentro do app; edição leve pode acrescentar indentação, salvar e desfazer. |

“Adaptativa” significa responder a espaço disponível, tarefa selecionada, estado de execução e preferências. Atualizações ao vivo não devem reorganizar a tela inesperadamente, roubar foco ou esconder a ação em uso. O DAG é uma vista especializada; a experiência principal precisa continuar compreensível sem ele.

Modernidade visual deve aparecer em tipografia, ritmo de espaçamento, alinhamento, densidade, superfícies, iconografia e transições consistentes. Definir paleta e identidade após explorar as direções; cores de status devem manter significado entre temas. Movimento precisa respeitar redução de animações e não atrapalhar leitura de logs ou revisão.

O Orchestrix deve permitir desenvolver pelo fluxo assistido dentro do aplicativo. Edição leve de código é uma capacidade delimitada, a ser priorizada conforme pesquisa, incluindo aviso de escrita concorrente e invalidação de verificações após alteração manual. IDE completo, debugger, LSP e ambiente integrado de testes não são requisitos iniciais. A verificação determinística do Core continua necessária; sua interface mostra evidência e ações relacionadas ao trabalho.

## 8 Critérios de aceite de experiência

- O usuário identifica projeto/tarefa atuais, estado, motivo de bloqueio e próxima ação sem interpretar logs brutos.
- O caminho principal funciona por mouse e teclado, com foco visível, atalhos descobríveis e retorno de foco após diálogos.
- Layouts compactos e amplos preservam ações; testar escalas comuns do Windows e texto ampliado a 200%, com contraste adequado e estados reconhecíveis além da cor.
- Temas claro e escuro mantêm legibilidade e hierarquia; diff e DAG têm navegação apropriada quando excedem a área disponível.
- Estados vazio, carregando, sem conexão, autenticação expirada, executando, bloqueado, rate limit, check falho, revisão rejeitada, conflito, reconectando, cancelado e aplicado possuem conteúdo e ações projetados.
- Preferências de layout persistem; eventos novos não deslocam o trabalho em revisão nem misturam versões do artefato.
- A implementação é comparada ao protótipo e validada com dados variados: nomes longos, muitas tarefas, diffs extensos e falhas reais.
- A avaliação de uso registra problemas, decisões e iterações; uma captura bonita não comprova que o fluxo está compreensível.

Esses critérios complementam os testes de execução do Core. D0 e D1 orientam o primeiro aplicativo; a pesquisa deve ser retomada ao surgir evidência que altere uma decisão de produto ou integração, sem se tornar uma etapa indefinida que impede entregas.

## 9 Recomendações consolidadas de D0 para D1

As recomendações abaixo conectam a pesquisa às próximas decisões de experiência. São propostas para avaliar; as preferências visuais já expressas pelo responsável continuam registradas no status. P0 indica comportamento essencial para o primeiro fluxo; P1 indica exploração delimitada.

| Prioridade | Recomendação | Evidência aprofundada | Saída esperada em D1 |
| --- | --- | --- | --- |
| P0 | Trabalhar por objetivo, etapas e resultado; oferecer caminho curto para correções pequenas. | [Posicionamento](./research/product-positioning.md): Superset/Cline já coordenam trabalho, e o fluxo precisa provar valor com uma única assinatura. | Jornada simples e jornada com dependências, sem exigir DAG em ambas. |
| P0 | Preset fácil de escolher, com resumo de conta/runtime, modelo/thinking e revisão; detalhes sob demanda. | [Posicionamento](./research/product-positioning.md), [variantes e escopo](./research/workflow-risk-patterns.md). | Usuário prevê a configuração da próxima tentativa e identifica escolhas não observadas. |
| P0 | Atenção por motivo e próxima ação, vinculada ao trabalho afetado. | [Jornadas](./research/solution-journeys.md), [controle do trabalho](./research/workflow-risk-patterns.md). | Diferenciar pergunta, permissão, falha, capacidade ocupada e revisão pronta. |
| P0 | Revisão como diff, findings e checks da mesma versão, com comentários persistentes. | [Revisão e contexto](./research/workflow-risk-patterns.md), [benchmark de experiência](./research/experience-decisions.md). | Trocar arquivo preserva rascunho; nova alteração torna a aprovação anterior identificável como desatualizada. |
| P0 | Explicar o destino de cada ação Git e preservar proveniência. | [Semântica de integração](./research/workflow-risk-patterns.md). | Usuário distingue atualizar base, aceitar tarefa e aplicar Run no destino. |
| P0 | Adaptar área central ao trabalho; persistir painéis e preservar foco/seleção. | [Referências visuais](./research/experience-decisions.md), [painéis e workflow](./research/workflow-risk-patterns.md). | Layout compacto/amplo, leitura confortável e atualização ao vivo sem deslocar a ação em uso. Studio permanece padrão; temas ficam em Aparência. |
| P0 | Mostrar contexto selecionado e continuidade entre etapas, mantendo identidade da conexão. | [Contexto e uma assinatura](./research/product-positioning.md), [sessões compartilhadas](./research/workflow-risk-patterns.md). | Inspeção de fontes/versão e revisão em outra sessão sem sugerir conta ou cota nova. |
| P1 | Explorar edição leve com indentação, salvar/desfazer e aviso de escritor ativo. | Objetivo do responsável e [escopo de experiência](./research/experience-decisions.md). | Decisão explícita sobre entrada no alpha, baseada no piloto e no contrato de escrita. |

Os [16 cenários de posicionamento](./research/product-positioning.md#requisitos-verificáveis-para-o-protótipo-e-piloto-d1) e os [sete requisitos de controle/revisão](./research/workflow-risk-patterns.md#requisitos-de-experiência-derivados) detalham o aceite. São critérios a usar em D1, sem participantes ou resultados de usabilidade inventados.

**Limite do encerramento:** não foram demonstradas quinze jornadas completas de concorrentes. O aceite de pesquisa foi revisto explicitamente na [conclusão de D0](./research/d0-conclusion.md), com demos em três referências, cobertura por passo e lacunas preservadas. Usabilidade do Orchestrix, contas, recuperação e integração têm validação própria em D1/M0/M1/M2; capturas e vídeos não concedem esses aceites.
