import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {loadedBinaryDiagnostic,ownBinaryPaths,parseOwnedProcesses,windowsProcessQuery} from '../scripts/native-preflight.mjs';

const root='C:\\Users\\A developer\\orchestrix',desktop=root+'\\apps\\desktop';
const allowed=ownBinaryPaths(root,desktop,'x86_64-pc-windows-msvc');
const executable=root+'\\target\\debug\\orchestrix.exe';
const database=root+'\\artifacts\\native-desktop\\qa-1792000000000-321\\desktop-client\\orchestrix.sqlite';
const row=(extra={})=>({ProcessId:123,ExecutablePath:executable,CommandLine:`"${executable}" daemon --database "${database}"`,...extra});

test('allowlist contains only exact owned Debug/Release and staged binary paths',()=>{
 assert.equal(allowed.length,8);assert.equal(new Set(allowed).size,8);
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
 assert.throws(()=>parseOwnedProcesses(JSON.stringify(Array.from({length:129},()=>row())),allowed,root),/response-schema/);
 assert.throws(()=>parseOwnedProcesses(JSON.stringify(row({ProcessId:0})),allowed,root),/process-id/);
 assert.deepEqual(parseOwnedProcesses('[]',allowed,root),[]);
 assert.match(windowsProcessQuery,/exact\.Contains/);assert.ok(!windowsProcessQuery.includes('Stop-Process'));
});

test('Windows PowerShell filters each allowlisted path before returning process metadata',{skip:process.platform!=='win32'},()=>{
 // Run the actual PowerShell query against synthetic CIM rows: no unrelated
 // process command lines are read while reproducing the PS 5.1 array behavior.
 const fixture=String.raw`function Get-CimInstance {
  [pscustomobject]@{ProcessId=123;ExecutablePath=$env:ORCHESTRIX_PREFLIGHT_TEST_EXE;CommandLine='owned fixture'}
  [pscustomobject]@{ProcessId=456;ExecutablePath='C:\unrelated\other.exe';CommandLine='unrelated fixture'}
 }
 `;
 const execute=value=>spawnSync('powershell.exe',['-NoProfile','-NonInteractive','-Command',fixture+windowsProcessQuery],{windowsHide:true,encoding:'utf8',timeout:10000,maxBuffer:1048576,env:{...process.env,ORCHESTRIX_NATIVE_PREFLIGHT_PATHS:JSON.stringify(value),ORCHESTRIX_PREFLIGHT_TEST_EXE:executable}});
 const result=execute(allowed);
 assert.equal(result.error,undefined);assert.equal(result.status,0,result.stderr);
 assert.deepEqual(parseOwnedProcesses(result.stdout,allowed,root),[{pid:123,executable,qaDatabase:null}]);
 assert.ok(!result.stdout.includes('unrelated'),'filtering occurs inside PowerShell before serialization');
 for(const malformed of [[],{},['valid',null]])assert.notEqual(execute(malformed).status,0,'invalid allowlist fails closed');
});
