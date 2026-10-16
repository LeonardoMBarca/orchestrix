import {test} from 'node:test';
import assert from 'node:assert/strict';
import childProcess from 'node:child_process';
import {EventEmitter,once} from 'node:events';
import {PassThrough} from 'node:stream';
import {loadedBinaryDiagnostic,ownBinaryPaths,parseOwnedProcesses,windowsProcessQuery,windowsProcessCommand,collectOwnedProcesses,PREFLIGHT_STARTUP_MS,PREFLIGHT_QUERY_MS,PREFLIGHT_STDOUT_BYTES} from '../scripts/native-preflight.mjs';

const root='C:\\Users\\A developer\\orchestrix',desktop=root+'\\apps\\desktop';
const allowed=ownBinaryPaths(root,desktop,'x86_64-pc-windows-msvc');
const executable=root+'\\target\\debug\\orchestrix.exe';
const database=root+'\\artifacts\\native-desktop\\qa-1792000000000-321\\desktop-client\\orchestrix.sqlite';
const row=(extra={})=>({ProcessId:123,ExecutablePath:executable,CommandLine:`"${executable}" daemon --database "${database}"`,...extra});

test('allowlist contains only exact owned Debug/Release and staged binary paths',()=>{
 assert.equal(allowed.length,16);assert.equal(new Set(allowed).size,16);
 for(const profile of ['debug','release'])for(const name of ['orchestrix','orchestrix-supervisor','orchestrix-runtime-fixture','orchestrix-startup-fixture'])assert.ok(allowed.includes(root+'\\target\\native-sidecars\\'+profile+'\\'+name+'.exe'));
 assert.ok(allowed.includes(root+'\\target\\release\\orchestrix-desktop.exe'));
 assert.throws(()=>ownBinaryPaths(root,desktop,'../escape'),/path-schema/);
 const other=[row({ExecutablePath:'C:\\somebody-else\\orchestrix.exe'}),row({ExecutablePath:executable+'.old'}),row({ExecutablePath:root+'-other\\target\\debug\\orchestrix.exe'}),row({ExecutablePath:null})];
 assert.deepEqual(parseOwnedProcesses(JSON.stringify(other),allowed,root),[]);
 const own=parseOwnedProcesses(JSON.stringify(row({ExecutablePath:executable.toUpperCase()})),allowed,root);
 assert.equal(own.length,1);assert.equal(own[0].pid,123);assert.equal(own[0].qaDatabase,database);
});

test('metadata parser identifies a scoped QA daemon without exposing arbitrary command lines',()=>{
 const processes=parseOwnedProcesses(JSON.stringify([row(),row({ProcessId:456,ExecutablePath:root+'\\target\\debug\\orchestrix-desktop.exe',CommandLine:'unrelated confidential text'})]),allowed,root);
 assert.equal(processes[0].qaDatabase,database);assert.equal(processes[1].qaDatabase,null);
 const diagnostic=loadedBinaryDiagnostic(processes);
 assert.match(diagnostic,/PID 123:/);assert.match(diagnostic,/shutdown --database/);assert.match(diagnostic,/prepare-native-shutdown-123/);
 assert.ok(!diagnostic.includes('confidential'));assert.match(diagnostic,/No process has been terminated/);
 assert.equal(loadedBinaryDiagnostic([]),'');
 for(const CommandLine of [null,`"${executable}" daemon --database "${database}`,`"${executable}" daemon --database "${database}" --database "${database}"`,`"${executable}" daemon --database "C:\\personal\\state.sqlite"`,`"${executable}" status --database "${database}"`,`"C:\\other.exe" daemon --database "${database}"`,`"${executable}" daemon --database "${root}\\artifacts\\native-desktop\\qa-123-4\\..\\..\\outside.sqlite"`]){
  assert.equal(parseOwnedProcesses(JSON.stringify(row({CommandLine})),allowed,root)[0].qaDatabase,null);
 }
});

test('malformed or unbounded process metadata blocks rather than pretending no binaries are loaded',()=>{
 assert.throws(()=>parseOwnedProcesses('not-json',allowed,root),/response-json/);
 assert.throws(()=>parseOwnedProcesses('null',allowed,root),/response-schema/);
 for(const malformed of ['{}','[{}]','[null]'])assert.throws(()=>parseOwnedProcesses(malformed,allowed,root),/response-schema/);
 assert.throws(()=>parseOwnedProcesses(JSON.stringify(Array.from({length:129},()=>row())),allowed,root),/response-schema/);
 assert.throws(()=>parseOwnedProcesses(JSON.stringify(row({ProcessId:0})),allowed,root),/process-id/);
 assert.deepEqual(parseOwnedProcesses('[]',allowed,root),[]);
 assert.match(windowsProcessQuery,/exact\.Contains/);assert.ok(!windowsProcessQuery.includes('Stop-Process'));
});


const wqlCases=[
 {
  "path": "C:\\fixture\\folder\\plain.exe",
  "literal": "'C:\\\\fixture\\\\folder\\\\plain.exe'"
 },
 {
  "path": "C:\\fixture\\O'Brien.exe",
  "literal": "'C:\\\\fixture\\\\O\\'Brien.exe'"
 },
 {
  "path": "C:\\fixture\\a\"b.exe",
  "literal": "'C:\\\\fixture\\\\a\\\"b.exe'"
 },
 {
  "path": "C:\\fixture\\app' OR ExecutablePath LIKE '%.exe",
  "literal": "'C:\\\\fixture\\\\app\\' OR ExecutablePath LIKE \\'%.exe'"
 }
];
const queryPaths=[...allowed,...wqlCases.map(value=>value.path)];
const expectedWql='SELECT ProcessId,ExecutablePath,CommandLine FROM Win32_Process WHERE '+[...allowed.map(value=>"ExecutablePath = '"+value.replaceAll('\\','\\\\')+"'"),...wqlCases.map(value=>'ExecutablePath = '+value.literal)].join(' OR ');

test('quoted and injection-shaped allowlist paths remain exact data',()=>{
 const result=parseOwnedProcesses(JSON.stringify(wqlCases.map((value,index)=>row({ProcessId:200+index,ExecutablePath:value.path,CommandLine:null}))),queryPaths,root);
 assert.deepEqual(result.map(value=>value.executable),wqlCases.map(value=>value.path));
 assert.deepEqual(parseOwnedProcesses(JSON.stringify(row({ExecutablePath:'C:\\fixture\\other.exe'})),queryPaths,root),[]);
});

const windowsOnly={skip:process.platform!=='win32'};
const marker='ORCHESTRIX_PREFLIGHT_READY_V1\n';
const realSpawn=childProcess.spawn;
function expectStaticQuery(executable,args,options){
 assert.equal(executable,'powershell.exe');
 assert.deepEqual(args,['-NoProfile','-NonInteractive','-Command',windowsProcessCommand]);
 assert.equal(options.windowsHide,true);assert.deepEqual(options.stdio,['ignore','pipe','pipe']);
}
function fakeChild(){
 const child=new EventEmitter();child.stdout=new PassThrough();child.stderr=new PassThrough();child.kills=0;
 child.kill=()=>{child.kills++;queueMicrotask(()=>child.emit('close',null));return true;};
 child.unref=()=>{};return child;
}

test('collector keeps distinct finite startup and query budgets; readiness and partial output cannot pass',windowsOnly,async t=>{
 assert.equal(PREFLIGHT_STARTUP_MS,30000);assert.equal(PREFLIGHT_QUERY_MS,10000);
 t.mock.timers.enable({apis:['setTimeout']});
 const child=fakeChild();t.mock.method(childProcess,'spawn',(...args)=>{expectStaticQuery(...args);return child;});
 const rejected=assert.rejects(collectOwnedProcesses(allowed,root),/native-preflight-query-timeout/);
 t.mock.timers.tick(PREFLIGHT_STARTUP_MS-1);assert.equal(child.kills,0);
 child.stderr.write(marker);child.stdout.write('[]');
 t.mock.timers.tick(PREFLIGHT_QUERY_MS-1);assert.equal(child.kills,0);
 t.mock.timers.tick(1);await rejected;assert.equal(child.kills,1);
});

test('collector startup times out without a marker even when partial JSON is present',windowsOnly,async t=>{
 t.mock.timers.enable({apis:['setTimeout']});const child=fakeChild();
 t.mock.method(childProcess,'spawn',(...args)=>{expectStaticQuery(...args);return child;});
 const rejected=assert.rejects(collectOwnedProcesses(allowed,root),/native-preflight-startup-timeout/);
 child.stdout.write('[]');t.mock.timers.tick(PREFLIGHT_STARTUP_MS);await rejected;assert.equal(child.kills,1);
});

test('collector requires confirmed exit and valid metadata, and enforces both stream budgets',windowsOnly,async t=>{
 let child;t.mock.method(childProcess,'spawn',(...args)=>{expectStaticQuery(...args);child=fakeChild();return child;});
 async function rejects(code,drive){const rejected=assert.rejects(collectOwnedProcesses(allowed,root),new RegExp(code));drive(child);await rejected;return child;}
 await rejects('native-preflight-readiness-unconfirmed',child=>{child.stdout.write('[]');child.emit('close',0);});
 await rejects('native-preflight-response-schema',child=>{child.stderr.write(marker);child.stdout.write('not-json');child.emit('close',0);});
 await rejects('native-preflight-process-exit',child=>{child.stderr.write(marker);child.stdout.write('[]');child.emit('close',1);});
 assert.equal((await rejects('native-preflight-output-budget',child=>{child.stderr.write(marker);child.stdout.write(Buffer.alloc(PREFLIGHT_STDOUT_BYTES+1));})).kills,1);
 assert.equal((await rejects('native-preflight-stderr-budget',child=>child.stderr.write(Buffer.alloc(1025)))).kills,1);
 assert.equal((await rejects('native-preflight-readiness-schema',child=>child.stderr.write(marker+marker))).kills,1);
 const aborted=new AbortController();aborted.abort();await assert.rejects(collectOwnedProcesses(allowed,root,{signal:aborted.signal}),/native-preflight-cancelled/);
});

test('cancel closes only the owned Node fixture and leaves another owned fixture running',{...windowsOnly,timeout:PREFLIGHT_STARTUP_MS+PREFLIGHT_QUERY_MS+5000},async t=>{
 // The production API accepts neither scripts nor executables. Only this test
 // intercepts spawn to exercise its I/O/ownership using two native Node children.
 const control=realSpawn(process.execPath,['-e',"process.stdout.write('control-ready');setInterval(()=>{},1000)"],{windowsHide:true,stdio:['ignore','pipe','ignore']});
 let child,closed=false;
 try{
  await once(control.stdout,'data');
  t.mock.method(childProcess,'spawn',(...args)=>{
   expectStaticQuery(...args);
   child=realSpawn(process.execPath,['-e',"process.stderr.write('ORCHESTRIX_PREFLIGHT_READY_V1\\n');setInterval(()=>{},1000)"],{windowsHide:true,stdio:['ignore','pipe','pipe']});
   child.once('close',()=>{closed=true;});return child;
  });
  const abort=new AbortController();const rejected=assert.rejects(collectOwnedProcesses(allowed,root,{signal:abort.signal}),/native-preflight-cancelled/);
  await once(child.stderr,'data');abort.abort();await rejected;
  assert.equal(closed,true);assert.equal(child.killed,true);
  assert.equal(control.exitCode,null);assert.equal(control.signalCode,null);
 }finally{
  if(child&&child.exitCode===null&&child.signalCode===null){child.kill();await once(child,'close');}
  if(control.exitCode===null&&control.signalCode===null){control.kill();await once(control,'close');}
 }
});

test('Windows PowerShell filters each allowlisted path before returning process metadata',{...windowsOnly,timeout:180000},async t=>{
 // The same static query runs in real PS 5.1 against synthetic CIM rows. The
 // test alone inserts this stub; production does not expose a script seam.
 const fixture=String.raw`function Get-CimInstance {
  param([string]$Query,[uint32]$OperationTimeoutSec)
  if ($OperationTimeoutSec -ne 5 -or -not [string]::Equals($Query,$env:ORCHESTRIX_PREFLIGHT_EXPECTED_WQL,[System.StringComparison]::Ordinal)) { throw 'native-preflight-fixture-query-mismatch' }
  [pscustomobject]@{ProcessId=123;ExecutablePath=$env:ORCHESTRIX_PREFLIGHT_TEST_EXE;CommandLine='owned fixture'}
  [pscustomobject]@{ProcessId=456;ExecutablePath='C:\unrelated\other.exe';CommandLine='unrelated fixture'}
 }
 `;
 let input=queryPaths;const executions=[];
 t.mock.method(childProcess,'spawn',(exe,args,options)=>{
  expectStaticQuery(exe,args,options);
  const command=args[3].replace(windowsProcessQuery,fixture+windowsProcessQuery);
  const child=realSpawn(exe,[...args.slice(0,3),command],{...options,env:{...options.env,ORCHESTRIX_NATIVE_PREFLIGHT_PATHS:JSON.stringify(input),ORCHESTRIX_PREFLIGHT_TEST_EXE:executable,ORCHESTRIX_PREFLIGHT_EXPECTED_WQL:expectedWql}});
  const execution={stdout:'',status:null,error:undefined};executions.push(execution);
  child.stdout.on('data',data=>{execution.stdout+=data.toString('utf8');});child.once('error',error=>{execution.error=error;});child.once('close',status=>{execution.status=status;});return child;
 });
 const result=await collectOwnedProcesses(queryPaths,root);
 assert.equal(executions[0].error,undefined);assert.equal(executions[0].status,0);
 assert.deepEqual(result,[{pid:123,executable,qaDatabase:null}]);
 assert.ok(!executions[0].stdout.includes('unrelated'),'filtering occurs inside PowerShell before serialization');
 for(const malformed of [[],{},['valid',null]]){
  input=malformed;await assert.rejects(collectOwnedProcesses(queryPaths,root),/native-preflight-process-exit/);
  assert.ok(Number.isInteger(executions.at(-1).status)&&executions.at(-1).status!==0,'invalid allowlist fails closed in real PowerShell');
 }
});
