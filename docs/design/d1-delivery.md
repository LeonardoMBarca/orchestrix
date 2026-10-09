# D1 — protótipo preparado para o piloto

Entrega em **8 de outubro de 2026**, revisada em **9 de outubro de 2026**. **Implementação do estudo e verificação técnica disponíveis; piloto humano pendente. D1 permanece aberto.**

Abrir [o protótipo](http://127.0.0.1:4173) conforme as [instruções locais](../../prototypes/desktop/README.md). O [roteiro](d1-pilot.md) apresenta tarefas e registro em branco; a [matriz](d1-scenario-matrix.md) relaciona os 23 critérios de produto/contas aos casos D1 e às verificações posteriores. O [sistema visual](d1-design-system.md) registra tokens e componentes.

## O que foi entregue

| Frente | Interação disponível |
| --- | --- |
| Entrada | Projeto de exemplo, caminho com espaços/acentos, Windows nativo ou WSL explícito, uma conexão e confirmação simulada da modalidade de autorização. |
| Trabalho curto | Uma tarefa percorre início, resultado, evidências, revisão, correção, aceite interno e aplicação do Run usando uma conexão. |
| Feature | Quatro tarefas em R-08; pergunta em OX-26 e dependências de OX-27. Cada resultado complementar pode ser lido, corrigido e validado. A aplicação espera todas as tarefas e apresenta cinco arquivos. |
| Escolhas | Roteamento automático, conexão fixa, modelos nominais de exemplo e esforço/contexto preferidos. A escolha fixa incompatível bloqueia o início; mudanças passam a novas tentativas. Efetivo desconhecido continua explícito. |
| Contexto | Manifesto de implementação com fontes, versão, base, objetivo, critérios, decisões e instruções de correção. A preparação futura não muda o pacote atual. Manifesto da revisão identifica sessão separada e a tentativa produtora do artefato. |
| Conexões | Registros pendentes separados dos ativos; identidade divergente, catálogo em descoberta/incompatível, limite, expiração, acesso negado, reconexão e gerenciamento de uso ilustrativo. Desconexão bloqueia admissões; término, logout e revogação são confirmações distintas. |
| Atenção | Pergunta, permissão recusada/concedida, check falho, falha terminal, perda de sinal, cancelamento sem confirmação e reconciliação. Evento de outra tarefa preserva arquivo, campo, cursor e posição. |
| Revisão | Artefato/tentativa, evidências, pedido de correção e novas versões. Aceite da tarefa e aplicação do Run são passos distintos. Mudança do candidato ou da base invalida confirmação. |
| Aparência | Studio padrão e sete temas. Tipografia 14px, controles 44/36px, densidade, texto ampliado, modo de foco, fila recolhível e largura ajustável por ponteiro/teclado. Preferências visuais persistem no navegador. |

Capturas desta entrega: [entrada](d1-entry.png), [contexto](d1-context.png), [conexão](d1-connection.png), [Studio](../research/previews/studio.png) e [revisão](../research/previews/review.png). As imagens mostram fixtures, não execução de agentes.

## Verificação técnica

`npm.cmd run check` e `npm.cmd test` passaram no Windows com Node **24.19.0**, Playwright **1.64.0** e Chrome **154.0.8037.98** headless. A suíte completa de 09/10 tem **24 cenários: dez do incremento D1, nove anteriores e cinco de acessibilidade/regressão**, em [d1.spec.mjs](../../prototypes/desktop/tests/d1.spec.mjs), [workspace.spec.mjs](../../prototypes/desktop/tests/workspace.spec.mjs) e [d1-accessibility.spec.mjs](../../prototypes/desktop/tests/d1-accessibility.spec.mjs).

Após a suíte completa, o caso da feature foi retestado e passou para verificar que sua política ilustrativa incorpora a resposta de redirecionamento. Os cenários cobrem ciclo com uma conexão, feature completa, dependências, Run incompleto após revalidar base, contexto futuro/produtor, autorização/desconexão, limite/descoberta/incompatibilidade, permissões, cancelamento incerto, atenção sem perda de foco e confirmação vencida. Também verificam criação e aplicação independente de R-12, que tarefa/arquivo de outro Run não entram no candidato nem reabrem um Run aplicado, e que a aba Código conserva a identidade da tarefa.

O teste de layout inclui arraste do separador, teclado e persistência, seis larguras de 320 a 1920 pixels CSS, texto ampliado e entrada WSL. Os testes anteriores cobrem sete temas, contraste dos pares principais, foco, persistência, conteúdo tratado como texto e ausência de requisições externas no percurso verificado. A [verificação visual adicional](d1-design-system.md#verificação-executada) documenta 144 combinações de layout e 105 pares de contraste, com seus limites. Isso não certifica acessibilidade completa ou usabilidade.

Durante a auditoria foram corrigidos: aplicação após revalidar Run incompleto; execução de correção em conexão indisponível; mistura de tarefas entre Runs; arquivo complementar mostrado sob outra tarefa; e decisões/instruções ausentes do Context Pack. Em 09/10 foi corrigido o foco perdido ao fechar a inspeção automática da entrada ou adicionar uma conexão: o fechamento conserva um destino válido ou foca o conteúdo atual sem mover a rolagem. Esses problemas são de verificação técnica, não erros atribuídos a participantes.

A rodada de 09/10 ativou `prefers-reduced-motion` e `forced-colors` pelo Chrome/Playwright, conferindo media queries, estilos computados, foco, seleção, campos e radios. Verificou títulos/objetivos nos limites de 100/2000 caracteres, nomes de conexão/workspace de 70, projeto de 50 e paths Unicode Windows/WSL de 180, com controles compactos e viewport de 320px. A revisão exploratória adicional passou 27 checks; cinco regressões reproduzíveis integram a suíte. Eventos de atenção continuam preservando foco, cursor, arquivo e posição.

Identificador reproduzível do estudo revisado em 09/10: SHA-256 **`df71f8b616e1fffefbd30dc155656f0360d8ff888eeeffb0c2a747def012ed4a`**. Calculado concatenando, nessa ordem, nome + byte nulo + conteúdo de `app.js`, `connections.js`, `index.html`, `styles.css` e `workspace.js`. Registrar esse identificador e eventuais mudanças locais no manifesto da sessão. Os testes/documentos não integram esse hash. A versão anterior de 08/10 tinha hash `ef624384520be62b618064180f87698dec31709b5bac556160f9f50b65acf6da`.

## Situação e limites

**OX-D03:** jornadas navegáveis preparadas para avaliação. **OX-D04:** direção escolhida, componentes/tokens e adaptação implementados como candidato de design. **OX-D05:** roteiro pronto e piloto individual solicitado ao responsável; nenhum resultado humano foi registrado. A aceitação da experiência depende desse uso e das iterações que ele indicar.

Todos os agentes, contas, catálogos, Context Packs, códigos, checks e operações Git são simulados. Nova revisão da fixture não prova que um comentário foi implementado por um agente. O protótipo não autentica, lê pastas, envia contexto, mede cotas nem executa código. O seed tem Runs independentes; o template Feature agrupa quatro tarefas no mesmo Run. Tarefas manuais adicionais percorrem criação, resultado ilustrativo, correção, aceite e revisão/aplicação em seus próprios Runs, sem entrar no candidato de R-08.

Edição manual leve com indentação, comentários por linha e comparação aprofundada de tentativas ficam como decisões de produto após o piloto. O estudo permite leitura e correção assistida; não há test explorer, debugger ou execução integrada de testes. A suspensão mínima de novas admissões pelo daemon está planejada em M2; M3 amplia controle global, políticas, DAG e pooling. Esses controles reais não estão implementados neste estudo. O modal de plano expõe a sequência/dependências ilustrativas, sem planner autônomo.

Zoom físico do navegador, escalas Windows 125/150/200%, leitor de tela, logs/diffs extensos e avaliação externa ainda precisam de verificação. Ampliação da fonte a 200% e redução do viewport não substituem essas condições. Cores forçadas e movimento reduzido foram exercitados como media queries no Chrome; isso não certifica a configuração de contraste do Windows ou tecnologias assistivas.

## Gate para retomar M0

O gate de [OX-D05](../DEVELOPMENT_BACKLOG.md#ox-d05-avaliação-e-iteração-de-experiência) permanece: direção escolhida, piloto registrado e nenhum problema crítico aberto de entendimento, controle ou aprovação do resultado errado. Testes técnicos ou a escolha anterior do Studio não substituem o piloto.

Próxima ação: executar e registrar o piloto individual, corrigir problemas críticos e retestar os casos afetados. Depois retomar M0. A avaliação com três a cinco desenvolvedores externos continua prevista antes do alpha em M2.
