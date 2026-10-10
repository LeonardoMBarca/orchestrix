# Preparação offline — turnos e observação de processos

Registro de **9 de outubro de 2026**, Windows nativo, Node **24.19.0**. Esta continuação atende ao pedido de prosseguir o plano com correções de D1 e trabalho preparatório independente. **O piloto humano continua pendente; M0 não foi retomado ou concluído.** Não houve execução de Codex/Claude/Antigravity, autenticação ou inferência nesta rodada.

Complementado em **10/10** com limites de saída e backpressure do cliente JSON-RPC, descritos ao final. A suíte offline atual passou **58/58**; os 48 casos da rodada original permanecem uma evidência histórica separada. Os gates e a ausência de execução live acima continuam válidos.

## Mudança concreta

O [harness](../../tools/runtime-harness/README.md) já tinha framing, correlação e testes de timeout, mas deixava duas lacunas de controle: uma nova chamada de início podia ser enviada depois de perder o ACK do primeiro turno; interrupção e acompanhamento simultâneos disputavam um único observador terminal. Também aguardava `close` para reconhecer término do pai, sem tratar o caso de um descendente mantendo os pipes abertos.

Agora a sessão bloqueia novas admissões enquanto um turno estiver ativo ou incerto. Uma rejeição RPC correlacionada, sem atividade contraditória, permite uma nova ação explícita; timeout de ACK não autoriza retry. Somente o terminal do turno comprovadamente próprio libera a admissão. Sem ACK/identidade recuperável, a sessão permanece bloqueada; recuperação persistida continua trabalho futuro.

Interrupção e acompanhamento podem observar o mesmo terminal com prazos independentes. Chamadas concorrentes de interrupção compartilham um RPC. ACK de interrupção não confirma término; perder esse ACK não apaga um terminal já recebido. Falha de um observador remove somente sua espera e conserva as demais.

## Processo pai, transporte e árvore

O cliente observa `exit` separadamente de `close`. Após `exit`, bloqueia novas requisições e concede até **250ms** para drenar respostas já em trânsito. Se o transporte não fechar, encerra seus próprios endpoints e registra observação desligada. Não envia kill a um pai já observado como encerrado. Isso segue a distinção documentada pelo Node: o pai pode terminar enquanto outros processos conservam os mesmos streams. [Node 24.19.0 — exit/close](https://nodejs.org/download/release/v24.19.0/docs/api/child_process.html#event-close)

| Campo | Evidência que representa |
| --- | --- |
| `parentExited` | Evento de saída do processo lançado. É falso quando o executável nem pôde ser iniciado. |
| `stdioClosed` | Fechamento natural dos streams observado antes do desligamento local. |
| `observationDetached` | O cliente desligou seus endpoints; não comprova saída de descendentes. |
| `treeTermination` | Continua `unknown` em todos os cenários deste supervisor. |

`stop()` compartilha uma operação entre chamadores. Dá prazo ao EOF e, se necessário, pede término somente ao pai ainda ativo. Ausência de confirmação de saída resulta em erro de término e estado desconhecido; a função não transforma pedido de kill em evidência terminal. Reentrância durante fechamento/dispatch não cria outro stop nem emite frames após a observação ser fechada.

## Evidência reproduzível

```powershell
cd tools/runtime-harness
npm.cmd run check
npm.cmd test
```

A suíte completa de **09/10** passou **48 testes**: 23 contratos anteriores, 19 de [ciclo de sessão](../../tools/runtime-harness/tests/session-lifecycle.test.mjs) e seis de [ciclo de processo](../../tools/runtime-harness/tests/process-lifecycle.test.mjs). São testes de contrato offline, sem benchmark de desempenho do runtime.

| Cenário exercitado | Resultado observado |
| --- | --- |
| Turno ativo, início ainda pendente e timeout do ACK | Segunda admissão bloqueada; nenhuma segunda chamada automática. |
| Terminal anterior/estrangeiro e ACK com aparência terminal | Não liberam o turno ativo; notificação terminal própria continua necessária. IDs de turno contraditórios no mesmo envelope falham com protocolo antes de registrar/liberar ownership; IDs coerentes são aceitos. |
| Interrupção com acompanhamento já pendente | Ambos recebem o terminal; ACK isolado não resolve nenhum como concluído. |
| Duas interrupções concorrentes | Um RPC; terminal e deadlines/limpeza preservados. |
| Timeout/falha de transporte | Resultado permanece desconhecido; nova admissão bloqueada. |
| Subprocesso fake real | Interrupção compartilhada e próximo turno após terminal confirmados offline. |
| Pai encerrado com descendente ativo e pipes herdados | Saída do pai observada; descendente ainda vivo; transporte desligado dentro do prazo; árvore `unknown`. |
| Resposta seguida de saída normal | Resposta consumida; streams naturalmente fechados; árvore ainda `unknown`. |
| Executável inexistente | Erro de lançamento; não declara saída de um pai inexistente. |
| Pai que permanece ativo após EOF | Pedido de término ao próprio pai e evento de saída observados. |
| Fechamento durante dispatch e stop reentrante | Sem frames posteriores ao fechamento ou segunda operação de stop. |

A [fixture de descendente](../../tools/runtime-harness/fixtures/process-tree.mjs) cria dois processos Node próprios, sem arquivo pessoal/rede, com janela oculta e vida máxima programada. O descendente usa `detached:true`, herda stdout/stderr e se encerra sozinho em **2,5s**. O teste apenas consulta sua existência, sem limpeza destrutiva por PID. O caso normal sem detached não reproduziu sobrevivência nesta máquina; não foi usado como prova de controle da árvore. O modo detached é documentado como permitindo sobrevivência ao pai no Windows. [Node 24.19.0 — detached](https://nodejs.org/download/release/v24.19.0/docs/api/child_process.html#optionsdetached)

No cenário que reproduz o problema, o timeout de drenagem é **150ms** e o prazo inicial de stop é **40ms**. O resultado conferido foi:

```json
{
  "code": 0,
  "signal": null,
  "parentExited": true,
  "treeTermination": "unknown",
  "stdioClosed": false,
  "observationDetached": true
}
```

O descendente estava vivo após esse resultado e depois encerrou por seu próprio timer. Fechar a observação não foi contabilizado como término de worker, tarefa ou Run.

## Limites e próxima prova técnica

Esta é uma prova do observador experimental em Node no Windows, com mocks e processos sintéticos. Não certifica cancelamento de ferramenta real, sandbox, retomada, autenticação isolada, capacidade/cotas ou supervisor Rust. Portabilidade para outros sistemas não foi executada nesta rodada. O resultado live anterior de 08/10 permanece uma evidência separada.

Depois do piloto, OX-001 ainda precisa testar alteração em worktree, resume real e contenção/encerramento de descendentes próprios. Para o supervisor Windows, Job Objects são uma superfície a investigar: a documentação prevê agrupamento e término dos processos associados, inclusive `KILL_ON_JOB_CLOSE`, com regras de associação/nesting/breakaway. Isso é orientação documental, **sem implementação ou teste de Job Object do Orchestrix nesta rodada**. [Microsoft — Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects)

O contrato de produção deve conservar estas distinções. Suspensão mínima de novas admissões pelo daemon pertence a M2; política global/DAG/pooling ampliados pertencem a M3. O harness não implementa esse daemon ou scheduler. [Plano e gates](../DEVELOPMENT_PLAN.md)

## Continuação offline — limites de saída JSONL em 10/10

O pedido de continuar o plano levou à preparação independente do transporte enquanto o piloto humano permanece pendente. O cliente já limitava frames recebidos, mas `write()` não limitava frames enviados nem o orçamento da fila Writable. Um runtime sem leitura de stdin podia acumular escritas. A correção permanece em [JsonRpcProcess](../../tools/runtime-harness/src/json-rpc.mjs), sem conexão com um provider real ou com o helper Windows.

`maxOutboundMessageBytes` passa a limitar cada frame de saída a **1 MiB por padrão**, medido em UTF-8 **incluindo o LF**. `maxPendingWriteBytes` admite no máximo **2 MiB por padrão** na soma de `stdin.writableLength` e bytes do novo frame, conferida antes da escrita. Os limites de bytes, inclusive o de entrada já existente, precisam ser inteiros positivos seguros; configuração inválida falha antes de lançar um processo. Retorno `false` do Writable é aceitação na fila, sem reenvio do frame.

As semânticas de erro são distintas:

| Evidência | Efeito observado |
| --- | --- |
| Frame excessivo (`outbound-limit`) ou JSON não serializável (`arguments`), com `dispatch: not-written` | Nenhuma escrita, pedido/timer limpos e transporte saudável conservado. Um início localmente recusado não cria turno ou terminal; admite outra ação explícita se não houver atividade contraditória/falha. |
| Soma da fila e novo frame excede o orçamento (`backpressure`) | O frame excedente não é escrito; o transporte falha, pedidos pendentes são rejeitados e somente o pai próprio ainda ativo recebe pedido de término. Turno previamente enviado permanece incerto; segunda admissão bloqueada. |
| Exceção síncrona durante escrita (`transport`, `dispatch: unknown`) | Não presume entrega ou rejeição pelo runtime; falha de transporte conserva incerteza. |
| Resposta localmente recusada para request do servidor | Ownership do ID é preservado até uma resposta explícita válida. Não registra uma permissão como respondida sem escrita. |
| Observação fechada durante `toJSON` reentrante | Reconfere disponibilidade após serialização e não escreve depois do fechamento. |

A [fixture própria](../../tools/runtime-harness/fixtures/write-backpressure.mjs) tem modos `echo` e `blocked`. O primeiro responde com contagens e um turno sintético; o segundo nunca lê stdin. Não inicia descendentes ou acessa rede, arquivos, contas ou autenticação. Ambos possuem timer de segurança de **oito segundos**; os testes encerram somente o filho que lançaram. Não houve instalação, servidor web ou alteração de login.

**Verificação executada:** `npm.cmd test` passou **58/58 em uma única rodada, sem falhas ou skips, em 12,97 segundos**, no Windows com Node **24.19.0**: 48 testes anteriores e dez novos de [escrita/backpressure](../../tools/runtime-harness/tests/write-backpressure.test.mjs). `npm.cmd run check` e `node --check` dos dois arquivos novos também passaram. Uma primeira execução focada aprovou nove casos antes do acréscimo do caso reentrante; a suíte completa contém os dez. Os oito testes nativos Windows de 09/10 não foram reexecutados nem contados como novos resultados nesta rodada.

Os novos casos conferem bytes UTF-8/LF na fronteira, rejeição antes da escrita, ausência de pedidos/timers remanescentes, resposta com ID próprio, frame aceito com retorno `false` sem duplicação, subprocesso real sem leitura de stdin, início localmente recusado seguido de ação explícita e início enviado que permanece incerto após saturação. São provas de contrato com dados sintéticos, sem medir latência/cotas de agentes.

**Limites:** o orçamento cobre frames e a fila local de escrita, não toda a memória do processo. `JSON.stringify` ainda aloca uma representação temporária antes de medir; buffers do sistema operacional, quantidade de RPCs simultâneos e outras coleções não passam a ter limite global por essa mudança. O cliente continua com `treeTermination: unknown`. O [watchdog independente de stdout foi comprovado no helper Windows sintético separado](windows-process-containment.md#continuação-em-1010--watchdog-e-stdout-bloqueado) em 10/10. Sua integração com runtime real e a retomada persistida continuam pendentes. Nenhum gate D1/M0 ou funcionalidade do Core foi concluído por essa preparação.
