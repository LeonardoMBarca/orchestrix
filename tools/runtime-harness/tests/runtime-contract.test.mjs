import test from 'node:test';
import assert from 'node:assert/strict';
import {RUNTIME_CONTRACT_VERSION,RuntimeContractError,RuntimeContractObserver,normalizeRuntimeDescriptor,normalizeRuntimeRequest,normalizeEffectiveConfiguration,capabilityDecision,assertRuntimeAdmission} from '../src/runtime-contract.mjs';
import {createFakeRuntime,fakeRuntimeRequest} from '../fixtures/fake-runtime.mjs';

const copy=value=>JSON.parse(JSON.stringify(value));
const kind=code=>error=>error instanceof RuntimeContractError&&error.code===code;
const proof={source:'synthetic',scope:'attempt',reference:'test-evidence'};
function fixture(options={}) {
  const runtime=createFakeRuntime(options),request=fakeRuntimeRequest();return {runtime,request,observer:new RuntimeContractObserver({descriptor:runtime.descriptor,request})};
}
function frame(observer,type,fields={}) {
  return JSON.stringify({version:RUNTIME_CONTRACT_VERSION,...observer.snapshot().scope,sequence:observer.sequence+1,type,...fields});
}
function send(observer,type,fields={},origin='runtime'){return observer.accept(frame(observer,type,fields),{origin});}
function start(observer){send(observer,'attempt.accepted');send(observer,'attempt.started');}
async function consume(runtime,observer,request=fakeRuntimeRequest()) {for await(const envelope of runtime.run(request))observer.accept(envelope.frame,{origin:envelope.origin});return observer.snapshot();}

test('successful provider-neutral fake normalizes a serial lifecycle without provider wire objects',async()=>{
  const {runtime,observer}=fixture();const result=await consume(runtime,observer);
  assert.equal(result.outcome,'completed');assert.equal(result.terminalConfirmed,true);assert.equal(result.phase,'terminal');
  assert.equal(result.effective.model.value,'model-1');assert.equal(result.process.runtime.exited,true);assert.equal(result.process.helper.exited,true);
  assert.equal(result.process.tree.termination,'confirmed');assert.equal(result.process.runtime.stdioClosed,true);assert.equal(result.process.helper.stdioClosed,true);
  assert.deepEqual(result.events.map(event=>event.sequence),Array.from({length:9},(_,index)=>index+1));
  assert.deepEqual(result.contextOwnership,{conversation:'runtime',projectMemory:'orchestrix'});assert.equal(runtime.descriptor.catalog.completeness,'partial');
  assert.equal(Object.isFrozen(result.effective.model),true);assert.throws(()=>{result.effective.model.value='other';},TypeError);
});

test('all required failure fixtures preserve observed facts and never fabricate completion',async()=>{
  for(const [scenario,outcome,phase] of [['auth-invalid','unknown','rejected'],['rate-limit','failed','terminal'],['interruption','interrupted','terminal'],['timeout','unknown','running'],['crash','unknown','running']]) {
    const {runtime,observer}=fixture({scenario});const result=await consume(runtime,observer);
    assert.equal(result.outcome,outcome,scenario);assert.equal(result.phase,phase,scenario);assert.equal(result.terminalConfirmed,['failed','interrupted'].includes(outcome),scenario);
    if(scenario==='auth-invalid')assert.equal(result.events[0].error.code,'authentication');
    if(scenario==='crash'){assert.equal(result.process.runtime.code,1);assert.equal(result.process.tree.termination,'unknown');}
    if(scenario==='timeout')assert.equal(result.observation,'uncertain');
    if(scenario==='interruption')assert.deepEqual(result.interrupt,{requested:true,acknowledged:true});
  }
});

test('invalid-event fake latches protocol failure without retaining its malformed frame',async()=>{
  const {runtime,observer}=fixture({scenario:'event-invalid'});
  await assert.rejects(consume(runtime,observer),kind('protocol'));const result=observer.snapshot();
  assert.equal(result.outcome,'unknown');assert.equal(result.fatal,true);assert.equal(result.events.length,0);
  assert.throws(()=>send(observer,'attempt.accepted'),kind('protocol'));assert.equal(observer.sequence,0);
});

test('fake execution is serial and releasing a generator does not create another effect',async()=>{
  const {runtime}=fixture();const first=runtime.run();assert.equal((await first.next()).done,false);
  const second=runtime.run();await assert.rejects(second.next(),kind('busy'));await first.return();
  let frames=0;for await(const envelope of runtime.run()){assert.equal(typeof envelope.frame,'string');frames++;}assert.equal(frames,9);
});

test('one model and one connection work without resume, quotas or parallel runtimes',async()=>{
  const {runtime,observer}=fixture({optionalUnsupported:true});assert.equal(runtime.descriptor.models.length,1);
  assert.equal(capabilityDecision(runtime.descriptor,'resume').allowed,false);assert.equal(capabilityDecision(runtime.descriptor,'resume').reason,'unsupported');
  assert.equal(capabilityDecision(runtime.descriptor,'quota').reason,'unknown');assert.equal(capabilityDecision(runtime.descriptor,'contextManagement').reason,'runtime-managed');
  assert.equal((await consume(runtime,observer)).outcome,'completed');
});

test('support, availability, configuration and evidence do not collapse into a boolean',()=>{
  const base=copy(createFakeRuntime().descriptor);
  for(const [overrides,reason] of [[{support:'unsupported'},'unsupported'],[{support:'unknown'},'unknown'],[{support:'runtime-managed'},'runtime-managed'],[{availability:'unknown'},'availability-unknown'],[{availability:'unavailable'},'availability-unavailable'],[{configuration:'unknown'},'configuration-unknown'],[{configuration:'disabled'},'configuration-disabled']]) {
    const descriptor=copy(base);Object.assign(descriptor.capabilities.resume,overrides);const decision=capabilityDecision(descriptor,'resume');assert.equal(decision.allowed,false);assert.equal(decision.reason,reason);
    assert.equal(decision.capability.evidence.source,'synthetic');
  }
  assert.equal(capabilityDecision(base,'resume').allowed,true);
});

test('connection capabilities are not silently inherited by every model',()=>{
  const descriptor=copy(createFakeRuntime().descriptor);descriptor.models.push({id:'model-2',availability:'available',capabilities:{}});
  const normalized=normalizeRuntimeDescriptor(descriptor);assert.equal(capabilityDecision(normalized,'reasoning').allowed,true);
  assert.equal(capabilityDecision(normalized,'reasoning',{modelId:'model-2'}).allowed,false);assert.equal(capabilityDecision(normalized,'reasoning',{modelId:'missing'}).reason,'unknown');
  const request=copy(fakeRuntimeRequest());request.requested.modelId='model-2';assert.throws(()=>assertRuntimeAdmission(normalized,request),kind('evidence-missing'));
  request.requested={modelPolicy:'auto',reasoning:'auto'};assert.equal(assertRuntimeAdmission(normalized,request),true);
});

test('reasoning levels are per model without union, and absent evidence is distinct from an empty list',()=>{
  const descriptor=copy(createFakeRuntime().descriptor);descriptor.models.push({...copy(descriptor.models[0]),id:'model-2',reasoningLevels:['high']});
  const request=copy(fakeRuntimeRequest());assert.equal(assertRuntimeAdmission(descriptor,request),true);
  request.requested.reasoning='maximum';assert.throws(()=>assertRuntimeAdmission(descriptor,request),kind('unsupported'));
  request.requested.reasoning='high';assert.throws(()=>assertRuntimeAdmission(descriptor,request),kind('unsupported'));
  request.requested.modelId='model-2';assert.equal(assertRuntimeAdmission(descriptor,request),true);
  request.requested.reasoning='low';assert.throws(()=>assertRuntimeAdmission(descriptor,request),kind('unsupported'));
  for(const levels of [undefined,null,[]]) {
    const model=copy(descriptor.models[0]);model.reasoningLevels=levels;
    const scoped=normalizeRuntimeDescriptor({...descriptor,models:[model]});const expected=Array.isArray(levels)?'unsupported':'evidence-missing';
    if(Array.isArray(levels))assert.deepEqual(scoped.models[0].reasoningLevels,[]);else assert.equal(scoped.models[0].reasoningLevels,null);
    assert.throws(()=>assertRuntimeAdmission(scoped,fakeRuntimeRequest()),kind(expected));
    assert.equal(scoped.models[0].capabilities.reasoning.support,'supported');
    const automatic=fakeRuntimeRequest({requested:{modelPolicy:'auto',reasoning:'auto'}});assert.equal(assertRuntimeAdmission(scoped,automatic),true);
  }
});

test('known effective effort must be advertised for the effective selected model',()=>{
  const descriptor=copy(createFakeRuntime().descriptor);descriptor.models.push({...copy(descriptor.models[0]),id:'model-2',reasoningLevels:['high']});
  const request=fakeRuntimeRequest({requested:{modelPolicy:'auto',reasoning:'auto'}});
  const effective=(model,reasoning)=>({model:{state:'known',value:model,evidence:proof},reasoning:{state:'known',value:reasoning,evidence:proof}});
  const valid=new RuntimeContractObserver({descriptor,request});send(valid,'configuration.observed',{effective:effective('model-2','high')});send(valid,'attempt.accepted');
  const wrong=new RuntimeContractObserver({descriptor,request});assert.throws(()=>send(wrong,'configuration.observed',{effective:effective('model-2','low')}),kind('unsupported'));
  descriptor.models[1].reasoningLevels=null;const missing=new RuntimeContractObserver({descriptor,request});assert.throws(()=>send(missing,'configuration.observed',{effective:effective('model-2','high')}),kind('evidence-missing'));
});

test('explicit effort with automatic model selection waits for a known selected model',()=>{
  const descriptor=createFakeRuntime().descriptor,request=fakeRuntimeRequest({requested:{modelPolicy:'auto',reasoning:'low'}});
  assert.throws(()=>assertRuntimeAdmission(descriptor,request),kind('evidence-missing'));
  const effective={model:{state:'known',value:'model-1',evidence:proof},reasoning:{state:'known',value:'low',evidence:proof}};
  assert.equal(assertRuntimeAdmission(descriptor,request,{effective}),true);
});

test('automatic selection cannot bypass known model unavailability, while unknown stays distinct',()=>{
  const descriptor=copy(createFakeRuntime().descriptor),request=fakeRuntimeRequest({requested:{modelPolicy:'auto',reasoning:'auto'}});
  const effective={model:{state:'known',value:'model-1',evidence:proof}};
  descriptor.models[0].availability='unavailable';
  const observer=new RuntimeContractObserver({descriptor,request});
  assert.throws(()=>send(observer,'configuration.observed',{effective}),kind('unavailable'));
  assert.throws(()=>assertRuntimeAdmission(descriptor,request,{effective}),kind('unavailable'));
  assert.equal(observer.snapshot().phase,'created');assert.equal(observer.snapshot().effective.model.state,'unknown');
  descriptor.models[0].availability='unknown';assert.equal(assertRuntimeAdmission(descriptor,request,{effective}),true);
  assert.equal(normalizeRuntimeDescriptor(descriptor).models[0].availability,'unknown');
});

test('Subscription Only and explicit API authorization never enable billing fallback',()=>{
  const descriptor=copy(createFakeRuntime().descriptor),request=copy(fakeRuntimeRequest());descriptor.connection.billing='api';
  assert.throws(()=>assertRuntimeAdmission(descriptor,request),kind('policy'));
  assert.throws(()=>normalizeRuntimeRequest({...request,paidFallback:'allowed'}),kind('policy'));
  assert.throws(()=>normalizeRuntimeRequest({...request,billingMode:'explicit-api'}),kind('policy'));
  const explicit=normalizeRuntimeRequest({...request,billingMode:'explicit-api',billingAuthorizationRef:'consent-1'});
  assert.equal(assertRuntimeAdmission(descriptor,explicit),true);assert.equal(explicit.paidFallback,'forbidden');
  assert.throws(()=>assertRuntimeAdmission(createFakeRuntime().descriptor,explicit),kind('policy'));
});

test('unknown auth and availability stay different from known authentication failure or rate limit',()=>{
  const base=copy(createFakeRuntime().descriptor);
  for(const [field,value,code] of [['authentication','unknown','authentication-unknown'],['authentication','unauthenticated','authentication'],['availability','unknown','availability-unknown'],['availability','unavailable','unavailable']]) {
    const descriptor=copy(base);descriptor.connection[field]=value;assert.throws(()=>assertRuntimeAdmission(descriptor,fakeRuntimeRequest()),kind(code));
  }
});

test('requested and effective remain distinct when CLI resolution is unknown',async()=>{
  const {runtime,observer}=fixture({effectiveUnknown:true});const result=await consume(runtime,observer);
  assert.equal(result.requested.modelId,'model-1');assert.equal(result.requested.reasoning,'low');
  for(const setting of Object.values(result.effective)){assert.equal(setting.state,'unknown');assert.equal(setting.value,null);assert.equal(setting.evidence.source,'none');}
  assert.equal(result.outcome,'completed');
  assert.throws(()=>normalizeEffectiveConfiguration({reasoning:{state:'unknown',value:'low'}}),kind('protocol'));
  assert.throws(()=>normalizeEffectiveConfiguration({reasoning:{state:'known',value:'low'}}),kind('protocol'));
});

test('every explicit known model, effort or execution mismatch blocks admission',()=>{
  for(const [key,value] of [['model','model-2'],['reasoning','high'],['sandbox','read-only'],['network','enabled'],['approval','ask']]) {
    const descriptor=copy(createFakeRuntime().descriptor);descriptor.models.push({id:'model-2',availability:'available',capabilities:{}});
    const observer=new RuntimeContractObserver({descriptor,request:fakeRuntimeRequest()});
    assert.throws(()=>send(observer,'configuration.observed',{effective:{[key]:{state:'known',value,evidence:proof}}}),kind('policy'));
    assert.equal(observer.snapshot().effective[key].state,'unknown');assert.equal(observer.snapshot().fatal,true);
    assert.throws(()=>send(observer,'attempt.accepted'),kind('policy'));assert.equal(observer.snapshot().outcome,'unknown');
  }
});

test('automatic intent can resolve a concrete model and effort without pretending unknown is known',()=>{
  const descriptor=createFakeRuntime().descriptor,request=fakeRuntimeRequest({requested:{modelPolicy:'auto',reasoning:'runtime-default'}});
  const observer=new RuntimeContractObserver({descriptor,request});
  send(observer,'configuration.observed',{effective:{model:{state:'known',value:'model-1',evidence:proof},reasoning:{state:'known',value:'low',evidence:proof}}});
  send(observer,'attempt.accepted');assert.equal(observer.snapshot().effective.reasoning.value,'low');assert.equal(observer.snapshot().requested.reasoning,'runtime-default');
});

test('ACK, helper exit, parent exit and closed pipes cannot imply a terminal or an empty tree',()=>{
  const {observer}=fixture();send(observer,'attempt.accepted');assert.equal(observer.snapshot().outcome,'unknown');assert.equal(observer.snapshot().phase,'accepted');
  send(observer,'process.exited',{role:'helper',code:0,signal:'none'},'supervisor');assert.equal(observer.snapshot().process.runtime.exited,false);
  send(observer,'process.exited',{role:'runtime',code:0,signal:'none'},'supervisor');
  send(observer,'process.streams-closed',{role:'runtime',streams:{stdin:true,stdout:true,stderr:true,faulted:false}},'supervisor');
  const state=observer.snapshot();assert.equal(state.outcome,'unknown');assert.equal(state.terminalConfirmed,false);assert.equal(state.process.tree.termination,'unknown');assert.equal(state.observation,'uncertain');
});

test('forced owned-tree cleanup with no runtime terminal preserves an unknown attempt outcome',()=>{
  const {observer}=fixture();start(observer);send(observer,'attempt.interrupt-requested');send(observer,'attempt.interrupt-acknowledged');
  assert.equal(observer.snapshot().outcome,'unknown');
  send(observer,'process.exited',{role:'runtime',code:137,signal:'killed'},'supervisor');
  send(observer,'process.tree-observed',{ownership:'confirmed',source:'owned-job',termination:'confirmed',activeCount:0,evidenceRef:'owned-launch-1'},'supervisor');
  send(observer,'process.exited',{role:'helper',code:0,signal:'none'},'supervisor');
  assert.equal(observer.snapshot().process.tree.termination,'confirmed');assert.equal(observer.snapshot().outcome,'unknown');assert.equal(observer.snapshot().terminalConfirmed,false);
});

test('provider JSON cannot forge supervisor provenance or attest tree ownership',()=>{
  const {observer}=fixture();start(observer);
  assert.throws(()=>send(observer,'process.exited',{role:'runtime',code:0,origin:'supervisor'}),kind('ownership'));
  assert.equal(observer.snapshot().process.runtime.exited,false);
  for(const fields of [{ownership:'unknown',source:'unknown',termination:'confirmed',activeCount:0,evidenceRef:'proof-1'},{ownership:'confirmed',source:'owned-job',termination:'confirmed',activeCount:1,evidenceRef:'proof-1'},{ownership:'confirmed',source:'owned-job',termination:'confirmed',activeCount:0}]) {
    const clean=fixture().observer;start(clean);send(clean,'process.exited',{role:'runtime',code:0},'supervisor');
    assert.throws(()=>send(clean,'process.tree-observed',fields,'supervisor'),error=>error instanceof RuntimeContractError);
    assert.equal(clean.snapshot().process.tree.termination,'unknown');
  }
});

test('supervisor cleanup cannot manufacture a runtime terminal from exit or a confirmed empty tree',()=>{
  const {observer}=fixture();start(observer);
  send(observer,'process.exited',{role:'runtime',code:0},'supervisor');
  send(observer,'process.tree-observed',{ownership:'confirmed',source:'owned-job',termination:'confirmed',activeCount:0,evidenceRef:'owned-proof-1'},'supervisor');
  assert.throws(()=>send(observer,'attempt.terminal',{outcome:'completed'},'supervisor'),kind('ownership'));
  assert.equal(observer.snapshot().outcome,'unknown');assert.equal(observer.snapshot().terminalConfirmed,false);
});

test('fatal runtime quarantine still records independently owned supervisor cleanup within its budgets',()=>{
  const {observer}=fixture();start(observer);assert.throws(()=>observer.accept('{broken'),kind('protocol'));
  send(observer,'process.exited',{role:'runtime',code:1},'supervisor');
  send(observer,'process.streams-closed',{role:'runtime',streams:{stdin:true,stdout:true,stderr:true,faulted:false}},'supervisor');
  send(observer,'process.tree-observed',{ownership:'confirmed',source:'owned-job',termination:'confirmed',activeCount:0,evidenceRef:'owned-proof-1'},'supervisor');
  assert.equal(observer.snapshot().process.tree.termination,'confirmed');assert.equal(observer.snapshot().fatal,true);
  assert.throws(()=>send(observer,'attempt.terminal',{outcome:'completed'}),kind('protocol'));assert.equal(observer.snapshot().outcome,'unknown');
});

test('loss or timeout can reconcile a late owned terminal but cannot infer it',()=>{
  for(const reason of ['timeout','transport-loss','crash']) {
    const {observer}=fixture();start(observer);observer.transportLost(reason);
    assert.equal(observer.snapshot().outcome,'unknown');assert.equal(observer.snapshot().observation,'uncertain');
    send(observer,'attempt.terminal',{outcome:'failed',error:{code:reason,phase:'running',dispatch:'unknown'}});
    assert.equal(observer.snapshot().outcome,'failed');assert.equal(observer.snapshot().terminalConfirmed,true);
  }
});

test('confirmed terminal survives later transport loss and contradictory terminal without changing its result',()=>{
  const {observer}=fixture();start(observer);send(observer,'attempt.terminal',{outcome:'completed'});observer.transportLost('timeout');
  assert.equal(observer.snapshot().outcome,'completed');assert.equal(observer.snapshot().terminalConfirmed,true);
  assert.throws(()=>send(observer,'attempt.terminal',{outcome:'failed'}),kind('protocol'));
  assert.equal(observer.snapshot().outcome,'completed');assert.equal(observer.snapshot().events.filter(event=>event.type==='attempt.terminal').length,1);
});

test('close is irreversible through repeated invalid accepts and later transport loss',()=>{
  const {observer}=fixture();start(observer);const sequence=observer.sequence;observer.close();
  for(let count=0;count<2;count++){assert.throws(()=>send(observer,'attempt.terminal',{outcome:'completed'}),kind('transport-loss'));assert.equal(observer.snapshot().observation,'closed');}
  observer.transportLost('timeout');assert.equal(observer.snapshot().observation,'closed');assert.equal(observer.sequence,sequence);assert.equal(observer.snapshot().outcome,'unknown');
});

test('ownership, revision, sequence, version and malformed frames latch failure before a later completion',()=>{
  for(const [overrides,code] of [[{connectionId:'foreign-connection'},'ownership'],[{sessionId:'foreign-session'},'ownership'],[{attemptId:'foreign-attempt'},'ownership'],[{connectionRevision:2},'ownership'],[{sequence:99},'sequence'],[{version:{major:2,minor:0}},'version']]) {
    const {observer}=fixture();start(observer);const previous=observer.sequence;
    assert.throws(()=>observer.accept(frame(observer,'attempt.terminal',{outcome:'completed',...overrides})),kind(code));
    assert.throws(()=>send(observer,'attempt.terminal',{outcome:'completed'}),kind(code));assert.equal(observer.sequence,previous);assert.equal(observer.snapshot().outcome,'unknown');
  }
  const {observer}=fixture();start(observer);assert.throws(()=>observer.accept('{broken'),kind('protocol'));assert.throws(()=>send(observer,'attempt.terminal',{outcome:'completed'}),kind('protocol'));
});

test('out-of-order start, completion and interrupt ACK are rejected; explicit failed terminal before start is allowed',()=>{
  for(const type of ['attempt.started','attempt.terminal','attempt.interrupt-acknowledged']) {
    const {observer}=fixture();assert.throws(()=>send(observer,type,{outcome:'completed'}),kind('protocol'));assert.equal(observer.snapshot().outcome,'unknown');
  }
  const {observer}=fixture();send(observer,'attempt.accepted');send(observer,'attempt.terminal',{outcome:'failed'});assert.equal(observer.snapshot().outcome,'failed');
});

test('replayed events do not produce duplicate state transitions or side effects',()=>{
  const {observer}=fixture();const accepted=frame(observer,'attempt.accepted');observer.accept(accepted);
  assert.throws(()=>observer.accept(accepted),kind('sequence'));assert.equal(observer.snapshot().events.length,1);assert.equal(observer.snapshot().phase,'accepted');
  assert.throws(()=>send(observer,'attempt.started'),kind('sequence'));assert.equal(observer.snapshot().outcome,'unknown');
});

test('compatible future minor fields are ignored, while unknown events become sanitized diagnostics',()=>{
  const descriptor=copy(createFakeRuntime().descriptor);descriptor.version.minor=7;descriptor.providerBlob={apiKey:'SYNTHETIC_SECRET'};
  const request=copy(fakeRuntimeRequest());request.version.minor=9;request.prompt='SYNTHETIC_SECRET';request.requested.token='SYNTHETIC_SECRET';
  const observer=new RuntimeContractObserver({descriptor,request});
  send(observer,'SYNTHETIC_SECRET',{version:{major:1,minor:12},payload:{credentials:'SYNTHETIC_SECRET'},nativeThread:'SYNTHETIC_SECRET'});
  const result=observer.snapshot();assert.equal(result.events[0].type,'runtime.diagnostic');assert.equal(result.events[0].code,'unknown-event');assert.equal(result.events[0].version.minor,12);
  assert.equal(JSON.stringify(observer.descriptor).includes('SYNTHETIC_SECRET'),false);assert.equal(JSON.stringify(observer.request).includes('SYNTHETIC_SECRET'),false);assert.equal(JSON.stringify(result).includes('SYNTHETIC_SECRET'),false);
});

test('runtime errors preserve typed classification without raw provider messages or secrets',()=>{
  const {observer}=fixture();send(observer,'runtime.error',{error:{code:'authentication',phase:'connect',dispatch:'not-sent',message:'SYNTHETIC_SECRET',providerBlob:{token:'SYNTHETIC_SECRET'}}});
  assert.equal(observer.snapshot().phase,'rejected');assert.equal(JSON.stringify(observer.snapshot()).includes('SYNTHETIC_SECRET'),false);
  const clean=fixture().observer;assert.throws(()=>send(clean,'runtime.error',{error:{code:'SYNTHETIC_SECRET',phase:'connect'}}),error=>!error.message.includes('SYNTHETIC_SECRET')&&error.code==='protocol');
});

test('frame, aggregate byte and event budgets fail closed, including UTF-8 byte accounting',()=>{
  const runtime=createFakeRuntime(),request=fakeRuntimeRequest();
  for(const limits of [{maxFrameBytes:1},{maxTotalBytes:1}]) {
    const observer=new RuntimeContractObserver({descriptor:runtime.descriptor,request,...limits});assert.throws(()=>send(observer,'attempt.accepted'),kind('budget'));assert.equal(observer.snapshot().fatal,true);assert.equal(observer.sequence,0);
  }
  const observer=new RuntimeContractObserver({descriptor:runtime.descriptor,request,maxEvents:1});send(observer,'attempt.accepted');assert.throws(()=>send(observer,'attempt.started'),kind('budget'));assert.equal(observer.snapshot().events.length,1);
  const clean=new RuntimeContractObserver({descriptor:runtime.descriptor,request});const text=frame(clean,'future.event',{ignored:'\u00e9'.repeat(20)});
  const bounded=new RuntimeContractObserver({descriptor:runtime.descriptor,request,maxFrameBytes:text.length});assert.throws(()=>bounded.accept(text),kind('budget'));
  for(const options of [{maxEvents:0},{maxFrameBytes:1048577},{maxTotalBytes:16777217}])assert.throws(()=>new RuntimeContractObserver({descriptor:runtime.descriptor,request,...options}),kind('budget'));
});

test('descriptor/request major incompatibility and mismatched connection revision fail before events',()=>{
  const descriptor=copy(createFakeRuntime().descriptor),request=copy(fakeRuntimeRequest());
  assert.throws(()=>normalizeRuntimeDescriptor({...descriptor,version:{major:2,minor:0}}),kind('version'));
  assert.throws(()=>normalizeRuntimeRequest({...request,version:{major:0,minor:0}}),kind('version'));
  assert.throws(()=>new RuntimeContractObserver({descriptor,request:{...request,connectionRevision:2}}),kind('ownership'));
  descriptor.models.push(copy(descriptor.models[0]));assert.throws(()=>normalizeRuntimeDescriptor(descriptor),kind('protocol'));
});
