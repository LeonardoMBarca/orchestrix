# Assinatura, contas e onboarding: contrato oficial e decisões de UX

Pesquisa D0 consultada em **8 de outubro de 2026**. Foram lidas fontes oficiais OpenAI sobre Sign in with ChatGPT. Não houve cadastro, OAuth, acesso a credenciais ou inferência nesta rodada. A integração descrita aqui é um **candidato para M0**, não uma capacidade implementada do Orchestrix.

## Achado que altera a pesquisa

A documentação atual oferece uso elegível do plano ChatGPT em aplicativos open source e hospedados localmente, mediante permissão própria. Identidade, autorização para uso do plano e conversas do ChatGPT são coisas diferentes: o fluxo não concede acesso àquelas conversas. A documentação encaminha aplicativos pagos ou hospedados remotamente a um processo distinto de interesse. **Consequência:** avaliar essa modalidade para o Orchestrix local; reavaliar sua elegibilidade se a distribuição ou monetização mudar. [Overview oficial](https://developers.openai.com/siwc/token-sharing-open-source)

O protocolo documenta contas salvas com registros separados e seleção de outra conta sem logout de todas as demais. Cada registro conserva identidade validada, client ID emitido e credenciais correspondentes; e-mail sozinho não identifica o registro. **Consequência:** há uma referência oficial para um account picker, além de múltiplas sessões. Isso não demonstra execução concorrente ou cotas independentes no Orchestrix. [Accounts and sessions](https://developers.openai.com/siwc/token-sharing-open-source/profiles-and-sessions)

Cliente e host também são diferentes: hosts do mesmo usuário/workspace podem compartilhar configuração e limites de uso. Criar instalações, client IDs ou sessões adicionais não autoriza apresentar capacidade adicional. Esse dado deve orientar `CapacityGroup` e o estado **capacidade desconhecida**. [Cliente versus host](https://developers.openai.com/siwc/token-sharing-open-source#a-client-vs-an-agent-host)

## Modalidades que o produto precisa distinguir

| Modalidade | O que a evidência estabelece | Limite / trabalho seguinte |
| --- | --- | --- |
| Autenticação gerida pelo Codex instalado | O [experimento antecipado](runtime-harness-results.md) confirmou uma resposta trivial e interrupção com autenticação ChatGPT existente. | Não demonstrou onboarding próprio, duas contas, retomada real ou cota independente. |
| Sign in with ChatGPT autorizado para o Orchestrix | Contrato oficial de registro inicial, consentimento, validação de identidade/permissão e reautorização. A conta nova só se torna ativa após validação. [Sign-in](https://developers.openai.com/siwc/token-sharing-open-source/sign-in) | Validar modalidade, versão, credenciais próprias, armazenamento e isolamento em OX-001/OX-002. Não importar tokens pessoais do Codex. |
| Credenciais de API ou outro provedor | Outra origem de autorização e cobrança, quando suportada pelo adapter. | Exigir escolha explícita e observável; não converter limite de assinatura em fallback pago. Nenhuma integração foi testada aqui. |

O domínio não deve deduzir cobrança pelo hostname ou pelo nome de um campo `api_key`. O fluxo oficial de uso do plano utiliza OAuth no endpoint público Responses; isso não o transforma automaticamente em cobrança por uma chave de API. O catálogo deve ser obtido para a conta selecionada. Um stream que começou a responder ainda pode terminar com falha de limite. [Models and inference](https://developers.openai.com/siwc/token-sharing-open-source/models-and-inference)

## Consequências para o adapter futuro

O guia oficial configura app-server por stdio com um provider Responses e token OAuth fornecido pelo aplicativo; o próprio aplicativo renova o token, reinicia o processo e retoma o thread salvo. `model/list` pode retornar catálogo embarcado, sem comprovar entitlement. **INT-10:** comparar esse caminho com autenticação nativa gerida pelo runtime; registrar separadamente identidade, origem de autorização, cobrança e lifecycle. O harness existente não testou essa modalidade. [Codex app-server para uso do plano](https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server)

Há limitações próprias de ferramentas, parâmetros e persistência. Histórico local e `thread/resume` não equivalem a armazenamento remoto de conversa; ferramentas locais e ferramentas hospedadas têm contratos diferentes. O catálogo de capabilities precisa refletir a combinação concreta, e não todas as funções do aplicativo ChatGPT. [Preview limitations](https://developers.openai.com/siwc/token-sharing-open-source/preview-limitations)

## Requisitos de experiência para D1

| ID | Situação simulada | Decisão para o protótipo / evidência no piloto |
| --- | --- | --- |
| ACC-01 | Abrir conexões com duas contas ChatGPT e uma conexão Claude. | Mostrar rótulo estável, provider, modalidade e estado. A pessoa distingue conta, perfil de agente e sessão; não precisa trocar login global para selecionar uma conexão. |
| ACC-02 | Adicionar conta enquanto outra tentativa está ativa. | Nova autorização fica pendente, separada da conexão em uso. O usuário prevê qual tentativa será afetada; nenhuma sessão muda de dono implicitamente. |
| ACC-03 | Duas contas com o mesmo e-mail ou múltiplos workspaces. | Rótulos distintos e identidade validada evitam colisão. Identificadores sensíveis e tokens não entram no histórico de execução nem em screenshots de diagnóstico. |
| ACC-04 | Primeira conexão com uso do plano autorizado. | Confirmar a modalidade uma vez e torná-la visível junto ao composer/modelo. Disponibilizar gerenciamento de uso e seguir a identidade visual oficial do botão. |
| ACC-05 | Limite observado, consentimento negado ou autorização expirada. | Explicar a causa e preservar a tarefa. Para limite, oferecer gerenciamento de uso; aguardar ou selecionar conexão elegível são opções conforme política. Nova autorização não é um retry silencioso. |
| ACC-06 | Alternar conta antes de iniciar nova tentativa. | Atualizar modelos/capabilities e explicar opções incompatíveis. Catálogo visível não aparece como garantia de acesso; a tentativa antiga conserva seu snapshot. |
| ACC-07 | Encerrar uma conexão com execução pendente. | Distinguir parar novas requisições, confirmar término do worker, sair da conta e confirmação de revogação. Falha de rede não aparece como revogação remota confirmada. |

As diretrizes oficiais pedem confirmação inicial do uso do plano, identificação perto da entrada de trabalho e acesso a gerenciamento de uso. Também distinguem uso do plano da assinatura/cobrança do aplicativo. São referências específicas dessa modalidade; não aplicar marca OpenAI a conexões de outros providers. [UI/UX guidelines](https://developers.openai.com/siwc/ui-ux-guidelines)

Os cenários complementam POS-03/04/05/06/13/16 do [posicionamento](product-positioning.md). D1 avalia compreensão em simulação. M0 comprova autenticação e lifecycle; M3 habilita pooling/fallback somente com identidade e capacidade demonstradas. A descoberta documental de multiaccount não antecipa esse aceite técnico.
