import test from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {PassThrough} from 'node:stream';
import {fileURLToPath} from 'node:url';
import {dirname,resolve} from 'node:path';
import {ExecJsonProcess,execArguments,runExecProbe} from '../exec-probe.mjs';

const directory=dirname(fileURLToPath(import.meta.url));
const fake=resolve(directory,'../fixtures/fake-exec.mjs');
const id='00000000-0000-4000-8000-000000000001';
const delay=ms=>new Promise(resolveDelay=>setTimeout(resolveDelay,ms));
function fixture(t,scenario='success',options={}) {
  const runner=new ExecJsonProcess({command:process.execPath,args:[fake,'--scenario',scenario],cwd:directory,timeoutMs:5000,startTimeoutMs:3000,...options});
  t.after(()=>runner.stop({graceMs:500}));return runner;
}
function fakeJob({readyDelay=0,confirmed=true,start=true}={}) {
  const child=new EventEmitter();child.stdin=new PassThrough();child.stdout=new PassThrough();child.stderr=new PassThrough();child.stops=0;
  child.ready=new Promise(resolveReady=>{setTimeout(()=>{child.emit('spawn');resolveReady(child);if(start)child.stdout.write(JSON.stringify({type:'thread.started',thread_id:id})+'\n'+JSON.stringify({type:'turn.started'})+'\n');},readyDelay);});
  child.stop=async graceMs=>{
    assert.ok(graceMs>=1);child.stops++;child.stdin.end();child.stdout.end();child.stderr.end();child.emit('exit',0,null);child.emit('close',0,null);
    return {code:0,parentExited:true,stdioClosed:true,treeTermination:confirmed?'confirmed':'unknown',activeCount:confirmed?0:null,ownershipConfirmed:true,helper:{code:0,parentExited:true,stdioClosed:true},helperStreams:{faulted:false,stdinClosed:true,stdoutClosed:true,stderrClosed:true}};
  };
  return child;
}

test('CLI success requires the owned thread, start, terminal, exit and closed byte streams',async t=>{
  const runner=fixture(t);const result=await runner.wait();
  assert.equal(result.passed,true);assert.equal(result.threadObserved,true);assert.equal(result.sameExpectedThread,null);
  assert.equal(result.startObserved,true);assert.equal(result.terminalStatus,'completed');assert.equal(result.process.parentExited,true);assert.equal(result.process.stdioClosed,true);
  assert.deepEqual(result.usage,{input_tokens:21,cached_input_tokens:3,output_tokens:8,reasoning_output_tokens:0});assert.deepEqual(result.items,{agent_message:1});
  assert.match(runner.agentText,/caf\u00e9/);assert.equal(runner.threadId,id);
  const serialized=JSON.stringify(result);assert.equal(serialized.includes(id),false);assert.equal(serialized.includes('Synthetic CLI response'),false);
  assert.equal(result.events[0].type,'session.observed');assert.equal(runner.startWaiters.size,0);
});

test('exact resume identity is observed without reporting the persisted thread identifier',async t=>{
  const runner=fixture(t,'success',{expectedThreadId:id});const result=await runner.wait();
  assert.equal(result.passed,true);assert.equal(result.sameExpectedThread,true);assert.equal(JSON.stringify(result).includes(id),false);
});

test('fragmented UTF-8 and CRLF JSONL preserve text and optional usage counters',async t=>{
  const runner=fixture(t,'split-utf8');const result=await runner.wait();
  assert.equal(result.passed,true);assert.match(runner.agentText,/caf\u00e9/);assert.match(runner.agentText,/\u{1f9e9}/u);
  assert.deepEqual(result.usage,{input_tokens:21,cached_input_tokens:3,output_tokens:8});
});

test('stderr is drained and counted, while unknown event payloads remain unrecorded',async t=>{
  for(const scenario of ['noisy-stderr','unknown','unknown-item']) {
    const runner=fixture(t,scenario);const result=await runner.wait();assert.equal(result.passed,true);
    assert.equal(JSON.stringify(result).includes('SYNTHETIC_SECRET'),false);
    if(scenario==='noisy-stderr')assert.ok(result.metrics.stderrBytesDrained>1024*1024);
    if(scenario==='unknown')assert.equal(result.unknownEvents,1);
    if(scenario==='unknown-item'){assert.deepEqual(result.items,{unknown:1});assert.equal(result.events.find(event=>event.type==='runtime.item').kind,'unknown');}
  }
});

for(const scenario of ['malformed','invalid-utf8','incomplete-json','bad-envelope','terminal-before-thread','terminal-before-start','duplicate-thread','duplicate-terminal','bad-item','contradictory-item','negative-usage']) {
  test(`CLI protocol rejects ${scenario} without exposing raw output`,async t=>{
    const runner=fixture(t,scenario);const result=await runner.wait();
    assert.equal(result.passed,false);assert.equal(result.errorKind,'protocol');assert.equal(JSON.stringify(result).includes('SYNTHETIC_SECRET'),false);
    assert.equal(runner.startWaiters.size,0);assert.equal(runner.closed,true);
  });
}

test('foreign resumed thread fails closed, without retrying or admitting another process',async t=>{
  const runner=fixture(t,'wrong-thread',{expectedThreadId:id});const result=await runner.wait();
  assert.equal(result.errorKind,'protocol');assert.equal(result.sameExpectedThread,false);assert.equal(result.passed,false);assert.equal(runner.threadId,null);
});

test('empty streams, missing terminals and nonzero exits cannot satisfy the runtime proof',async t=>{
  for(const scenario of ['empty','no-terminal','nonzero-exit']) {
    const runner=fixture(t,scenario);const result=await runner.wait();assert.equal(result.passed,false);
    assert.equal(result.errorKind,scenario==='nonzero-exit'?'process-exit':'terminal-missing');
  }
});

test('terminal failure before start resolves the observer without authorizing cancellation',async t=>{
  const runner=fixture(t,'failed-before-start');const observer=runner.waitForStart(3000);const observed=await observer;
  assert.deepEqual(observed,{started:false,terminal:{status:'failed',errorKind:'runtime-error'}});
  assert.equal((await runner.wait()).passed,false);assert.equal((await runner.cancel()).requested,false);assert.equal(runner.cancelRequested,false);
});

test('reported runtime errors are sanitized and never fall back to another invocation',async t=>{
  const runner=fixture(t,'error');const result=await runner.wait();
  assert.equal(result.errorKind,'runtime-error');assert.equal(result.passed,false);assert.equal(JSON.stringify(result).includes('SYNTHETIC_SECRET'),false);
  assert.equal(JSON.stringify(result).includes('private'),false);assert.ok(result.events.some(event=>event.type==='runtime.error'));
});

test('frame, response and event budgets reject oversized streams',async t=>{
  const frames=fixture(t,'oversized',{maxMessageBytes:128});assert.equal((await frames.wait()).errorKind,'protocol');
  const responses=fixture(t,'success',{maxResponseBytes:4});assert.equal((await responses.wait()).errorKind,'protocol');
  const events=fixture(t,'success',{maxEvents:2});const result=await events.wait();assert.equal(result.errorKind,'protocol');assert.ok(result.events.length<=2);
});

test('unobserved start times out and cleans pending observers without a cancellation claim',async t=>{
  const runner=fixture(t,'no-start');const pending=runner.waitForStart(150);await assert.rejects(pending,error=>error.kind==='timeout');
  const result=await runner.wait();assert.equal(result.passed,false);assert.equal(result.startObserved,false);assert.equal(result.errorKind,'timeout');
  assert.equal(runner.startWaiters.size,0);assert.equal(runner.cancelRequested,false);
});

test('plain process stop after observed start does not claim a confirmed tree or turn interruption',async t=>{
  const runner=fixture(t,'hang');assert.deepEqual(await runner.waitForStart(3000),{started:true});const cancelled=await runner.cancel({graceMs:500});
  assert.equal(cancelled.requested,true);assert.equal(cancelled.startObserved,true);assert.equal(cancelled.processTerminationConfirmed,false);assert.equal(cancelled.turnInterruptedConfirmed,false);
  assert.equal((await runner.wait()).passed,false);assert.equal(runner.cancelRequested,true);
});

test('owned Job cancellation requires its explicit final tree proof and remains distinct from turn interruption',async t=>{
  for(const confirmed of [true,false]) {
    const child=fakeJob({confirmed});const runner=new ExecJsonProcess({command:'synthetic.exe',cwd:directory,spawnProcess:()=>child});t.after(()=>runner.stop());
    const cancelled=await runner.cancel();assert.equal(cancelled.requested,true);assert.equal(cancelled.processTerminationConfirmed,confirmed);assert.equal(cancelled.turnInterruptedConfirmed,false);
    assert.equal(child.stops,1);assert.equal((await runner.wait()).passed,false);assert.equal(runner.startWaiters.size,0);
  }
});

test('turn deadlines begin after runtime ready, independently from bridge startup',async t=>{
  const child=fakeJob({readyDelay:80});const runner=new ExecJsonProcess({command:'synthetic.exe',cwd:directory,timeoutMs:1000,startTimeoutMs:20,spawnProcess:()=>child});t.after(()=>runner.stop());
  await delay(45);assert.equal(runner.failure,null);assert.equal(runner.deadline,undefined);assert.equal(runner.startDeadline,undefined);
  assert.deepEqual(await runner.waitForStart(1000),{started:true});assert.equal(runner.failure,null);await runner.cancel();
});

test('helper termination without runtime exit remains unknown and cannot leave wait hanging',async t=>{
  const child=fakeJob({start:false});child.stop=async()=>({parentExited:false,stdioClosed:false,treeTermination:'unknown',activeCount:null,helper:{parentExited:true,stdioClosed:true}});
  const runner=new ExecJsonProcess({command:'synthetic.exe',cwd:directory,timeoutMs:1000,startTimeoutMs:20,spawnProcess:()=>child});t.after(()=>runner.stop());
  const result=await runner.wait();assert.equal(result.passed,false);assert.equal(result.process.parentExited,false);assert.equal(result.process.stdioClosed,false);assert.equal(result.errorKind,'timeout');
});

test('start observer cap and invalid deadlines cannot create additional processes',async t=>{
  const child=fakeJob({readyDelay:80});const runner=new ExecJsonProcess({command:'synthetic.exe',cwd:directory,spawnProcess:()=>child});t.after(()=>runner.stop());
  const observers=Array.from({length:16},()=>runner.waitForStart(1000));assert.equal(runner.startWaiters.size,16);
  await assert.rejects(runner.waitForStart(1000),error=>error.kind==='contract');await assert.rejects(runner.waitForStart(0),error=>error.kind==='arguments');
  assert.deepEqual(await Promise.all(observers),Array.from({length:16},()=>({started:true})));assert.equal(runner.startWaiters.size,0);await runner.cancel();assert.equal(child.stops,1);
});

test('CLI flags pin subscription-only surface and disable extensions for initial and exact resume',()=>{
  for(const threadId of [null,id]) {
    const args=execArguments({cwd:directory,model:'gpt-5.3-codex',effort:'low',prompt:'Synthetic task',threadId,mcpDisableOverrides:['mcp_servers.example.enabled=false']});
    assert.ok(args.indexOf('-C')<args.indexOf('exec'));assert.equal(args[args.indexOf('-C')+1],directory);assert.equal(args.at(-1),'Synthetic task');
    for(const value of ['--json','--no-daemon','features.hooks=false','features.plugins=false','features.apps=false','features.multi_agent=false','mcp_servers.example.enabled=false','model_provider="openai"','approval_policy="never"','approvals_reviewer="user"','sandbox_mode="workspace-write"','sandbox_workspace_write.network_access=false','sandbox_workspace_write.exclude_tmpdir_env_var=true','sandbox_workspace_write.exclude_slash_tmp=true','web_search="disabled"','model="gpt-5.3-codex"','model_reasoning_effort="low"'])assert.ok(args.includes(value),value);
    assert.equal(args.includes('app-server'),false);assert.equal(args.includes('--listen'),false);
    for(const value of ['--last','--all','--fork','--dangerously-bypass-approvals-and-sandbox','--yolo','--ephemeral','--worktree'])assert.equal(args.includes(value),false,value);
    if(threadId)assert.equal(args[args.indexOf('resume')+1],id);else assert.equal(args.includes('resume'),false);
  }
});

test('CLI arguments reject unbound resume names, unsafe roots, invalid sandbox and MCP injection',()=>{
  const valid={cwd:directory,model:'gpt-5.3-codex',effort:'low',prompt:'Synthetic task'};
  for(const overrides of [{threadId:'--last'},{threadId:'friendly-name'},{cwd:'relative'},{sandbox:'danger-full-access'},{prompt:'x'.repeat(8193)},{model:'model\nsecret'},{mcpDisableOverrides:['notify=["bad"]']},{mcpDisableOverrides:['mcp_servers.x.enabled=true']}])assert.throws(()=>execArguments({...valid,...overrides}),error=>error.kind==='arguments');
  const readOnly=execArguments({...valid,sandbox:'read-only',threadId:id});assert.ok(readOnly.includes('sandbox_mode="read-only"'));
});

test('probe rejects implicit opt-in and excessive budgets before touching a fixture or provider',async()=>{
  for(const options of [{live:'true'},{windowsJob:'true'},{maxInvocations:0},{maxInvocations:3},{maxInvocations:1.5},{requestTimeoutMs:0},{turnTimeoutMs:-1}]) {
    const report=await runExecProbe(options);assert.equal(report.status,'failed');assert.equal(report.error.kind,'arguments');assert.equal(report.liveStarted,false);assert.equal(report.invocations,0);assert.equal(report.workspace,undefined);assert.deepEqual(report.processes,[]);
  }
  if(process.platform==='win32') {
    const report=await runExecProbe({live:true,windowsJob:false});assert.equal(report.error.kind,'supervision');assert.equal(report.liveStarted,false);assert.equal(report.workspace,undefined);
  }
});
