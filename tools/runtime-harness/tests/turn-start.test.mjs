import test from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {CodexSession} from '../src/codex-session.mjs';
import {HarnessError} from '../src/json-rpc.mjs';
import {observeCancellation} from '../probe.mjs';

const delay=ms=>new Promise(resolveDelay=>setTimeout(resolveDelay,ms));
const kind=expected=>error=>error instanceof HarnessError&&error.kind===expected;
function fixture(handler=async()=>({turn:{id:'owned-turn',status:'inProgress'}}),options={}) {
  const client=new EventEmitter();client.calls=[];client.failure=null;client.closed=false;
  client.request=(method,params)=>{client.calls.push({method,params});return handler(method,params,client);};
  client.fail=error=>{client.failure=error;client.emit('failure',error);};
  const session=new CodexSession(client,{turnTimeoutMs:1000,interruptTimeoutMs:100,...options});session.threadId='owned-thread';
  return {client,session};
}
function started(client,turnId='owned-turn',threadId='owned-thread') {
  client.emit('notification',{method:'turn/started',params:{threadId,turn:{id:turnId,status:'inProgress'}}});
}
function completed(client,turnId='owned-turn',status='completed',threadId='owned-thread') {
  client.emit('notification',{method:'turn/completed',params:{threadId,turn:{id:turnId,status}}});
}

test('pending ACK has no owned turn, and its later ACK alone never confirms execution start',async()=>{
  let acknowledge;
  const {client,session}=fixture(()=>new Promise(resolveAck=>{acknowledge=resolveAck;}));
  const pending=session.startTurn('Synthetic bounded turn');
  await assert.rejects(session.waitForTurnStart('owned-turn'),kind('contract'));
  assert.equal(session.startPending,true);assert.equal(client.calls.length,1);
  acknowledge({turn:{id:'owned-turn',status:'inProgress'}});assert.equal(await pending,'owned-turn');
  let settled=false;const observer=session.waitForTurnStart('owned-turn',1000).then(result=>{settled=true;return result;});
  await delay(10);assert.equal(settled,false);assert.equal(session.startedTurns.size,0);
  assert.equal(session.turnAdmission,'active');assert.equal(client.calls.length,1);
  started(client);assert.deepEqual(await observer,{started:true});assert.equal(session.startWaiters.size,0);
  completed(client);
});

test('foreign thread/turn notifications cannot release observers or establish owned start',async()=>{
  const {client,session}=fixture();const id=await session.startTurn('Synthetic');
  let settled=false;const observer=session.waitForTurnStart(id,1000).then(result=>{settled=true;return result;});
  started(client,'foreign-turn');started(client,id,'foreign-thread');
  completed(client,'foreign-turn');completed(client,id,'completed','foreign-thread');
  await delay(10);assert.equal(settled,false);assert.equal(session.startedTurns.size,0);assert.equal(session.turns.size,0);
  assert.equal(session.startWaiters.get(id).size,1);assert.equal(client.calls.length,1);
  started(client,id);assert.deepEqual(await observer,{started:true});
  assert.equal(session.startedTurns.has(id),true);assert.equal(session.startWaiters.size,0);completed(client,id);
});

test('owned pre-ACK start is buffered and an already observed start resolves without RPC or new listeners',async()=>{
  const {client,session}=fixture(async(method,params,current)=>{started(current);return {turn:{id:'owned-turn'}};});
  const id=await session.startTurn('Synthetic');assert.equal(session.startedTurns.has(id),true);
  const listeners=client.listenerCount('notification');
  assert.deepEqual(await session.waitForTurnStart(id),{started:true});
  started(client);assert.equal(session.events.filter(event=>event.type==='attempt.turn.started').length,1);
  assert.equal(client.listenerCount('notification'),listeners);assert.equal(session.startWaiters.size,0);
  assert.equal(client.calls.length,1);completed(client,id);
});

test('a terminal before start settles with its terminal evidence and cannot acquire a retroactive start',async()=>{
  for(const status of ['completed','failed','interrupted']) {
    const {client,session}=fixture();const id=await session.startTurn('Synthetic');const observer=session.waitForTurnStart(id);
    completed(client,id,status);const result=await observer;
    assert.equal(result.started,false);assert.strictEqual(result.terminal,session.turns.get(id));assert.equal(result.terminal.status,status);
    assert.equal(session.startedTurns.has(id),false);assert.equal(session.startWaiters.size,0);
    assert.equal(session.turnAdmission,'idle');assert.equal(session.events.some(event=>event.type==='attempt.interrupt.requested'),false);
    started(client,id);assert.equal(session.startedTurns.has(id),false);
    assert.deepEqual(await session.waitForTurnStart(id),result);assert.equal(client.calls.length,1);
  }
});

test('terminal-before-start arriving before ACK keeps its terminal result after buffering',async()=>{
  const {client,session}=fixture(async(method,params,current)=>{completed(current);started(current);return {turn:{id:'owned-turn'}};});
  const id=await session.startTurn('Synthetic');const result=await session.waitForTurnStart(id);
  assert.equal(result.started,false);assert.equal(result.terminal.status,'completed');
  assert.equal(session.startedTurns.size,0);assert.equal(session.startWaiters.size,0);assert.equal(session.turnAdmission,'idle');
  assert.equal(client.calls.length,1);
});

test('start-observer deadlines are independent, stay uncertain after late start and await owned terminal',async()=>{
  const {client,session}=fixture();const id=await session.startTurn('Synthetic');
  const longer=session.waitForTurnStart(id,1000);const short=session.waitForTurnStart(id,15);
  await assert.rejects(short,kind('timeout'));
  assert.equal(session.turnAdmission,'uncertain');assert.equal(session.startWaiters.get(id).size,1);
  assert.equal(session.events.some(event=>event.type==='attempt.observation.unknown'&&event.phase==='turn-start-observation'&&event.reason==='timeout'),true);
  await assert.rejects(session.startTurn('Unsafe duplicate'),kind('contract'));
  started(client,id);assert.deepEqual(await longer,{started:true});assert.equal(session.startWaiters.size,0);
  assert.equal(session.turnAdmission,'uncertain');assert.equal(client.calls.length,1);
  completed(client,id);assert.equal(session.turnAdmission,'idle');
});

test('default start deadline uses interruptTimeoutMs and timeout never sends a cancellation RPC',async()=>{
  const {client,session}=fixture(undefined,{interruptTimeoutMs:15});const id=await session.startTurn('Synthetic');
  await assert.rejects(session.waitForTurnStart(id),kind('timeout'));
  assert.equal(session.startWaiters.size,0);assert.equal(session.turnAdmission,'uncertain');
  assert.deepEqual(client.calls.map(call=>call.method),['turn/start']);completed(client,id);
});

test('failure and closed transports reject every pending observer, remove timers and preserve uncertain admission',async()=>{
  for(const failure of [true,false]) {
    const {client,session}=fixture();const id=await session.startTurn('Synthetic');
    const results=Promise.allSettled([session.waitForTurnStart(id,1000),session.waitForTurnStart(id,1000),session.waitForTurn(id,1000)]);
    if(failure)client.fail(new HarnessError('transport','Synthetic transport loss'));
    else {client.closed=true;client.emit('closed');}
    const outcomes=await results;assert.ok(outcomes.every(value=>value.status==='rejected'&&value.reason.kind===(failure?'transport':'process-exit')));
    assert.equal(session.startWaiters.size,0);assert.equal(session.waiters.size,0);assert.equal(session.turnAdmission,'uncertain');
    assert.equal(session.turns.size,0);assert.equal(client.calls.length,1);
    await assert.rejects(session.waitForTurnStart(id),kind(failure?'transport':'process-exit'));
    assert.equal(session.events.some(event=>event.type==='attempt.observation.unknown'&&event.phase==='turn-start-observation'),true);
  }
});

test('unowned IDs and invalid deadlines reject locally even when start or terminal evidence exists',async()=>{
  const {client,session}=fixture();const id=await session.startTurn('Synthetic');
  for(const value of ['',null,7,'foreign-turn','x'.repeat(513)])await assert.rejects(session.waitForTurnStart(value,1000),kind('contract'));
  for(const timeout of [0,-1,NaN,Infinity,1.5,2147483648])await assert.rejects(session.waitForTurnStart(id,timeout),kind('contract'));
  started(client,id);await assert.rejects(session.waitForTurnStart(id,0),kind('contract'));
  completed(client,id);await assert.rejects(session.waitForTurnStart(id,Infinity),kind('contract'));
  assert.equal(session.startWaiters.size,0);assert.equal(client.calls.length,1);
});

test('observer cap is shared across start and terminal observers; cancelling one does not affect the others',async()=>{
  const {client,session}=fixture();const id=await session.startTurn('Synthetic');
  const startObservers=Array.from({length:8},()=>session.subscribeTurnStart(id,1000));
  const terminalObservers=Array.from({length:8},()=>session.subscribeTurn(id,1000));
  const pending=Promise.allSettled([...startObservers,...terminalObservers].map(observer=>observer.promise));
  await assert.rejects(session.waitForTurnStart(id,1000),kind('contract'));
  await assert.rejects(session.waitForTurn(id,1000),kind('contract'));
  startObservers[0].cancel();startObservers[0].cancel(new Error('Duplicate cancel'));
  assert.equal(session.startWaiters.get(id).size,7);assert.equal(session.waiters.get(id).size,8);
  assert.equal(session.turnAdmission,'active');assert.equal(session.startedTurns.size,0);
  const replacement=session.waitForTurnStart(id,1000);started(client,id);
  assert.deepEqual(await replacement,{started:true});assert.equal(session.startWaiters.size,0);assert.equal(session.waiters.get(id).size,8);
  completed(client,id);const results=await pending;
  assert.equal(results[0].status,'rejected');assert.equal(results[0].reason.kind,'cancelled');
  assert.ok(results.slice(1,8).every(value=>value.status==='fulfilled'&&value.value.started===true));
  assert.ok(results.slice(8).every(value=>value.status==='fulfilled'&&value.value.status==='completed'));
  assert.equal(session.waiters.size,0);assert.equal(client.calls.length,1);
});

test('settled/cancelled observers leave no callbacks to mutate a later turn',async()=>{
  let sequence=0;const {client,session}=fixture(async()=>({turn:{id:`turn-${++sequence}`}}));
  const firstId=await session.startTurn('Synthetic first');const observer=session.subscribeTurnStart(firstId,15);
  const rejection=assert.rejects(observer.promise,kind('cancelled'));observer.cancel();await rejection;
  completed(client,firstId);const secondId=await session.startTurn('Synthetic second');const current=session.waitForTurnStart(secondId,1000);
  started(client,firstId);await delay(25);assert.equal(session.turnAdmission,'active');assert.equal(session.startedTurns.has(firstId),false);
  started(client,secondId);assert.deepEqual(await current,{started:true});assert.equal(session.startWaiters.size,0);
  completed(client,secondId);assert.equal(session.turnAdmission,'idle');assert.equal(client.calls.length,2);
});

test('probe cancellation waits for owned start, acknowledges interrupt and requires its terminal',async()=>{
  const {client,session}=fixture(async(method,params,current)=>{
    if(method==='turn/start')return {turn:{id:'owned-turn'}};
    if(method==='turn/interrupt'){completed(current,params.turnId,'interrupted');return {};}
    throw new Error('Unexpected RPC');
  });
  const id=await session.startTurn('Synthetic');const pending=observeCancellation(session,id);
  await delay(10);assert.deepEqual(client.calls.map(call=>call.method),['turn/start']);
  started(client,id);
  assert.deepEqual(await pending,{startObserved:true,requestAcknowledged:true,terminalStatus:'interrupted',interruptedConfirmed:true});
  assert.deepEqual(client.calls.map(call=>call.method),['turn/start','turn/interrupt']);
});

test('probe terminal before start never sends interrupt or claims cancellation, even for interrupted status',async()=>{
  for(const status of ['completed','interrupted']) {
    const {client,session}=fixture();const id=await session.startTurn('Synthetic');const pending=observeCancellation(session,id);
    completed(client,id,status);
    assert.deepEqual(await pending,{startObserved:false,requestAcknowledged:false,terminalStatus:status,interruptedConfirmed:false});
    assert.deepEqual(client.calls.map(call=>call.method),['turn/start']);
  }
});

test('probe preserves a terminal racing after start without inventing an interrupt ACK',async()=>{
  const {client,session}=fixture();const id=await session.startTurn('Synthetic');const pending=observeCancellation(session,id);
  started(client,id);completed(client,id,'interrupted');
  assert.deepEqual(await pending,{startObserved:true,requestAcknowledged:false,terminalStatus:'interrupted',interruptedConfirmed:false});
  assert.deepEqual(client.calls.map(call=>call.method),['turn/start']);
});

test('probe terminal after a rejected interrupt is retained but does not pass cancellation evidence',async()=>{
  const {client,session}=fixture(async(method,params,current)=>{
    if(method==='turn/start')return {turn:{id:'owned-turn'}};
    completed(current,params.turnId,'interrupted');throw new HarnessError('rpc','Synthetic interrupt rejection');
  });
  const id=await session.startTurn('Synthetic');started(client,id);
  assert.deepEqual(await observeCancellation(session,id),{startObserved:true,requestAcknowledged:false,terminalStatus:'interrupted',interruptedConfirmed:false});
  assert.equal(session.turns.get(id).status,'interrupted');assert.equal(session.turnAdmission,'idle');
});

test('probe start timeout keeps admission uncertain and never dispatches interrupt',async()=>{
  const {client,session}=fixture(undefined,{interruptTimeoutMs:15});const id=await session.startTurn('Synthetic');
  await assert.rejects(observeCancellation(session,id),kind('timeout'));
  assert.deepEqual(client.calls.map(call=>call.method),['turn/start']);assert.equal(session.turnAdmission,'uncertain');
  completed(client,id,'interrupted');
});
