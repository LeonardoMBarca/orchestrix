# OX-D01 — Posicionamento e decisões de produto

Rodada complementar consultada em **8 de outubro de 2026**. A pesquisa aprofunda páginas oficiais sobre coordenação, configuração e workflow, incluindo Antigravity como nona referência. Não houve instalação, login, inferência ou teste de contas. Capacidades abaixo são **documentadas**; escolhas do Orchestrix são **propostas**, sujeitas à avaliação D1. O encerramento e a revisão explícita do método estão na [conclusão de D0](d0-conclusion.md); a [proveniência](landscape-and-provenance.md) atualiza versões/superfícies, incluindo Agent Canvas OpenHands.

## Hipótese de posicionamento

O Orchestrix deve ser um aplicativo para conduzir um objetivo de desenvolvimento até um resultado verificável: organizar tarefas, escolher execução conforme preferências, preparar contexto, verificar, revisar, corrigir e integrar. A pessoa precisa entender cada escolha e conseguir ajustá-la sem coordenar manualmente todas as conversas.

Esse posicionamento depende da combinação e da qualidade do fluxo. Executar vários agentes, escolher modelo/thinking, usar worktrees, encadear tarefas e revisar diffs já aparecem nas referências. Não há base nesta rodada para anunciar exclusividade, melhor qualidade de código, maior velocidade ou economia garantida. Uma aplicação Windows também não é exclusiva: Codex documentou sua versão nativa em 04/03/2026, e Cline Desktop documenta Windows em beta. [Codex changelog](https://learn.chatgpt.com/docs/changelog), [Cline Desktop](https://docs.cline.bot/usage/cline-desktop)

A hipótese principal a validar é: **controle compreensível e rastreável do ciclo completo, útil desde a primeira assinatura, que possa crescer para diferentes runtimes e contas sem mudar o modo de trabalhar**.

## O que a nova evidência muda

| Referência e superfície examinada | Capacidade documentada relevante | Consequência proposta para o Orchestrix |
| --- | --- | --- |
| **Conductor — aplicativo/coordenador de workspaces** | Distingue harness, sessão e modelo; cada chat usa um harness/modelo. Oferece Plan/Fast e reasoning conforme agente/modelo, além de Goals para Codex local. Chats no mesmo workspace compartilham branch/código. Checks agrega Git, CI, review threads e todos conforme integrações. [Harnesses](https://www.conductor.build/docs/reference/harnesses), [Agent modes](https://www.conductor.build/docs/concepts/agent-modes), [Checks](https://www.conductor.build/docs/reference/checks) | Controles por sessão e revisão agregada são referências existentes. Adaptar divulgação por capabilities e esclarecer propriedade de escrita; investigar o vínculo entre configuração, versão verificada e aprovação do resultado. A fonte de harnesses descreve um app Mac; Windows não foi estabelecido nessa superfície. |
| **Codex app — produto com runtime Codex** | O changelog registra Windows nativo, threads paralelas, worktrees, revisão e automações; automações podem selecionar modelo/reasoning e execução local/worktree. Não são recursos exclusivos da proposta Orchestrix. [Changelog, entradas de 04 e 12/03/2026](https://learn.chatgpt.com/docs/changelog) | Adotar continuidade projeto → execução → revisão e atenção contextual. A interoperabilidade entre runtimes é um objetivo adicional, a comprovar por adapters; não apresentar o app Codex como simples chat ou Windows como lacuna universal. |
| **Superset — produto, CLI e skill de coordenação** | `superset:orchestrate` permite coordenador Claude/Codex, workers mistos, branches isoladas, acompanhamento e resultado estruturado verificado antes de integrar. CLI oferece modelo/esforço, resume/fork e handoff a partir de saída recente com limite de contexto. FAQ informa Windows planejado sem data; worktree não é sandbox. [Orchestration](https://docs.superset.sh/orchestration), [CLI reference](https://docs.superset.sh/cli/cli-reference), [FAQ](https://docs.superset.sh/faq) | É sobreposição direta com a visão, inclusive além da UI. Adaptar prompts delimitados, handoffs e resultado revisável. Investigar o benefício de políticas e evidências mantidas pelo Core, compreensíveis sem depender de uma conversa coordenadora. |
| **Vibe Kanban — workflow de trabalho e agentes** | Chat documenta escolha de agente, variantes com modelos/prompts/comportamentos, Queue/Stop e cartões de aprovação. Comunicado de 10/04/2026 mantém workspaces locais e anuncia transição comunitária após encerramento dos serviços remotos. [Chat](https://www.vibekanban.com/docs/workspaces/chat-interface), [Comunicado](https://www.vibekanban.com/blog/shutdown) | Adotar fila e atenção explícitas; adaptar variantes para perfis com efeito claro na próxima tentativa. Board, chat e aprovação já existem. Não assumir disponíveis os fluxos cloud ainda presentes em documentação histórica. |
| **cmux — terminal nativo macOS e API de controle** | Notificações possuem ciclo recebido/não lido/lido e navegação ao workspace. Restore reconstrói layout/metadados; agentes suportados podem retomar pelo ID nativo capturado. Não faz checkpoint de processos arbitrários. [Notifications](https://cmux.com/docs/notifications), [Session restore](https://cmux.com/docs/session-restore), [Getting started](https://cmux.com/docs/getting-started) | Adotar navegação por atenção e a distinção entre restaurar interface, sessão e processo. Adaptar ao fluxo guiado do Orchestrix; a referência examinada exige macOS, sem extrapolar para outros produtos da marca. |
| **OpenHands — plataforma e SDK; guias do SDK examinados** | Model Routing oferece roteador por regras e extensão própria, com desenvolvimento ativo declarado. Iterative Refinement demonstra implementação → crítica → correção, com limite de iterações. [Model routing](https://docs.openhands.dev/sdk/guides/llm-routing), [Iterative refinement](https://docs.openhands.dev/sdk/guides/iterative-refinement) | Roteamento e review iterativo já são padrões documentados. Adaptar limites e separação de papéis; a nota dada por um agente não substitui checks nem aprovação de integração. Um exemplo de SDK não comprova ergonomia de um desktop Windows. |
| **Cline — família com Desktop, Kanban, CLI e SDK** | SDK Teams documenta coordenador, especialistas, task board, mensagens e resultados persistidos por sessão. CLI publica exemplo de pipeline com modelos/thinking por fase. Kanban documenta dependências e review/ship. Desktop possui sessões paralelas e Windows beta. [Teams](https://docs.cline.bot/sdk/guides/multi-agent-teams), [Model orchestration](https://docs.cline.bot/cli/samples/model-orchestration), [Kanban workflow](https://docs.cline.bot/kanban/core-workflow), [Desktop](https://docs.cline.bot/usage/cline-desktop) | Corrigir qualquer classificação restrita a agente individual/IDE. Adaptar escopo por fase e cadeia de tarefas. Importar conversa Codex/Claude para continuar no harness Cline não equivale a supervisionar o processo original ou manter sua identidade de sessão. |
| **OpenCode — runtime extensível com agentes e clientes** | Agentes primários/subagentes podem configurar prompt, modelo e acesso a ferramentas; subagentes herdam modelo quando não configurado. Documenta execução Windows direta e recomenda WSL; desktop pode conectar a servidor WSL. [Agents](https://opencode.ai/docs/agents/), [Windows/WSL](https://opencode.ai/docs/windows-wsl/) | Adotar perfis especializados com herança explícita e capacidades limitadas. Diferenciar a política do Orchestrix da configuração interna de um runtime; não converter Windows nativo e WSL em uma única combinação de suporte. |

As categorias se sobrepõem: runtime, SDK integrável e aplicativo de coordenação podem pertencer à mesma família. Comparar a superfície concreta evita atribuir uma capacidade do SDK a toda interface do produto.

**Não verificado nesta rodada:** isolamento de múltiplas contas do mesmo provedor, agregação de capacidade compartilhada e enforcement de todos os controles em cada produto. Isso é ausência de evidência deste levantamento, não afirmação de ausência do recurso. Uma variante, um preset, uma sessão e uma worktree não comprovam uma conta diferente nem cota adicional.

## Antigravity como referência adicional

Consulta oficial complementar em **8 de outubro de 2026**, motivada pelo runtime citado na visão do usuário. Antigravity é uma família de produtos: o anúncio de 19/05/2026 apresenta **Antigravity 2.0**, aplicativo independente do IDE para agentes síncronos/assíncronos, disponível em Windows, macOS e Linux. Distingue essa aplicação do antigo Agent Manager e do Antigravity IDE; a documentação atual do IDE descreve editor, terminal, browser e agentes paralelos. [Anúncio 2.0](https://www.antigravity.google/blog/introducing-google-antigravity-2), [IDE](https://www.antigravity.google/docs/ide/overview/)

| Dimensão | Evidência oficial e consequência proposta |
| --- | --- |
| Organização e contexto | O 2.0 documenta subagentes dinâmicos e tarefas assíncronas. Projects organiza pastas, configurações e conversas; conversas podem usar pastas locais ou novas worktrees. **Adaptar** escopo explícito por tarefa/tentativa, mantendo Context Pack do Orchestrix distinto do contexto interno do runtime. Não assumir isolamento de todas as pastas por uma worktree. [Anúncio](https://www.antigravity.google/blog/introducing-google-antigravity-2), [Projects](https://www.antigravity.google/docs/projects/) |
| Modelo e thinking | A disponibilidade de modelos depende do plano; há variantes identificadas como thinking e seleção por conversa. Mudar o modelo durante a execução conserva o anterior até o turno terminar ou ser cancelado. **Adotar** configurações estáveis por tentativa e **adaptar** controles conforme capabilities; a página não estabelece um controle universal de esforço. Reforça POS-04/POS-05. [Models](https://www.antigravity.google/docs/models/) |
| Artefatos e revisão | Planos, diffs e gravações podem ser inspecionados e comentados; Planning/Fast e Request review/Always proceed organizam o fluxo de revisão. **Adotar** feedback junto ao resultado e **adaptar** profundidade conforme tarefa. Artefato narrativo não comprova check executado ou integração válida; essa evidência continua necessária no Orchestrix. [Artifacts](https://www.antigravity.google/docs/artifacts/), [Artifact review](https://www.antigravity.google/docs/artifact-review/) |
| Windows e permissões | O app possui versão Windows, mas a documentação informa que o sistema atualizado de permissões está em macOS/Linux; Windows mantém configurações anteriores, incluindo revisão de comandos e sandbox. **Adaptar** capabilities por combinação runtime/versão/ambiente; suporte desktop não comprova enforcement. [Anúncio](https://www.antigravity.google/blog/introducing-google-antigravity-2), [Agent settings](https://www.antigravity.google/docs/agent-settings) |

**Limite de integração:** o Google anuncia CLI, SDK e API oficiais. Isso é evidência de superfícies da própria família, sem demonstrar que um aplicativo terceiro consegue supervisionar suas sessões com todas as mesmas capacidades. Nesta rodada não foram examinados os contratos dessas interfaces, autenticação/isolamento de contas, origem de cobrança, quotas, cancelamento/recuperação ou acesso programático aos artefatos. Não houve instalação, login ou execução. [Anúncio das superfícies](https://www.antigravity.google/blog/introducing-google-antigravity-2)

**Decisão proposta:** considerar Antigravity referência direta de coordenação, sem reduzir o produto a IDE nem prometer adapter disponível. Para D1, aplicar POS-04/05/07/09/16 também a uma conexão Antigravity **simulada**, explicitando dados desconhecidos. **Descartar para o MVP** reproduzir o IDE/browser completos; a hipótese do Orchestrix continua sendo a coerência do workflow entre runtimes. A viabilidade do adapter permanece pendência técnica de validação futura, separada desta pesquisa D0.

## Valor com uma única assinatura

O primeiro fluxo deve funcionar com uma Connection e um runtime. O scheduler pode executar as etapas em sequência, mantendo tarefas e contexto organizados. Paralelismo e novas contas ampliam possibilidades quando validados; não são pré-requisitos para usar o aplicativo.

| Necessidade | Comportamento proposto | Como avaliar o benefício |
| --- | --- | --- |
| Evitar repetir a coordenação entre conversas | Template simples conduz implementar → verificar → revisar → corrigir, com limites e próxima ação visíveis. Planejamento extenso é opcional para trabalho pequeno. | Registrar instruções/copias manuais necessárias para terminar a mesma tarefa; não prometer ganho antes de medir. |
| Usar escolhas compatíveis com a conexão | Preset escolhe entre modelos/esforços realmente disponíveis, com preferência manual e explicação do motivo. | Usuário consegue prever a próxima escolha e corrigir uma preferência inadequada. Nenhuma opção fictícia ou fallback oculto. |
| Separar implementação e avaliação | Revisão em sessão/contexto separado pode usar a mesma conta e o mesmo modelo, se necessário. Mostrar diversidade apenas quando observável. | Usuário distingue revisão separada de diversidade de modelo. Avaliar findings e checks; não assumir independência estatística. |
| Acompanhar uma fila sem vigiar chats | Mostrar dependência, capacidade ocupada, limite observado e falta de informação com ações diferentes. | Usuário identifica por que a próxima etapa espera e o que pode fazer. Mais sessões não aumentam a cota declarada. |
| Reaproveitar trabalho confirmado | Handoff inclui objetivo, decisões, diff e evidências pertinentes, com fontes e versão; conversa anterior continua acessível. | Usuário entende o que foi enviado e detecta contexto desatualizado sem ler todo o transcript. Economia de tokens permanece hipótese. |

Revisões e agentes adicionais também consomem uso. Um template deve permitir ajuste de profundidade e limites de correção. Não converter tokens em dinheiro estimado de assinatura, anunciar economia percentual ou migrar silenciosamente para API paga quando a assinatura chega ao limite.

## Matriz de adotar, adaptar e descartar no MVP

As referências justificam padrões, não demonstram vantagem empírica do Orchestrix.

| Decisão | Tratamento | Referência/razão | Efeito no escopo |
| --- | --- | --- | --- |
| Objetivo com etapas e resultado revisável | **Adotar** | Superset, Cline Kanban e OpenHands mostram coordenação/etapas além de chat. | Templates poucos e claros; DAG como vista especializada, não entrada obrigatória. |
| Escolhas por capabilities | **Adotar** | Conductor e OpenCode explicitam diferenças e herança por agente/modelo. | Opções indisponíveis têm explicação; dados desconhecidos permanecem desconhecidos. |
| Presets de preferência por papel/fase | **Adaptar** | Variantes Vibe Kanban, Cline por fase e controles Codex/Superset. | Presets Ágil/Padrão/Cuidadoso expressam intenção; não garantem qualidade, tempo ou custo. |
| Coordenação proposta por um agente | **Adaptar** | Superset skill e Cline Teams são referências diretas. | Proposta do coordenador passa por política e estado do Core; não substitui autoridade sobre permissões/aceite. |
| Contexto entre etapas | **Adaptar** | Superset handoff e Cline importação/teams demonstram continuidade. | Context Pack por tentativa com fontes/versão; evitar exportar todo histórico por padrão. |
| Atenção e Queue/Stop | **Adotar** | cmux e Vibe Kanban tornam atenção/atividade explícitas. | Motivo, ação e vínculo à tentativa; ausência de sinal não vira pausa. |
| Revisão próxima do diff | **Adotar** | Conductor Checks e workflows Cline/Codex. | Findings/checks vinculados ao artefato; alteração posterior exige nova avaliação. |
| Integração orientada somente por prompt ao agente | **Descartar** | Cline Kanban demonstra outra escolha possível; Orchestrix necessita recuperação e aprovação rastreáveis. | Core executa integração serial, verifica base/candidato e reconcilia Git. |
| Paralelismo máximo como valor inicial | **Descartar** | Não resolve o caso de uma única capacidade/cota compartilhada. | Sequência útil com uma conexão; concorrência só onde houver independência e suporte demonstrados. |
| Copiar terminal/IDE, browser, cloud ou frota remota completos | **Descartar para o MVP** | São escopos amplos presentes nas referências, sem necessidade demonstrada aqui. | Leitura/diff e desenvolvimento assistido primeiro; edição leve conforme D1. |
| Vários transportes por runtime e oito adapters simultâneos | **Descartar para o MVP** | A pesquisa de integração já recomenda contrato pequeno e validação incremental. | Primeiro runtime comprovado, depois um segundo que teste interoperabilidade. |
| Multiplicar sessões e chamar isso de multiaccount | **Descartar** | Sessão não prova identidade, autenticação ou capacidade independentes. | Connection, session e capacity group permanecem distintos; multiaccount depende de OX-002. |

## Decisões propostas para D1

1. **Fluxo principal:** Projeto → Objetivo → Plano/etapas → Execução → Revisão/correção → Aplicação. A pessoa pode iniciar uma correção pequena sem desenhar um DAG ou configurar todos os papéis.
2. **Configuração gradual:** mostrar preset e conexão disponível no início; revelar modelo, esforço, contexto e política em um resumo inspecionável. Preferências por projeto/papel podem sobrescrever defaults com origem explícita.
3. **Escolha automática explicável:** iniciar com regras simples de disponibilidade, compatibilidade, preferência e tipo de etapa. Não prometer selecionar o “melhor modelo”; permitir escolha fixa e registrar por que uma alternativa foi excluída.
4. **Política estável por tentativa:** alterações de preferência valem para novas tentativas. O histórico conserva o snapshot usado, inclusive solicitado, configuração resolvida e informação não observada.
5. **Resultado como centro da revisão:** abrir tarefa mostra alteração, checks, findings e versão relacionados; conversa e log ficam acessíveis como apoio. Aceitar resultado de tarefa e aplicar resultado do Run à branch alvo são operações distintas.
6. **Windows como compromisso verificável:** priorizar projeto existente, paths com espaços/acentos, teclado, escala e lifecycle nativos. WSL é ambiente explícito por Connection, sem conversão implícita. Suporte desktop Windows e enforcement do sandbox são evidências separadas.

## Requisitos verificáveis para o protótipo e piloto D1

Os testes abaixo usam cenários simulados e tarefas de uso. Confirmam entendimento e controles da interface; não certificam runtimes, autenticação ou supervisão do aplicativo de produção.

| ID | Cenário de avaliação | Critério de aceite |
| --- | --- | --- |
| POS-01 | Abrir projeto com somente uma conexão disponível. | Usuário inicia e conclui o fluxo simulado sem cadastrar outra conta; fila sequencial funciona e explica a etapa seguinte. |
| POS-02 | Pedir uma correção pequena e uma feature com dependências. | Usuário distingue os templates e identifica objetivo/aceite; planejamento detalhado não é exigido na correção pequena. |
| POS-03 | Escolher Automático e depois modelo fixo. | Resumo explica escolha, preferência e alternativas; opção não suportada não parece executável. |
| POS-04 | Runtime informa modelo/esforço parcialmente. | UI distingue solicitado, configuração resolvida e não informado. Não chama configuração de telemetria da execução nem expõe reasoning interno como controle de thinking. |
| POS-05 | Mudar preset durante execução. | Tentativa atual conserva snapshot; nova tentativa recebe nova preferência. Usuário prevê corretamente o efeito antes de salvar. |
| POS-06 | Duas sessões usam a mesma conexão. | UI mostra sessões sem anunciar contas/cotas extras. Capacidade compartilhada e limite desconhecido são identificáveis; escolha de cobrança não muda silenciosamente. |
| POS-07 | Inspecionar contexto de implementação e revisão. | Usuário localiza fontes, versão e escopo enviado; revisão recebe diff/evidências pertinentes, sem sugerir acesso universal ao projeto. |
| POS-08 | Evento novo chega durante inspeção de diff. | Seleção, foco e posição permanecem; atenção revela motivo/ação e leva à tentativa correta. |
| POS-09 | Worker termina, mas um check falha. | Tarefa não aparece como pronta para aplicação; usuário localiza a evidência e solicita correção da versão correta. |
| POS-10 | Candidato muda depois da revisão, ou branch alvo avança. | Aprovação anterior não habilita aplicar silenciosamente; UI identifica versão desatualizada e caminho de revalidação. |
| POS-11 | Aceitar uma tarefa dentro de um Run com outras tarefas abertas. | Usuário distingue aceitação interna de tarefa e aplicação final à branch alvo; não interpreta uma como a outra. |
| POS-12 | Timeout/perda de conexão antes de confirmar cancelamento. | Estado fica desconhecido/reconciliando, com motivo. UI não afirma pausa/término nem propõe duplicar automaticamente o worker. |
| POS-13 | Uma assinatura chega ao limite observado. | Usuário entende aguardar/ajustar próxima execução ou selecionar outra conexão autorizada; não há fallback automático para API. |
| POS-14 | Revisão usa mesma conta/modelo da implementação. | UI identifica sessão separada e diversidade conhecida/desconhecida corretamente; usuário não confunde runtime diferente com modelo diferente. |
| POS-15 | Operar por teclado em layout compacto/amplo e texto ampliado a 200%. | Ações principais e foco permanecem acessíveis; retornos de diálogo são previsíveis. Logs/diff extensos não escondem os controles do fluxo. |
| POS-16 | Projeto Windows com path longo/espaços/acentos e conexão WSL simulada. | Ambiente/cwd são explícitos; UI não sugere conversão automática ou suporte validado onde existe somente simulação. |

No piloto, registrar conclusão, ajuda necessária, interpretações erradas e erros de controle/aprovação para POS-01 a POS-16. Qualquer aceitação do artefato errado, execução duplicada por incompreensão ou fallback de cobrança não compreendido é problema crítico a corrigir para a saída inicial de D1, antes de retomar M0 e depois implementar o Desktop. Testes de uso devem comparar o fluxo proposto com a rotina real do participante; estas referências documentais não fornecem baseline de desempenho.

## Lacunas que permanecem abertas

O pacote proposto ainda precisa provar relevância para alguém satisfeito com seu agente atual, inclusive com uma única assinatura. As demos não certificam quinze jornadas completas de concorrentes. A [conclusão](d0-conclusion.md) preserva esses limites e encerra a pesquisa por critério explicitamente consolidado. Compatibilidade de conexões e avaliação do protótipo seguem M0/D1. O [spike Codex](./runtime-harness-results.md) é delimitado; não generalizar a multiaccount, código real ou todos os runtimes. O [contrato oficial de contas](subscription-account-ux.md) fundamenta um candidato de onboarding, sem teste de capacidade.

Esta rodada orienta prioridades e retifica comparações superficiais. Seus cenários POS-01 a POS-16, junto a ACC-01 a ACC-07, alimentam a próxima etapa D1; o [status atual](../DEVELOPMENT_STATUS.md) registra os gates ainda pendentes.
