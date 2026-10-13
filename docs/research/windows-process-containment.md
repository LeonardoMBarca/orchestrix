# Preparação offline — contenção de processos no Windows

**Estado atual — 13/10/2026:** o gate piloto D1 está liberado. A bridge experimental está conectada ao `JsonRpcProcess`, descrita na [continuação integrada](#integração-do-transporte--1210). Duas tentativas live usaram **2/5 turnos autorizados**, mas não produziram diff; o diagnóstico identificou ausência de `SYSTEMDRIVE` no ambiente nativo. A correção mínima passou comparação determinística direta/Job, sem inferência; provas live após a correção continuam pendentes. Os trechos de 09/10 e 10/10 são históricos; OX-001/M0 permanecem abertos.

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

## Integração do transporte — 12/10

O [adapter Node experimental](../../tools/runtime-harness/src/windows-job-process.mjs) conecta o `JsonRpcProcess` ao novo [job-bridge.ps1](../../tools/runtime-harness/windows/job-bridge.ps1), reutilizando `NativeJobProbe.LaunchRaw` e [NativeJobBridge](../../tools/runtime-harness/windows/NativeJobBridge.cs). O entry point sintético anterior conserva fixture/hash, ambiente e limites próprios; não foi convertido em lançador genérico. Esta integração continua em OX-001, sem antecipar o supervisor de produção de OX-007.

Três named pipes locais, com nonce aleatório, transportam **bytes** de stdin/stdout/stderr do runtime. Stdio do helper transporta somente comandos/eventos de controle limitados; seus envelopes não entram no JSON-RPC. `ready` exige as três conexões, Job configurado, associação atômica, pertença conferida, resume e watchdog armado. O executável `.exe` é resolvido explicitamente, sem shell ou busca implícita no cwd. O ambiente mantém a allowlist do harness, excluindo API keys e overrides herdados; os overrides de hooks/extensões continuam explícitos e verificados.

O relatório separa terminal do turno, saída da raiz, saída do helper, EOF/drenagem de cada stream e contabilização do Job. `treeTermination=confirmed` exige registro contínuo do helper próprio, raiz observada e Job vazio. Helper morto sem esse registro mantém `unknown`, mesmo quando PIDs conhecidos desaparecem. Falha antes de criar raiz não inventa saída de runtime. Árvore confirmada com I/O incompleto conserva ambas as observações e impede sucesso do transporte.

`stop()` compartilha cleanup, tenta EOF gracioso e depois solicita stop do Job. `JsonRpcProcess.terminateOwnedJob()` pede força explicitamente **sem EOF prévio**, evitando contar saída graciosa como término forçado. O percurso preparado espera início do turno próprio e exige saída **137**, `termination=forced`, helper com saída zero, streams fechados/drenados e Job vazio. Um terminal recebido antes ou durante a força impede alegar término de processo durante turno ainda ativo. Nenhum desses caminhos fabrica `turn.interrupted`, mata por nome ou abre um outsider; o percurso cooperativo por `turn/interrupt` permanece distinto.

### Interface, prazos e limites

`JsonRpcProcess({windowsJob:true})` seleciona a bridge; a opção também aceita `{lifetimeMs,startupTimeoutMs}`. O caminho sem bridge conserva `treeTermination=unknown`. Os probes expõem `--windows-job`; `--windows-sandbox mxc` escolhe o sandbox do subprocesso separadamente. `--process-stop` no worktree probe substitui a interrupção cooperativa do terceiro turno pela prova de força no Job próprio. Job Object não substitui o sandbox de arquivos/rede.

A bridge tem prazo de startup de **10 s** antes da confirmação e watchdog monotônico por execução de **1–300 s**, com máximo de **300 s** para turnos limitados a **90 s**. O helper sintético anterior conserva máximo de **30 s**. São orçamentos distintos; os deadlines anteriores não foram ampliados para passar testes. Stop durante startup encerra somente o helper criado e impede admissão posterior. Add-Type, setup e lançamento anterior ao watchdog continuam sujeitos ao prazo externo de startup, sem garantia de deadline nativo nessa etapa.

O stop comum concede **3 s** de graça por padrão; depois de enviar stop, o adapter espera até **4,5 s** pelo helper e, se necessário, encerra somente esse processo próprio, esperando mais **1 s**. O helper consulta o término do Job por até **3 s** e concede até **1,5 s** para as bombas fecharem. O watchdog continua independente do controle/stdout, com **1 s** de graça antes do autoencerramento do helper; a via abrupta pode não emitir `stopped`. `CancelSynchronousIo` e `CancelIoEx` usam handles próprios retidos durante cancelamento, sem I/O sob o lock do Job.

JSON-RPC conserva **1 MiB** por frame e **2 MiB** de fila Writable. O controle Node limita frames a **8 KiB**, registros a **4096** e inspeções pendentes a **16**; a fila C# de controle mantém **128 linhas**. O launchspec tem orçamento de **128 KiB** em UTF-8 no Node e limite de **131072 caracteres** no leitor nativo; args/ambiente também têm validação própria. Cada bomba C# usa chunks de **8 KiB**; os PassThrough Node têm `highWaterMark` de **64 KiB** e propagam backpressure. Buffers do sistema operacional e representações temporárias não estão incluídos nesses limites. Após saída da raiz/helper, a drenagem é limitada a **250 ms** e distingue EOF bruto de bytes ainda não consumidos, inclusive UTF-8 incompleto.

### Verificação anterior à correção de SYSTEMDRIVE

```powershell
cd tools/runtime-harness
npm.cmd run check
npm.cmd test
npm.cmd run test:windows-job
npm.cmd run test:windows-transport
# Apenas metadados, sem inferência:
node worktree-probe.mjs --windows-job --windows-sandbox mxc
node exec-probe.mjs --windows-job --windows-sandbox mxc
```

A suíte antiga foi reexecutada: **9/9 em 31,86 s**, sem skips. No caso de controle bloqueado, membros conhecidos saíram em **3524 ms** e helper em **4672 ms**, com deadline de **4 s** e safety de **20 s**; foram observados **19 frames**, **82835 bytes**, frame final parcial e nenhuma confirmação `stopped` inventada. A suíte offline passou **124/124 em 37,72 s**; sete casos posteriores passaram separadamente em **0,40 s**, totalizando **131 casos distintos**. As rodadas permanecem separadas.

A suíte de integração passou **20/20 em 44,43 s**, usando transporte, bridge nativa e fake runtime: UTF-8/correlação, interrupção cooperativa, raiz encerrada com descendente ativo, outsider preservado, crash, EOF de runtime/controle, deadline independente, timeout de turno, stderr extenso, stdout/stdin bloqueados, protocolo inválido, força explícita, falhas de lançamento, stop antes da admissão, perda do helper, resposta final pausada, UTF-8 truncado e timeout de startup. São provas sintéticas, sem inferência ou autenticação de provider.

Uma primeira rodada teve **14/15 aprovações**: o fixture idle encerrou naturalmente por usar safety timer `unref`; foi corrigido para continuar ativo, mantendo a expectativa de saída **137** e deadline de **1 s**. Outra rodada teve **17/18**: o JSON inválido podia falhar durante a resolução de `turn/start` ou no acompanhamento posterior; o teste agora exige a mesma falha `protocol` em todo o percurso, sem pressupor que o ACK chega antes. Corridas entre fechamento do helper, pipes independentes e PassThrough foram corrigidas e cobertas por regressões; nenhum prazo foi ampliado ou erro transformado em sucesso.

Dois preflights reais **sem inferência** passaram em `0.162.0-alpha.17.2` com MXC: [worktree/app-server](evidence/codex-m0-job-metadata.json) e [preparação da comparação CLI](evidence/codex-m0-exec-metadata.json). Incluindo os bootstraps necessários para desativar MCP, foram observados **quatro processos app-server**, todos com saída zero da raiz e do helper, stdio fechado/drenado e Job vazio. O preflight da comparação CLI usa leituras do **app-server**; não executa inferência por `exec` nem comprova CLI edit/resume.

### Tentativas live e diagnóstico de ambiente — 13/10

Foram usados **dois dos cinco turnos autorizados**: um pelo [app-server](evidence/codex-m0-job-live-blocked.json) e outro pela [CLI](evidence/codex-m0-exec-live-blocked.json). Ambos receberam terminal `completed`, mas o diff permaneceu vazio e os checks da tentativa ficaram `notRun`. Os percursos pararam nesse gate, sem retomada ou prova de força durante turno ativo. Um terminal de conversa não substitui alteração válida, diff/checks ou aceite da tentativa.

O diagnóstico consultou somente as threads dos fixtures próprios: `thread/list` com cwd exato e `sourceKinds` `appServer`/`exec`, seguido de `thread/read` no escopo dessas threads. Em cada thread, **duas operações `commandExecution`** falharam com saída **1** e erro MXC `cannot locate required Windows platform directory`, HRESULT **`0x80070003`**. Não foi salvo transcript; o registro conserva apenas o erro técnico necessário e os resultados observados.

A comparação sem inferência executou o mesmo comando fixo de escrita/checks: lançamento direto teve saída **0** e **cinco checks aprovados**; via Job, teve saída **1** com o mesmo erro MXC. A diferença foi rastreada à ausência de `SYSTEMDRIVE` no ambiente construído pela bridge. A implementação oficial do [libuv distribuído com Node 24.19.0](https://github.com/nodejs/node/blob/v24.19.0/deps/uv/src/win/process.c) inclui `SystemDrive` em `required_vars` e complementa variáveis essenciais em `make_program_env`; o caminho direto do Node e `CreateProcessW` com bloco explícito não tinham ambientes equivalentes.

A correção acrescenta **somente `SYSTEMDRIVE`** à allowlist JavaScript e permite essa chave no ambiente raw C#. Não herda o ambiente integral, não habilita credenciais/overrides e não muda Job, MXC, pins de versão/modelo, permissões, exclusões de temporários, rede ou deadlines. Na [repetição do comando fixo](evidence/codex-m0-job-mxc-diagnostic.json), os caminhos direto e Job tiveram saída **0** e **cinco checks aprovados cada**; o Job ficou vazio, com término `confirmed`/`graceful`. Essa prova determinística não é uma edição produzida pelo modelo.

**Rodada final após a correção:** check passou; **131/131 offline em 39,67 s**, **21/21 de transporte Windows em 40,75 s** e **9/9 do helper em 28,28 s**, sem falhas ou skips. No caso de stdout de controle bloqueado, com deadline de **4 s**, membros conhecidos saíram em **3580 ms** e helper em **4599 ms**; foram observados **19 frames**, **82837 bytes**, frame final parcial e nenhuma mensagem `stopped` inventada. Os resultados anteriores de 131 casos distintos, 20/20 de transporte e 9/9 do helper acima permanecem históricos. Nenhuma prova live pós-correção foi executada e não há retry automático; essa execução exige autorização específica.

### Matriz atual por ambiente

Ambiente: Windows 11 Home Single Language **10.0.26200**, PowerShell **5.1.26100.9444** / CLR **4.0.30319.42000**, Node **24.19.0**. Outras versões/sistemas não foram validados.

| Capacidade | Evidência | Estado/limite |
| --- | --- | --- |
| JSON-RPC bidirecional dentro de Job próprio | Fake integrado, mesma bridge de bytes | Comprovado sinteticamente |
| Cleanup de descendente após saída da raiz / outsider preservado | Pertença antes, Job vazio depois, outsider vivo | Comprovado sinteticamente |
| Interrupção cooperativa e força explícita | Terminal `interrupted` mantém processo; força 137 não inventa terminal | Comprovado sinteticamente |
| Watchdog, saída bloqueada e falhas de lançamento | Suítes nativas separadas e integração | Comprovado nos casos testados; falha de observação conserva `unknown` |
| Edição/checks/retomada/interrupção Codex `0.162.0-alpha.17.2` | Live MXC anterior, sem bridge | Comprovado no recorte anterior; não demonstra Job integrado |
| Codex real dentro da bridge, sem inferência | Dois preflights / quatro app-servers com bootstrap | Metadata e cleanup comprovados; não demonstra trabalho do modelo |
| Edição/retomada Codex dentro da bridge e força durante turno ativo | Primeiro turno app-server `completed`, diff vazio e checks `notRun` | Edição não comprovada; retomada/força não executadas; live pós-correção pendente |
| CLI `exec`/resume reais, mesmo fixture/modelo/effort | [Comparação preparada](cli-app-server-comparison.md); primeiro turno CLI `completed`, diff vazio e checks `notRun` | Edição não comprovada; resume não executado; sem ACK equivalente de configuração resolvida; live pós-correção pendente |
| MXC com ambiente mínimo corrigido, direto e dentro do Job | Comando fixo, saída zero e cinco checks em cada caminho; Job `confirmed`/`graceful` | Comprovado sem inferência; não demonstra trabalho do modelo |
| Contas independentes, sandbox completo e recuperação durável | Fora deste incremento | `Unknown`; OX-002/M1 preservados |

Não foi incorporado código das referências. A leitura focada de Jackalope (`process_control.rs`) e Microsoft Conductor (`execution/local.py`), identificados no [catálogo](REFERENCE_REPOSITORIES.md), ajudou a conferir ownership e drenagem; seus mecanismos não foram tratados como prova Windows equivalente. Nenhum script upstream foi executado.

O helper continua **experimental**, sem definir a dependência final Rust/Tauri. Processos delegados a serviços fora do Job, handles duplicados, todas as combinações de nesting e recuperação após crash não estão certificados. A autorização original limitou o trabalho a cinco turnos; **2/5 foram usados** e **três não foram usados**, sem reiniciar ou redistribuir o orçamento automaticamente. A regressão pós-correção passou; o próximo passo é a prova live com autorização específica, preservando execução serial, **90 s por turno**, **300 s por Job**, sem retry ou fallback pago. Edição/retomada e força integradas após a correção continuam pendentes; OX-001/M0 permanecem abertos e OX-003 conserva a dependência do primeiro runtime aprovado.
