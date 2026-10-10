# Backlog inicial de desenvolvimento do Orchestrix

Este backlog começa por D0/D1, pesquisa e design, implementa M0 a M2 do [plano de desenvolvimento](./DEVELOPMENT_PLAN.md) e prepara M3 e M4. A [execução do plano](./DEVELOPMENT_STATUS.md) entrega pesquisa, protótipo e primeiro spike real Codex; os tickets de produção ainda não foram implementados. O objetivo é fundamentar a solução e sua experiência antes de fechar um ciclo confiável de implementação, verificação, revisão e integração no aplicativo.

Os critérios de aceite devem ser demonstrados antes de considerar uma tarefa concluída. A sequência principal é D0 → D1 → M0 → M1 → M2. Dependências permitem paralelizar tarefas independentes dentro do marco. **D0 está concluído pelo critério de pesquisa consolidado em 08/10/2026; D1 tem [estudo implementado e verificado](./design/d1-delivery.md), com piloto humano pendente.** A [conclusão](./research/d0-conclusion.md) registra a alteração de método e os limites; os experimentos antecipados não fecham D1/M0.

## 1 Fila inicial e dependências

| Ticket | Entrega | Marco | Depende de |
| --- | --- | --- | --- |
| OX-D01 | Pesquisa comparativa de soluções | D0 | — |
| OX-D02 | Benchmark de UX e referências visuais | D0 | —; conclusões coordenadas com OX-D01 |
| OX-D03 | Jornadas, informação e wireframes | D1 | OX-D01, OX-D02 |
| OX-D04 | Direção visual, protótipo e tokens | D1 | OX-D03 |
| OX-D05 | Avaliação de uso e iteração | D1 | OX-D04 |
| OX-001 | Harness Codex e matriz de capacidades | M0 | D0 consolidado; D1 com gate piloto de OX-D05 |
| OX-002 | Spikes Claude, Antigravity e isolamento de contas | M0 | D0 consolidado; D1 com gate piloto de OX-D05 |
| OX-003 | Contrato normalizado e runtime fake | M0 | Primeiro runtime aprovado em OX-001 ou OX-002 |
| OX-004 | Domínio de tarefa, tentativa e conexão | M1 | OX-003 |
| OX-005 | SQLite, migrações, eventos e idempotência | M1 | OX-004 |
| OX-006 | Git worktrees e proveniência de artefatos | M1 | OX-004 |
| OX-007 | Supervisor e primeiro adapter de produção | M1 | OX-003, OX-005 |
| OX-008 | Política efetiva e Context Pack inicial | M1 | OX-003, OX-004 |
| OX-009 | Verificação determinística e worker completo | M1 | OX-005 a OX-008 |
| OX-010 | API local e daemon supervisionado | M1/M2 | OX-003, OX-005, OX-007 |
| OX-011 | Reconciliação e recuperação de falhas | M1 | OX-005 a OX-010 |
| OX-012 | Shell Desktop e abertura de projeto | M2 | OX-010, OX-D04, gate piloto de OX-D05; integração final com OX-011 |
| OX-013 | Revisão independente e correções limitadas | M2 | OX-008, OX-009 |
| OX-014 | Integração serial e aprovações | M2 | OX-006, OX-009, OX-013 |
| OX-015 | Inspeção e controles na interface | M2 | OX-012 a OX-014 |
| OX-016 | Control Center básico | M2 | OX-008, OX-012 |
| OX-017 | Demonstração de uso real e instalação alpha | M2 | OX-011, OX-015, OX-016; avaliação ampliada e iterações de OX-D05 |

OX-D01 e OX-D02 entregaram a pesquisa consolidada. OX-D03/OX-D04 têm protótipo e verificação técnica disponíveis; o próximo trabalho é o piloto inicial de OX-D05 e suas iterações; então OX-001/OX-002 retomam a viabilidade dos runtimes, aproveitando o harness antecipado. Após OX-004, persistência, Git e política/contexto podem avançar em paralelo. A integração final do worker depende da convergência desses contratos. Protótipos de design podem usar dados simulados; a avaliação externa de OX-D05 continua antes da conclusão do alpha. Os resultados orientam os ADRs propostos no plano.

## Pesquisa e design antes do aplicativo

### OX-D01 Pesquisa comparativa de soluções

Aprofundar o [levantamento inicial](./PRODUCT_AND_UX_RESEARCH.md), comparando produtos próximos e contratos integráveis. Investigar público/problema, fluxo, agentes/contas, modelo/thinking, contexto, worktrees, políticas, revisão, integração, recuperação, manutenção e licença. Priorizar fontes oficiais e separar documentação, demonstração, teste próprio e hipótese.

**Aceite consolidado em 08/10/2026:** matriz com pelo menos seis soluções relevantes, fontes/data e lacunas explícitas; mapear abrir projeto, iniciar trabalho, acompanhar execução, resolver atenção e revisar/aplicar em pelo menos três referências, com demonstrações efetivamente examinadas em pelo menos três delas. Identificar por passo o observado, o apenas documentado e o não verificado; registrar tempos/fontes e encaminhar lacunas à decisão/ticket dependente. Registro de adotar/adaptar/descartar orienta escopo e spikes; não inferir multiaccount de multissessão.

**Concluído como pesquisa.** Nove famílias, demonstrações Conductor/Cline/Superset, matriz de proveniência e critérios para D1. O requisito anterior, interpretado como quinze jornadas completas por teste/demo, **não foi cumprido integralmente**: foi substituído pelo método acima, com justificativa e cobertura em [D0 — conclusão](./research/d0-conclusion.md). Os gates técnicos e de piloto não foram reduzidos.

**Complemento documental de OX-D01 em 09/10:** [concorrentes e implicações](./research/competitive-update-2026-10-09.md). Jackalope/Cobalt ampliam o levantamento; Superset tem evidência de multiaccount registrada. CMP-01 a CMP-05 e o protocolo futuro orientam OX-D05/OX-017 e hipóteses M3/M4, sem alterar aceites ou criar requisito de testar todos os concorrentes antes de retomar M0.

### OX-D02 Benchmark UX e visual

Comparar pelo menos três referências de interação/visual e as interfaces dos similares. Anotar hierarquia, densidade, navegação, feedback, leitura de código/diff, configuração, atenção, teclado e adaptação. Avaliar telas reais e estados, além de apresentações de marketing.

**Aceite:** referências anotadas e vinculadas a necessidades do Orchestrix; decisões sobre foco, painéis e atenção têm justificativa. O resultado propõe critérios de qualidade e identidade, sem copiar marca/layout nem fixar uma estética apenas por tema ou biblioteca de componentes.

**Concluído como benchmark de pesquisa.** [Referências visuais e critérios](./research/experience-decisions.md), [controle/contexto](./research/workflow-risk-patterns.md), observações de mídias e [prioridades consolidadas](./research/d0-conclusion.md) orientam D1. A usabilidade do Orchestrix ainda depende de OX-D05.

### OX-D03 Jornadas e arquitetura de informação

Desenhar o fluxo de desenvolvimento assistido dentro do app: projeto → trabalho → execução → código/diff → revisão/correção → aplicação. Organizar navegação, conteúdo, ações e divulgação progressiva. Explorar o lugar da edição leve com indentação e limites de escrita concorrente, definindo se entra no alpha ou em incremento posterior.

**Aceite:** wireframes navegáveis cobrem sucesso e falhas essenciais, com linguagem alinhada aos estados do Core. O fluxo assistido não exige editor externo nem interpretação de logs brutos; DAG e políticas avançadas permanecem acessíveis. Estados, seleção e relação entre tarefa/artefato ficam explícitos.

**Progresso OX-D03:** [jornadas navegáveis e verificação técnica](./design/d1-delivery.md) disponíveis. O [piloto](./design/d1-pilot.md) valida linguagem/compreensão; edição leve é hipótese de incremento após avaliação. Ticket sujeito ao aceite de experiência.

### OX-D04 Protótipo e identidade visual

Explorar duas ou três direções visuais nas mesmas jornadas, escolher uma com o responsável pelo produto e criar protótipo de alta fidelidade. Definir tokens e componentes para tipografia, espaço, superfícies, bordas, ícones, temas, foco, seleção, status e movimento. Projetar layouts compactos/amplos e painéis recolhíveis/redimensionáveis.

**Direção escolhida e reafirmada em 10/10:** manter a [pasta de padronização `design-system/`](../design-system/README.md) como referência das próximas telas. Navy/índigo, hierarquia e logos transparentes são compartilhados pelo site e Studio; o favicon usa símbolo sem fundo. Voo noturno, montanhas e movimento responsivo pertencem ao site; o app conserva superfícies calmas, sem paisagem/partículas, entrada pelo chat e Studio progressivo. Studio é o padrão, com Atelier, Horizon, Deep Black, Medieval, Forest, Dawn e Institutional como alternativas. A aprovação da aparência não substitui OX-D05.

Manter o [catálogo textual de ativos visuais](./design/visual-asset-catalog.md), incluindo imagens em uso e conceitos disponíveis: descrição/aliases para busca, caminho, finalidade, origem/prompt e estado. Novos assets e variantes precisam de registro antes do uso. Organizar marca, fundos, ícones e experimentos sem confundir screenshots de QA com arte de produto; preservar as fontes e justificar promoção de um conceito para uso na interface.

Aplicar o [contrato de idiomas](./design/interface-languages.md): site em inglês; interface do app em English por padrão, com Português e Español selecionáveis e persistentes. Avaliar texto/nome acessível, reflow e troca sem perder rascunhos ou alterar conteúdo/contratos. OX-012/OX-016 levam a preferência global do estudo ao armazenamento de configurações do Desktop; idioma da interface e instruções de idioma aos agentes continuam separados.

Aplicar o [contrato de diagnósticos](./design/connection-diagnostics.md) ao cadastro e Settings: rodada automática, progresso por verificação, detalhes por recurso e recheck no mesmo escopo. A linguagem distingue suporte, configuração, evidência e resultado; uma capability não suportada não aparece como probe falho. Runtime sem adapter autenticado e telemetria ausente conservam os bloqueios/dados desconhecidos. Esta revisão de tela e lógica continua sujeita ao piloto de OX-D05.

**Aceite:** protótipo navegável de onboarding, workspace, revisão e configuração, com estados vazio, carregando, bloqueado, falha, conflito e reconexão. Temas, ações e conteúdo são coerentes; componentes têm regras de adaptação e teclado. Dados simulados não sugerem recursos de runtime inexistentes.

**Progresso OX-D04:** Studio padrão e sete alternativas, [tokens/componentes](./design/d1-design-system.md), painéis, teclado, texto ampliado, retorno de foco e três idiomas implementados no estudo. O incremento por [projetos/conversas](./design/d1-chat-direction.md) conserva entrada guiada e acrescenta avulsa; a entrada mobile foi refinada conforme o registro ao final. A suíte anterior à revisão de sessões passou **77/77 em uma rodada única em 10/10**; a revisão atual passou **101/101**, conforme o status. Os 36/36 de 09/10 e os 73 casos por cobertura consolidada da primeira revisão de idiomas permanecem históricos. O catálogo de **1.012 chaves** e `npm.cmd run check` passaram. Os [resultados e limites](./DEVELOPMENT_STATUS.md) preservam verificações de cores forçadas/movimento reduzido, Unicode, isolamento e bloqueio de aplicação desatualizada. Conforto, zoom/escala real e acessibilidade completa permanecem pendentes.

### OX-D05 Avaliação e iteração de experiência

Avaliar iniciar trabalho, entender bloqueio, localizar evidência, pedir correção, ajustar preferência e aprovar o artefato correto. Fazer piloto com o responsável e buscar três a cinco desenvolvedores externos, registrando tarefas, ajuda, erros e desconforto. Iterar e documentar o que mudou.

**Gate piloto para retomar M0 e liberar depois o Desktop de M2:** direção escolhida, piloto registrado e nenhum problema crítico aberto de entendimento, controle ou aprovação do resultado errado. Se o piloto tiver só o responsável, manter avaliação externa identificada como pendente. **Gate de conclusão do alpha:** avaliação ampliada com desenvolvedores externos registrada durante M2 e problemas críticos corrigidos antes de concluir OX-017. Resultado de amostra pequena não vira prova estatística de usabilidade.

**Progresso OX-D05:** [roteiro, registro e manifesto](./design/d1-pilot.md) prontos; ajuda direta e feedback qualitativo do responsável registrados. Iteração por projetos/conversas implementada e verificada; identidade, entrada progressiva e idiomas receberam aprovação qualitativa. Execução/conclusão das tarefas do piloto ainda não confirmadas. Gate aberto; aprovação estética e testes automatizados não substituem esse percurso.

## 2 Tickets para validar os runtimes

### OX-001 Harness Codex

Comparar CLI estruturada e app-server em um repositório descartável. Registrar versão, modo de autenticação, transporte, eventos, seleção de modelo/raciocínio, cwd, retomada e superfície de aprovação. Incluir a modalidade oficial de uso do plano ChatGPT com OAuth próprio como candidato separado da autenticação gerida pelo runtime, conforme [INT-10 e pesquisa de contas](./research/subscription-account-ux.md). Não fazer o harness depender de configuração pessoal oculta.

Validar as leituras oficiais para os diagnósticos: catálogo paginado, esforços aceitos, service tiers, multi-agent nativo, configuração sanitizada e permissões no escopo da identidade selecionada. Não criar thread/turno para uma leitura de metadata. Registrar versão e distinguir recurso suportado, configuração carregada e execução observada; uma seleção Ultra não substitui essa evidência. Probes ativos futuros exigem limites e escopo próprios, sem integrar inferência ao cadastro automático.

**Progresso:** [harness experimental](../tools/runtime-harness/README.md) disponível; assinatura ChatGPT, turno trivial e interrupção confirmada validados em `0.162.0-alpha.2`. [Resultado e matriz provisória](./research/runtime-harness-results.md). A autenticação existente é dependência explícita; ambiente/endpoints/extensões são limitados por overrides e configuração resolvida verificada. CLI comparativa, retomada, código em worktree e árvore de processos ainda pendentes; ticket aberto.

**Aceite:** executar tarefa trivial, capturar evento normalizado, classificar exit/error e interromper sem deixar subprocesso identificado ativo. Gravar fixtures sem segredos e registrar capacidades `supported`, `unsupported`, `unknown` ou `runtime-managed`. Decidir qual superfície atende ao primeiro adapter e por quê.

**Preparação independente de OX-001 em 09/10:** [contenção de Job Object Windows](./research/windows-process-containment.md), com oito testes nativos aprovados e 48 anteriores retestados. O helper controla somente a fixture própria; integrar supervisor/protocolo e demonstrar runtime real permanecem pendentes. Esse resultado não substitui o gate de D1 nem conclui OX-001.

### OX-002 Spikes de provedores e contas

Executar contrato equivalente em Claude Code e Antigravity. Investigar como duas conexões próprias podem selecionar autenticação independente, sem alterar o login global de processos existentes. Distinguir CLI local, SDK e distribuição futura do aplicativo.

Documentar quais leituras seguras de recursos e configuração cada adapter permite e quais permanecem desconhecidas. Aplicar a matriz específica dos [diagnósticos](./design/connection-diagnostics.md), sem confundir catálogo API com recursos de assinatura. Para backends API, preservar deployments/permissões ARM no Azure, região/autenticação/inference profiles no Bedrock e a distinção Gemini/Antigravity. Uma ausência de leitura ou autorização não equivale a suporte negado pelo modelo.

**Aceite:** produzir matriz por versão com fonte oficial, evidência local e limitações. Registrar também o caminho de autenticação permitido para uso pessoal/local e distribuição pública/comercial, incluindo eventuais requisitos de aprovação. Sessões existentes mantêm a identidade original. Se múltiplas contas não forem demonstradas, marcar esse recurso como desconhecido ou não suportado naquele adapter. Falha neste spike não bloqueia um worker com outro runtime validado.

### OX-003 Contrato e runtime fake

**Incremento de leitura:** o [pacote provider-discovery e a bridge do wizard](./design/resource-discovery-increment.md) já consultam catálogos API ao salvar, com paginação/partial, capacidades e origem, segurança e contratos de teste. Levar este contrato ao domínio de produção, ao primeiro adapter de OX-007 e ao Control Center de OX-016. Login de runtime, cofre, AWS signer e adaptação Vertex não estão encerrados; preservar os critérios e a dependência do runtime aprovado.

O [incremento de diagnósticos](./design/connection-diagnostics.md) acrescenta um relatório versionado com identidade/revisão, rodada, checks, features, recursos, proveniência e política. Preservar eixos distintos para estado da verificação, suporte, disponibilidade e configuração. Fixtures devem cobrir capability explicitamente não suportada, campo ausente, operação não aplicável, leitura falha/parcial, identidade não vinculada, recheck e resposta atrasada de uma revisão anterior. Nenhuma fixture vira autenticação real.

Definir `RuntimeDescriptor`, capabilities por conexão/modelo, configuração solicitada/efetiva, eventos, erros e término. Operações opcionais, como retomada e telemetria de cota, precisam expressar ausência de suporte.

**Preparação:** 11 cenários do fake app-server e **58 testes do contrato experimental** estão no harness preparatório, incluindo [admissão/interrupção, observação de descendente e limites de saída/fila](./research/offline-lifecycle-validation.md). A rodada de 10/10 aprovou os 48 testes anteriores e dez novos com uma fixture própria que pode deixar de ler stdin. Limites padrão de 1 MiB por frame e 2 MiB na fila Writable não certificam memória global. O watchdog do helper tem prova sintética separada em nove casos nativos; a integração ao transporte/runtime continua pendente. Não são o contrato Rust final nem dispensam a dependência do primeiro runtime aprovado ou o gate humano D1. Eventos de perda de sinal não viram sucesso/falha terminal inventados.

**Aceite:** fixtures e fake runtime reproduzem sucesso, evento inválido, autenticação inválida, rate limit, interrupção, timeout e crash. O Core recebe o mesmo contrato sem importar eventos específicos de cada provedor. Campos desconhecidos compatíveis não derrubam o parser; incompatibilidades são diagnosticadas.

## 3 Tickets para o worker confiável

### OX-004 Domínio e estados

Implementar Project, Repository, Connection, CapacityGroup, Run, Task, TaskAttempt e referências de sessão. Separar conclusão de tentativa, aceitação de artefato e integração. Definir transições e condições de dependência antes de ligar o scheduler.

Incluir definições e versões dos [modos de ação e perfis executores](./design/action-modes-and-worker-profiles.md), reutilizando AgentProfile e distinguindo estratégia de recursos, finalidade do modo e comportamento do papel. Alterar uma definição não reescreve tentativas existentes.

Incluir projetos com várias conversas e conversas avulsas conforme a direção de D1: identidade/ordem próprias, relação opcional com projeto e referências ao trabalho e versão. Associar uma conversa a um projeto preserva identidade/histórico, sem mudar snapshots de tentativas anteriores. A conversa da interface não funde sessões nativas ou contas; mensagem, comando de execução e confirmação de aplicação conservam papéis distintos.

**Aceite:** transição inválida falha; nova tentativa preserva a anterior; duas conexões usam o mesmo adapter sem compartilhar identidade de sessão; tarefas de código e análise possuem critérios explícitos de conclusão. V1 aceita um repositório por Project sem igualar suas identidades.

### OX-005 Persistência e eventos

Escolher acesso SQLite com spike pequeno. Criar migrações, estado atual, eventos append-only e transações para comando, estado e evidência. Reservar execução com chave de idempotência e identidade da tentativa.

Persistir modos locais, perfis personalizados e suas versões. Os snapshots das tentativas identificam modo, perfil e política efetiva; edição e restauração de padrões conservam a proveniência do trabalho anterior.

Persistir projetos, suas várias conversas, conversas avulsas, pedidos/orientações e referências com idempotência, sem duplicar transcripts internos dos providers. Recuperar seleção, histórico e relação com tentativas ao reconectar a interface. Associar uma conversa a um projeto não duplica mensagens nem recria seus Runs. O estudo D1 recupera sessões/rascunhos no armazenamento do navegador com limites; não demonstra a persistência de produção, idempotência ou reconexão de runtimes.

**Aceite:** reinício recarrega dados; reaplicar comando não duplica tentativa; falha de transação não deixa estado sem evento; migração é validada sobre banco existente de teste. Eventos têm sequência e versão de schema. Outputs são sanitizados antes de persistir; fixtures com segredos fictícios conhecidos comprovam redaction nos caminhos suportados.

### OX-006 Worktrees e artefatos

Criar worktree e branch por tentativa a partir de base conhecida. Persistir origem, paths e diffs; capturar artefatos completos, incluindo arquivos novos relevantes. Resolver paths com validação explícita antes de criação e limpeza.

**Aceite:** duas tentativas não alteram a árvore principal; alterações preexistentes são preservadas; crash mantém worktree recuperável; branch/path existente produz diagnóstico. Limpeza só remove recursos comprovadamente pertencentes à tentativa e nunca descarta mudanças sem política explícita.

### OX-007 Supervisor e adapter

Implementar o primeiro adapter validado em OX-001 ou OX-002 e supervisão de stdin/stdout/stderr, processo, timeout e cancelamento. Selecionar a conexão explicitamente, sanitizar ambiente, aplicar redaction aos outputs antes de gravação e validar modo de cobrança. Testar árvore de processos no Windows.

Integrar leituras de diagnóstico sem iniciar trabalho de modelo, hooks ou ferramentas para completar metadata. Aplicar e verificar por tentativa a política de delegação nativa aninhada desativada por padrão enquanto o Orchestrix coordena workers; não alterar o login/configuração pessoal do runtime. Se uma restrição obrigatória não puder ser aplicada com mecanismo validado, bloquear aquela combinação em vez de declarar contenção comprovada. Diagnóstico de Full access não concede permissões.

**Aceite:** saída extensa não bloqueia leitura; crash, auth failure e rate limit têm classificações distintas; cancelamento gracioso escala para encerramento; nenhuma opção de API é ativada como fallback silencioso. Execução incerta permanece identificável para reconciliação.

### OX-008 Política e contexto

Implementar schema inicial versionado, precedência, limites obrigatórios e snapshot efetivo. Construir Context Pack por seleção explícita, com critérios de aceite, instruções pertinentes, fontes e manifesto. Começar com elegibilidade e preferências ordenadas.

Resolver requisitos e padrões de modo/perfil sem criar precedência implícita. Implementar regras determinísticas para **Efficiency, Performance, Balanced e Custom**, considerando tipo, complexidade, risco e importância, com explicação e overrides conforme a [direção de routing](./design/routing-strategies.md). Presets especializados usam a mesma resolução. Verificações mecânicas usam ferramentas determinísticas; não aguardar classificação sofisticada ou histórico para entregar a V1. Uma conta/um runtime/concorrência 1 é uma configuração obrigatória de aceite; cota não observável permanece desconhecida e Subscription Only impede fallback cobrado.

Consumir os diagnósticos atuais do escopo correto, mantendo reasoning, service tier, ferramentas, permissões e native multi-agent como decisões independentes. Registrar solicitado/efetivo e a evidência usada; cache ou capacidade agregada não libera todos os modelos. Uma futura opção de delegação nativa exige benefício, escopo e budgets de tokens/tempo/concorrência, além de cancelamento e responsabilidade definidos; permanece desativada por padrão, sem substituir o scheduler.

**Aceite:** mesmas entradas produzem a mesma resolução; override não viola restrição obrigatória; setting não suportado gera bloqueio ou fallback explícito. O pacote enviado é inspecionável, tem origem/versão e indica omissões. Modelos disponíveis dependem da conexão e versão observadas.

### OX-009 Verificação e execução completa

Adicionar evaluators para comandos configurados de teste, build e lint, com cwd, timeout, saída e exit code. Montar a CLI para tarefa manual com worktree, política, runtime e checks. Comandos descobertos no projeto precisam ser apresentados/validados conforme política antes de executar.

**Aceite:** um agente declarar sucesso não basta; teste falho impede aceitação; cancelar evaluator encerra processo; relatório identifica tentativa e commit/diff verificado. Ausência de teste é exibida e tratada pela política, sem resultado verde inventado.

### OX-010 API local e daemon

Definir commands, queries, snapshots, envelope de eventos e cursor sem tipos Tauri no domínio. Criar daemon com instância única, transporte restrito ao usuário e cliente CLI de diagnóstico. Registrar ADR de transporte e encerramento.

**Aceite:** cliente consulta e suspende novas admissões sem Desktop; segundo cliente não cria segundo scheduler; reconexão recupera snapshot/eventos com sequência consistente; política de fechar janela é distinta de encerrar daemon. A suspensão mínima impede novos trabalhos e preserva tentativas ativas; não confirma pausa de turno ou término de worker. Transporte rejeita acesso não autorizado no limite suportado pelo sistema operacional.

### OX-011 Recuperação

Reconciliar tentativas, worktrees, artefatos, locks e identidade de processos antes de retomar admissões. Testar interrupção do Core em pontos críticos, incluindo a janela entre spawn e registro de processo.

**Aceite:** reinício não duplica execução, não considera PID reutilizado como processo original e preserva trabalho. Tentativa perdida ou incerta gera evento e ação explícita. Recuperação das operações de integração e aplicação no destino será ampliada e validada em OX-014.

## 4 Tickets para o aplicativo utilizável

### OX-012 Desktop e projeto

Criar shell Tauri/React, seleção de repositório Git, recentes, conexão ao daemon e estados de disponibilidade. Oferecer entrada por projeto/fluxo guiado e por conversa avulsa, várias conversas por projeto e associação posterior preservando histórico. Adicionar cadastro de Connection, seleção de autenticação pelo fluxo oficial, verificação de identidade/disponibilidade e desconexão. Exibir suporte do runtime sem exigir criação de um projeto novo.

Portar o diagnóstico automático de cadastro e o recheck de Settings, com progresso, verificações por tipo/provedor, detalhe por modelo e próxima ação para falhas/bloqueios reais. Cancelar ou editar/desconectar invalida a rodada correspondente; reabrir não promove um resultado histórico a observação atual. Nenhum botão de diagnóstico concede Full access ou autoriza execução API. Avaliar essa compreensão no piloto de experiência.

Portar os tokens, assets, componentes, navegação e estados de OX-D04/OX-D05, consultando a [pasta de padronização](../design-system/README.md), para Tauri/React/TypeScript. A interface inicial usa Studio navy/índigo, logo transparente e chat como entrada, com detalhes técnicos progressivos e painéis adaptativos. Preservar os sete temas alternativos e levar a preferência global de idioma ao armazenamento do Desktop: English por padrão, Português e Español disponíveis, sem alterar conteúdo ou estado do trabalho. O estudo isolado em HTML/CSS/JavaScript permanece referência; não muda a stack nem autoriza antecipar o gate de D1.

**Aceite:** abrir projeto existente não altera seu conteúdo; usuário registra e verifica uma conexão sem expor credenciais; janela reconecta ao mesmo run; erro de Git/runtime tem ação clara; fechar segue a preferência de continuar execução ou suspender novas admissões pelo daemon, preservando tentativas ativas, e reabrir reconecta ao estado existente. Comparação com protótipo, teclado, temas e layout compacto/amplo passam pelo aceite de experiência. O build Windows é reproduzível.

### OX-013 Revisão e correção

Criar perfis implementador e revisor, sessão revisora independente, findings estruturados e tentativas de correção limitadas. Revisor recebe diff e evidências da versão em revisão. Alterações posteriores invalidam aceitação anterior até nova verificação.

**Aceite:** ciclo completo funciona com uma conta e um runtime; rejection volta para correção com histórico e base no snapshot/diff exato rejeitado; mudanças anteriores são preservadas e checks/review repetidos sobre o resultado corrigido. Orçamento esgotado exige intervenção; outra família de modelo só é obrigatória quando a política o exige. Findings apontam artefato/arquivo e evidência.

### OX-014 Integração e aprovação

Preparar integração serial em branch/worktree do run e aplicação final no destino autorizado do usuário. Fazer checks sobre o resultado combinado e aprovação sobre os artefatos e base exatos. Detectar mudanças na base e preservar a árvore de trabalho do usuário. Distinguir Task concluída internamente de Run aplicado ao destino.

**Aceite:** conflito ou check falho bloqueia integração; mudança da base exige revalidação; aprovação expirada/incompatível não autoriza outro diff. Artefatos integrados têm proveniência e liberam dependências segundo o contrato da tarefa. Run só aparece como entregue após aplicação final confirmada no destino; alterações locais incompatíveis geram bloqueio explícito. Crash entre efeito Git e persistência é reconciliado sem duplicação; nenhuma operação destrutiva é usada para descartar conflito.

### OX-015 Inspeção e controle

Entregar criação/edição de tarefa manual com objetivo, critérios, risco, contexto e checks, além de tentativas, sessões, timeline, arquivos alterados, diff e resultados. Expor suspensão/retomada mínima de novas admissões pelo daemon, cancelamento, retry e aprovação. A suspensão pertence a M2 e não exige o scheduler ampliado de M3; interromper uma tentativa continua sendo uma operação distinta. Oferecer abertura de arquivos no editor externo.

Manter o chat como fluxo inicial e revelar inspeção/Studio conforme a necessidade, usando os [padrões aprovados](../design-system/README.md). Detalhes técnicos e estados de atenção precisam ser acessíveis sem obrigar toda pessoa a começar por eles; superfícies de trabalho não recebem as montanhas ou partículas do site.

**Aceite:** usuário entende o bloqueio atual, configuração efetiva e resultado do trabalho sem abrir o banco ou ler transcript completo; pode interromper execução; UI reconectada não perde eventos nem duplica comandos. Atualizações preservam seleção/foco e versão em revisão; painéis não escondem a ação principal em janela compacta. Pedidos de correção sobre código/diff funcionam dentro do app.

### OX-016 Control Center inicial

Editar presets e preferências de conexão, modelo, reasoning, revisão, contexto e limites. Mostrar configuração solicitada/efetiva, validação e mudança de versão. Sem telemetria real, cota aparece como desconhecida.

Conservar os [diagnósticos de conexão](./design/connection-diagnostics.md), suas datas/origens e recheck em Accounts, com distinção entre suporte, disponibilidade, configuração e execução observada. Exibir Ultra/reasoning, velocidade, multi-agent nativo e permissões separadamente; não classificar dados desconhecidos como falha de uma conta. Leituras administrativas de uso/crédito e futuros probes ativos seguem autorizações/limites específicos, sem fallback cobrado sob Subscription Only.

Oferecer as quatro estratégias, presets especializados, catálogo dos seis modos nativos, criação de modos especializados locais e edição dos campos autorizados dos perfis executores. Edição parcial dos modos nativos conserva sua finalidade e contratos obrigatórios. Seleção simples fica próxima do chat; regras por escopo, overrides, explicações e restauração de padrões permanecem inspecionáveis sob demanda.

Aplicar divulgação progressiva e linguagem definidas no protótipo: preferências frequentes e presets primeiro, parâmetros avançados sob demanda.

Preservar a galeria de oito temas e a escolha persistente de idioma como configurações globais independentes de modo, perfil, projeto e runtime. Aparência e idioma ficam acessíveis sem configurar políticas do orquestrador; essa migração reaproveita OX-D04/OX-012 e o [contrato de idiomas](./design/interface-languages.md).

**Aceite:** exportar/importar preserva política sem segredos e valida referências de Connections; bindings importados exigem confirmação de identidade/remapeamento e permanecem bloqueados quando não resolvidos. Mudança afeta novas decisões e preserva snapshots antigos; reduzir concorrência drena admissões; preferência incompatível mostra fallback antes ou durante a decisão, conforme o contrato.

### OX-017 Alpha de uso real

Usar o Desktop para uma alteração pequena no próprio Orchestrix, documentar o fluxo e produzir instruções de instalação local. Manter CI com fake/fixtures e testes live opt-in.

Demonstrar também uso serial com uma conta, escolha de recursos justificável para tarefas simples e complexas, parâmetros não suportados com fallback/bloqueio explícito e uma consulta de status Git sem chamada de modelo. Registrar um fluxo real representativo por modo nativo, incluindo fontes em Research, arquivo exportado/revisado em Presentations e sugestões sem aplicação automática em Improvement Review. Catálogo no site não constitui entrega desses fluxos.

**Aceite:** abrir projeto → criar tarefa → implementar → verificar → revisar → corrigir → aprovar integração sem copiar prompts. Demonstrar também falha de teste, cancelamento e reinício. Registrar avaliação de uso ampliada, QA visual, teclado, temas e adaptação com dados variados; corrigir problemas críticos. Registrar limitações e medir tempo e intervenções, sem alegar ganho de produtividade antes de comparação.

## 5 Backlog seguinte

| Ordem | Capacidade | Critério principal |
| --- | --- | --- |
| 1 | Scheduler de DAG e locks exatos | Dependências e capacidade são reservadas atomicamente; conflitos semânticos não executam em paralelo. |
| 2 | Segundo adapter e múltiplas Connections | Conta/capacidade/sessão permanecem identificadas; autenticação isolada demonstrada. |
| 3 | Cooldown, grupos de cota e fallback | Respeitar capacidade compartilhada, permitir pausa e explicar disponibilidade desconhecida. |
| 4 | Planner com saída estruturada | Plano editável; validação de schema, ciclos, risco e limites antes de executar. |
| 5 | Replanejamento controlado | Nova revisão preserva tentativas e reconcilia tarefas ativas. |
| 6 | Memória e documentação | Conhecimento validado com fontes influencia Context Packs futuros. |
| 7 | Beta, benchmark e VS Code | Instalação externa funciona; extensão reutiliza API; comparação apresenta evidências. |

A suspensão mínima de novas admissões e a preferência de fechar a janela pertencem ao fluxo inicial de M2, com o comando de daemon definido em OX-010 e exposto em OX-012/OX-015. M3 amplia políticas e controle global para DAG, concorrência e escopos de projeto/conexão/grupo; não adia o controle mínimo de M2. Essas capacidades são trabalho planejado, sem implementação de daemon/scheduler demonstrada pelo protótipo.

O próximo trabalho concreto é concluir a validação da revisão de interface solicitada pelo responsável; o piloto foi adiado até sua aprovação. Depois executar e registrar o piloto inicial da [entrega D1](./design/d1-delivery.md) por projetos/conversas, corrigir problemas críticos e retestar os casos afetados. Entrada guiada, chat, contexto, Studio/componentes, painéis, foco e idiomas foram implementados no estudo; **77/77 cenários passaram em uma rodada única em 10/10**, com o histórico de 73 casos preservado. A aparência aprovada permanece documentada na [pasta de padronização](../design-system/README.md). A [preparação offline](./research/offline-lifecycle-validation.md) e a [prova nativa Windows](./research/windows-process-containment.md) ampliam contratos e observação/contenção de processos próprios, sem retomar M0 ou fechar D1. A [conclusão de D0](./research/d0-conclusion.md) e o [handoff](./research/discovery-handoff.md) preservam evidência, hipóteses e lacunas. Depois OX-001 retoma worktree, sessões e supervisão integrada da árvore de runtime real no Windows; OX-001/OX-002 orientam o contrato final de OX-003. Não é necessário implementar todos os adapters para iniciar o worker confiável.

**Refinamento de OX-D04/OX-D05 em 10/10:** os utilitários expansíveis, ações de contexto lado a lado e introdução após o campo foram implementados em mobile. **Options** oferece idioma/aparência e mantém os controles disponíveis no desktop. Os 27 testes focados passaram, incluindo cursor, foco, teclado e troca de largura. `check:file` passou sete composições: desktop em inglês e 390×844/320×900 em EN/PT-BR/ES. O campo inteiro está na primeira tela; o botão Send pode exigir rolagem. Texto ampliado a 200% mantém reflow sem forçar todo o formulário na mesma tela. O [manifesto](./design/d1-build-manifest.json) e os [seis passos atuais](./design/d1-pilot.md#primeiro-percurso-atual--seis-passos-pelo-chat) ligam as observações à versão. O piloto humano ainda deve avaliar descoberta e conforto, com os demais P0 preservados.

**Preparação independente de OX-001/OX-003 em 10/10:** o [helper Windows](./research/windows-process-containment.md) recebeu watchdog em thread própria, armado antes de `ready`, com I/O fora do lock de handles e encerramento restrito ao Job próprio. A rodada final passou **9/9 casos nativos**; sob stdout bloqueado, a árvore e o helper terminaram antes do timeout de segurança da fixture, sem inventar `stopped`. Os **58/58 testes de contrato** anteriores permanecem evidência separada, sem nova execução nesta rodada. O próximo trabalho técnico após o gate D1 é integrar supervisão/transporte e provar o comportamento no runtime real; também permanecem limites de memória global e setup anterior ao watchdog. Não houve alteração dos milestones ou aceite de OX-001/003.


**Revisão de OX-D03/OX-D04/OX-D05 em 10/10 — substitui Options e seletores:** [sessões, painéis e Settings](./design/d1-shell-revision.md). Árvore de projetos/sessões, entrada avulsa vazia, criação dentro de projeto e vínculo posterior preservando contexto antigo; docking à esquerda/direita, abas e persistência visual, com blur durante escolha de lado; busca ampliada; ajuda externa com imagens; Settings flutuante com perfil/contas/uso observado, idioma, Themes e instruções pessoais. Portar o [contrato do shell](../design-system/workspace-shell.md) em OX-012/OX-015/OX-016 sem alterar Tauri/React/TypeScript ou acrescentar milestones. O responsável adiou o piloto até considerar a interface adequada; aprovação estética não fecha o gate. UI com linguagem de produto não significa runtime, autenticação ou telemetria implementados.


**Refinamento seguinte de OX-D03/OX-D04/OX-D05 em 10/10:** Sessions à esquerda, um New session principal, retirada do destino inferior, entrada Compact sem rolagem nos viewports de notebook definidos a 100%, Settings maior/centralizada com fechamento externo e texto em range 80–200%. Conexão escolhe Subscription/API antes do provider. A [pesquisa oficial de catálogos e capacidades](./research/api-provider-discovery-2026-10-10.md) e o [contrato de conexão](./design/api-connection-contract.md) complementam OX-002/OX-003/OX-007/OX-016: escopo de conta/endpoint, parâmetros nativos, tools/web/thinking observados, credenciais no host seguro e dados desconhecidos explícitos. Gemini API é distinto do runtime Antigravity; Azure e AWS usam os campos da superfície escolhida. O estudo valida o wizard/seam com testes; implementar adapters de rede, cofre e telemetria permanece nos tickets existentes, sem fechar gates.
