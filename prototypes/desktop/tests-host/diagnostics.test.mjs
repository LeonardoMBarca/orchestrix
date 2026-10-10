import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';

const source=readFileSync(new URL('../connection-diagnostics.js',import.meta.url),'utf8');
const sandbox={module:{exports:{}},URL,Date};runInNewContext(source,sandbox);
const D=sandbox.module.exports;
const plain=value=>JSON.parse(JSON.stringify(value));
const at='2026-10-10T15:00:00.000Z';
const fixed={startedAt:at,completedAt:at,runId:'diagnostic-run'};
const api=(provider='openai',patch={})=>({id:'connection-a',mode:'api',apiProvider:provider,provider,endpoint:provider==='openai'?'https://api.openai.com/v1':provider==='anthropic'?'https://api.anthropic.com/v1':provider==='gemini'?'https://generativelanguage.googleapis.com/v1beta':provider==='azure'?'https://resource.openai.azure.com':'https://custom.example/v1',identity:{revision:1},authMethod:'api-key',...patch});
const runtime=(provider='Codex',patch={})=>({id:'runtime-a',mode:'own-plan',provider,identity:{revision:2},runtimeVersion:'0.162.0-alpha.2',...patch});
const scope=c=>plain(D.plan(c).scope);
function catalog(c,models=[],patch={}){return {schemaVersion:1,scope:scope(c),source:{kind:c.mode==='api'?'provider-metadata':'runtime-observation',operation:c.mode==='api'?'models.list':'model/list'},observedAt:at,status:'complete',models,authentication:'unknown',entitlement:'unknown',usage:'unknown',credits:'unknown',...patch};}
const cap=(status,extra={})=>({status,observed:status!=='unknown',evidence:[],...extra});
const model=(id='model-a',capabilities={},patch={})=>({id,name:id,capabilities,...patch});
const binding=(c,patch={})=>({identityBound:true,scope:scope(c),source:{kind:'runtime-observation',operation:'config/read'},observedAt:at,...patch});
const report=(c,cat,patch={})=>D.buildReport(c,cat,{...fixed,...patch});
const check=(r,id)=>r.checks.find(item=>item.id===id);

test('pure UMD works in browser and Node without network, storage or runtime execution',()=>{
  const browser={URL,Date};runInNewContext(source,browser);
  assert.equal(typeof browser.OrchestrixConnectionDiagnostics.buildReport,'function');
  assert.deepEqual(Object.keys(D).sort(),['buildReport','isCurrent','plan','summarize']);
  const plan=D.plan(api());assert.ok(plan.steps.every(item=>item.network===false));
  assert.equal(plan.policy.nativeDelegation.effective,'disabled');assert.equal(plan.policy.nativeDelegation.applyStatus,'not-applied');
  assert.equal(plan.policy.orchestrixMultiAgent.status,'planned');assert.equal(plan.policy.orchestrixMultiAgent.nativeDelegationRequired,false);
});

test('provider-specific plans distinguish Azure, Bedrock, Google API and Antigravity runtime',()=>{
  const hints=c=>D.plan(c).steps.find(step=>step.id==='providerSpecific').providerSpecific;
  assert.ok(hints(api('azure',{deployment:'app'})).includes('management-deployments-separate'));
  assert.ok(hints(api('bedrock',{region:'us-east-1'})).includes('inference-profiles'));
  assert.ok(hints(api('gemini')).includes('not-antigravity-runtime'));
  assert.ok(hints(runtime('Antigravity')).includes('not-gemini-developer-api'));
  assert.ok(hints(runtime('Codex')).includes('reasoning-tiers-native-multi-independent'));
  assert.ok(hints(runtime('Claude Code')).includes('subscription-not-api'));
});

test('configuration success and catalog HTTP success never imply authentication, quotas or execution access',()=>{
  const c=api();const r=report(c,catalog(c,[model()]));
  assert.equal(check(r,'config').status,'passed');assert.equal(check(r,'catalog').status,'passed');
  assert.equal(check(r,'identity').status,'unknown');assert.equal(check(r,'access').status,'unknown');
  assert.equal(r.models[0].available,'unknown');assert.equal(r.features.usage.support,'unknown');assert.equal(r.features.credits.support,'unknown');
  assert.equal(r.features.reasoning.support,'unknown');assert.equal(r.features.functionCalling.support,'unknown');
  assert.equal(r.status,'partial');assert.equal(r.policy.hostPermissions.effective,'requires-user-grant');
});

test('missing metadata is unknown; explicit unsupported observations pass the diagnostic without fabrication',()=>{
  const c=api('anthropic');const r=report(c,catalog(c,[model('one',{reasoning:cap('unsupported'),nativeWeb:cap('supported'),context:cap('supported',{inputTokens:200000,outputTokens:8000})})]));
  assert.equal(r.features.reasoning.support,'unsupported');assert.equal(r.features.reasoning.available,'unavailable');
  assert.equal(check(r,'reasoning').status,'passed');assert.equal(check(r,'reasoning').code,'observed-unsupported');
  assert.equal(r.features.nativeWeb.support,'supported');assert.equal(r.features.nativeWeb.available,'unknown');
  assert.equal(r.models[0].features.context.details.inputTokens,200000);assert.equal(r.models[0].features.context.details.windowTokens,undefined);
  assert.equal(r.features.functionCalling.support,'unknown');
});

test('aggregates are explicit about some models and preserve the per-model matrix',()=>{
  const c=api('anthropic');const r=report(c,catalog(c,[model('one',{reasoning:cap('supported',{levels:['low','high']})}),model('two',{reasoning:cap('unsupported')}),model('three')]));
  assert.equal(r.features.reasoning.support,'supported');assert.deepEqual(plain(r.features.reasoning.counts),{supported:1,unsupported:1,unknown:1,'not-applicable':0});
  assert.equal(r.features.reasoning.details.coverage,'some-resources');assert.deepEqual(plain(r.features.reasoning.details.levels),['low','high']);
  assert.equal(r.models[2].features.reasoning.support,'unknown');assert.equal(r.summary.modelCount,3);
});

test('account, endpoint, deployment, region and revision scope mismatches discard observations',()=>{
  const c=api();const base=catalog(c,[model('foreign',{reasoning:cap('supported')})]);
  for(const patch of [{connectionId:'connection-b'},{provider:'anthropic'},{endpoint:'https://another.example/v1'},{revision:0},{region:'us-east-1'},{deployment:'other'}]){
    const r=report(c,{...base,scope:{...base.scope,...patch}});
    assert.equal(r.models.length,0);assert.equal(r.features.reasoning.support,'unknown');assert.equal(check(r,'catalog').status,'blocked');
  }
  const r=report(c,{...base,models:[{...base.models[0],scope:{...base.scope,connectionId:'foreign'}}]});assert.equal(r.models.length,0);assert.equal(check(r,'catalog').code,'invalid-catalog');
});

test('reports become stale on account revision, backend or runtime changes and on disconnect',()=>{
  const c=api();const r=report(c,catalog(c));assert.equal(D.isCurrent(r,c),true);
  for(const changed of [{...c,id:'other'},{...c,identity:{revision:2}},{...c,endpoint:'https://other.example/v1'},{...c,lifecycle:'disconnected'}])assert.equal(D.isCurrent(r,changed),false);
  const rt=runtime();const rr=report(rt);assert.equal(D.isCurrent(rr,{...rt,runtimeVersion:'next'}),false);
  assert.equal(D.isCurrent(r,c,catalog({...c,id:'other'})),false);
});

test('legacy, simulated and unbound catalogs cannot establish official observations',()=>{
  const c=api();
  for(const cat of [{models:[model()]},{...catalog(c),schemaVersion:undefined},{...catalog(c),source:'adapter'},{...catalog(c),source:{kind:'validated-simulated'}},{...catalog(c),scope:{provider:'openai'}}]){
    const r=report(c,cat);assert.equal(r.models.length,0);assert.equal(check(r,'catalog').status,'blocked');
  }
  const rt=runtime();const r=report(rt,catalog(rt,[model('one',{reasoning:cap('supported')})]));assert.equal(r.models.length,0);assert.equal(check(r,'identity').status,'blocked');assert.equal(check(r,'reasoning').status,'blocked');
});

test('only matching official identity-bound runtime observations unlock metadata inspection',()=>{
  const c=runtime();const cat=catalog(c,[model('one',{reasoning:cap('supported',{levels:['ultra']})})]);
  for(const patch of [{identityBound:false},{scope:{...scope(c),connectionId:'other'}},{scope:{...scope(c),revision:1}},{source:{kind:'user-declaration'}}])assert.equal(report(c,cat,{runtimeObservation:binding(c,patch)}).models.length,0);
  const r=report(c,cat,{runtimeObservation:binding(c)});assert.equal(r.models.length,1);assert.equal(check(r,'identity').status,'passed');assert.equal(r.features.reasoning.support,'supported');assert.equal(check(r,'access').status,'unknown');
});

test('Ultra reasoning, native delegation and Fast or Ultrafast tiers remain independent observations',()=>{
  const c=runtime();const models=[model('ultra-only',{reasoning:cap('supported',{levels:['ultra']})}),model('speed-only',{}, {serviceTiers:{value:[{id:'ultrafast',name:'Ultra fast',description:'tier'}]},multiAgentVersion:{value:'disabled'}}),model('agents-only',{}, {multiAgentVersion:{value:'v2'}})];
  const r=report(c,catalog(c,models),{runtimeObservation:binding(c)});
  assert.equal(r.models[0].features.fastMode.support,'unknown');assert.equal(r.models[0].features.nativeMultiAgent.support,'unknown');
  assert.equal(r.models[1].features.fastMode.support,'supported');assert.equal(r.models[1].features.nativeMultiAgent.support,'unsupported');assert.equal(r.models[1].features.reasoning.support,'unknown');
  assert.equal(r.models[2].features.nativeMultiAgent.support,'supported');assert.equal(r.models[2].features.fastMode.support,'unknown');
  assert.deepEqual(plain(r.features.fastMode.details.serviceTiers),['ultrafast']);assert.equal(r.policy.nativeDelegation.effective,'disabled');assert.equal(r.features.orchestrixMultiAgent.details.implementation,'planned');
});

test('permission observations require full-access sandbox and never approvals together; user grants stay independent',()=>{
  const c=runtime();const cat=catalog(c);
  for(const config of [{approval_policy:'never'},{sandbox_mode:'danger-full-access'},{sandbox_mode:'workspace-write',approval_policy:'never'}]){
    const r=report(c,cat,{runtimeObservation:binding(c,{config,features:{fullAccess:cap('supported')}})});
    assert.equal(r.features.fullAccess.support,'unknown');assert.equal(r.features.fullAccess.available,'unknown');
  }
  const r=report(c,cat,{runtimeObservation:binding(c,{config:{sandbox_mode:'danger-full-access',approval_policy:'never',agents:{enabled:true}}})});
  assert.equal(r.features.fullAccess.support,'supported');assert.equal(r.features.fullAccess.details.currentConfigurationAllowsFullAccess,true);assert.equal(r.features.fullAccess.details.executionGrantVerified,false);
  assert.equal(r.features.fullAccess.effectivePolicy,'requires-user-grant');assert.equal(r.features.fullAccess.available,'unknown');assert.equal(r.policy.hostPermissions.observedGrant,'unknown');
  assert.equal(r.policy.nativeDelegation.observedRuntimeState,'enabled');assert.equal(r.policy.nativeDelegation.effective,'disabled');assert.equal(check(r,'nativeDelegationPolicy').details.runtimeSettingChanged,false);
  assert.equal(r.policy.nativeDelegation.applyStatus,'not-applied');assert.equal(r.policy.nativeDelegation.observationStatus,'observed');assert.equal(r.policy.nativeDelegation.conflict,true);
});

test('Azure base models and manually declared deployments do not verify a deployment or its access',()=>{
  const c=api('azure',{deployment:'app'});const cat=catalog(c,[model('app',{}, {kind:'deployment',source:{kind:'user-declaration'}})],{scope:{...scope(c),endpoint:'https://resource.openai.azure.com/openai/v1'}});
  const manual=report(c,cat);assert.equal(check(manual,'catalog').status,'passed');assert.equal(manual.features.deployment.support,'unknown');assert.equal(manual.models[0].available,'unknown');
  const listed=report(c,{...cat,models:[model('app',{}, {kind:'deployment',source:{kind:'provider-metadata'}})]});assert.equal(listed.features.deployment.support,'supported');assert.equal(listed.features.deployment.available,'unknown');assert.equal(listed.features.deployment.details.inferenceAccessVerified,false);
});

test('Bedrock control/runtime endpoints match only their configured region; profile capabilities do not inherit',()=>{
  const c=api('bedrock',{region:'us-east-1',endpoint:'https://bedrock-runtime.us-east-1.amazonaws.com'});
  const cat=catalog(c,[model('foundation',{streaming:cap('supported')}),model('profile',{}, {kind:'inference-profile'})],{scope:{...scope(c),endpoint:'https://bedrock.us-east-1.amazonaws.com'}});
  const r=report(c,cat);assert.equal(check(r,'catalog').status,'passed');assert.equal(r.models[1].features.streaming.support,'unknown');assert.equal(r.features.regionProfile.details.profileCatalogObserved,true);assert.equal(r.features.regionProfile.support,'unknown');
  const mismatch=report(c,{...cat,scope:{...cat.scope,endpoint:'https://bedrock.us-west-2.amazonaws.com',region:'us-west-2'}});assert.equal(mismatch.models.length,0);
});

test('Gemini method observations do not become Antigravity runtime support',()=>{
  const c=api('gemini');const r=report(c,catalog(c,[model('models/one',{}, {methods:['generateContent','countTokens']})]));
  assert.equal(r.features.generationMethods.support,'supported');assert.equal(r.features.generationMethods.details.antigravityRuntime,false);assert.equal(r.features.nativeMultiAgent.support,'not-applicable');assert.equal(r.features.sandbox.support,'not-applicable');
  const antigravity=runtime('Antigravity');const wrong=report(antigravity,catalog(c,[model()]));assert.equal(wrong.models.length,0);assert.equal(check(wrong,'identity').status,'blocked');
});

test('running, cancellation, errors and disconnected reports cannot retain current availability or passed catalog',()=>{
  const c=api();const cat=catalog(c,[model('one',{reasoning:cap('supported')},{access:cap('supported')})]);
  for(const phase of ['queued','running','cancelled']){
    const r=report(c,cat,{phase});assert.equal(r.models.length,0);assert.equal(r.features.reasoning.support,'unknown');assert.notEqual(check(r,'catalog').status,'passed');assert.equal(r.completedAt,phase==='cancelled'?at:null);
  }
  for(const errorCode of ['adapter-unavailable','credentials-missing','disconnected']){const r=report(c,null,{errorCode});assert.equal(r.status,'blocked');assert.equal(check(r,'catalog').code,errorCode);}
  const failure=report(c,cat,{phase:'failed',errorCode:'rate-limited'});assert.equal(failure.status,'failed');assert.equal(failure.models.length,0);assert.equal(failure.features.reasoning.support,'unknown');
});

test('credential echoes and arbitrary errors are removed from selected report data',()=>{
  const key='synthetic-private-api-secret';const c=api();
  const r=report(c,catalog(c,[model('one',{reasoning:cap('supported',{levels:[key]})},{serviceTiers:[key]})]),{redactValues:[key],error:{message:key,stack:key},runId:key});
  assert.equal(JSON.stringify(r).includes(key),false);assert.equal(r.runId,null);assert.deepEqual(plain(r.models[0].features.reasoning.details.levels),[]);
  const malicious=report({...c,endpoint:`https://${key}.example/v1`,apiKey:key},catalog(c,[model(key)]));assert.equal(JSON.stringify(malicious).includes(key),false);assert.equal(check(malicious,'config').status,'failed');
  const raw=report(c,null,{phase:'failed',errorCode:key,error:{code:key,message:key}});assert.equal(JSON.stringify(raw).includes(key),false);assert.equal(check(raw,'catalog').code,'failed');
});

test('cross-account field evidence and manual declarations cannot promote feature capabilities',()=>{
  const c=runtime();const foreign={...scope(c),connectionId:'foreign'};
  const r=report(c,catalog(c,[model('one',{reasoning:cap('supported',{evidence:[{scope:foreign}]})},{serviceTiers:{value:[{id:'fast',name:'Fast'}],evidence:[{scope:foreign}]},multiAgentVersion:{value:'v2',evidence:[{scope:foreign}]}})]),{runtimeObservation:binding(c)});
  assert.equal(r.features.reasoning.support,'unknown');assert.equal(r.features.fastMode.support,'unknown');assert.equal(r.features.nativeMultiAgent.support,'unknown');
  const declared=report(api('azure',{deployment:'app'}),catalog(api('azure',{deployment:'app'}),[model('app',{reasoning:cap('supported')},{kind:'deployment',source:{kind:'user-declaration'}})]));assert.equal(declared.features.reasoning.support,'unknown');
  const fieldScoped=report(c,catalog(c,[model('one',{reasoning:cap('supported',{scope:foreign})})]),{runtimeObservation:binding(c)});assert.equal(fieldScoped.features.reasoning.support,'unknown');
});

test('resource access observations are scoped and do not flow from one model to another',()=>{
  const c=api();const r=report(c,catalog(c,[model('one',{}, {access:cap('supported')}),model('two',{}, {access:cap('unsupported')}),model('three')]));
  assert.deepEqual(plain(r.models.map(item=>item.available)),['available','unavailable','unknown']);assert.deepEqual(plain(r.summary.resources),{available:1,unavailable:1,unknown:1});assert.equal(check(r,'access').status,'unknown');
});

test('resource and value bounds prevent oversized, duplicate or unsafe report metadata',()=>{
  const c=api();const oversized=report(c,catalog(c,Array.from({length:501},(_,i)=>model(`model-${i}`))));assert.equal(oversized.models.length,0);
  const deduped=report(c,catalog(c,[model('same'),model('same'),model('same',{}, {kind:'deployment'})]));assert.equal(deduped.models.length,2);
  const control=report(c,catalog(c,[model('unsafe\nname')]));assert.equal(control.models.length,0);assert.equal(check(control,'catalog').code,'invalid-catalog');
  const fakeEndpoint=report({...c,endpoint:'https://attacker.example/v1'},catalog(c));assert.equal(check(fakeEndpoint,'config').status,'failed');
});

test('diagnostics never mutate connection, catalog, runtime config or user permissions',()=>{
  const c=runtime(),cat=catalog(c,[model('one',{reasoning:cap('supported',{levels:['high']})})]),runtimeObservation=binding(c,{config:{sandbox_mode:'workspace-write',approval_policy:'never',agents:{enabled:true}}});
  const before=JSON.stringify({c,cat,runtimeObservation});const r=report(c,cat,{runtimeObservation});
  assert.equal(JSON.stringify({c,cat,runtimeObservation}),before);assert.equal(r.policy.nativeDelegation.effective,'disabled');assert.equal(r.policy.orchestrixMultiAgent.status,'planned');
  assert.equal(r.checks.length,20);assert.equal(Object.values(r.summary.counts).reduce((a,b)=>a+b,0),r.checks.length);
});

test('runtime provider web capability does not become per-model native web or tool-calling support',()=>{
  const c=runtime();const cat=catalog(c,[model()],{providerCapabilities:{scope:scope(c),webSearch:cap('supported')}});
  const r=report(c,cat,{runtimeObservation:binding(c)});
  assert.equal(r.features.nativeWeb.support,'supported');assert.equal(r.features.nativeWeb.details.scope,'runtime-provider');assert.equal(r.models[0].features.nativeWeb.support,'unknown');assert.equal(r.models[0].features.functionCalling.support,'unknown');
});

test('service-tier display names and unversioned string extensions do not establish speed support',()=>{
  const c=runtime();const cat=catalog(c,[model('names-only',{}, {serviceTiers:{value:[{name:'fast'},{name:'ultrafast'}]}}),model('string-extension',{}, {serviceTiers:{value:['fast']}}),model('official-id',{}, {serviceTiers:{value:[{id:'ultrafast',name:'Display label'}]}})]);
  const r=report(c,cat,{runtimeObservation:binding(c)});
  assert.equal(r.models[0].features.fastMode.support,'unknown');assert.equal(r.models[1].features.fastMode.support,'unknown');assert.equal(r.models[2].features.fastMode.support,'supported');
});

test('missing runtime identity blocks an unattempted catalog while attempted authentication failures remain failed',()=>{
  const c=runtime();const unavailable=report(c,null,{errorCode:'authentication-required'});
  assert.equal(check(unavailable,'catalog').status,'blocked');assert.equal(unavailable.status,'blocked');
  const attempted=report(c,null,{errorCode:'authentication-required',attempted:true});assert.equal(check(attempted,'catalog').status,'failed');
  const providerFailure=report(api(),null,{errorCode:'authentication-required'});assert.equal(check(providerFailure,'catalog').status,'failed');
});

test('official scoped telemetry preserves zero, percentages, balances and reset without promoting unknown support',()=>{
  const c=api();const cat=catalog(c,[],{features:{usage:{status:'unknown',observed:true,scope:scope(c),usedPercent:0,remainingPercent:100,resetAt:'2026-10-11T03:00:00Z',credit:{remaining:12.5,spent:0,currency:'USD'}},credits:cap('supported',{details:{balance:0,amount:25.5,currency:'USD'}})}});
  const r=report(c,cat);assert.equal(r.features.usage.support,'unknown');assert.equal(r.features.usage.observed,true);assert.equal(r.features.usage.available,'unknown');
  assert.deepEqual(plain(r.features.usage.details),{usedPercent:0,remainingPercent:100,resetAt:'2026-10-11T03:00:00.000Z',credit:{remaining:12.5,spent:0,currency:'USD'}});
  assert.deepEqual(plain(r.features.credits.details),{balance:0,amount:25.5,currency:'USD'});assert.equal(r.features.credits.available,'unknown');assert.equal(check(r,'usage').status,'passed');
});

test('malformed, unobserved, foreign or secret telemetry is discarded rather than invented',()=>{
  const c=api();const key='synthetic-private-api-secret';
  const r=report(c,catalog(c,[],{features:{usage:{status:'unknown',observed:true,usedPercent:Infinity,remainingPercent:101,availablePercent:-1,resetAt:'2026-02-30T00:00:00Z',currency:key,credit:{balance:NaN,amount:-1,remaining:'10',spent:Infinity,currency:key}},credits:cap('supported',{balance:7,scope:{...scope(c),connectionId:'foreign'}})}}),{redactValues:[key]});
  assert.equal(r.features.usage.support,'unknown');assert.equal(r.features.usage.observed,false);assert.deepEqual(plain(r.features.usage.details),{});assert.deepEqual(plain(r.features.credits.details),{});assert.equal(JSON.stringify(r).includes(key),false);
  const unobserved=report(c,catalog(c,[],{features:{usage:{status:'supported',observed:false,usedPercent:60}}}));assert.equal(unobserved.features.usage.observed,false);assert.deepEqual(plain(unobserved.features.usage.details),{});
  const declared=report(c,catalog(c,[],{features:{credits:cap('supported',{balance:10,source:{kind:'user-declaration'}})}}));assert.equal(declared.features.credits.observed,false);
  const circular={balance:0};circular.credit=circular;const bounded=report(c,catalog(c,[],{features:{credits:cap('supported',{credit:circular})}}));assert.deepEqual(plain(bounded.features.credits.details),{credit:{balance:0}});
});
