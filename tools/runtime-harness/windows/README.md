# Windows Job Object — spike sintético

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
