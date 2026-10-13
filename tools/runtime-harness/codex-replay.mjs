#!/usr/bin/env node
// Offline evidence replay only. No Codex process, transport, login or inference.
import {EventEmitter} from 'node:events';
import {createHash} from 'node:crypto';
import {readFile,stat,mkdir,writeFile} from 'node:fs/promises';
import {dirname,join,resolve,basename} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {CodexIdentityMap,CodexRuntimeAdapter} from './src/codex-contract.mjs';
import {RuntimeContractError,RuntimeContractObserver} from './src/runtime-contract.mjs';

const root=dirname(fileURLToPath(import.meta.url));
const record=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const reject=code=>{throw new RuntimeContractError(code);};
const integer=(value,max)=>Number.isSafeInteger(value)&&value>=0&&value<=max;
const alias=(value,role)=>typeof value==='string'&&new RegExp(`^${role}-[1-9][0-9]{0,5}$`).test(value);
const references=Object.freeze({
  'job-live':join(root,'../../docs/research/evidence/codex-m0-job-live.json'),
  'mxc-live':join(root,'../../docs/research/evidence/codex-m0-mxc-live.json'),
});
const attemptCreators=new Set(['attempt.turn.accepted','attempt.turn.started','attempt.turn.ended','response.delta']);
const attemptEvents=new Set([...attemptCreators,'attempt.interrupt.sent','attempt.interrupt.requested','attempt.observation.unknown']);

function configuration(value) {
  if(value===undefined||value===null)return {};
  if(!record(value))reject('protocol');
  const selected={};
  for(const side of ['requested','resolvedConfiguration']) {
    if(value[side]===undefined||value[side]===null)continue;
    if(!record(value[side]))reject('protocol');
    selected[side]=Object.fromEntries(['model','effort','sandbox','networkAccess','approval'].filter(key=>Object.hasOwn(value[side],key)).map(key=>[key,value[side][key]]));
  }
  return selected;
}

function legacyEvent(input) {
  if(!record(input)||typeof input.type!=='string'||!/^[-A-Za-z0-9_.:]{1,120}$/.test(input.type))reject('protocol');
  for(const key of ['threadId','turnId','requestId'])if(input[key]!=null&&!alias(input[key],key))reject('ownership');
  return Object.fromEntries(['type','threadId','turnId','bytes','status','errorKind','reason',...(input.dispatch==='not-written'?['dispatch']:[])].filter(key=>Object.hasOwn(input,key)).map(key=>[key,input[key]]));
}

function replayEvidence(pins,sourceKind) {
  const provided=pins.requested??{},fields=Object.keys(provided),defaults=['strategy','presetRef'];
  if(typeof provided.model!=='string'||!provided.model.length)defaults.push('modelPolicy','modelId');
  if(!['minimal','low','medium','high','maximum'].includes(provided.effort))defaults.push('reasoning');
  if(!['read-only','workspace-write'].includes(provided.sandbox))defaults.push('sandbox');
  if(typeof provided.networkAccess!=='boolean')defaults.push('network');
  if(provided.approval!=='never')defaults.push('approval');
  return {intentSource:fields.length?`${sourceKind}-configuration`:'contract-defaults',requestedConfigurationFields:fields,defaultedIntentFields:defaults,defaultsAreHistoricalEvidence:false,accountBinding:'not-recorded'};
}

/** Same neutral consumer used for fake/runtime envelopes; no native event parser. */
export function consumeCodexReplayAttempt({descriptor,snapshot}) {
  const request={version:snapshot.version,...snapshot.scope,requested:snapshot.requested};
  const observer=new RuntimeContractObserver({descriptor,request});
  for(const event of snapshot.events)observer.accept(JSON.stringify(event),{origin:event.origin});
  return observer.snapshot();
}

/** Process observations require explicit synthetic assignments, never array order. */
export function replayCodexReport(report,{sourceKind='recorded',processAssignments=[]}={}) {
  if(!record(report)||!['recorded','synthetic'].includes(sourceKind)||!Array.isArray(report.events)||report.events.length>4096||!Array.isArray(processAssignments)||processAssignments.length>64)reject('protocol');
  if(sourceKind!=='synthetic'&&processAssignments.length)reject('ownership');
  const processes=report.processes??[];if(!Array.isArray(processes)||processes.length>64)reject('budget');
  const runtimeVersion=report.runtimeVersion??'unknown';
  if(typeof runtimeVersion!=='string'||!(/^[0-9]+\.[0-9]+\.[0-9]+[-A-Za-z0-9.]*$/.test(runtimeVersion)||['unknown','synthetic-replay'].includes(runtimeVersion)))reject('version');
  const connectionId='replay-connection-1',connectionRevision=1;
  const mappings=new CodexIdentityMap({connectionId,connectionRevision}),sessions=new Map(),attempts=new Map(),adapters=[];
  const auth=report.auth??{};
  const discovery={auth:{authenticated:typeof auth.authenticated==='boolean'?auth.authenticated:undefined,subscriptionGate:typeof auth.subscriptionGate==='boolean'?auth.subscriptionGate:undefined,type:['chatgpt','apiKey'].includes(auth.type)?auth.type:undefined},availability:typeof report.availability==='boolean'?report.availability:null,models:[],catalog:{completeness:'unknown'}};
  const usedConfigurations=new Set(),assignedProcesses=new Set();
  let segment=null,lastSequence=0,dispatches=0,unassignedEvents=0;
  const noDispatch=()=>{dispatches++;reject('policy');};
  try {
    for(const raw of report.events) {
      if(!integer(raw?.sequence,Number.MAX_SAFE_INTEGER)||raw.sequence<=lastSequence)reject('sequence');lastSequence=raw.sequence;
      const event=legacyEvent(raw);
      if(event.type==='session.started'||event.type==='session.resumed') {
        if(!alias(event.threadId,'threadId'))reject('ownership');
        if(!sessions.has(event.threadId)){if(sessions.size>=64)reject('budget');sessions.set(event.threadId,`session-${sessions.size+1}`);}
        const key=event.type==='session.started'?'configuration':'resumedConfiguration';
        // A singular report field proves only its first matching attachment.
        const pins=usedConfigurations.has(key)?{}:configuration(report[key]);usedConfigurations.add(key);
        segment={nativeId:event.threadId,localId:sessions.get(event.threadId),configuration:pins};
        continue;
      }
      if(!event.turnId||!attemptEvents.has(event.type)){unassignedEvents++;continue;}
      if(!segment)reject('ownership');
      if(event.threadId&&event.threadId!==segment.nativeId)reject('ownership');
      const attemptKey=`${segment.localId}:${event.turnId}`;
      let attempt=attempts.get(attemptKey);
      if(!attempt) {
        if(!attemptCreators.has(event.type)){unassignedEvents++;continue;}
        if(attempts.size>=64)reject('budget');
        const attemptId=`attempt-${attempts.size+1}`;
        const client=new EventEmitter();client.request=noDispatch;
        const session={threadId:segment.nativeId,configuration:segment.configuration,accountBinding:null,client,subscribe:()=>()=>{},startTurn:noDispatch,waitForTurn:noDispatch,interrupt:noDispatch};
        const adapter=new CodexRuntimeAdapter({session,discovery,runtimeVersion,connectionId,connectionRevision,sessionId:segment.localId,mappings,evidenceSource:sourceKind==='synthetic'?'synthetic':'configuration'});
        adapters.push(adapter);adapter.prepareReplayAttempt({attemptId,nativeTurnId:event.turnId,effort:segment.configuration.requested?.effort});
        attempt={attemptId,adapter,segment};attempts.set(attemptKey,attempt);
      } else if(attempt.segment!==segment)reject('ownership');
      attempt.adapter.replayEvent(event);
    }
    for(const attempt of attempts.values())attempt.adapter.flushOutput(attempt.attemptId);
    const assignments=new Set();
    for(const assignment of processAssignments) {
      if(!record(assignment)||typeof assignment.attemptId!=='string'||!integer(assignment.processIndex,processes.length-1))reject('ownership');
      const key=`${assignment.attemptId}:${assignment.processIndex}`;if(assignments.has(key))reject('ownership');
      const attempt=[...attempts.values()].find(value=>value.attemptId===assignment.attemptId);if(!attempt)reject('ownership');
      if(!record(processes[assignment.processIndex]))reject('protocol');
      // One adapter per attempt prevents this explicit association spreading
      // to unrelated turns attached to the same native app-server session.
      attempt.adapter.observeProcess(processes[assignment.processIndex]);
      assignments.add(key);assignedProcesses.add(assignment.processIndex);
    }
    const results=[...attempts.values()].map(attempt=>{
      const descriptor=attempt.adapter.descriptor,snapshot=attempt.adapter.snapshot(attempt.attemptId);
      const consumed=consumeCodexReplayAttempt({descriptor,snapshot});
      // Reconsumption includes normalized defaults in its byte accounting;
      // semantic state, rather than frame serialization byte count, is stable.
      const semantic=value=>{const {budget,...state}=value;return state;};
      if(JSON.stringify(semantic(snapshot))!==JSON.stringify(semantic(consumed)))reject('protocol');
      return {descriptor,snapshot,replayEvidence:replayEvidence(attempt.segment.configuration,sourceKind)};
    });
    return {schemaVersion:1,validationKind:'offline-replay',sourceKind,runtimeVersion,dispatches,sessionCount:sessions.size,attemptCount:results.length,unassignedLegacyEvents:unassignedEvents,assignedProcessObservations:assignedProcesses.size,unassignedProcessObservations:processes.length-assignedProcesses.size,attempts:results};
  } finally {for(const adapter of adapters)adapter.dispose();}
}

/** Stable projection is intentionally suitable for a future Rust fixture test. */
export function codexReplayFacts(result) {
  return {sessionCount:result.sessionCount,attemptCount:result.attemptCount,dispatches:result.dispatches,assignedProcessObservations:result.assignedProcessObservations,unassignedProcessObservations:result.unassignedProcessObservations,attempts:result.attempts.map(({descriptor,snapshot})=>({
    sessionId:snapshot.scope.sessionId,attemptId:snapshot.scope.attemptId,turnId:snapshot.scope.turnId,
    catalogCompleteness:descriptor.catalog.completeness,reasoningLevels:descriptor.models[0]?.reasoningLevels??null,
    admissionObserved:snapshot.admissionObserved,startObserved:snapshot.startObserved,outcome:snapshot.outcome,terminalConfirmed:snapshot.terminalConfirmed,interrupt:snapshot.interrupt,
    modelState:snapshot.effective.model.state,reasoningState:snapshot.effective.reasoning.state,
    runtimeExited:snapshot.process.runtime.exited,runtimeCode:snapshot.process.runtime.code,helperExited:snapshot.process.helper.exited,helperCode:snapshot.process.helper.code,treeTermination:snapshot.process.tree.termination,activeCount:snapshot.process.tree.activeCount,
  }))};
}

function expectedFacts(actual,expected) {
  if(Array.isArray(expected)) {
    if(!Array.isArray(actual)||actual.length!==expected.length)reject('protocol');
    expected.forEach((value,index)=>expectedFacts(actual[index],value));
  } else if(record(expected)) {
    if(!record(actual))reject('protocol');for(const [key,value] of Object.entries(expected)){if(!Object.hasOwn(actual,key))reject('protocol');expectedFacts(actual[key],value);}
  } else if(actual!==expected)reject('protocol');
}

export function replayCodexFixture(fixture,referenceReports={}) {
  if(!record(fixture)||fixture.schemaVersion!==1||!Array.isArray(fixture.cases)||fixture.cases.length>16)reject('protocol');
  const names=new Set();
  const cases=fixture.cases.map(item=>{
    if(!record(item)||typeof item.name!=='string'||!/^[-a-z0-9]{1,80}$/.test(item.name)||names.has(item.name)||!record(item.source))reject('protocol');names.add(item.name);
    const synthetic=item.source.kind==='synthetic';if(!synthetic&&item.source.kind!=='reference')reject('protocol');
    const report=synthetic?item.source.report:referenceReports[item.source.reference];if(!report)reject('evidence-missing');
    const replay=replayCodexReport(report,{sourceKind:synthetic?'synthetic':'recorded',processAssignments:item.processAssignments??[]});
    const facts=codexReplayFacts(replay);if(item.expected!==undefined)expectedFacts(facts,item.expected);
    return {name:item.name,facts,replay};
  });
  return {schemaVersion:1,validationKind:'offline-replay',status:'passed',dispatches:0,cases};
}

async function readInput(path) {
  if((await stat(path)).size>4*1024*1024)reject('budget');
  const bytes=await readFile(path);if(bytes.length>4*1024*1024)reject('budget');
  let value;try{value=JSON.parse(bytes.toString('utf8'));}catch{reject('protocol');}
  return {value,sha256:createHash('sha256').update(bytes).digest('hex')};
}

export async function runCodexReplay({output=join(root,'artifacts',`codex-replay-${Date.now()}.json`)}={}) {
  const fixture=await readInput(join(root,'fixtures/codex-contract-replay.json'));
  const reports={},sourceHashes=[{reference:'portable-fixture',sha256:fixture.sha256}];
  for(const [reference,path] of Object.entries(references)){const loaded=await readInput(path);reports[reference]=loaded.value;sourceHashes.push({reference,sha256:loaded.sha256});}
  const result={...replayCodexFixture(fixture.value,reports),sourceHashes};
  const target=resolve(output);await mkdir(dirname(target),{recursive:true});await writeFile(target,JSON.stringify(result,null,2)+'\n');
  return {result,output:target};
}

async function main() {
  let output;const args=process.argv.slice(2);
  for(let index=0;index<args.length;index++) {
    if(args[index]==='--output'&&args[index+1])output=args[++index];
    else if(args[index]==='--help'){process.stdout.write('node codex-replay.mjs [--output path]\nOffline replay of recorded app-server evidence and portable synthetic cases. No provider process or inference.\n');return;}
    else reject('policy');
  }
  const completed=await runCodexReplay({output});
  process.stdout.write(JSON.stringify({validationKind:'offline-replay',status:completed.result.status,cases:completed.result.cases.length,dispatches:0,artifact:basename(completed.output)})+'\n');
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href)main().catch(error=>{process.stderr.write(JSON.stringify({validationKind:'offline-replay',status:'failed',code:error instanceof RuntimeContractError?error.code:'local-error'})+'\n');process.exitCode=1;});
