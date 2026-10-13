// Provider-neutral in-memory fixture. No network, auth stores, subprocesses or timers.
import {RUNTIME_CONTRACT_VERSION,normalizeRuntimeDescriptor,normalizeRuntimeRequest,normalizeEffectiveConfiguration,assertRuntimeAdmission,RuntimeContractError} from '../src/runtime-contract.mjs';

const proof={source:'synthetic',scope:'connection',reference:'fixture-evidence',observedAt:'2026-10-13T00:00:00.000Z'};
const available={support:'supported',availability:'available',configuration:'enabled',evidence:proof};
export function fakeRuntimeRequest(overrides={}) {
  return normalizeRuntimeRequest({version:RUNTIME_CONTRACT_VERSION,connectionId:'connection-1',connectionRevision:1,sessionId:'session-1',attemptId:'attempt-1',billingMode:'subscription-only',contextPackRef:'context-pack-1',requested:{strategy:'balanced',modelPolicy:'specific',modelId:'model-1',reasoning:'low',sandbox:'workspace-write',network:'disabled',approval:'never'},...overrides});
}
export function createFakeRuntime({scenario='success',optionalUnsupported=false,effectiveUnknown=false}={}) {
  if(!['success','event-invalid','auth-invalid','rate-limit','interruption','timeout','crash'].includes(scenario))throw new RuntimeContractError('unsupported');
  const descriptor=normalizeRuntimeDescriptor({version:RUNTIME_CONTRACT_VERSION,adapterId:'fake-runtime',runtimeVersion:'1.0.0',connection:{id:'connection-1',revision:1,authentication:scenario==='auth-invalid'?'unauthenticated':'authenticated',billing:'subscription',availability:'available'},catalog:{completeness:'partial'},capabilities:{modelSelection:available,reasoning:available,resume:optionalUnsupported?{support:'unsupported',availability:'unknown',configuration:'unknown',evidence:proof}:available,quota:optionalUnsupported?undefined:available,contextManagement:{support:'runtime-managed',evidence:proof}},models:[{id:'model-1',availability:'available',reasoningLevels:['low'],capabilities:{modelSelection:available,reasoning:available,tools:available}}]});
  let active=false;
  return Object.freeze({descriptor,async *run(rawRequest=fakeRuntimeRequest()) {
    if(active)throw new RuntimeContractError('busy');active=true;
    try {
      const request=normalizeRuntimeRequest(rawRequest);let sequence=0;
      const frame=(type,fields={},origin='runtime')=>Object.freeze({origin,frame:JSON.stringify({version:RUNTIME_CONTRACT_VERSION,connectionId:request.connectionId,connectionRevision:request.connectionRevision,sessionId:request.sessionId,attemptId:request.attemptId,sequence:++sequence,type,...fields})});
      try{assertRuntimeAdmission(descriptor,request);}catch(error){yield frame('runtime.error',{error:{code:error.code,phase:'admission',dispatch:'not-sent'}});return;}
      if(scenario==='event-invalid'){yield {origin:'runtime',frame:'{"version":'};return;}
      const requested=request.requested;
      const effective=effectiveUnknown?{}:Object.fromEntries(Object.entries({model:requested.modelId??'model-1',reasoning:requested.reasoning==='auto'?'runtime-default':requested.reasoning,sandbox:requested.sandbox,network:requested.network,approval:requested.approval}).map(([key,value])=>[key,{state:'known',value,evidence:{...proof,scope:'attempt'}}]));
      yield frame('configuration.observed',{effective:normalizeEffectiveConfiguration(effective)});
      yield frame('attempt.accepted');
      if(scenario==='rate-limit'){yield frame('runtime.error',{error:{code:'rate-limit',phase:'admission',dispatch:'sent'}});yield frame('attempt.terminal',{outcome:'failed',error:{code:'rate-limit',phase:'admission',dispatch:'sent'}});return;}
      yield frame('attempt.started');
      if(scenario==='timeout'){yield frame('runtime.error',{error:{code:'timeout',phase:'running',dispatch:'unknown'}});return;}
      if(scenario==='crash'){yield frame('runtime.error',{error:{code:'crash',phase:'running',dispatch:'sent'}});yield frame('process.exited',{role:'runtime',code:1,signal:'none'},'supervisor');return;}
      if(scenario==='interruption'){yield frame('attempt.interrupt-requested');yield frame('attempt.interrupt-acknowledged');}
      yield frame('attempt.terminal',{outcome:scenario==='interruption'?'interrupted':'completed'});
      yield frame('process.exited',{role:'runtime',code:0,signal:'none'},'supervisor');
      yield frame('process.streams-closed',{role:'runtime',streams:{stdin:true,stdout:true,stderr:true,faulted:false}},'supervisor');
      yield frame('process.tree-observed',{ownership:'confirmed',source:'owned-process-group',termination:'confirmed',activeCount:0,evidenceRef:'synthetic-tree-proof'},'supervisor');
      yield frame('process.exited',{role:'helper',code:0,signal:'none'},'supervisor');
      yield frame('process.streams-closed',{role:'helper',streams:{stdin:true,stdout:true,stderr:true,faulted:false}},'supervisor');
    } finally {active=false;}
  }});
}
