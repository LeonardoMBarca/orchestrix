import test from 'node:test';
import assert from 'node:assert/strict';
import {selectableConnection,diagnosticRunning,diagnosticStillCurrent,reportedModels,connectionResourceFacts,diagnosticNextAction,diagnosticLabel,connectionError} from '../src/connection-state.ts';
import {connectionTranslations} from '../src/connection-locales.ts';
const connection={id:'connection-a',revision:3,bindingRevision:2,name:'My account',label:'My account',provider:'Codex',mode:'subscription',status:'observed',enabled:true,linked:true};
const report={latest:{id:'diagnostic-a',state:'partial',connectionRevision:2,current:true,observedAtMs:10,validUntilMs:200,complete:false},catalog:{runtimeVersion:'0.162.0-alpha.17.2',connection:{id:'connection-a',revision:2,authentication:'authenticated',billing:'subscription'},models:[{id:'model-opaque',availability:'unknown',reasoningLevels:['low','maximum']}],capabilities:{contextManagement:{support:'runtime-managed'}}},catalogLabels:[{id:'model-opaque',label:'Actual provider model',nativeEfforts:['low','xhigh']}],controlObservations:['control:native-delegation-disabled']};
test('selection excludes disabled or unlinked connections without fabricating old mock facts',()=>{
 assert.equal(selectableConnection(connection),true);
 assert.equal(selectableConnection({...connection,enabled:false}),false);
 assert.equal(selectableConnection({...connection,linked:false}),false);
 assert.equal(selectableConnection({id:'legacy-renderer-fixture'}),true);
});
test('late diagnostic replies cannot cross identity, registration revision or selection generation',()=>{
 assert.equal(diagnosticStillCurrent('a',2,{id:'a',revision:2},4,4),true);
 for(const selected of [null,{id:'b',revision:2},{id:'a',revision:3}])assert.equal(diagnosticStillCurrent('a',2,selected,4,4),false);
 assert.equal(diagnosticStillCurrent('a',2,{id:'a',revision:2},4,5),false);
});
test('reported model labels and native effort are display facts separate from normalized reasoning choices',()=>{
 assert.deepEqual(reportedModels(report,connection),[{id:'model-opaque',name:'Actual provider model',efforts:['low','maximum'],nativeEfforts:['low','xhigh']}]);
 assert.deepEqual(reportedModels(report,{...connection,id:'other'}),[]);
 assert.deepEqual(reportedModels(report,{...connection,bindingRevision:4}),[]);
 assert.equal(reportedModels(report,{...connection,revision:4})[0].name,'Actual provider model','rename keeps binding/catalog identity');
});
test('partial catalog preserves observed positives and never supplies unknown speed, quota or entitlement',()=>{
 const facts=connectionResourceFacts(report);
 assert.equal(facts.find(f=>f.id==='models').state,'reported');
 assert.equal(facts.find(f=>f.id==='authentication').state,'reported');
 assert.equal(facts.find(f=>f.id==='context').state,'declared');
 assert.equal(facts.find(f=>f.id==='permissions').state,'restricted');
 for(const id of ['usage','speed','structured-output'])assert.equal(facts.find(f=>f.id===id).state,'unknown');
 assert.match(facts.find(f=>f.id==='models').description,/does not confirm execution access/);
 assert.match(diagnosticNextAction(report,50),/Some catalog data/);
 assert.match(diagnosticNextAction(report,200),/stale/);
});
test('absent facts remain unknown rather than a failed account score',()=>{
 assert.equal(connectionResourceFacts({}).every(f=>f.state==='unknown'),true);
 assert.match(diagnosticNextAction({}),/Link a runtime/);
 assert.equal(diagnosticRunning({latest:{state:'running'}}),true);
 assert.equal(diagnosticRunning({latest:{state:'failed'}}),false);
});
test('unsupported structured output is separate from configured policy restrictions and duplicate-home errors are actionable',()=>{
 const facts=connectionResourceFacts({...report,structuredResultSupport:false});
 assert.equal(facts.find(f=>f.id==='structured-output').state,'unsupported');
 assert.equal(facts.find(f=>f.id==='permissions').state,'restricted');
 assert.match(connectionError('auth-store-already-linked'),/Use the existing connection/);
 assert.match(connectionError('connection-binding-revision-changed'),/Select it again/);
 assert.equal(connectionError('unrecognized-specific-code'),'unrecognized-specific-code');
});
test('authentication, policy, timeout, cancellation and backoff offer different next actions',()=>{
 const action=(state,errorCode)=>diagnosticNextAction({latest:{state,errorCode}},50);
 assert.match(action('failed','authentication'),/official Codex flow/);
 assert.match(action('failed','policy'),/paid API fallback is not allowed/);
 assert.match(action('failed','timeout'),/Historical evidence is preserved/);
 assert.match(action('cancelled'),/No capability was marked unsupported/);
 assert.match(action('backoff'),/does not bypass limits/);
 assert.match(action('coalesced'),/already being observed/);
 assert.match(diagnosticNextAction({...report,identityChanged:true},50),/explicitly confirm/);
});
test('new diagnostic states and capability descriptions have Portuguese and Spanish labels',()=>{
 for(const state of ['running','complete','partial','failed','cancelled','interrupted','backoff','coalesced','runtime-metadata','official-read-only-metadata','synthetic','observed-label']){
  const label=diagnosticLabel(state);assert.ok(connectionTranslations[label]?.every(s=>s.length>0),label);
 }
 for(const fact of connectionResourceFacts(report)){
  assert.ok(connectionTranslations[fact.label],fact.label);
  assert.ok(connectionTranslations[fact.description],fact.description);
 }
});
