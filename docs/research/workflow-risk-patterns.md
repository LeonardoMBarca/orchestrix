# D0 — controle do trabalho, contexto e revisão

Rodada de **8 de outubro de 2026**. Complementa as [jornadas comparativas](solution-journeys.md) e o [benchmark de experiência](experience-decisions.md) com documentação de Vibe Kanban/GitHub Desktop e três capturas oficiais adicionais, efetivamente examinadas. Não houve instalação, login ou operação desses produtos. A referência Vibe Kanban é de um projeto comunitário cuja documentação ainda contém material de fases anteriores; versão exata das capturas não foi confirmada.

## Evidência visual adicional

Os PNGs canônicos publicados na documentação foram baixados para cache temporário e abertos como imagens. URLs com parâmetros de transformação falharam no leitor web; os arquivos públicos sem esses parâmetros estavam acessíveis. Não foram copiados para o repositório. Imagens estáticas comprovam apenas os elementos visíveis.

| Captura oficial | Observado | O que permanece desconhecido |
| --- | --- | --- |
| [Conflito](https://mintcdn.com/vibekanban/QA35mU65cg2kMRzj/images/workspaces-conflict-dialog.png), vinculada a [Git Operations](https://www.vibekanban.com/docs/workspaces/git-operations) | Modal sobre diff; arquivo afetado; ação de resolução pelo agente; opção de nova sessão; ação de cancelar. | Se cancelar fecha o modal ou aborta uma operação Git; resultado da resolução; versão a que o formulário pertence. |
| [Variantes](https://mintcdn.com/vibekanban/QA35mU65cg2kMRzj/images/workspaces-agent-selection.png), vinculada a [Chat Interface](https://www.vibekanban.com/docs/workspaces/chat-interface) | Menu com Default, Approvals, Opus, Plan e entrada de customização. | Como cada variante resolve modelo, permissões, thinking e contexto na execução concreta. |
| [Changes/Git](https://mintcdn.com/vibekanban/QA35mU65cg2kMRzj/images/workspaces-changes-panel.png), vinculada a [Changes](https://www.vibekanban.com/docs/workspaces/changes) | Diff lado a lado; árvore com contagem por arquivo; área Git com branch e ação de PR; terminal/notas recolhíveis. | Qual versão do artefato está congelada para revisão; resultado de uma operação final de aplicação. |

Identificação dos arquivos examinados, para repetição futura:

| Arquivo | Dimensões originais | Bytes | SHA-256 |
| --- | --- | --- | --- |
| workspaces-conflict-dialog.png | 1804 × 826 | 171363 | `720fecd312cb73564cc9748bd14e1f9c3dad2e84d0a706dda98491dbc55a89b0` |
| workspaces-agent-selection.png | 1740 × 1246 | 155397 | `555aebbdef9f627713d5c6b2e2e23537f93377aabd64f4b8d3093288ec6b073f` |
| workspaces-changes-panel.png | 2269 × 1618 | 454448 | `6a5f45c1bcf649cd29671d34ae6e154f4074e6c28ca2ef78ec8b90d96538edf2` |

## Achados documentais e suas implicações

**Sessões e contexto.** O Vibe Kanban descreve sessões com conversações próprias e arquivos/Git compartilhados. Trocar a sessão visível mantém os processos em execução; uma sessão nova precisa receber contexto do trabalho anterior. [Sessions](https://www.vibekanban.com/docs/workspaces/sessions)

**Inferência para Orchestrix:** navegação não altera identidade de execução. O revisor deve receber o artefato e as evidências escolhidos, mesmo tendo outra conversação. A interface deve identificar quem escreve e em qual ambiente; uma revisão aberta precisa continuar vinculada à sua versão quando surgir outra alteração.

**Orientar, aguardar e interromper.** A documentação diferencia enviar mensagem, enfileirar acompanhamento e parar a execução. Também descreve aprovação de plano e feedback para revisão. Isso é um percurso documental, sem transições verificadas nesta rodada. [Chat Interface](https://www.vibekanban.com/docs/workspaces/chat-interface)

**Inferência para Orchestrix:** o composer deve informar o efeito do envio durante uma execução. Um pedido de correção pode ser preparado sem fingir que interrompeu o escritor. A fila de atenção precisa distinguir pergunta, permissão, falha, espera por capacidade e revisão pronta; cada item mostra motivo, trabalho afetado e ação possível.

**Feedback no diff.** O guia descreve comentários acumulados e enviados juntos pelo chat. Depois do envio, entram no histórico, e outra rodada requer comentários novos. [Reviewing Code](https://www.vibekanban.com/docs/reviewing-code)

**Inferência para Orchestrix:** manter rascunhos ao trocar de arquivo e mostrar quantos comentários acompanham a correção. O envio deve registrar tarefa, tentativa, versão e trecho; a rodada seguinte preserva findings anteriores e informa o que foi atendido. Ainda é necessário validar essa linguagem em D1.

**Direção de integração.** A ação Merge documentada traz a target branch para a working branch; a integração de volta ocorre normalmente por PR. A captura de conflito apresenta resolução por agente, enquanto o passo a passo também descreve resolução em editor e continuação/abort. Essas evidências não estabelecem uma única transição atual. [Git Operations](https://www.vibekanban.com/docs/workspaces/git-operations)

**Inferência para Orchestrix:** cada ação deve explicitar origem, destino e efeito: atualizar base de trabalho, preparar candidato e aplicar no destino. Uma confirmação precisa citar o resultado em revisão. Fechar um diálogo e abortar uma operação devem ter rótulos próprios quando ambos existirem. Não extrapolar uma imagem antiga para o funcionamento atual do produto.

**Histórico e reversão.** GitHub Desktop documenta seleção de commit/arquivo para inspecionar diff. Sua reversão cria outro commit e preserva o original no histórico. [Histórico](https://docs.github.com/en/desktop/making-changes-in-a-branch/viewing-the-branch-history-in-github-desktop), [reverter commit](https://docs.github.com/en/desktop/managing-commits/reverting-a-commit-in-github-desktop)

**Inferência para Orchestrix:** tornar proveniência acessível pelo resultado. A UI deve preservar tentativas/revisões anteriores e explicar qual artefato foi aplicado. Reversão é uma operação posterior a projetar, com efeito explícito; não prometer desfazer instantâneo para qualquer integração.

**Adaptação do workspace.** O guia do Vibe Kanban descreve painéis recolhíveis/redimensionáveis, vistas de contexto e preferências de layout persistidas. Isso demonstra uma intenção de design documentada; não mede conforto ou adaptação real em diferentes telas. [Interface Guide](https://www.vibekanban.com/docs/workspaces/interface)

**Inferência para Orchestrix:** a área central acompanha o trabalho selecionado. Texto do objetivo/composer prevalece na formulação; diff/evidências ganham área na revisão; detalhes de decisão ficam acessíveis no inspector. Persistir o layout escolhido e preservar foco/seleção durante eventos. Mudanças de tema mantêm essa estrutura.

## Adotar, adaptar e deixar para depois

| Padrão | Proposta para Orchestrix | Motivo e verificação em D1 |
| --- | --- | --- |
| Árvore de alterações ligada ao diff | Adotar | Localizar arquivo e versão sem abrir transcript. Verificar com arquivos novos, excluídos e nomes longos. |
| Comentários enviados em conjunto | Adaptar | Vincular o envio ao artefato e preservar rascunhos; verificar troca de arquivo e nova rodada. |
| Presets que combinam várias escolhas | Adaptar | Mostrar resumo concreto de modelo/thinking/revisão/permissões, com detalhes por dimensão. Menu de preset não deve ocultar o efeito de uma escolha. |
| Várias conversações no mesmo checkout | Adaptar ao contrato de escrita do Orchestrix | Conversação independente não implica escrita isolada. Mostrar writers ativos e impedir revisão/aplicação de versão incompatível. |
| Painéis ajustáveis e preferência persistida | Adotar | Manter área de leitura e ações em janela compacta; testar foco e texto ampliado. |
| Terminal/browser sempre visíveis | Fora da composição padrão inicial | O fluxo principal é objetivo → trabalho → resultado; detalhes complementares aparecem conforme a necessidade. |
| Um botão genérico de merge | Especificar por operação | O usuário precisa explicar qual branch recebe quais alterações antes de confirmar. |
| Desfazer universal | Adiar | Primeiro preservar histórico e explicar o efeito; operações recuperáveis precisam de contrato demonstrado. |

## Requisitos de experiência derivados

Propostas de D0; não são resultados de avaliação nem novos recursos já implementados:

1. **Atenção tem motivo:** texto legível, trabalho/versão afetados, última evidência e próxima ação; cor funciona como apoio.
2. **Ação tem efeito:** enviar, enfileirar, interromper, fechar, abortar e aplicar descrevem operações distintas.
3. **Revisão tem versão:** diff, comentários, findings e checks apontam para o mesmo artefato; mudança posterior invalida a confirmação aplicável.
4. **Preferência tem escopo:** resumo mostra as escolhas do preset e quando uma mudança passa a valer; detalhes ficam acessíveis.
5. **Contexto tem continuidade:** outra sessão recebe material selecionado e pode explicar sua origem; histórico do provedor permanece responsabilidade do runtime.
6. **Adaptação preserva trabalho:** redimensionar, trocar tema ou receber evento mantém texto em edição, arquivo, comentário e foco.
7. **Resultado tem proveniência:** aplicação confirmada aponta destino e versão; tentativas anteriores continuam consultáveis.

Os sete temas existentes são opções visuais. O valor de uma interface adaptativa dependerá desses comportamentos e do piloto, além da paleta. A rodada acrescenta evidência estática/documental; não completa as cinco jornadas dinâmicas em três produtos exigidas por OX-D01.
