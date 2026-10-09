# Pesquisa complementar de concorrentes — 9 de outubro de 2026

**Existem soluções com a mesma proposta central do Orchestrix.** A pesquisa complementar encontrou sobreposição em coordenação entre runtimes, múltiplas contas, escolha automática de execução, contexto de projeto e revisão. Jackalope e Cobalt não estavam no mapeamento inicial de D0; Superset já estava, mas seu suporte documental a múltiplas contas ainda não havia sido registrado.

Este relatório complementa a [conclusão de D0 de 08/10](d0-conclusion.md), o [posicionamento](product-positioning.md) e a [matriz de proveniência](landscape-and-provenance.md). A rodada inicial examinou nove famílias; as duas novas referências ampliam o levantamento documental para onze. As demos da rodada anterior conservam sua cobertura original.

## Método e limites

Consulta à web em **09/10/2026**, priorizando páginas dos fornecedores, documentação oficial e repositórios oficiais. Pergunta: quais produtos reúnem agentes de código e contas próprias, coordenam trabalho e permitem configurar modelo, raciocínio e contexto dentro de um aplicativo?

- **Documentado:** capacidade descrita pela fonte, sem validação própria de funcionamento.
- **Não verificado:** a investigação não estabeleceu o recurso ou seu comportamento; isso não significa ausência.
- **Inferência:** interpretação comparativa da pesquisa.
- **Proposta:** decisão ou experimento recomendado para o Orchestrix, ainda sem resultado.

Não houve instalação, autenticação, inferência, execução de tarefas ou teste de quotas dos concorrentes nesta rodada. Não foram medidos desempenho, produtividade, conforto, estabilidade ou economia. Links web e branches `main` são mutáveis; a data é de consulta, sem fixar uma versão instalada. As publicações de contas do Cobalt e do Jackalope têm datas próprias indicadas abaixo. Licenças, preços e condições para reutilização dos dois novos produtos não foram auditados; nenhum código foi incorporado.

## Comparação documental

| Produto / superfície | Sobreposição documentada | Disponibilidade ou modo de operação | Limite relevante |
| --- | --- | --- | --- |
| **Jackalope — desktop** | Seleção automática de agente/modelo/conta com restrições e capacidade reportada; decisões por regras locais ou agente, preferências por projeto e handoff configurável após quota reconhecida. [Roteamento](https://jackalope.dev/knowledge/task-routing-and-quotas/). Perfis separados Codex/Claude Code e conta preservada na continuação. [Contas, 06/09/2026](https://jackalope.dev/blog/work-and-personal-accounts/). | Downloads Windows/macOS/Linux; **early access**, dependente de aprovação ou convite. [Download](https://jackalope.dev/download/). | Capacidade desconhecida não equivale a ilimitada; a fonte deixa comparações reais de custo/qualidade abertas. Perfis não são isolamento do sistema operacional. Funcionamento não foi testado aqui. |
| **Superset — desktop, CLI e skill** | Coordenador Claude Code/Codex com workers mistos e workspaces isolados. [Orquestração](https://docs.superset.sh/orchestration). Perfis múltiplos Claude Code/Codex e quota por conta. [Usage](https://docs.superset.sh/usage). Seleção de modelo/thinking e integração Antigravity CLI. [Agentes](https://docs.superset.sh/agent-integration). | Orquestração incluída no desktop; CLI pode ser conduzida pelo agente. | A troca documentada escolhe o default para novos lançamentos; agentes ativos conservam a conta até relançar. Histórico Claude compartilhado; alguns planos não expõem quota. Não foi confirmado balanceamento automático de contas/modelos por quota nessas fontes. |
| **Conductor — aplicativo e workspaces** | Camada acima de Claude Code, Codex, Cursor e OpenCode; organiza sessões, modelos, branches, diffs, checks e PRs. [Harnesses](https://www.conductor.build/docs/reference/harnesses). | A referência descreve aplicativo Mac e uso de conta/chave do provedor. | Cada chat usa um harness/modelo; chats do mesmo workspace compartilham branch/código. Pool de contas e roteamento automático conjunto não foram estabelecidos nesta fonte. |
| **Cobalt — cloud harness** | Várias contas por agente, incluindo duas contas ChatGPT para Codex; conta, modelo e reasoning por tarefa; estado de conta limitada e vínculo da execução à conta escolhida. [Contas, 15/08/2026](https://cobaltcode.ai/blog/multiple-agent-accounts). | **Cloud-first**, com ambientes dedicados e workspaces compartilhados. | A fonte apresenta troca deliberada quando há limite e informa que não há failover silencioso entre contas salvas. Não comprova seleção automática conjunta de conta/modelo/contexto. |
| **Cline Kanban — workflow local** | Agente decompõe tarefas, dependências iniciam trabalho, worktree por tarefa, diff/comentários retornam ao agente e entrega por Commit/PR. [README oficial](https://github.com/cline/kanban/blob/main/README.md). | Servidor local aberto no navegador; identificado como **research preview**. | Fluxos de auto-commit e entrega orientada por prompt diferem dos controles planejados no Orchestrix. Não foi comprovado pool de assinaturas; a pesquisa não certifica enforcement ou integração sem conflitos. |
| **OpenHands Agent Canvas — aplicativo** | Workspace visual com agentes paralelos/worktrees, automações e conexão a Claude Code, Codex e Gemini CLI via ACP, usando assinaturas existentes. [Produto](https://www.openhands.dev/product/canvas). | Aplicativo para Windows/macOS/Linux; backends local, remoto e cloud. | Não foi estabelecida nesta página uma política automática unificada de conta/modelo/thinking/contexto. Capacidades do SDK continuam separadas das do aplicativo; multiaccount não foi testado. |

## Contexto e thinking: comparação precisa

Jackalope documenta seleção automática de orientações relevantes, ajustes manuais, instruções/lições do projeto e inspeção do contexto entregue; atualizações afetam tarefas futuras. Sua demonstração usa dados de exemplo e não executa um agente nativo. Isso também sobrepõe parte do Context Pack proposto. [Contexto de projeto](https://jackalope.dev/features/project-context-for-coding-agents/).

Os presets Quick/Balanced/Thorough do Jackalope são descritos como instruções de profundidade do trabalho. Não comprovam seleção automática do parâmetro de reasoning do provedor. [Esforço da tarefa](https://jackalope.dev/knowledge/task-composer-and-effort-levels/). Superset e Cobalt documentam controles de modelo/raciocínio, conforme as fontes da matriz. Nenhuma dessas evidências demonstra superioridade da escolha automática ou equivalência completa a todos os contratos do Orchestrix.

**Inferência:** Jackalope é a proposta publicada mais próxima encontrada nesta rodada para decisões automáticas; Superset é uma referência direta para coordenação e contas; Cobalt é uma referência adicional para contas e execução cloud. Essa ordem orienta investigação, sem representar ranking de qualidade ou recomendação de compra. Acesso antecipado e research preview precisam ser considerados ao planejar testes.

## O que muda no posicionamento e no plano

As decisões abaixo são recomendações de pesquisa, sem substituir os ADRs aceitos ou registrar recursos implementados.

| ID | Decisão proposta | Aplicação no plano |
| --- | --- | --- |
| CMP-01 | Não apresentar vários agentes, múltiplas contas, Windows, worktrees, contexto inspecionável ou roteamento automático como exclusividade comprovada. | Atualizar o posicionamento e avaliar a qualidade da combinação do fluxo. |
| CMP-02 | Priorizar Jackalope/Superset para decisões locais e contas; Cobalt para contas/cloud; Conductor/Cline/Canvas para organização e revisão. | Escolher comparadores conforme plataforma, acesso e recurso da tarefa; registrar impedimentos sem inventar resultados. |
| CMP-03 | Avaliar uma hipótese concreta: concluir desenvolvimento com menos coordenação e erros de entendimento, mantendo controle e conforto. | OX-D05 avalia compreensão do protótipo; OX-017 mede o fluxo real antes de alegar vantagem. Aparência agradável depende de uso, além dos temas. |
| CMP-04 | Medir qualidade das decisões de execução e contexto quando existirem implementações comparáveis. | M3/M4: escolhas compatíveis, explicações, intervenção manual, consumo reportado e retomada. Não prometer economia ou o melhor modelo sem dados. |
| CMP-05 | Preservar o primeiro ciclo útil com uma conexão, a arquitetura Core/desktop e os gates existentes. | Concorrência confirma a categoria; não demonstra viabilidade dos adapters próprios nem justifica ampliar escopo antes de D1/M0/M1/M2. |

## Protocolo proposto para comparação futura

Esta é uma proposta de validação vinculada a **OX-D05, OX-017 e M3/M4**, não um benchmark já executado nem um novo requisito de saída de D0/D1. O piloto humano continua pendente; o próximo trabalho do backlog permanece o mesmo.

| Caso | O que observar | Momento adequado |
| --- | --- | --- |
| Correção pequena com uma conta | Tempo para iniciar/concluir, configuração, prompts e cópias manuais, ajuda necessária, entendimento do resultado e conforto relatado. | OX-D05 simulado; comparação de execução em OX-017. |
| Feature com dependências e revisão/correção | Intervenções para coordenar etapas, trabalho bloqueado, contexto reenviado, checks ligados ao resultado e aprovação do artefato correto. | OX-017; paralelismo depois em M3. |
| Check falho e alteração depois da revisão | Clareza da ação seguinte e detecção de evidência/aprovação desatualizada. | OX-D05 para entendimento; OX-017 para execução. |
| Conta limitada ou capacidade desconhecida | Identidade da conta, comportamento da fila/fallback, preservação de progresso e necessidade de intervenção. | M3, com contas autorizadas e limites reportados; fixture identificada quando não houver condição real. |
| Reinício durante trabalho | Reconciliação, continuidade e ausência de execução duplicada; capacidade de entender o estado recuperado. | OX-017 após a implementação de recuperação. |

Usar o mesmo repositório/base, objetivo, critérios e checks; registrar versões, plataforma, contas, modelos/configurações e condições de execução. Respeitar as capacidades disponíveis em cada produto e anotar combinações incompatíveis. Separar aprendizagem, setup e execução; registrar solicitado versus observado e desconhecidos. Se comparar tempos ou consumo, repetir condições e apresentar limites da amostra. Consumo reportado e custo efetivamente cobrado são medidas diferentes; ausência de dados não vira economia.

**Situação ao final desta atualização:** pesquisa complementar documentada; dois concorrentes acrescentados e evidência Superset corrigida. Nenhum piloto, benchmark prático, adapter ou marco de produção foi concluído por este relatório. As hipóteses de diferenciação precisam de resultado mensurável antes de orientar alegações de superioridade.
