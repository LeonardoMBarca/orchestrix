# Execução do plano — pesquisa, protótipo e primeiro spike

Atualizado em **8 de outubro de 2026**. Este registro acompanha entregas e evidências do [plano](./DEVELOPMENT_PLAN.md), sem transformar pesquisa documental ou protótipos em implementação de produção.

**D0 concluído como pesquisa; D1 com protótipo e verificação técnica entregues, piloto humano pendente.** O [relatório de conclusão](./research/d0-conclusion.md) registra decisões, evidências e a revisão explícita do critério metodológico de OX-D01. O requisito anterior de quinze jornadas completas não foi demonstrado integralmente; não foi contabilizado como teste aprovado. A sequência permanece D0 → D1 com piloto inicial → retomada de M0 → M1 → M2. Protótipo/harness são antecipações aproveitáveis; não houve novas execuções live de runtime nesta rodada.

## Entregas disponíveis

| Frente | Entrega concreta | Evidência / limite |
| --- | --- | --- |
| Solução | [Levantamento](./PRODUCT_AND_UX_RESEARCH.md), [posicionamento](./research/product-positioning.md), [abrangência/proveniência](./research/landscape-and-provenance.md) e [jornadas](./research/solution-journeys.md) | Nove famílias, incluindo Antigravity; público, capacidades, contas, versões, manutenção e licença. Demos Conductor/Cline/Superset examinadas, com cobertura parcial explícita; Vibe/OpenCode complementam a observação. |
| Integração | [Decisões para os spikes](./research/integration-decisions.md), [contas/assinatura](./research/subscription-account-ux.md) e [protocolo local](./research/codex-protocol-spike.md) | Contrato mínimo e candidato OAuth próprio oficial para uso do plano ChatGPT. Registro de contas é documentado; isolamento, renovação e cotas não foram executados. Ainda não há adapter de produção. |
| M0 / Codex | [Harness executável](../tools/runtime-harness/README.md) e [resultado real](./research/runtime-harness-results.md) | Stdio via assinatura ChatGPT validado: resposta exata, interrupção terminal confirmada e processo pai encerrado. Repositório temporário inalterado; árvore/retomada/multiaccount pendentes. |
| Verificação do harness | [23 testes offline](../tools/runtime-harness/tests/contract.test.mjs) | Passaram no Node 24.19.0; 11 cenários sintéticos, framing/Unicode, limites, correlação, permissões, timeout, crash e sanitização. Não certificam sandbox ou quotas reais. |
| Experiência | [Benchmark](./research/experience-decisions.md), [atenção/revisão](./research/attention-and-review-benchmark.md), [controle/contexto](./research/workflow-risk-patterns.md) e [conclusão](./research/d0-conclusion.md) | Três referências de interação e critérios de foco, densidade, painéis, teclado e estados. POS-01 a POS-16 e ACC-01 a ACC-07 orientam D1; 14 decisões priorizadas. Sem avaliação com usuários. |
| Design | [Entrega D1](./design/d1-delivery.md), [protótipo](../prototypes/desktop/README.md) e [sistema visual](./design/d1-design-system.md) | Entrada com uma conexão, feature com quatro tarefas/cinco arquivos, contexto versionado, revisão, atenção/conexões, sete temas, painéis e teclado. Studio padrão; simulação integral. |
| Verificação do protótipo | [10 cenários D1](../prototypes/desktop/tests/d1.spec.mjs) e [nove anteriores](../prototypes/desktop/tests/workspace.spec.mjs) | 19 passaram no Chrome local. Ciclos, isolamento dos Runs, controle/proveniência, teclado/arraste/persistência e reflow. Texto 200% e seis larguras CSS; não certifica Core ou usabilidade. |

## Situação dos tickets

| Ticket | Situação | O que falta para o aceite completo |
| --- | --- | --- |
| OX-D01 | **Concluído como pesquisa pelo critério consolidado** | Limites de atenção/recuperação/aplicação dos concorrentes preservados. Não certifica contas/runtimes; testes próprios seguem M0. [Cobertura e alteração do aceite](./research/d0-conclusion.md). |
| OX-D02 | **Concluído como benchmark de pesquisa** | Validar no protótipo/piloto as decisões de experiência; não há avaliação fictícia de conforto/desempenho dos concorrentes. |
| OX-D03 | Jornadas do estudo implementadas e verificadas | Avaliar linguagem e compreensão no piloto. Entrada, contexto, decisões/permissões e limite/auth simulados; edição manual leve e incrementos avançados após avaliação. |
| OX-D04 | Studio refinado, sete temas, tokens/componentes e painéis entregues como candidato | Avaliar conforto/controle no piloto; ampliar verificação de acessibilidade, escala Windows e conteúdo extenso. |
| OX-D05 | [Roteiro e registro](./design/d1-pilot.md) preparados; piloto individual solicitado | Resposta/uso do responsável ainda não registrados. Corrigir/retestar problemas críticos; avaliação externa continua antes do alpha. |
| OX-001 | Primeiro percurso real Codex validado como antecipação; ampliação após D1 | Comparar CLI/app-server e modalidade OAuth própria; testar retomada real, código em worktree e descendentes no Windows. Registrar matriz e aprovar superfície do primeiro adapter. |
| OX-002 | Pesquisa/inventário local | Claude e Antigravity não disponíveis no PATH. Validar modalidades e contas independentes sem alterar login global; não bloquear um worker Codex por falta de outro provider. |
| OX-003 | Experimento preparatório entregue | Fake e eventos normalizados no harness; contrato final e runtime de produção aguardam gate técnico de OX-001/002. |
| OX-004 a OX-017 | Não iniciados | Contratos, execução e gates indicados no backlog. |

O encerramento de D0 vem da pesquisa consolidada, não dos testes do protótipo/harness. D1 e M0 continuam abertos; a implementação Desktop de M2 depende de seus gates. O [handoff](./research/discovery-handoff.md) organiza a próxima etapa e os limites técnicos.

## Decisões de produto para avaliar

1. **Trabalho como centro:** projeto → objetivo → Run/tarefa → tentativa → resultado. Sessões e modelos explicam o trabalho dentro de um inspector; não organizam toda a navegação.
2. **Atenção com motivo e ação:** espera, sinal desconhecido, falha, pergunta, revisão e aplicação não compartilham uma indicação genérica de atividade.
3. **Revisão dentro do app:** diff, evidências e pedido de correção preservam tarefa, tentativa e versão. Edição manual leve permanece uma decisão posterior do piloto.
4. **Aceite e aplicação separados:** validar uma tarefa na branch interna do Run precede revisar o candidato de integração no destino. O seed usa um Run de uma tarefa; a Feature guiada tem quatro tarefas no mesmo Run e bloqueia aplicação incompleta. Integração Git real continua em M1/M2.
5. **Escolhas explicáveis:** configurações solicitadas ficam em snapshots por tentativa; efetivo desconhecido continua explícito. Uma mudança de preferência só passa a novas tentativas.
6. **Múltiplas contas desde o domínio:** identificar conexão e capacidade observável. A interface não transforma configurações ou sessões distintas em uma promessa de cota independente.
7. **Desktop como workspace assistido:** formular, acompanhar, ler, corrigir e aplicar. As evidências de checks são parte do trabalho; test explorer, debugger e outras funções de IDE ficam fora do escopo inicial.

Essas propostas derivam dos registros de pesquisa acima. Ainda precisam de validação de uso e dos contratos demonstrados pelos spikes; não substituem os ADRs aceitos.

## Próxima sequência concreta

Executar e registrar o piloto inicial do [incremento D1](./design/d1-delivery.md), usando a [matriz](./design/d1-scenario-matrix.md); corrigir e retestar problemas críticos. Protótipo e verificação técnica estão disponíveis. Depois retomar M0: comparar modalidades de autenticação/CLI/app-server, alteração pequena em worktree descartável, diff/checks ligados à tentativa, retomada não ephemeral e descendentes sintéticos no Windows. O Core com persistência, política/contexto e recuperação vem depois desse gate.

**Feedback visual do responsável:** gostou das três explorações iniciais, definiu Studio como padrão e pediu temas adicionais, incluindo preto profundo e medieval, com nomes em inglês e seleção nas configurações. Foram implementados Studio, Atelier, Horizon, Deep Black, Medieval, Forest e Dawn em **Preferências → Aparência**, acessíveis pelo botão **Personalizar aparência →** na navegação, com persistência local. Esse feedback confirma a preferência visual inicial; não equivale a um piloto de conclusão das jornadas.

Não instalar Rust ou comprometer a estrutura final de crates para produzir um estudo de interface. O protótipo está isolado em `prototypes/desktop`; o Core/daemon e o Desktop Tauri continuam marcos de implementação próprios.
