#!/usr/bin/env node
// Versioned offline inputs/expectations for the Rust Core, using the JS wire oracle.
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {dirname,join,resolve} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {RUNTIME_CONTRACT_VERSION,RuntimeContractObserver,normalizeRuntimeDescriptor,normalizeRuntimeRequest,normalizeEffectiveConfiguration,capabilityDecision,assertRuntimeAdmission} from './src/runtime-contract.mjs';
import {createFakeRuntime,fakeRuntimeRequest} from './fixtures/fake-runtime.mjs';
import {replayCodexFixture} from './codex-replay.mjs';

const root=dirname(fileURLToPath(import.meta.url));
const target=join(root,'../../crates/orchestrix-core/tests/fixtures/runtime-conformance-v1.json');
const copy=value=>JSON.parse(JSON.stringify(value));
const proof={source:'synthetic',scope:'attempt',reference:'conformance-proof',observedAt:'2026-10-13T00:00:00.000Z'};
const configured=(fields={model:'model-1',reasoning:'low',sandbox:'workspace-write',network:'disabled',approval:'never'})=>Object.fromEntries(Object.entries(fields).map(([key,value])=>[key,{state:'known',value,evidence:proof}]));
const outcome=operation=>{try{return {ok:true,value:operation()};}catch(error){return {ok:false,code:error.code};}};

export async function buildConformanceCorpus({lineEndings='as-is'}={}) {
  if(!['as-is','lf','crlf'].includes(lineEndings))throw new Error('Invalid test input line endings');
  const readInput=async file=>{
    let text=await readFile(file,'utf8');
    if(lineEndings!=='as-is')text=text.replace(/\r?\n/g,lineEndings==='lf'?'\n':'\r\n');
    return Buffer.from(text.replace(/\r\n/g,'\n'),'utf8');
  };
  const descriptors={base:copy(createFakeRuntime().descriptor)},requests={base:copy(fakeRuntimeRequest())},cases=[];
  function add(name,{descriptor='base',request='base',limits,actions=[]}={}) {
    if(cases.some(item=>item.name===name))throw new Error('Duplicate conformance case');
    const d=typeof descriptor==='string'?descriptors[descriptor]:descriptor,r=typeof request==='string'?requests[request]:request;
    let observer;const constructor=outcome(()=>{observer=new RuntimeContractObserver({descriptor:d,request:r,...limits});return {descriptor:observer.descriptor,request:observer.request,snapshot:observer.snapshot()};});
    const steps=[];
    if(observer)for(const action of actions) {
      let value=copy(action);
      if(action.type){const {origin='runtime',version=RUNTIME_CONTRACT_VERSION,type,fields={},sequence,...rest}=action;value={op:'accept',origin,frame:JSON.stringify({...observer.snapshot().scope,version,sequence:sequence??observer.sequence+1,type,...fields,...rest})};}
      const expected=outcome(()=>{
        if(value.op==='accept')return observer.accept(value.frame,{origin:value.origin??'runtime'});
        if(value.op==='close')return observer.close();
        if(value.op==='transport-lost')return observer.transportLost(value.reason);
        if(value.op==='admission')return assertRuntimeAdmission(observer.descriptor,observer.request,{effective:value.effective??observer.snapshot().effective});
        if(value.op==='capability')return capabilityDecision(observer.descriptor,value.name,{modelId:value.modelId});
        if(value.op==='effective')return normalizeEffectiveConfiguration(value.value);
        throw new Error('Unknown conformance operation');
      });
      steps.push({action:value,expected,snapshot:observer.snapshot()});
    }
    cases.push({name,descriptor,request,...(limits?{limits}:{}),constructor,steps});
  }
  for(const scenario of ['success','event-invalid','auth-invalid','rate-limit','interruption','timeout','crash']) {
    const fake=createFakeRuntime({scenario});descriptors[`fake-${scenario}`]=copy(fake.descriptor);
    const actions=[];for await(const envelope of fake.run())actions.push({op:'accept',...envelope});
    add(`fake-${scenario}`,{descriptor:`fake-${scenario}`,actions});
  }
  const references={},sourceHashes=[];
  for(const [name,file] of Object.entries({'job-live':'codex-m0-job-live.json','mxc-live':'codex-m0-mxc-live.json'})) {
    const bytes=await readInput(join(root,'../../docs/research/evidence',file));references[name]=JSON.parse(bytes.toString('utf8'));sourceHashes.push({reference:name,sha256:createHash('sha256').update(bytes).digest('hex')});
  }
  const fixtureBytes=await readInput(join(root,'fixtures/codex-contract-replay.json'));
  sourceHashes.push({reference:'portable-codex-fixture',sha256:createHash('sha256').update(fixtureBytes).digest('hex')});
  const replay=replayCodexFixture(JSON.parse(fixtureBytes.toString('utf8')),references);
  for(const item of replay.cases)for(const {descriptor,snapshot} of item.replay.attempts) {
    const name=`codex-${item.name}-${snapshot.scope.attemptId}`;
    descriptors[name]=descriptor;requests[name]={version:snapshot.version,...snapshot.scope,requested:snapshot.requested};
    add(name,{descriptor:name,request:name,actions:snapshot.events.map(event=>({op:'accept',origin:event.origin,frame:JSON.stringify(event)}))});
  }
  descriptors.unknown=copy(descriptors.base);descriptors.unknown.catalog.completeness='unknown';
  Object.assign(descriptors.unknown.models[0],{availability:'unknown',reasoningLevels:null,capabilities:{}});
  const accepted={type:'attempt.accepted'},started={type:'attempt.started'},complete={type:'attempt.terminal',fields:{outcome:'completed'}};
  const startObserved={type:'attempt.start-observed'},terminal=(outcome='completed')=>({type:'attempt.terminal-observed',fields:{outcome}});
  const processExit={type:'process.exited',origin:'supervisor',fields:{role:'runtime',code:137,signal:'killed'}};
  const tree={type:'process.tree-observed',origin:'supervisor',fields:{ownership:'confirmed',source:'owned-job',termination:'confirmed',activeCount:0,evidenceRef:'owned-proof'}};
  add('factual-terminal-before-ack',{descriptor:'unknown',actions:[terminal(),{type:'attempt.admission-observed'}]});
  add('factual-start-before-ack',{descriptor:'unknown',actions:[startObserved,{type:'attempt.admission-observed'},terminal()]});
  add('factual-configuration-mismatch-is-observation',{descriptor:'unknown',actions:[{type:'configuration.observation-recorded',fields:{effective:configured({model:'model-1',reasoning:'high',sandbox:'read-only'})}},terminal()]});
  add('legacy-preflight-not-entitlement',{descriptor:'unknown',actions:[accepted,processExit,tree]});
  add('late-interrupt-ack-without-send',{actions:[terminal('interrupted'),{type:'attempt.interrupt-result',fields:{acknowledged:true}}]});
  add('interrupt-terminal-before-ack',{actions:[startObserved,{type:'attempt.interrupt-requested'},terminal('interrupted'),{type:'attempt.interrupt-result',fields:{acknowledged:true}}]});
  add('interrupt-ack-cannot-erase',{actions:[{type:'attempt.interrupt-result',fields:{acknowledged:true}},{type:'attempt.interrupt-result',fields:{acknowledged:false}}]});
  add('legacy-success',{actions:[{type:'configuration.observed',fields:{effective:configured()}},accepted,started,complete]});
  for(const [key,value] of [['sandbox','read-only'],['network','enabled'],['approval','ask'],['reasoning','high']])add(`legacy-config-mismatch-${key}`,{actions:[{type:'configuration.observed',fields:{effective:configured({[key]:value})}},processExit,tree]});
  for(const field of ['connectionId','sessionId','attemptId','turnId','connectionRevision'])add(`foreign-${field}`,{actions:[accepted,started,{...complete,fields:{outcome:'completed',[field]:field==='connectionRevision'?2:'foreign'}},processExit,tree]});
  add('duplicate-sequence',{actions:[accepted,{...started,sequence:1},processExit,tree]});
  add('sequence-gap',{actions:[accepted,{...started,sequence:3},processExit,tree]});
  for(const type of ['attempt.started','attempt.terminal','attempt.interrupt-acknowledged'])add(`legacy-invalid-${type.replaceAll('.','-')}`,{actions:[{type,fields:{outcome:'completed'}},processExit,tree]});
  add('malformed-json-cleanup',{actions:[accepted,started,{op:'accept',frame:'{"broken":"SYNTHETIC_SECRET',origin:'runtime'},processExit,tree,complete]});
  add('foreign-supervisor-in-json',{actions:[startObserved,{type:'process.exited',fields:{role:'runtime',code:0,origin:'supervisor'}},processExit,tree]});
  add('supervisor-cannot-create-terminal',{actions:[startObserved,processExit,tree,{...terminal(),origin:'supervisor'}]});
  add('forced-stop-no-terminal',{actions:[startObserved,processExit,tree,{type:'process.exited',origin:'supervisor',fields:{role:'helper',code:0}}]});
  add('root-helper-streams-not-tree',{actions:[accepted,{type:'process.exited',origin:'supervisor',fields:{role:'helper',code:0}},processExit,{type:'process.streams-closed',origin:'supervisor',fields:{role:'runtime',streams:{stdin:true,stdout:true,stderr:true,faulted:false}}}]});
  add('monotonic-streams-fault',{actions:[processExit,...[{stdin:true,stdout:false,stderr:true,faulted:true},{stdin:false,stdout:true,stderr:false,faulted:false}].map(streams=>({type:'process.streams-observed',origin:'supervisor',fields:{role:'runtime',streams}})),tree]});
  add('legacy-streams-duplicate',{actions:[...Array.from({length:2},()=>({type:'process.streams-closed',origin:'supervisor',fields:{role:'runtime',streams:{stdin:true,stdout:true,stderr:true,faulted:false}}}))]});
  for(const code of ['protocol','ownership','sequence','version','budget'])add(`runtime-fatal-${code}`,{actions:[startObserved,{type:'runtime.error',fields:{fatal:true,error:{code,phase:'running',dispatch:'sent',message:'SYNTHETIC_SECRET'}}},processExit,tree,terminal()]});
  for(const reason of ['timeout','transport-loss','crash'])add(`loss-reconcile-${reason}`,{actions:[accepted,started,{op:'transport-lost',reason},{type:'attempt.terminal',fields:{outcome:'failed',error:{code:reason,phase:'running',dispatch:'unknown'}}}]});
  add('close-irreversible',{actions:[accepted,started,{op:'close'},complete,{op:'transport-lost',reason:'timeout'},processExit]});
  add('terminal-retained-after-loss',{actions:[accepted,started,complete,{op:'transport-lost',reason:'crash'},terminal('failed')]});
  add('known-error-only-whitelist',{actions:[{type:'runtime.error',fields:{error:{code:'authentication',phase:'connect',dispatch:'not-sent',message:'SYNTHETIC_SECRET',native:{key:'SYNTHETIC_SECRET'}}}}]});
  add('future-fields-sanitized',{actions:[{type:'SYNTHETIC_SECRET',version:{major:1,minor:9},fields:{nativeThread:'SYNTHETIC_SECRET',payload:{credentials:'SYNTHETIC_SECRET'}}}]});
  add('major-event-incompatible',{actions:[{...terminal(),version:{major:2,minor:0}},processExit,tree]});
  for(const type of ['configuration.observation-recorded', 'attempt.admission-observed', 'attempt.start-observed', 'attempt.terminal-observed', 'attempt.interrupt-result','output.observed','process.streams-observed'])add(`v10-unknown-${type.replaceAll('.','-')}`,{actions:[{type,version:{major:1,minor:0},origin:type.startsWith('process.')?'supervisor':'runtime',fields:{outcome:'completed',acknowledged:true,channel:'assistant',bytes:2,role:'runtime',streams:{stdin:true,stdout:true,stderr:true,faulted:false}}}]});
  for(const limits of [{maxFrameBytes:1},{maxTotalBytes:1},{maxEvents:1}])add(`budget-${Object.keys(limits)[0]}`,{limits,actions:[accepted,started,processExit]});
  for(const limits of [{maxEvents:0},{maxFrameBytes:1048577},{maxTotalBytes:16777217}])add(`invalid-budget-${Object.keys(limits)[0]}`,{limits});
  add('output-safe-integer-overflow',{actions:[{type:'output.observed',fields:{channel:'assistant',bytes:Number.MAX_SAFE_INTEGER}},{type:'output.observed',fields:{channel:'tool',bytes:1}},processExit,tree]});
  add('output-counters-secret-fields',{actions:[...['assistant','tool','other'].map(channel=>({type:'output.observed',fields:{channel,bytes:12,text:'SYNTHETIC_SECRET',command:'SYNTHETIC_SECRET'}})),terminal()]});
  const unicodeFrame=JSON.stringify({...requests.base,sequence:1,type:'future.event',ignored:'é'.repeat(20)});
  add('utf8-byte-boundary',{limits:{maxFrameBytes:unicodeFrame.length},actions:[{op:'accept',origin:'runtime',frame:unicodeFrame}]});
  for(const [field,value] of [['authentication','unknown'],['authentication','unauthenticated'],['availability','unknown'],['availability','unavailable'],['billing','api']]){const descriptor=copy(descriptors.base);descriptor.connection[field]=value;add(`preflight-${field}-${value}`,{descriptor,actions:[{op:'admission'},accepted]});}
  for(const levels of [null,[],['high']]){const descriptor=copy(descriptors.base);descriptor.models[0].reasoningLevels=levels;add(`preflight-levels-${levels===null?'null':levels.join('-')||'empty'}`,{descriptor,actions:[{op:'admission'},accepted]});}
  add('explicit-api-consent',{descriptor:{...copy(descriptors.base),connection:{...descriptors.base.connection,billing:'api'}},request:{...requests.base,billingMode:'explicit-api',billingAuthorizationRef:'consent-1'},actions:[{op:'admission'},accepted]});
  for(const [name,change] of [['paid-fallback',r=>r.paidFallback='allowed'],['api-without-consent',r=>r.billingMode='explicit-api'],['request-major',r=>r.version.major=2],['request-revision',r=>r.connectionRevision=2],['bad-request-id',r=>r.sessionId='path/private']]){const request=copy(requests.base);change(request);add(name,{request});}
  add('descriptor-major',{descriptor:{...copy(descriptors.base),version:{major:2,minor:0}}});
  add('duplicate-model',{descriptor:{...copy(descriptors.base),models:[descriptors.base.models[0],descriptors.base.models[0]]}});
  const union=copy(descriptors.base);union.models.push({id:'model-2',availability:'available',capabilities:{}});
  add('capabilities-not-union',{descriptor:union,actions:[{op:'capability',name:'reasoning'},{op:'capability',name:'reasoning',modelId:'model-2'},{op:'capability',name:'quota',modelId:'missing'},{op:'capability',name:'not-a-capability'}]});
  for(const support of ['unsupported','unknown','runtime-managed']){const descriptor=copy(descriptors.base);descriptor.capabilities.resume.support=support;add(`optional-${support}`,{descriptor,actions:[{op:'capability',name:'resume'}]});}
  add('effective-unknown-not-requested',{actions:[{type:'configuration.observed',fields:{effective:{}}},accepted,started,complete]});
  add('effective-value-needs-evidence',{actions:[{op:'effective',value:{reasoning:{state:'known',value:'high'}}},{op:'effective',value:{reasoning:{state:'unknown',value:'low'}}}]});
  add('date-normalized-utc',{actions:[{op:'effective',value:{reasoning:{state:'known',value:'low',evidence:{...proof,observedAt:'2026-10-12T21:00:00-03:00'}}}}]});
  add('floating-integral-json-numbers',{actions:[{op:'accept',origin:'runtime',frame:'{"version":{"major":1.0,"minor":1.0},"connectionId":"connection-1","sessionId":"session-1","attemptId":"attempt-1","connectionRevision":1.0,"sequence":1.0,"type":"attempt.admission-observed"}'}]});
  let deep={private:'SYNTHETIC_SECRET'};for(let depth=0;depth<500;depth++)deep={future:deep};
  add('deep-compatible-unknown-field',{actions:[{type:'future.event',fields:{payload:deep}},accepted,started,complete]});
  add('deep-compatible-evidence-field',{actions:[{type:'configuration.observation-recorded',fields:{effective:{model:{state:'known',value:'model-1',evidence:{...proof,native:deep}}}}},accepted,started,complete]});
  add('deep-invalid-known-field',{actions:[{type:'output.observed',fields:{channel:'assistant',bytes:deep}},processExit,tree]});
  const brokenDeep=JSON.stringify({...requests.base,sequence:1,type:'future.event',payload:deep}).slice(0,-1);
  add('deep-malformed-unknown-field',{actions:[{op:'accept',origin:'runtime',frame:brokenDeep},processExit,tree]});
  add('timestamp-leap-second-invalid',{actions:[{op:'effective',value:{reasoning:{state:'known',value:'low',evidence:{...proof,observedAt:'2026-10-13T00:00:60Z'}}}}]});
  return {schemaVersion:1,contractVersion:RUNTIME_CONTRACT_VERSION,validationKind:'offline-conformance',hashMode:'lf-normalized-input-bytes',sourceHashes,descriptors,requests,cases};
}

export function serializeConformanceCorpus(corpus) {
  // One case per line makes future golden-data diffs attributable to a named case.
  const {cases,...header}=corpus;
  return JSON.stringify(header,null,2).slice(0,-2)+',\n  "cases": [\n'+cases.map(item=>'    '+JSON.stringify(item)).join(',\n')+'\n  ]\n}\n';
}
async function main(){const args=process.argv.slice(2);if(args.some(arg=>arg!=='--check'))throw new Error('Only --check is supported');const corpus=await buildConformanceCorpus(),text=serializeConformanceCorpus(corpus);if(args.includes('--check')){if(await readFile(target,'utf8')!==text)throw new Error('Conformance corpus is stale');}else{await mkdir(dirname(target),{recursive:true});await writeFile(target,text);}process.stdout.write(JSON.stringify({status:'passed',cases:corpus.cases.length,validationKind:corpus.validationKind,providerCalls:0})+'\n');}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href)main().catch(()=>{process.stderr.write('Conformance corpus generation failed; no raw evidence printed.\n');process.exitCode=1;});
