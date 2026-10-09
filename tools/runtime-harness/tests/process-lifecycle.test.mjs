import test from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter,once} from 'node:events';
import {dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {JsonRpcProcess} from '../src/json-rpc.mjs';

const root=join(dirname(fileURLToPath(import.meta.url)),'..');
const delay=milliseconds=>new Promise(resolve=>setTimeout(resolve,milliseconds));
function isAlive(pid) {try{process.kill(pid,0);return true;}catch(error){if(error.code==='ESRCH')return false;throw error;}}
async function waitUntil(check,timeoutMs) {
  const deadline=Date.now()+timeoutMs;
  while(!check()) {if(Date.now()>=deadline)throw new Error('Synthetic fixture exceeded its deadline');await delay(20);}
}
function script(t,source,options={}) {
  const client=new JsonRpcProcess({command:process.execPath,args:['--input-type=module','-e',source],requestTimeoutMs:1500,...options});
  t.after(()=>client.stop());return client;
}

test('parent exit with a live descendant settles observation without claiming tree termination',{timeout:6000},async t=>{
  const client=new JsonRpcProcess({command:process.execPath,args:[join(root,'fixtures/process-tree.mjs')],requestTimeoutMs:4000,exitDrainTimeoutMs:150});
  t.after(()=>client.stop());
  const [notification]=await once(client,'notification');
  assert.equal(notification.method,'fixture/descendantReady');
  const descendantPid=notification.params.pid;
  // No PID-based destructive cleanup. The fixture expires by its own timer.
  t.after(()=>waitUntil(()=>!isAlive(descendantPid),4000));
  let killCalls=0;const originalKill=client.child.kill.bind(client.child);
  client.child.kill=(...args)=>{killCalls++;return originalKill(...args);};
  const rejected=assert.rejects(client.request('fixture/exitWithUnansweredRequest'),error=>error.kind==='process-exit');
  await once(client.child,'exit');
  assert.equal(client.parentExited,true);assert.equal(client.closed,false);assert.equal(isAlive(descendantPid),true);
  await assert.rejects(client.request('fixture/mustNotStart'),error=>error.kind==='process-exit');
  const stopping=client.stop(40);assert.equal(client.stop(40),stopping);
  const result=await stopping;await rejected;
  assert.equal(result.parentExited,true);assert.equal(result.code,0);
  assert.equal(result.treeTermination,'unknown');assert.equal(result.stdioClosed,false);assert.equal(result.observationDetached,true);
  assert.equal(isAlive(descendantPid),true);assert.equal(killCalls,0);assert.equal(client.pending.size,0);
  assert.deepEqual(await client.stop(),result);
});

test('a response buffered before normal exit is consumed before observation closes',async t=>{
  const source=`import {createInterface} from 'node:readline';const input=createInterface({input:process.stdin});input.once('line',line=>{const request=JSON.parse(line);process.stdout.write(JSON.stringify({id:request.id,result:{acknowledged:true}})+'\\n',()=>process.exit(0));});`;
  const client=script(t,source);
  assert.deepEqual(await client.request('fixture/respondAndExit'),{acknowledged:true});
  const result=await client.stop();
  assert.equal(result.parentExited,true);assert.equal(result.stdioClosed,true);assert.equal(result.observationDetached,false);
  assert.equal(result.treeTermination,'unknown');assert.equal(client.pending.size,0);
});

test('launch failure does not invent a parent process exit',{timeout:3000},async t=>{
  const client=new JsonRpcProcess({command:join(root,'fixtures','does-not-exist-orchestrix-runtime.exe'),requestTimeoutMs:1000});
  t.after(()=>client.stop());
  await assert.rejects(client.request('fixture/unreachable'),error=>error.kind==='launch');
  const result=await client.stop();
  assert.equal(client.pid,undefined);assert.equal(client.spawned,false);assert.equal(result.parentExited,false);
  assert.equal(result.stdioClosed,true);assert.equal(result.treeTermination,'unknown');
});

test('stop kills only its still-running parent after EOF grace and observes the exit',{timeout:3000},async t=>{
  const client=script(t,"process.stdout.write(JSON.stringify({method:'fixture/ready'})+'\\n');process.stdin.resume();setInterval(()=>{},1000);");
  await once(client,'notification');
  const result=await client.stop(80);
  assert.equal(result.parentExited,true);assert.equal(result.treeTermination,'unknown');
  assert.equal(result.stdioClosed,true);assert.equal(result.observationDetached,false);
});

test('an observer detaching during dispatch cannot receive the remaining buffered frames',()=>{
  const client=Object.create(JsonRpcProcess.prototype);EventEmitter.call(client);
  Object.assign(client,{failure:null,closed:false,parentExited:false,buffer:'',decoder:new TextDecoder('utf-8',{fatal:true}),maxMessageBytes:1024,messages:0,
    pending:new Map(),serverRequests:new Set(),resolveClosed:()=>{},child:{stdin:{destroy(){}},stdout:{destroy(){}},stderr:{destroy(){}},unref(){}}});
  const observed=[];
  client.on('notification',message=>{observed.push(message.method);client.detachTransport();});
  client.consume(Buffer.from('{"method":"fixture/first"}\n{"method":"fixture/mustNotDispatch"}\n'));
  assert.deepEqual(observed,['fixture/first']);assert.equal(client.messages,1);assert.equal(client.closed,true);
  assert.equal(client.exit.parentExited,false);assert.equal(client.exit.treeTermination,'unknown');
});

test('reentrant stop callers share the installed operation before shutdown starts',async()=>{
  const client=Object.create(JsonRpcProcess.prototype);client.stopPromise=null;
  let calls=0,nested;
  client.stopOwnedProcess=async()=>{calls++;nested=client.stop(40);return {parentExited:true,treeTermination:'unknown'};};
  const stopping=client.stop(40);const outcome=await stopping;
  assert.equal(nested,stopping);assert.equal(calls,1);assert.equal(outcome.treeTermination,'unknown');
});
