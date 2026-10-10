# Preparação offline — contenção de processos no Windows

Registro de **9 de outubro de 2026**. Continuação do plano mantendo o escopo inicial: reduzir a incerteza de supervisão antes de integrar o primeiro worker. **D1 aguarda observações do piloto; M0 e OX-001 permanecem abertos.** Esta prova usa processos Node sintéticos próprios, sem agente de código, autenticação, inferência ou alteração de projeto do usuário.

**Continuação em 10/10:** watchdog independente do stdout implementado e suíte nativa final **9/9 aprovada em 26,40s**, com o caso de pressão descrito ao final. Os oito casos/25,16s de 09/10 e os 48 testes offline daquela rodada permanecem resultados históricos separados. O [cliente JSON-RPC passou 58/58](offline-lifecycle-validation.md#continuação-offline--limites-de-saída-jsonl-em-1010) em outra preparação offline de 10/10; essa suíte não foi reexecutada na rodada do watchdog.

## Problema e implementação experimental

A [preparação anterior](offline-lifecycle-validation.md) demonstrou um descendente sobrevivendo ao pai. O [cliente JSON-RPC](../../tools/runtime-harness/src/json-rpc.mjs) conserva `treeTermination: unknown`: observar a saída do pai ou fechar pipes não confirma o término de seus descendentes.

O [helper separado](../../tools/runtime-harness/windows/job-probe.ps1), com [interop C#](../../tools/runtime-harness/windows/NativeJobProbe.cs), cria um Job Object anônimo, configura `KILL_ON_JOB_CLOSE` e associa o filho durante `CreateProcessW`, com `PROC_THREAD_ATTRIBUTE_JOB_LIST`. O filho nasce suspenso; somente executa após verificação de pertença ao Job próprio. Isso evita a janela de execução ou filho suspenso sem associação de uma sequência create/assign separada. O handle do Job é verificado como não herdável e fica fora da lista explícita de handles de stdio. Não são habilitados flags de breakaway. [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects), [atributos de criação](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute), [criação suspensa](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags).

Comandos e eventos são JSONL limitados; só campos selecionados são emitidos. Paths, argumentos, stdout/stderr brutos e mensagens de exceção não são relatados. O lançamento recebe executável/cwd explícitos e argumentos com quoting Windows, sem shell. Um ambiente limitado exclui variáveis de credenciais e `NODE_OPTIONS`. O script deve corresponder ao SHA-256 da fixture do repositório; cópias são aceitas. Essa checagem reduz lançamento acidental de outro script, sem autenticar um repositório malicioso ou constituir sandbox.

No encerramento controlado, o helper chama `TerminateJobObject` e consulta `ActiveProcesses`, além de observar o handle da raiz com prazo. Pedido de término sozinho não representa confirmação. Erro/timeout conserva estado desconhecido. Ao perder o helper abruptamente, o último handle do Job fecha e o teste observa os membros conhecidos. [Término de Job](https://learn.microsoft.com/en-us/windows/win32/api/jobapi2/nf-jobapi2-terminatejobobject).

## Verificação reproduzível

Pré-requisitos desta prova opcional: **Windows 10+ x64**, Windows PowerShell x64 com `Add-Type`/compilação C# disponível e Node 24+. O ambiente desta rodada é **Windows 11 Home Single Language 10.0.26200**, PowerShell **5.1.26100.9444**, CLR **4.0.30319.42000** e Node **24.19.0**. Foram usados componentes já instalados, sem instalar toolchains ou pacotes. O helper compila o arquivo C# a cada processo PowerShell; essa abordagem é apenas do experimento.

```powershell
cd tools/runtime-harness
npm.cmd run check
npm.cmd test
npm.cmd run test:windows-job
```

`npm test` mantém os testes anteriores separados. `test:windows-job` executa a prova nativa; em outro sistema, os casos são pulados, sem registrar suporte Windows como demonstrado. A [fixture](../../tools/runtime-harness/fixtures/windows-job-tree.mjs) cria raiz e descendente `detached`, ambos com timer de segurança de **20 segundos**. O caso histórico de deadline usou **2 segundos**; na continuação de 10/10, deadline e pressão usam **4 segundos**, mantendo exigência de término em menos de 10s após obter a identidade da árvore, para distinguir contenção da expiração natural. A pasta temporária com espaços/acentos é mantida para inspeção, sem remoção recursiva automática.

**Resultado histórico de 09/10:** checks Node e **48 testes anteriores passaram**. A suíte nativa final passou **8/8**, sem skips, em **25,16 segundos**, após reforço das evidências de estágio, associação e término. As duas suítes somam **56 testes aprovados naquela rodada**, mantendo os comandos separados. A duração é da suíte sintética, sem significado de benchmark de agentes. Compilação C# ocorreu pelo próprio helper nos oito casos.

| Caso nativo aprovado | Evidência observada pelos asserts |
| --- | --- |
| Associação e path com espaços/acentos | Raiz e descendente vivos no Job específico antes de stop; Job vazio e membros conhecidos encerrados depois. |
| Raiz encerra antes do descendente | Saída da raiz com código zero, descendente ainda vivo/associado; stop encerra o membro restante. |
| Helper termina abruptamente | Membros conhecidos desaparecem antes do timer de segurança; não se exige mensagem final de um helper já encerrado. |
| EOF do controlador | Motivo `stdin-eof`, Job vazio e término observado. |
| Deadline | Motivo `deadline`, código 2 e término antes da expiração do fixture. |
| Falha controlada antes de resume | Categoria `injected-before-resume`, raiz criada/encerrada, ausência de ready e Job vazio. Não basta falhar antes de criar o filho. |
| Processo independente | PID externo não pertence ao Job; processo próprio externo permanece vivo após stop. |
| Executável ausente | Categoria `launch`, sem raiz criada ou saída de raiz inventada. |

## Limites e aplicação no plano

- A evidência cobre os processos comprovadamente associados ao Job nas condições testadas. Não demonstra término de processos externos, delegados a serviço, WMI ou outras rotas fora dessa associação. Jobs aninhados e restrições do ambiente podem impedir lançamento; não há fallback por breakaway. [Limites de Jobs](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).
- O teste de crash observa os PIDs conhecidos após a pertença ter sido confirmada. O teste não conserva handles dos descendentes fora do helper; reutilização de PID limita essa observação. Os caminhos de encerramento controlado também consultam o Job e o handle da raiz.
- A falha antes de resume é controlada, com cleanup. Não equivale a crash abrupto nessa etapa. Duplicata deliberada do handle do Job, netos, carga elevada e toda combinação de Jobs externos não foram cobertos pelos oito casos.
- O término é forçado; não comprova cancelamento gracioso de turno, flush/preservação de trabalho, recuperação durável ou liberação segura de locks do produto. Job Object não é sandbox de arquivos/rede ou autorização de ferramentas.
- Na rodada original, o deadline dependia do loop escritor. A continuação abaixo comprovou watchdog independente para execução ativa e stdout bloqueado; setup/lançamento antes de armar a thread, integração de runtime real e todos os caminhos de falha permanecem fora da prova.
- O helper não está conectado ao `JsonRpcProcess` ou ao Codex. O contrato anterior continua com árvore desconhecida. Antes de promover suporte, OX-001/OX-003/OX-007 precisam demonstrar integração, protocolo/stdio, runtime real e condições de encerramento do worker.

O próximo trabalho mantém o [handoff](discovery-handoff.md): registrar o piloto inicial D1 e, depois, retomar M0 com código em worktree, retomada de sessão e supervisão integrada. Esta preparação pode ser aproveitada no supervisor Rust; não fixa dependência de PowerShell/C# no aplicativo final nem adiciona feature de produto.

## Continuação em 10/10 — watchdog e stdout bloqueado

Uma thread C# de [NativeJobProbe](../../tools/runtime-harness/windows/NativeJobProbe.cs) agora inicia um `Stopwatch` monotônico depois de `Launch`, **antes de qualquer `ready`**. Ao atingir o deadline, ela marca `WatchdogTriggered` e chama `Stop(3000)` no handle do Job próprio, sem escrever stdout/stderr ou esperar pelo loop PowerShell. Todas as consultas e operações sobre handles usam o mesmo lock; uma confirmação de stop é conservada para chamadores concorrentes. Console/pipe I/O, Join e Dispose de streams permanecem fora do lock. Dispose cancela/junta a thread antes de liberar seu evento.

O loop responsivo em [job-probe.ps1](../../tools/runtime-harness/windows/job-probe.ps1) observa a flag de deadline antes das ações e depois de consultar o Job vazio. Assim, o esvaziamento pelo watchdog não vira saída natural de código zero. O formato normal continua exigindo `ActiveProcesses=0` e saída observada da raiz para reportar `treeTermination=confirmed`; pedido de término, sozinho, permanece insuficiente.

Após Stop, a thread concede **1000ms** ao controlador para emitir `stopped` e concluir Dispose. Se ele continuar preso, o fallback chama `TerminateProcess(GetCurrentProcess(), code)`, exclusivamente no próprio helper. O pseudo handle não envolve busca/abertura de um PID externo; o autoencerramento interrompe as threads e solicita cancelamento do I/O. [GetCurrentProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentprocess), [TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess).

Código **3** exige Stop confirmado e uma escrita de Console ainda em andamento; código **4** conserva fallback sem essas duas evidências. Ambos representam saída abrupta do helper e podem cortar o último frame. Eles **não substituem uma mensagem `stopped` ausente ou um terminal do runtime**. A saída do processo e o fechamento dos pipes são observações distintas, inclusive no teste: o stdout só volta a ser drenado depois de provar saída do helper.

O hook sintético `output-flood` recebe um único comando de controle, emite um marker e tenta produzir **4096 frames com padding fixo de 4096 caracteres**. Cada frame continua abaixo de 8192 caracteres; o volume planejado de 16MiB exerce pressão sem ultrapassar a fila de 128 comandos de entrada. O observador pausa stdout imediatamente ao interpretar o marker, após conferir raiz/descendente vivos e associados ao Job específico. Somente nesse caso de perda do stdout é permitido um último frame incompleto; os oito casos normais preservam parsing estrito.

**Resultado final:** `npm.cmd run test:windows-job` passou **9/9 em uma única execução, sem falhas ou skips, em 26,40 segundos** no mesmo Windows/PowerShell/Node registrados acima. Compilação C# ocorreu pelo helper. No caso novo, com deadline de 4s, os membros conhecidos desapareceram em **3733ms** e o helper saiu com código 3 em **4762ms**, contados desde o marker. Foram observados **19/4096 frames**, **82.834 bytes**, último frame parcial e **nenhuma mensagem `stopped`**. Ambas as saídas ficaram abaixo da margem de 10s e muito antes do safety timer de 20s; esse timer não foi contado como sucesso.

As tentativas anteriores explicam o desenho e os limites da evidência: com `Environment.Exit`, os membros já haviam terminado, mas o host PowerShell permaneceu preso durante a saída gerenciada; o teste falhou na liveness do helper e seu cleanup encerrou apenas esse helper próprio. O fallback nativo passou no reteste focado. Uma primeira suíte completa teve **8/9 aprovações em 88,12s** sob carga: o deadline de 2s encerrou corretamente a fixture antes de `fixture.ready`, sem identidades suficientes para provar aquele caso. O orçamento foi alterado para 4s; a rodada final 9/9 acima é o resultado aprovado, sem converter a falha anterior em evidência positiva.

**Limites preservados:** a thread só arma depois de Launch/resume. Add-Type, setup, chamadas nativas de lançamento travadas e diagnósticos de erro antes dessa etapa não são cobertos. Stop/EOF continuam consumidos pelo loop; sob stdout bloqueado, podem aguardar o deadline independente. Não se demonstra deadline de tempo real sob suspensão/falta de agendamento, todos os erros de query/autoencerramento ou cleanup após cancelar o watchdog. O encerramento abrupto não garante flush, trabalho preservado ou recuperação durável. PIDs externos conhecidos ainda têm o limite de reutilização já registrado. JsonRpcProcess/Codex continuam separados deste helper, sem nova execução de provider, autenticação, servidor, instalação, supervisor de produção ou gate D1/M0 concluído.
