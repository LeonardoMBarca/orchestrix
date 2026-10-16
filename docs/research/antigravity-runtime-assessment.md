# Antigravity — avaliação de integração para OX-002

Consulta em **15/10/2026**, a partir de `docs/development-plan`, HEAD `14f28d1`. Escopo: fontes oficiais e metadados locais, sem instalação, login, inferência, probes autenticados, execução da interface ou acesso a credenciais. Este documento aprofunda a viabilidade deixada aberta na [pesquisa anterior](product-positioning.md#antigravity-como-referência-adicional); não altera seus resultados históricos nem conclui OX-002.

## Decisão deste recorte

**Não liberar um adapter Antigravity por assinatura nesta rodada.** Existe uma CLI oficial controlável por processo, mas elegibilidade de uso pelo Orchestrix, isolamento de autenticação/configuração, contenção e cobrança ainda não estão comprovados. Nenhum segundo provider foi implementado. O caminho Codex já validado permanece independente desta investigação, conforme [ADR-0001](../adr/0001-single-runtime-first-class.md).

O item 6 dos [Additional Terms](https://www.antigravity.google/terms) restringe acesso ao serviço por software de terceiros e cita o uso de OAuth do Antigravity por OpenClaw. A página distingue contratos Enterprise dos termos pessoais. Isso exige esclarecer com o Google o enquadramento de um produto terceiro que supervisiona a CLI oficial, tanto no uso pessoal quanto na distribuição pública/comercial. A existência de documentação de automação não resolve expressamente esse ponto. Não se presume aprovação ou uma exceção por usar subprocessos; autorização local do usuário não constitui permissão do fornecedor. Também não foi identificada nesta rodada licença para redistribuir o binário CLI dentro do Orchestrix. [Termos aplicáveis](https://www.antigravity.google/terms)

## Fontes, superfícies e versões

As páginas funcionais são documentação contínua, sem contrato congelado por release. Versão publicada e comportamento documentado ficam separados de execução observada.

| Superfície | Referência consultada | Observação nesta máquina |
| --- | --- | --- |
| Antigravity CLI `agy` | Changelog: **1.3.6, 15/10/2026**. [Releases](https://www.antigravity.google/docs/changelog#antigravity-cli) | Não encontrada no PATH nem no caminho padrão consultado; nenhum comando executado. |
| Python SDK `google-antigravity` | Changelog: **0.1.22, 12/10/2026**. [SDK](https://www.antigravity.google/docs/sdk/overview/), [releases](https://www.antigravity.google/docs/changelog#antigravity-sdk) | Não instalado ou importado por esta rodada; disponibilidade local não investigada. |
| Aplicativo local | Metadados do arquivo `Antigravity.exe`: **FileVersion 2.9.1**, ProductVersion **2.9.1.0**, Company Google | Apenas existência, versão, tamanho, hash e resultado Authenticode observados. Não comprova CLI, login ou entitlement. |
| Serviço gerenciado/API | SDK distingue runtime Python local da API REST/gRPC gerenciada. [SDK overview](https://www.antigravity.google/docs/sdk/overview/) | Não acessado; conexão API separada, sem autorização de uso nesta rodada. |

O changelog 1.3.6 também registra cancelamento e repetição internos após conexão sem dados por cinco minutos; 1.3.4 registra backoff/retries próprios. A política do adapter deverá distinguir essas repetições internas de um redispatch pelo Orchestrix. Não há evidência de controle oficial para desativá-las. Auto mode foi anunciado em 1.3.5 para macOS/Linux, com Windows futuro; não transferir essa capacidade entre sistemas. [Changelog](https://www.antigravity.google/docs/changelog#antigravity-cli)

### Evidência local delimitada

Inspeção PowerShell somente de metadados:

- `Get-Command agy, antigravity -All`: ambos ausentes no PATH herdado.
- `%LOCALAPPDATA%\agy\bin\agy.exe`: ausente no caminho documentado para Windows. Isso não prova ausência em todos os discos. [Instalação oficial](https://www.antigravity.google/docs/cli/install/)
- `%LOCALAPPDATA%\Programs\Antigravity\Antigravity.exe`: presente, **222.855.680 bytes**, SHA-256 `561e6d0d28bd19f7326a8546f388366a8cc673c35306029bfe370d0dd03a1545`; `Get-AuthenticodeSignature` retornou `NotSigned`. Esses metadados não autenticam a procedência do arquivo nem identificam a superfície de integração.
- Não foi identificado um executável CLI apropriado para executar `--help`/`--version` com segurança. O aplicativo encontrado não foi iniciado. Não foram lidos keyring, perfis, tokens, bancos de conversa, configuração pessoal, logs ou processos do produto.

## Autenticação e cobrança

O fluxo CLI documentado utiliza login Google e keyring do sistema; a documentação atual de autenticação informa compartilhamento de credenciais entre superfícies locais. O modo `modelProvider: gemini` mais `GEMINI_API_KEY` usa a Gemini API diretamente, sem sessão Google da CLI. ADC/conta Business são modalidades Cloud/Enterprise distintas; o nome do executável não identifica a origem de cobrança. Não houve validação de identidade ou plano da conta local. [Authenticate](https://www.antigravity.google/docs/install?tab=cli)

O SDK apresenta Gemini API key, configuração Cloud/Vertex e opções de modelos locais. Não documenta nesse quickstart uma equivalência entre API key e cota de assinatura pessoal. Não será usado como fallback para tornar um adapter Subscription artificialmente viável. [SDK overview](https://www.antigravity.google/docs/sdk/overview/)

Planos pessoais têm limites variáveis por plano/capacidade e podem consumir créditos adicionais após a cota básica. O controle de overage oferece **Never/Always**; no CLI aparece também `useG1Credits`. Para Subscription Only, a decisão do Orchestrix é exigir origem de autenticação confirmada, overage desativado e enforcement verificável no escopo próprio, sem alterar a configuração pessoal. Enquanto esse controle não for comprovado, a combinação fica bloqueada; desconhecido não significa crédito zero. [Plans](https://www.antigravity.google/docs/plans/), [CLI AI credits](https://www.antigravity.google/docs/cli/credits/)

## Matriz de capacidades

Legenda: **oficial** = documentado, sem prova local; **observado** = metadado efetivamente lido nesta rodada; **unsupported** = limitação explícita da superfície; **unknown** = ausência de prova, não uma negação do recurso no produto. A referência CLI é a release publicada acima; páginas contínuas não provam que todas as releases anteriores tenham o mesmo comportamento.

| Capacidade | Estado/fonte oficial | Situação para Orchestrix |
| --- | --- | --- |
| Processo local sem TUI | **Oficial:** headless, JSON/NDJSON; stdout separado de diagnósticos stderr. [Headless](https://www.antigravity.google/docs/cli/headless/) | **Unknown** local; candidato técnico preferencial, sujeito aos gates anteriores. |
| Sessão contínua | **Oficial:** stdin `stream-json`, eventos `user`; saída `init`, `step_update`, `result` por turno. [Headless](https://www.antigravity.google/docs/cli/headless/) | **Unknown** tradução/conformance. IDs nativos ficam no adapter. |
| Encerramento/cancelamento | **Oficial:** EOF aguarda o turno; **unsupported:** `control_request`/`control_response`. [Headless](https://www.antigravity.google/docs/cli/headless/) | **Unknown** cancelamento cooperativo headless no Windows. Fechar stdin não o comprova. |
| Resultados observáveis | **Oficial:** terminal/uso e schema; contadores cumulativos na sessão. Aprovação de ferramenta pode ser negada com exit 0. [Headless](https://www.antigravity.google/docs/cli/headless/) | **Unknown** execução local. Exit 0 não substituirá diff/checks. |
| Retomada explícita | **Oficial:** `--conversation <id>`; `--continue` consulta cache por workspace e pode criar outra conversa se não encontrar a anterior. [Resume](https://www.antigravity.google/docs/cli/commands/resume/) | **Unknown** retomada real. Exigir o ID esperado; não usar “mais recente” como identidade. |
| Modelo por execução | **Oficial:** catálogo varia com plano; seleção pertence ao turno. [Models](https://www.antigravity.google/docs/models/) | **Unknown** catálogo/entitlement desta conta. Não importar modelos Gemini API como catálogo Subscription. |
| Thinking por execução | **Oficial:** `--effort` não altera default salvo; níveis dependem do modelo, incluindo `xhigh`/`max` somente quando ofertados. [Effort](https://www.antigravity.google/docs/cli/commands/effort/) | **Unknown** valores efetivos/combinações locais. Nenhuma equivalência universal com `maximum`. |
| Fast | **Oficial:** `/fast` altera o fluxo de planejamento. [CLI reference](https://www.antigravity.google/docs/cli/reference/) | **Unknown** controle headless/observação efetiva; não equivale automaticamente a service tier ou thinking baixo. |
| Permissões | **Oficial:** deny/ask/allow, precedência, concessões implícitas no workspace e normalização Windows. [Permissions](https://www.antigravity.google/docs/permissions/#antigravity-cli) | **Unknown** enforcement de escrita/leitura/rede e regras herdadas. “Request review” não é “read-only”. |
| Sandbox | **Oficial:** `--sandbox` e configuração CLI; sandbox de terminal tem limites e escapes próprios. [Sandbox](https://www.antigravity.google/docs/sandbox/#antigravity-cli) | **Unknown** contenção Windows. Job Object prova ownership de processos, não sandbox do runtime. |
| Delegação nativa | **Oficial:** subagentes, ferramentas e herança de permissões; ferramentas inválidas podem travar configuração customizada. [Subagents](https://www.antigravity.google/docs/subagents/) | **Unknown** forma de desabilitar/comprovar toda delegação no escopo de um worker. Restrição obrigatória sem enforcement bloqueia a combinação. |
| Cota/reset | **Oficial:** `/usage` atualiza o backend e exibe painel por modelo. [Usage](https://www.antigravity.google/docs/cli/commands/usage/) | **Unknown** leitura tipada/estável/autorizada para o adapter. Não fazer scraping do TUI ou inventar percentuais. |
| Configuração própria | **Oficial:** settings CLI persistentes e alguns overrides de invocação. [Settings](https://www.antigravity.google/docs/cli/using/) | **Unknown** diretório/perfil independente, isolamento do keyring e bloqueio de hooks/MCP herdados. Mudar HOME não comprova isolamento. |
| Múltiplas contas | **Unknown:** não foi encontrado contrato que vincule dois processos a perfis pessoais independentes sem afetar o login existente. [Authenticate](https://www.antigravity.google/docs/install?tab=cli) | Não anunciar; múltiplas sessões ou nomes de perfil não provam duas contas/cotas. |
| Coexistência app/CLI | **Oficial:** Remote Control distingue instâncias e menciona login CLI separado do editor; Authenticate descreve autenticação compartilhada. [Remote Control](https://www.antigravity.google/docs/remote-control/), [Authenticate](https://www.antigravity.google/docs/install?tab=cli) | **Unknown** isolamento por versão/superfície; a diferença documental precisa de prova, sem logout/relogin global. |
| Controle remoto | **Oficial:** túnel e modalidade persistente que registra serviço/tarefa do sistema. [Remote Control](https://www.antigravity.google/docs/remote-control/) | Fora da prova mínima; não iniciar daemon remoto nem tratá-lo como API local do Orchestrix. |
| Distribuição/assinatura | Restrição de terceiros nos termos pessoais; contratos Enterprise são distintos. [Terms](https://www.antigravity.google/terms) | **Bloqueado** até esclarecer permissão para este escopo, sem declarar que o fornecedor rejeitou um pedido ainda não feito. |

## Consequências para o contrato existente

Estas são decisões de integração propostas, não comportamento implementado:

- Manter Connection → sessão nativa → tentativa explicitamente vinculadas e catálogo separado de autenticação/cobrança. Uma conta e execução serial bastariam para a primeira prova; multiaccount não será pré-requisito inventado.
- Registrar configuração solicitada e campos efetivamente confirmados. A ausência de confirmação permanece unknown; nenhum default observado em Codex vira configuração obrigatória do Antigravity.
- Preservar EOF, término do processo, fechamento de streams, encerramento da árvore e terminal remoto como fatos separados. Não emitir terminal fictício ao forçar somente o Job próprio.
- Não repetir turnos incertos, nem confundir retries internos com nova autorização. Preservar artefatos antes de intervir; checks continuam determinísticos e externos à narrativa do modelo.
- Tratar retomar conversa nativa e construir Context Pack como operações distintas. Não importar transcript interno como memória do Orchestrix, conforme [Context Ownership](../CONTEXT_OWNERSHIP.md).

## Próxima prova mínima, condicionada

**Próximo passo liberado agora:** registrar esta avaliação e manter a conexão Antigravity sem adapter executável. Não instalar para contornar os gates. Não perguntar ao usuário por autorização de inferência antes de tornar concreto o caminho elegível.

1. **Elegibilidade:** obter esclarecimento oficial sobre uso da CLI por Orchestrix pessoal/público, condições comerciais e redistribuição. Documentar a resposta e a superfície/contrato correspondente. Uma resposta Enterprise não libera por associação a assinatura pessoal.
2. **Metadados, com instalação autorizada separadamente se necessária:** identificar a CLI oficial e fixar versão/hash; executar apenas `--version`/`--help` e help de subcomandos revisados, com timeout de até 30 s e saída limitada a 64 KiB. Nenhum prompt, login, serviço ou update automático. Verificar antes os efeitos de primeira execução.
3. **Preflight autorizado sem inferência:** provar escopo próprio de configuração/autenticação, ausência de alteração do login existente, origem Subscription, overage impedido e políticas obrigatórias efetivas. Leituras administrativas precisam de autorização e limites; não iniciar plugins/hooks/MCP só para descobrir capacidades. Se qualquer requisito obrigatório continuar unknown, não enviar o primeiro turno.
4. **Somente então propor live limitado e opt-in:** até três turnos seriais, um worktree descartável e fixture sem código do Orchestrix; edição/check externo, reinício e retomada do mesmo ID recuperando marcador não reenviado, e interrupção após início confirmado. Até 90 s por turno/300 s por Job próprio, sem retries do Orchestrix, fallback API/overage ou mudança de configuração pessoal. Verificar viabilidade do cancelamento Windows antes de prometer cancelamento cooperativo; eventual força do Job não prova terminal remoto. Subagentes nativos devem permanecer comprovadamente desativados. Nenhum turno foi autorizado ou consumido por este documento.

## Verificações desta entrega

Leitura dos documentos vigentes, pesquisa web oficial e inventário local descrito acima. `git diff --check` e destinos dos links internos foram conferidos. Não houve teste de runtime, conta, cota, retomada, sandbox ou coexistência; não houve build, alteração de dependências, UI ou novo adapter. Plano/backlog/status e o aceite do runtime Codex permanecem sob sua autoridade existente.
