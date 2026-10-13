import test from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {JsonRpcProcess,HarnessError} from '../src/json-rpc.mjs';
import {CodexSession} from '../src/codex-session.mjs';
import {CodexIdentityMap,CodexRuntimeAdapter} from '../src/codex-contract.mjs';
import {RuntimeContractObserver} from '../src/runtime-contract.mjs';
import {runtimeEnvironment} from '../src/runtime-environment.mjs';
import {createFakeRuntime,fakeRuntimeRequest} from '../fixtures/fake-runtime.mjs';

const root=join(dirname(fileURLToPath(import.meta.url)),'..');
const connectionId='connection-contract-1',connectionRevision=1,sessionId='session-contract-1';
const prompt='SYNTHETIC_PROMPT_DO_NOT_EXPORT';
const code=expected=>error=>error?.code===expected||error?.kind===expected;
const observedCodes=snapshot=>[snapshot.fault?.code,snapshot.terminal?.error?.code,...snapshot.events.map(event=>event.error?.code)].filter(Boolean);

function adapterFor(t,session,discovery,options={}) {
  const adapter=new CodexRuntimeAdapter({session,discovery,runtimeVersion:'synthetic-fixture',connectionId,connectionRevision,sessionId,evidenceSource:'synthetic',...options});
  t.after(()=>adapter.dispose());return adapter;
}

async function prepared(t,scenario='success',{thread=true,...clientOptions}={}) {
  const client=new JsonRpcProcess({command:process.execPath,args:[join(root,'fixtures/fake-app-server.mjs'),'--scenario',scenario],cwd:root,env:runtimeEnvironment(),requestTimeoutMs:2000,...clientOptions});
  t.after(()=>client.stop(1000));
  const calls=[],nativeRequest=client.request.bind(client);
  client.request=(method,...args)=>{calls.push(method);return nativeRequest(method,...args);};
  const session=new CodexSession(client,{turnTimeoutMs:2000,interruptTimeoutMs:1000});
  if(client.child.ready)await client.child.ready;
  await session.initialize();const discovery=await session.discover();
  if(thread)await session.startThread({cwd:root,model:'fake-model',effort:'low',sandbox:'read-only'});
  return {client,session,discovery,calls,adapter:adapterFor(t,session,discovery)};
}

function assertScope(snapshot,attemptId) {
  const {turnId,...owner}=snapshot.scope;
  assert.deepEqual(owner,{connectionId,connectionRevision,sessionId,attemptId});
  assert.match(turnId,/^[-A-Za-z0-9_]{1,80}$/);
  assert.deepEqual(snapshot.events.map(event=>event.sequence),snapshot.events.map((_,index)=>index+1));
  assert.ok(snapshot.events.every(event=>event.connectionId===connectionId&&event.connectionRevision===connectionRevision&&event.sessionId===sessionId&&event.attemptId===attemptId&&event.turnId===turnId));
}

function assertPrivate(snapshot,additional=[]) {
  const serialized=JSON.stringify(snapshot);
  for(const value of [prompt,'thread-fake-001','turn-fake-001','fake-model',root,...additional])assert.equal(serialized.includes(JSON.stringify(value).slice(1,-1)),false,'Native/private value escaped the normalized boundary');
}

// This consumer understands only the portable contract, regardless of adapter.
function consumeNormalized(descriptor,request,envelopes) {
  const observer=new RuntimeContractObserver({descriptor,request});
  for(const {origin,frame} of envelopes)observer.accept(frame,{origin});
  return observer.snapshot();
}

const nativeThread='NATIVE_THREAD_DO_NOT_EXPORT';
const nativeModel='native.model/DO_NOT_EXPORT';
const nativeModels=[nativeModel,'another.native/model'];
function notification(client,method,turnId,status,threadId=nativeThread,extra={}) {
  client.emit('notification',{method,params:{threadId,...extra,...(method==='item/agentMessage/delta'?{turnId}:{turn:{id:turnId,status}})}});
}

async function mockPrepared(t,{onTurn,onInterrupt,models=nativeModels,resumed=false,mappings}={}) {
  const client=new EventEmitter();client.calls=[];client.failure=null;client.closed=false;
  client.notify=()=>{};client.reply=()=>{};client.rejectRequest=()=>{};
  client.fail=error=>{if(!client.failure){client.failure=error;client.emit('failure',error);}};
  let turns=resumed?1:0;
  client.request=async(method,params)=>{
    client.calls.push({method});
    if(method==='initialize')return {};
    if(method==='account/read')return {account:{type:'chatgpt',email:'fixture@orchestrix.invalid',planType:'pro'},requiresOpenaiAuth:true};
    if(method==='model/list')return {data:models.map((model,index)=>({id:model,model,isDefault:model===nativeModel,defaultReasoningEffort:'low',supportedReasoningEfforts:[{reasoningEffort:'low'},{reasoningEffort:'high'}]})),nextCursor:null};
    if(method==='account/rateLimits/read')return {ordinaryUsageAllowed:true};
    if(method==='thread/start'||method==='thread/resume')return {thread:{id:nativeThread,sessionId:nativeThread,cwd:root,ephemeral:method==='thread/start'?params.ephemeral:false,modelProvider:'openai',status:{type:'idle'},turns:[]},cwd:root,model:nativeModel,modelProvider:'openai',reasoningEffort:'low',approvalPolicy:'never',approvalsReviewer:'user',sandbox:{type:'readOnly',networkAccess:false}};
    if(method==='turn/start'){
      const turnId=`NATIVE_TURN_DO_NOT_EXPORT_${++turns}`;
      if(onTurn)return onTurn({client,params,turnId});
      notification(client,'turn/started',turnId,'inProgress');notification(client,'turn/completed',turnId,'completed');return {turn:{id:turnId,status:'inProgress'}};
    }
    if(method==='turn/interrupt'){
      if(onInterrupt)return onInterrupt({client,params});
      notification(client,'turn/completed',params.turnId,'interrupted');return {};
    }
    throw new Error(`Unexpected synthetic request: ${method}`);
  };
  const session=new CodexSession(client,{turnTimeoutMs:1000,interruptTimeoutMs:1000});
  await session.initialize();const discovery=await session.discover();
  if(resumed)await session.resumeThread({threadId:nativeThread,cwd:root,model:nativeModel,effort:'low',sandbox:'read-only'});
  else await session.startThread({cwd:root,model:nativeModel,effort:'low',sandbox:'read-only',ephemeral:false});
  return {client,session,discovery,adapter:adapterFor(t,session,discovery,{mappings})};
}

test('Codex fixture traverses the portable boundary with known settings and private local references',async t=>{
  const {client,adapter,calls}=await prepared(t);
  const admitted=await adapter.startAttempt({attemptId:'attempt-success',text:prompt,effort:'low'});
  assert.equal(admitted.attemptId,'attempt-success');assert.equal(admitted.sessionId,sessionId);assert.notEqual(admitted.turnId,'turn-fake-001');
  const snapshot=await adapter.waitForAttempt('attempt-success',2000);
  assert.equal(snapshot.outcome,'completed');assert.equal(snapshot.terminalConfirmed,true);
  assert.equal(snapshot.effective.reasoning.state,'known');assert.equal(snapshot.effective.reasoning.value,'low');
  assert.equal(snapshot.effective.sandbox.value,'read-only');assert.equal(snapshot.effective.network.value,'disabled');assert.equal(snapshot.effective.approval.value,'never');
  assertScope(snapshot,'attempt-success');assertPrivate(snapshot);
  assert.equal(calls.filter(method=>method==='turn/start').length,1);
  const stopped=await client.stop(1000);adapter.observeProcess(stopped);
  const final=adapter.snapshot('attempt-success');assert.equal(final.outcome,'completed');assert.equal(final.process.runtime.exited,true);assert.equal(final.process.runtime.code,0);
  assert.equal(final.process.tree.termination,'unknown');assert.equal(final.process.helper.exited,false);
});

test('the same provider-neutral consumer accepts Codex-normalized and in-memory fake envelopes',async t=>{
  const {adapter}=await prepared(t);await adapter.startAttempt({attemptId:'attempt-consumer',text:prompt,effort:'low'});
  const snapshot=await adapter.waitForAttempt('attempt-consumer',2000);
  const replay=consumeNormalized(adapter.descriptor,{version:snapshot.version,...snapshot.scope,requested:snapshot.requested},snapshot.events.map(event=>({origin:event.origin,frame:JSON.stringify(event)})));
  const fake=createFakeRuntime(),envelopes=[];for await(const envelope of fake.run())envelopes.push(envelope);
  const synthetic=consumeNormalized(fake.descriptor,fakeRuntimeRequest(),envelopes);
  for(const result of [replay,synthetic]){assert.equal(result.outcome,'completed');assert.equal(result.terminalConfirmed,true);assert.equal(result.phase,'terminal');}
  assert.deepEqual(replay.effective,snapshot.effective);
});

test('serial turns retain one session and distinct attempts without replaying a previous terminal',async t=>{
  const {adapter,calls}=await prepared(t);
  const first=await adapter.startAttempt({attemptId:'attempt-first',text:prompt,effort:'low'});const before=await adapter.waitForAttempt(first.attemptId,2000);
  const second=await adapter.startAttempt({attemptId:'attempt-second',text:prompt,effort:'low'});const after=await adapter.waitForAttempt(second.attemptId,2000);
  assert.equal(first.sessionId,second.sessionId);assert.notEqual(first.turnId,second.turnId);
  assertScope(before,first.attemptId);assertScope(after,second.attemptId);assert.deepEqual(adapter.snapshot(first.attemptId),before);
  assert.equal(adapter.snapshots().length,2);
  await assert.rejects(adapter.startAttempt({attemptId:first.attemptId,text:prompt,effort:'low'}));
  assert.equal(calls.filter(method=>method==='turn/start').length,2);
});

test('timeout keeps the owned attempt unknown, with no automatic retry, then records explicit interruption',async t=>{
  const {adapter,calls}=await prepared(t,'hang');await adapter.startAttempt({attemptId:'attempt-timeout',text:prompt,effort:'low'});
  await assert.rejects(adapter.waitForAttempt('attempt-timeout',40),code('timeout'));
  const timedOut=adapter.snapshot('attempt-timeout');assert.equal(timedOut.outcome,'unknown');assert.equal(timedOut.terminalConfirmed,false);assert.ok(observedCodes(timedOut).includes('timeout'));
  assert.equal(calls.filter(method=>method==='turn/start').length,1);
  const interrupted=await adapter.interrupt('attempt-timeout');assert.equal(interrupted.outcome,'interrupted');assert.equal(interrupted.terminalConfirmed,true);
  assert.equal(interrupted.interrupt.requested,true);assert.equal(interrupted.interrupt.acknowledged,true);
  assert.equal(calls.filter(method=>method==='turn/interrupt').length,1);
});

test('rate-limited dispatch rejects without a fabricated terminal or fallback invocation',async t=>{
  const {adapter,calls}=await prepared(t,'rate-limit');
  await assert.rejects(adapter.startAttempt({attemptId:'attempt-rate-limit',text:prompt,effort:'low'}),code('rpc'));
  const snapshot=adapter.snapshot('attempt-rate-limit');assert.equal(snapshot.outcome,'unknown');assert.equal(snapshot.terminalConfirmed,false);assert.ok(observedCodes(snapshot).includes('rate-limit'));
  assert.equal(snapshot.events.find(event=>event.error?.code==='rate-limit').error.dispatch,'sent');
  assert.equal(calls.filter(method=>method==='turn/start').length,1);assert.equal(calls.filter(method=>method==='turn/interrupt').length,0);assertPrivate(snapshot);
});

test('unauthenticated metadata blocks before thread or turn creation',async t=>{
  const {adapter,calls}=await prepared(t,'auth-failure',{thread:false});
  assert.equal(adapter.descriptor.connection.authentication,'unauthenticated');
  await assert.rejects(adapter.startAttempt({attemptId:'attempt-auth',text:prompt,effort:'low'}),code('authentication'));
  const snapshot=adapter.snapshot('attempt-auth');assert.equal(snapshot.outcome,'unknown');assert.equal(snapshot.terminalConfirmed,false);
  assert.equal(snapshot.events.find(event=>event.error?.code==='authentication').error.dispatch,'not-sent');
  assert.equal(calls.includes('thread/start'),false);assert.equal(calls.includes('turn/start'),false);
});

for(const [scenario,kind] of [['malformed','protocol'],['crash','process-exit']])test(`${scenario} fixture preserves unknown terminal independently of observed parent exit`,async t=>{
  const {adapter,client,calls}=await prepared(t,scenario);
  await assert.rejects(async()=>{await adapter.startAttempt({attemptId:`attempt-${scenario}`,text:prompt,effort:'low'});return adapter.waitForAttempt(`attempt-${scenario}`,2000);},code(kind));
  const stopped=await client.stop(1000);adapter.observeProcess(stopped);
  const snapshot=adapter.snapshot(`attempt-${scenario}`);assert.equal(snapshot.outcome,'unknown');assert.equal(snapshot.terminalConfirmed,false);
  assert.equal(snapshot.process.runtime.exited,true);assert.equal(snapshot.process.tree.termination,'unknown');assert.equal(snapshot.process.helper.exited,false);
  if(scenario==='crash')assert.equal(snapshot.process.runtime.code,23);
  assert.equal(calls.filter(method=>method==='turn/start').length,1);assertPrivate(snapshot);
});

test('unknown notifications and drained stderr do not leak native payloads or consume output budget',async t=>{
  for(const scenario of ['unknown-notification','noisy-stderr','split-utf8']){
    const {adapter,client}=await prepared(t,scenario);const attemptId=`attempt-${scenario}`;
    await adapter.startAttempt({attemptId,text:prompt,effort:'low'});const snapshot=await adapter.waitForAttempt(attemptId,2000);
    assert.equal(snapshot.outcome,'completed');assert.ok(snapshot.budget.bytes<16384);assert.ok(snapshot.budget.frames<20);
    assertPrivate(snapshot,['fixture/unknownNotification','SYNTHETIC STDERR','Olá','Synthetic success']);
    if(scenario==='noisy-stderr')assert.ok(client.stderrBytes>1024*1024);
  }
});

test('pre-ACK owned start and terminal normalize after admission while foreign identities stay excluded',async t=>{
  const {adapter}=await mockPrepared(t,{onTurn:({client,turnId})=>{
    notification(client,'turn/completed','FOREIGN_TURN_DO_NOT_EXPORT','completed');
    notification(client,'turn/completed',turnId,'completed','FOREIGN_THREAD_DO_NOT_EXPORT');
    notification(client,'turn/started',turnId,'inProgress');notification(client,'turn/completed',turnId,'completed');return {turn:{id:turnId}};
  }});
  const admitted=await adapter.startAttempt({attemptId:'attempt-preack',text:prompt,effort:'low'});const snapshot=await adapter.waitForAttempt(admitted.attemptId,1000);
  assert.equal(snapshot.outcome,'completed');assert.equal(snapshot.terminalConfirmed,true);
  const types=snapshot.events.map(event=>event.type);assert.ok(types.indexOf('attempt.admission-observed')>=0);assert.ok(types.indexOf('attempt.admission-observed')<types.indexOf('attempt.start-observed'));
  assertPrivate(snapshot,[nativeThread,nativeModel,'NATIVE_TURN_DO_NOT_EXPORT_1','FOREIGN_TURN_DO_NOT_EXPORT','FOREIGN_THREAD_DO_NOT_EXPORT']);
});

test('owned terminal before ACK/start remains factual without inventing a start',async t=>{
  for(const outcome of ['completed','failed','interrupted']){
    const {adapter}=await mockPrepared(t,{onTurn:({client,turnId})=>{
      notification(client,'turn/completed',turnId,outcome);notification(client,'turn/started',turnId,'inProgress');return {turn:{id:turnId}};
    }});
    const attemptId=`attempt-early-${outcome}`;await adapter.startAttempt({attemptId,text:prompt,effort:'low'});const snapshot=await adapter.waitForAttempt(attemptId,1000);
    assert.equal(snapshot.outcome,outcome);assert.equal(snapshot.terminalConfirmed,true);assert.equal(snapshot.startObserved,false);assert.equal(snapshot.events.some(event=>event.type==='attempt.start-observed'),false);
    assert.equal(snapshot.interrupt.requested,false);assertScope(snapshot,attemptId);
  }
});

test('interrupt terminal preceding its ACK preserves both facts without duplicate cancellation',async t=>{
  const {adapter,client}=await mockPrepared(t,{onTurn:({client,turnId})=>{notification(client,'turn/started',turnId,'inProgress');return {turn:{id:turnId}};},onInterrupt:({client,params})=>{notification(client,'turn/completed',params.turnId,'interrupted');return {};}});
  await adapter.startAttempt({attemptId:'attempt-interrupt-race',text:prompt,effort:'low'});
  const snapshot=await adapter.interrupt('attempt-interrupt-race');assert.equal(snapshot.outcome,'interrupted');assert.equal(snapshot.interrupt.requested,true);assert.equal(snapshot.interrupt.acknowledged,true);
  assert.equal(snapshot.terminalConfirmed,true);await adapter.interrupt('attempt-interrupt-race');assert.equal(client.calls.filter(call=>call.method==='turn/interrupt').length,1);
});

test('lost turn ACK quarantines admission without resending the first or next attempt',async t=>{
  const {adapter,client}=await mockPrepared(t,{onTurn:()=>{throw new HarnessError('timeout','Synthetic lost ACK; no provider called');}});
  await assert.rejects(adapter.startAttempt({attemptId:'attempt-lost-ack',text:prompt,effort:'low'}),code('timeout'));
  const snapshot=adapter.snapshot('attempt-lost-ack');assert.equal(snapshot.outcome,'unknown');assert.equal(snapshot.terminalConfirmed,false);
  await assert.rejects(adapter.startAttempt({attemptId:'attempt-unsafe-retry',text:prompt,effort:'low'}));
  assert.equal(client.calls.filter(call=>call.method==='turn/start').length,1);
});

test('shared private mappings survive catalog reorder and thread resume with stable local model/session IDs',async t=>{
  const mappings=new CodexIdentityMap({connectionId,connectionRevision});
  const first=await mockPrepared(t,{mappings});await first.adapter.startAttempt({attemptId:'attempt-before-restart',text:prompt,effort:'low'});const before=await first.adapter.waitForAttempt('attempt-before-restart',1000);
  first.adapter.dispose();
  const resumed=await mockPrepared(t,{mappings,resumed:true,models:[...nativeModels].reverse()});await resumed.adapter.startAttempt({attemptId:'attempt-after-restart',text:prompt,effort:'low'});const after=await resumed.adapter.waitForAttempt('attempt-after-restart',1000);
  assert.equal(before.scope.sessionId,after.scope.sessionId);assert.notEqual(before.scope.turnId,after.scope.turnId);assert.equal(before.effective.model.value,after.effective.model.value);
  assert.deepEqual(first.adapter.descriptor.models.map(model=>model.id).sort(),resumed.adapter.descriptor.models.map(model=>model.id).sort());
  assertPrivate(after,[nativeThread,nativeModel,'NATIVE_TURN_DO_NOT_EXPORT_1','NATIVE_TURN_DO_NOT_EXPORT_2','fixture@orchestrix.invalid']);
});

test('private mappings cannot be silently reused for another connection revision',async t=>{
  const {session,discovery}=await mockPrepared(t);
  const mappings=new CodexIdentityMap({connectionId,connectionRevision:2});
  assert.throws(()=>new CodexRuntimeAdapter({session,discovery,runtimeVersion:'synthetic-fixture',connectionId,connectionRevision,sessionId,mappings,evidenceSource:'synthetic'}),code('ownership'));
});

test('oversized native output fails before it can become a portable terminal or retained payload',async t=>{
  const oversized='SYNTHETIC_OVERSIZED_DO_NOT_EXPORT'.repeat(3000);
  const {adapter}=await mockPrepared(t,{onTurn:({client,turnId})=>{
    notification(client,'turn/started',turnId,'inProgress');notification(client,'item/agentMessage/delta',turnId,undefined,nativeThread,{delta:oversized});return {turn:{id:turnId}};
  }});
  await assert.rejects(async()=>{await adapter.startAttempt({attemptId:'attempt-output-cap',text:prompt,effort:'low'});return adapter.waitForAttempt('attempt-output-cap',1000);},code('protocol'));
  const snapshot=adapter.snapshot('attempt-output-cap');assert.equal(snapshot.outcome,'unknown');assert.equal(snapshot.terminalConfirmed,false);assert.ok(snapshot.budget.bytes<16384);
  assertPrivate(snapshot,['SYNTHETIC_OVERSIZED_DO_NOT_EXPORT']);
});

test('native RPC classification retains only bounded tags and the adapter interprets only known tags',async t=>{
  const secret='SYNTHETIC_RPC_DETAIL_DO_NOT_EXPORT';
  for(const [tag,expected] of [['rate_limit','rate-limit'],['authentication','authentication'],['future_native_kind','evidence-missing'],['invalid tag/value','evidence-missing'],['x'.repeat(65),'evidence-missing'],[null,'evidence-missing']]){
    const source=`import {createInterface} from 'node:readline'; const lines=createInterface({input:process.stdin,crlfDelay:Infinity}); lines.on('line',line=>{const {id}=JSON.parse(line);process.stdout.write(JSON.stringify({id,error:{code:-32029,message:${JSON.stringify(secret)},data:{kind:${JSON.stringify(tag)},token:${JSON.stringify(secret)},nested:{private:${JSON.stringify(secret)}}}}})+'\\n');});`;
    const wire=new JsonRpcProcess({command:process.execPath,args:['--input-type=module','-e',source],cwd:root,env:runtimeEnvironment(),requestTimeoutMs:2000});t.after(()=>wire.stop(1000));
    let rejected;try{await wire.request('turn/start',{});}catch(error){rejected=error;}
    assert.ok(rejected instanceof HarnessError);assert.equal(rejected.kind,'rpc');assert.equal(rejected.message,'Runtime rejected request');
    const bounded=typeof tag==='string'&&/^[A-Za-z][A-Za-z0-9_-]{0,63}$/.test(tag);
    assert.deepEqual(rejected.metadata,{method:'turn/start',code:-32029,...(bounded?{dataTag:tag}:{})});assert.equal(JSON.stringify(rejected).includes(secret),false);
    const {adapter}=await mockPrepared(t,{onTurn:()=>{throw rejected;}});const attemptId=`attempt-rpc-${expected}-${typeof tag==='string'?tag.length:'null'}`;
    await assert.rejects(adapter.startAttempt({attemptId,text:prompt,effort:'low'}),code('rpc'));
    const snapshot=adapter.snapshot(attemptId);assert.ok(observedCodes(snapshot).includes(expected));assert.equal(snapshot.outcome,'unknown');assert.equal(snapshot.terminalConfirmed,false);
    assertPrivate(snapshot,[secret,...(typeof tag==='string'&&tag!==expected?[tag]:[])]);await wire.stop(1000);
  }
});

test('failed native terminals classify known nested codes and preserve unknown causes without inventing protocol errors',async t=>{
  const secret='SYNTHETIC_TERMINAL_DETAIL_DO_NOT_EXPORT';
  for(const [nativeCode,expected] of [[{type:'rateLimitExceeded'},'rate-limit'],[{type:'unauthorized'},'authentication'],[{type:'newUnknownNativeFailure',nested:secret},null]]){
    const {adapter}=await mockPrepared(t,{onTurn:({client,turnId})=>{
      notification(client,'turn/started',turnId,'inProgress');
      client.emit('notification',{method:'turn/completed',params:{threadId:nativeThread,turn:{id:turnId,status:'failed',error:{codexErrorInfo:nativeCode,message:secret,additionalDetails:secret}}}});
      return {turn:{id:turnId}};
    }});
    await adapter.startAttempt({attemptId:'attempt-native-error',text:prompt,effort:'low'});const snapshot=await adapter.waitForAttempt('attempt-native-error',1000);
    assert.equal(snapshot.outcome,'failed');assert.equal(snapshot.terminalConfirmed,true);assert.equal(snapshot.fatal,false);
    if(expected)assert.equal(snapshot.terminal.error?.code,expected);else assert.equal(snapshot.terminal.error,null);
    assert.equal(observedCodes(snapshot).includes('protocol'),false);assertPrivate(snapshot,[secret,'newUnknownNativeFailure']);
  }
});

test('many native deltas are batched within portable budgets while preserving their exact byte count',async t=>{
  const {adapter,client}=await mockPrepared(t,{onTurn:({client,turnId})=>{notification(client,'turn/started',turnId,'inProgress');return {turn:{id:turnId}};}});
  await adapter.startAttempt({attemptId:'attempt-output-batches',text:prompt,effort:'low'});
  const chunk='PRIVATE_CHUNK_DO_NOT_EXPORT'.padEnd(64,'x');
  for(let index=0;index<1000;index++)notification(client,'item/agentMessage/delta','NATIVE_TURN_DO_NOT_EXPORT_1',undefined,nativeThread,{delta:chunk});
  notification(client,'turn/completed','NATIVE_TURN_DO_NOT_EXPORT_1','completed');
  const snapshot=await adapter.waitForAttempt('attempt-output-batches',1000);
  assert.equal(snapshot.outcome,'completed');assert.equal(snapshot.output.assistant,64000);assert.equal(snapshot.output.bytes,64000);
  assert.ok(snapshot.output.events<=32);assert.ok(snapshot.budget.frames<64);assert.ok(snapshot.budget.bytes<32768);assert.equal(snapshot.fatal,false);
  assertPrivate(snapshot,[chunk,'PRIVATE_CHUNK_DO_NOT_EXPORT']);
});

test('owned supervisor facts remain independent from terminal success and helper or stream faults',async t=>{
  const {adapter}=await mockPrepared(t);await adapter.startAttempt({attemptId:'attempt-owned-cleanup',text:prompt,effort:'low'});
  const terminal=await adapter.waitForAttempt('attempt-owned-cleanup',1000);assert.equal(terminal.outcome,'completed');
  adapter.observeProcess({parentExited:true,code:137,signal:null,streams:{stdin:true,stdout:false,stderr:true},stdioClosed:false,helper:{parentExited:true,code:1,stdioClosed:true},helperStreams:{stdinClosed:true,stdoutClosed:true,stderrClosed:true,faulted:true},ownershipConfirmed:true,activeCount:0,treeTermination:'confirmed',termination:'forced'});
  const snapshot=adapter.snapshot('attempt-owned-cleanup');assert.equal(snapshot.outcome,'completed');assert.equal(snapshot.terminalConfirmed,true);
  assert.equal(snapshot.process.runtime.code,137);assert.equal(snapshot.process.runtime.stdioClosed,false);assert.equal(snapshot.process.helper.code,1);assert.equal(snapshot.process.helper.stdioClosed,false);assert.equal(snapshot.process.helper.streams.faulted,true);
  assert.equal(snapshot.process.tree.termination,'confirmed');assert.equal(snapshot.process.tree.activeCount,0);
  adapter.observeProcess({parentExited:true,code:137,ownershipConfirmed:true,activeCount:0,treeTermination:'confirmed'});
  assert.deepEqual(adapter.snapshot('attempt-owned-cleanup'),snapshot);
});

test('Windows owned Job stops an offline hanging fixture without fabricating a remote terminal',{skip:process.platform!=='win32',timeout:30000},async t=>{
  const {adapter,client,session}=await prepared(t,'hang',{windowsJob:{lifetimeMs:15000}});
  await adapter.startAttempt({attemptId:'attempt-windows-owned-stop',text:prompt,effort:'low'});
  assert.equal((await session.waitForTurnStart(session.activeTurnId)).started,true);
  const stopped=await client.terminateOwnedJob();adapter.observeProcess(stopped);
  assert.equal(stopped.code,137);assert.equal(stopped.termination,'forced');assert.equal(stopped.activeCount,0);assert.equal(stopped.ownershipConfirmed,true);
  assert.equal(stopped.treeTermination,'confirmed');assert.equal(stopped.stdioClosed,true);assert.equal(stopped.helper.code,0);assert.equal(stopped.helper.parentExited,true);assert.equal(stopped.helper.stdioClosed,true);
  assert.deepEqual(stopped.streams,{stdin:true,stdout:true,stderr:true});
  assert.deepEqual(stopped.helperStreams,{stdinClosed:true,stdoutClosed:true,stderrClosed:true,faulted:false});
  const snapshot=adapter.snapshot('attempt-windows-owned-stop');assert.equal(snapshot.outcome,'unknown');assert.equal(snapshot.terminalConfirmed,false);
  assert.equal(snapshot.process.runtime.code,137);assert.equal(snapshot.process.runtime.stdioClosed,true);assert.equal(snapshot.process.helper.code,0);assert.equal(snapshot.process.helper.stdioClosed,true);
  assert.equal(snapshot.process.tree.termination,'confirmed');assert.equal(snapshot.process.tree.activeCount,0);assertPrivate(snapshot);
  const replayed=consumeNormalized(adapter.descriptor,{version:snapshot.version,...snapshot.scope,requested:snapshot.requested},snapshot.events.map(event=>({origin:event.origin,frame:JSON.stringify(event)})));
  assert.equal(replayed.outcome,'unknown');assert.equal(replayed.terminalConfirmed,false);assert.equal(replayed.process.tree.termination,'confirmed');assert.equal(replayed.process.runtime.code,137);assert.equal(replayed.process.helper.code,0);
});

test('adapter merges late stream closure without losing earlier fault or terminal observations',async t=>{
  const {adapter}=await mockPrepared(t);await adapter.startAttempt({attemptId:'attempt-late-streams',text:prompt,effort:'low'});
  await adapter.waitForAttempt('attempt-late-streams',1000);
  adapter.observeProcess({parentExited:true,code:0,streams:{stdin:true,stdout:false,stderr:false},helperStreams:{stdinClosed:true,stdoutClosed:false,stderrClosed:false,faulted:true}});
  assert.equal(adapter.snapshot('attempt-late-streams').process.runtime.stdioClosed,false);
  adapter.observeProcess({parentExited:true,code:0,streams:{stdin:true,stdout:true,stderr:true},helperStreams:{stdinClosed:true,stdoutClosed:true,stderrClosed:true,faulted:false}});
  const snapshot=adapter.snapshot('attempt-late-streams');
  assert.equal(snapshot.process.runtime.stdioClosed,true);assert.equal(snapshot.process.helper.streams.faulted,true);
  assert.equal(snapshot.process.helper.stdioClosed,false);assert.equal(snapshot.outcome,'completed');
  assert.equal(snapshot.events.filter(event=>event.type==='process.exited').length,1);
});

test('native backpressure remains a budget fault with unknown terminal rather than an unclassified provider error',async t=>{
  const {adapter,client}=await mockPrepared(t,{onTurn:({client,turnId})=>{notification(client,'turn/started',turnId,'inProgress');return {turn:{id:turnId}};}});
  await adapter.startAttempt({attemptId:'attempt-backpressure',text:prompt,effort:'low'});
  client.fail(new HarnessError('backpressure','SYNTHETIC_PRIVATE_MESSAGE'));
  const snapshot=adapter.snapshot('attempt-backpressure');assert.equal(snapshot.fault.code,'budget');
  assert.equal(snapshot.outcome,'unknown');assert.equal(snapshot.terminalConfirmed,false);assert.equal(snapshot.fatal,true);
  assertPrivate(snapshot,['SYNTHETIC_PRIVATE_MESSAGE']);
});

test('locally rejected start and interrupt frames retain not-sent instead of inventing provider dispatch',async t=>{
  const first=await prepared(t,'hang'),limit=first.client.maxOutboundMessageBytes;first.client.maxOutboundMessageBytes=32;
  await assert.rejects(first.adapter.startAttempt({attemptId:'attempt-local-start',text:prompt,effort:'low'}),code('outbound-limit'));
  const rejected=first.adapter.snapshot('attempt-local-start');
  assert.equal(rejected.events.find(event=>event.error?.code==='budget').error.dispatch,'not-sent');
  assert.equal(rejected.admissionObserved,false);assert.equal(first.session.knownTurns.size,0);assert.equal(first.client.failure,null);
  first.client.maxOutboundMessageBytes=limit;
  const second=await prepared(t,'hang');await second.adapter.startAttempt({attemptId:'attempt-local-interrupt',text:prompt,effort:'low'});
  second.client.maxOutboundMessageBytes=32;
  await assert.rejects(second.adapter.interrupt('attempt-local-interrupt'),code('outbound-limit'));
  const interrupted=second.adapter.snapshot('attempt-local-interrupt');
  assert.equal(interrupted.events.find(event=>event.error?.code==='budget').error.dispatch,'not-sent');
  assert.equal(interrupted.interrupt.requested,true);assert.equal(interrupted.interrupt.acknowledged,false);
  assert.equal(interrupted.outcome,'unknown');assert.equal(interrupted.terminalConfirmed,false);assert.equal(second.client.failure,null);
  second.client.maxOutboundMessageBytes=limit;
});
