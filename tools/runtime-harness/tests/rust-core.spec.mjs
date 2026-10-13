import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {runProbe} from '../probe.mjs';
import {createFakeRuntime,fakeRuntimeRequest} from '../fixtures/fake-runtime.mjs';
import {RuntimeContractObserver,RUNTIME_INPUT_PROFILE} from '../src/runtime-contract.mjs';
import {buildConformanceCorpus,serializeConformanceCorpus} from '../export-conformance.mjs';

const root=join(dirname(fileURLToPath(import.meta.url)),'../../..');
const binary=join(root,'target/debug/examples',process.platform==='win32'?'runtime_observe.exe':'runtime_observe');
function consume(descriptor,request,frames,inputProfile=RUNTIME_INPUT_PROFILE) {
  return consumeBundle(JSON.stringify({inputProfile,descriptor,request,frames}));
}
function consumeBundle(input) {
  const result=spawnSync(binary,[],{input,encoding:'utf8',windowsHide:true,timeout:5000,maxBuffer:4*1024*1024});
  assert.equal(result.error,undefined,'Build runtime_observe before running this opt-in suite');
  assert.equal(result.signal,null);return {exitCode:result.status,...JSON.parse(result.stdout)};
}
for(const [scenario,outcome] of [['success','completed'],['hang','interrupted'],['rate-limit','unknown'],['malformed','unknown']]) {
  test(`CodexSession/adapter ${scenario} normalized output is consumed by the Rust Core unchanged`,async()=>{
    const report=await runProbe({mode:'fake',scenario,requestTimeoutMs:2000,turnTimeoutMs:2000});
    assert.equal(report.liveStarted,false);assert.equal(report.workspace.filesUnchanged,true);
    const {descriptor,attempts}=report.normalized,snapshot=attempts[0];
    const request={version:snapshot.version,...snapshot.scope,requested:snapshot.requested};
    const frames=snapshot.events.map(event=>({origin:event.origin,frame:JSON.stringify(event)}));
    const expected=new RuntimeContractObserver({descriptor,request});for(const envelope of frames)expected.accept(envelope.frame,{origin:envelope.origin});
    const actual=consume(descriptor,request,frames);
    assert.equal(actual.exitCode,0);assert.equal(actual.status,'observed');assert.equal(actual.snapshot.outcome,outcome);
    assert.deepEqual(actual.snapshot,expected.snapshot());assert.deepEqual(actual.descriptor,descriptor);
    assert.equal(actual.snapshot.process.runtime.exited,true);
    assert.equal(JSON.stringify(actual).includes('turn-fake-001'),false);
  });
}
test('provider-neutral fake uses the same Rust consumer without Codex messages or SDKs',async()=>{
  const fake=createFakeRuntime(),request=fakeRuntimeRequest(),frames=[];
  for await(const envelope of fake.run(request))frames.push(envelope);
  const actual=consume(fake.descriptor,request,frames);
  assert.equal(actual.exitCode,0);assert.equal(actual.snapshot.outcome,'completed');assert.equal(actual.snapshot.process.tree.termination,'confirmed');
  assert.deepEqual(actual.snapshot.contextOwnership,{conversation:'runtime',projectMemory:'orchestrix'});
});
test('the Rust executable refuses forged provenance, revision changes and secret-bearing malformed input',()=>{
  const fake=createFakeRuntime(),request=fakeRuntimeRequest();
  for(const [fields,origin,code] of [[{type:'process.exited',role:'runtime',code:0,origin:'supervisor'},'runtime','ownership'],[{type:'attempt.admission-observed',connectionRevision:2},'runtime','ownership'],[{type:'attempt.admission-observed'},'supervisor','ownership']]) {
    const frame=JSON.stringify({...request,sequence:1,...fields});
    const actual=consume(fake.descriptor,request,[{origin,frame}]);
    assert.equal(actual.exitCode,1);assert.equal(actual.code,code);assert.equal(actual.snapshot.outcome,'unknown');assert.equal(actual.snapshot.fatal,true);
  }
  const actual=consume(fake.descriptor,request,[{origin:'runtime',frame:'{"secret":"SYNTHETIC_PRIVATE_CREDENTIAL'}]);
  assert.equal(actual.code,'protocol');assert.equal(JSON.stringify(actual).includes('SYNTHETIC_PRIVATE_CREDENTIAL'),false);
});
test('portable conformance corpus is reproducible with the same recorded inputs and zero provider dispatch',async()=>{
  const corpus=await buildConformanceCorpus();assert.equal(corpus.validationKind,'offline-conformance');
  assert.equal(corpus.cases.length,245);assert.equal(corpus.sourceHashes.length,4);assert.deepEqual(corpus.inputProfile,RUNTIME_INPUT_PROFILE);
});

test('CRLF and LF source checkouts produce identical portable data and explicitly canonical hashes',async()=>{
  const lf=await buildConformanceCorpus({lineEndings:'lf'}),crlf=await buildConformanceCorpus({lineEndings:'crlf'});
  assert.equal(lf.hashMode,'lf-normalized-input-bytes');assert.deepEqual(lf.sourceHashes,crlf.sourceHashes);
  assert.equal(serializeConformanceCorpus(lf),serializeConformanceCorpus(crlf));
});

test('timezone-free legacy timestamps now reject identically without becoming execution evidence',()=>{
  const fake=createFakeRuntime(),request=fakeRuntimeRequest();
  const frame=JSON.stringify({...request,sequence:1,type:'configuration.observation-recorded',effective:{reasoning:{state:'known',value:'low',evidence:{source:'configuration',scope:'attempt',reference:'legacy-date',observedAt:'2026-10-13T00:00:00'}}}});
  const reference=new RuntimeContractObserver({descriptor:fake.descriptor,request});assert.throws(()=>reference.accept(frame),error=>error.code==='protocol');
  const actual=consume(fake.descriptor,request,[{origin:'runtime',frame}]);
  assert.equal(actual.exitCode,1);assert.equal(actual.code,'protocol');assert.equal(actual.snapshot.outcome,'unknown');
  assert.equal(actual.snapshot.effective.reasoning.state,'unknown');
  assert.deepEqual(actual.snapshot,reference.snapshot());
});

test('the Rust bridge requires a supported host input profile and cannot silently downgrade',()=>{
  const fake=createFakeRuntime(),request=fakeRuntimeRequest();
  for(const profile of [null,{id:'legacy-date-parse',revision:1},{...RUNTIME_INPUT_PROFILE,revision:2}]) {
    const actual=consume(fake.descriptor,request,[],profile);assert.equal(actual.code,'version');assert.equal(actual.exitCode,1);assert.equal(actual.snapshot,undefined);
  }
  const actual=consume(fake.descriptor,request,[]);assert.deepEqual(actual.inputProfile,RUNTIME_INPUT_PROFILE);
});

test('the bridge accepts integral JSON profile revision 1.0 but refuses an omitted profile',()=>{
  const fake=createFakeRuntime(),request=fakeRuntimeRequest();
  const bundle=JSON.stringify({inputProfile:RUNTIME_INPUT_PROFILE,descriptor:fake.descriptor,request,frames:[]});
  assert.equal(consumeBundle(bundle.replace('"revision":1','"revision":1.0')).status,'observed');
  assert.equal(consumeBundle(JSON.stringify({descriptor:fake.descriptor,request,frames:[]})).code,'version');
});
