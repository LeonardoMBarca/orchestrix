import test from 'node:test';
import assert from 'node:assert/strict';
import {once} from 'node:events';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {JsonRpcProcess,HarnessError} from '../src/json-rpc.mjs';
import {CodexSession} from '../src/codex-session.mjs';

const root=join(dirname(fileURLToPath(import.meta.url)),'..');
const hasKind=kind=>error=>error instanceof HarnessError&&error.kind===kind;
async function fixture(t,mode='echo',options={}) {
  const client=new JsonRpcProcess({command:process.execPath,args:[join(root,'fixtures/write-backpressure.mjs'),mode],requestTimeoutMs:2000,...options});
  t.after(()=>client.stop(100));
  const writes=[];const original=client.child.stdin.write.bind(client.child.stdin);
  client.child.stdin.write=(chunk,...args)=>{
    const accepted=original(chunk,...args);
    writes.push({bytes:Buffer.byteLength(chunk),accepted});return accepted;
  };
  await once(client,'notification');return {client,writes};
}
const bytes=message=>Buffer.byteLength(`${JSON.stringify(message)}\n`,'utf8');
function sessionFor(client) {
  const session=new CodexSession(client,{turnTimeoutMs:1500});session.threadId='fixture-thread';return session;
}

test('byte budgets reject invalid configuration before any child is launched',()=>{
  for(const name of ['maxMessageBytes','maxOutboundMessageBytes','maxPendingWriteBytes']) {
    for(const value of [0,-1,1.5,NaN,Infinity,Number.MAX_SAFE_INTEGER+1,'1024']) {
      assert.throws(()=>new JsonRpcProcess({command:process.execPath,[name]:value}),error=>hasKind('arguments')(error)&&error.metadata.limit===name);
    }
  }
});

test('outbound boundary counts UTF-8 bytes and LF, rejects before write and keeps the runtime usable',async t=>{
  const params={text:'🧩 café'};
  const frameBytes=bytes({id:1,method:'fixture/echo',params});
  const {client,writes}=await fixture(t,'echo',{maxOutboundMessageBytes:frameBytes});
  assert.deepEqual(await client.request('fixture/echo',params),{received:1,frameBytes});
  await assert.rejects(client.request('fixture/echo',{text:`${params.text}a`}),error=>hasKind('outbound-limit')(error)&&error.metadata.dispatch==='not-written'&&error.metadata.frameBytes===frameBytes+1);
  assert.equal(writes.length,1);assert.equal(client.pending.size,0);assert.equal(client.failure,null);
  assert.equal((await client.request('fixture/echo',{text:'small'})).received,2);
  assert.equal(writes.length,2);
});

test('the trailing newline cannot exceed the outbound frame budget',async t=>{
  const params={text:'🧩'};const frameBytes=bytes({id:1,method:'fixture/echo',params});
  const {client,writes}=await fixture(t,'echo',{maxOutboundMessageBytes:frameBytes-1});
  await assert.rejects(client.request('fixture/echo',params),error=>hasKind('outbound-limit')(error)&&error.metadata.frameBytes===frameBytes);
  assert.equal(writes.length,0);assert.equal(client.pending.size,0);assert.equal(client.failure,null);
});

test('serialization failures are sanitized local rejections with no lingering request or write',async t=>{
  const {client,writes}=await fixture(t);const circular={};circular.self=circular;
  for(const params of [circular,{number:1n},{toJSON(){throw new Error('SYNTHETIC PRIVATE CONTENT');}}]) {
    await assert.rejects(client.request('fixture/invalid',params),error=>hasKind('arguments')(error)&&error.metadata.dispatch==='not-written'&&!error.message.includes('PRIVATE CONTENT'));
    assert.equal(client.pending.size,0);
  }
  assert.throws(()=>client.write(undefined),hasKind('arguments'));
  assert.equal(writes.length,0);assert.equal(client.failure,null);
  assert.equal((await client.request('fixture/echo')).received,1);
});

test('a locally oversized reply does not consume ownership before an explicit valid response',async t=>{
  const {client,writes}=await fixture(t,'echo',{maxOutboundMessageBytes:256});
  const observed=once(client,'request');await client.request('fixture/requestPermission');
  assert.equal((await observed)[0].id,'owned-request');
  assert.throws(()=>client.reply('owned-request',{text:'x'.repeat(256)}),hasKind('outbound-limit'));
  assert.equal(client.serverRequests.has('owned-request'),true);assert.equal(writes.length,1);
  client.reply('owned-request',{decision:'decline'});
  assert.equal(client.serverRequests.has('owned-request'),false);assert.equal(writes.length,2);
  assert.throws(()=>client.reply('owned-request',{}),hasKind('protocol'));
});

test('closing observation during serialization cannot write a frame afterward',async t=>{
  const {client,writes}=await fixture(t);
  await assert.rejects(client.request('fixture/reentrant',{toJSON(){client.detachTransport();return {};}}),hasKind('process-exit'));
  assert.equal(client.closed,true);assert.equal(writes.length,0);assert.equal(client.pending.size,0);
  // detachTransport owns pipe endpoints, not termination; stop the fixture's
  // still-owned parent explicitly rather than leaving its safety timer running.
  if(!client.parentExited){client.child.kill();await once(client.child,'exit');}
});

test('a false Writable return accepts exactly one frame and a draining runtime still responds',async t=>{
  const {client,writes}=await fixture(t);
  const text='x'.repeat(client.child.stdin.writableHighWaterMark+32768);
  const outcome=await client.request('fixture/large',{text});
  assert.equal(outcome.received,1);assert.equal(writes.length,1);assert.equal(writes[0].accepted,false);
  assert.equal(client.failure,null);assert.equal(client.pending.size,0);
});

test('an owned child that never reads stdin cannot grow the Writable queue beyond its byte budget',{timeout:5000},async t=>{
  const budget=128*1024;
  const {client,writes}=await fixture(t,'blocked',{maxOutboundMessageBytes:64*1024,maxPendingWriteBytes:budget});
  let failures=0;client.on('failure',()=>failures++);
  const outcomes=[];
  for(let index=0;index<16&&!client.failure;index++) {
    outcomes.push(client.request('fixture/queued',{text:'x'.repeat(48*1024)}).then(()=>null,error=>error));
  }
  assert.equal(client.failure?.kind,'backpressure');assert.equal(failures,1);
  assert.ok(writes.length>0);assert.ok(writes.reduce((total,write)=>total+write.bytes,0)<=budget);
  assert.equal(client.pending.size,0);
  const written=writes.length;
  await assert.rejects(client.request('fixture/mustNotRetry'),hasKind('backpressure'));
  assert.equal(writes.length,written);
  assert.ok((await Promise.all(outcomes)).every(error=>hasKind('backpressure')(error)));
  const stopped=await client.stop(100);
  assert.equal(stopped.parentExited,true);assert.equal(stopped.treeTermination,'unknown');
});

test('a rejected local start admits a later explicit start without inventing a turn or terminal',async t=>{
  const {client,writes}=await fixture(t,'echo',{maxOutboundMessageBytes:1024});const session=sessionFor(client);
  await assert.rejects(session.startTurn('x'.repeat(2048)),hasKind('outbound-limit'));
  await assert.rejects(session.startTurn(1n),hasKind('arguments'));
  assert.equal(writes.length,0);assert.equal(session.turnAdmission,'idle');assert.equal(session.startPending,false);
  assert.equal(session.knownTurns.size,0);assert.equal(session.turns.size,0);assert.equal(session.events.length,0);
  const id=await session.startTurn('Explicit smaller request');
  assert.equal((await session.waitForTurn(id)).status,'completed');assert.equal(writes.length,1);
  assert.equal(session.events.filter(event=>event.type==='attempt.turn.ended').length,1);
});

test('backpressure after an accepted start preserves uncertainty and blocks a second admission',{timeout:5000},async t=>{
  const {client,writes}=await fixture(t,'blocked',{maxOutboundMessageBytes:64*1024,maxPendingWriteBytes:128*1024});
  const session=sessionFor(client);const starting=session.startTurn('x'.repeat(32*1024));
  const rejected=assert.rejects(starting,hasKind('backpressure'));
  for(let index=0;index<16&&!client.failure;index++) {
    try{client.notify('fixture/fill',{text:'x'.repeat(48*1024)});}catch(error){assert.ok(hasKind('backpressure')(error));}
  }
  await rejected;const written=writes.length;
  assert.equal(session.turnAdmission,'uncertain');assert.equal(session.startPending,false);assert.equal(client.pending.size,0);
  assert.equal(session.knownTurns.size,0);assert.equal(session.turns.size,0);
  assert.equal(session.events.some(event=>event.type==='attempt.turn.ended'),false);
  assert.equal(session.events.at(-1).type,'attempt.observation.unknown');
  await assert.rejects(session.startTurn('Unsafe second start'),hasKind('contract'));
  assert.equal(writes.length,written);
});
