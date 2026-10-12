import test from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {resolve} from 'node:path';
import {CodexSession} from '../src/codex-session.mjs';
import {HarnessError} from '../src/json-rpc.mjs';

const cwd=resolve('synthetic-disposable-worktree');
const options={cwd,model:'synthetic-announced-model',effort:'low'};
const kind=expected=>error=>error instanceof HarnessError&&error.kind===expected;
function clientFor(handler) {
  const client=new EventEmitter();client.calls=[];client.failure=null;client.closed=false;
  client.request=async(method,params)=>{client.calls.push({method,params});return handler(method,params,client);};
  client.fail=error=>{client.failure=error;client.emit('failure',error);};
  return client;
}
function descriptor({threadId='persisted-owned-thread',sandbox='workspace-write',ephemeral=false}={}) {
  return {
    thread:{id:threadId,sessionId:threadId,cwd,modelProvider:'openai',ephemeral,status:{type:'idle'},turns:[]},
    cwd,model:options.model,reasoningEffort:'low',modelProvider:'openai',approvalPolicy:'never',approvalsReviewer:'user',
    sandbox:sandbox==='read-only'?{type:'readOnly',networkAccess:false}:{type:'workspaceWrite',writableRoots:[],networkAccess:false,excludeTmpdirEnvVar:true,excludeSlashTmp:true},
  };
}

test('default thread creation keeps the ephemeral read-only probe and reports complete pins',async()=>{
  const client=clientFor(()=>descriptor({sandbox:'read-only',ephemeral:true}));
  const session=new CodexSession(client);const configuration=await session.startThread(options);
  assert.equal(client.calls[0].method,'thread/start');assert.equal(client.calls[0].params.sandbox,'read-only');
  assert.equal(client.calls[0].params.ephemeral,true);assert.equal(configuration.ephemeral,true);
  assert.equal(configuration.requested.cwd,cwd);assert.equal(configuration.requested.reviewer,'user');
  assert.equal(configuration.resolvedConfiguration.model,options.model);
  assert.equal(configuration.resolvedConfiguration.effort,'low');
  assert.deepEqual(session.sandboxPolicy,{type:'readOnly',networkAccess:false});
});

test('persistent creation is opt-in and a workspace profile never enables network or global temp writes',async()=>{
  const client=clientFor(()=>descriptor());const session=new CodexSession(client);
  const configuration=await session.startThread({...options,sandbox:'workspace-write',ephemeral:false});
  assert.equal(client.calls[0].method,'thread/start');assert.equal(client.calls[0].params.ephemeral,false);
  assert.deepEqual(client.calls[0].params.config.sandbox_workspace_write,{network_access:false,writable_roots:[],exclude_tmpdir_env_var:true,exclude_slash_tmp:true});
  assert.equal(configuration.ephemeral,false);assert.equal(configuration.resolvedConfiguration.ephemeral,false);
  assert.equal(configuration.resolvedConfiguration.cwd,cwd);assert.equal(configuration.resolvedConfiguration.sandbox,'workspaceWrite');
});

test('resume uses the exact recorded thread with pinned configuration, never fork or replacement creation',async()=>{
  const client=clientFor(()=>descriptor());const session=new CodexSession(client);
  const configuration=await session.resumeThread({...options,threadId:'persisted-owned-thread'});
  assert.equal(client.calls.length,1);assert.equal(client.calls[0].method,'thread/resume');
  assert.equal(client.calls[0].params.threadId,'persisted-owned-thread');
  assert.equal(client.calls[0].params.cwd,cwd);assert.equal(client.calls[0].params.model,options.model);
  assert.equal(client.calls[0].params.modelProvider,'openai');assert.equal(client.calls[0].params.approvalPolicy,'never');
  assert.equal(client.calls[0].params.approvalsReviewer,'user');assert.equal(client.calls[0].params.config.model_reasoning_effort,'low');
  assert.equal(Object.hasOwn(client.calls[0].params,'ephemeral'),false);
  assert.equal(configuration.threadId,'persisted-owned-thread');assert.equal(configuration.requested.ephemeral,false);
  assert.equal(session.events.filter(event=>event.type==='session.resumed').length,1);
});

test('startTurn uses only the confirmed workspace policy and rejects changed effort',async()=>{
  const resolved=descriptor();resolved.sandbox.writableRoots=[cwd];
  const client=clientFor(method=>method==='turn/start'?{turn:{id:'new-owned-turn'}}:resolved);
  const session=new CodexSession(client);const configuration=await session.resumeThread({...options,threadId:'persisted-owned-thread'});
  configuration.resolvedConfiguration.sandboxPolicy.networkAccess=true;
  configuration.resolvedConfiguration.writableRoots.push(resolve('outside'));
  configuration.resolvedConfiguration.effort='high';configuration.requested.effort='high';
  await assert.rejects(session.startTurn('Synthetic bounded edit',{effort:'high'}),kind('policy'));
  assert.equal(client.calls.length,1);
  assert.equal(await session.startTurn('Synthetic bounded edit'),'new-owned-turn');
  assert.deepEqual(client.calls[1].params.sandboxPolicy,{type:'workspaceWrite',writableRoots:[cwd],networkAccess:false,excludeTmpdirEnvVar:true,excludeSlashTmp:true});
  assert.equal(client.calls[1].params.effort,'low');assert.equal(client.calls[1].params.threadId,'persisted-owned-thread');
});

test('resume rejects mismatched identity, ownership, configuration and execution boundaries before any turn',async()=>{
  const mutations=[
    ['thread identity',value=>{value.thread.id='different-thread';}],
    ['session tree identity',value=>{value.thread.sessionId='different-session';}],
    ['forked record',value=>{value.thread.forkedFromId='parent-thread';}],
    ['model',value=>{value.model='different-model';}],
    ['effort',value=>{value.reasoningEffort='high';}],
    ['provider',value=>{value.modelProvider='paid-api';}],
    ['thread provider',value=>{value.thread.modelProvider='paid-api';}],
    ['cwd',value=>{value.cwd=resolve('different-worktree');}],
    ['thread cwd',value=>{value.thread.cwd=resolve('different-worktree');}],
    ['approval',value=>{value.approvalPolicy='on-request';}],
    ['reviewer',value=>{value.approvalsReviewer='autoReview';}],
    ['network',value=>{value.sandbox.networkAccess=true;}],
    ['unknown network',value=>{delete value.sandbox.networkAccess;}],
    ['sandbox',value=>{value.sandbox.type='dangerFullAccess';}],
    ['writable root outside',value=>{value.sandbox.writableRoots=[resolve('outside-worktree')];}],
    ['writable root ancestor',value=>{value.sandbox.writableRoots=[resolve(cwd,'..')];}],
    ['relative writable root',value=>{value.sandbox.writableRoots=['.'];}],
    ['unknown writable roots',value=>{delete value.sandbox.writableRoots;}],
    ['global tmp writable',value=>{value.sandbox.excludeTmpdirEnvVar=false;}],
    ['slash tmp writable',value=>{value.sandbox.excludeSlashTmp=false;}],
    ['ephemeral record',value=>{value.thread.ephemeral=true;}],
    ['unknown ephemeral',value=>{delete value.thread.ephemeral;}],
    ['active thread',value=>{value.thread.status={type:'active',activeFlags:[]};}],
    ['unknown status',value=>{delete value.thread.status;}],
    ['contradictory active history',value=>{value.thread.turns=[{id:'old-turn',status:'inProgress'}];}],
  ];
  for(const [label,mutate] of mutations) {
    const value=descriptor();mutate(value);const client=clientFor(()=>value);const session=new CodexSession(client);
    await assert.rejects(session.resumeThread({...options,threadId:'persisted-owned-thread'}),kind('policy'),label);
    assert.equal(session.threadId,null,label);assert.equal(session.threadAdmission,'uncertain',label);
    await assert.rejects(session.startTurn('Must not execute'),kind('contract'),label);
    await assert.rejects(session.resumeThread({...options,threadId:'persisted-owned-thread'}),kind('contract'),label);
    assert.equal(client.calls.length,1,label);assert.equal(session.events.some(event=>event.type==='session.resumed'),false,label);
  }
});

test('read-only resume can be selected explicitly but still requires persisted idle identity',async()=>{
  const client=clientFor(()=>descriptor({sandbox:'read-only'}));const session=new CodexSession(client);
  await session.resumeThread({...options,threadId:'persisted-owned-thread',sandbox:'read-only'});
  assert.equal(client.calls[0].params.sandbox,'read-only');assert.deepEqual(session.sandboxPolicy,{type:'readOnly',networkAccess:false});
});

test('pending admission, running turns and uncertain ownership prohibit resume without another request',async()=>{
  let acknowledge;
  const client=clientFor(()=>new Promise(resolveAck=>{acknowledge=resolveAck;}));const session=new CodexSession(client);
  const pending=session.startThread({...options,sandbox:'workspace-write',ephemeral:false});
  await assert.rejects(session.resumeThread({...options,threadId:'persisted-owned-thread'}),kind('contract'));
  assert.equal(client.calls.length,1);acknowledge(descriptor());await pending;
  await assert.rejects(session.resumeThread({...options,threadId:'persisted-owned-thread'}),kind('contract'));
  for(const state of [{turnAdmission:'active'},{turnAdmission:'uncertain'},{startPending:true},{activeTurnId:'pending-turn'}]) {
    const isolated=new CodexSession(client);Object.assign(isolated,state);
    await assert.rejects(isolated.resumeThread({...options,threadId:'persisted-owned-thread'}),kind('contract'));
  }
  assert.equal(client.calls.length,1);
});

test('lost thread ACK remains uncertain and cannot be retried through start or resume',async()=>{
  const client=clientFor(()=>{throw new HarnessError('timeout','Synthetic lost admission ACK');});const session=new CodexSession(client);
  await assert.rejects(session.resumeThread({...options,threadId:'persisted-owned-thread'}),kind('timeout'));
  assert.equal(session.threadAdmission,'uncertain');assert.equal(session.threadPending,false);
  await assert.rejects(session.startThread(options),kind('contract'));
  await assert.rejects(session.resumeThread({...options,threadId:'persisted-owned-thread'}),kind('contract'));
  assert.equal(client.calls.length,1);
});

test('a definite rejection permits an explicit new attempt unless a started notification contradicts it',async()=>{
  for(const contradictory of [false,true]) {
    let reject=true;
    const client=clientFor((method,params,current)=>{
      if(reject){if(contradictory)current.emit('notification',{method:'thread/started',params:{thread:{id:'unexpected-thread'}}});throw new HarnessError('rpc','Synthetic definite rejection');}
      return descriptor();
    });
    const session=new CodexSession(client);
    await assert.rejects(session.resumeThread({...options,threadId:'persisted-owned-thread'}),kind('rpc'));
    reject=false;
    if(contradictory){assert.equal(session.threadAdmission,'uncertain');await assert.rejects(session.resumeThread({...options,threadId:'persisted-owned-thread'}),kind('contract'));assert.equal(client.calls.length,1);}
    else {assert.equal(session.threadAdmission,'idle');await session.resumeThread({...options,threadId:'persisted-owned-thread'});assert.equal(client.calls.length,2);}
  }
});

test('invalid local resume requests never reach the runtime',async()=>{
  for(const invalid of [{threadId:''},{threadId:7},{cwd:'relative/path'},{model:''},{effort:''},{sandbox:'danger-full-access'}]) {
    const client=clientFor(()=>{throw Error('Should never dispatch');});const session=new CodexSession(client);
    await assert.rejects(session.resumeThread({...options,threadId:'persisted-owned-thread',...invalid}),kind('arguments'));
    assert.equal(client.calls.length,0);assert.equal(session.threadAdmission,'idle');
  }
  const client=clientFor(()=>{throw Error('Should never dispatch');});const session=new CodexSession(client);
  await assert.rejects(session.establishThread('thread/fork',{...options,threadId:'persisted-owned-thread',sandbox:'workspace-write',ephemeral:false}),kind('arguments'));
  assert.equal(client.calls.length,0);
});

test('discovery binds observable account identity in memory without exposing metadata or refreshing tokens',async()=>{
  let account={type:'chatgpt',email:'synthetic-account@example.invalid',planType:'pro',accessToken:'SYNTHETIC_SECRET'};
  const client=clientFor(method=>method==='account/read'?{account,requiresOpenaiAuth:true}:method==='model/list'?{data:[]}:{ordinaryUsageAllowed:true});
  const session=new CodexSession(client);const discovery=await session.discover();const binding=session.accountBinding;
  assert.match(binding,/^[a-f0-9]{64}$/);assert.equal(discovery.auth.subscriptionGate,true);
  assert.equal(JSON.stringify({discovery,events:session.events}).includes(binding),false);
  assert.equal(JSON.stringify({discovery,events:session.events}).includes(account.email),false);
  assert.equal(JSON.stringify({discovery,events:session.events}).includes('SYNTHETIC_SECRET'),false);
  account={...account,accessToken:'DIFFERENT_SYNTHETIC_SECRET'};await session.discover();assert.equal(session.accountBinding,binding);
  account={...account,email:'different-account@example.invalid'};await session.discover();assert.notEqual(session.accountBinding,binding);
  account={type:'chatgpt',email:null,planType:'pro'};await session.discover();assert.equal(session.accountBinding,null);
  account={type:'apiKey',email:'synthetic-account@example.invalid'};const apiDiscovery=await session.discover();assert.equal(session.accountBinding,null);assert.equal(apiDiscovery.auth.subscriptionGate,false);
  account=null;await session.discover();assert.equal(session.accountBinding,null);
  assert.ok(client.calls.filter(call=>call.method==='account/read').every(call=>call.params.refreshToken===false));
});
