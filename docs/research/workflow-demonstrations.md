# D0 — demonstrações adicionais e cobertura das jornadas

Pesquisa realizada em **8 de outubro de 2026**, para aprofundar criação de trabalho, atenção, recuperação e revisão/aplicação. Foram localizadas mídias primárias novas por inspeção do HTML das páginas oficiais e da árvore pública de arquivos do Superset. Dez MP4 foram baixados para cache temporário, totalizando **77.905.770 bytes**. Não houve instalação de produtos, login, acesso a credenciais, inferência, operação de agentes ou alteração de serviços externos.

Este registro acrescenta observações; **não declara cumpridos os cinco percursos em três referências** do aceite original de OX-D01. As mídias anteriores permanecem em [solution-journeys.md](solution-journeys.md) e [attention-and-review-benchmark.md](attention-and-review-benchmark.md). O eventual encerramento de D0 precisa explicitar o critério aplicado e preservar esta cobertura real.

## Método e limites

- **Documentado:** interação descrita em documentação, código ou relato do mantenedor.
- **Observado em amostras de demo:** frames de um MP4 publicado oficialmente foram extraídos por Windows Media/WinRT, em tempos identificados, e examinados. Estados sucessivos permitem observar a transição descrita; não comprovam todas as ações entre eles, áudio, backend, latência ou enforcement.
- **Demonstração ilustrativa:** animação ou saída predefinida identificada no código que produz a mídia. Pode informar linguagem e organização, mas não comprova execução funcional.
- **Não demonstrado:** a consequência necessária não aparece nas cenas examinadas. Um controle visível não comprova seu acionamento nem resultado.

Somente os frames citados abaixo foram examinados; não se declara reprodução contínua. Nenhuma gravação tem data de captura ou versão do aplicativo confirmada nesta rodada. Consulta atual e publicação em site oficial não tornam a interface gravada necessariamente atual. Arquivos e frames ficaram fora do repositório.

## Cline Kanban: criação, dependências e feedback

Os três clips estão incorporados na [página oficial do Cline CLI](https://cline.bot/cli). São superfícies Kanban, não uma avaliação de toda a família Cline.

| Mídia | Metadados originais | Tempos examinados | Observado e limite |
| --- | --- | --- | --- |
| [Section-01.mp4](https://cline.bot/assets/images/kanban/Section-01.mp4) | 8,576 s; 1648 × 1080; 1.377.303 bytes | 0, 2, 4, 6, 8 s | Projeto `stormwatch` selecionado; pedido para criar quatro tarefas, ligá-las e iniciar uma. Depois aparecem quatro cartões e relações; a primeira entra em execução. A abertura inicial do projeto e sua autenticação não aparecem. |
| [Section-02.mp4](https://cline.bot/assets/images/kanban/Section-02.mp4) | 6,315 s; 1076 × 720; 1.342.310 bytes | 0, 2, 4, 6 s | Diff e resumo existentes; texto digitado em comentário de linha. Em 4 s, o trecho/comentário aparece como mensagem ao agente, com `Thinking`; em 6 s, retorna a cena inicial. Entrega de feedback é observável; correção nova, checks e aplicação não aparecem antes do loop. |
| [Section-03.mp4](https://cline.bot/assets/images/kanban/Section-03.mp4) | 11,926 s; 1116 × 720; 1.613.614 bytes | 0, 1, 2, 3, 4, 6, 8, 10 s | Tarefa em Review e cartões dependentes; vínculo adicional desenhado. Em 3 s, a tarefa revisada desaparece após cursor junto ao ícone de lixeira; depois dependentes entram em In Progress, exibem ferramentas/alterações e chegam a Review. **Não é demonstração de commit:** a base não foi confirmada alterada, e botões Commit/Open PR não comprovam integração. |

**Conclusão observacional:** nova evidência de criação → relações → execução e diff → comentário → mensagem ao agente, além de progressão das tarefas. Espera por dependência não equivale a pergunta de permissão, falha ou recuperação de processo. A documentação descreve envio de commit/PR pelo agente, mas não transforma a remoção do cartão observada em integração confirmada. [Core Workflow](https://docs.cline.bot/kanban/core-workflow)

**Propostas para D1:** mostrar o efeito de remover/arquivar uma tarefa sobre dependências; distinguir feedback preparado, enviado e atendido; preservar comentários e versão do artefato durante a correção. As cenas não estabelecem que a alternativa do Orchestrix será mais eficaz.

### Cline Workflows: pergunta respondida e continuidade observadas

O [artigo oficial de 21/05/2025 sobre v3.16](https://cline.bot/blog/cline-v3-16-one-shot-automation-with-workflows-plus-ui-stability-gains) incorpora [workflows-splash.mp4](https://storage.ghost.io/c/d6/fe/d6feb101-a8e6-444b-bae8-3ca714794abb/content/media/2025/05/workflows-splash.mp4): **44,651 s, 3840 × 2160, 69.143.634 bytes**. Foram examinados frames em 0, 12, 16, 20, 24, 28, 32, 36, 40 e 43 s. É referência histórica da extensão no VS Code, não prova do Kanban atual.

| Tempos | Observado | Limite |
| --- | --- | --- |
| 0–16 s | Repositório e workflow abertos; comando `/pr-review.md #3627` preparado. | Login e abertura inicial não aparecem. |
| 20–28 s | Tarefa iniciada; operações sobre informações da PR, arquivos e diff; saída e leitura de código. | Autoaprovação está habilitada; não comprova sua segurança. |
| 32–36 s | Pergunta com alternativas para aprovar, discutir ou revisar manualmente primeiro. | Recomendação do agente não comprova correção do código. |
| 40–43 s | Alternativa de revisão manual selecionada; agente reconhece a escolha e continua com orientações. | **Pergunta → resposta → continuidade observadas.** Não é recuperação de crash; aprovação/merge final não ocorre na cena. |

**Proposta para D1:** separar recomendação do agente, decisão humana e efeito confirmado. Adiar aprovação deve conservar o trabalho e indicar a próxima ação, sem anunciar aplicação.

## Superset: workspace, terminais e revisão

Os arquivos novos foram localizados na [árvore oficial de mídia](https://github.com/superset-sh/superset/tree/main/apps/marketing/public/hero) e em `apps/marketing/public/videos/blog/phone-diff-review/`. Os URLs públicos responderam HTTP 200. A branch `main` é mutável; os hashes ao final identificam o conteúdo consultado.

| Mídia | Metadados originais | Tempos examinados | Observado e limite |
| --- | --- | --- | --- |
| [worktrees.mp4](https://superset.sh/hero/worktrees.mp4) | 5,334 s; 1728 × 1080; 668.149 bytes | 0, 2, 4 s | Formulário de workspace com nome/branch/base; depois nova aba, aviso de workspace criada e saída de setup. A criação da workspace é observável; não demonstra conexão nova ou tarefa de código concluída. |
| [agents.mp4](https://superset.sh/hero/agents.mp4) | 3,134 s; 1728 × 1080; 385.197 bytes | 0, 1, 2, 3 s | Três panes de terminal, interface OpenCode, Claude com conversa existente e animação em outro terminal. O pedido seguinte permanece no composer do Claude. Não se observou envio desse pedido, resposta a bloqueio ou recuperação. |
| [changes.mp4](https://superset.sh/hero/changes.mp4) | 2,6 s; 1728 × 1080; 388.092 bytes | 0, 2 s | Diff de arquivo, grupos de alterações/commits e controles de staging/commit. Não houve confirmação de staging, commit ou merge nas amostras. |
| [phone diff review](https://superset.sh/videos/blog/phone-diff-review/demo.mp4) | 5,5 s; 600 × 1304; 115.025 bytes | 0, 2, 4 s | Viewer móvel de arquivo alterado e deslocamento horizontal do diff. O conteúdo apresenta a mesma alteração. Não mostra comentário, execução nova, aprovação ou aplicação. |

**Propostas para D1:** criação deve revelar base e ambiente; acompanhar várias sessões precisa conservar sua identidade; um diff legível deve oferecer navegação apropriada ao espaço disponível. Controles de aplicação precisam mostrar resultado confirmado, não apenas sua presença.

### Demo CLI: saída predefinida, excluída da comprovação funcional

Foi examinada a [demo CLI oficial](https://raw.githubusercontent.com/superset-sh/superset/main/packages/cli/demo/superset-cli.mp4): 73,52 s, 1980 × 900, 993.907 bytes; frames em 0, 2, 4, 6, 8, 10, 12, 16, 20, 24, 30, 45 e 60 s. Mostra comandos, criação de ticket e mensagens de sucesso.

A auditoria do [arquivo que gera a gravação](https://github.com/superset-sh/superset/blob/main/packages/cli/demo/superset-cli.tape) revelou que o script redefine `superset()` como função de shell, com `echo`/`printf` e respostas fixas para os comandos apresentados. Portanto, **essa gravação é uma ilustração roteirizada**. Não comprova sincronização, processo iniciado, workspace funcional, encerramento ou aplicação. Não executamos o script. Sua exclusão evita usar uma animação de sucesso como evidência de comportamento real.

### Recuperação: buscas adicionais não encontraram gravação pública

A [PR #6572](https://github.com/superset-sh/superset/pull/6572) relata QA de retomada e menciona gravação na conversa. Foram lidos body, comentários e reviews pela API pública, sem autenticação; não surgiu URL de mídia correspondente. A [PR #7493](https://github.com/superset-sh/superset/pull/7493) expõe captura de notificação e relata QA, sem sequência dinâmica nova. Esses relatos continuam **documentados**, conforme o benchmark anterior, e não passam a demos observadas.

## Alternativas examinadas e limites externos

- **Vibe Kanban:** HTML público de Git Operations e homepage não expôs nova mídia dessa transição; README atual foi acessível, sem vídeo pertinente. A consulta adicional à árvore do repositório pela API GitHub recebeu HTTP 403 com limite público esgotado. Nenhuma autenticação foi tentada para contornar esse limite. As demos e capturas anteriores continuam úteis, sem virarem integração confirmada.
- **Cline checkpoints:** o README da [tag v3.16.0](https://github.com/cline/cline/blob/v3.16.0/README.md) identifica uma captura de compare/restore; o anexo redirecionou para PNG e retornou HTTP 403. Não era nova demo dinâmica utilizável. A alternativa de Workflows foi efetivamente baixada e examinada, conforme a seção anterior; volume maior não foi tratado como impedimento.
- **Antigravity:** o HTML da [homepage oficial](https://antigravity.google/) revelou novos players e [an-ai-ide-core.mp4](https://antigravity.google/assets/video/landing/an-ai-ide-core.mp4). Este clip foi examinado em 0, 2, 4, 6, 8, 10 e 12 s: 12,054 s, 1350 × 1350, 1.878.539 bytes. Mostra animação de edição/completion de `LoginButton.tsx`, sem projeto aberto, coordenação, resposta a atenção ou aplicação final. Foi excluído da cobertura funcional buscada; aparência de editor não equivale à demonstração do ciclo de um orquestrador.

## Cobertura adicional dos cinco percursos

Esta tabela descreve somente o que a rodada acrescenta. A comparação documental já existente e as novas observações Conductor, registradas separadamente, devem acompanhar qualquer balanço final.

| Percurso | Cline: Kanban e Workflows históricos distinguidos | Superset | Vibe Kanban / Antigravity |
| --- | --- | --- | --- |
| Abrir projeto/conectar | Projeto existente visível; abertura/conexão não mostrada. | Criação de workspace a partir de projeto existente observada; conexão não mostrada. | Sem nova transição pertinente. |
| Iniciar trabalho | Kanban: pedido/cartões/vínculos e início. Workflows: tarefa de PR iniciada. | Setup observado; pedido novo ao agente não enviado nas amostras. CLI ilustrativa excluída. | Sem nova transição pertinente. |
| Acompanhar execução | Kanban: ferramentas/status e dependentes em execução/Review. Workflows: operações de leitura/análise da PR. | Panes e saída existentes visíveis; não comprova ciclo de novo trabalho. | Sem nova transição pertinente. |
| Resolver atenção/bloqueio | Workflows: pergunta, resposta de revisão manual e continuidade observadas. Kanban: dependências e feedback. Falha/crash recuperado não demonstrado. | Recuperação descrita em PR, sem mídia acessível correspondente. | Sem nova transição pertinente. |
| Revisar/aplicar | Kanban: diff/comentário enviado. Workflows: recomendação, confirmação solicitada e aprovação adiada. Patch corrigido/aplicação final não demonstrados. | Diff desktop/móvel observado; aplicação final não demonstrada. | Sem nova transição pertinente. |

## Auditoria do aceite e passagem para D1

O [aceite de OX-D01](../DEVELOPMENT_BACKLOG.md#ox-d01-pesquisa-comparativa-de-soluções) original pede percursos de abrir projeto, iniciar trabalho, acompanhar execução, resolver atenção e revisar/aplicar em pelo menos três referências por teste ou demo, com limites registrados. Sua leitura natural exige examinar os cinco tipos em cada uma das três referências. Não exige uma mídia única, reprodução integral nem certificação técnica do concorrente. Várias demos podem compor a evidência; uma sequência observada de estados pode sustentar uma transição específica.

**Resultado perante esse critério:** não é possível marcar quinze percursos completos como cumpridos com estas fontes. A resposta a pergunta agora possui sequência observada na extensão Cline histórica; aplicação final e recuperação de processo continuam com cobertura documental/parcial. Também não seria correto reclassificar botões, previews, saída predefinida ou intenção de marketing como transições funcionais concluídas.

**Recomendação metodológica explícita:** o objetivo do responsável é pesquisa aprofundada suficiente para fundamentar o aplicativo, e não certificar quinze jornadas de concorrentes. É justificável encerrar D0 por decisão de suficiência de pesquisa, desde que o registro diga que o detalhamento anterior foi substituído, apresente esta cobertura e transfira cada desconhecido a um experimento nomeado. Isso seria mudança transparente do gate, não cumprimento do critério antigo. Este documento sozinho não altera o backlog nem declara D0 encerrado.

| Desconhecido remanescente | Próxima validação proposta | Saída exigida |
| --- | --- | --- |
| Usuário entende atenção, feedback enviado e resultado ainda não aplicado? | Piloto D1, POS-08 a POS-12 e requisitos de controle/revisão. | Interpretações, ajuda e erros registrados; problemas críticos corrigidos. |
| Remover cartão, aceitar tarefa e integrar Run têm efeitos compreensíveis? | D1, POS-10/11, com dependências e mudança da base simuladas. | Usuário identifica operação, origem/destino e versão afetada. |
| Runtime aceita recusa, cancela e retoma com identidade coerente? | Retomada de M0, OX-001/002. | Eventos e término/retomada observados; capabilities/limites registrados. |
| Aplicação final e recuperação Git preservam a versão aprovada? | M1/M2, OX-006/011/014. | Checks e aprovação vinculados ao artefato; reconciliação demonstrada. |
| O ciclo tem valor com uma única assinatura? | D1, POS-01/02/14, e demonstração alpha OX-017. | Comparação com a rotina do participante, sem economia ou superioridade presumidas. |

A sequência principal permanece **D0 → D1 com piloto → retomada de M0**. Experimentos antecipados não substituem o piloto nem certificam as lacunas acima.

## Identificação das mídias consultadas

Hashes SHA-256 dos bytes públicos baixados; não foram persistidos vídeos ou frames no repositório.

| Arquivo | SHA-256 |
| --- | --- |
| Section-01.mp4 | `8e20c7ee9be266f1ce3bf7f53a366579704b335fc20faa1e7d8f8aed8f6bc7d9` |
| Section-02.mp4 | `6b1b924d1251460fdd74b8e2090f554079099f05e633d41ace5f85f17b732f3c` |
| Section-03.mp4 | `ecc761cb5b7e066872e26e671a4a800f891d06032f1a660eff5480501dbe281e` |
| workflows-splash.mp4 | `2648dcdcc648722939d3477ee72d9679aba7b678d7f0b8e31a50852f5726af5b` |
| worktrees.mp4 | `77a6f5e06d4648be62a34e321916e8db6375665684499c8c97b96ddff805d193` |
| agents.mp4 | `a6f6744eb989afc260c776dba9d3c61dc981f0b440d9d3b2c638d2f8bf0b4f0b` |
| changes.mp4 | `31fad9f1caf3b278473675d50d6115c6dd7f31b51dceba5b8340acc83a7daaf0` |
| phone-diff-review/demo.mp4 | `d0a92446fe1587f69504266ed8701fa089880b6ce2dcfcb19efa29f4c128f1d5` |
| superset-cli.mp4 | `e910037174d81f1297aea763341d2431263c2496cfe6aa35fdacfa022584af2a` |
| an-ai-ide-core.mp4 | `71732c1611f50bbb9f857bd6f6b04110a3a5836c778fd9cc905149343e9644d1` |
