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

**Aceite:** protótipo navegável de onboarding, workspace, revisão e configuração, com estados vazio, carregando, bloqueado, falha, conflito e reconexão. Temas, ações e conteúdo são coerentes; componentes têm regras de adaptação e teclado. Dados simulados não sugerem recursos de runtime inexistentes.

**Progresso OX-D04:** Studio padrão, sete temas, [tokens/componentes](./design/d1-design-system.md), painéis, teclado, texto ampliado e retorno de foco implementados no estudo. 24 cenários técnicos passaram em 09/10, incluindo media queries de cores forçadas/movimento reduzido e limites Unicode; conforto, zoom/escala real e acessibilidade completa permanecem pendentes.

### OX-D05 Avaliação e iteração de experiência

Avaliar iniciar trabalho, entender bloqueio, localizar evidência, pedir correção, ajustar preferência e aprovar o artefato correto. Fazer piloto com o responsável e buscar três a cinco desenvolvedores externos, registrando tarefas, ajuda, erros e desconforto. Iterar e documentar o que mudou.

**Gate piloto para retomar M0 e liberar depois o Desktop de M2:** direção escolhida, piloto registrado e nenhum problema crítico aberto de entendimento, controle ou aprovação do resultado errado. Se o piloto tiver só o responsável, manter avaliação externa identificada como pendente. **Gate de conclusão do alpha:** avaliação ampliada com desenvolvedores externos registrada durante M2 e problemas críticos corrigidos antes de concluir OX-017. Resultado de amostra pequena não vira prova estatística de usabilidade.

**Progresso OX-D05:** [roteiro, registro e manifesto](./design/d1-pilot.md) prontos; piloto individual solicitado ao responsável, ainda sem resultado humano. Gate aberto; não substituído por testes automatizados.

## 2 Tickets para validar os runtimes

### OX-001 Harness Codex

Comparar CLI estruturada e app-server em um repositório descartável. Registrar versão, modo de autenticação, transporte, eventos, seleção de modelo/raciocínio, cwd, retomada e superfície de aprovação. Incluir a modalidade oficial de uso do plano ChatGPT com OAuth próprio como candidato separado da autenticação gerida pelo runtime, conforme [INT-10 e pesquisa de contas](./research/subscription-account-ux.md). Não fazer o harness depender de configuração pessoal oculta.

**Progresso:** [harness experimental](../tools/runtime-harness/README.md) disponível; assinatura ChatGPT, turno trivial e interrupção confirmada validados em `0.162.0-alpha.2`. [Resultado e matriz provisória](./research/runtime-harness-results.md). A autenticação existente é dependência explícita; ambiente/endpoints/extensões são limitados por overrides e configuração resolvida verificada. CLI comparativa, retomada, código em worktree e árvore de processos ainda pendentes; ticket aberto.

**Aceite:** executar tarefa trivial, capturar evento normalizado, classificar exit/error e interromper sem deixar subprocesso identificado ativo. Gravar fixtures sem segredos e registrar capacidades `supported`, `unsupported`, `unknown` ou `runtime-managed`. Decidir qual superfície atende ao primeiro adapter e por quê.

### OX-002 Spikes de provedores e contas

Executar contrato equivalente em Claude Code e Antigravity. Investigar como duas conexões próprias podem selecionar autenticação independente, sem alterar o login global de processos existentes. Distinguir CLI local, SDK e distribuição futura do aplicativo.

**Aceite:** produzir matriz por versão com fonte oficial, evidência local e limitações. Registrar também o caminho de autenticação permitido para uso pessoal/local e distribuição pública/comercial, incluindo eventuais requisitos de aprovação. Sessões existentes mantêm a identidade original. Se múltiplas contas não forem demonstradas, marcar esse recurso como desconhecido ou não suportado naquele adapter. Falha neste spike não bloqueia um worker com outro runtime validado.

### OX-003 Contrato e runtime fake

Definir `RuntimeDescriptor`, capabilities por conexão/modelo, configuração solicitada/efetiva, eventos, erros e término. Operações opcionais, como retomada e telemetria de cota, precisam expressar ausência de suporte.

**Preparação:** 11 cenários fake e 48 testes do contrato experimental estão no harness preparatório, incluindo [admissão/interrupção e observação de descendente no Windows](./research/offline-lifecycle-validation.md). Não são o contrato Rust final nem dispensam a dependência do primeiro runtime aprovado ou o gate humano D1. Eventos de perda de sinal não viram sucesso/falha terminal inventados.

**Aceite:** fixtures e fake runtime reproduzem sucesso, evento inválido, autenticação inválida, rate limit, interrupção, timeout e crash. O Core recebe o mesmo contrato sem importar eventos específicos de cada provedor. Campos desconhecidos compatíveis não derrubam o parser; incompatibilidades são diagnosticadas.

## 3 Tickets para o worker confiável

### OX-004 Domínio e estados

Implementar Project, Repository, Connection, CapacityGroup, Run, Task, TaskAttempt e referências de sessão. Separar conclusão de tentativa, aceitação de artefato e integração. Definir transições e condições de dependência antes de ligar o scheduler.

**Aceite:** transição inválida falha; nova tentativa preserva a anterior; duas conexões usam o mesmo adapter sem compartilhar identidade de sessão; tarefas de código e análise possuem critérios explícitos de conclusão. V1 aceita um repositório por Project sem igualar suas identidades.

### OX-005 Persistência e eventos

Escolher acesso SQLite com spike pequeno. Criar migrações, estado atual, eventos append-only e transações para comando, estado e evidência. Reservar execução com chave de idempotência e identidade da tentativa.

**Aceite:** reinício recarrega dados; reaplicar comando não duplica tentativa; falha de transação não deixa estado sem evento; migração é validada sobre banco existente de teste. Eventos têm sequência e versão de schema. Outputs são sanitizados antes de persistir; fixtures com segredos fictícios conhecidos comprovam redaction nos caminhos suportados.

### OX-006 Worktrees e artefatos

Criar worktree e branch por tentativa a partir de base conhecida. Persistir origem, paths e diffs; capturar artefatos completos, incluindo arquivos novos relevantes. Resolver paths com validação explícita antes de criação e limpeza.

**Aceite:** duas tentativas não alteram a árvore principal; alterações preexistentes são preservadas; crash mantém worktree recuperável; branch/path existente produz diagnóstico. Limpeza só remove recursos comprovadamente pertencentes à tentativa e nunca descarta mudanças sem política explícita.

### OX-007 Supervisor e adapter

Implementar o primeiro adapter validado em OX-001 ou OX-002 e supervisão de stdin/stdout/stderr, processo, timeout e cancelamento. Selecionar a conexão explicitamente, sanitizar ambiente, aplicar redaction aos outputs antes de gravação e validar modo de cobrança. Testar árvore de processos no Windows.

**Aceite:** saída extensa não bloqueia leitura; crash, auth failure e rate limit têm classificações distintas; cancelamento gracioso escala para encerramento; nenhuma opção de API é ativada como fallback silencioso. Execução incerta permanece identificável para reconciliação.

### OX-008 Política e contexto

Implementar schema inicial versionado, precedência, limites obrigatórios e snapshot efetivo. Construir Context Pack por seleção explícita, com critérios de aceite, instruções pertinentes, fontes e manifesto. Começar com elegibilidade e preferências ordenadas.

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

Criar shell Tauri/React, seleção de repositório Git, recentes, conexão ao daemon e estados de disponibilidade. Adicionar cadastro de Connection, seleção de autenticação pelo fluxo oficial, verificação de identidade/disponibilidade e desconexão. Exibir suporte do runtime sem exigir criação de um projeto novo.

Usar os tokens, componentes, navegação e estados validados em OX-D04/OX-D05. A interface inicial já precisa da direção visual escolhida e adaptação de painéis.

**Aceite:** abrir projeto existente não altera seu conteúdo; usuário registra e verifica uma conexão sem expor credenciais; janela reconecta ao mesmo run; erro de Git/runtime tem ação clara; fechar segue a preferência de continuar execução ou suspender novas admissões pelo daemon, preservando tentativas ativas, e reabrir reconecta ao estado existente. Comparação com protótipo, teclado, temas e layout compacto/amplo passam pelo aceite de experiência. O build Windows é reproduzível.

### OX-013 Revisão e correção

Criar perfis implementador e revisor, sessão revisora independente, findings estruturados e tentativas de correção limitadas. Revisor recebe diff e evidências da versão em revisão. Alterações posteriores invalidam aceitação anterior até nova verificação.

**Aceite:** ciclo completo funciona com uma conta e um runtime; rejection volta para correção com histórico e base no snapshot/diff exato rejeitado; mudanças anteriores são preservadas e checks/review repetidos sobre o resultado corrigido. Orçamento esgotado exige intervenção; outra família de modelo só é obrigatória quando a política o exige. Findings apontam artefato/arquivo e evidência.

### OX-014 Integração e aprovação

Preparar integração serial em branch/worktree do run e aplicação final no destino autorizado do usuário. Fazer checks sobre o resultado combinado e aprovação sobre os artefatos e base exatos. Detectar mudanças na base e preservar a árvore de trabalho do usuário. Distinguir Task concluída internamente de Run aplicado ao destino.

**Aceite:** conflito ou check falho bloqueia integração; mudança da base exige revalidação; aprovação expirada/incompatível não autoriza outro diff. Artefatos integrados têm proveniência e liberam dependências segundo o contrato da tarefa. Run só aparece como entregue após aplicação final confirmada no destino; alterações locais incompatíveis geram bloqueio explícito. Crash entre efeito Git e persistência é reconciliado sem duplicação; nenhuma operação destrutiva é usada para descartar conflito.

### OX-015 Inspeção e controle

Entregar criação/edição de tarefa manual com objetivo, critérios, risco, contexto e checks, além de tentativas, sessões, timeline, arquivos alterados, diff e resultados. Expor suspensão/retomada mínima de novas admissões pelo daemon, cancelamento, retry e aprovação. A suspensão pertence a M2 e não exige o scheduler ampliado de M3; interromper uma tentativa continua sendo uma operação distinta. Oferecer abertura de arquivos no editor externo.

**Aceite:** usuário entende o bloqueio atual, configuração efetiva e resultado do trabalho sem abrir o banco ou ler transcript completo; pode interromper execução; UI reconectada não perde eventos nem duplica comandos. Atualizações preservam seleção/foco e versão em revisão; painéis não escondem a ação principal em janela compacta. Pedidos de correção sobre código/diff funcionam dentro do app.

### OX-016 Control Center inicial

Editar presets e preferências de conexão, modelo, reasoning, revisão, contexto e limites. Mostrar configuração solicitada/efetiva, validação e mudança de versão. Sem telemetria real, cota aparece como desconhecida.

Aplicar divulgação progressiva e linguagem definidas no protótipo: preferências frequentes e presets primeiro, parâmetros avançados sob demanda.

**Aceite:** exportar/importar preserva política sem segredos e valida referências de Connections; bindings importados exigem confirmação de identidade/remapeamento e permanecem bloqueados quando não resolvidos. Mudança afeta novas decisões e preserva snapshots antigos; reduzir concorrência drena admissões; preferência incompatível mostra fallback antes ou durante a decisão, conforme o contrato.

### OX-017 Alpha de uso real

Usar o Desktop para uma alteração pequena no próprio Orchestrix, documentar o fluxo e produzir instruções de instalação local. Manter CI com fake/fixtures e testes live opt-in.

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

O próximo trabalho concreto é executar e registrar o piloto inicial da [entrega D1](./design/d1-delivery.md), corrigir problemas críticos e retestar os casos afetados. Jornadas/contexto/onboarding, Studio/componentes, painéis e foco foram implementados no estudo; 24 cenários técnicos passaram em 09/10. A [preparação offline](./research/offline-lifecycle-validation.md) ampliou contratos e observação de processos, sem retomar M0 ou fechar D1. A [conclusão de D0](./research/d0-conclusion.md) e o [handoff](./research/discovery-handoff.md) preservam evidência, hipóteses e lacunas. Depois OX-001 retoma worktree, sessões e contenção/término da árvore no Windows; OX-001/OX-002 orientam o contrato final de OX-003. Não é necessário implementar todos os adapters para iniciar o worker confiável.
