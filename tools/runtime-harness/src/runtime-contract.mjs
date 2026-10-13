// OX-003 initial portable contract. No provider SDK, processes, filesystem or timers.
import {RUNTIME_INPUT_PROFILE,isPortableJson,portableTimestamp} from './runtime-input.mjs';
export {RUNTIME_INPUT_PROFILE} from './runtime-input.mjs';
export const RUNTIME_CONTRACT_VERSION=Object.freeze({major:1,minor:1});
export const CAPABILITY_NAMES=Object.freeze(['modelSelection','reasoning','resume','quota','tools','webSearch','streaming','sandbox','approvals','nativeDelegation','contextManagement']);
const supportValues=['supported','unsupported','unknown','runtime-managed'];
const availabilityValues=['available','unavailable','unknown'];
const configurationValues=['enabled','disabled','unknown'];
const evidenceSources=['none','declared','catalog','configuration','execution','synthetic'];
const reasoningValues=['auto','runtime-default','minimal','low','medium','high','maximum'];
const errorCodes=['authentication','authentication-unknown','unavailable','availability-unknown','evidence-missing','rate-limit','unsupported','policy','timeout','crash','transport-loss','protocol','ownership','sequence','version','budget','busy'];
const ids=['connectionId','sessionId','attemptId'];
const observationalEvents=new Set(['configuration.observation-recorded','attempt.admission-observed','attempt.start-observed','attempt.terminal-observed','attempt.interrupt-result','output.observed','process.streams-observed']);
const fatalErrorCodes=new Set(['protocol','ownership','sequence','version','budget']);
const encoder=new TextEncoder();
const isRecord=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const clone=value=>JSON.parse(JSON.stringify(value));
const freeze=value=>{if(value&&typeof value==='object'){for(const child of Object.values(value))freeze(child);Object.freeze(value);}return value;};
const integer=(value,min=0,max=Number.MAX_SAFE_INTEGER)=>Number.isSafeInteger(value)&&value>=min&&value<=max;

export class RuntimeContractError extends Error {
  constructor(code){super(`Runtime contract rejected: ${errorCodes.includes(code)?code:'protocol'}`);this.name='RuntimeContractError';this.code=errorCodes.includes(code)?code:'protocol';}
}
const reject=code=>{throw new RuntimeContractError(code);};
function token(value){if(typeof value!=='string'||value.length<1||value.length>100||/[^-A-Za-z0-9_.:]/.test(value))reject('protocol');return value;}
function localId(value){if(typeof value!=='string'||value.length<1||value.length>80||/[^-A-Za-z0-9_]/.test(value))reject('ownership');return value;}
function oneOf(value,values){if(!values.includes(value))reject('protocol');return value;}
function version(value){if(!isRecord(value)||!integer(value.major)||!integer(value.minor))reject('version');if(value.major!==1)reject('version');return {major:1,minor:value.minor};}
function scope(value){if(!isRecord(value))reject('ownership');return {...Object.fromEntries(ids.map(key=>[key,localId(value[key])])),turnId:value.turnId==null?null:localId(value.turnId)};}
function evidence(value={}) {
  if(!isRecord(value))reject('protocol');
  const source=oneOf(value.source??'none',evidenceSources),result={source,scope:oneOf(value.scope??'connection',['connection','model','attempt']),reference:value.reference==null?null:localId(value.reference),observedAt:null};
  if(value.observedAt!=null){result.observedAt=portableTimestamp(value.observedAt);if(result.observedAt===null)reject('protocol');}
  if(source==='none'&&(result.reference!==null||result.observedAt!==null))reject('protocol');return result;
}
function capability(value={}) {
  if(!isRecord(value))reject('protocol');
  return {support:oneOf(value.support??'unknown',supportValues),availability:oneOf(value.availability??'unknown',availabilityValues),configuration:oneOf(value.configuration??'unknown',configurationValues),evidence:evidence(value.evidence)};
}
function capabilities(value={}) {
  if(!isRecord(value))reject('protocol');return Object.fromEntries(CAPABILITY_NAMES.map(name=>[name,capability(value[name])]));
}

/** Descriptor contains local references, never credentials, endpoint config or native blobs. */
export function normalizeRuntimeDescriptor(value) {
  if(!isRecord(value)||!isRecord(value.connection)||!Array.isArray(value.models)||value.models.length>64)reject('protocol');
  const connection={id:localId(value.connection.id),revision:value.connection.revision,authentication:oneOf(value.connection.authentication,['authenticated','unauthenticated','unknown']),billing:oneOf(value.connection.billing,['subscription','api','unknown']),availability:oneOf(value.connection.availability??'unknown',availabilityValues)};
  if(!integer(connection.revision,1))reject('ownership');
  const modelIds=new Set();
  const models=value.models.map(model=>{
    if(!isRecord(model))reject('protocol');const id=localId(model.id);if(modelIds.has(id))reject('protocol');modelIds.add(id);
    let reasoningLevels=null;
    if(model.reasoningLevels!=null){if(!Array.isArray(model.reasoningLevels)||model.reasoningLevels.length>5||new Set(model.reasoningLevels).size!==model.reasoningLevels.length)reject('protocol');reasoningLevels=model.reasoningLevels.map(level=>oneOf(level,reasoningValues.filter(value=>!['auto','runtime-default'].includes(value))));}
    return {id,availability:oneOf(model.availability??'unknown',availabilityValues),reasoningLevels,capabilities:capabilities(model.capabilities)};
  });
  return freeze({version:version(value.version),adapterId:token(value.adapterId),runtimeVersion:token(value.runtimeVersion),connection,catalog:{completeness:oneOf(value.catalog?.completeness??'unknown',['complete','partial','unknown'])},capabilities:capabilities(value.capabilities),models,contextOwnership:{conversation:'runtime',projectMemory:'orchestrix'}});
}

/** No union across models: a connection feature never grants it to every model. */
export function capabilityDecision(descriptor,name,{modelId}={}) {
  if(!CAPABILITY_NAMES.includes(name))reject('unsupported');
  const normalized=normalizeRuntimeDescriptor(descriptor);
  const owner=modelId===undefined?normalized:normalized.models.find(model=>model.id===modelId);
  const observed=owner?.capabilities[name]??capability();
  const reason=observed.support!=='supported'?observed.support:observed.availability!=='available'?`availability-${observed.availability}`:observed.configuration!=='enabled'?`configuration-${observed.configuration}`:null;
  return freeze({allowed:reason===null,reason,capability:clone(observed)});
}

/** Policy intent is neutral. Adapters map these values; none is silently honored. */
export function normalizeRuntimeRequest(value) {
  if(!isRecord(value)||!isRecord(value.requested))reject('protocol');
  const owner=scope(value),requested=value.requested,modelPolicy=oneOf(requested.modelPolicy??'auto',['auto','specific','runtime-default']);
  const modelId=requested.modelId==null?null:localId(requested.modelId);
  if((modelPolicy==='specific')!==(modelId!==null))reject('policy');
  const billingMode=oneOf(value.billingMode??'subscription-only',['subscription-only','explicit-api']);
  if(value.paidFallback!==undefined&&value.paidFallback!=='forbidden')reject('policy');
  const billingAuthorizationRef=value.billingAuthorizationRef==null?null:localId(value.billingAuthorizationRef);
  if(billingMode==='explicit-api'&&billingAuthorizationRef===null)reject('policy');
  if(!integer(value.connectionRevision,1))reject('ownership');
  return freeze({version:version(value.version),...owner,connectionRevision:value.connectionRevision,billingMode,paidFallback:'forbidden',billingAuthorizationRef,contextPackRef:value.contextPackRef==null?null:localId(value.contextPackRef),requested:{strategy:oneOf(requested.strategy??'balanced',['efficiency','performance','balanced','custom']),presetRef:requested.presetRef==null?null:localId(requested.presetRef),modelPolicy,modelId,reasoning:oneOf(requested.reasoning??'auto',reasoningValues),sandbox:oneOf(requested.sandbox??'read-only',['read-only','workspace-write']),network:oneOf(requested.network??'disabled',['enabled','disabled']),approval:oneOf(requested.approval??'never',['never','ask','runtime-managed'])}});
}

function requireCapability(descriptor,name,modelId) {
  const decision=capabilityDecision(descriptor,name,{modelId});
  if(decision.allowed)return;
  if(decision.reason==='unsupported'||decision.reason==='runtime-managed')reject('unsupported');
  if(decision.reason==='configuration-disabled')reject('policy');
  if(decision.reason==='availability-unavailable')reject('unavailable');
  reject('evidence-missing');
}
function requireReasoningLevel(descriptor,model,level) {
  if(!model)reject('evidence-missing');requireCapability(descriptor,'reasoning',model.id);
  if(model.reasoningLevels===null)reject('evidence-missing');
  if(!model.reasoningLevels.includes(level))reject('unsupported');
}
export function assertRuntimeAdmission(descriptor,request,{effective}={}) {
  const runtime=normalizeRuntimeDescriptor(descriptor),intent=normalizeRuntimeRequest(request);
  if(intent.connectionId!==runtime.connection.id||intent.connectionRevision!==runtime.connection.revision)reject('ownership');
  if(runtime.connection.authentication!=='authenticated')reject(runtime.connection.authentication==='unknown'?'authentication-unknown':'authentication');
  if(runtime.connection.availability!=='available')reject(runtime.connection.availability==='unknown'?'availability-unknown':'unavailable');
  if(intent.billingMode==='subscription-only'&&runtime.connection.billing!=='subscription'||intent.billingMode==='explicit-api'&&runtime.connection.billing!=='api')reject('policy');
  const resolved=normalizeEffectiveConfiguration(effective);
  if(resolved.model.state==='known') {
    const selected=runtime.models.find(model=>model.id===resolved.model.value);
    if(!selected)reject('evidence-missing');
    if(selected.availability==='unavailable')reject('unavailable');
  }
  if(intent.requested.modelPolicy==='specific') {
    const selected=runtime.models.find(model=>model.id===intent.requested.modelId);
    if(!selected)reject('evidence-missing');
    if(selected.availability!=='available')reject(selected.availability==='unknown'?'availability-unknown':'unavailable');
    requireCapability(runtime,'modelSelection',selected.id);
    if(!['auto','runtime-default'].includes(intent.requested.reasoning))requireReasoningLevel(runtime,selected,intent.requested.reasoning);
  } else if(!['auto','runtime-default'].includes(intent.requested.reasoning)) {
    requireReasoningLevel(runtime,resolved.model.state==='known'?runtime.models.find(model=>model.id===resolved.model.value):null,intent.requested.reasoning);
  }
  return true;
}

const settingValues={model:null,reasoning:reasoningValues.filter(value=>value!=='auto'),sandbox:['read-only','workspace-write'],network:['enabled','disabled'],approval:['never','ask','runtime-managed']};
export function normalizeEffectiveConfiguration(value={}) {
  if(!isRecord(value))reject('protocol');
  return freeze(Object.fromEntries(Object.entries(settingValues).map(([key,values])=>{
    const input=value[key]??{};if(!isRecord(input))reject('protocol');
    const state=oneOf(input.state??'unknown',['known','unknown','runtime-managed']),proof=evidence(input.evidence);
    let resolved=null;
    if(state==='known') {
      resolved=values?oneOf(input.value,values):localId(input.value);
      if(!['configuration','execution','synthetic'].includes(proof.source))reject('protocol');
    } else if(input.value!=null)reject('protocol');
    return [key,{state,value:resolved,evidence:proof}];
  })));
}

function runtimeError(value) {
  if(!isRecord(value))reject('protocol');
  return {code:oneOf(value.code,errorCodes),phase:oneOf(value.phase,['connect','admission','running','cleanup']),dispatch:oneOf(value.dispatch??'unknown',['not-sent','sent','unknown']),retryable:value.retryable===true};
}
function closedStreams(value) {
  if(!isRecord(value)||!['stdin','stdout','stderr','faulted'].every(key=>typeof value[key]==='boolean'))reject('protocol');
  return Object.fromEntries(['stdin','stdout','stderr','faulted'].map(key=>[key,value[key]]));
}
function processState(){return {exited:false,code:null,signal:'unknown',streams:null,stdioClosed:false};}

/** One locally owned attempt. Frames are JSON strings; transport framing stays in adapters. */
export class RuntimeContractObserver {
  constructor({descriptor,request,inputProfile=RUNTIME_INPUT_PROFILE,maxFrameBytes=65536,maxEvents=256,maxTotalBytes=1048576}={}) {
    if(!isRecord(inputProfile)||inputProfile.id!==RUNTIME_INPUT_PROFILE.id||inputProfile.revision!==RUNTIME_INPUT_PROFILE.revision)reject('version');
    if(!integer(maxFrameBytes,1,1048576)||!integer(maxEvents,1,4096)||!integer(maxTotalBytes,1,16777216))reject('budget');
    this.descriptor=normalizeRuntimeDescriptor(descriptor);this.request=normalizeRuntimeRequest(request);
    if(this.descriptor.connection.id!==this.request.connectionId||this.descriptor.connection.revision!==this.request.connectionRevision)reject('ownership');
    this.maxFrameBytes=maxFrameBytes;this.maxEvents=maxEvents;this.maxTotalBytes=maxTotalBytes;
    this.sequence=0;this.bytes=0;this.events=[];this.phase='created';this.observation='healthy';this.terminal=null;this.effective=normalizeEffectiveConfiguration();
    this.admissionObserved=false;this.startObserved=false;this.output={assistant:0,tool:0,other:0,bytes:0,events:0};
    this.interrupt={requested:false,acknowledged:false};this.process={runtime:processState(),helper:processState(),tree:{ownership:'unknown',source:'unknown',termination:'unknown',activeCount:null,evidenceRef:null}};this.fault=null;this.fatal=false;
  }
  faulted(code,{fatal=false}={}){if(!this.fatal)this.fault={code};this.fatal||=fatal;if(this.observation!=='closed')this.observation='uncertain';}
  accept(frame,{origin='runtime'}={}) {
    try {
      if(this.observation==='closed')reject('transport-loss');
      if(!['runtime','supervisor'].includes(origin))reject('ownership');
      if(this.fatal&&origin!=='supervisor')reject(this.fault.code);
      if(typeof frame!=='string')reject('protocol');if(frame.length>this.maxFrameBytes)reject('budget');const bytes=encoder.encode(frame).byteLength;
      if(bytes>this.maxFrameBytes||this.bytes+bytes>this.maxTotalBytes||this.events.length>=this.maxEvents)reject('budget');
      if(!isPortableJson(frame))reject('protocol');
      let input;try{input=JSON.parse(frame);}catch{reject('protocol');}
      if(!isRecord(input))reject('protocol');const wireVersion=version(input.version),owner=scope(input);
      if([...ids,'turnId'].some(key=>owner[key]!==this.request[key]))reject('ownership');
      if(input.connectionRevision!==this.request.connectionRevision)reject('ownership');
      if(!integer(input.sequence,1)||input.sequence!==this.sequence+1)reject('sequence');
      if(typeof input.type!=='string'||input.type.length>100)reject('protocol');
      if(this.fatal&&!input.type.startsWith('process.'))reject(this.fault.code);
      // Provenance comes from the trusted host call, never a JSON field sent by
      // a provider. Structure validation cannot establish OS ownership itself.
      if(input.type.startsWith('process.')&&origin!=='supervisor')reject('ownership');
      if((input.type.startsWith('attempt.')||input.type.startsWith('configuration.')||input.type==='output.observed')&&origin!=='runtime')reject('ownership');
      const event={version:wireVersion,...owner,connectionRevision:this.request.connectionRevision,sequence:input.sequence,type:input.type,origin};
      // v1.0 retains its original vocabulary and semantics. New factual events
      // use a v1.1 envelope; older consumers retain them only as diagnostics.
      if(wireVersion.minor===0&&observationalEvents.has(event.type))event.type='runtime.diagnostic';
      this.apply(event,input);this.sequence=input.sequence;this.bytes+=bytes;this.events.push(freeze(event));return freeze(clone(event));
    } catch(error) {const safe=error instanceof RuntimeContractError?error:new RuntimeContractError('protocol');this.faulted(safe.code,{fatal:true});throw safe;}
  }
  apply(event,input) {
    switch(event.type) {
      case 'configuration.observed': {
        if(this.phase!=='created')reject('protocol');const observed=normalizeEffectiveConfiguration(input.effective);
        if(observed.model.state==='known'&&!this.descriptor.models.some(model=>model.id===observed.model.value))reject('unsupported');
        if(observed.model.state==='known'&&this.descriptor.models.find(model=>model.id===observed.model.value).availability==='unavailable')reject('unavailable');
        // Initial v1 has no implicit degradation policy. Unknown stays unknown;
        // a known mismatch cannot silently satisfy the requested configuration.
        for(const key of ['sandbox','network','approval'])if(observed[key].state==='known'&&observed[key].value!==this.request.requested[key])reject('policy');
        if(observed.model.state==='known'&&this.request.requested.modelPolicy==='specific'&&observed.model.value!==this.request.requested.modelId)reject('policy');
        if(observed.reasoning.state==='known'&&!['auto','runtime-default'].includes(this.request.requested.reasoning)&&observed.reasoning.value!==this.request.requested.reasoning)reject('policy');
        if(observed.reasoning.state==='known'&&observed.reasoning.value!=='runtime-default')requireReasoningLevel(this.descriptor,observed.model.state==='known'?this.descriptor.models.find(model=>model.id===observed.model.value):null,observed.reasoning.value);
        this.effective=observed;event.effective=clone(observed);break;
      }
      case 'configuration.observation-recorded': {
        const observed=normalizeEffectiveConfiguration(input.effective);
        if(observed.model.state==='known'&&!this.descriptor.models.some(model=>model.id===observed.model.value))reject('ownership');
        this.effective=observed;event.effective=clone(observed);break;
      }
      case 'attempt.accepted':
        if(this.phase!=='created'||this.observation!=='healthy')reject('protocol');assertRuntimeAdmission(this.descriptor,this.request,{effective:this.effective});this.admissionObserved=true;this.phase='accepted';break;
      case 'attempt.started':
        if(this.phase!=='accepted')reject('protocol');this.startObserved=true;this.phase='running';break;
      case 'attempt.admission-observed':
        if(this.admissionObserved||!['created','running','terminal'].includes(this.phase))reject('protocol');
        this.admissionObserved=true;if(!this.terminal&&!this.startObserved)this.phase='accepted';break;
      case 'attempt.start-observed':
        if(this.startObserved||this.terminal||!['created','accepted'].includes(this.phase))reject('protocol');
        this.startObserved=true;this.phase='running';break;
      case 'attempt.interrupt-requested':
        if(!['accepted','running'].includes(this.phase)||this.interrupt.requested)reject('protocol');this.interrupt.requested=true;break;
      case 'attempt.interrupt-acknowledged':
        if(!this.interrupt.requested||this.interrupt.acknowledged||this.terminal)reject('protocol');this.interrupt.acknowledged=true;break;
      case 'attempt.interrupt-result':
        if(typeof input.acknowledged!=='boolean')reject('protocol');
        if(this.interrupt.acknowledged&&!input.acknowledged)reject('protocol');
        this.interrupt.acknowledged||=input.acknowledged;event.acknowledged=input.acknowledged;break;
      case 'attempt.terminal': {
        const outcome=oneOf(input.outcome,['completed','failed','interrupted']);
        if(this.terminal||!['accepted','running'].includes(this.phase)||outcome==='completed'&&this.phase!=='running')reject('protocol');
        const error=input.error==null?null:runtimeError(input.error);if(outcome==='completed'&&error)reject('protocol');
        this.terminal={outcome,error};this.phase='terminal';event.outcome=outcome;event.error=error;break;
      }
      case 'attempt.terminal-observed': {
        const outcome=oneOf(input.outcome,['completed','failed','interrupted']);
        if(this.terminal||!['created','accepted','running'].includes(this.phase))reject('protocol');
        if(input.startObserved!==undefined&&(typeof input.startObserved!=='boolean'||input.startObserved!==this.startObserved))reject('protocol');
        const error=input.error==null?null:runtimeError(input.error);if(outcome==='completed'&&error)reject('protocol');
        this.terminal={outcome,error};this.phase='terminal';event.outcome=outcome;event.error=error;event.startObserved=this.startObserved;break;
      }
      case 'output.observed': {
        const channel=oneOf(input.channel,['assistant','tool','other']);if(!integer(input.bytes))reject('protocol');
        if(!integer(this.output.bytes+input.bytes)||!integer(this.output[channel]+input.bytes))reject('budget');
        this.output.bytes+=input.bytes;this.output[channel]+=input.bytes;this.output.events++;event.channel=channel;event.bytes=input.bytes;break;
      }
      case 'runtime.error': {
        const error=runtimeError(input.error);event.error=error;
        if(input.fatal!==undefined&&typeof input.fatal!=='boolean')reject('protocol');
        if(input.fatal===true&&!fatalErrorCodes.has(error.code))reject('protocol');
        if(input.fatal===true){event.fatal=true;this.faulted(error.code,{fatal:true});}
        if(!this.terminal){if(this.phase==='created'&&error.dispatch==='not-sent')this.phase='rejected';else this.observation='uncertain';}break;
      }
      case 'process.exited': {
        const role=oneOf(input.role,['runtime','helper']),observed=this.process[role];
        if(observed.exited||!(input.code===null||integer(input.code,-2147483648,4294967295)))reject('protocol');
        const signal=oneOf(input.signal??'unknown',['none','terminated','killed','unknown']);
        Object.assign(observed,{exited:true,code:input.code,signal});event.role=role;event.code=input.code;event.signal=signal;
        if(role==='runtime'&&!this.terminal)this.observation='uncertain';break;
      }
      case 'process.streams-closed': {
        const role=oneOf(input.role,['runtime','helper']),observed=this.process[role],streams=closedStreams(input.streams);
        if(observed.streams)reject('protocol');observed.streams=streams;observed.stdioClosed=streams.stdin&&streams.stdout&&streams.stderr&&!streams.faulted;event.role=role;event.streams=streams;break;
      }
      case 'process.streams-observed': {
        const role=oneOf(input.role,['runtime','helper']),observed=this.process[role],streams=closedStreams(input.streams);
        const merged=Object.fromEntries(Object.keys(streams).map(key=>[key,streams[key]||observed.streams?.[key]===true]));
        observed.streams=merged;observed.stdioClosed=merged.stdin&&merged.stdout&&merged.stderr&&!merged.faulted;
        event.role=role;event.streams=streams;break;
      }
      case 'process.tree-observed': {
        const ownership=oneOf(input.ownership,['confirmed','unknown']),source=oneOf(input.source,['owned-job','owned-process-group','unknown']),termination=oneOf(input.termination,['confirmed','unknown']);
        const activeCount=input.activeCount;if(!(activeCount===null||integer(activeCount)))reject('protocol');
        const evidenceRef=input.evidenceRef==null?null:localId(input.evidenceRef);
        if(ownership==='confirmed'&&(source==='unknown'||evidenceRef===null))reject('ownership');
        if(termination==='confirmed'&&(ownership!=='confirmed'||activeCount!==0||!this.process.runtime.exited))reject('protocol');
        if(this.process.tree.termination==='confirmed'&&termination!=='confirmed')reject('protocol');
        this.process.tree={ownership,source,termination,activeCount,evidenceRef};Object.assign(event,this.process.tree);break;
      }
      default:event.type='runtime.diagnostic';event.code='unknown-event';
    }
  }
  transportLost(reason='transport-loss') {
    if(!['timeout','transport-loss','crash'].includes(reason))reject('protocol');
    this.faulted(reason);return this.snapshot();
  }
  close(){this.observation='closed';return this.snapshot();}
  snapshot() {
    return freeze(clone({version:RUNTIME_CONTRACT_VERSION,scope:{...scope(this.request),connectionRevision:this.request.connectionRevision},phase:this.phase,admissionObserved:this.admissionObserved,startObserved:this.startObserved,observation:this.observation,fatal:this.fatal,outcome:this.terminal?.outcome??'unknown',terminalConfirmed:this.terminal!==null,terminal:this.terminal,requested:this.request.requested,effective:this.effective,interrupt:this.interrupt,output:this.output,process:this.process,fault:this.fault,events:this.events,budget:{frames:this.events.length,bytes:this.bytes,maxEvents:this.maxEvents,maxFrameBytes:this.maxFrameBytes,maxTotalBytes:this.maxTotalBytes},contextOwnership:{conversation:'runtime',projectMemory:'orchestrix'}}));
  }
}
