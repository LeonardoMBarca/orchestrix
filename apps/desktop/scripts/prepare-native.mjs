import {spawnSync} from 'node:child_process';
import {copyFileSync,mkdirSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
import {loadedBinaryDiagnostic,ownBinaryPaths,parseOwnedProcesses,windowsProcessQuery} from './native-preflight.mjs';

// Build only Orchestrix-owned helpers. Reference repositories are never executed.
const root=fileURLToPath(new URL('../../../',import.meta.url));
const desktop=fileURLToPath(new URL('../',import.meta.url));
const release=process.argv.includes('--release');
const profile=release?'release':'debug';
const cargo=process.platform==='win32'?path.join(process.env.USERPROFILE,'.cargo','bin','cargo.exe'):'cargo';
function run(args){const result=spawnSync(cargo,args,{cwd:root,stdio:'inherit',windowsHide:true});if(result.status!==0)process.exit(result.status??1);}
const host=spawnSync(cargo,['-vV'],{cwd:root,encoding:'utf8',windowsHide:true});
if(host.status!==0)throw new Error('cargo-host-unavailable');
const triple=host.stdout.match(/^host:\s*(\S+)$/m)?.[1];if(!triple)throw new Error('cargo-host-schema');
if(process.platform==='win32'){
  const allowed=ownBinaryPaths(root,desktop,triple);
  const query=spawnSync('powershell.exe',['-NoProfile','-NonInteractive','-Command',windowsProcessQuery],{cwd:root,encoding:'utf8',windowsHide:true,timeout:10000,maxBuffer:1048576,env:{...process.env,ORCHESTRIX_NATIVE_PREFLIGHT_PATHS:JSON.stringify(allowed)}});
  if(query.error||query.status!==0)throw new Error('native-preflight-unavailable: could not inspect exact owned binary paths; no build/staging started');
  const diagnostic=loadedBinaryDiagnostic(parseOwnedProcesses(query.stdout,allowed,root));
  if(diagnostic){console.error(diagnostic);process.exit(1);}
}
run(['build','-p','orchestrix-core','--bins','--locked','--offline',...(release?['--release']:[])]);
const suffix=process.platform==='win32'?'.exe':'';
const binaries=path.join(desktop,'src-tauri','binaries');mkdirSync(binaries,{recursive:true});
for(const name of ['orchestrix','orchestrix-supervisor']){
  copyFileSync(path.join(root,'target',profile,name+suffix),path.join(binaries,`${name}-${triple}${suffix}`));
}
console.log(`Staged ${profile} Core and supervisor for ${triple}`);
