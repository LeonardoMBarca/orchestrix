# OX-D01 — Decisões de integração de runtimes

Pesquisa realizada em **8 de outubro de 2026**, com documentação oficial e inspeção local somente leitura. Este documento propõe escolhas para os spikes M0 e para o contrato OX-003; não altera os ADRs aceitos nem certifica um adapter como funcional. Nenhum modelo, login, conta, token ou configuração pessoal foi acessado durante a inspeção.

## Recomendação executável

Manter um contrato interno pequeno, com adapters específicos onde necessário e um transporte ACP opcional. O Core continua responsável por política, tarefas, tentativas, evidências e integração. O protocolo do provedor não se torna o modelo de domínio nem a API do daemon.

O primeiro experimento deve ser offline, com processo fake e fixtures. Em seguida, comparar Codex CLI estruturada e app-server na versão instalada. O app-server é o candidato para a experiência interativa do produto; sua maturidade experimental exige versão suportada explícita e teste de compatibilidade. Se não satisfizer o contrato, a limitação não justifica retirar aprovações ou isolamento: manter o modo permitido mais limitado ou escolher outro runtime validado.

## Decisões propostas

| ID | Escolha | Tratamento no MVP | Evidência necessária para confirmar |
| --- | --- | --- | --- |
| INT-01 | Contrato Orchestrix independente do transporte | **Adotar.** Comandos, eventos e capabilities próprios; sem tipos de ACP/Tauri no domínio. | Fake e adapter real passam os mesmos cenários. |
| INT-02 | JSON-RPC/NDJSON por stdio para processos locais | **Adotar como base.** Caminho explícito do executável, argumentos estruturados e stderr separado. | UTF-8, fragmentação, saída extensa, timeout e cancelamento no Windows. |
| INT-03 | ACP como adapter de comunicação | **Adaptar.** Implementar apenas o subconjunto negociado por capabilities; não exigir ACP de todos os runtimes. | Uma implementação concreta demonstra permissões, cancelamento e configuração efetiva. |
| INT-04 | Codex nativo em vez de wrapper universal | **Investigar primeiro.** CLI para tarefa delimitada; app-server candidato para sessões/aprovações do produto. | Contrato da versão suportada, autenticação observável e supervisão independente do daemon pessoal. |
| INT-05 | Cline e OpenCode como candidatos ACP | **Investigar depois do contrato base.** Escolher um para validar o transporte genérico; não implementar ACP e HTTP do mesmo runtime simultaneamente. | Mesma identidade após resume e enforcement requerido pela política. |
| INT-06 | OpenHands como infraestrutura central | **Descartar para o MVP.** Aproveitar conceitos; um futuro adapter pode tratá-lo como worker. | Necessidade de uso que compense Python, outro lifecycle e nova camada de políticas. |
| INT-07 | Conta, provedor e runtime separados | **Adotar.** Uma Connection fixa o perfil de autenticação; sessões pertencem a ela. | Duas conexões não mudam login, armazenamento ou sessões uma da outra. |
| INT-08 | Progresso do agente como prova de conclusão | **Descartar.** Saída do runtime encerra uma etapa; Git/checks/revisão/integração determinam aceite. | Teste falho e artefato alterado invalidam aceite apesar de mensagem de sucesso. |
| INT-09 | Windows nativo e WSL como mesmo ambiente | **Descartar.** Registrar ambiente de execução; validar cada combinação sem conversão implícita de paths. | cwd, Git, processo e sandbox funcionam no ambiente declarado. |
| INT-10 | OAuth próprio para uso autorizado do plano ChatGPT | **Investigar em M0**, comparando a modalidade oficial OSS/local com autenticação gerida pelo Codex. [Pesquisa e UX](subscription-account-ux.md). | Registro/consentimento próprios, identidade validada, origem de uso/cobrança, renovação com retomada e duas conexões sem colisão. Nenhuma dessas propriedades foi testada por esta pesquisa. |

As decisões são recomendações desta pesquisa. A promoção para compromisso arquitetural deve registrar resultado do spike e ADR apropriado.

## ACP: o que reutilizar e o que conservar no Core

ACP negocia versão, capabilities e métodos de autenticação. Sessões básicas permitem criar, enviar prompt, receber updates e cancelar. Recursos omitidos são tratados como não suportados; criar um objeto genérico de capabilities não torna model selection, thinking ou retomada universalmente disponíveis. **Proposta:** mapear capacidades para `supported`, `unsupported`, `unknown` ou `runtime-managed` e guardar versão da observação. [ACP v1 — Initialization](https://agentclientprotocol.com/protocol/v1/initialization)

O transporte stdio usa JSON-RPC UTF-8 delimitado por newline; stdout contém somente mensagens do protocolo e stderr pode conter logs. O transporte HTTP está descrito como proposta em andamento. **Proposta:** usar stdio no spike ACP; o transporte entre Desktop e daemon é uma decisão independente. [ACP v1 — Transports](https://agentclientprotocol.com/protocol/v1/transports)

`session/load` é opcional e reproduz o histórico por notifications. `session/resume`, quando anunciado, reconecta sem replay. `cwd` deve ser absoluto. **Proposta:** distinguir replay de atividade nova, preservar IDs observáveis e nunca converter replay em nova conclusão de tarefa. O vínculo Connection/session/cwd precisa ser validado antes de qualquer retomada; sessão indisponível pode exigir Context Pack e sessão nova. [ACP v1 — Session Setup](https://agentclientprotocol.com/protocol/v1/session-setup)

Updates incluem mensagens, ferramentas, planos e uso quando informado. Permissão pode ser solicitada pelo agente; enviar cancelamento não é a confirmação final. A resposta ao prompt precisa encerrar como cancelada após o trabalho cessar, e updates podem chegar antes dessa resposta. **Proposta:** manter estado `cancelling`, resolver aprovações pendentes e aguardar confirmação ou timeout. Plano emitido pelo agente é proposta, não DAG aprovado do Core. [ACP v1 — Prompt Turn](https://agentclientprotocol.com/protocol/v1/prompt-turn)

ACP diferencia autenticação por protocolo de autenticação em terminal separado. O efeito de logout sobre sessões já ativas não é garantido. **Proposta:** onboarding separado de execução; nunca trocar login no processo que atende outra tentativa. Não inferir conta ou origem da cobrança apenas porque o handshake de autenticação terminou. [ACP v1 — Authentication](https://agentclientprotocol.com/protocol/v1/authentication)

ACP não deve carregar as responsabilidades de scheduler, grupos de capacidade, planos versionados, política efetiva, aprovação de integração ou memória do projeto. Essas informações permanecem no Core. Tampouco a existência de uma solicitação de permissão significa que toda ação interna do runtime será interceptada: essa promessa depende de seu enforcement demonstrado.

## Escolha por runtime

### Codex

A superfície documentada do app-server inclui threads persistentes, retomada, turns, interrupt, eventos, pedidos de aprovação, catálogo de modelos/esforço e consulta de autenticação/limites. Solicitar cancelamento e receber a resposta imediata não substitui observar o término do turn. Há campos/métodos experimentais opcionais. **Proposta:** adapter nativo com stdio e superfície mínima; registrar solicitado/efetivo, vincular aprovações à tentativa e não importar o armazenamento interno de threads como banco do Orchestrix. Consultas de conta devem virar referências opacas e estado normalizado, sem conservar tokens ou e-mail em eventos. [Codex App Server](https://learn.chatgpt.com/docs/app-server)

A referência CLI classifica app-server como experimental e sujeito a mudanças. A CLI noninteractive oferece saída estruturada e retomada por ID. **Proposta:** fixar intervalo de versões testadas e guardar fixtures correspondentes; não considerar a presença do comando prova de estabilidade. Perfis/configuração são candidatos ao isolamento, não evidência de duas contas independentes. [Codex CLI — Reference](https://learn.chatgpt.com/docs/cli/reference)

**Resultado local:** somente versão e ajuda foram verificadas. O binário encontrado pertence à extensão VS Code e identifica-se como alpha. Ele serve ao spike, mas um aplicativo independente precisa localizar uma instalação explicitamente registrada, diagnosticar atualização/incompatibilidade e funcionar sem depender de um editor instalado. Não reutilizar o daemon pessoal do usuário para controlar tentativas isoladas antes de demonstrar esse limite.

### Cline

A referência CLI oferece configuração e dados separados, session ID, provider/model/thinking e escolha de backend. Autoaprovação é padrão na CLI, mas não em ACP; a CLI também pode salvar uma API key recebida como argumento. **Proposta:** testar backend local, diretórios distintos e flags explícitas; não enviar segredos em argumentos nem delegar worktree ao runtime quando o Core já a controla. A investigação precisa comprovar isolamento da autenticação, não somente do histórico. [Cline — CLI Reference](https://docs.cline.bot/cli/cli-reference)

O modo `--acp` usa stdio e documenta seleção de provider/model, permissões pelo cliente e retomada. Ele também informa login ChatGPT, além de modalidades Cline, mas isso não estabelece condições comerciais de outros provedores. `CLINE_PROVIDER` pode fixar o provider. **Proposta:** candidato ao primeiro spike do adapter ACP, com autoaprovação desativada e provider fixado; confirmar a identidade/cobrança selecionadas antes de admitir tarefa. Regras de assinatura permanecem questão por combinação de runtime, autenticação e provedor. [Cline — ACP](https://docs.cline.bot/usage/acp)

### OpenCode

OpenCode documenta subprocesso ACP via stdio, com ferramentas e sistema de permissões; alguns slash commands não são suportados. **Proposta:** tentar esse caminho quando o transporte ACP já existir. Não confundir disponibilidade de protocolo com teste de todos os recursos. [OpenCode — ACP Support](https://docs.opencode.ai/docs/acp/)

Como alternativa, o servidor HTTP oferece OpenAPI, sessões, abort, diff, resposta a permissões e SSE. A autenticação de transporte pode usar senha; ela é distinta da autenticação do provider. **Proposta:** usar adapter HTTP somente se o spike revelar uma capacidade necessária indisponível por ACP. Nesse caso, preferir processo próprio em loopback, autenticação restrita e reconciliação após desconexão; SSE sozinho não estabelece replay durável de eventos do Orchestrix. [OpenCode — Server](https://docs.opencode.ai/docs/server/)

Permissões incluem allow/ask/deny e padrões, com precedência de regras e defaults permissivos. **Proposta:** gerar configuração explícita por perfil, validar ordem e negar alterações do revisor. Padrões de shell não são sandbox do sistema operacional; testes precisam demonstrar o enforcement prometido. [OpenCode — Permissions](https://docs.opencode.ai/docs/permissions/)

A CLI informa que autenticação pode vir do arquivo local, ambiente e `.env` do projeto. Configuração personalizada é mesclada com outras camadas. **Proposta:** não considerar um diretório de config prova suficiente de conta/cobrança isoladas; investigar toda origem efetiva e falhar fechado no modo de assinatura estrito. Nunca ler ou copiar arquivos pessoais de autenticação para montar fixtures. [OpenCode — CLI](https://docs.opencode.ai/docs/cli/), [OpenCode — Config](https://docs.opencode.ai/docs/config/)

### OpenHands

`ACPAgent` delega a execução/contexto a subprocesso JSON-RPC, injeta contexto adicional e pode operar remotamente. Porém sua documentação descreve autoaprovação de permissões e fallback de autenticação para API keys. **Proposta:** não usar esse wrapper diretamente como integração central do MVP. Os limites de aprovação e cobrança do Orchestrix exigiriam adaptação adicional. Considerar OpenHands mais tarde como worker distinto quando houver necessidade demonstrada; não adicionar containers, servidor multiusuário e execução remota apenas por estarem disponíveis. [OpenHands — ACP Agent](https://docs.openhands.dev/sdk/guides/agent-acp)

## Contrato mínimo proposto para OX-003

O adapter expõe observações/capabilities e operações de início, envio de trabalho, resposta a pedido de permissão, cancelamento e encerramento. Retomada, seleção de modelo/esforço e cota são opcionais. O domínio recebe eventos normalizados; parsing e versões de wire protocol ficam na infraestrutura.

| Objeto | Campos relevantes | Regra |
| --- | --- | --- |
| Connection | installation ID, auth profile reference, ambiente, provider autorizado, billing mode, capacity group | Identidade e cobrança desconhecidas não são substituídas por suposição. |
| CapabilitySnapshot | runtime version, protocol version, connection ID, modelos/configuração observáveis, enforcement | Guardar estado e origem; ausência de informação vira desconhecido. |
| SessionReference | connection ID, provider session ID, cwd, versão observada | Trocar conexão inicia sessão nova; mesma conta não garante resume. |
| AttemptConfig | requested/effective model e reasoning, policy/context hash, workspace, timeout | Conservar snapshot por tentativa e explicar fallback. |
| PermissionRequest | request ID, attempt ID, ação/escopo, opções disponíveis, validade | Resposta vale para o pedido exato; cancelamento/reconexão não aprova automaticamente. |
| RuntimeObservation | connection-lost, execução unknown, instante e origem da observação | Perda de transporte ou ausência de eventos não confirma término, falha ou pausa. |
| RuntimeOutcome | completed/failed/cancelled, código observado, evidência de término | Resultado terminal confirmado não autoriza integração nem conclui tarefa por si só. |

A API do adapter e o envelope de eventos precisam ter IDs de tentativa, sessão e correlation; nem todo evento global precisa de task ID. Campos novos compatíveis são tolerados, mas mensagens essenciais inválidas bloqueiam a execução com diagnóstico. Eventos de replay são identificados e não provocam novamente efeitos externos.

Perda de sinal mantém a execução como desconhecida até a reconciliação de processo/sessão. Não iniciar uma tentativa substituta nem liberar capacidade ou locks de escrita enquanto o worker original puder continuar ativo. Se a supervisão confirmar encerramento sem resultado de protocolo, registrar a falha observada e sua evidência; essa confirmação é distinta da perda de transporte.

O runtime pode ser Cline/OpenCode enquanto o provider/model efetivo é outro. Portanto, revisão com runtime diferente não prova diversidade de modelo; duas contas também não. Registrar runtime, provider e família/modelo separadamente e mostrar diversidade desconhecida quando não for observável.

O Core coleta diffs e arquivos novos, fixa a versão verificada, executa checks e registra findings. Mudança posterior invalida aceite anterior. Integração serial e aprovação final pertencem ao Core, com reconciliação de Git depois de crash. Não copiar o próprio planejamento interno ou sistema de tarefas de um worker para substituir esse estado.

## Windows e ciclo de vida

A documentação oficial do Codex descreve sandbox nativo Windows com modos elevated/unelevated e diferenças de enforcement. **Proposta:** capability de sandbox representa a configuração efetiva observada; o spike inicial não instala nem modifica firewall, usuários ou políticas do sistema. [Codex — Windows Sandbox](https://learn.chatgpt.com/docs/windows/windows-sandbox)

OpenCode pode rodar nativamente, mas sua documentação recomenda WSL para a experiência Windows. **Proposta:** não impor WSL ao Orchestrix; classificar suporte por ambiente e manter paths e Git no mesmo lado da fronteira. Um adapter WSL é outra combinação de execução a testar. [OpenCode — Windows/WSL](https://docs.opencode.ai/docs/windows-wsl/)

Job Objects permitem supervisionar grupos de processos e encerrar processos associados; o comportamento de descendentes depende de associação e breakaway. **Proposta:** investigar essa fronteira no supervisor Rust, inclusive compatibilidade com o sandbox do runtime. Acknowledgment de cancelamento, processo pai encerrado e árvore encerrada são evidências distintas. PID sem identidade adicional não basta para recuperação. [Microsoft — Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)

Desconectar Desktop não encerra uma tentativa; encerrar daemon deve seguir política e reconciliar trabalho. No MVP, preferir processo controlado por tentativa e sessão persistida opcional; reaproveitar processos longos somente quando trouxer benefício observado. Interfaces de filesystem/terminal pelo cliente só são anunciadas quando implementadas com cwd, limites e auditoria adequados.

## Ambiente observado e harness offline

| Ferramenta | Evidência local |
| --- | --- |
| Node | `node --version`: `v24.19.0` |
| npm | `npm.cmd --version`: `11.17.0` |
| Git | `git --version`: `2.53.0.windows.2` |
| Codex | `codex --version`: `0.162.0-alpha.2`; ajuda de CLI, exec e app-server disponível |
| Rust/Cargo | `Get-Command rustc,cargo` não encontrou comandos no PATH desta sessão; instalação global não foi investigada. |

Não foram executados login, doctor, listagem de contas/modelos, app-server, inferência, instalação ou atualização. Os valores descrevem o ambiente observado, não versões exigidas para o produto.

**Viabilidade observada na pesquisa inicial:** processo Node fake e testes com módulos nativos, sem instalar SDK de provedor. `node:test` oferece runner embutido; `child_process` permite pipes de subprocessos, que precisam ser consumidos para não bloquear saída extensa. No Windows, wrappers `.cmd` têm semântica diferente de executáveis; o fake pode usar `process.execPath` e argumentos estruturados. A documentação de child process consultada é a corrente; implementar usando APIs já disponíveis no Node observado e testar localmente. [Node — Test Runner](https://nodejs.org/docs/latest-v24.x/api/test.html), [Node — Child Process](https://nodejs.org/api/child_process.html)

O harness deve cobrir seis grupos de cenários, usando somente dados sintéticos:

1. Handshake/capabilities, sucesso e configuração incompatível sem fallback oculto.
2. JSON fragmentado, UTF-8 dividido entre chunks, CRLF, evento adicional compatível e mensagem essencial inválida.
3. Permissão pendente, aprovação/rejeição exatas e cancelamento sem resposta humana.
4. Auth failure/rate limit simulados, timeout, crash e stderr volumoso.
5. Session load/resume simulado, replay e tentativa duplicada sem novo efeito.
6. Connection A/B com diretórios sintéticos e marcadores de identidade, garantindo que o Core não mistura sessões.

Esses testes validam o contrato e o supervisor básico, **não** provam autenticação real, condições de assinatura, telemetria de cota, sandbox nativo ou encerramento de netos no Windows. Árvore de processos e recuperação após morte do daemon exigem spike específico antes de declarar worker confiável.

## Saída de OX-D01 e próximos experimentos

Esta pesquisa fornece escolhas propostas, fontes e testes a executar. O adapter ainda não foi implementado. A sequência recomendada é: fake offline → comparação nativa Codex → um runtime ACP → demonstração de identidade/permissões/cobrança → contrato consolidado OX-003. Não iniciar por todos os providers, dois transportes por runtime ou infraestrutura distribuída.

Essa é a ordem interna proposta para os experimentos de M0. [D0 foi consolidado](d0-conclusion.md); a próxima etapa é D1 com piloto inicial, antes da retomada de M0. O [primeiro harness](runtime-harness-results.md) antecipado não determina a próxima atividade de design. Incluir INT-10 na comparação das modalidades Codex quando M0 for retomado.

Promover um adapter exige evidência da versão concreta: conexão escolhida, modelo/configuração efetivos ou explicitamente desconhecidos, worktree respeitada, eventos normalizados, aprovação aplicável, cancelamento observado e falhas classificadas. Suporte multiaccount só recebe `supported` depois do teste independente de autenticação, estado e capacidade compartilhada; autorização de assinatura deve usar as fontes do provedor apropriadas, sem tratar uma implementação intermediária como autorização comercial universal.
