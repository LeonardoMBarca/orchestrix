import test from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {resolve,dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {JsonRpcProcess} from '../src/json-rpc.mjs';
import {CodexSession} from '../src/codex-session.mjs';
import {runProbe,runtimeEnvironment} from '../probe.mjs';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const delay=milliseconds=>new Promise(resolveDelay=>setTimeout(resolveDelay,milliseconds));
function clientFor(scenario,options={}) {
  return new JsonRpcProcess({command:process.execPath,args:[join(root,'fixtures/fake-app-server.mjs'),'--scenario',scenario],cwd:root,requestTimeoutMs:2000,...options});
}
async function prepared(t,scenario='success',options={}) {
  const client=clientFor(scenario,options);t.after(()=>client.stop());
  const session=new CodexSession(client,{turnTimeoutMs:2000});
  await session.initialize();const discovery=await session.discover();
  assert.equal(discovery.auth.subscriptionGate,true);assert.equal(discovery.availability,true);
  await session.startThread({cwd:root,model:'fake-model',effort:'low'});
  return {client,session};
}
function scriptClient(t,source,options={}) {
  const client=new JsonRpcProcess({command:process.execPath,args:['--input-type=module','-e',source],cwd:root,requestTimeoutMs:1000,...options});
  t.after(()=>client.stop());return client;
}

test('success preserves requested/resolved settings and terminal evidence',async t=>{
  const {session,client}=await prepared(t);const id=await session.startTurn('Synthetic',{effort:'low'});
  const result=await session.waitForTurn(id);
  assert.equal(result.status,'completed');assert.equal(session.configuration.resolvedConfiguration.effort,'low');
  assert.equal(session.configuration.resolvedConfiguration.ephemeral,true);
  assert.equal(client.serverRequests.size,0);assert.match(session.deltaText.get(id),/Synthetic success/);
});
test('split UTF-8, CRLF and chunks preserve the complete message',async t=>{
  const {session}=await prepared(t,'split-utf8');const id=await session.startTurn('Synthetic');await session.waitForTurn(id);
  assert.equal(session.deltaText.get(id),'Olá 🧩 — café: resultado sintético, sem execução real.\nSegunda linha no mesmo delta.');
});
test('stdout stays responsive while more than one MiB of stderr is drained',async t=>{
  const {session,client}=await prepared(t,'noisy-stderr');const id=await session.startTurn('Synthetic');await session.waitForTurn(id);
  assert.ok(client.stderrBytes>1024*1024);assert.ok(!Object.hasOwn(client,'stderrText'));
  assert.ok(!JSON.stringify(session.events).includes('SYNTHETIC STDERR'));
});
test('command approval is declined by the exact server request ID',async t=>{
  const {session,client}=await prepared(t,'permission');const id=await session.startTurn('Synthetic');const outcome=await session.waitForTurn(id);
  assert.equal(outcome.status,'completed');assert.equal(client.serverRequests.size,0);
  assert.equal(session.events.filter(event=>event.type==='permission.denied').length,1);
  assert.ok(!JSON.stringify(session.events).includes('synthetic fixture command'));
});
test('permission requests grant an empty turn scope and unknown requests get errors',()=>{
  const client=new EventEmitter();const replies=[];client.reply=(id,result)=>replies.push({id,result});client.rejectRequest=id=>replies.push({id,rejected:true});client.fail=error=>{throw error;};
  const session=new CodexSession(client);session.threadId='thread-1';
  client.emit('request',{id:'p1',method:'item/permissions/requestApproval',params:{threadId:'thread-1',permissions:{network:true},secret:'SYNTHETIC_SECRET'}});
  client.emit('request',{id:'p2',method:'unknown/request',params:null});
  assert.deepEqual(replies,[{id:'p1',result:{permissions:{},scope:'turn'}},{id:'p2',rejected:true}]);
  assert.ok(!JSON.stringify(session.events).includes('SYNTHETIC_SECRET'));
});
test('malformed runtime JSON fails closed and confirms only the parent exit',async t=>{
  const {session,client}=await prepared(t,'malformed');
  await assert.rejects(async()=>session.waitForTurn(await session.startTurn('Synthetic')),error=>error.kind==='protocol');
  const exit=await client.stop();assert.equal(exit.parentExited,true);assert.equal(exit.treeTermination,'unknown');
  assert.equal(session.turns.size,0);
});
test('runtime crash is unknown outcome, never a completed task',async t=>{
  const {session}=await prepared(t,'crash');
  await assert.rejects(async()=>session.waitForTurn(await session.startTurn('Synthetic')),error=>error.kind==='process-exit');
  assert.equal(session.turns.size,0);
});
test('timeout keeps the result unknown and interruption awaits the terminal notification',async t=>{
  const {session}=await prepared(t,'hang');const id=await session.startTurn('Synthetic');
  await assert.rejects(session.waitForTurn(id,40),error=>error.kind==='timeout');
  assert.equal(session.turns.has(id),false);
  assert.equal(session.events.at(-1).type,'attempt.observation.unknown');
  const cancelled=await session.interrupt(id);assert.equal(cancelled.status,'interrupted');
});
test('slow turn can be interrupted; acknowledgement alone is not the result',async t=>{
  const {session}=await prepared(t,'slow');const id=await session.startTurn('Synthetic');const result=await session.interrupt(id);
  assert.equal(result.status,'interrupted');
  assert.ok(session.events.some(event=>event.type==='attempt.interrupt.requested'));
  assert.ok(session.events.some(event=>event.type==='attempt.turn.ended'&&event.status==='interrupted'));
});
test('unknown valid notification is diagnosed without exposing its payload',async t=>{
  const {session}=await prepared(t,'unknown-notification');const id=await session.startTurn('Synthetic');await session.waitForTurn(id);
  assert.ok(session.events.some(event=>event.method==='fixture/unknownNotification'));
  assert.ok(!JSON.stringify(session.events).includes('"synthetic":true'));
});
test('auth failure blocks before creating a thread or sending a turn',async()=>{
  const report=await runProbe({mode:'fake',scenario:'auth-failure'});
  assert.equal(report.status,'blocked-auth');assert.equal(report.liveStarted,false);
  assert.equal(report.events.some(event=>event.type==='session.started'),false);
  assert.equal(report.workspace.filesUnchanged,true);assert.equal(report.process.parentExited,true);
});
test('synthetic rate-limit response is sanitized and does not trigger retries',async t=>{
  const {session}=await prepared(t,'rate-limit');
  await assert.rejects(session.startTurn('Synthetic'),error=>error.kind==='rpc'&&error.metadata.code===-32029);
  assert.equal(session.knownTurns.size,0);assert.equal(session.events.filter(event=>event.type==='attempt.turn.started').length,0);
});
test('two fake processes keep identical provider IDs isolated by attempt ownership',async t=>{
  const [left,right]=await Promise.all([prepared(t),prepared(t)]);left.session.attemptId='attempt-left';right.session.attemptId='attempt-right';
  const ids=await Promise.all([left.session.startTurn('left'),right.session.startTurn('right')]);
  assert.equal(ids[0],ids[1]);await Promise.all([left.session.waitForTurn(ids[0]),right.session.waitForTurn(ids[1])]);
  assert.notEqual(left.client.pid,right.client.pid);
  assert.ok(left.session.events.filter(event=>event.type==='attempt.turn.ended').every(event=>event.attemptId==='attempt-left'));
  assert.ok(right.session.events.filter(event=>event.type==='attempt.turn.ended').every(event=>event.attemptId==='attempt-right'));
});
test('out-of-order responses correlate by ID, with numeric and string IDs distinct',async t=>{
  const source=`import {createInterface} from 'node:readline';const lines=createInterface({input:process.stdin});let messages=[];lines.on('line',line=>{messages.push(JSON.parse(line));if(messages.length===2){process.stdout.write(JSON.stringify({id:'1',result:{method:'wrong-string-id'}})+'\\n');for(const message of messages.reverse())process.stdout.write(JSON.stringify({id:message.id,result:{method:message.method}})+'\\n');}});`;
  const client=scriptClient(t,source);const results=await Promise.all([client.request('first'),client.request('second')]);
  assert.deepEqual(results,[{method:'first'},{method:'second'}]);
});
test('timed-out request does not consume the following request response',async t=>{
  const source=`import {createInterface} from 'node:readline';const lines=createInterface({input:process.stdin});lines.on('line',line=>{const message=JSON.parse(line);setTimeout(()=>process.stdout.write(JSON.stringify({id:message.id,result:message.method})+'\\n'),message.method==='late'?100:5);});`;
  const client=scriptClient(t,source);await assert.rejects(client.request('late',{},60),error=>error.kind==='timeout');
  assert.equal(await client.request('next'),'next');await delay(110);assert.equal(client.pending.size,0);
});
test('invalid UTF-8 and oversized frame reject the protocol',async t=>{
  const invalid=scriptClient(t,"process.stdout.write(Buffer.from([255,10]));setInterval(()=>{},1000);");
  await assert.rejects(invalid.request('probe'),error=>error.kind==='protocol');
  const oversized=scriptClient(t,"process.stdout.write('x'.repeat(300));setInterval(()=>{},1000);",{maxMessageBytes:128});
  await assert.rejects(oversized.request('probe'),error=>error.kind==='protocol');
});
test('a terminal notification before the start ACK is buffered for the returned turn only',async()=>{
  const client=new EventEmitter();client.request=async()=>{client.emit('notification',{method:'turn/completed',params:{threadId:'own-thread',turn:{id:'own-turn',status:'completed'}}});client.emit('notification',{method:'turn/completed',params:{threadId:'own-thread',turn:{id:'other-turn',status:'completed'}}});return {turn:{id:'own-turn'}};};
  const session=new CodexSession(client);session.threadId='own-thread';
  const id=await session.startTurn('Synthetic');assert.equal((await session.waitForTurn(id)).status,'completed');assert.equal(session.turns.has('other-turn'),false);
  session.observe({method:'turn/completed',params:null});session.observe({method:'turn/completed',params:{threadId:'other-thread',turn:{id:'own-turn',status:'failed'}}});
  assert.equal(session.turns.get(id).status,'completed');
});
test('a lost turn-start ACK does not issue a duplicate turn or claim completion',async()=>{
  const client=new EventEmitter();let starts=0;client.request=async()=>{starts++;throw Object.assign(new Error('Synthetic timeout'),{kind:'timeout'});};
  const session=new CodexSession(client);session.threadId='own-thread';
  await assert.rejects(session.startTurn('Synthetic'));assert.equal(starts,1);assert.equal(session.startPending,false);assert.equal(session.turns.size,0);
  assert.equal(session.events.at(-1).type,'attempt.observation.unknown');
});
test('event and cumulative response limits bound an otherwise valid stream',()=>{
  const client=new EventEmitter();const failures=[];client.fail=error=>failures.push(error.kind);
  const session=new CodexSession(client);session.threadId='t';session.knownTurns.add('u');
  session.observe({method:'item/agentMessage/delta',params:{threadId:'t',turnId:'u',delta:'x'.repeat(65537)}});
  assert.deepEqual(failures,['protocol']);assert.equal(session.deltaText.size,0);
  session.events=Array(4096).fill({});session.event('extra');assert.deepEqual(failures,['protocol','protocol']);assert.equal(session.events.length,4096);
});
test('terminal state cannot be overwritten by a conflicting provider notification',()=>{
  const client=new EventEmitter();const failures=[];client.fail=error=>failures.push(error.kind);
  const session=new CodexSession(client);session.threadId='t';session.knownTurns.add('u');
  for(const status of ['completed','completed','failed'])session.observe({method:'turn/completed',params:{threadId:'t',turn:{id:'u',status}}});
  assert.equal(session.turns.get('u').status,'completed');assert.deepEqual(failures,['protocol']);
  assert.equal(session.events.filter(event=>event.type==='attempt.turn.ended').length,1);
});
test('overflow before a turn-start ACK fails instead of silently dropping a terminal event',()=>{
  const client=new EventEmitter();const failures=[];client.fail=error=>failures.push(error.kind);
  const session=new CodexSession(client);session.threadId='t';session.startPending=true;
  for(let index=0;index<129;index++)session.observe({method:'turn/completed',params:{threadId:'t',turn:{id:`u${index}`,status:'completed'}}});
  assert.equal(session.earlyNotifications.length,128);assert.deepEqual(failures,['protocol']);assert.equal(session.turns.size,0);
});
test('endpoint/resource discovery does not accept falsy or malformed config as a safe boundary',async()=>{
  const client=new EventEmitter();let config={model_provider:'openai',chatgpt_base_url:'https://chatgpt.com/backend-api/',openai_base_url:false,mcp_servers:[]};client.request=async()=>({config});
  const session=new CodexSession(client);let result=await session.inspectEndpoints(root);
  assert.equal(result.officialEndpointConfirmed,false);assert.equal(result.resources.mcpDisabled,false);assert.equal(session.mcpDisableOverrides,null);
  config={...config,openai_base_url:'',mcp_servers:{example:{enabled:true}}};result=await session.inspectEndpoints(root);
  assert.equal(result.officialEndpointConfirmed,true);assert.equal(result.resources.mcpDisabled,false);assert.deepEqual(session.mcpDisableOverrides,['mcp_servers.example.enabled=false']);
  config.mcp_servers={example:{enabled:false}};result=await session.inspectEndpoints(root);assert.equal(result.resources.mcpDisabled,true);
});
test('API credentials, endpoint overrides and executable hooks are absent from the child env',()=>{
  const env=runtimeEnvironment({Path:'os-path',SystemRoot:'os-root',CODEX_HOME:'auth-location',OPENAI_API_KEY:'SYNTHETIC_SECRET',CODEX_API_KEY:'SYNTHETIC_SECRET',CODEX_REFRESH_TOKEN_URL_OVERRIDE:'https://synthetic.invalid',NODE_OPTIONS:'--require injection',UNRELATED_SECRET:'SYNTHETIC_SECRET'});
  assert.deepEqual(env,{Path:'os-path',SystemRoot:'os-root',CODEX_HOME:'auth-location'});assert.ok(!JSON.stringify(env).includes('SYNTHETIC_SECRET'));
});
