# Windows Job Object — harness experimental

**Estado atual — 13/10/2026:** a bridge ligada ao `JsonRpcProcess` passou a nova prova pós-correção com **5/5 turnos autorizados usados**: três app-server e dois CLI. App-server está aprovado como primeiro runtime experimental no recorte local single-account/serial e libera o início de OX-003; OX-001/OX-002/M0 permanecem abertos. A [nova prova abaixo](#nova-prova-live-e-recorte-aprovado--1310) conserva a distinção entre força local e terminal remoto. As duas tentativas bloqueadas da autorização anterior de 2/5 turnos e o helper sintético de 09/10–10/10 permanecem históricos.

Data: 09/10/2026. Experimento offline separado de `JsonRpcProcess`, do protocolo Codex e do Core Rust/Tauri. Não lança providers, inferência ou autenticação. D1 continua com piloto humano pendente.

Complementado em 10/10 com watchdog C# independente do stdout de controle. O resultado histórico de oito casos abaixo permanece separado da nova rodada; os gates do produto continuam abertos.

A rodada final do watchdog passou **9/9 em 26,40s**, sem falhas ou skips. No stdout bloqueado, membros conhecidos terminaram em 3733ms e helper em 4762ms desde o marker, com deadline de 4s/safety de 20s; nenhum `stopped` foi inventado. O [registro](../../../docs/research/windows-process-containment.md#continuação-em-1010--watchdog-e-stdout-bloqueado) preserva a primeira rodada 8/9, o ajuste do orçamento e os limites.

`job-probe.ps1` compila `NativeJobProbe.cs` com Add-Type no PowerShell instalado; não instala dependências nem grava binário no repositório. Requer Windows 10+ de 64 bits, .NET do PowerShell e Node nativo. Foi exercitado em Windows 11 10.0.26200, PowerShell 5.1.26100.9444/.NET CLR 4.0.30319.42000 e Node 24.19.0. Outras versões não foram validadas.

## Ownership e lançamento

Cada instância cria um Job anônimo e um ownerId aleatório. O handle do Job não é herdável; essa flag é consultada antes do lançamento. O Job recebe apenas `KILL_ON_JOB_CLOSE`, sem permitir breakaway.

O processo é criado com `CREATE_SUSPENDED` e `STARTUPINFOEX`. A lista `JOB_LIST` associa o processo ao Job na própria criação; `HANDLE_LIST` transmite apenas os três handles de stdio do filho. O handle do Job e o stdin de controle do helper ficam fora dessa lista. A associação ao Job específico é conferida com IsProcessInJob antes de ResumeThread; a retomada exige contagem anterior igual a 1. Ready é emitido somente depois dessa sequência.

Esse contrato segue a API oficial de [atributos de criação de processo](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute), que documenta JOB_LIST para Windows 10+/Server 2016+. O uso da lista evita depender de um intervalo separado CreateProcess→AssignProcessToJobObject.

O NodePath é passado explicitamente como applicationName para [CreateProcessW](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw); não há cmd.exe, PowerShell interpolando o comando filho ou busca de executável no PATH. Argumentos recebem quoting conforme as [regras CRT da Microsoft](https://learn.microsoft.com/en-us/cpp/c-language/parsing-c-command-line-arguments?view=msvc-170). O filho recebe uma allowlist de variáveis de sistema; NODE_OPTIONS e credenciais não entram.

O fixture informado deve ter os mesmos bytes SHA-256 do `fixtures/windows-job-tree.mjs` do harness. Cópias em diretórios temporários com espaços, acentos e outro basename são aceitas. Essa checagem evita lançamento acidental de outro script; não é uma fronteira contra alteração maliciosa do próprio repositório.

## Interface experimental

```powershell
& .\windows\job-probe.ps1 -NodePath 'C:\caminho\node.exe' -FixturePath 'C:\caminho\windows-job-tree.mjs' -LifetimeMs 10000
```

Parâmetros adicionais: Scenario (identificador simples, default tree) e FailureStage (none ou before-resume). O helper passa ao fixture `--scenario <valor> --lifetime-ms <valor>`. O fixture atual conserva sua própria safety lifetime de 20s; LifetimeMs do helper é um deadline separado, de 1000 a 30000ms.

Stdin é JSONL UTF-8 com os comandos:

| Entrada | Efeito |
| --- | --- |
| `{"type":"inspect","pid":123}` | Consulta a lista completa do Job próprio. Um outsider retorna inOwnedJob=false sem abrir seu processo. Um membro listado recebe uma segunda verificação no Job específico. |
| `{"type":"exit-root"}` | Encaminha somente esse comando fixo ao fixture. A intenção fixture.rootExiting não é tratada como saída confirmada. |
| `{"type":"stop"}` | Termina apenas o Job desta instância e espera sua contabilização. |
| `{"type":"output-flood"}` | Hook sintético de teste: marker e até 4096 frames com padding fixo de 4096 caracteres, para pressionar stdout sem saturar a fila de entrada. Não transmite conteúdo do runtime. |
| EOF | Encerra o Job próprio, sem presumir que EOF no fixture teria parado descendentes. |

Stdout é JSONL sanitizado: schemaVersion=1, sequence, ownerId, type e campos permitidos:

- ready: rootPid, jobConfigured, rootInJob, resumed, atomicAssignment, jobHandleInherited, watchdogArmed.
- fixture.ready: rootPid, descendantPid, lifetimeMs, rootInJob, descendantInJob. Os PIDs são da fixture própria e a associação é conferida antes do relay.
- inspect: pid, inOwnedJob, activeCount.
- root.exit: code e activeCount, após o handle do processo raiz sinalizar.
- error: categoria local e código Win32 quando disponível; sem mensagens brutas.
- stopped: reason, activeCount, rootStarted, rootExited, termination=forced e treeTermination.
- output.flood.start / output.flood: contagem/tamanho do hook de teste, índice e padding fixo, exclusivamente para a prova de backpressure do controlador.

Não transmite paths, args, ambiente, conteúdo bruto de stdout/stderr ou mensagens arbitrárias do filho. Filas têm limite de 128 linhas, frames de 8192 caracteres e stderr drenado com limite. Os leitores são threads locais do helper.

Exit codes: 0 para stop/EOF/saída normal observada, 2 para deadline ou injeção before-resume, 1 para outras falhas ou limpeza não confirmada. Um código 2 com stopped confirmado significa deadline encerrado por contenção; não é um resultado terminal de um runtime.

O fallback independente usa **3** quando o watchdog confirmou Job vazio/saída da raiz e uma escrita de Console ainda está em andamento; **4** cobre fallback sem essas duas evidências. São códigos locais de saída abrupta do helper, sem fabricar uma mensagem `stopped` ou um resultado de agente.

## Término e evidência

Stop usa TerminateJobObject no handle próprio. Confirmed exige QueryInformationJobObject indicar activeCount=0 e, se o root nasceu, seu handle estar sinalizado. Timeout/erro de query mantém unknown. O helper não encontra árvores para matar por nome/PID, não chama taskkill e não manipula Jobs globais.

A lista de PIDs consulta assigned e returned, inclusive em sucesso, para rejeitar/expandir resultados parciais; o buffer tem limite. Essas semânticas são documentadas em [JOBOBJECT_BASIC_PROCESS_ID_LIST](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_process_id_list).

Se o helper morre abruptamente, o Windows fecha seu único handle do Job. [KILL_ON_JOB_CLOSE](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_basic_limit_information) encerra membros quando o último handle é fechado; por isso filhos/observadores não recebem esse handle. Essa via abrupta não emite stopped: a observação externa deve continuar distinta do evento inexistente.

Na rodada de **09/10**, os oito testes opcionais cobriram associação pai/descendente detached, caminho com espaços/Unicode, saída do root mantendo descendente, morte abrupta do helper, EOF, deadline antes da safety expiry, falha controlada antes de retomar, outsider preservado e executável ausente. A execução final daquela rodada, após o reforço de invariantes, passou **8/8**, registrada no [relatório de contenção Windows](../../../docs/research/windows-process-containment.md). A suíte atual tem nove casos, com a prova de stdout bloqueado acrescentada abaixo. Nenhum processo de provider participou.

## Limites preservados

Este resultado comprova a fixture sintética e o Job criado neste helper. Não comprova containment do Codex, integração de stdio JSON-RPC real, término de um turno remoto, isolamento de conta, filesystem/network sandbox ou revogação de credenciais.

Jobs ambientais/nesting podem impor restrições ou impedir o lançamento; a falha é reportada, sem habilitar breakaway. Processos criados por serviços externos e handles duplicados por terceiros não são cobertos por esta prova. A query do Job usa o handle próprio; as verificações externas do descendente nesta rodada ainda usam PID conhecido, sem handle externo retido contra reuso de PID.

Na rodada original de 09/10, o deadline dependia do loop do helper e stdout bloqueado não havia sido exercitado. A continuação abaixo separa esse deadline do escritor; sua evidência não promove o helper a supervisor de produção. Cores, piloto D1, recuperação durável e o Core continuam fora deste diretório.

## Watchdog independente do stdout — 10/10

Depois de `Launch` concluir, `ArmWatchdog` inicia uma thread C# e um `Stopwatch` monotônico **antes do primeiro `ready`**. Ela não escreve stdout/stderr, não encaminha comandos ao fixture e não depende do loop PowerShell. Ao vencer LifetimeMs, registra a flag de deadline e executa `Stop(3000)` no Job próprio. O loop responsivo conserva reason=deadline, inclusive se observar o Job esvaziado depois dessa flag.

Consultas, `Stop` e fechamento dos handles compartilham um lock; uma confirmação de stop é preservada para chamadores concorrentes. Console/pipe I/O, Join do watchdog e Dispose de streams ficam fora desse lock. Dispose cancela a thread e espera seu término com prazo antes de liberar o evento de cancelamento.

Após o stop, o watchdog concede **1000ms** para o controlador emitir `stopped` e executar Dispose. Se ele continuar preso, `TerminateProcess(GetCurrentProcess(), code)` encerra exclusivamente o próprio helper, sem buscar ou abrir outro PID. A API documenta o pseudo handle do processo atual e o autoencerramento/cancelamento de I/O. [GetCurrentProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentprocess), [TerminateProcess](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess).

O caminho abrupto pode cortar o último frame; não executa cleanup/flush gerenciado nem garante uma mensagem final. O cliente precisa distinguir saída do helper, fechamento dos pipes e evidência externa dos membros. Os testes comuns mantêm parsing estrito; somente a prova de perda do stdout permite frame final parcial. O comando stop/EOF ainda é consumido pelo loop: sob stdout bloqueado, sua leitura pode esperar o deadline independente.

O watchdog **não cobre Add-Type, setup, lançamento ou diagnósticos antes de ArmWatchdog**. Sua prova também não garante deadlines de tempo real sob suspensão do sistema/falta de agendamento, todos os erros de query/autoencerramento ou I/O de cleanup depois do cancelamento. A fixture conserva o safety timer de 20s, que não é aceito como sucesso de contenção. Veja o [registro atualizado](../../../docs/research/windows-process-containment.md#continuação-em-1010--watchdog-e-stdout-bloqueado).

## Bridge de transporte — 12/10

O [job-bridge.ps1](job-bridge.ps1), a classe [NativeJobBridge](NativeJobBridge.cs) e `NativeJobProbe.LaunchRaw` são a parte nativa do [adapter Node](../src/windows-job-process.mjs). A associação atômica ao Job, a lista explícita de handles e a verificação antes de `ResumeThread` são reutilizadas. `job-probe.ps1` mantém sua validação de fixture/hash, ambiente e limites próprios; seus testes continuam separados.

O Node cria três named pipes locais com nonce de **24 bytes**, codificado em **48 caracteres hexadecimais**, e roles `stdin`, `stdout`, `stderr`. A bridge exige o mesmo nonce e roles distintos. Essas pipes transportam somente bytes do runtime, sem decodificação/transcrição pelo helper. Stdin/stdout do helper são um canal separado de controle JSONL; stdout/stderr do runtime nunca viram eventos de controle ou logs nativos.

O primeiro frame de controle tem `type=launch`, executável/cwd absolutos, args, ambiente limitado, nomes das três pipes e `lifetimeMs`. O Node limita esse frame a **128 KiB** em UTF-8; o leitor nativo limita-o a **131072 caracteres**. Args são limitados a **256**; o ambiente admite no máximo **64 chaves** da allowlist do runtime, **32767 caracteres por valor** e **128 KiB** no bloco UTF-16. API keys, `NODE_OPTIONS` e overrides herdados não são aceitos. Após esse frame, o leitor continua usando o mesmo `Console.In`, preservando bytes já bufferizados.

As três pipes são conectadas antes de lançar o filho. `ready` só é emitido após associação atômica, pertença ao Job conferida, resume e watchdog armado. O Job não é herdado, não permite breakaway e não é encontrado por nome/PID. Lançamento incompatível com Jobs ambientais falha; não há fallback sem contenção.

| Controle após launch | Efeito |
| --- | --- |
| `{"type":"inspect","pid":123}` | Consulta apenas o Job próprio; outsider não é aberto. |
| `{"type":"close-stdin"}` | Fecha somente stdin do runtime; não encerra o Job. |
| `{"type":"stop"}` | Solicita encerramento do Job próprio. |
| EOF do controle | Encerra o Job com motivo `control-eof`; não equivale a EOF de stdin do runtime. |

Os eventos preservam `schemaVersion`, `sequence`, `ownerId` e campos limitados: `ready`; `inspect`; `root.exit` com código observado e contagem do Job; `streams.closed` com `stdinClosed`, `stdoutClosed`, `stderrClosed`, `faulted`; `error` somente com categoria/código nativo; `stopped` com motivo, contagem, raiz iniciada/encerrada, `termination=graceful|forced` e `treeTermination=confirmed|unknown`. Nenhum evento inclui paths, args, ambiente ou conteúdo do runtime.

`graceful` exige Job vazio e raiz encerrada **antes** de solicitar `TerminateJobObject`. `confirmed` exige contagem zero e handle da raiz sinalizado, quando ela nasceu. Falha de query, timeout ou perda do helper conserva `unknown`; ausência de raiz não vira saída inventada. Streams incompletos mantêm suas flags e impedem sucesso de transporte, mesmo quando o Job foi confirmado vazio. Encerrar a árvore não cria um terminal de turno remoto.

### I/O e orçamentos da bridge

Cada bomba usa chunks fixos de **8192 bytes**, sem fila nativa ilimitada. Cancelamento de I/O síncrono usa o handle da thread própria; I/O overlapped usa `CancelIoEx` com lease do SafeHandle exato, conservada até a bomba reconhecer fechamento. Isso cobre a corrida entre verificar o pedido de fechamento e submeter uma nova escrita. Não há I/O ou Join sob o lock do Job, lookup destrutivo por PID ou morte de thread externa.

O controle mantém frames de **8192 caracteres** e até **128 linhas** no C#; o Node aplica **8 KiB** por frame, **4096 registros** e **16 inspeções pendentes**. JSON-RPC mantém **1 MiB** por frame e **2 MiB** de fila Writable; os PassThrough usam `highWaterMark` de **64 KiB**. Essas medidas não certificam memória global, buffers do sistema operacional ou representações temporárias.

`ArmBridgeWatchdog` aceita **1–300 s**; `ArmWatchdog` sintético conserva **1–30 s**. O adapter Node aplica startup de até **10 s**, antes de receber `ready`; não se afirma que o watchdog nativo já cobre Add-Type/setup/lançamento. Depois de armado, ele independe de controle/stdout e conserva **1 s** de graça antes do autoencerramento do helper quando necessário. A via abrupta pode cortar o frame final e não emitir `stopped`.

O helper espera até **3 s** por término do Job e **1,5 s** por fechamento das bombas. O adapter comum concede **3 s** para EOF gracioso, espera até **4,5 s** após solicitar stop e, se necessário, encerra somente seu helper, esperando mais **1 s**. Drenagem Node após saída é limitada a **250 ms**; EOF da pipe e consumo completo do PassThrough são observações distintas. `JsonRpcProcess.terminateOwnedJob()` pula o EOF gracioso para provar força explícita; `--process-stop` seleciona esse percurso no worktree probe, sem afirmar `turn.interrupted`.

### Evidência atual e próximo passo

Na rodada anterior à correção de ambiente, **131 casos offline distintos** passaram em duas execuções: **124/124 em 37,72 s** e **7/7 em 0,40 s**. A integração bridge/fake passou **20/20 em 44,43 s**; os nove casos do helper original foram retestados com **9/9 em 31,86 s**. Esses resultados permanecem históricos. As falhas intermediárias de fixture `unref` e corrida de admissão/observação estão registradas no [relatório](../../../docs/research/windows-process-containment.md#integração-do-transporte--1210), sem reduzir o aceite ou ampliar deadlines.

Dois preflights reais, **sem inferência**, passaram em Codex `0.162.0-alpha.17.2`/MXC: [worktree](../../../docs/research/evidence/codex-m0-job-metadata.json) e [preparação CLI](../../../docs/research/evidence/codex-m0-exec-metadata.json). Incluindo bootstrap de MCP, quatro app-servers tiveram saída zero da raiz/helper, stdio fechado/drenado e Job vazio. O preflight CLI consulta metadata via app-server; não executa CLI edit/resume.

Foram usados **2/5 turnos autorizados**, um de [app-server](../../../docs/research/evidence/codex-m0-job-live-blocked.json) e um de [CLI](../../../docs/research/evidence/codex-m0-exec-live-blocked.json). Ambos tiveram terminal `completed`, diff vazio e checks `notRun`; não houve resume nem força. O diagnóstico ficou restrito às threads dos fixtures próprios, via cwd exato e `sourceKinds` `appServer`/`exec`, sem salvar transcript. Duas operações `commandExecution` por thread falharam com saída **1**, erro MXC `cannot locate required Windows platform directory` e HRESULT **`0x80070003`**.

O [comando fixo sem inferência](../../../docs/research/evidence/codex-m0-job-mxc-diagnostic.json) passou direto com saída **0** e cinco checks, mas falhou via Job com saída **1** e o mesmo erro. A correção permite **somente `SYSTEMDRIVE`** no ambiente raw C# e na allowlist JavaScript. A implementação oficial do [libuv em Node 24.19.0](https://github.com/nodejs/node/blob/v24.19.0/deps/uv/src/win/process.c) complementa `SystemDrive` e outras variáveis essenciais em `required_vars`/`make_program_env`; `CreateProcessW` com bloco explícito precisa recebê-las no escopo validado. Não houve herança do ambiente integral ou alteração de Job, MXC, pins, permissões ou deadlines.

A repetição fixa teve saída **0** e **cinco checks aprovados em cada caminho**, direto e Job; o Job ficou vazio com término `confirmed`/`graceful`. Na regressão pós-correção, **anterior à nova entrega live**, check passou; **131/131 offline em 39,67 s**, **21/21 de transporte em 40,75 s** e **9/9 do helper em 28,28 s**, sem falhas ou skips. Sob stdout de controle bloqueado, com deadline de **4 s**, membros conhecidos saíram em **3580 ms** e helper em **4599 ms**; observaram-se **19 frames**, **82837 bytes**, frame final parcial e nenhuma mensagem `stopped` inventada. Essas suítes 131/21/9 não foram repetidas na entrega live seguinte.

Na conclusão desse diagnóstico ainda não havia prova live pós-correção. Três turnos da autorização original não foram usados, sem retry automático, nova sequência ou redistribuição do orçamento. A prova seguinte teve uma nova autorização explícita e mantém esse histórico separado.

### Nova prova live e recorte aprovado — 13/10

Foram usados **5/5 turnos da nova autorização pós-correção**, serialmente, com **90 s por turno** e **300 s por Job**, sem retry ou fallback pago. HEAD **`f93dae9`**, com correção de runtime **`b6015e3`**; Codex **`0.162.0-alpha.17.2`**, MXC, **`gpt-6.1-sol`** / **`low`**. Nenhuma proteção, composição do Job, pin ou deadline foi alterado nessa prova.

O [app-server passou três turnos em 48,985 s](../../../docs/research/evidence/codex-m0-job-live.json): [edição/diff](../../../docs/research/evidence/codex-m0-job-live.patch) com cinco checks; restart/resume da mesma thread e identidade observável da conta, preservando política, marcador e artefato; força durante o terceiro turno ativo observado, sem ferramentas. A última raiz terminou com **137**, helper com **0**, Job confirmado vazio, `activeCount=0` e todos os streams completos. `startObserved`, `processTerminationConfirmed` e `forcedConfirmed` foram verdadeiros; `terminalStatus=null` e `turnInterruptedConfirmed=false` impedem tratar a força como `turn.interrupted`.

A [CLI passou duas invocações](../../../docs/research/evidence/codex-m0-exec-live.json): [edição/diff](../../../docs/research/evidence/codex-m0-exec-live.patch) com cinco checks e resume do ID exato, recuperando marcador anterior e preservando conta observável e artefato. Todos os **seis processos desse percurso** — quatro app-servers de metadata/bootstrap e dois `exec` — encerraram `graceful`, com raiz/helper **0**, Job confirmado vazio, `activeCount=0` e stdio fechado/drenado. Não houve turno CLI dedicado à força nem prova de lock atômico da autenticação ou ACK equivalente da configuração.

| Recurso | Estado no recorte validado |
| --- | --- |
| Primeiro runtime experimental app-server, versão/MXC/modelo/effort acima | `supported`, observado localmente com uma conta e execução serial. |
| Edição/checks, restart/resume, política e artefato app-server | `supported`, observado no percurso integrado. |
| Força no Job durante turno app-server ativo | `supported`, árvore vazia/streams completos; resultado remoto sem terminal permanece desconhecido. |
| CLI edit/resume por ID exato | Observado; ACK de configuração e interrupção por RPC não são observáveis no stream examinado. |
| Catálogo completo/paginação, service tiers e registro de features nativas | Parcial ou `unknown`; descoberta atual conserva somente a primeira página/modelos/esforços. |
| Sandbox completo, multiaccount e serviços externos fora do Job | `unknown`; nenhuma certificação por associação à prova local. |

**Decisão:** app-server é a superfície do primeiro adapter experimental, pela configuração efetiva reconhecida em start/resume e comunicação bidirecional, incluindo aprovações/interrupção. A interrupção cooperativa real anterior e a força local integrada atual têm evidências separadas. Essa aprovação restrita libera o início de **OX-003**, com contrato wire/provider-neutral versionado, fake executável e validação/replay nos tools experimentais. O futuro Core Rust deve consumir esse contrato; não há crate/Core implementado, Cargo/rustc não estão no PATH e não foram instalados.

OX-001 permanece aberto para diagnóstico completo/paginação e OAuth próprio **INT-10**. OX-002/M0, supervisor Rust de produção, recuperação durável, isolamento de contas e sandbox completo não se encerram com o primeiro runtime aprovado. Job Object continua distinto de sandbox de arquivos/rede. Os **5/5 turnos da nova autorização foram usados**; não iniciar nova inferência ou retry com base nesse registro.
