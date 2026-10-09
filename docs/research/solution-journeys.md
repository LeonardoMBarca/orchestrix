# Jornadas de soluções semelhantes ao Orchestrix

**Referência atual:** D0 foi consolidado em [d0-conclusion.md](d0-conclusion.md), com alteração explícita do método. Este arquivo conserva a rodada inicial e seus limites. Novas demos e a matriz ampliada estão em [Conductor](conductor-dynamic-evidence.md), [Cline/Superset](workflow-demonstrations.md) e [OpenCode](opencode-media-observation.md). As tabelas anteriores não pretendem representar toda a cobertura final.

Pesquisa OX-D01 consultada em **8 de outubro de 2026**. A rodada inicial inspecionou seis capturas, duas mídias oficiais e previews de dois vídeos oficiais no YouTube. O aprofundamento acrescentou cinco capturas e uma demo de diff Conductor: [controle/contexto](workflow-risk-patterns.md) e [atenção/revisão](attention-and-review-benchmark.md). Não houve instalação dos produtos concorrentes, autenticação, chamadas de modelo, teste com contas ou benchmark de desempenho nessa pesquisa D0. O spike Codex é uma entrega separada de M0.

Classificação de evidência:

- **Documentado:** a fonte oficial descreve a interação. Não significa que a jornada tenha sido executada nesta pesquisa.
- **Observado em captura oficial:** o elemento aparece na imagem vinculada. A imagem não comprova transições, funcionamento ou disponibilidade na versão instalada pelo usuário.
- **Observado em amostras de demo oficial:** frames de mídia dinâmica publicada pelo produto foram examinados em tempos identificados. Somente as cenas descritas foram verificadas; isso não substitui teste hands-on.
- **Observado em frames de preview oficial:** imagens do storyboard público do player foram examinadas. Permitem verificar cenas amostradas, mas não reprodução contínua, áudio, cliques intermediários nem resultado de operações entre frames.
- **Não verificado:** falta evidência suficiente nas fontes consultadas, há dependência não testada ou documentação potencialmente histórica. Não é afirmação de inexistência.
- **Inferência para o Orchestrix:** proposta de adaptação ou tradeoff, que ainda exige validação de produto.

As demos complementares estão registradas adiante; nenhuma instalação própria percorreu as jornadas. Esta matriz compara cinco jornadas por produto, sem atribuir notas de usabilidade. Conta, autenticação do aplicativo, autenticação do agente e autenticação Git são conceitos diferentes. Suporte a múltiplos agentes ou sessões não comprova múltiplas contas nem cotas independentes.

## Conductor

| Jornada | Evidência e fluxo oficial | Limite da verificação |
| --- | --- | --- |
| Abrir projeto e conectar | **Documentado.** Adicionar repositório local, GitHub ou novo; criar workspace com branch e arquivos próprios. GitHub usa autenticação disponível no terminal. Autenticação do harness é tratada separadamente. [Primeiro workspace](https://www.conductor.build/docs/first-workspace), [autenticação e problemas](https://www.conductor.build/docs/troubleshooting/issues). | **Não verificado:** gerenciamento de múltiplas identidades simultâneas e cotas por conta/modelo. |
| Formular trabalho e contexto | **Documentado.** Iniciar chat, anexar contexto, usar instruções do repositório e arquivos de handoff em `.context`. Controles de modo/reasoning dependem do harness e modelo. [Primeiro workspace](https://www.conductor.build/docs/first-workspace), [modos](https://www.conductor.build/docs/concepts/agent-modes). | Seleção disponível não comprova roteamento automático ideal nem qualidade do contexto. |
| Acompanhar paralelismo e atenção | **Documentado.** Workspaces separados isolam branches; chats no mesmo workspace compartilham código. Sidebar sinaliza necessidade de input. [Agentes paralelos](https://www.conductor.build/docs/concepts/parallel-agents), [problemas de agentes](https://www.conductor.build/docs/troubleshooting/issues). | **Não verificado:** prevenção automática de conflitos entre escritores do mesmo workspace. |
| Resolver bloqueio ou falha | **Documentado.** Abrir sessão que pede input/permissão; cancelar resposta e orientar próximo passo quando necessário. Troubleshooting distingue autenticação, scripts, agentes, workspace e Git. [Troubleshooting](https://www.conductor.build/docs/troubleshooting/issues). | Recuperação de cada caso não foi testada. |
| Revisar, corrigir e integrar | **Documentado.** Diff com comentários enviados ao agente; Checks reúne Git/PR/CI/review/todos conforme integrações. Conferir alterações, resolver itens e então fazer merge. [Diff](https://www.conductor.build/docs/reference/diff-viewer), [Checks](https://www.conductor.build/docs/reference/checks). | **Não verificado:** eficácia dos bloqueios em todas as combinações de harness/integração. |

**Inferências para o Orchestrix:** vincular tarefa, workspace e resultado; converter comentários no diff em pedidos de correção rastreáveis. Tradeoff: o compartilhamento de um workspace pode favorecer colaboração, mas exige política explícita de escrita concorrente.

## Superset

| Jornada | Evidência e fluxo oficial | Limite da verificação |
| --- | --- | --- |
| Abrir projeto e conectar | **Documentado.** Adicionar pasta ou Git URL; busca GitHub depende do login `gh` do host escolhido. Há launchers/configuração por agente. [Primeira sessão](https://docs.superset.sh/first-workspace), [agentes](https://docs.superset.sh/agent-integration). | **Não verificado:** pooling de contas e cotas independentes. |
| Formular trabalho e contexto | **Documentado.** Descrever tarefa, escolher agente e criar worktree. Scripts preparam dependências/arquivos. Modelos e reasoning disponíveis variam por runtime. [Primeira sessão](https://docs.superset.sh/first-workspace), [agentes](https://docs.superset.sh/agent-integration). | **Não verificado:** montagem automática do melhor contexto para cada tarefa. |
| Acompanhar paralelismo e atenção | **Documentado.** Tutorial distribui mesmo prompt entre agentes em workspaces separados; sidebar mostra atividade. Hooks/wrappers alimentam status e notificações. Alguns agentes só sinalizam conclusão, sem espera por input. [Primeira sessão](https://docs.superset.sh/first-workspace), [status](https://docs.superset.sh/agent-status). | Ausência de evento de espera não comprova que o agente continue trabalhando. |
| Resolver bloqueio ou falha | **Documentado.** Perguntas, permissões e plano aparecem no chat. `Clear Status` corrige indicador obsoleto após encerramento forçado. Troubleshooting orienta diagnóstico de PATH, host, terminais e Git. [Status](https://docs.superset.sh/agent-status), [troubleshooting](https://docs.superset.sh/troubleshooting). | Indicador persistido não comprova processo vivo; recuperação não foi executada. |
| Revisar, corrigir e integrar | **Documentado.** Changes permite diff split/unified, edição e stage/commit/push/PR. Receita de feedback cria workspace a partir da PR, fornece comentários ao agente e pede nova revisão. [Primeira sessão](https://docs.superset.sh/first-workspace), [feedback de PR](https://docs.superset.sh/recipes/pr-feedback). | Receita depende de permissões Git/GitHub e prompt; sucesso não foi testado. |

**Inferências para o Orchestrix:** adotar criação centrada na descrição do trabalho; declarar capacidades e confiança de status por adapter. Tradeoff: comparar tentativas paralelas ajuda explorar soluções, mas decidir o vencedor e transportar partes úteis ainda pode exigir revisão manual.

## Vibe Kanban

O comunicado de **10 de abril de 2026** encerra a empresa e prevê retirada dos antigos serviços remotos após 30 dias, preservando workspaces locais e manutenção comunitária. A documentação ainda contém login cloud e board remoto; estes não são tratados como capacidades atuais verificadas. [Comunicado oficial](https://www.vibekanban.com/blog/shutdown), [onboarding ainda publicado](https://www.vibekanban.com/docs/getting-started).

| Jornada | Evidência e fluxo oficial | Limite da verificação |
| --- | --- | --- |
| Abrir projeto e conectar | **Documentado.** Preferências iniciais de agente/editor/notificação; criação seleciona repositórios em disco e branches, podendo incluir vários repos. [Onboarding](https://www.vibekanban.com/docs/getting-started), [criação](https://www.vibekanban.com/docs/workspaces/creating-workspaces). | **Não verificado:** disponibilidade atual do login/board remoto; autenticação de várias contas de agentes. |
| Formular trabalho e contexto | **Documentado.** Tarefa, repositórios, target branches e agente formam o workspace; setup scripts são opcionais. Chat aceita arquivos via `@`, imagens e variantes de agente. [Criação](https://www.vibekanban.com/docs/workspaces/creating-workspaces), [chat](https://www.vibekanban.com/docs/workspaces/chat-interface). | Variante é configuração, não evidência de conta distinta. Medidor de contexto não é quota da assinatura. |
| Acompanhar paralelismo e atenção | **Documentado.** Workspaces permitem sessões e execuções paralelas. Chat distingue Idle/Running/Queued/Sending; permite Queue e Stop; painel pode exibir tarefas do agente. [Criação](https://www.vibekanban.com/docs/workspaces/creating-workspaces), [chat](https://www.vibekanban.com/docs/workspaces/chat-interface). | **Não verificado:** agregação consistente de atenção entre todos os runtimes. |
| Resolver bloqueio ou falha | **Documentado.** Chat expõe erro e cartão de aprovação/revisão de plano; timeout pode exigir nova mensagem. Git apresenta conflitos com continuar/abortar. [Chat](https://www.vibekanban.com/docs/workspaces/chat-interface), [Git](https://www.vibekanban.com/docs/workspaces/git-operations). | Não foi avaliada recuperação após crash ou queda do runtime. |
| Revisar, corrigir e integrar | **Documentado.** Changes oferece diffs e comentários; enviar mensagem entrega feedback ao agente, seguido de nova revisão e PR. Git distingue working/target; seu comando Merge traz target para working. [Changes](https://www.vibekanban.com/docs/workspaces/changes), [Git](https://www.vibekanban.com/docs/workspaces/git-operations). | **Não verificado:** semântica atual de aplicação local na target branch. Não confundir atualizar working branch com integrar resultado final. |

**Inferências para o Orchestrix:** manter revisão e correção próximas; distinguir fila de mensagem, execução e aprovação. Tradeoff: referências de planejamento remoto são úteis conceitualmente, mas precisam ser separadas das capacidades locais mantidas hoje.

## cmux

Esta matriz trata o **aplicativo nativo macOS**, sem transferir capacidades de outros produtos cmux para ele. Workspaces são contêineres de panes/surfaces, e não necessariamente worktrees. [Introdução](https://cmux.com/docs/getting-started), [conceitos](https://cmux.com/docs/concepts).

| Jornada | Evidência e fluxo oficial | Limite da verificação |
| --- | --- | --- |
| Abrir projeto e conectar | **Documentado.** Primeiro workspace já abre terminal; CLI permite controle. Instalação de hooks encontra agentes no PATH e habilita integrações. [Introdução](https://cmux.com/docs/getting-started), [resume e hooks](https://cmux.com/docs/session-restore). | **Não verificado:** fluxo unificado de conexão de contas e leitura de cotas. |
| Formular trabalho e contexto | **Documentado.** Usuário organiza terminais/browser e seus agentes; scripts/templates podem escolher checkout, worktree ou SSH. [Conceitos](https://cmux.com/docs/concepts), [cmux home](https://cmux.com/blog/cmux-home). | **Não verificado:** montagem semântica centralizada do contexto ou roteamento automático de modelo. |
| Acompanhar paralelismo e atenção | **Documentado.** Workspaces com panes/surfaces; notificações têm unread e navegação direta ao workspace. Há contexto de evento do agente quando disponível. [Conceitos](https://cmux.com/docs/concepts), [notificações](https://cmux.com/docs/notifications). | Cobertura de eventos depende da integração; não inferir evento universal para todo CLI. |
| Resolver bloqueio ou falha | **Documentado.** Hook de notificação falho usa comportamento padrão e alerta. Restauração recupera layout/metadados; agentes suportados retomam por token nativo capturado. Há matriz de resume por agente. [Notificações](https://cmux.com/docs/notifications), [restauração](https://cmux.com/docs/session-restore). | Layout restaurado não equivale a checkpoint de processo vivo. Resume de cada runtime não foi testado. |
| Revisar, corrigir e integrar | **Documentado.** Terminais/browser e scripts permitem compor um fluxo próprio. **Observado em captura oficial:** navegador com PR ao lado de terminais. [cmux home](https://cmux.com/blog/cmux-home), [captura do projeto](https://raw.githubusercontent.com/manaflow-ai/cmux/main/docs/assets/main-first-image.png). | **Não verificado:** pipeline nativo completo de diff → comentários → correção → aplicação do resultado. |

**Inferências para o Orchestrix:** navegação por atenção reduz custo de vigiar sessões; separar recuperação de interface, sessão e processo. Tradeoff: flexibilidade de primitivas beneficia usuários avançados, mas deixa decisões de fluxo e configuração para scripts.

## Capturas oficiais inspecionadas

As imagens abaixo foram efetivamente abertas e examinadas. Seus links são referências, não cópias incorporadas ao projeto; URLs com hash ou branch `main` podem mudar. Data/versão exata das imagens não foi confirmada.

| Produto e referência | Observado na imagem | Uso possível em D1 — inferência |
| --- | --- | --- |
| [Conductor — novo workspace](https://www.conductor.build/docs-assets/images/new-workspace.png), vinculado ao [tutorial](https://www.conductor.build/docs/first-workspace) | Sidebar de workspaces; centro com preparo inicial e composer; Changes vazio e terminal à direita. | Explicar o que foi preparado antes do primeiro envio; testar um estado vazio útil. |
| [Superset — criação por prompt](https://docs.superset.sh/_next/static/media/prompt-first.1ecsaxm7lzgu-.png), vinculado ao [tutorial](https://docs.superset.sh/first-workspace) | Composer amplo, sugestões e seletores de agente, host, projeto e branch; sidebar de trabalho. | Começar pelo objetivo e revelar opções de execução na mesma composição. |
| [Superset — diff](https://docs.superset.sh/_next/static/media/changes.0ljpesxvt8xuy.png), vinculado ao [tutorial](https://docs.superset.sh/first-workspace) | Diff lado a lado, contador de arquivos vistos, lista de arquivos, commit e histórico. | Oferecer revisão concentrada e progresso explícito por arquivo. |
| [Vibe Kanban — execução](https://mintcdn.com/vibekanban/QA35mU65cg2kMRzj/images/workspaces-running-state.png), vinculado ao [chat](https://www.vibekanban.com/docs/workspaces/chat-interface) | Mensagens de ferramentas; composer com Queue e Stop durante execução. | Tornar a diferença entre orientar próxima etapa e interromper a atual visível. |
| [Vibe Kanban — comentários](https://mintcdn.com/vibekanban/QA35mU65cg2kMRzj/images/workspaces-inline-comments.png), vinculado a [Changes](https://www.vibekanban.com/docs/workspaces/changes) | Diff lado a lado, comentário existente, formulário de comentário, árvore de arquivos e Git. | Associar correção à linha e versão revisada; separar rascunho de feedback enviado. |
| [cmux — múltiplas sessões](https://raw.githubusercontent.com/manaflow-ai/cmux/main/docs/assets/main-first-image.png), do [repositório oficial](https://github.com/manaflow-ai/cmux) | Sidebar, grade de terminais, browser local e browser com PR. | Usar resumos de atenção; testar se tantos painéis ajudam ou sobrecarregam a jornada. |

## Demos oficiais — cenas observadas e impedimentos

As rodadas foram limitadas às referências abaixo. O GIF e o MP4 acessíveis foram baixados apenas para cache temporário e analisados por APIs de arquivo/imagem do Windows, sem instalar ferramentas ou operar interfaces externas. Nenhuma mídia ultrapassou 30 MB. Os tempos abaixo são posições na mídia, não medidas de desempenho do produto; a reprodução pode estar acelerada ou editada.

| Referência e acesso | Cenas efetivamente observadas | Jornada coberta e limite |
| --- | --- | --- |
| **Superset — Design Mode.** [Página oficial](https://docs.superset.sh/browser), [MP4 oficial](https://docs.superset.sh/videos/design-mode.mp4). Arquivo de 1.188.065 bytes; duração de 11 s; versão/data exata não confirmada. | **Observado em amostras de demo oficial:** 0 s: browser e terminal do agente lado a lado. 2 s: Design ativo e elemento destacado. 5 s: cartão contextual com texto e seletor de sessão Claude. 8 s: nova mensagem de feedback/contexto aparece no terminal. 10 s: outro elemento selecionado com cartão de prompt. | Formular contexto e solicitar correção visual. **Não verificado na demo:** mudança final produzida, testes, diff ou aplicação. O estado inicial já contém sessão e mensagens anteriores. |
| **Vibe Kanban — QA com browser.** [Artigo oficial, 27/11/2025](https://www.vibekanban.com/blog/does-playwright-mcp-unlock-autonomous-qa), [GIF oficial](https://www.vibekanban.com/images/posts/does-playwright-mcp-unlock-autonomous-qa/browser-demo.gif). Arquivo de 4.935.001 bytes, 328 frames e 21,87 s. Referência histórica. | **Observado em amostras de demo oficial:** 0 s: aplicação-alvo em sign-in, Vibe Kanban mostra pedido de QA e chamadas de ferramentas. 5,47 s: browser-alvo exibe onboarding. 10,93 s: captura/câmera. 16,40 s: lista de contatos. 21,80 s: onboarding novamente. Logs visíveis incluem navegação, cliques e screenshots de Playwright; Stop e fila para próximo turno aparecem. | Acompanhar trabalho/contexto e verificações visuais. **Não verificado na demo:** conexão de conta do agente, paralelismo entre agentes, bloqueio, correção ou integração final. O sign-in visto é da aplicação-alvo, não do Vibe Kanban. |
| **Conductor — AskUserQuestion, versão 0.31.3 de 19/01/2026.** [Publicação oficial](https://www.conductor.build/changelog/0.31.3-askuserquestion-improvements), [MP4 referenciado](https://conductor-marketing.t3.tigrisfiles.io/uploads/1768852746133-Clipboard-20260117-203656-672.mp4). | **Não verificado — impedimento concreto de acesso:** host de mídia falhou TLS nesta sessão: HttpWebRequest retornou erro de envio; curl/Schannel, `SEC_E_INVALID_TOKEN`; urllib/OpenSSL, `WRONG_VERSION_NUMBER`. A publicação foi aberta, mas nenhum frame do vídeo foi observado ou baixado. | A publicação descreve interação de pergunta ao usuário; a demonstração não foi verificada. A rodada foi encerrada após essas tentativas, sem inferir comportamento do vídeo a partir do texto. |

### Complemento por storyboards oficiais

Não havia browser habilitado no inventário CUA desta sessão, impedindo a observação do player por UI. O HTML público do YouTube expôs storyboards, mas não um URL direto de stream reproduzível pela análise de arquivo usada aqui. Foram examinadas somente as imagens públicas de preview, sem instalar downloader, contornar login ou afirmar reprodução do vídeo. As folhas completas de preview abrangem a duração anunciada, com baixa resolução e intervalos de amostragem; isso não representa um percurso completo executado.

| Referência e método | Cenas observadas em frames de preview oficial | Limite |
| --- | --- | --- |
| **Superset — What is Superset in 60s?**, publicado em 02/01/2026 pelo canal Superset e vinculado ao [overview oficial](https://docs.superset.sh/overview). [Vídeo](https://www.youtube.com/watch?v=7jhPfMDwTUc), 58 s. Sete folhas de preview, frames de 295 × 180 pixels a intervalos de 1 s; total inferior a 0,3 MB. | Aproximadamente 12–19 s: terminal inicialmente vazio recebe comandos/saída. 20–26 s: panes divididos; CLI Claude aparece à direita. 27–43 s: alternância de sessão e saída em dois panes. 44–58 s: menu, mudança de terminal e nova saída. | Texto pequeno não permite confirmar comandos, modelo ou conta. Nenhum cartão de bloqueio, revisão final ou integração foi observado. |
| **OpenAI — Introducing the Codex app**, publicado em 02/02/2026; canal OpenAI e [catálogo oficial](https://learn.chatgpt.com/videos). [Vídeo](https://www.youtube.com/watch?v=HFM3se4lNiw), 261 s. Quinze folhas de preview, frames de 320 × 180 pixels a intervalos de 2 s; total inferior a 0,8 MB. | Aproximadamente 18–48 s: projeto existente, composer, captura de voz e tarefa enviada. 54–64 s: saída e mudança de thread. 92–108 s: diff, arquivos e controles Open/Commit. 124 s: tela solicitada no simulador. 216–220 s: formulário de automação. | O botão Commit não comprova commit efetuado. Resultado no simulador não comprova checks. Conexão de conta, resolução de bloqueio e integração final não foram observadas. |

### Cobertura efetivamente observada nas referências acessíveis

Esta tabela registra somente cenas, usando as fontes acima. **Parcial** significa que algum trecho da jornada foi visto; **não verificado** significa ausência de cena suficiente nesta pesquisa. Não se converte descrição da documentação em ação observada.

| Jornada | Superset: MP4 e preview | Vibe Kanban: GIF | Codex app: preview | Conductor: MP4 de diff |
| --- | --- | --- | --- | --- |
| Abrir projeto e conectar | Não verificado; sessão já existente. | Não verificado; workspace já existente. | Parcial: projeto existente e New Thread; abertura de repositório/login não vistos. | Não verificado; workspace já existente. |
| Formular trabalho e contexto | Parcial: seleção visual e feedback enviados ao terminal. | Parcial: pedido de QA já presente. | Parcial: composição e envio de tarefa vistos. | Não verificado; alterações anteriores à mídia. |
| Acompanhar paralelismo e atenção | Parcial: panes/sessões e saída; atenção não vista. | Parcial: logs; paralelismo entre agentes não visto. | Parcial: mudança de thread; trabalho concorrente efetivo não confirmado. | Não verificado. |
| Resolver bloqueio ou falha | Não verificado. | Não verificado. | Não verificado. | Não verificado. |
| Revisar, corrigir e integrar | Parcial: pedido de correção visual; resultado não visto. | Não verificado. | Parcial: diff e resultado visual; correção/commit/integração não confirmados. | Parcial: abrir diff, mudar layout e selecionar commit; correção/integração não confirmadas. |

**Retrato do aceite antes da consolidação:** havia cenas de quatro produtos, três mídias amostradas e previews Superset/Codex. As cinco jornadas completas em três referências não estavam demonstradas. A nova rodada acrescentou workflow completo Conductor e demos Cline/Superset, sem transformar lacunas em sucesso. A [conclusão atual](d0-conclusion.md) registra cobertura, critério substituído e encerramento como pesquisa; a antiga tentativa AskUserQuestion permaneceu encerrada por TLS.

## Decisões candidatas para o protótipo

Estas são hipóteses do Orchestrix, e não conclusões de desempenho dos concorrentes:

1. Entrada por objetivo: projeto, descrição, contexto e uma política padrão compreensível; opções avançadas sob demanda.
2. Atenção com motivo: separar executando, sem sinal recente, pergunta, aprovação, erro e pronto para revisar.
3. Execução com origem explícita: agente/runtime, identidade, modelo, reasoning e contexto, inclusive quando algum dado não é conhecido.
4. Revisão vinculada à versão: arquivos alterados, evidências de verificação, comentários pendentes/enviados e ação de pedir correção.
5. Integração com semântica clara: atualizar branch de trabalho, aplicar à branch alvo e criar PR devem ter ações e consequências distinguíveis.

Antes de chamar a comparação de benchmark hands-on, faltam execução controlada das jornadas, observação de falhas reais e avaliação de uso. Autenticação de múltiplas contas, leitura de quotas, retomada, cancelamento e aprovação precisam de spikes por adapter; estes dados não devem ser presumidos por semelhança visual.
