import test from 'node:test';
import assert from 'node:assert/strict';
import {observeOwnedProcessStop,runWorktreeProbe} from '../worktree-probe.mjs';
import {cleanWindowsJobExit} from '../src/windows-job-process.mjs';

function state(){return {ownershipConfirmed:true,parentExited:true,code:137,treeTermination:'confirmed',activeCount:0,stdioClosed:true,termination:'forced',helper:{parentExited:true,stdioClosed:true,code:0},helperStreams:{faulted:false,stdinClosed:true,stdoutClosed:true,stderrClosed:true}};}
function session(){const s={turns:new Map(),waitForTurnStart:async()=>({started:true}),calls:0};s.client={windowsJob:true,terminateOwnedJob:async()=>{s.calls++;return state();}};return s;}
test('forced proof requires owned Job before observing a turn or sending stop',async()=>{
  const s=session();s.client.windowsJob=false;await assert.rejects(observeOwnedProcessStop(s,'turn'),error=>error.kind==='supervision');assert.equal(s.calls,0);
});
test('a turn terminal before observed start cannot request process force',async()=>{
  const s=session();s.waitForTurnStart=async()=>({started:false,terminal:{status:'completed'}});
  const result=await observeOwnedProcessStop(s,'turn');assert.equal(result.forcedConfirmed,false);assert.equal(result.terminalStatus,'completed');assert.equal(s.calls,0);
});
test('a previously started but already completed turn cannot request force as active cancellation',async()=>{
  const s=session();s.turns.set('turn',{status:'completed'});const result=await observeOwnedProcessStop(s,'turn');
  assert.equal(result.startObserved,true);assert.equal(result.forcedConfirmed,false);assert.equal(s.calls,0);
});
test('a terminal racing with process stop preserves tree proof and rejects active-turn cancellation proof',async()=>{
  const s=session();s.client.terminateOwnedJob=async()=>{s.calls++;s.turns.set('turn',{status:'completed'});return state();};
  const result=await observeOwnedProcessStop(s,'turn');assert.equal(result.processTerminationConfirmed,true);assert.equal(result.forcedConfirmed,false);assert.equal(result.terminalStatus,'completed');assert.equal(result.turnInterruptedConfirmed,false);assert.equal(s.calls,1);
});
test('unknown tree or incomplete byte streams never pass explicit forced proof',async()=>{
  for(const mutation of [value=>value.treeTermination='unknown',value=>value.helper.code=3,value=>value.stdioClosed=false,value=>value.helperStreams.faulted=true]) {
    const s=session();s.client.terminateOwnedJob=async()=>{const value=state();mutation(value);return value;};
    const result=await observeOwnedProcessStop(s,'turn');assert.equal(result.forcedConfirmed,false);assert.equal(result.processTerminationConfirmed,false);assert.equal(result.turnInterruptedConfirmed,false);
  }
});
test('process-stop opt-in without Job or with implicit live rejects before creating fixture',async()=>{
  for(const options of [{processStop:true,windowsJob:false},{live:'true'},{processStop:'true',windowsJob:true}]) {
    const report=await runWorktreeProbe(options);assert.equal(report.status,'failed');assert.equal(report.error.kind,'arguments');assert.equal(report.workspace,undefined);assert.equal(report.liveStarted,false);
  }
});
test('normal success cannot accept forced root exit or helper-only tree evidence',()=>{
  assert.equal(cleanWindowsJobExit(state()),false);assert.equal(cleanWindowsJobExit(state(),137),true);
  for(const mutation of [value=>value.parentExited=false,value=>value.ownershipConfirmed=false,value=>value.activeCount=1,value=>value.helper.parentExited=false,value=>value.helperStreams.stdoutClosed=false]){const value=state();mutation(value);assert.equal(cleanWindowsJobExit(value,137),false);}
});
