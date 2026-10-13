import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile,mkdtemp,rm} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {dirname,join} from 'node:path';
import {tmpdir} from 'node:os';
import {fileURLToPath} from 'node:url';
import {spawnSync} from 'node:child_process';
import {replayCodexReport,replayCodexFixture,codexReplayFacts,consumeCodexReplayAttempt,runCodexReplay} from '../codex-replay.mjs';

const root=join(dirname(fileURLToPath(import.meta.url)),'..');
const fixturePath=join(root,'fixtures/codex-contract-replay.json');
const fixture=JSON.parse(await readFile(fixturePath,'utf8'));
const referencePaths={
  'job-live':join(root,'../../docs/research/evidence/codex-m0-job-live.json'),
  'mxc-live':join(root,'../../docs/research/evidence/codex-m0-mxc-live.json'),
};
const references=Object.fromEntries(await Promise.all(Object.entries(referencePaths).map(async([name,path])=>[name,JSON.parse(await readFile(path,'utf8'))])));
const clone=value=>JSON.parse(JSON.stringify(value));
const errorCode=expected=>error=>error?.code===expected;
const synthetic=name=>clone(fixture.cases.find(item=>item.name===name));
const replay=item=>replayCodexReport(item.source.report,{sourceKind:'synthetic',processAssignments:item.processAssignments??[]});
const basic=()=>({runtimeVersion:'synthetic-replay',events:[{sequence:1,type:'session.started',threadId:'threadId-1'},{sequence:2,type:'attempt.turn.started',turnId:'turnId-2'}],processes:[]});
const semantic=snapshot=>{const {budget,...state}=snapshot;return state;};

test('portable JSON fixtures verify both recorded reports and all synthetic expectations offline',()=>{
  const suite=replayCodexFixture(fixture,references);
  assert.equal(suite.status,'passed');assert.equal(suite.validationKind,'offline-replay');assert.equal(suite.dispatches,0);
  assert.equal(suite.cases.length,5);
  assert.deepEqual(suite.cases.map(item=>item.name),fixture.cases.map(item=>item.name));
  assert.equal(suite.cases.filter(item=>item.replay.sourceKind==='recorded').length,2);
});

test('recorded start and restart share only local session/model references and preserve unknown catalogs',()=>{
  for(const report of Object.values(references)) {
    const result=replayCodexReport(report);
    assert.equal(result.sessionCount,1);assert.equal(result.attemptCount,3);assert.equal(result.dispatches,0);
    assert.deepEqual(result.attempts.map(item=>item.snapshot.scope.sessionId),['session-1','session-1','session-1']);
    assert.deepEqual(result.attempts.map(item=>item.snapshot.scope.turnId),['turn-1','turn-2','turn-3']);
    assert.deepEqual(result.attempts.map(item=>item.snapshot.scope.attemptId),['attempt-1','attempt-2','attempt-3']);
    for(const {descriptor,snapshot} of result.attempts) {
      assert.equal(descriptor.catalog.completeness,'unknown');assert.equal(descriptor.models[0].reasoningLevels,null);
      assert.equal(descriptor.models[0].availability,'unknown');assert.equal(snapshot.admissionObserved,false);
      assert.equal(snapshot.effective.model.value,'model-1');
      assert.deepEqual(descriptor.contextOwnership,{conversation:'runtime',projectMemory:'orchestrix'});
      assert.equal(descriptor.capabilities.quota.support,'unknown');assert.equal(descriptor.capabilities.nativeDelegation.support,'unknown');
    }
  }
});

test('recorded process arrays never acquire ownership by position or process-count coincidence',()=>{
  for(const report of Object.values(references)) {
    const result=replayCodexReport(report);
    assert.equal(result.unassignedProcessObservations,report.processes.length);assert.equal(result.assignedProcessObservations,0);
    for(const {snapshot} of result.attempts) {
      assert.equal(snapshot.process.runtime.exited,false);assert.equal(snapshot.process.helper.exited,false);
      assert.equal(snapshot.process.tree.ownership,'unknown');assert.equal(snapshot.process.tree.termination,'unknown');
      assert.equal(snapshot.events.some(event=>event.origin==='supervisor'),false);
    }
    assert.throws(()=>replayCodexReport(report,{processAssignments:[{attemptId:'attempt-1',processIndex:0}]}),errorCode('ownership'));
  }
});

test('recorded owned-job stop remains unknown remote terminal without manufacturing interruption',()=>{
  const result=replayCodexReport(references['job-live']);
  assert.deepEqual(result.attempts.slice(0,2).map(item=>item.snapshot.outcome),['completed','completed']);
  const final=result.attempts[2].snapshot;
  assert.equal(final.outcome,'unknown');assert.equal(final.terminalConfirmed,false);
  assert.deepEqual(final.interrupt,{requested:false,acknowledged:false});
});

test('legacy interrupt ACK is replayed as acknowledgement without inventing an observed send',()=>{
  for(const result of [replayCodexReport(references['mxc-live']),replay(synthetic('synthetic-ack-without-sent-record'))]) {
    const snapshot=result.attempts.at(-1).snapshot;
    assert.equal(snapshot.outcome,'interrupted');assert.equal(snapshot.terminalConfirmed,true);
    assert.deepEqual(snapshot.interrupt,{requested:false,acknowledged:true});
    assert.equal(snapshot.events.some(event=>event.type==='attempt.interrupt-requested'),false);
    assert.equal(snapshot.events.some(event=>event.type==='attempt.interrupt-result'&&event.acknowledged===true),true);
  }
});

test('absent historical intent is labelled contract defaults and effective settings stay unknown',()=>{
  const result=replay(synthetic('synthetic-terminal-without-start-or-config'));
  const {snapshot,replayEvidence}=result.attempts[0];
  assert.equal(replayEvidence.intentSource,'contract-defaults');assert.deepEqual(replayEvidence.requestedConfigurationFields,[]);
  assert.equal(replayEvidence.defaultsAreHistoricalEvidence,false);assert.equal(replayEvidence.accountBinding,'not-recorded');
  assert.deepEqual(replayEvidence.defaultedIntentFields,['strategy','presetRef','modelPolicy','modelId','reasoning','sandbox','network','approval']);
  assert.equal(snapshot.requested.sandbox,'read-only');assert.equal(snapshot.requested.reasoning,'auto');
  assert.equal(snapshot.admissionObserved,false);assert.equal(snapshot.startObserved,false);
  for(const setting of Object.values(snapshot.effective))assert.equal(setting.state,'unknown');
});

test('requested configuration and resolved observations remain distinct without expanding capability evidence',()=>{
  const report=basic();
  report.configuration={requested:{model:'NATIVE_REQUESTED_PRIVATE',effort:'low',sandbox:'read-only',networkAccess:false,approval:'never'},resolvedConfiguration:{model:'NATIVE_RESOLVED_PRIVATE',effort:'high',sandbox:'workspaceWrite',networkAccess:true,approval:'never'}};
  const {descriptor,snapshot,replayEvidence}=replayCodexReport(report,{sourceKind:'synthetic'}).attempts[0];
  assert.notEqual(snapshot.requested.modelId,snapshot.effective.model.value);
  assert.equal(snapshot.requested.reasoning,'low');assert.equal(snapshot.effective.reasoning.value,'high');
  assert.equal(snapshot.requested.sandbox,'read-only');assert.equal(snapshot.effective.sandbox.value,'workspace-write');
  assert.equal(snapshot.requested.network,'disabled');assert.equal(snapshot.effective.network.value,'enabled');
  assert.equal(descriptor.models[0].reasoningLevels,null);assert.equal(descriptor.catalog.completeness,'unknown');
  assert.deepEqual(replayEvidence.defaultedIntentFields,['strategy','presetRef']);
});

test('missing resumed configuration does not inherit a historical attachment configuration',()=>{
  const report=clone(references['job-live']);delete report.resumedConfiguration;
  const result=replayCodexReport(report);
  assert.equal(result.attempts[0].snapshot.effective.model.state,'known');
  assert.equal(result.attempts[1].snapshot.effective.model.state,'unknown');
  assert.equal(result.attempts[1].replayEvidence.intentSource,'contract-defaults');
  assert.equal(result.attempts[1].snapshot.scope.sessionId,result.attempts[0].snapshot.scope.sessionId);
});

test('attachment identities reject a conflicting thread, raw native ID, or alias reused after attachment change',()=>{
  const mismatch=basic();mismatch.events[1].threadId='threadId-2';assert.throws(()=>replayCodexReport(mismatch),errorCode('ownership'));
  const raw=basic();raw.events[0].threadId='PRIVATE_NATIVE_UUID';assert.throws(()=>replayCodexReport(raw),errorCode('ownership'));
  const reused=basic();reused.events.push({sequence:3,type:'session.resumed',threadId:'threadId-1'},{sequence:4,type:'attempt.turn.ended',turnId:'turnId-2',status:'completed'});
  assert.throws(()=>replayCodexReport(reused),errorCode('ownership'));
  const detached=basic();detached.events.shift();assert.throws(()=>replayCodexReport(detached),errorCode('ownership'));
});

test('native turn references may repeat in distinct sessions without merging attempts or terminal ownership',()=>{
  const report=basic();
  report.events.push(
    {sequence:3,type:'attempt.turn.ended',turnId:'turnId-2',status:'completed'},
    {sequence:4,type:'session.started',threadId:'threadId-2'},
    {sequence:5,type:'attempt.turn.started',turnId:'turnId-2'},
    {sequence:6,type:'attempt.turn.ended',turnId:'turnId-2',status:'interrupted'},
  );
  const result=replayCodexReport(report,{sourceKind:'synthetic'});assert.equal(result.sessionCount,2);assert.equal(result.attemptCount,2);
  const [first,second]=result.attempts.map(item=>item.snapshot);
  assert.notEqual(first.scope.sessionId,second.scope.sessionId);assert.notEqual(first.scope.attemptId,second.scope.attemptId);assert.notEqual(first.scope.turnId,second.scope.turnId);
  assert.equal(first.outcome,'completed');assert.equal(second.outcome,'interrupted');
});

test('permission, unknown, interrupt-only and loss-only diagnostics never fabricate a new attempt',()=>{
  const report={runtimeVersion:'synthetic-replay',events:[
    {sequence:1,type:'session.started',threadId:'threadId-1'},
    {sequence:2,type:'permission.observed',turnId:'turnId-9'},
    {sequence:3,type:'runtime.notification',turnId:'turnId-9'},
    {sequence:4,type:'attempt.interrupt.requested',turnId:'turnId-9'},
    {sequence:5,type:'attempt.observation.unknown',turnId:'turnId-9',reason:'timeout'},
  ]};
  const result=replayCodexReport(report,{sourceKind:'synthetic'});
  assert.equal(result.attemptCount,0);assert.equal(result.unassignedLegacyEvents,4);assert.equal(result.dispatches,0);
});

test('source sequences and collection budgets are enforced before replay can imply success',()=>{
  for(const sequence of [0,1,-1,NaN]) {const report=basic();report.events[1].sequence=sequence;assert.throws(()=>replayCodexReport(report),errorCode('sequence'));}
  const oversized=basic();oversized.events=Array.from({length:4097},(_,index)=>({sequence:index+1,type:'runtime.notification'}));assert.throws(()=>replayCodexReport(oversized),errorCode('protocol'));
  const processes=basic();processes.processes=Array(65).fill({});assert.throws(()=>replayCodexReport(processes),errorCode('budget'));
});

test('explicit synthetic process ownership affects only its assigned attempt with supervisor origin',()=>{
  const result=replay(synthetic('synthetic-explicit-stop-association'));
  const first=result.attempts[0].snapshot,second=result.attempts[1].snapshot;
  assert.equal(first.events.some(event=>event.origin==='supervisor'),false);assert.equal(first.process.tree.termination,'unknown');
  const processEvents=second.events.filter(event=>event.type.startsWith('process.'));
  assert.ok(processEvents.length>=5);assert.equal(processEvents.every(event=>event.origin==='supervisor'),true);
  assert.equal(second.process.runtime.code,137);assert.equal(second.process.helper.code,0);assert.equal(second.process.tree.termination,'confirmed');
  assert.equal(second.outcome,'unknown');assert.equal(second.terminalConfirmed,false);
  assert.equal(second.process.runtime.stdioClosed,true);assert.equal(second.process.helper.stdioClosed,true);
});

test('explicit later process observations preserve partial streams and then confirm actual closure',()=>{
  const report=basic();
  report.processes=[
    {parentExited:true,code:137,streams:{stdin:true,stdout:false,stderr:true},ownershipConfirmed:true,treeTermination:'unknown',activeCount:1},
    {parentExited:true,code:137,streams:{stdin:true,stdout:true,stderr:true},ownershipConfirmed:true,treeTermination:'confirmed',activeCount:0,helper:{parentExited:true,code:0},helperStreams:{stdinClosed:true,stdoutClosed:true,stderrClosed:true,faulted:false}},
  ];
  const partial=replayCodexReport(report,{sourceKind:'synthetic',processAssignments:[{attemptId:'attempt-1',processIndex:0}]}).attempts[0].snapshot;
  assert.equal(partial.process.runtime.stdioClosed,false);assert.equal(partial.process.runtime.streams.stdout,false);assert.equal(partial.process.tree.termination,'unknown');
  const complete=replayCodexReport(report,{sourceKind:'synthetic',processAssignments:[{attemptId:'attempt-1',processIndex:0},{attemptId:'attempt-1',processIndex:1}]}).attempts[0].snapshot;
  assert.equal(complete.process.runtime.stdioClosed,true);assert.equal(complete.process.helper.stdioClosed,true);assert.equal(complete.process.tree.termination,'confirmed');
  assert.equal(complete.process.runtime.code,137);assert.equal(complete.process.helper.code,0);assert.equal(complete.outcome,'unknown');
  assert.equal(complete.events.filter(event=>event.type==='process.exited'&&event.role==='runtime').length,1);
});

test('process evidence without confirmed owned-job membership never proves tree cleanup',()=>{
  const report=basic();report.processes=[{parentExited:true,code:137,ownershipConfirmed:false,treeTermination:'confirmed',activeCount:0,helper:{parentExited:true,code:0}}];
  const result=replayCodexReport(report,{sourceKind:'synthetic',processAssignments:[{attemptId:'attempt-1',processIndex:0}]});
  const snapshot=result.attempts[0].snapshot;
  assert.equal(snapshot.process.runtime.exited,true);assert.equal(snapshot.process.helper.exited,true);
  assert.equal(snapshot.process.tree.termination,'unknown');assert.equal(snapshot.process.tree.activeCount,null);assert.equal(snapshot.terminalConfirmed,false);
});

test('invalid, missing and duplicate explicit process associations are rejected',()=>{
  const item=synthetic('synthetic-explicit-stop-association');
  for(const assignments of [[{attemptId:'missing',processIndex:0}],[{attemptId:'attempt-2',processIndex:1}],[{attemptId:'attempt-2',processIndex:-1}],Array(2).fill({attemptId:'attempt-2',processIndex:0})]) {
    assert.throws(()=>replayCodexReport(item.source.report,{sourceKind:'synthetic',processAssignments:assignments}),errorCode('ownership'));
  }
});

test('pending assistant byte observations flush at evidence EOF without inventing a terminal response',()=>{
  const report=basic();report.events.push({sequence:3,type:'response.delta',turnId:'turnId-2',bytes:7});
  const snapshot=replayCodexReport(report).attempts[0].snapshot;
  assert.equal(snapshot.output.bytes,7);assert.equal(snapshot.output.assistant,7);assert.equal(snapshot.output.events,1);
  assert.equal(snapshot.outcome,'unknown');assert.equal(snapshot.terminalConfirmed,false);
  assert.equal(snapshot.events.at(-1).type,'output.observed');
  for(const bytes of [-1,65537]) {const invalid=clone(report);invalid.events[2].bytes=bytes;assert.throws(()=>replayCodexReport(invalid),errorCode('budget'));}
});

test('the neutral observer reconsumes normalized replay events and rejects forged supervisor provenance',()=>{
  const result=replay(synthetic('synthetic-explicit-stop-association'));
  for(const attempt of result.attempts)assert.deepEqual(semantic(consumeCodexReplayAttempt(attempt)),semantic(attempt.snapshot));
  const forged=clone(result.attempts[1]);forged.snapshot.events.find(event=>event.type==='process.exited').origin='runtime';
  assert.throws(()=>consumeCodexReplayAttempt(forged),errorCode('ownership'));
});

test('fixture expectations and reference selection cannot silently pass a mismatched replay',()=>{
  const wrong=clone(fixture);wrong.cases[0].expected.attemptCount=999;
  assert.throws(()=>replayCodexFixture(wrong,references),errorCode('protocol'));
  assert.throws(()=>replayCodexFixture(fixture,{}),errorCode('evidence-missing'));
  const duplicate=clone(fixture);duplicate.cases[1].name=duplicate.cases[0].name;
  assert.throws(()=>replayCodexFixture(duplicate,references),errorCode('protocol'));
});

test('normalized replay retains no source payloads, native aliases, model names, paths or credentials',()=>{
  const report=basic(),secret='SYNTHETIC_PRIVATE_PAYLOAD_DO_NOT_EXPORT';
  report.cwd=secret;report.account={email:secret};report.transcript=secret;report.auth={type:'chatgpt',authenticated:true,subscriptionGate:true,email:secret,token:secret};
  report.configuration={requested:{model:secret,effort:'low',sandbox:'workspace-write',networkAccess:false,approval:'never',cwd:secret},resolvedConfiguration:{model:secret,effort:'low',sandbox:'workspaceWrite',networkAccess:false,approval:'never',cwd:secret},raw:secret};
  report.events[1].payload=secret;report.events.push({sequence:3,type:'response.delta',turnId:'turnId-2',bytes:3,text:secret});
  report.processes=[{path:secret,raw:secret}];
  const serialized=JSON.stringify(replayCodexReport(report));
  for(const forbidden of [secret,'threadId-1','turnId-2','transcript','token','email'])assert.equal(serialized.includes(forbidden),false,forbidden);
  assert.equal(codexReplayFacts(replayCodexReport(report)).attempts[0].reasoningLevels,null);
});

test('offline command writes reproducible source hashes and normalized artifacts without source paths',async t=>{
  const directory=await mkdtemp(join(tmpdir(),'orchestrix-replay-test-'));t.after(()=>rm(directory,{recursive:true,force:true}));
  const output=join(directory,'replay.json');const completed=await runCodexReplay({output});
  const result=JSON.parse(await readFile(output,'utf8'));
  assert.equal(result.validationKind,'offline-replay');assert.equal(result.status,'passed');assert.equal(result.dispatches,0);
  assert.deepEqual(result,completed.result);assert.equal(result.sourceHashes.length,3);
  for(const [reference,path] of [['portable-fixture',fixturePath],...Object.entries(referencePaths)]) {
    assert.equal(result.sourceHashes.find(item=>item.reference===reference).sha256,createHash('sha256').update(await readFile(path)).digest('hex'));
  }
  const serialized=JSON.stringify(result);
  for(const path of [directory,fixturePath,...Object.values(referencePaths)])assert.equal(serialized.includes(path),false);
  assert.equal(result.cases.every(item=>item.replay.dispatches===0),true);
});

test('CLI refuses live/provider flags before reading evidence or launching a runtime',()=>{
  const child=spawnSync(process.execPath,[join(root,'codex-replay.mjs'),'--live'],{encoding:'utf8',timeout:5000,windowsHide:true});
  assert.equal(child.error,undefined);assert.equal(child.status,1);assert.equal(child.stdout,'');
  assert.deepEqual(JSON.parse(child.stderr),{validationKind:'offline-replay',status:'failed',code:'policy'});
});
