import test from 'node:test';
import assert from 'node:assert/strict';
import {RuntimeContractObserver,RuntimeContractError,normalizeRuntimeRequest,assertRuntimeAdmission} from '../src/runtime-contract.mjs';
import {createFakeRuntime,fakeRuntimeRequest} from '../fixtures/fake-runtime.mjs';

const copy=value=>JSON.parse(JSON.stringify(value));
const kind=code=>error=>error instanceof RuntimeContractError&&error.code===code;
const proof={source:'configuration',scope:'attempt',reference:'configuration-1'};
function fixture({catalogUnknown=false,turnId='turn-1'}={}) {
  const descriptor=copy(createFakeRuntime().descriptor);
  if(catalogUnknown){descriptor.catalog.completeness='unknown';descriptor.models[0].availability='unknown';descriptor.models[0].reasoningLevels=null;descriptor.models[0].capabilities.reasoning={support:'unknown'};}
  const request=fakeRuntimeRequest({turnId});return {descriptor,request,observer:new RuntimeContractObserver({descriptor,request})};
}
function frame(observer,type,fields={}) {return JSON.stringify({version:{major:1,minor:1},...observer.snapshot().scope,sequence:observer.sequence+1,type,...fields});}
function send(observer,type,fields={},origin='runtime'){return observer.accept(frame(observer,type,fields),{origin});}

test('observed admission records a real ACK without inventing model entitlement or rerunning host preflight',()=>{
  const {descriptor,request,observer}=fixture({catalogUnknown:true});assert.throws(()=>assertRuntimeAdmission(descriptor,request),kind('availability-unknown'));
  send(observer,'attempt.admission-observed');const state=observer.snapshot();
  assert.equal(state.phase,'accepted');assert.equal(state.admissionObserved,true);assert.equal(state.startObserved,false);assert.equal(state.outcome,'unknown');
  assert.equal(observer.descriptor.models[0].availability,'unknown');assert.equal(observer.descriptor.models[0].reasoningLevels,null);
  const legacy=new RuntimeContractObserver({descriptor,request});assert.throws(()=>send(legacy,'attempt.accepted'),kind('availability-unknown'));
});

test('terminal observed before any ACK or start is factual completion with both observations absent',()=>{
  const {observer}=fixture({catalogUnknown:true});const event=send(observer,'attempt.terminal-observed',{outcome:'completed'});const state=observer.snapshot();
  assert.equal(event.startObserved,false);assert.equal(state.outcome,'completed');assert.equal(state.terminalConfirmed,true);
  assert.equal(state.admissionObserved,false);assert.equal(state.startObserved,false);assert.equal(state.phase,'terminal');
  send(observer,'attempt.admission-observed');assert.equal(observer.snapshot().admissionObserved,true);assert.equal(observer.snapshot().phase,'terminal');assert.equal(observer.snapshot().outcome,'completed');
});

test('completed after ACK without start remains completed without synthesizing a started event',()=>{
  const {observer}=fixture();send(observer,'attempt.admission-observed');send(observer,'attempt.terminal-observed',{outcome:'completed',startObserved:false});
  assert.equal(observer.snapshot().startObserved,false);assert.equal(observer.snapshot().admissionObserved,true);
  assert.deepEqual(observer.snapshot().events.map(event=>event.type),['attempt.admission-observed','attempt.terminal-observed']);
  const strict=fixture().observer;send(strict,'attempt.accepted');assert.throws(()=>send(strict,'attempt.terminal',{outcome:'completed'}),kind('protocol'));
});

test('start observed before ACK and a later ACK cannot regress running or terminal phase',()=>{
  const {observer}=fixture();send(observer,'attempt.start-observed');
  assert.equal(observer.snapshot().phase,'running');assert.equal(observer.snapshot().startObserved,true);assert.equal(observer.snapshot().admissionObserved,false);
  send(observer,'attempt.admission-observed');assert.equal(observer.snapshot().phase,'running');assert.equal(observer.snapshot().admissionObserved,true);
  const terminal=send(observer,'attempt.terminal-observed',{outcome:'completed'});assert.equal(terminal.startObserved,true);
  const early=fixture().observer;send(early,'attempt.start-observed');send(early,'attempt.terminal-observed',{outcome:'completed'});send(early,'attempt.admission-observed');
  assert.equal(early.snapshot().phase,'terminal');assert.equal(early.snapshot().startObserved,true);assert.equal(early.snapshot().outcome,'completed');
});

test('v1.0 attempt.started still requires admission and observational start is never fabricated from a claim',()=>{
  const {observer}=fixture();assert.throws(()=>send(observer,'attempt.started'),kind('protocol'));
  const clean=fixture().observer;assert.throws(()=>send(clean,'attempt.terminal-observed',{outcome:'completed',startObserved:true}),kind('protocol'));
  assert.equal(clean.snapshot().startObserved,false);assert.equal(clean.snapshot().outcome,'unknown');
});

test('interrupted terminal can precede interrupt ACK, whose result adds evidence without replacing terminal',()=>{
  for(const acknowledged of [true,false]) {
    const {observer}=fixture();send(observer,'attempt.start-observed');send(observer,'attempt.interrupt-requested');
    send(observer,'attempt.terminal-observed',{outcome:'interrupted'});const terminal=copy(observer.snapshot().terminal);
    send(observer,'attempt.interrupt-result',{acknowledged});assert.deepEqual(observer.snapshot().terminal,terminal);
    assert.equal(observer.snapshot().interrupt.acknowledged,acknowledged);assert.equal(observer.snapshot().outcome,'interrupted');
  }
  // Older recordings retained the RPC result, without a separate send observation.
  const {observer}=fixture();send(observer,'attempt.terminal-observed',{outcome:'interrupted'});
  send(observer,'attempt.interrupt-result',{acknowledged:true});
  assert.equal(observer.snapshot().interrupt.requested,false);assert.equal(observer.snapshot().interrupt.acknowledged,true);
  assert.equal(observer.snapshot().startObserved,false);assert.equal(observer.snapshot().outcome,'interrupted');
  const strict=fixture().observer;assert.throws(()=>send(strict,'attempt.interrupt-acknowledged'),kind('protocol'));
});

test('interrupt ACK alone cannot imply interruption and acknowledged evidence cannot be erased',()=>{
  const {observer}=fixture();send(observer,'attempt.admission-observed');send(observer,'attempt.interrupt-requested');send(observer,'attempt.interrupt-result',{acknowledged:true});
  assert.equal(observer.snapshot().outcome,'unknown');assert.equal(observer.snapshot().terminalConfirmed,false);
  assert.throws(()=>send(observer,'attempt.interrupt-result',{acknowledged:false}),kind('protocol'));assert.equal(observer.snapshot().interrupt.acknowledged,true);
});

test('configuration facts can be recorded with missing catalog evidence and visible requested/effective differences',()=>{
  const {observer}=fixture({catalogUnknown:true});const effective={model:{state:'known',value:'model-1',evidence:proof},reasoning:{state:'known',value:'high',evidence:proof},sandbox:{state:'known',value:'read-only',evidence:proof}};
  send(observer,'configuration.observation-recorded',{effective});
  assert.equal(observer.snapshot().effective.reasoning.value,'high');assert.equal(observer.snapshot().requested.reasoning,'low');assert.equal(observer.snapshot().effective.sandbox.value,'read-only');
  assert.equal(observer.snapshot().phase,'created');assert.equal(observer.snapshot().admissionObserved,false);
  const strict=fixture({catalogUnknown:true}).observer;assert.throws(()=>send(strict,'configuration.observed',{effective}),kind('policy'));
  const absent=fixture().observer;assert.throws(()=>send(absent,'configuration.observation-recorded',{effective:{model:{state:'known',value:'foreign-model',evidence:proof}}}),kind('ownership'));
});

test('recording configuration never skips schema validation or fabricates known values',()=>{
  const {observer}=fixture();send(observer,'configuration.observation-recorded',{effective:{}});
  assert.equal(observer.snapshot().effective.model.state,'unknown');assert.equal(observer.snapshot().effective.model.value,null);
  const clean=fixture().observer;assert.throws(()=>send(clean,'configuration.observation-recorded',{effective:{reasoning:{state:'known',value:'high'}}}),kind('protocol'));
});

test('output counters retain channel and byte counts while discarding raw messages, commands and credentials',()=>{
  const {observer}=fixture();for(const [channel,bytes] of [['assistant',12],['tool',8],['other',0]])send(observer,'output.observed',{channel,bytes,text:'SYNTHETIC_SECRET',command:'SYNTHETIC_SECRET',providerBlob:{token:'SYNTHETIC_SECRET'}});
  assert.deepEqual(observer.snapshot().output,{assistant:12,tool:8,other:0,bytes:20,events:3});assert.equal(JSON.stringify(observer.snapshot()).includes('SYNTHETIC_SECRET'),false);
  assert.equal(observer.snapshot().startObserved,false);assert.equal(observer.snapshot().admissionObserved,false);assert.equal(observer.snapshot().outcome,'unknown');
  for(const bytes of [-1,1.5,null]) {const clean=fixture().observer;assert.throws(()=>send(clean,'output.observed',{channel:'assistant',bytes}),kind('protocol'));}
});

test('output counter overflow and frame/event limits remain fail closed',()=>{
  const {observer}=fixture();send(observer,'output.observed',{channel:'assistant',bytes:Number.MAX_SAFE_INTEGER});
  assert.throws(()=>send(observer,'output.observed',{channel:'tool',bytes:1}),kind('budget'));assert.equal(observer.snapshot().output.bytes,Number.MAX_SAFE_INTEGER);
  const {descriptor,request}=fixture();const bounded=new RuntimeContractObserver({descriptor,request,maxEvents:1});send(bounded,'output.observed',{channel:'other',bytes:0});assert.throws(()=>send(bounded,'attempt.admission-observed'),kind('budget'));
});

test('optional local turn identity binds requests, envelopes and snapshots with legacy absence normalized to null',()=>{
  const {observer,request}=fixture();assert.equal(request.turnId,'turn-1');assert.equal(observer.snapshot().scope.turnId,'turn-1');
  assert.throws(()=>observer.accept(frame(observer,'attempt.admission-observed',{turnId:'foreign-turn'})),kind('ownership'));
  const absent=fixture({turnId:null}).observer;const legacy=JSON.parse(frame(absent,'attempt.admission-observed'));delete legacy.turnId;
  absent.accept(JSON.stringify(legacy));assert.equal(absent.snapshot().scope.turnId,null);assert.equal(absent.snapshot().events[0].turnId,null);
  const raw=copy(fakeRuntimeRequest());delete raw.turnId;assert.equal(normalizeRuntimeRequest(raw).turnId,null);
  const required=fixture().observer;const missing=JSON.parse(frame(required,'attempt.admission-observed'));delete missing.turnId;assert.throws(()=>required.accept(JSON.stringify(missing)),kind('ownership'));
});

test('typed fatal runtime errors quarantine runtime outcomes while preserving independently supervised cleanup',()=>{
  for(const code of ['protocol','ownership','sequence','version','budget']) {
    const {observer}=fixture();send(observer,'attempt.start-observed');send(observer,'runtime.error',{fatal:true,error:{code,phase:'running',dispatch:'unknown',message:'SYNTHETIC_SECRET'}});
    assert.equal(observer.snapshot().fatal,true);assert.equal(observer.snapshot().fault.code,code);
    send(observer,'process.exited',{role:'runtime',code:1},'supervisor');send(observer,'process.tree-observed',{ownership:'confirmed',source:'owned-job',termination:'confirmed',activeCount:0,evidenceRef:'owned-job-proof'},'supervisor');
    assert.equal(observer.snapshot().process.tree.termination,'confirmed');assert.throws(()=>send(observer,'attempt.terminal-observed',{outcome:'completed'}),kind(code));
    assert.equal(observer.snapshot().outcome,'unknown');assert.equal(JSON.stringify(observer.snapshot()).includes('SYNTHETIC_SECRET'),false);
  }
});

test('nonfatal loss can still observe a late factual terminal without reviving closed state',()=>{
  const {observer}=fixture();send(observer,'runtime.error',{error:{code:'timeout',phase:'running',dispatch:'unknown'}});assert.equal(observer.snapshot().fatal,false);
  send(observer,'attempt.terminal-observed',{outcome:'failed',error:{code:'timeout',phase:'running',dispatch:'unknown'}});assert.equal(observer.snapshot().outcome,'failed');
  observer.close();assert.throws(()=>send(observer,'attempt.admission-observed'),kind('transport-loss'));assert.equal(observer.snapshot().observation,'closed');assert.equal(observer.snapshot().outcome,'failed');
});

test('new observational events in v1.0 envelopes degrade to diagnostics without changing facts',()=>{
  const {observer}=fixture();for(const type of ['configuration.observation-recorded','attempt.admission-observed','attempt.start-observed','attempt.terminal-observed','attempt.interrupt-result','output.observed'])send(observer,type,{version:{major:1,minor:0},outcome:'completed',acknowledged:true,channel:'assistant',bytes:10});
  assert.equal(observer.snapshot().events.every(event=>event.type==='runtime.diagnostic'&&event.code==='unknown-event'),true);
  assert.equal(observer.snapshot().outcome,'unknown');assert.equal(observer.snapshot().admissionObserved,false);assert.equal(observer.snapshot().startObserved,false);assert.equal(observer.snapshot().output.bytes,0);
});

test('new factual events remain runtime-owned and cannot be produced by exit-only supervisor evidence',()=>{
  for(const type of ['configuration.observation-recorded','attempt.admission-observed','attempt.start-observed','attempt.terminal-observed','attempt.interrupt-result','output.observed']) {
    const {observer}=fixture();assert.throws(()=>send(observer,type,{outcome:'completed',acknowledged:true,channel:'assistant',bytes:10},'supervisor'),kind('ownership'));
    assert.equal(observer.snapshot().outcome,'unknown');assert.equal(observer.snapshot().admissionObserved,false);
  }
});

test('normalized terminal frames roundtrip through the same provider-neutral observer with explicit error null',()=>{
  for(const [terminalType,outcome] of [['attempt.terminal','completed'],['attempt.terminal','failed'],['attempt.terminal-observed','completed'],['attempt.terminal-observed','interrupted']]) {
    const {descriptor,request,observer}=fixture();
    if(terminalType==='attempt.terminal'){send(observer,'attempt.accepted',{version:{major:1,minor:0}});send(observer,'attempt.started',{version:{major:1,minor:0}});}
    else {send(observer,'attempt.start-observed');send(observer,'output.observed',{channel:'assistant',bytes:12});}
    send(observer,terminalType,{outcome,...(terminalType==='attempt.terminal'?{version:{major:1,minor:0}}:{})});
    send(observer,'process.exited',{role:'runtime',code:0},'supervisor');
    send(observer,'process.tree-observed',{ownership:'confirmed',source:'owned-job',termination:'confirmed',activeCount:0,evidenceRef:'job-proof-1'},'supervisor');
    const source=observer.snapshot();assert.equal(source.events.find(event=>event.type===terminalType).error,null);
    const consumer=new RuntimeContractObserver({descriptor,request});
    for(const event of source.events)consumer.accept(JSON.stringify(event),{origin:event.origin});
    const replayed=consumer.snapshot();assert.deepEqual(replayed.events,source.events);assert.deepEqual(replayed.terminal,source.terminal);
    assert.equal(replayed.outcome,source.outcome);assert.equal(replayed.startObserved,source.startObserved);assert.equal(replayed.admissionObserved,source.admissionObserved);
    assert.deepEqual(replayed.output,source.output);assert.deepEqual(replayed.process,source.process);
  }
});

test('stream observations advance partial closure monotonically and retain a previously observed fault',()=>{
  const {observer}=fixture();
  send(observer,'process.streams-observed',{role:'runtime',streams:{stdin:true,stdout:false,stderr:false,faulted:false}},'supervisor');
  assert.equal(observer.snapshot().process.runtime.stdioClosed,false);
  send(observer,'process.streams-observed',{role:'runtime',streams:{stdin:false,stdout:true,stderr:true,faulted:false}},'supervisor');
  assert.deepEqual(observer.snapshot().process.runtime.streams,{stdin:true,stdout:true,stderr:true,faulted:false});
  assert.equal(observer.snapshot().process.runtime.stdioClosed,true);
  send(observer,'process.streams-observed',{role:'helper',streams:{stdin:true,stdout:false,stderr:false,faulted:true}},'supervisor');
  send(observer,'process.streams-observed',{role:'helper',streams:{stdin:true,stdout:true,stderr:true,faulted:false}},'supervisor');
  assert.equal(observer.snapshot().process.helper.streams.faulted,true);assert.equal(observer.snapshot().process.helper.stdioClosed,false);
  assert.equal(observer.snapshot().outcome,'unknown');assert.equal(observer.snapshot().terminalConfirmed,false);
  const consumer=new RuntimeContractObserver({descriptor:observer.descriptor,request:observer.request});
  for(const event of observer.snapshot().events)consumer.accept(JSON.stringify(event),{origin:event.origin});
  assert.deepEqual(consumer.snapshot().process,observer.snapshot().process);
});

test('stream updates retain supervisor origin, version boundary and strict legacy closure semantics',()=>{
  const fields={role:'runtime',streams:{stdin:true,stdout:true,stderr:true,faulted:false}};
  const {observer}=fixture();assert.throws(()=>send(observer,'process.streams-observed',fields),kind('ownership'));
  const old=fixture().observer;send(old,'process.streams-observed',{...fields,version:{major:1,minor:0}},'supervisor');
  assert.equal(old.snapshot().events[0].type,'runtime.diagnostic');assert.equal(old.snapshot().process.runtime.streams,null);
  const strict=fixture().observer;send(strict,'process.streams-closed',fields,'supervisor');
  assert.throws(()=>send(strict,'process.streams-closed',fields,'supervisor'),kind('protocol'));
});
