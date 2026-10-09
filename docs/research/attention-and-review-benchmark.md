# Atenção, recuperação e revisão: Conductor e Superset

**Rodada anterior à consolidação:** este registro conserva os métodos e limites da análise inicial. O [workflow Conductor posterior](conductor-dynamic-evidence.md) e as [novas demos Cline/Superset](workflow-demonstrations.md) ampliam a evidência. A referência atual de aceite/metodologia é [D0 — conclusão](d0-conclusion.md); os limites abaixo não são removidos pelo encerramento da pesquisa.

Rodada D0 consultada em **8 de outubro de 2026**, centrada nas lacunas de bloqueio/recuperação e revisão/aplicação. É uma comparação documental com observação de mídia publicada, sem instalação, login, chamadas de modelo, operação dos produtos ou medidas de desempenho. Não encerra o aceite de OX-D01 nem modifica gates.

O principal avanço é uma demo acessível do Conductor que mostra abertura e mudança de apresentação do diff. A documentação atual acrescenta decisões relevantes: intenção de encerramento persistida, feedback ligado ao trecho revisado, retomada com estado explícito e integração condicionada ao código presente na workspace. As transições completas de bloqueio → resposta → retomada e revisão → correção → integração final continuam sem demonstração própria nesta rodada.

## Método e acesso

- **Observado em amostras de mídia:** frames extraídos do MP4 público por APIs de arquivo do Windows. Não significa reprodução contínua, escuta do áudio ou teste do produto.
- **Observado em captura:** elementos presentes em PNG oficial; não prova o clique nem sua consequência.
- **Documentado:** fluxo descrito em fonte primária. Relatos de QA do mantenedor pertencem a esta categoria; não foram repetidos por nós.
- **Inferência para o Orchestrix:** proposta de experiência ou contrato a validar, sem atribuir eficácia comprovada ao concorrente.

Foi baixado um único vídeo novo, abaixo de 30 MB. O host Tigris que havia falhado em TLS na rodada anterior não foi novamente acessado. A pesquisa Superset foi delimitada à documentação atual, ao artigo de feedback e a PRs oficiais de recuperação/notificações. Não foram procuradas novas mídias depois dessa rodada.

## Demo Conductor: transições de revisão realmente vistas

Fonte: [Diff Viewer oficial](https://www.conductor.build/docs/reference/diff-viewer), que incorpora [ConductorDiffExplainer.mp4](https://meltylabs.t3.storage.dev/conductor-docs/ConductorDiffExplainer.mp4). HTTP 200; **27.196.630 bytes**, duração **48,874 s**, resolução original **1336 × 1080**. Foram inspecionados frames em 0, 6, 8, 10, 12, 16, 18, 20, 22, 24, 26, 30, 32, 34, 36, 42 e 48 s. Os tempos referem-se à mídia editada, não à latência do aplicativo.

**Data e versão da gravação não confirmadas.** O menu mostra commits de agosto de 2025, o que não determina a data da gravação ou da interface. A mídia não certifica a aparência atual do produto.

| Tempos | Observado em amostras | Limite |
| --- | --- | --- |
| 0–10 s | Chat existente e lista de três arquivos alterados; cursor sobre um arquivo. | Geração das alterações ocorreu antes da mídia. |
| 12–18 s | Diff aberto; árvore de arquivos, código e indicação branch → main. Conteúdo muda durante navegação. | Não foi inspecionada a correção do código. |
| 20, 22 e 26 s | Menu indica Split; depois o código ocupa uma coluna e o menu indica Unified. | Transição de apresentação comprovada por estados sucessivos. |
| 30–32 s | Menu de alterações locais/commits; depois identificador de commit e loading do diff. | Conteúdo final filtrado não confirmado; em 34 s o viewer está fechado. |
| 34–48 s | Retorno ao chat; Create PR permanece visível. | Não houve criação confirmada de PR, checks, merge ou aplicação final. |

**Nova evidência de revisão, com cobertura parcial:** lista → diff, mudança de layout e filtro/loading. Comentário → envio → correção, resposta a bloqueio e integração final não foram observados.

O [workflow oficial](https://www.conductor.build/docs/concepts/workflow) também incorpora [workflow.mp4](https://melty.t3.storage.dev/workflow.mp4). O HEAD respondeu 200, mas informou **771.097.315 bytes**; não foi baixado por exceder o limite de mídia desta pesquisa. O resumo escrito dessa página descreve revisão, feedback, PR, checks e merge; permanece **documentado**, não observado no vídeo.

## Conductor: percurso reconstruído por documentação atual

### Bloqueio e recuperação

**Documentado:** abrir a sessão sinalizada como necessitando input e responder à questão; diante de permissão, examinar a ação e aprovar, negar ou orientar outra abordagem. Quando parece travado, verificar comandos/input pendentes, cancelar a resposta se necessário e enviar a próxima ação precisa. Confusão persistente pode exigir outro chat ou checkpoint. [Troubleshooting](https://www.conductor.build/docs/troubleshooting/issues), sem data de atualização visível, consultado em 08/10/2026.

**Documentado:** checkpoint captura alterações de código entre turnos e fica separado da história Git da branch. Restaurar pelo ícone na mensagem remove permanentemente aquela mensagem e as posteriores e reverte alterações desde seu envio, inclusive alterações manuais abrangidas. Há alerta para múltiplos chats na mesma workspace. [Checkpoints](https://www.conductor.build/docs/reference/checkpoints), consultado em 08/10/2026.

**Não verificado:** um pedido concreto respondido e seguido de retomada; abrangência de restauração em todos os harnesses; segurança da restauração com escritores concorrentes. A documentação de checkpoint descreve uma operação sobre código e conversa, não apenas reconexão de um processo.

### Revisão, correção e aplicação

**Documentado:** revisão manual no diff e ação Review para revisão por agente têm papéis separados. Comentários de linhas tornam-se anexos no composer; enviar entrega o feedback ao agente. Depois vêm nova revisão e Checks. [Workflow](https://www.conductor.build/docs/concepts/workflow)

**Documentado:** Checks agrega Git, PR, CI, deployments, comentários e todos conforme o repositório/integrações. Pode bloquear ou desencorajar merge; a fonte não promete bloqueio universal de toda condição. [Checks](https://www.conductor.build/docs/reference/checks)

**Documentado, versão 0.89.0 de 29/09/2026:** Review encaminha comentários não resolvidos ao chat; Checks permite reexecutar ou pedir correção. Merge é bloqueado se a PR remota mudou em relação ao código presente na workspace. Comentários editados externamente reaparecem como novos; checks aguardando aprovação recebem indicação distinta. [Changelog exato](https://www.conductor.build/changelog/0.89.0-sign-in-with-chatgpt)

**Não verificado:** enforcement dessa proteção na instalação concreta, checks realmente executados ou merge concluído. A ação merge publicada não equivale a aplicação local de um patch arbitrário no checkout do usuário.

## Superset: percurso reconstruído e evidência de recuperação

### Bloqueio, status e retomada

**Documentado:** hooks/wrappers alimentam status; cobertura de espera varia por agente. Um indicador que ficou running após encerramento forçado pode ser limpo com Clear Status. Perguntas, permissões e revisão de plano podem aparecer no chat, com sinal de atenção. [Status e notificações](https://docs.superset.sh/agent-status), consultado em 08/10/2026.

**Documentado:** morte inesperada do terminal pode relançar sessão por seu ID e comando de resume; mostra Resuming, ou Failed to resume com Retry/dismiss. Sessões fechadas intencionalmente e agentes encerrados normalmente permanecem encerrados. Fork copia a conversa e mantém os arquivos na mesma workspace; handoff inicia sessão nova com contexto limitado. [Sessões](https://docs.superset.sh/agent-sessions), consultado em 08/10/2026.

**Documentado em PR oficial #6572, merge em 18/08/2026:** retomada usa reivindicação durável no host e coalescência de pedidos concorrentes. A PR relata um bug de ressurreição de sessão encerrada deliberadamente e sua correção com motivo de término persistido. Relata QA positivo de retomada; o banner de falha não tinha sido exercitado ao vivo. Após queda em massa, panes elegíveis podem relançar vários agentes. [PR #6572](https://github.com/superset-sh/superset/pull/6572)

A PR menciona uma gravação na conversa. A página pública e os endpoints de comentários/reviews consultados não forneceram um endereço de mídia correspondente. **Nenhum frame dessa gravação foi observado.** Não se converte o relato de QA em demo assistida.

### Capturas adicionais examinadas

| Fonte/data | Observado em captura | Limite temporal e funcional |
| --- | --- | --- |
| [Banner publicado em 09/08/2026](https://superset.sh/changelog/2026-08-09-resume-banner.png) | Terminal interrompido; mensagem com agente, Resume e dismiss. | Captura anterior à retomada automática documentada; não comprova a UI atual nem recuperação após clique. |
| [Notificação da PR #7493](https://raw.githubusercontent.com/superset-sh/superset/56392e075fbb5d45418862b59767f33d7c6f0645/docs/screenshots/notification-content-preview.png), merge em 13/09/2026 | Título projeto/workspace, subtítulo agente/Finished e resumo da resposta. | Mostra conclusão, não pergunta de permissão nem retomada. |

**Documentado na [PR #7493](https://github.com/superset-sh/superset/pull/7493):** notificações receberam contexto, conteúdo e falhas, com fallback quando falta texto. O autor relata validação nativa em macOS; renderização Windows/Linux continuava não testada nessa PR. A captura não demonstra paridade entre sistemas operacionais.

### Revisão, feedback e integração

**Documentado, artigo de 24/09/2026:** selecionar linhas na aba Code da PR, escrever feedback e escolher agente/sessão. Sem workspace vinculada, cria-se checkout da PR. O agente recebe arquivo, intervalo e lado do diff; publicar no GitHub é opcional. Depois é preciso inspecionar patch/testes, e o usuário decide o merge. [Artigo oficial](https://superset.sh/blog/send-pr-feedback-to-your-agent)

**Documentado:** Review separa comentários abertos/resolvidos e sincroniza após mutações; Changes distingue a abertura do painel das ações commit/push/Create PR e oferece foco por arquivo/seção. [Diff Viewer](https://docs.superset.sh/diff-viewer)

**Documentado:** a tela de PR reúne checks, revisão e diff; oferece squash, merge commit e rebase. Checks cancelados contam como falha. Merge/comentário/fechamento atuam na PR real pela conexão GitHub. [Pull Requests](https://docs.superset.sh/pull-requests)

**Não verificado nesta rodada:** feedback enviado nessa tela seguido de patch novo, revisão concluída e merge confirmado. O artigo de feedback e as páginas atuais consultadas não expuseram nova mídia dinâmica desse percurso. O MP4 Design Mode observado anteriormente demonstra outro tipo de feedback; não preenche esta lacuna.

## Decisões propostas de experiência para Orchestrix

São inferências a validar no protótipo/piloto, não alterações do backlog:

| Decisão | Aplicação concreta | Por que adaptar assim |
| --- | --- | --- |
| **Adotar atenção com causa e destino** | Cada item identifica tarefa/tentativa, pergunta ou ação exata, origem e próximo passo; abrir leva ao evento correspondente. | Ícone/unread sozinho não explica o bloqueio. A fonte do sinal e sua cobertura precisam permanecer visíveis quando relevantes. |
| **Adotar resposta pendente até confirmação** | Depois de responder, mostrar envio/aguardo; rejeição, expiração ou falha preservam a pergunta e oferecem recuperação. | Interação otimista não pode afirmar aprovação aplicada ou retomada antes da confirmação do runtime. |
| **Adaptar recuperação com intenção persistida** | Distinguir reconectar visualização, retomar sessão e iniciar nova tentativa; stop explícito impede retomada automática. Retry não duplica o worker. | A PR Superset fornece um caso real documentado em que confundir crash e encerramento voluntário ressuscitou trabalho. |
| **Adotar feedback ligado à revisão** | Rascunho contém arquivo, linhas/lado, versão do artefato e destinatário; separar enviar ao agente de publicar comentário externo. | O percurso de feedback é fundamentado; a ligação à versão é exigência proposta para evitar corrigir código diferente do revisado. |
| **Adotar contexto e progresso de revisão** | Mostrar base/versão revisadas, loading ou falha de atualização, arquivos examinados e checks que faltam. Alterações novas invalidam aceite anterior. | Demo Conductor torna escopo e loading concretos; documento atual trata divergência remota antes do merge. |
| **Adaptar aplicação como etapa explícita** | Antes de integrar, apresentar destino, artefato verificado e pendências; durante, estado pendente; depois, resultado reconciliado de Git. | Presença de botão e texto do agente não comprovam efeito. Commit, publicação da PR e integração são operações diferentes. |
| **Descartar restauração ambígua** | Apresentar arquivos/versões afetados e efeito sobre conversa antes de restauração; preservar trilha de auditoria do Orchestrix. | Checkpoint pode abranger alterações manuais e vários chats. Não copiar a aparência de undo sem revelar o escopo real. |

O custo dessas distinções deve ser testado com usuários: informações relevantes à decisão ficam próximas da ação; IDs, protocolo e diagnóstico técnico permanecem em detalhes. O objetivo é coordenar trabalho e decisões sem exigir que o usuário interprete o estado de cada terminal.

## Fechamento da rodada

Foi acrescentada **uma demo real com transições de revisão parcialmente observadas**, duas capturas novas e fontes atuais de recuperação, feedback e proteção de merge. O percurso dinâmico completo de bloqueio/recuperação continua documental; integração final continua não observada. Não há evidência nova de multiaccount, identidade de cobrança ou cotas independentes.

Próximo teste útil de UX: uma pergunta respondida mas rejeitada pelo runtime, uma retomada indisponível e um artefato alterado após revisão. Esses estados podem ser ensaiados no protótipo sem login ou inferência; o piloto mede se o usuário reconhece causa, consequência e próximo passo. Não substituem a evidência de percurso em concorrentes exigida pelo gate existente.
