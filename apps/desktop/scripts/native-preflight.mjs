import path from 'node:path';
import childProcess from 'node:child_process';
import {performance} from 'node:perf_hooks';

const win=path.win32;
const identity=value=>win.normalize(value).toLowerCase();

export function ownBinaryPaths(root,desktop,triple){
 if(!win.isAbsolute(root)||!win.isAbsolute(desktop)||!/^[a-z0-9_.-]{1,120}$/i.test(triple))throw Error('native-preflight-path-schema');
 const result=[];
 for(const profile of ['debug','release'])for(const name of ['orchestrix-desktop','orchestrix','orchestrix-supervisor'])result.push(win.join(root,'target',profile,`${name}.exe`));
 for(const name of ['orchestrix','orchestrix-supervisor'])result.push(win.join(desktop,'src-tauri','binaries',`${name}-${triple}.exe`));
 for(const profile of ['debug','release'])for(const name of ['orchestrix','orchestrix-supervisor','orchestrix-runtime-fixture','orchestrix-startup-fixture'])result.push(win.join(root,'target','native-sidecars',profile,`${name}.exe`));
 return result;
}

// Read-only metadata. The exact-path allowlist is applied before serializing
// anything; process names, other command lines and credentials are not logged.
export const windowsProcessQuery=String.raw`
$ErrorActionPreference = 'Stop'
$allowed = ConvertFrom-Json -InputObject $env:ORCHESTRIX_NATIVE_PREFLIGHT_PATHS
if ($allowed -isnot [System.Array] -or $allowed.Count -eq 0 -or $allowed.Count -gt 32) { throw 'native-preflight-allowlist-schema' }
$exact = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
$criteria = [System.Collections.Generic.List[string]]::new()
# Windows PowerShell 5.1 returns a JSON array as one pipeline object.
# Wrapping ConvertFrom-Json in @() nests it and joins every path into one key.
foreach ($binary in $allowed) {
  if ($binary -isnot [string] -or [string]::IsNullOrWhiteSpace($binary)) { throw 'native-preflight-allowlist-schema' }
  [void]$exact.Add($binary)
  # WQL literals use backslash escaping. Paths remain data with equality;
  # no wildcard, class, projection or executable comes from the allowlist.
  $literal = $binary.Replace('\','\\').Replace("'","\'").Replace('"','\"')
  [void]$criteria.Add("ExecutablePath = '" + $literal + "'")
}
$query = 'SELECT ProcessId,ExecutablePath,CommandLine FROM Win32_Process WHERE ' + [string]::Join(' OR ',$criteria)
$owned = @(Get-CimInstance -Query $query -OperationTimeoutSec 5 | Where-Object { $_.ExecutablePath -and $exact.Contains([string]$_.ExecutablePath) } | Select-Object ProcessId,ExecutablePath,CommandLine)
ConvertTo-Json -InputObject $owned -Depth 3 -Compress
`;

function argumentsOf(command){
 if(typeof command!=='string'||command.length>32768)return null;
 const args=[];let i=0;
 while(i<command.length){
  while(/\s/.test(command[i]??'')&&i<command.length)i++;
  if(i===command.length)break;
  let text='',quoted=false;
  while(i<command.length&&(quoted||!/\s/.test(command[i]))){
   let slashes=0;while(command[i]==='\\'){slashes++;i++;}
   if(command[i]==='"'){
    text+='\\'.repeat(Math.floor(slashes/2));
    if(slashes%2)text+='"';else if(quoted&&command[i+1]==='"'){text+='"';i++;}else quoted=!quoted;
    i++;
   }else{text+='\\'.repeat(slashes);if(i<command.length&&(quoted||!/\s/.test(command[i])))text+=command[i++];}
  }
  if(quoted)return null;
  args.push(text);
 }
 return args;
}

function qaDatabase(root,executable,command){
 const args=argumentsOf(command);
 if(!args||identity(args[0]??'')!==identity(executable)||args[1]!=='daemon')return null;
 const databases=[];
 for(let i=2;i<args.length;i++){
  if(args[i]==='--database'){if(!args[i+1])return null;databases.push(args[++i]);}
  else if(args[i].startsWith('--database='))databases.push(args[i].slice('--database='.length));
 }
 if(databases.length!==1||!win.isAbsolute(databases[0]))return null;
 const database=win.normalize(databases[0]);
 const relative=win.relative(win.join(root,'artifacts','native-desktop'),database).split('\\');
 return relative.length===3&&/^qa-\d+-\d+$/.test(relative[0])&&relative[1].toLowerCase()==='desktop-client'&&relative[2].toLowerCase()==='orchestrix.sqlite'?database:null;
}

export function parseOwnedProcesses(output,allowed,root){
 let decoded;try{decoded=JSON.parse(output.replace(/^\uFEFF/,''));}catch{throw Error('native-preflight-response-json');}
 const rows=Array.isArray(decoded)?decoded:decoded&&typeof decoded==='object'?[decoded]:null;
 if(!rows||rows.length>128)throw Error('native-preflight-response-schema');
 const exact=new Set(allowed.map(identity)),result=[];
 for(const row of rows){
  if(!row||typeof row!=='object'||Array.isArray(row)||!['ProcessId','ExecutablePath','CommandLine'].every(key=>Object.hasOwn(row,key))||(row.ExecutablePath!==null&&typeof row.ExecutablePath!=='string')||(row.CommandLine!==null&&typeof row.CommandLine!=='string'))throw Error('native-preflight-response-schema');
  if(!Number.isSafeInteger(row.ProcessId)||row.ProcessId<1||row.ProcessId>4294967295)throw Error('native-preflight-process-id');
  if(row.ExecutablePath===null||!win.isAbsolute(row.ExecutablePath)||!exact.has(identity(row.ExecutablePath)))continue;
  const executable=win.normalize(row.ExecutablePath);
  result.push({pid:row.ProcessId,executable,qaDatabase:qaDatabase(root,executable,row.CommandLine)});
 }
 return result.sort((a,b)=>a.pid-b.pid);
}

const psQuote=text=>`'${text.replaceAll("'","''")}'`;
export function loadedBinaryDiagnostic(processes){
 if(!processes.length)return '';
 const lines=['Orchestrix native build/staging is blocked by loaded owned binaries.','Close the listed Desktop window and inspect active work before stopping its daemon. No process has been terminated.'];
 for(const process of processes){
  lines.push(`PID ${process.pid}: ${process.executable}`);
  if(process.qaDatabase)lines.push(`  Identified QA daemon only; explicit graceful Shutdown after inspecting it:\n  & ${psQuote(process.executable)} shutdown --database ${psQuote(process.qaDatabase)} --command-id ${psQuote(`prepare-native-shutdown-${process.pid}`)}`);
  else lines.push('  Close its owned window, or use the identified daemon\'s explicit Cancel/Shutdown and wait for its own jobs to end.');
 }
 lines.push('Then rerun this command. Do not terminate processes by name or request administrator access.');
 return lines.join('\n');
}


// These are separate host-observation budgets. PowerShell must first reach its
// body; the query then retains its existing 10 s budget (including CIM's 5 s).
// Neither the marker nor elapsed time establishes that metadata is trustworthy.
export const PREFLIGHT_STARTUP_MS=30000;
export const PREFLIGHT_QUERY_MS=10000;
export const PREFLIGHT_STDOUT_BYTES=1048576;
const STDERR_BYTES=1024,CLOSE_MS=1000;
const readyMarker='ORCHESTRIX_PREFLIGHT_READY_V1';
export const windowsProcessCommand="[Console]::Error.WriteLine('"+readyMarker+"')\n"+windowsProcessQuery;
const failure=code=>Object.assign(new Error(code),{code});

export async function collectOwnedProcesses(allowed,root,{signal}={}){
 if(process.platform!=='win32')throw failure('native-preflight-platform');
 if(!win.isAbsolute(root)||!Array.isArray(allowed)||!allowed.length||allowed.length>32||allowed.some(value=>typeof value!=='string'||!win.isAbsolute(value)||value.length>32768))throw failure('native-preflight-allowlist-schema');
 const paths=allowed.map(value=>win.normalize(value));
 if(new Set(paths.map(identity)).size!==paths.length)throw failure('native-preflight-allowlist-schema');
 if(signal?.aborted)throw failure('native-preflight-cancelled');
 return new Promise((resolve,reject)=>{
  let child,stage='startup',ready=false,settled=false,problem=null,stdoutBytes=0,stderrBytes=0,stderrLine='';
  let phaseDeadline=performance.now()+PREFLIGHT_STARTUP_MS,phaseTimer,closeTimer;
  const stdout=[];
  function cleanup(){clearTimeout(phaseTimer);clearTimeout(closeTimer);signal?.removeEventListener('abort',cancel);}
  function finish(error,value){if(settled)return;settled=true;cleanup();if(error)reject(error);else resolve(value);}
  function stop(code){
   if(settled||problem)return;
   problem=failure(code);clearTimeout(phaseTimer);
   if(!child){finish(problem);return;}
   try{child.kill();}catch{}
   closeTimer=setTimeout(()=>{
    // A failed stop never means that the process ended. Release bounded local
    // readers and report this uncertainty instead of accepting partial output.
    child.stdout?.destroy();child.stderr?.destroy();child.unref();
    finish(failure('native-preflight-stop-unconfirmed'));
   },CLOSE_MS);
  }
  function cancel(){stop('native-preflight-cancelled');}
  function deadline(){stop(stage==='startup'?'native-preflight-startup-timeout':'native-preflight-query-timeout');}
  phaseTimer=setTimeout(deadline,PREFLIGHT_STARTUP_MS);
  signal?.addEventListener('abort',cancel,{once:true});
  try{
   child=childProcess.spawn('powershell.exe',['-NoProfile','-NonInteractive','-Command',windowsProcessCommand],{
    windowsHide:true,stdio:['ignore','pipe','pipe'],env:{...process.env,ORCHESTRIX_NATIVE_PREFLIGHT_PATHS:JSON.stringify(paths)}
   });
  }catch{finish(failure('native-preflight-spawn'));return;}
  if(performance.now()>phaseDeadline)stop('native-preflight-startup-timeout');
  child.once('error',()=>stop('native-preflight-spawn'));
  child.stdout.on('data',bytes=>{
   if(problem||settled)return;
   stdoutBytes+=bytes.length;
   if(stdoutBytes>PREFLIGHT_STDOUT_BYTES){stop('native-preflight-output-budget');return;}
   stdout.push(bytes);
  });
  child.stderr.on('data',bytes=>{
   if(problem||settled)return;
   stderrBytes+=bytes.length;
   if(stderrBytes>STDERR_BYTES){stop('native-preflight-stderr-budget');return;}
   stderrLine+=bytes.toString('utf8');
   let end;
   while((end=stderrLine.indexOf('\n'))!==-1){
    const line=stderrLine.slice(0,end).replace(/\r$/,'');stderrLine=stderrLine.slice(end+1);
    if(line!==readyMarker)continue; // Never expose arbitrary provider/OS errors.
    if(ready){stop('native-preflight-readiness-schema');return;}
    if(performance.now()>phaseDeadline){stop('native-preflight-startup-timeout');return;}
    ready=true;stage='query';clearTimeout(phaseTimer);
    phaseDeadline=performance.now()+PREFLIGHT_QUERY_MS;phaseTimer=setTimeout(deadline,PREFLIGHT_QUERY_MS);
   }
  });
  child.once('close',code=>{
   if(settled)return;
   if(problem){finish(problem);return;}
   if(code!==0){finish(failure('native-preflight-process-exit'));return;}
   if(!ready){finish(failure('native-preflight-readiness-unconfirmed'));return;}
   if(performance.now()>phaseDeadline){finish(failure('native-preflight-query-timeout'));return;}
   try{finish(null,parseOwnedProcesses(Buffer.concat(stdout,stdoutBytes).toString('utf8'),paths,root));}
   catch{finish(failure('native-preflight-response-schema'));}
  });
 });
}
