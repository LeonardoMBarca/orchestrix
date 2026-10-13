import test from 'node:test';
import assert from 'node:assert/strict';
import {EventEmitter} from 'node:events';
import {CodexSession} from '../src/codex-session.mjs';
import {HarnessError} from '../src/json-rpc.mjs';
import {CodexRuntimeAdapter} from '../src/codex-contract.mjs';

const model=(id,extra={})=>({id,model:id,isDefault:false,defaultReasoningEffort:'low',supportedReasoningEfforts:[{reasoningEffort:'low',description:'Native advertised level'}],...extra});
function fixture({pages=[{data:[model('announced')],nextCursor:null}],account={account:{type:'chatgpt',email:'synthetic-account@example.invalid',accountId:'SYNTHETIC_PRIVATE_ID',planType:'pro'},requiresOpenaiAuth:true},rate={ordinaryUsageAllowed:true},handler}={}) {
  const client=new EventEmitter();client.calls=[];client.closed=false;client.failure=null;let page=0;
  client.request=async(method,params,timeout)=>{
    client.calls.push({method,params,timeout});
    if(handler)return handler(method,params,timeout);
    const value=method==='account/read'?account:method==='account/rateLimits/read'?rate:pages[page++];
    if(value instanceof Error)throw value;
    return value;
  };
  return {client,session:new CodexSession(client)};
}
const listCalls=client=>client.calls.filter(call=>call.method==='model/list');

test('discovery projects missing, partial, unsupported and unmapped reasoning honestly into the portable descriptor',async()=>{
  const {session}=fixture({pages:[{data:[
    {id:'absent',model:'absent'},
    model('partial',{supportedReasoningEfforts:[{reasoningEffort:'low'},null]}),
    model('unsupported',{defaultReasoningEffort:null,supportedReasoningEfforts:[]}),
    model('known'),
    model('unmapped',{supportedReasoningEfforts:[{reasoningEffort:'low'},{reasoningEffort:'xhigh'}]}),
  ],nextCursor:null}]});
  const discovery=await session.discover();
  const adapter=new CodexRuntimeAdapter({session,discovery,runtimeVersion:'synthetic-fixture',evidenceSource:'synthetic'});
  try {
    const rows=adapter.descriptor.models;
    assert.deepEqual(rows.map(row=>row.capabilities.reasoning.support),['unknown','unknown','unsupported','supported','supported']);
    assert.deepEqual(rows.map(row=>row.reasoningLevels),[null,null,[],['low'],null]);
    assert.ok(rows.every(row=>row.availability==='unknown'&&row.capabilities.reasoning.availability==='unknown'));
    assert.equal(adapter.descriptor.catalog.completeness,'complete');
    assert.equal(JSON.stringify(adapter.descriptor).includes('xhigh'),false);
  }finally{adapter.dispose();}
});

test('bounded pagination preserves each model native efforts and only explicit metadata',async()=>{
  const {session,client}=fixture({pages:[
    {data:[model('first',{isDefault:true,supportedReasoningEfforts:['low',{reasoningEffort:'native-future-level',description:'A future value'}],serviceTiers:[{id:'fast',name:'Fast',description:'Reported tier'}],defaultServiceTier:'fast',multiAgentVersion:'v2',inputModalities:['text','audio'],untrusted:'SYNTHETIC_SECRET'})],nextCursor:'opaque-page-two'},
    {data:[model('second',{defaultReasoningEffort:'medium',supportedReasoningEfforts:[{reasoningEffort:'medium'}]})],nextCursor:null},
  ]});
  const result=await session.discover();
  assert.deepEqual(result.models.map(item=>item.model),['first','second']);
  assert.deepEqual(result.models[0].efforts,['low','native-future-level']);
  assert.deepEqual(result.models[1].efforts,['medium']);
  assert.equal(result.models[0].isDefault,true);assert.equal(result.models[1].defaultEffort,'medium');
  assert.deepEqual(result.models[0].reasoning,{support:'supported',completeness:'complete'});
  assert.deepEqual(result.models[0].serviceTiers,[{id:'fast',name:'Fast',description:'Reported tier'}]);
  assert.equal(result.models[0].defaultServiceTier,'fast');assert.equal(result.models[0].multiAgentVersion,'v2');
  assert.deepEqual(result.models[0].inputModalities,['text','audio']);
  assert.deepEqual(result.catalog,{source:'model/list',scope:'visible-models',includeHidden:false,completeness:'complete',pages:2,modelCount:2,reason:'complete'});
  assert.deepEqual(listCalls(client).map(call=>call.params),[{includeHidden:false,limit:100},{includeHidden:false,limit:100,cursor:'opaque-page-two'}]);
  assert.ok(listCalls(client).every(call=>call.timeout>0&&call.timeout<=5000));
  assert.equal(client.calls.some(call=>/thread|turn|experimentalFeature/.test(call.method)),false);
  assert.equal(result.models.every(item=>item.availability==='unknown'),true);
  assert.deepEqual(result.connection,{authentication:'authenticated',billing:'subscription',availability:'available',bindingObserved:true});
  assert.ok(session.accountBinding);assert.equal(Object.hasOwn(result,'accountBinding'),false);
  for(const secret of ['SYNTHETIC_PRIVATE_ID','synthetic-account@example.invalid','SYNTHETIC_SECRET','opaque-page-two'])assert.equal(JSON.stringify({result,events:session.events}).includes(secret),false);
});

test('explicit terminal empty catalog is complete without implying an unavailable connection',async()=>{
  const {session}=fixture({pages:[{data:[],nextCursor:null}]});const result=await session.discover();
  assert.equal(result.catalog.completeness,'complete');assert.equal(result.catalog.modelCount,0);
  assert.deepEqual(result.models,[]);assert.equal(result.availability,true);
});

test('missing cursor leaves single-page coverage unknown and does not invent schema defaults',async()=>{
  const {session,client}=fixture({pages:[{data:[{id:'minimal',model:'minimal'}]}]});const result=await session.discover();
  assert.equal(result.catalog.completeness,'unknown');assert.equal(result.catalog.reason,'cursor-missing');
  const entry=result.models[0];assert.deepEqual(entry.efforts,[]);assert.equal(entry.defaultEffort,null);
  assert.deepEqual(entry.reasoning,{support:'unknown',completeness:'unknown'});
  for(const field of ['serviceTiers','defaultServiceTier','multiAgentVersion','inputModalities'])assert.equal(entry[field],null);
  assert.equal(entry.availability,'unknown');assert.equal(listCalls(client).length,1);
});

test('explicit empty reasoning support is distinct from absent or malformed metadata',async()=>{
  const {session}=fixture({pages:[{data:[
    model('none',{defaultReasoningEffort:null,supportedReasoningEfforts:[],serviceTiers:[],inputModalities:[]}),
    model('malformed',{supportedReasoningEfforts:[null,{reasoningEffort:'low'},9]}),
    model('contradictory',{defaultReasoningEffort:'unlisted'}),
  ],nextCursor:null}]});const {models}=await session.discover();
  assert.deepEqual(models[0].reasoning,{support:'unsupported',completeness:'complete'});assert.deepEqual(models[0].serviceTiers,[]);
  assert.deepEqual(models[1].efforts,['low']);assert.deepEqual(models[1].reasoning,{support:'unknown',completeness:'partial'});
  assert.deepEqual(models[2].reasoning,{support:'unknown',completeness:'partial'});
});

test('reasoning metadata is bounded without treating a truncated list as complete',async()=>{
  const levels=Array.from({length:65},(_,index)=>`native-${index}`);
  const {session}=fixture({pages:[{data:[model('bounded',{defaultReasoningEffort:levels[0],supportedReasoningEfforts:levels,serviceTiers:Array.from({length:33},(_,index)=>({id:String(index),name:'Tier',description:''})),inputModalities:Array(17).fill('text')})],nextCursor:null}]});
  const result=await session.discover();assert.equal(result.models[0].efforts.length,64);
  assert.equal(result.models[0].reasoning.support,'unknown');assert.equal(result.models[0].reasoning.completeness,'partial');
  assert.equal(result.models[0].serviceTiers,null);assert.equal(result.models[0].inputModalities,null);
});

test('authentication gate prevents model/rate reads and does not return an earlier identity binding',async()=>{
  const {session,client}=fixture({account:{account:null,requiresOpenaiAuth:true}});session.accountBinding='OLD_BINDING';
  const result=await session.discover();assert.equal(session.accountBinding,null);
  assert.deepEqual(client.calls.map(call=>call.method),['account/read']);assert.equal(result.auth.subscriptionGate,false);
  assert.deepEqual(result.models,[]);assert.equal(result.availability,null);
  assert.equal(result.connection.authentication,'unauthenticated');assert.equal(result.connection.bindingObserved,false);
  assert.equal(result.catalog.completeness,'unknown');assert.equal(result.catalog.reason,'blocked-auth');
});

test('API authentication never crosses the existing subscription-only discovery gate',async()=>{
  const {session,client}=fixture({account:{account:{type:'apiKey'},requiresOpenaiAuth:true}});
  const result=await session.discover();assert.equal(result.auth.subscriptionGate,false);assert.equal(result.connection.billing,'api');
  assert.equal(listCalls(client).length,0);assert.equal(result.availability,null);
});

test('account availability and model entitlement remain separate and read failure stays unknown',async()=>{
  for(const rate of [{ordinaryUsageAllowed:false},{ordinaryUsageAllowed:'true'},new Error('SYNTHETIC_PRIVATE_RATE_ERROR')]) {
    const {session}=fixture({rate});const result=await session.discover();
    assert.equal(result.models[0].availability,'unknown');
    assert.equal(result.availability,rate.ordinaryUsageAllowed===false?false:null);
    assert.equal(result.connection.availability,rate.ordinaryUsageAllowed===false?'unavailable':'unknown');
    assert.equal(JSON.stringify(result).includes('SYNTHETIC_PRIVATE_RATE_ERROR'),false);
  }
});

test('first catalog read failure is rejected while a later page retains explicitly partial observations',async()=>{
  const error=new HarnessError('rpc','SYNTHETIC_PRIVATE_PAGE_ERROR');
  const first=fixture({pages:[error]});await assert.rejects(first.session.discover(),value=>value===error);
  assert.equal(first.client.calls.some(call=>call.method==='account/rateLimits/read'),false);
  const later=fixture({pages:[{data:[model('observed')],nextCursor:'second'},error]});
  const result=await later.session.discover();assert.equal(result.catalog.completeness,'partial');assert.equal(result.catalog.reason,'read-failed');
  assert.deepEqual(result.models.map(value=>value.id),['observed']);assert.equal(result.availability,true);
  assert.equal(JSON.stringify(result).includes('SYNTHETIC_PRIVATE_PAGE_ERROR'),false);
});

test('invalid page shape fails closed initially or marks a later catalog partial',async()=>{
  for(const invalid of [null,[],{}, {data:{}},{data:'invalid'}]) {
    const first=fixture({pages:[invalid]});await assert.rejects(first.session.discover(),error=>error.kind==='protocol');
    const later=fixture({pages:[{data:[model('kept')],nextCursor:'next'},invalid]});
    const result=await later.session.discover();assert.equal(result.catalog.completeness,'partial');assert.equal(result.models.length,1);
  }
});

test('repeated opaque cursor terminates pagination without leaking it or duplicating identical models',async()=>{
  const item=model('same');const {session,client}=fixture({pages:[{data:[item],nextCursor:''},{data:[item],nextCursor:''}]});
  const result=await session.discover();assert.equal(listCalls(client).length,2);assert.equal(listCalls(client)[1].params.cursor,'');
  assert.equal(result.catalog.completeness,'partial');assert.equal(result.catalog.reason,'cursor-cycle');assert.equal(result.models.length,1);
});

test('page/model budgets produce explicit partial results and stop further RPCs',async()=>{
  const pages=fixture({pages:[{data:[model('first')],nextCursor:'two'},{data:[model('second')],nextCursor:'three'}]});
  const result=await pages.session.discover({maxPages:2});assert.equal(listCalls(pages.client).length,2);
  assert.equal(result.catalog.reason,'page-limit');assert.equal(result.catalog.completeness,'partial');assert.equal(result.models.length,2);
  const models=fixture({pages:[{data:[model('one'),model('two'),model('three')],nextCursor:'more'}]});
  const limited=await models.session.discover({maxModels:2});assert.equal(limited.models.length,2);assert.equal(limited.catalog.reason,'model-limit');
  assert.equal(limited.catalog.completeness,'partial');assert.equal(listCalls(models.client).length,1);
});

test('server exceeding requested page size cannot bypass the row budget',async()=>{
  const {session,client}=fixture({pages:[{data:[model('one'),model('two')],nextCursor:null}]});
  const result=await session.discover({pageSize:1});assert.equal(result.models.length,1);assert.equal(result.catalog.completeness,'partial');
  assert.equal(result.catalog.reason,'page-size-exceeded');assert.equal(listCalls(client).length,1);
});

test('malformed rows cannot make incomplete model coverage appear complete',async()=>{
  const {session}=fixture({pages:[{data:[null,{id:'missing-model'},model('valid'),model('bad-id',{id:'\nunsafe'})],nextCursor:null}]});
  const result=await session.discover();assert.deepEqual(result.models.map(value=>value.id),['valid']);
  assert.equal(result.catalog.completeness,'partial');assert.equal(result.catalog.reason,'invalid-model');
});

test('conflicting duplicate models are removed rather than granting arbitrarily selected capabilities',async()=>{
  const {session}=fixture({pages:[{data:[model('ambiguous'),model('independent')],nextCursor:'second'},
    {data:[model('ambiguous',{defaultReasoningEffort:'high',supportedReasoningEfforts:['high']})],nextCursor:null}]});
  const result=await session.discover();assert.deepEqual(result.models.map(value=>value.id),['independent']);
  assert.equal(result.catalog.reason,'conflicting-model');assert.equal(result.catalog.completeness,'partial');
});

test('invalid cursor is never sent as a next request',async()=>{
  for(const nextCursor of [7,{},'\nunsafe','x'.repeat(4097)]) {
    const {session,client}=fixture({pages:[{data:[model('safe')],nextCursor}]});const result=await session.discover();
    assert.equal(result.catalog.completeness,'partial');assert.equal(result.catalog.reason,'invalid-cursor');assert.equal(listCalls(client).length,1);
  }
});

test('discovery bounds are validated before authentication or any other RPC',async()=>{
  for(const bounds of [null,[],{maxPages:0},{maxPages:11},{maxModels:257},{pageSize:101},{timeoutMs:15001},{timeoutMs:NaN},{maxPages:1.5},{unexpected:true}]) {
    const {session,client}=fixture();await assert.rejects(session.discover(bounds),error=>error.kind==='arguments');assert.equal(client.calls.length,0);
  }
});

test('failed account recheck clears the previous in-memory binding',async()=>{
  const failure=new HarnessError('rpc','Synthetic account unavailable');const {session,client}=fixture({account:failure});session.accountBinding='OLD_BINDING';
  await assert.rejects(session.discover(),error=>error===failure);assert.equal(session.accountBinding,null);assert.equal(client.calls.length,1);
});

test('the monotonic catalog deadline stops another page without extending its request budget',async t=>{
  const ticks=[1000,1000,1001,1500];t.mock.method(performance,'now',()=>ticks.shift()??1500);
  const {session,client}=fixture({pages:[{data:[model('first')],nextCursor:'unread-page'}]});
  const result=await session.discover({timeoutMs:500});
  assert.equal(listCalls(client).length,1);assert.equal(listCalls(client)[0].timeout,500);
  assert.equal(result.catalog.completeness,'partial');assert.equal(result.catalog.reason,'deadline');assert.equal(result.models.length,1);
});

test('late or unstarted catalog reads cannot claim complete coverage',async t=>{
  const ticks=[1000,1000,1501,2000,2001];t.mock.method(performance,'now',()=>ticks.shift()??2001);
  const late=fixture();const result=await late.session.discover({timeoutMs:500});
  assert.equal(listCalls(late.client).length,1);assert.equal(result.models.length,0);
  assert.equal(result.catalog.completeness,'unknown');assert.equal(result.catalog.reason,'deadline');
  const unstarted=fixture();const unknown=await unstarted.session.discover({timeoutMs:1});
  assert.equal(listCalls(unstarted.client).length,0);assert.equal(unknown.catalog.completeness,'unknown');
  assert.equal(unknown.catalog.reason,'deadline');
});

test('a malformed default flag cannot select a default model by truthiness',async()=>{
  const {session}=fixture({pages:[{data:[model('not-default',{isDefault:'false'})],nextCursor:null}]});
  assert.equal((await session.discover()).models[0].isDefault,false);
});
