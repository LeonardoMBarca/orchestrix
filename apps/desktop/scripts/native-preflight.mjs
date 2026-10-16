import path from 'node:path';

const win=path.win32;
const identity=value=>win.normalize(value).toLowerCase();

export function ownBinaryPaths(root,desktop,triple){
 if(!win.isAbsolute(root)||!win.isAbsolute(desktop)||!/^[a-z0-9_.-]{1,120}$/i.test(triple))throw Error('native-preflight-path-schema');
 const result=[];
 for(const profile of ['debug','release'])for(const name of ['orchestrix-desktop','orchestrix','orchestrix-supervisor'])result.push(win.join(root,'target',profile,`${name}.exe`));
 for(const name of ['orchestrix','orchestrix-supervisor'])result.push(win.join(desktop,'src-tauri','binaries',`${name}-${triple}.exe`));
 return result;
}

// Read-only metadata. The exact-path allowlist is applied before serializing
// anything; process names, other command lines and credentials are not logged.
export const windowsProcessQuery=String.raw`
$ErrorActionPreference = 'Stop'
$allowed = ConvertFrom-Json -InputObject $env:ORCHESTRIX_NATIVE_PREFLIGHT_PATHS
if ($allowed -isnot [System.Array] -or $allowed.Count -eq 0 -or $allowed.Count -gt 32) { throw 'native-preflight-allowlist-schema' }
$exact = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
# Windows PowerShell 5.1 returns a JSON array as one pipeline object.
# Wrapping ConvertFrom-Json in @() nests it and joins every path into one key.
foreach ($binary in $allowed) {
  if ($binary -isnot [string] -or [string]::IsNullOrWhiteSpace($binary)) { throw 'native-preflight-allowlist-schema' }
  [void]$exact.Add($binary)
}
$owned = @(Get-CimInstance -ClassName Win32_Process -OperationTimeoutSec 5 | Where-Object { $_.ExecutablePath -and $exact.Contains([string]$_.ExecutablePath) } | Select-Object ProcessId,ExecutablePath,CommandLine)
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
  if(typeof row?.ExecutablePath!=='string'||!win.isAbsolute(row.ExecutablePath)||!exact.has(identity(row.ExecutablePath)))continue;
  if(!Number.isSafeInteger(row.ProcessId)||row.ProcessId<1||row.ProcessId>4294967295)throw Error('native-preflight-process-id');
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
