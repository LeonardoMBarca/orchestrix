# Preparação offline — turnos e observação de processos

Registro de **9 de outubro de 2026**, Windows nativo, Node **24.19.0**. Esta continuação atende ao pedido de prosseguir o plano com correções de D1 e trabalho preparatório independente. **O piloto humano continua pendente; M0 não foi retomado ou concluído.** Não houve execução de Codex/Claude/Antigravity, autenticação ou inferência nesta rodada.

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

A suíte completa passou **48 testes**: 23 contratos anteriores, 19 de [ciclo de sessão](../../tools/runtime-harness/tests/session-lifecycle.test.mjs) e seis de [ciclo de processo](../../tools/runtime-harness/tests/process-lifecycle.test.mjs). São testes de contrato offline, sem benchmark de desempenho do runtime.

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
