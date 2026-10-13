# OX-003 — integração Codex ao contrato 1.x

Data: **13/10/2026**. Base: branch `docs/development-plan`, `f9ff432`. D1 e a escolha local do app-server permanecem liberados. Este incremento integra a fronteira experimental; não executa novos turnos nem conclui o Core Rust/M0.

## Caminho executável

```text
JsonRpcProcess → CodexSession → CodexRuntimeAdapter
                                      ↓
                        RuntimeContractObserver (contrato 1.1)
                                      ↓
                        snapshots/eventos provider-neutral
```

O [adapter](../../tools/runtime-harness/src/codex-contract.mjs) é usado por [probe](../../tools/runtime-harness/probe.mjs) e [worktree-probe](../../tools/runtime-harness/worktree-probe.mjs), não apenas por uma função demonstrativa. O relatório ganha `normalized`, com descritor, tentativas e diagnósticos. O harness de worktree conserva grupos por attachment/processo e mantém suas verificações de diff, checks e checkout principal. O percurso live dessa nova instrumentação **não foi executado**.

`CodexIdentityMap` conserva, em memória privada, os vínculos nativos de modelo, thread, turno e conta observável. O contrato recebe referências locais de conexão/revisão, sessão, tentativa e turno. Reiniciar/retomar conserva a sessão e os modelos locais, criando outra tentativa/turno; compartilhar o mapa com outra revisão ou identidade conhecida é recusado. Esse mapa não é persistência durável nem prova de contas independentes.

O consumidor importa somente o contrato, sem tipos ou mensagens Codex. O mesmo observador reconsome o fake provider-neutral, os eventos produzidos pelo adapter conectado à fixture JSON-RPC e os eventos do replay. Configuração resolvida é evidência de configuração, sem alegar telemetria da execução. Ausência não é preenchida com o pedido.

## Ajuste mínimo versionado

O [contrato 1.1](../contracts/runtime-contract-v1.md#additive-v11--factual-adapter-observations) acrescenta observações factuais de configuração, admissão, início, terminal, resultado de interrupção, bytes e streams. As regras estritas 1.0 continuam testadas.

- Um terminal explícito pode existir sem início observado, inclusive antes do ACK. O buffering nativo conserva ownership antes de encaminhar notificações; a sequência normalizada é local, não um timestamp da ordem física no transporte.
- O terminal de interrupção pode chegar antes do ACK. Relatórios antigos guardam o ACK sem registrar o envio; `requested=false` e `acknowledged=true` conservam essa lacuna, sem sintetizar um request.
- A saída da raiz pode anteceder o fechamento dos streams. Observações posteriores acumulam fechamento/fault sem apagar uma falha anterior.
- Erro fatal de protocolo/identidade/budget bloqueia eventos do runtime, mantendo cleanup independente dentro dos budgets. Terminal previamente confirmado é preservado.

Registrar um ACK real não executa autorização/preflight. Os gates de assinatura, disponibilidade, endpoint e configuração continuam no host/integração nativa. Subscription Only não admite API/fallback pago. Encerrar o Job nunca cria terminal remoto. Raiz, helper, streams e árvore têm evidências separadas; somente ownership e contagem do Job próprios permitem confirmar a árvore.

## Descoberta e capacidades

`CodexSession.discover()` agora pagina `model/list`, com limites de **100 registros/página, 10 páginas, 256 modelos e 15 s para o catálogo**, RPCs de até 5 s. Cursor ausente, repetido/inválido, falha posterior, cutoff e registros conflitantes produzem cobertura unknown/partial explícita. A primeira leitura inválida continua bloqueando a descoberta. O escopo é o catálogo visível (`includeHidden=false`), não todos os modelos do fornecedor.

Cada modelo conserva somente metadata explicitamente presente: esforços, completude/suporte de reasoning, service tiers, tier padrão, versão de multi-agent e modalidades de entrada. Esses campos nativos são leitura de catálogo; tiers/delegação/ferramentas/cota não ganham suporte normalizado ou autorização de execução por associação.

O descritor portátil limita a 64 modelos e marca truncamento como parcial. Disponibilidade por modelo permanece `unknown`; catálogo não é entitlement. Reasoning ausente/parcial mantém suporte/níveis desconhecidos, distinguindo uma lista explicitamente vazia. O mapeamento atual aceita somente nomes neutros idênticos (`minimal`, `low`, `medium`, `high`, `maximum`); nomes nativos não mapeados, como `xhigh`, não viram `maximum`. Listas sem tradução integral ficam `null`, e dispatch manual de nível não mapeado é recusado antes do RPC. Não se faz união de capacidades entre modelos.

A paginação e essa projeção foram verificadas com fixtures offline, **sem nova consulta autenticada de catálogo**. Diagnóstico completo e OAuth próprio/INT-10 permanecem em OX-001, sem virar pré-requisitos adicionais para o primeiro contrato single-account.

## Replay e passagem para Rust

[codex-replay.mjs](../../tools/runtime-harness/codex-replay.mjs) lê dois relatórios sanitizados existentes: [Job live](evidence/codex-m0-job-live.json) e [MXC live](evidence/codex-m0-mxc-live.json). A [fixture JSON portátil](../../tools/runtime-harness/fixtures/codex-contract-replay.json) contém três casos sintéticos adicionais, entradas e projeções esperadas, para futura conformance Rust.

Os relatórios reais não guardam catálogo completo, ACK de admissão nem vínculo explícito de cada item de `processes[]` com uma tentativa. O replay conserva esses campos desconhecidos: quatro observações de processo no primeiro relatório e duas no segundo ficam **sem associação**, sem usar posição/contagem para inventar ownership. Evidências históricas de Job não são negadas; simplesmente não são atribuídas pelo tradutor sem vínculo suficiente. Casos sintéticos com associações explícitas verificam força (raiz 137/helper 0/árvore vazia, terminal remoto unknown) e streams parciais seguidos de fechamento.

Cada tentativa informa os campos de intenção presentes, defaults neutros não históricos e binding de conta não registrado. EOF só descarrega contadores de bytes pendentes, sem criar erro ou terminal. As saídas não retêm mensagens, comandos, caminhos, modelos/IDs nativos, cursores ou credenciais. Tags/códigos sintéticos de autenticação/rate limit testam tradução e sanitização; não certificam todos os formatos reais de erro do serviço.

Turnos são identificados por sessão + referência nativa, permitindo IDs iguais em sessões distintas sem misturar terminais. Permissões negadas, eventos desconhecidos e sinais isolados de interrupção/perda não criam tentativas fantasmas; ficam sem associação quando não há lifecycle/saída própria suficiente. Rejeição RPC confirma envio; rejeição local `not-written` continua `not-sent`. Iniciar um pedido de interrupção registra intenção, sem afirmar que seus bytes chegaram ao runtime.

O comando é offline, rejeita flags live/provider e grava artefato ignorado pelo Git, com hashes dos inputs:

```powershell
cd tools/runtime-harness
npm.cmd run replay:codex
```

Reproduzir esses dados demonstra sua tradução, sem comprovar nova autenticação, inferência, resume real ou contenção Windows. O domínio futuro continua sem dependência de Node: wire JSON, regras de transição, fixture e resultados esperados são reutilizáveis; o JavaScript é a especificação executável experimental.

## Verificação desta rodada

| Verificação executada | Resultado |
| --- | --- |
| `npm.cmd run check` e `check:codex-contract` | Passaram |
| `npm.cmd test` | **255/255**, 61,320 s, zero falhas/skips |
| `npm.cmd run test:windows-transport` | **21/21**, 48,653 s, zero falhas/skips |
| `npm.cmd run test:windows-job` | **9/9**, 33,301 s, zero falhas/skips |
| `node codex-replay.mjs --output artifacts/codex-contract-replay-current.json` | **5 casos**, zero dispatches |

Os 255 incluem os 162 anteriores e **93 novos casos**: 24 de adapter Codex, seis de harness integrado/capture/opt-in, 22 de replay, 22 de descoberta/projeção e 19 da revisão 1.1. A suíte do adapter inclui um Job Windows real contendo **runtime sintético**, sem fornecedor. As duas suítes Windows foram reexecutadas porque houve mudança no transporte JSON-RPC; usam somente fixtures próprias.

A revisão corrigiu roundtrip de `error:null`, propagação sanitizada de tags RPC, versão inválida do fake, projeção indevida de reasoning desconhecido, classificação de backpressure e separação entre falha de normalização e encerramento observado. O novo teste de dispatch local inicialmente detectou a perda de `not-written` no evento de erro de interrupção; a propagação foi corrigida, e as regressões focada e completa passaram. A API importável do probe agora valida modo e booleano live antes de criar processo; flags inválidas não podem contornar o opt-in. Nenhum deadline/critério foi ampliado ou removido.

## Aceite de OX-003 e próximo passo

**Continuação posterior à instalação autorizada, em base `31f15ad`:** o [Core Rust agora consome o contrato](rust-runtime-contract-increment.md), com corpus de 104 casos/309 operações e pipeline integrado offline. O bloqueio de toolchain foi removido. O checklist abaixo preserva o ponto de partida deste incremento JavaScript; o registro Rust identifica consumo entregue e paridade de timestamps legados ainda pendente, sem concluir OX-003/M0 ou repetir live.

| Entregável/critério vigente | Situação verificável |
| --- | --- |
| Contrato, descritor, configuração, eventos, erros e término normalizados | Implementados no experimento 1.1, com suporte/ausência/unknown explícitos |
| Fake: sucesso, evento inválido, auth, rate limit, interrupção, timeout e crash | Cobertos; não são provas dos provedores |
| Integração do primeiro runtime ao mesmo consumidor | Adapter/harness + fixture JSON-RPC + replay histórico implementados; nova instrumentação live não exercitada |
| Campos futuros, major incompatível, identidade/revisão, duplicatas e budgets | Regressões aprovadas, incluindo quarentena e cleanup |
| Core recebe o contrato sem importar tipos nativos | Demonstrado no consumidor JavaScript; **integração/conformance Rust pendente** |

**OX-003 permanece em andamento.** Próximo incremento: implementar o consumo/conformance da mesma fronteira no Core Rust usando as fixtures e resultados existentes. Nesta rodada, Cargo/rustc não foram encontrados no PATH nem em `%USERPROFILE%\.cargo\bin`; nenhuma toolchain foi instalada. Disponibilizar esse ambiente é a preparação necessária para executar a validação Rust. Não é necessário repetir os cinco turnos live para portar o consumidor.

OX-001/diagnóstico completo, OX-002, M0 e avaliação externa mantêm suas pendências. SQLite, scheduler, worker e Desktop de produção/OX-004 não foram iniciados. Shell/design-system, execução serial com uma assinatura, estratégias/presets e separação entre contexto do runtime e memória do Orchestrix permanecem preservados.
