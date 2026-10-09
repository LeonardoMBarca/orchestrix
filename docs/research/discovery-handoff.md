# Handoff da pesquisa D0 para D1 e a retomada posterior de M0

Criado em **8 de outubro de 2026**, com continuação registrada em **9 de outubro de 2026**. D0 foi encerrado como pesquisa na [conclusão consolidada](d0-conclusion.md), que registra a revisão explícita do método e o requisito anterior não demonstrado integralmente. Este handoff organiza D1 e as validações técnicas posteriores; não atribui aceite aos spikes nem substitui ADRs.

**Ordem de trabalho:** D0 concluído → D1 com piloto inicial → retomada de M0. Aproveitar o protótipo existente; os fundamentos técnicos adiante orientam M0 depois do piloto.

## Evidência disponível e limite

[Jornadas](solution-journeys.md), [Conductor](conductor-dynamic-evidence.md), [Cline/Superset](workflow-demonstrations.md) e [OpenCode](opencode-media-observation.md) ampliam capturas/mídias anteriores. O [posicionamento](product-positioning.md) e a [proveniência](landscape-and-provenance.md) comparam nove famílias. As cenas fundamentam composição, feedback, atividade e revisão; os passos apenas documentados continuam marcados. Não houve quinze jornadas completas de concorrentes, benchmark de produtividade ou teste de cotas/contas.

A demonstração completa do workflow Conductor foi obtida e examinada por 68 frames; feedback, correção, PR, merge e arquivamento têm estados sucessivos observados, com limites de confirmação remota. Atenção do agente e recuperação permanecem incompletas. A mídia antiga AskUserQuestion falhou em TLS; não foi repetida. YouTube permanece preview. A tentativa posterior de player por Computer Use foi encerrada por política de URL indeterminável; a pesquisa continuou com mídia pública. Esses limites constam dos relatórios.

## D1: trabalho imediato

1. Avaliar onboarding/entrada, contexto/arquivos, tarefa curta e relação objetivo/tarefas/tentativas da [entrega D1](../design/d1-delivery.md). Usar POS-01 a POS-14 e [ACC-01 a ACC-07](subscription-account-ux.md), com capabilities e dados desconhecidos explícitos.
2. Verificar condições adicionais de Studio, tokens/componentes, painéis e estados já preparados: conteúdo extenso, zoom, cores forçadas e movimento reduzido. Usar POS-15/16; a verificação técnica existente de teclado/layout não substitui conforto no piloto ou escala do aplicativo nativo.
3. Executar OX-D05 com o responsável: registrar conclusão, ajuda, erros e desconforto. Corrigir problemas críticos; decidir edição leve e nomes de presets. Avaliação externa permanece antes do alpha.

Os critérios e fontes estão no [benchmark](experience-decisions.md) e nas [14 decisões de D0](d0-conclusion.md). Dados simulados permitem avaliar compreensão; não certificam runtime ou cota.

## Continuação em 09/10 — D1 e preparação offline

O responsável solicitou prosseguimento do plano. O piloto individual continua solicitado, sem resultado humano registrado; a continuação não conta como feedback de uso nem fecha OX-D05. O pré-requisito de piloto registrado e ausência de problemas críticos para retomar M0 permanece no [backlog](../DEVELOPMENT_BACKLOG.md).

O escopo desta rodada foi verificar condições adicionais do protótipo D1 e preparar cenários offline de supervisão Windows com processos próprios e dados sintéticos. A [entrega D1](../design/d1-delivery.md) registra correção do foco e 24 cenários aprovados. A [preparação offline](offline-lifecycle-validation.md) registra 48 testes aprovados, incluindo pai encerrado com descendente ativo, cancelamento sem confirmação e perda de observação, preservando estado desconhecido quando faltar evidência. Não houve nova autenticação, chamada de modelo ou execução de agente de código. As versões, condições e limites estão nesses registros; não são resultados humanos ou aceite do supervisor de produção.

Essa preparação antecipa trabalho independente sem considerar M0 retomado/concluído, aprovar adapter ou iniciar o Core de produção. O [status](../DEVELOPMENT_STATUS.md) conserva o registro de entregas/resultados; o roteiro do piloto existente permanece disponível, sem substituir seu uso por novos questionários.

**Escopo de controle planejado:** em M2, o daemon terá suspensão/retomada mínima de novas admissões, inclusive na preferência de fechamento da janela, preservando tentativas ativas. Isso é distinto de pausa de turno, cancelamento e término confirmado. M3 amplia políticas e controle global do scheduler para DAG, concorrência e escopos de projeto/conexão/grupo. O protótipo não demonstra esses controles de daemon/scheduler implementados.

## M0: decisões fundamentadas para a retomada depois do piloto

As escolhas abaixo orientam o harness e contratos provisórios quando M0 for retomado. O gate piloto continua conforme backlog. A evidência técnica e os testes propostos constam em [decisões de integração](integration-decisions.md); fake e probe antecipados permanecem em [runtime-harness-results.md](runtime-harness-results.md).

| Escolha proposta | Por que já permite um spike | Limite a preservar |
| --- | --- | --- |
| Core separado dos adapters | Protocolos diferentes podem produzir um mesmo envelope de eventos e resultados. | O protocolo do worker não substitui scheduler, política, plano ou aceite do Orchestrix. |
| Connection, Session e Attempt explícitas | Permite testar correlação, replay, troca de conexão e isolamento sintético. | Identidade sintética não comprova conta real, assinatura ou capacidade independente. |
| Configuração solicitada e efetiva distintas | Evita afirmar que uma escolha de modelo/thinking foi aplicada sem observação. | Capacidade ausente ou não observável permanece desconhecida; não inventar fallback. |
| Estados de perda de sinal e cancelamento intermediários | Permite simular timeout, crash, aprovação pendente e reconciliação. | Perda do pipe não prova término; pedido de cancelamento não prova árvore encerrada. |
| Resultado do runtime separado do aceite de Git | Permite testar checks falhos, revisão e invalidação por mudança de artefato. | Mensagem de sucesso não autoriza integração; o aceite pertence à versão revisada. |
| Um supervisor controlado por tentativa | Permite medir framing, stderr, exit e limites do processo fake no Windows. | Não reutilizar o daemon pessoal nem afirmar supervisão de descendentes sem teste específico. |

O fake pode usar somente subprocessos e dados sintéticos, sem SDK de provedor, login ou chamadas de modelo. Deve exercitar JSON fragmentado/UTF-8, configuração incompatível, permissão exata, cancelamento, falha de autenticação/cota simulada, replay e conexões A/B. Esse avanço não depende de comparar estilos visuais de concorrentes. Seus resultados também não promovem automaticamente um adapter nativo a confiável.

Para o primeiro adapter nativo, a pesquisa recomenda comparar o caminho delimitado de execução Codex com a superfície de sessões/aprovações do app-server. A escolha depende da versão concreta, das capacidades necessárias e dos resultados do spike; não da aparência do aplicativo Codex. ACP continua candidato negociado para uma integração posterior, sem exigir que todos os runtimes adotem o mesmo transporte. [Pesquisa técnica e fontes](integration-decisions.md)

Incluir INT-10: OAuth próprio oficial para uso autorizado do plano ChatGPT em apps OSS locais. O contrato documenta registros separados e renovação pelo aplicativo; o caminho é distinto do login Codex usado no probe. Comparar elegibilidade, identidade, cobrança, capabilities e resume, sem importar credenciais pessoais. [Pesquisa de contas/assinatura](subscription-account-ux.md)

## Pendências que afetam principalmente o desenho de UX

- Avaliar no protótipo pergunta/permissão, resposta e retomada, inclusive rejeição e erro. A pesquisa sustenta distinguir estados; a interação mais clara exige piloto, e sua confirmação no runtime exige M0/M1.
- Avaliar envio de comentário, nova alteração, revisão e aplicação correta. Conductor acrescenta evidência dinâmica; envio Cline/Superset e aplicação final não têm confirmação integral em todas as referências. Não inferir enforcement dessa comparação.
- Validar com usuários a densidade da visão de trabalho, prioridade da fila de atenção, opções avançadas no composer e navegação por teclado. Capturas e previews não medem compreensão ou custo de uso.
- Diferenciar visualmente autenticação do aplicativo, Git e agente; capacidade do modelo, contexto e cota. A forma final exige pesquisa, mas a separação conceitual já deve existir no contrato.

Esses cenários pertencem à avaliação D1; pesquisa externa pode ser retomada se uma decisão concreta depender de detalhe ainda não observado. Resultados offline serão aproveitados quando M0 for retomado, conforme backlog.

## Riscos que bloqueiam ações específicas

| Condição ainda não demonstrada | Ação que precisa esperar | Trabalho que pode continuar |
| --- | --- | --- |
| Origem efetiva de autenticação, identidade e cobrança desconhecida | Admitir execução real sob promessa de assinatura estrita, roteamento multiaccount ou cota independente. | Inventário de capacidades sem credenciais; fixtures e perfis sintéticos. |
| Condições do provedor/runtime não verificadas para a modalidade escolhida | Distribuir ou executar essa combinação como integração suportada. A restrição documentada para autenticação claude.ai no Agent SDK não autoriza nem proíbe automaticamente toda integração CLI; a modalidade precisa ser verificada. | Contrato genérico e pesquisa oficial da modalidade concreta. |
| Enforcement de política e resposta a permissões não demonstrados | Permitir ferramenta ou ação que exija o limite de segurança prometido. | Testar requests sintéticos, rejeição, escopo e validade das aprovações. |
| Worker original pode continuar ativo após perda de conexão/cancelamento | Liberar locks/capacidade, remover sua worktree ou iniciar outro escritor substituto. | Registrar estado desconhecido e investigar supervisor/reconciliação. |
| Versão do artefato revisado mudou, checks falharam ou ownership de Git é incerto | Integrar o resultado na branch destino. | Preservar diff, evidências e findings; preparar nova revisão. |
| Sessão não vinculada de forma confiável à Connection e ao ambiente | Retomar em outra conta/perfil, importar estado pessoal ou declarar isolamento. | Testar correlação com referências opacas e caminhos sintéticos. |

Esses limites bloqueiam a ação dependente, não toda a execução do plano. A validação deve produzir evidência da instalação e versão reais: sem tokens, e-mails ou arquivos pessoais de autenticação em fixtures. Windows nativo e WSL exigem combinações explicitamente identificadas, sem conversão implícita de paths. [Contrato, riscos e supervisor Windows](integration-decisions.md)

## Próxima decisão de pesquisa

Após o harness, registrar quais propriedades foram demonstradas offline e quais continuam desconhecidas no runtime real. O teste nativo deve ser delimitado e ter identidade, ambiente, política e destino de alterações definidos antes de admitir uma tarefa. UX pode continuar sendo refinada; não deve ser usada como substituto da evidência de autenticação, cancelamento, enforcement e integração segura.

OX-D01/OX-D02 estão concluídos como pesquisa pelo critério consolidado no [backlog](../DEVELOPMENT_BACKLOG.md) e na [conclusão](d0-conclusion.md). Autenticação, recuperação, integração e piloto conservam seus gates independentes.
