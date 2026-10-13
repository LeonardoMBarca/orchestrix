import test from 'node:test';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {mkdtemp,writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {JsonRpcProcess,HarnessError} from '../src/json-rpc.mjs';
import {WindowsJobProcess} from '../src/windows-job-process.mjs';
import {CodexSession} from '../src/codex-session.mjs';
import {runtimeEnvironment} from '../src/runtime-environment.mjs';
import {observeCancellation} from '../probe.mjs';
import {observeOwnedProcessStop} from '../worktree-probe.mjs';

const root=join(dirname(fileURLToPath(import.meta.url)),'..');
const options={skip:process.platform!=='win32',timeout:20000};
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const alive=pid=>{try{process.kill(pid,0);return true;}catch(error){if(error.code==='ESRCH')return false;throw error;}};
async function until(check,ms=4000){const end=Date.now()+ms;while(!check()){if(Date.now()>=end)throw new Error('Synthetic state deadline exceeded');await delay(10);}}
function client(t,{scenario='success',fixture='fake-app-server.mjs',args,job={},...rest}={}) {
  const c=new JsonRpcProcess({command:process.execPath,args:[join(root,'fixtures',fixture),...(args||['--scenario',scenario])],cwd:root,env:runtimeEnvironment(),windowsJob:{lifetimeMs:10000,...job},...rest});
  t.after(()=>c.stop(100));return c;
}
async function session(c,{attemptId='owned-attempt',turnTimeoutMs=2000}={}) {
  await c.child.ready;const s=new CodexSession(c,{attemptId,turnTimeoutMs,interruptTimeoutMs:1500});
  await s.initialize();await s.discover();await s.startThread({cwd:root,model:'synthetic-announced-model',effort:'low'});return s;
}
function confirmed(state,{code,termination}={}) {
  assert.equal(state.parentExited,true);assert.equal(state.treeTermination,'confirmed');assert.equal(state.activeCount,0);assert.equal(state.ownershipConfirmed,true);
  assert.equal(state.helper.parentExited,true);assert.equal(state.helper.stdioClosed,true);
  if(code!==undefined)assert.equal(state.code,code);
  if(termination)assert.equal(state.termination,termination);
}

test('native launch preserves the Windows platform drive without forwarding secrets or hooks',options,async t=>{
  const env=runtimeEnvironment();const drive=Object.entries(env).find(([key])=>key.toUpperCase()==='SYSTEMDRIVE')?.[1];
  assert.ok(drive,'Windows SystemDrive is required for this integration regression');
  const code="console.log(JSON.stringify({drive:process.env.SystemDrive,api:!!process.env.OPENAI_API_KEY,hook:!!process.env.NODE_OPTIONS,other:!!process.env.UNRELATED_SECRET}))";
  const child=new WindowsJobProcess({command:process.execPath,args:['-e',code],cwd:root,env:{...env,OPENAI_API_KEY:'SYNTHETIC_SECRET',NODE_OPTIONS:'--require nonexistent-injection',UNRELATED_SECRET:'SYNTHETIC_SECRET'},lifetimeMs:10000});
  t.after(()=>child.stop(100));let output='';child.stdout.on('data',chunk=>{output+=chunk.toString();});child.stderr.resume();
  await child.ready;child.stdin.end();await child.finished.promise;
  const state=await child.stop(100);confirmed(state,{code:0,termination:'graceful'});assert.equal(state.stdioClosed,true);
  assert.deepEqual(JSON.parse(output),{drive,api:false,hook:false,other:false});
});

test('raw UTF-8, JSON-RPC correlation and helper control remain separate',options,async t=>{
  const c=client(t,{scenario:'split-utf8'});const s=await session(c);
  const results=await Promise.all([c.request('account/read'),c.request('model/list'),c.request('account/read')]);
  assert.equal(results[0].account.type,'chatgpt');assert.ok(Array.isArray(results[1].data));assert.deepEqual(results[0],results[2]);
  const id=await s.startTurn('Synthetic bounded message');assert.equal((await s.waitForTurn(id)).status,'completed');
  assert.ok(s.events.every(event=>event.attemptId==='owned-attempt'));assert.equal(c.failure,null);
  const stopped=await c.stop();confirmed(stopped,{code:0,termination:'graceful'});assert.equal(stopped.stdioClosed,true);
});
test('cooperative turn interruption does not terminate the Job or imply process exit',options,async t=>{
  const c=client(t,{scenario:'hang'});const s=await session(c);const id=await s.startTurn('Synthetic cancellation');
  assert.equal((await observeCancellation(s,id)).interruptedConfirmed,true);
  assert.equal(c.parentExited,false);assert.equal(c.child.supervision().treeTermination,'unknown');
  assert.equal((await c.request('account/read')).account.type,'chatgpt');
  const stopped=await c.stop();confirmed(stopped,{code:0,termination:'graceful'});
});
test('root exit with inherited pipes held by descendant still cleans only the owned Job',options,async t=>{
  const c=client(t,{fixture:'job-app-server.mjs',args:['descendant']});
  let descendant;c.on('notification',m=>{if(m.method==='fixture/descendant')descendant=m.params.pid;});
  await session(c);await until(()=>!!descendant);assert.equal((await c.child.inspect(descendant)).inOwnedJob,true);
  const outsider=spawn(process.execPath,['-e','setTimeout(()=>{},20000)'],{env:runtimeEnvironment(),windowsHide:true,stdio:'ignore'});t.after(()=>{if(alive(outsider.pid))outsider.kill();});
  await once(outsider,'spawn');assert.equal((await c.child.inspect(outsider.pid)).inOwnedJob,false);
  c.notify('fixture/exit-root');await c.closedPromise;
  assert.equal(c.parentExited,true);assert.equal(c.exit.treeTermination,'unknown');assert.equal(c.exit.stdioClosed,false);assert.equal(alive(descendant),true);
  const first=c.stop(100),second=c.stop(100);assert.equal(first,second);const stopped=await first;
  confirmed(stopped,{code:0,termination:'forced'});await until(()=>!alive(descendant));assert.equal(alive(outsider.pid),true);
});
test('runtime crash is distinct from helper success and never produces a completed turn',options,async t=>{
  const c=client(t,{scenario:'crash'});const s=await session(c);const id=await s.startTurn('Synthetic crash');
  await assert.rejects(s.waitForTurn(id),error=>error instanceof HarnessError&&error.kind==='process-exit');
  const stopped=await c.stop(100);confirmed(stopped,{code:23,termination:'graceful'});assert.equal(stopped.helper.code,0);
  assert.equal(s.events.some(event=>event.type==='attempt.completed'),false);
});
test('runtime stdin EOF is separate from helper control EOF',options,async t=>{
  const c=client(t);await c.child.ready;await c.request('initialize');c.child.stdin.end();await c.closedPromise;
  const stopped=await c.stop(100);confirmed(stopped,{code:0,termination:'graceful'});assert.equal(stopped.stdioClosed,true);
});
test('control EOF terminates an active owned runtime, not a cooperative turn',options,async t=>{
  const c=client(t,{scenario:'hang'});const s=await session(c);const id=await s.startTurn('Synthetic active turn');await s.waitForTurnStart(id);
  const terminal=s.waitForTurn(id).catch(error=>error);c.child.helper.stdin.end();const stopped=await c.stop(100);
  confirmed(stopped,{code:137,termination:'forced'});assert.equal(c.child.stopped.reason,'control-eof');
  assert.equal((await terminal).kind,'process-exit');assert.equal(s.turns.get(id)?.status==='interrupted',false);
});
test('independent native lifetime deadline stops an unresponsive runtime',options,async t=>{
  const c=client(t,{fixture:'job-app-server.mjs',args:['blocked-input'],job:{lifetimeMs:1000}});await c.child.ready;
  await c.child.finished.promise;const stopped=await c.stop(100);confirmed(stopped,{code:137,termination:'forced'});
  assert.equal(c.child.stopped.reason,'deadline');assert.equal(stopped.helper.code,2);
});
test('turn timeout retains uncertain remote state until owned Job stop',options,async t=>{
  const c=client(t,{scenario:'hang'});const s=await session(c,{turnTimeoutMs:100});const id=await s.startTurn('Synthetic timeout');
  await assert.rejects(s.waitForTurn(id),error=>error.kind==='timeout');assert.equal(c.parentExited,false);assert.equal(c.child.supervision().treeTermination,'unknown');
  const stopped=await c.stop(100);confirmed(stopped,{code:137,termination:'forced'});
});
test('extensive stderr is drained without consuming runtime/control frames',options,async t=>{
  const c=client(t,{scenario:'noisy-stderr'});const s=await session(c);const id=await s.startTurn('Synthetic stderr');
  assert.equal((await s.waitForTurn(id)).status,'completed');assert.ok(c.stderrBytes>65536);assert.equal(c.failure,null);
  confirmed(await c.stop(),{code:0,termination:'graceful'});
});
test('blocked raw stdout remains bounded while Job control and watchdog can stop',options,async t=>{
  const c=client(t,{fixture:'job-app-server.mjs',args:['flood-output'],job:{lifetimeMs:1000}});await c.child.ready;
  c.child.stdout.pause();c.child.stdout.removeAllListeners('data');
  await c.child.finished.promise;const stopped=await c.stop(100);confirmed(stopped,{code:137,termination:'forced'});
  assert.ok(c.child.stdout.readableLength<=131072);assert.ok(c.child.stdout.writableLength<=131072);
  assert.equal(c.child.stopped.reason,'deadline');
  // Closing an observation endpoint is not equivalent to draining buffered output.
  assert.equal(stopped.stdioClosed,false);
});
test('blocked runtime stdin preserves write budget and cleans the owned Job',options,async t=>{
  const budget=128*1024,c=client(t,{fixture:'job-app-server.mjs',args:['blocked-input'],maxPendingWriteBytes:budget,maxOutboundMessageBytes:65536});
  await c.child.ready;
  for(let i=0;i<16&&!c.failure;i++)try{c.notify('fixture/queued',{text:'x'.repeat(48*1024)});}catch{}
  assert.equal(c.failure?.kind,'backpressure');assert.ok(c.child.stdin.writableLength<=budget);assert.equal(c.pending.size,0);
  confirmed(await c.stop(100),{code:137,termination:'forced'});
});
test('malformed runtime bytes fail protocol without contaminating helper control',options,async t=>{
  const c=client(t,{scenario:'malformed'});const s=await session(c);
  await assert.rejects(async()=>{const id=await s.startTurn('Synthetic malformed');await s.waitForTurn(id);},error=>error.kind==='protocol');assert.equal(c.failure.kind,'protocol');
  const stopped=await c.stop(100);confirmed(stopped);assert.ok(c.child.controlSequence>=3);assert.equal(c.child.controlFaulted,undefined);
});
test('explicit forced process proof never invents turn interruption',options,async t=>{
  const c=client(t,{scenario:'hang'});const s=await session(c);const id=await s.startTurn('Synthetic forced proof');
  const result=await observeOwnedProcessStop(s,id);
  assert.equal(result.forcedConfirmed,true);assert.equal(result.turnInterruptedConfirmed,false);assert.equal(result.terminalStatus,null);
  assert.equal(s.events.some(event=>event.type==='attempt.interrupt.requested'),false);
  const stopped=await c.stop();confirmed(stopped,{code:137,termination:'forced'});assert.equal(c.child.stopped.reason,'requested');
});
test('missing executable fails admission without claiming root/tree exit',options,async t=>{
  const c=new JsonRpcProcess({command:join(root,'missing-synthetic.exe'),cwd:root,windowsJob:true});t.after(()=>c.stop(100));
  await assert.rejects(c.request('initialize'),error=>error.kind==='launch');const stopped=await c.stop(100);
  assert.equal(stopped.parentExited,false);assert.equal(stopped.treeTermination,'unknown');assert.equal(stopped.ownershipConfirmed,false);assert.equal(c.child.helper,undefined);
});
test('stop before admission does not launch a runtime or helper',options,async()=>{
  const child=new WindowsJobProcess({command:process.execPath,args:['-e','process.exit(0)'],cwd:root});child.on('error',()=>{});
  const stopped=await child.stop(100);await assert.rejects(child.ready,error=>error.kind==='cancelled');
  await delay(20);assert.equal(child.helper,undefined);assert.equal(stopped.parentExited,false);assert.equal(stopped.treeTermination,'unknown');
});
test('helper loss keeps tree termination unknown even though its Job handle closes',options,async t=>{
  const c=client(t,{scenario:'hang'});await c.child.ready;const pid=c.pid;c.child.helper.kill();await c.child.finished.promise;
  const stopped=await c.stop(100);assert.equal(stopped.helper.parentExited,true);assert.equal(stopped.treeTermination,'unknown');
  await until(()=>!alive(pid));assert.equal(stopped.parentExited,false);
});
test('a buffered final reply drains before transport closes',options,async t=>{
  const c=client(t,{fixture:'job-app-server.mjs',args:['buffered-final']});await c.child.ready;c.child.stdout.pause();
  const reply=c.request('fixture/final');await until(()=>c.parentExited);assert.equal(c.closed,false);
  c.child.stdout.resume();assert.deepEqual(await reply,{text:'final 🧩'});
  confirmed(await c.stop(),{code:0,termination:'graceful'});
});
test('incomplete UTF-8 at EOF remains a protocol failure, not parent success',options,async t=>{
  const c=client(t,{fixture:'job-app-server.mjs',args:['incomplete-utf8']});await c.child.ready;
  await assert.rejects(c.request('fixture/final'),error=>error.kind==='protocol');assert.equal(c.failure.kind,'protocol');
  const stopped=await c.stop(100);confirmed(stopped,{code:0});
});
test('native launch failure reports no runtime ownership and preserves its category',options,async t=>{
  const directory=await mkdtemp(join(tmpdir(),'orchestrix-native-launch-'));const binary=join(directory,'invalid.exe');await writeFile(binary,'Synthetic non-executable fixture');
  const c=new JsonRpcProcess({command:binary,cwd:root,windowsJob:true});t.after(()=>c.stop(100));
  await assert.rejects(c.request('initialize'),error=>error.kind==='native-launch'&&error.metadata.category==='launch');
  const stopped=await c.stop(100);assert.equal(stopped.parentExited,false);assert.equal(stopped.ownershipConfirmed,false);assert.equal(stopped.treeTermination,'unknown');assert.equal(c.child.controlFaulted,undefined);
});
test('startup timeout cannot admit a runtime after the expired launch budget',options,async t=>{
  const c=client(t,{job:{startupTimeoutMs:30}});
  await assert.rejects(c.child.ready,error=>error.kind==='launch-timeout');
  const stopped=await c.stop(100);assert.equal(stopped.ownershipConfirmed,false);assert.equal(stopped.parentExited,false);assert.equal(stopped.treeTermination,'unknown');assert.equal(c.pid,undefined);
  if(c.child.helper)assert.equal(stopped.helper.parentExited,true);
});
