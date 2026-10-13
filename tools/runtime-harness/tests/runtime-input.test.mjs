import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {RUNTIME_INPUT_PROFILE,RuntimeContractError,RuntimeContractObserver,normalizeEffectiveConfiguration,normalizeRuntimeDescriptor,normalizeRuntimeRequest} from '../src/runtime-contract.mjs';
import {createFakeRuntime,fakeRuntimeRequest} from '../fixtures/fake-runtime.mjs';

const vectors=JSON.parse(readFileSync(new URL('../fixtures/portable-input-v1.json',import.meta.url),'utf8'));
const proof={source:'synthetic',scope:'attempt',reference:'codec-test'};
const setting=observedAt=>({reasoning:{state:'known',value:'low',evidence:{...proof,observedAt}}});
const isProtocol=error=>error instanceof RuntimeContractError&&error.code==='protocol';
const fixture=()=>({descriptor:createFakeRuntime().descriptor,request:fakeRuntimeRequest()});

test('bounded local IDs and version tokens cannot accept a terminal newline',()=>{
  for(const field of ['adapterId','runtimeVersion']){const value={...fixture().descriptor,[field]:'valid\n'};assert.throws(()=>normalizeRuntimeDescriptor(value),isProtocol);}
  assert.throws(()=>normalizeRuntimeRequest({...fixture().request,sessionId:'session-1\n'}),error=>error.code==='ownership');
});

test('multiple invalid known fields retain deterministic validation priority',()=>{
  const descriptor=JSON.parse(JSON.stringify(fixture().descriptor));descriptor.connection.authentication='invalid';descriptor.models[0].id='!';
  assert.throws(()=>normalizeRuntimeDescriptor(descriptor),isProtocol);
  assert.throws(()=>normalizeRuntimeRequest({...fixture().request,connectionRevision:0,version:{major:2,minor:0}}),error=>error.code==='ownership');
  assert.throws(()=>normalizeEffectiveConfiguration({reasoning:{state:'known',value:'low',evidence:{...proof,reference:'!',observedAt:'invalid'}}}),error=>error.code==='ownership');
});

test('the portable ingress profile is explicit and unsupported profiles cannot downgrade it',()=>{
  assert.deepEqual(RUNTIME_INPUT_PROFILE,vectors.inputProfile);assert.equal(Object.isFrozen(RUNTIME_INPUT_PROFILE),true);
  for(const inputProfile of [null,{id:'legacy-date-parse',revision:1},{...RUNTIME_INPUT_PROFILE,revision:2}])
    assert.throws(()=>new RuntimeContractObserver({...fixture(),inputProfile}),error=>error.code==='version');
});
for(const vector of vectors.timestamps) {
  test(`portable timestamp ${vector.name}: explicit expected UTC or protocol rejection`,()=>{
    const descriptor=JSON.parse(JSON.stringify(fixture().descriptor));
    descriptor.capabilities.resume.evidence={...proof,observedAt:vector.input};
    if(vector.output===null){assert.throws(()=>normalizeEffectiveConfiguration(setting(vector.input)),isProtocol);assert.throws(()=>normalizeRuntimeDescriptor(descriptor),isProtocol);}
    else {
      const observed=normalizeEffectiveConfiguration(setting(vector.input));
      assert.equal(observed.reasoning.evidence.observedAt,vector.output);
      assert.deepEqual(normalizeEffectiveConfiguration(observed),observed);
      assert.equal(normalizeRuntimeDescriptor(descriptor).capabilities.resume.evidence.observedAt,vector.output);
    }
  });
}
for(const vector of vectors.frames) {
  test(`portable JSON ${vector.name}: rejected frames preserve uncertainty and allow owned cleanup`,()=>{
    const observer=new RuntimeContractObserver(fixture());
    const header={...observer.snapshot().scope,version:{major:1,minor:1},sequence:1,type:vector.type};
    const frame=JSON.stringify(header).slice(0,-1)+vector.tail+'}';
    if(vector.code) {
      assert.throws(()=>observer.accept(frame),error=>error.code===vector.code);
      const state=observer.snapshot();assert.equal(state.fatal,true);assert.equal(state.outcome,'unknown');assert.equal(state.events.length,0);
      assert.equal(state.effective.reasoning.state,'unknown');assert.equal(state.budget.bytes,0);
      const cleanup={...header,type:'process.exited',role:'runtime',code:137,signal:'killed'};
      observer.accept(JSON.stringify(cleanup),{origin:'supervisor'});
      assert.equal(observer.snapshot().process.runtime.code,137);assert.equal(observer.snapshot().terminalConfirmed,false);
    } else {
      const event=observer.accept(frame);assert.equal(observer.snapshot().observation,'healthy');
      if(vector.bytes!==undefined){assert.equal(event.bytes,vector.bytes);assert.equal(observer.snapshot().output.bytes,vector.bytes);}
      assert.equal(JSON.stringify(observer.snapshot()).includes('SYNTHETIC_SECRET'),false);
    }
  });
}
test('raw JavaScript lone surrogates cannot become replacement-character UTF8 frames',()=>{
  for(const value of ['\ud800','\udc00']) {
    const observer=new RuntimeContractObserver(fixture());
    const header=JSON.stringify({...observer.snapshot().scope,version:{major:1,minor:1},sequence:1,type:'future.event'});
    assert.throws(()=>observer.accept(header.slice(0,-1)+',"ignored":"'+value+'"}'),isProtocol);
  }
});
test('timestamp observations and rejections are independent of the host timezone',()=>{
  const moduleUrl=new URL('../src/runtime-contract.mjs',import.meta.url).href;
  const script=`import {normalizeEffectiveConfiguration} from ${JSON.stringify(moduleUrl)};
    let text='';for await(const chunk of process.stdin)text+=chunk;
    const result=JSON.parse(text).map(({input})=>{try{return normalizeEffectiveConfiguration({reasoning:{state:'known',value:'low',evidence:{source:'synthetic',observedAt:input}}}).reasoning.evidence.observedAt;}catch(error){return error.code;}});
    process.stdout.write(JSON.stringify(result));`;
  const expected=vectors.timestamps.map(vector=>vector.output??'protocol');
  for(const TZ of ['UTC','America/Sao_Paulo','Asia/Tokyo']) {
    const result=spawnSync(process.execPath,['--input-type=module','-e',script],{input:JSON.stringify(vectors.timestamps),env:{...process.env,TZ},encoding:'utf8',windowsHide:true,timeout:5000});
    assert.equal(result.error,undefined);assert.equal(result.status,0);assert.deepEqual(JSON.parse(result.stdout),expected,TZ);
  }
});
