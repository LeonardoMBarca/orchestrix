import test from 'node:test';
import assert from 'node:assert/strict';
import {runProbe} from '../probe.mjs';
import {RuntimeContractObserver} from '../src/runtime-contract.mjs';
import {collectCodexObservation} from '../src/codex-contract.mjs';

function consume(descriptor,snapshot) {
  const observer=new RuntimeContractObserver({descriptor,request:{version:snapshot.version,...snapshot.scope,requested:snapshot.requested}});
  for(const event of snapshot.events)observer.accept(JSON.stringify(event),{origin:event.origin});
  return observer.snapshot();
}

for(const [scenario,status,outcome] of [['success','passed','completed'],['hang','passed','interrupted'],['rate-limit','failed','unknown'],['malformed','failed','unknown']]) {
  test(`the exported harness retains a consumable normalized ${scenario} report and shutdown evidence`,async()=>{
    const report=await runProbe({mode:'fake',scenario,requestTimeoutMs:2000,turnTimeoutMs:2000});
    assert.equal(report.status,status);assert.equal(report.liveStarted,false);assert.equal(report.process.parentExited,true);
    assert.equal(report.workspace.filesUnchanged,true);assert.equal(report.normalized.attempts.length,1);
    const snapshot=report.normalized.attempts[0],replayed=consume(report.normalized.descriptor,snapshot);
    assert.equal(snapshot.outcome,outcome);assert.equal(replayed.outcome,outcome);assert.deepEqual(replayed.process,snapshot.process);
    assert.equal(snapshot.process.runtime.exited,true);assert.equal(snapshot.process.tree.termination,'unknown');
    assert.equal(JSON.stringify(report.normalized).includes('turn-fake-001'),false);
    if(scenario==='hang'){assert.equal(snapshot.interrupt.requested,true);assert.equal(snapshot.interrupt.acknowledged,true);}
    if(scenario==='rate-limit')assert.ok(snapshot.events.some(event=>event.error?.code==='rate-limit'));
    if(scenario==='malformed')assert.equal(snapshot.terminalConfirmed,false);
  });
}

test('invalid imported harness options fail before workspace or provider process creation',async()=>{
  for(const options of [{mode:'other',live:false},{mode:'codex',live:'false'},{mode:'codex',live:1}]) {
    const report=await runProbe({...options,binary:'NEVER_LAUNCH_INVALID_OPTIONS'});
    assert.equal(report.status,'failed');assert.equal(report.error.kind,'arguments');assert.equal(report.liveStarted,false);
    assert.equal(report.workspace,undefined);assert.equal(report.process,undefined);assert.deepEqual(report.events,[]);
  }
});

test('report capture distinguishes normalization failure from already confirmed process exit and disposes once',()=>{
  let disposed=0;
  const state={parentExited:true,code:0,treeTermination:'confirmed'};
  const adapter={
    descriptor:{version:{major:1,minor:1}},diagnostics:{unscopedNotifications:0},
    observeProcess:observed=>{assert.equal(observed,state);throw Object.assign(new Error('SYNTHETIC_PRIVATE_MESSAGE'),{code:'protocol'});},
    snapshots:()=>[{process:{runtime:{exited:true,code:0}},outcome:'completed'}],dispose:()=>disposed++,
  };
  const report=collectCodexObservation(adapter,state);
  assert.equal(report.normalizationError,'protocol');assert.equal(report.attempts[0].process.runtime.code,0);
  assert.equal(report.attempts[0].outcome,'completed');assert.equal(disposed,1);
  assert.equal(JSON.stringify(report).includes('SYNTHETIC_PRIVATE_MESSAGE'),false);
});
