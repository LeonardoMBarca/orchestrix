import test from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {dirname,join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {JsonRpcProcess,HarnessError} from '../src/json-rpc.mjs';
import {CodexSession} from '../src/codex-session.mjs';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const delay=ms=>new Promise(resolveDelay=>setTimeout(resolveDelay,ms));
const hasKind=kind=>error=>error instanceof HarnessError&&error.kind===kind;
function boundedClient(handler) {
  const client=new EventEmitter();client.calls=[];client.failure=null;client.closed=false;
  client.request=(method,params,timeoutMs)=>{client.calls.push({method,params,timeoutMs});return handler(method,params,timeoutMs);};
  client.fail=error=>{if(client.failure)return;client.failure=error;client.emit('failure',error);};
  return client;
}
function sessionFor(client,options={}) {
  const session=new CodexSession(client,{turnTimeoutMs:500,interruptTimeoutMs:200,...options});
  session.threadId='thread-owned';return session;
}
function completed(client,id,status='completed',threadId='thread-owned') {
  client.emit('notification',{method:'turn/completed',params:{threadId,turn:{id,status}}});
}

test('one active turn owns admission until its own terminal notification',async()=>{
  let sequence=0;
  const client=boundedClient(async()=>({turn:{id:`turn-${++sequence}`}}));
  const session=sessionFor(client);const id=await session.startTurn('Synthetic first');
  await assert.rejects(session.startTurn('Synthetic duplicate'),hasKind('contract'));
  completed(client,'unowned-turn');completed(client,id,'completed','different-thread');
  await assert.rejects(session.startTurn('Synthetic still blocked'),hasKind('contract'));
  assert.equal(client.calls.length,1);assert.equal(session.activeTurnId,id);
  completed(client,id);assert.equal(session.turnAdmission,'idle');
  assert.equal(await session.startTurn('Synthetic next'),'turn-2');
  completed(client,'turn-2');
});

test('contradictory alternate turn IDs fail in both directions without changing ownership or a confirmed result',async()=>{
  for(const method of ['turn/started','turn/completed','item/agentMessage/delta']) {
    for(const [canonical,alternate] of [['own-turn','foreign-turn'],['foreign-turn','own-turn']]) {
      const client=boundedClient(async()=>({turn:{id:'own-turn'}}));const session=sessionFor(client);
      await session.startTurn('Synthetic own turn');
      const params=method==='item/agentMessage/delta'
        ?{threadId:'thread-owned',turnId:canonical,turn:{id:alternate},delta:'Contradictory synthetic delta'}
        :{threadId:'thread-owned',turn:{id:canonical,status:'completed'},turnId:alternate};
      client.emit('notification',{method,params});
      assert.equal(client.failure?.kind,'protocol');assert.equal(session.turns.size,0);assert.equal(session.deltaText.size,0);
      assert.equal(session.activeTurnId,'own-turn');assert.equal(session.turnAdmission,'uncertain');
      assert.equal(session.events.some(event=>['attempt.turn.started','attempt.turn.ended','response.delta'].includes(event.type)),false);
      await assert.rejects(session.startTurn('Synthetic unsafe next turn'),hasKind('contract'));assert.equal(client.calls.length,1);
    }
  }
  for(const [canonical,alternate] of [['own-turn','foreign-turn'],['foreign-turn','own-turn']]) {
    const client=boundedClient(async()=>({turn:{id:'own-turn'}}));const session=sessionFor(client);
    await session.startTurn('Synthetic confirmed turn');completed(client,'own-turn');const confirmed=session.turns.get('own-turn');
    client.emit('notification',{method:'turn/completed',params:{threadId:'thread-owned',turn:{id:canonical,status:'failed'},turnId:alternate}});
    assert.equal(client.failure?.kind,'protocol');assert.strictEqual(session.turns.get('own-turn'),confirmed);
    assert.equal(confirmed.status,'completed');assert.equal(session.turns.size,1);assert.equal(session.turns.has('foreign-turn'),false);
  }
  const client=boundedClient(async()=>({turn:{id:'own-turn'}}));const session=sessionFor(client);
  await session.startTurn('Synthetic coherent extra IDs');
  client.emit('notification',{method:'turn/started',params:{threadId:'thread-owned',turn:{id:'own-turn'},turnId:'own-turn'}});
  client.emit('notification',{method:'item/agentMessage/delta',params:{threadId:'thread-owned',turnId:'own-turn',turn:{id:'own-turn'},delta:'Coherent synthetic delta'}});
  client.emit('notification',{method:'turn/completed',params:{threadId:'thread-owned',turn:{id:'own-turn',status:'completed'},turnId:'own-turn'}});
  assert.equal(client.failure,null);assert.equal(session.turns.get('own-turn').status,'completed');
  assert.equal(session.deltaText.get('own-turn'),'Coherent synthetic delta');assert.equal(session.turnAdmission,'idle');
});

test('an in-flight start cannot admit a second request',async()=>{
  let acknowledge;
  const client=boundedClient(()=>new Promise(resolveAck=>{acknowledge=resolveAck;}));
  const session=sessionFor(client);const first=session.startTurn('Synthetic first');
  await assert.rejects(session.startTurn('Synthetic duplicate'),hasKind('contract'));
  assert.equal(client.calls.length,1);acknowledge({turn:{id:'own-turn'}});
  assert.equal(await first,'own-turn');completed(client,'own-turn');
});

test('lost start ACK blocks retry and foreign or old terminal events cannot release it',async()=>{
  let sequence=0;
  const client=boundedClient(async()=>{if(++sequence===1)return {turn:{id:'previous-turn'}};throw new HarnessError('timeout','Synthetic lost ACK');});
  const session=sessionFor(client);await session.startTurn('Synthetic previous');completed(client,'previous-turn');
  await assert.rejects(session.startTurn('Synthetic lost ACK'),hasKind('timeout'));
  completed(client,'previous-turn');completed(client,'unacknowledged-turn');
  await assert.rejects(session.startTurn('Synthetic unsafe retry'),hasKind('contract'));
  assert.equal(client.calls.length,2);assert.equal(session.startPending,false);assert.equal(session.turnAdmission,'uncertain');
  assert.equal(session.turns.has('unacknowledged-turn'),false);
});

test('definite RPC rejection permits a later explicit start, without an automatic retry',async()=>{
  const client=boundedClient(async()=>{if(client.calls.length===1)throw new HarnessError('rpc','Synthetic rate limit',{code:-32029});return {turn:{id:'next-turn'}};});
  const session=sessionFor(client);
  await assert.rejects(session.startTurn('Synthetic rejected'),hasKind('rpc'));
  assert.equal(session.turnAdmission,'idle');assert.equal(client.calls.length,1);
  assert.equal(await session.startTurn('Synthetic explicit request'),'next-turn');completed(client,'next-turn');
});

test('a rejection with contradictory pre-ACK activity stays uncertain',async()=>{
  const client=boundedClient(async()=>{completed(client,'unknown-turn');throw new HarnessError('rpc','Synthetic inconsistent rejection');});
  const session=sessionFor(client);
  await assert.rejects(session.startTurn('Synthetic inconsistent response'),hasKind('rpc'));
  await assert.rejects(session.startTurn('Synthetic retry'),hasKind('contract'));
  assert.equal(client.calls.length,1);assert.equal(session.turnAdmission,'uncertain');
});

test('missing, invalid or reused ACK turn IDs cannot establish new ownership',async()=>{
  for(const id of [null,7,{},'', 'x'.repeat(513)]) {
    const client=boundedClient(async()=>({turn:{id}}));const session=sessionFor(client);
    await assert.rejects(session.startTurn('Synthetic invalid ID'),hasKind('protocol'));
    await assert.rejects(session.startTurn('Synthetic retry'),hasKind('contract'));
    assert.equal(client.calls.length,1);assert.equal(session.turnAdmission,'uncertain');
  }
  const client=boundedClient(async()=>({turn:{id:'reused-turn'}}));const session=sessionFor(client);
  await session.startTurn('Synthetic first');completed(client,'reused-turn');
  await assert.rejects(session.startTurn('Synthetic reused ID'),hasKind('protocol'));
  completed(client,'reused-turn');assert.equal(session.turnAdmission,'uncertain');
});

test('own terminal before start ACK releases admission while another turn does not',async()=>{
  let sequence=0;
  const client=boundedClient(async()=>{const id=`turn-${++sequence}`;completed(client,'different-turn');completed(client,id);return {turn:{id}};});
  const session=sessionFor(client);
  assert.equal(await session.startTurn('Synthetic first'),'turn-1');assert.equal(session.turnAdmission,'idle');
  assert.equal(await session.startTurn('Synthetic second'),'turn-2');assert.equal(session.turns.size,2);
  assert.equal(session.turns.has('different-turn'),false);
});

test('terminal-looking start ACK is not a terminal notification',async()=>{
  const client=boundedClient(async()=>({turn:{id:'own-turn',status:'completed'}}));const session=sessionFor(client);
  await session.startTurn('Synthetic immediate result');
  await assert.rejects(session.startTurn('Synthetic duplicate'),hasKind('contract'));
  assert.equal(session.turns.size,0);completed(client,'own-turn');assert.equal(session.turnAdmission,'idle');
});

test('observer deadlines remain independent and timeout cannot free turn admission',async()=>{
  let sequence=0;const client=boundedClient(async()=>({turn:{id:`turn-${++sequence}`}}));const session=sessionFor(client);
  const id=await session.startTurn('Synthetic');
  const longer=session.waitForTurn(id,500);const short=session.waitForTurn(id,20);
  await assert.rejects(short,hasKind('timeout'));
  assert.equal(session.waiters.get(id).size,1);assert.equal(session.turnAdmission,'uncertain');
  await assert.rejects(session.startTurn('Synthetic duplicate'),hasKind('contract'));
  completed(client,id);assert.equal((await longer).status,'completed');assert.equal(session.waiters.size,0);
  assert.equal(await session.startTurn('Synthetic next'),'turn-2');completed(client,'turn-2');
});

test('interrupt and an existing waiter receive the same terminal; ACK alone does not settle either',async()=>{
  const client=boundedClient(async method=>method==='turn/start'?{turn:{id:'own-turn'}}:{});const session=sessionFor(client);
  const id=await session.startTurn('Synthetic');const original=session.waitForTurn(id);
  let interruptionSettled=false;
  const interruption=session.interrupt(id).then(result=>{interruptionSettled=true;return result;});
  await delay(20);assert.equal(interruptionSettled,false);assert.equal(session.waiters.get(id).size,2);
  await assert.rejects(session.startTurn('Synthetic duplicate'),hasKind('contract'));
  completed(client,id,'interrupted');assert.strictEqual(await original,await interruption);
  assert.equal(session.waiters.size,0);assert.equal(session.interrupts.size,0);
});

test('concurrent interrupt callers share one RPC and terminal outcome',async()=>{
  const client=boundedClient(async method=>{if(method==='turn/start')return {turn:{id:'own-turn'}};await delay(20);return {};});const session=sessionFor(client);
  const id=await session.startTurn('Synthetic');const first=session.interrupt(id),second=session.interrupt(id);
  assert.strictEqual(first,second);assert.equal(client.calls.filter(call=>call.method==='turn/interrupt').length,1);
  completed(client,id,'interrupted');assert.strictEqual(await first,await second);
  assert.equal(session.waiters.size,0);assert.equal(session.interrupts.size,0);
});

test('interrupt RPC failure removes only its observer and preserves an existing waiter',async()=>{
  const client=boundedClient(async method=>{if(method==='turn/start')return {turn:{id:'own-turn'}};throw new HarnessError('rpc','Synthetic interrupt rejection');});const session=sessionFor(client);
  const id=await session.startTurn('Synthetic');const original=session.waitForTurn(id);
  await assert.rejects(session.interrupt(id),hasKind('rpc'));
  assert.equal(session.waiters.get(id).size,1);assert.equal(session.turnAdmission,'uncertain');
  completed(client,id);assert.equal((await original).status,'completed');assert.equal(session.waiters.size,0);
});

test('confirmed terminal survives an interrupt ACK failure and later interruption sends no RPC',async()=>{
  const client=boundedClient(async method=>{if(method==='turn/start')return {turn:{id:'own-turn'}};completed(client,'own-turn','interrupted');throw new HarnessError('timeout','Synthetic lost interrupt ACK');});const session=sessionFor(client);
  const id=await session.startTurn('Synthetic');const result=await session.interrupt(id);
  assert.equal(result.status,'interrupted');const sent=client.calls.length;
  assert.strictEqual(await session.interrupt(id),result);assert.equal(client.calls.length,sent);
  assert.equal(session.waiters.size,0);assert.equal(session.interrupts.size,0);
  assert.equal(session.events.filter(event=>event.type==='attempt.interrupt.requested').length,0);
});

test('interrupt deadline expires without consuming another observer or inventing a terminal',async()=>{
  const client=boundedClient(async method=>method==='turn/start'?{turn:{id:'own-turn'}}:{});const session=sessionFor(client,{interruptTimeoutMs:20});
  const id=await session.startTurn('Synthetic');const original=session.waitForTurn(id,500);
  await assert.rejects(session.interrupt(id),hasKind('timeout'));
  assert.equal(session.turns.size,0);assert.equal(session.waiters.get(id).size,1);assert.equal(session.turnAdmission,'uncertain');
  completed(client,id,'interrupted');assert.equal((await original).status,'interrupted');assert.equal(session.waiters.size,0);
});

test('deadline rejection is consumed while waiting for a later interrupt ACK',async()=>{
  const client=boundedClient(async method=>{if(method==='turn/start')return {turn:{id:'own-turn'}};await delay(60);return {};});const session=sessionFor(client,{interruptTimeoutMs:20});
  const id=await session.startTurn('Synthetic');
  await assert.rejects(session.interrupt(id),hasKind('timeout'));
  assert.equal(session.waiters.size,0);assert.equal(session.interrupts.size,0);assert.equal(session.turns.size,0);
  completed(client,id,'interrupted');assert.equal(session.turnAdmission,'idle');
});

test('transport loss rejects all observers and keeps admission uncertain',async()=>{
  const client=boundedClient(async method=>method==='turn/start'?{turn:{id:'own-turn'}}:{});const session=sessionFor(client);
  const id=await session.startTurn('Synthetic');
  const observers=[session.waitForTurn(id),session.waitForTurn(id),session.interrupt(id)].map(promise=>promise.then(()=>assert.fail('No terminal expected'),error=>error.kind));
  await delay(5);client.fail(new HarnessError('transport','Synthetic stream loss'));
  assert.deepEqual(await Promise.all(observers),['transport','transport','transport']);
  assert.equal(session.waiters.size,0);assert.equal(session.interrupts.size,0);assert.equal(session.turnAdmission,'uncertain');
  await assert.rejects(session.startTurn('Synthetic duplicate'),hasKind('contract'));
});

test('unowned turns, invalid deadlines and excess observers cannot send interruption RPCs',async()=>{
  const client=boundedClient(async()=>({turn:{id:'own-turn'}}));const session=sessionFor(client);
  await assert.rejects(session.interrupt('unknown'),hasKind('contract'));assert.equal(client.calls.length,0);
  const id=await session.startTurn('Synthetic');
  for(const timeout of [0,-1,NaN,Infinity,2147483648])await assert.rejects(session.waitForTurn(id,timeout),hasKind('contract'));
  const observers=Array.from({length:16},()=>session.waitForTurn(id));
  await assert.rejects(session.waitForTurn(id),hasKind('contract'));await assert.rejects(session.interrupt(id),hasKind('contract'));
  assert.equal(client.calls.length,1);completed(client,id);assert.equal((await Promise.all(observers)).length,16);assert.equal(session.waiters.size,0);
});

test('real offline fake shares interruption with a pending waiter and admits the next turn afterward',async t=>{
  const client=new JsonRpcProcess({command:process.execPath,args:[join(root,'fixtures/fake-app-server.mjs'),'--scenario','hang'],cwd:root,requestTimeoutMs:2000});
  t.after(()=>client.stop());
  const session=new CodexSession(client,{turnTimeoutMs:2000,interruptTimeoutMs:1000});
  await session.initialize();await session.startThread({cwd:root,model:'fake-model',effort:'low'});
  const id=await session.startTurn('Offline fixture first');const original=session.waitForTurn(id);
  await assert.rejects(session.startTurn('Blocked duplicate'),hasKind('contract'));
  const interruption=session.interrupt(id);assert.strictEqual(await original,await interruption);
  const next=await session.startTurn('Offline fixture next');assert.notEqual(next,id);
  await assert.rejects(session.waitForTurn(next,20),hasKind('timeout'));
  await assert.rejects(session.startTurn('Blocked after timeout'),hasKind('contract'));
  assert.equal((await session.interrupt(next)).status,'interrupted');assert.equal(session.waiters.size,0);
});
