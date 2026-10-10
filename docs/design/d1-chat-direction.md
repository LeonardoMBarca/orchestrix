# D1 — projetos e conversas de desenvolvimento

Registro em **9 de outubro de 2026**. **Direção comunicada pelo responsável; incremento implementado e passagem técnica aprovada. Avaliação de uso pendente.** Este documento registra o ajuste do protótipo D1, sem substituir ADRs ou declarar recursos de produção.

## Origem e efeito da decisão

O responsável gostou da interface, mas a relatou pouco intuitiva e sentiu falta de chat. Quer usar o Orchestrix como agente de código e pediu preservar a interface existente, sem mudar tudo. A primeira interpretação de uma conversa central única por projeto foi refinada pelo responsável ainda em 09/10: **preservar o início por projetos e o fluxo guiado; permitir várias conversas em um projeto e conversas avulsas como ponto de partida**. Projeto e conversa têm identidades diferentes, com vínculo opcional. Studio, tarefas, diffs e inspectors permanecem partes do mesmo fluxo.

Antes desse feedback, o responsável pediu instruções concretas para o piloto e recebeu um roteiro de oito passos. Foi ajuda direta de facilitação, sem execução ou resultado confirmado. O [registro do piloto](d1-pilot.md) separa essa preparação do feedback de direção. Não há evidência para atribuir a dificuldade com as instruções a um botão específico ou para afirmar que o chat já resolveu a experiência.

## Contrato de experiência do estudo

| Área | Comportamento disponível para avaliar |
| --- | --- |
| Projeto e entrada | **Abrir projeto** preserva preparação e templates guiados. **Projeto ou espaço** seleciona o contexto; **Conversa** seleciona o chat. **Nova conversa** cria outro chat no projeto sem apagar os anteriores nem seu trabalho. |
| Conversa avulsa | **Conversa avulsa** abre um espaço independente sem repositório para formular/planejar e percorrer o ciclo simulado. Aplicação é bloqueada por falta de destino. Associação posterior da avulsa a um projeto **não integra este incremento**; será trabalho de M1/M2 conforme o plano. |
| Conversa e execução | Composer e **Enviar** pertencem à conversa selecionada, vinculada a projeto ou avulsa. O primeiro pedido prepara tarefa/Run demonstrativos sem iniciar execução. Mensagens posteriores registram orientação ligada ao trabalho; outro trabalho exige ação explícita. Envio, preparação e execução não devem parecer o mesmo evento. |
| Trabalho em contexto | Cartões apresentam **Ver tarefa**, **Iniciar demonstração** e **Revisar resultado/Pedir correção**, conforme estado; **Novo trabalho nesta conversa** abre outro pedido explícito. Tarefas/dependências e detalhes continuam acessíveis; fila e Studio são preservados. Projeto/espaço, conversa selecionada e trabalho referenciado precisam ser identificáveis. |
| Correção | **Pedir correção** abre revisão com comentário preparado; o envio explícito cria a tentativa. Follow-up indica a tarefa e versão do artefato. Se o destino não estiver claro, torná-lo explícito antes do envio; não depender apenas da tarefa selecionada silenciosamente. |
| Revisão e entrega | Abrir alterações permite inspecionar diff, checks, contexto e conta da tentativa produtora. Aceitar uma tarefa no Run continua distinto de aplicar o candidato completo no destino. Espaço avulso não habilita aplicação sem repositório/destino. |
| Preferências | Conexão, modelo, raciocínio e contexto permanecem inspecionáveis. Mudanças afetam decisões futuras e preservam snapshots de tentativas anteriores. |
| Atenção e controle | Pergunta, permissão, falha, revisão, limite e sinal desconhecido conservam motivo e ação vinculados ao trabalho. Uma mensagem não confirma cancelamento nem autoriza duplicar execução incerta. |
| Adaptação | Composer, mensagens e ações ficam disponíveis em janela compacta, texto 200% e teclado. Eventos novos preservam rascunho, foco, seleção e posição de leitura. |

**Project != Conversation.** Um projeto pode reunir várias conversas; uma conversa pode começar sem projeto. Neste documento, “sessão de chat” significa essa conversa do aplicativo, não uma RuntimeSession do provider. Mensagens ligam Run, tarefa e artefato quando aplicável, sem fundir Project, Run, TaskAttempt e RuntimeSession. Cada tentativa mantém identidade, política e contexto; revisão em sessão separada continua conforme a política. A conversa pode reunir referências a essas etapas sem transferir conversa privada ou raciocínio interno entre providers.

O incremento usa mensagens e trabalho **simulados**, com esse limite visível. Não implementa inferência, acesso ao repositório, planner, scheduler, autenticação ou Git reais. Controles e verificações constam da [entrega](d1-delivery.md#passagem-técnica-do-incremento-por-conversa--0910); este documento não define um contrato público de chat ou transporte. Composer usa **Mensagem para o projeto** no projeto e **Mensagem nesta conversa** na avulsa.

**Histórico anterior à revisão de sessões/painéis:** o estado então implementado preservava mensagens/rascunhos e referências próprias da conversa, além de Task/Run/artefato/base/configuração ao alternar projetos/espaços na mesma execução do protótipo. Criar outro projeto ou outra conversa gera identidade própria, mesmo com nome igual; rótulo não é identidade. Projetos/espaços anteriores podem ser selecionados novamente **em memória**, preservando seus snapshots e trabalho. Temas são preferências globais, e trocar espaço não restaura autorizações antigas das conexões. A inicialização específica da primeira fixture guiada está descrita no [README do estudo](../../prototypes/desktop/README.md#dados-e-limites-do-cenário). Não há persistência durável, recuperação após reload/restart ou histórico nativo importado. Associação avulsa→projeto permanece posterior.

## Casos para a passagem técnica e o piloto

Estes casos reaplicam requisitos D1 existentes à entrada por conversa, sem criar gates novos. A [matriz D1](d1-scenario-matrix.md) e o [piloto](d1-pilot.md) continuam as referências do aceite.

| Caso | O que conferir | Critérios existentes relacionados |
| --- | --- | --- |
| CHAT-01 — começar por projeto ou chat | Preservar entrada de projeto/fluxo guiado; abrir duas conversas no mesmo projeto e uma avulsa. A pessoa localiza o composer, formula resultado/critério e entende Run/tarefa e próxima ação com uma conexão. | POS-01/02; P0-01/02. |
| CHAT-02 — acompanhar e inspecionar | Progresso e atenção apontam para o trabalho certo; a pessoa abre conta/modelo/contexto e distingue solicitado de informado pelo runtime. | POS-03/04/07/08; P0-03/04/05. |
| CHAT-03 — pedir uma correção do resultado | Mensagem identifica tarefa/artefato, nova tentativa conserva proveniência e resultado anterior permanece revisável. | POS-07/08/09; P0-04/06. |
| CHAT-04 — aceitar e depois aplicar | Aceite de tarefa não aparece como entrega no destino; Run incompleto e aprovação desatualizada continuam bloqueados. Na avulsa, aplicação fica bloqueada por ausência de destino, sem migração automática para projeto. | POS-09/10/11; P0-07. |
| CHAT-05 — preservar controle e cobrança | Conta/limite/sinal desconhecido não autorizam fallback pago ou duplicação; confirmações conservam escopo. | POS-06/12/13/14; ACC-01 a ACC-07; P0-01/03/08. |
| CHAT-06 — continuar com conforto | Trocas entre duas conversas de um projeto, avulsa e projetos com mesmo nome/IDs diferentes preservam mensagens, rascunho e snapshots de Task/Run/artefato/base/configuração, sem misturar trabalho ou revisão. Teclado, reflow, texto ampliado e eventos preservam foco; Studio e acesso à revisão continuam reconhecíveis. | POS-08/15/16; P0-09. |

Na avaliação, registrar se a pessoa encontrou a entrada sem ajuda, a ajuda realmente fornecida, o caminho usado e o que entendeu sobre projeto, conversa, mensagem, tarefa e aplicação. Na avulsa, conferir compreensão do destino ausente; associação posterior não é uma ação disponível para concluir o caso e não deve ser simulada verbalmente. Feedback de preferência e testes técnicos são evidências distintas da execução humana. Os casos acima ainda não têm resultados de uso humano.

## Estado e próximo registro

- Feedback e refinamento de direção de produto registrados em 09/10: entrada por projetos/fluxo guiado, várias conversas e entrada avulsa.
- Implementação entregue; **36/36 testes da suíte completa passaram**, incluindo [12 casos de conversa](../../prototypes/desktop/tests/project-chat.spec.mjs). Capturas desktop/compactas renderizadas e examinadas na [entrega](d1-delivery.md). Esse resultado não certifica entendimento humano.
- Piloto individual guiado com projeto e conversa: proposto; execução e observações pendentes.
- D1/OX-D05 continuam abertos; avaliação externa posterior e a ordem D1 → retomada de M0 permanecem conforme o backlog.

Versão, labels e resultado técnico estão registrados no [piloto](d1-pilot.md) e na [entrega D1](d1-delivery.md). Os 24 casos anteriores foram retestados junto aos 12 novos; resultados reais de uso ainda precisam ser registrados.


## Revisão atual de sessões — 10/10

A [nova estrutura](d1-shell-revision.md) substitui os seletores por árvore de sessões/projetos. Começar avulso ou dentro de um projeto usa o mesmo chat; vincular a um projeto existente ou criar um em torno da sessão conserva sua identidade, histórico, rascunho e snapshots antigos. Projetos compartilham contexto somente para novas tentativas. Seleção/rascunhos persistem no armazenamento do navegador com limites de tamanho e quantidades; isso não demonstra a persistência/recuperação dos runtimes ou a base de produção de M1. A abertura normal começa vazia. Fixtures de execução e seus históricos ficam restritos à flag interna de teste. O responsável adiou o piloto até aprovar a revisão da interface.
