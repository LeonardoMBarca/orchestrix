// Experimental Codex boundary. Native identities and protocol names stop here.
import {HarnessError} from './errors.mjs';
import {RUNTIME_CONTRACT_VERSION,RuntimeContractError,RuntimeContractObserver,normalizeRuntimeDescriptor,normalizeRuntimeRequest} from './runtime-contract.mjs';

const identities=new WeakMap();
const local=value=>typeof value==='string'&&/^[-A-Za-z0-9_]{1,80}$/.test(value);
const native=value=>typeof value==='string'&&value.length>0&&value.length<=512&&!/[\u0000-\u001f]/.test(value);
const neutral=new Set(['minimal','low','medium','high','maximum']);
const reasoning=value=>neutral.has(value)?value:null;
const fail=code=>{throw new RuntimeContractError(code);};
const safeError=error=>{
  const value=error?.kind??error?.code??error;
  if(value==='rpc'&&error.metadata?.dataTag==='rate_limit')return 'rate-limit';
  if(value==='rpc'&&error.metadata?.dataTag==='authentication')return 'authentication';
  if(['usageLimitExceeded','rateLimitExceeded','sessionBudgetExceeded','rate-limit'].includes(value))return 'rate-limit';
  if(['unauthorized','authentication','subscription-gate'].includes(value))return 'authentication';
  if(['timeout','crash','policy','ownership','budget','sequence','version','unsupported','evidence-missing','authentication-unknown','availability-unknown','unavailable','busy'].includes(value))return value;
  if(value==='launch-timeout')return 'timeout';
  if(value==='capability')return 'unsupported';
  if(['outbound-limit','backpressure'].includes(value))return 'budget';
  if(['arguments','contract'].includes(value))return 'policy';
  if(['protocol','normalization'].includes(value))return 'protocol';
  if(['launch','process-exit','transport','transport-unavailable','helper-exit','helper-protocol','helper-transport','termination'].includes(value))return 'transport-loss';
  return 'evidence-missing';
};

/** Opaque, connection-scoped mapping retained across app-server restart/resume. */
export class CodexIdentityMap {
  constructor({connectionId,connectionRevision}={}) {
    if(!local(connectionId)||!Number.isSafeInteger(connectionRevision)||connectionRevision<1)fail('ownership');
    identities.set(this,{connectionId,connectionRevision,models:new Map(),sessions:new Map(),localSessions:new Map(),attempts:new Set(),turns:new Map(),turnSequence:0,binding:null});Object.freeze(this);
  }
}

/** Report capture is independent of whether process shutdown was confirmed. */
export function collectCodexObservation(adapter,state) {
  let normalizationError=null;
  try {adapter.observeProcess(state);}catch(error){normalizationError=safeError(error);}
  try {return {descriptor:adapter.descriptor,attempts:adapter.snapshots(),diagnostics:adapter.diagnostics,...(normalizationError?{normalizationError}:{})};}
  finally {adapter.dispose();}
}
function store(mapping,connectionId,connectionRevision) {
  const value=identities.get(mapping);
  if(!value||value.connectionId!==connectionId||value.connectionRevision!==connectionRevision)fail('ownership');return value;
}
function modelRef(state,name) {
  if(!native(name))fail('protocol');
  if(!state.models.has(name)){if(state.models.size>=256)fail('budget');state.models.set(name,`model-${state.models.size+1}`);}
  return state.models.get(name);
}
function bindSession(state,nativeId,localId) {
  if(!native(nativeId)||!local(localId))fail('ownership');
  if(state.sessions.has(nativeId)&&state.sessions.get(nativeId)!==localId||state.localSessions.has(localId)&&state.localSessions.get(localId)!==nativeId)fail('ownership');
  if(!state.sessions.has(nativeId)&&state.sessions.size>=64)fail('budget');state.sessions.set(nativeId,localId);state.localSessions.set(localId,nativeId);
}
const proof=(source,scope,reference)=>({source,scope,reference});
const capability=(support='unknown',observed=false,evidence={source:'none'})=>({support,availability:observed?'available':'unknown',configuration:observed?'enabled':'unknown',evidence});
function descriptorFor({state,discovery={},configuration,runtimeVersion,source}) {
  const configured=configuration?.resolvedConfiguration,selected=configured?.model;
  const listed=Array.isArray(discovery.models)?discovery.models:[];
  const rows=listed.filter(row=>native(row?.model));
  if(native(selected)&&!rows.some(row=>row.model===selected))rows.unshift({model:selected,efforts:null});
  else if(native(selected))rows.sort((a,b)=>Number(b.model===selected)-Number(a.model===selected));
  const unique=[...new Map(rows.map(row=>[row.model,row])).values()];
  const models=unique.slice(0,64).map(row=>{
    const id=modelRef(state,row.model),active=selected===row.model;
    const complete=Array.isArray(row.efforts)&&row.reasoning?.completeness==='complete'&&['supported','unsupported'].includes(row.reasoning?.support);
    const levels=complete&&row.efforts.every(value=>reasoning(value)!==null)?[...new Set(row.efforts.map(reasoning))]:null;
    const evidence=proof(source==='synthetic'?'synthetic':active?'configuration':'catalog','model',id);
    return {id,availability:'unknown',reasoningLevels:levels,capabilities:{
      modelSelection:capability('supported',active,evidence),
      reasoning:capability(complete?row.reasoning.support:'unknown',active&&reasoning(configured?.effort)!==null,evidence),
    }};
  });
  const auth=discovery.auth??{},available=discovery.availability;
  return normalizeRuntimeDescriptor({version:RUNTIME_CONTRACT_VERSION,adapterId:'codex-app-server',runtimeVersion,
    connection:{id:state.connectionId,revision:state.connectionRevision,authentication:auth.authenticated===true?'authenticated':auth.authenticated===false?'unauthenticated':'unknown',billing:auth.subscriptionGate===true?'subscription':auth.type==='apiKey'?'api':'unknown',availability:available===true?'available':available===false?'unavailable':'unknown'},
    catalog:{completeness:unique.length>64?'partial':discovery.catalog?.completeness??'unknown'},models,
    capabilities:{resume:capability('supported',false,proof(source==='synthetic'?'synthetic':'declared','connection','protocol-surface')),contextManagement:{support:'runtime-managed',evidence:proof(source==='synthetic'?'synthetic':'declared','connection','runtime-context')}}});
}
function effectiveFor(configuration,state,source) {
  const observed=configuration?.resolvedConfiguration??{},result={};
  const put=(key,value)=>{if(value!==undefined&&value!==null)result[key]={state:'known',value,evidence:proof(source==='synthetic'?'synthetic':'configuration','attempt','configuration-observation')};};
  if(native(observed.model))put('model',modelRef(state,observed.model));
  put('reasoning',reasoning(observed.effort));
  put('sandbox',observed.sandbox==='workspaceWrite'||observed.sandbox==='workspace-write'?'workspace-write':observed.sandbox==='readOnly'||observed.sandbox==='read-only'?'read-only':null);
  put('network',typeof observed.networkAccess==='boolean'?(observed.networkAccess?'enabled':'disabled'):null);
  put('approval',observed.approval==='never'?'never':null);return result;
}

/** Owns one native app-server session attachment; its observers are provider-neutral. */
export class CodexRuntimeAdapter {
  #session;#discovery;#runtimeVersion;#connectionId;#revision;#sessionId;#mapping;#state;#source;
  #records=new Map();#nativeTurns=new Map();#pending=null;#active=null;#unsubscribe;#failureListener;#closedListener;#disposed=false;#unscoped=0;#limits;
  constructor({session,discovery,runtimeVersion,connectionId='connection-1',connectionRevision=1,sessionId='session-1',mappings,evidenceSource='configuration',limits={}}={}) {
    if(!session||typeof session.subscribe!=='function'||!local(sessionId)||!['configuration','synthetic'].includes(evidenceSource))fail('ownership');
    this.#session=session;this.#discovery=discovery??{};this.#runtimeVersion=runtimeVersion;this.#connectionId=connectionId;this.#revision=connectionRevision;this.#sessionId=sessionId;this.#source=evidenceSource;this.#limits=limits;
    this.#mapping=mappings??new CodexIdentityMap({connectionId,connectionRevision});this.#state=store(this.#mapping,connectionId,connectionRevision);
    const binding=session.accountBinding;if(binding){if(this.#state.binding&&this.#state.binding!==binding)fail('ownership');this.#state.binding=binding;}
    if(session.threadId)bindSession(this.#state,session.threadId,sessionId);
    this.#unsubscribe=session.subscribe(event=>this.#onSessionEvent(event));
    this.#failureListener=error=>this.#onFailure(error);this.#closedListener=()=>this.#onFailure(new HarnessError('process-exit','Runtime observation closed'));
    session.client.on('failure',this.#failureListener);session.client.on('closed',this.#closedListener);
  }
  get descriptor(){return descriptorFor({state:this.#state,discovery:this.#discovery,configuration:this.#session.configuration,runtimeVersion:this.#runtimeVersion,source:this.#source});}
  #emit(record,type,fields={},origin='runtime') {
    return record.observer.accept(JSON.stringify({version:RUNTIME_CONTRACT_VERSION,...record.observer.snapshot().scope,sequence:record.observer.sequence+1,type,...fields}),{origin});
  }
  #error(record,error,dispatch=record.nativeId?'sent':'unknown') {
    const code=safeError(error),fatal=['protocol','ownership','sequence','version','budget'].includes(code);
    if(record.observer.snapshot().fatal)return;
    this.#flushOutput(record);
    this.#emit(record,'runtime.error',{error:{code,phase:record.nativeId?'running':'admission',dispatch,retryable:false},fatal});
  }
  #flushOutput(record) {
    if(record.outputBatchBytes&&!record.observer.snapshot().fatal){this.#emit(record,'output.observed',{channel:'assistant',bytes:record.outputBatchBytes});record.outputBatchBytes=0;record.deltaCount=0;}
  }
  #onFailure(error) {
    for(const record of this.#records.values())if(!record.observer.snapshot().terminalConfirmed)try{this.#error(record,error);}catch{}
  }
  #onSessionEvent(event) {
    if(this.#disposed)return;
    let record=event.turnId?this.#nativeTurns.get(event.turnId):null;
    if(event.type==='attempt.turn.accepted') {
      record=this.#pending??(record?.replay?record:null);if(!record)throw new HarnessError('normalization','Admission has no locally prepared attempt');
      const key=`${this.#sessionId}:${event.turnId}`;
      if(this.#state.turns.has(key)||record.nativeId&&!record.replay)throw new HarnessError('normalization','Duplicate native turn admission');
      this.#state.turns.set(key,record.request.attemptId);record.nativeId=event.turnId;this.#nativeTurns.set(event.turnId,record);
    }
    if(!record){this.#unscoped++;return;}
    try {
      switch(event.type) {
        case 'attempt.turn.accepted':this.#emit(record,'attempt.admission-observed');this.#active=record;break;
        case 'attempt.turn.started':this.#emit(record,'attempt.start-observed');break;
        case 'attempt.turn.ended': {
          this.#flushOutput(record);const code=safeError(event.errorKind);
          const error=event.errorKind&&code!=='evidence-missing'?{code,phase:'running',dispatch:'sent',retryable:false}:undefined;
          this.#emit(record,'attempt.terminal-observed',{outcome:event.status,...(error?{error}:{})});if(this.#active===record)this.#active=null;break;
        }
        case 'attempt.interrupt.sent':this.#emit(record,'attempt.interrupt-requested');break;
        case 'attempt.interrupt.requested':this.#emit(record,'attempt.interrupt-result',{acknowledged:true});break;
        case 'attempt.observation.unknown':this.#error(record,{kind:event.reason},event.dispatch==='not-written'?'not-sent':undefined);break;
        case 'response.delta':
          if(!Number.isSafeInteger(event.bytes)||event.bytes<0||record.outputTotal+event.bytes>65536)fail('budget');
          record.outputTotal+=event.bytes;record.outputBatchBytes+=event.bytes;record.deltaCount++;
          if(record.outputBatchBytes>=4096||record.deltaCount>=32)this.#flushOutput(record);break;
        default:this.#unscoped++;
      }
    } catch(error){record.error=error.code??'protocol';throw error;}
  }
  #prepare({attemptId,effort,requested={},replay=false,nativeTurnId}={}) {
    if(this.#disposed||!local(attemptId)||this.#state.attempts.has(attemptId))fail('ownership');
    if(this.#state.attempts.size>=256)fail('budget');
    const configuration=this.#session.configuration,raw=configuration?.requested??{};
    const selected=raw.model;
    const requestedEffort=effort??raw.effort;
    if(!replay&&requestedEffort!=null&&!reasoning(requestedEffort))fail('unsupported');
    const turnId=`turn-${++this.#state.turnSequence}`;
    const request=normalizeRuntimeRequest({version:RUNTIME_CONTRACT_VERSION,connectionId:this.#connectionId,connectionRevision:this.#revision,sessionId:this.#sessionId,attemptId,turnId,billingMode:'subscription-only',paidFallback:'forbidden',requested:{...requested,modelPolicy:native(selected)?'specific':'auto',modelId:native(selected)?modelRef(this.#state,selected):null,reasoning:reasoning(requestedEffort)??'auto',sandbox:raw.sandbox==='workspace-write'?'workspace-write':'read-only',network:raw.networkAccess===true?'enabled':'disabled',approval:'never'}});
    const record={request,observer:new RuntimeContractObserver({descriptor:this.descriptor,request,...this.#limits}),nativeId:nativeTurnId??null,processSeen:new Set(),error:null,replay,outputBatchBytes:0,outputTotal:0,deltaCount:0};
    this.#state.attempts.add(attemptId);this.#records.set(attemptId,record);
    if(nativeTurnId)this.#nativeTurns.set(nativeTurnId,record);
    this.#emit(record,'configuration.observation-recorded',{effective:effectiveFor(configuration,this.#state,this.#source)});return record;
  }
  async startAttempt({attemptId,text,effort,requested}={}) {
    if(this.#pending||this.#active)fail('busy');const record=this.#prepare({attemptId,effort,requested});
    this.#pending=record;let called=false;
    try {
      if(this.#discovery.auth?.subscriptionGate!==true)throw new HarnessError(this.#discovery.auth?.authenticated===false?'authentication':this.#discovery.auth?.authenticated===true?'policy':'authentication-unknown','Subscription execution gates not confirmed');
      if(this.#discovery.availability!==true)throw new HarnessError(this.#discovery.availability===false?'unavailable':'availability-unknown','Connection availability not confirmed');
      if(!this.#session.threadId)throw new HarnessError('contract','An owned native session is required');
      called=true;const id=await this.#session.startTurn(text,{effort});
      if(record.nativeId!==id)throw new HarnessError('normalization','Native admission was not mapped to the prepared attempt');
      return {attemptId,sessionId:this.#sessionId,turnId:record.request.turnId};
    } catch(error){try{this.#error(record,error,error.metadata?.dispatch==='not-written'||!called?'not-sent':record.nativeId||error.kind==='rpc'?'sent':'unknown');}catch{}throw error;}
    finally{this.#pending=null;}
  }
  #record(attemptId){const record=this.#records.get(attemptId);if(!record)fail('ownership');return record;}
  async waitForAttempt(attemptId,timeoutMs) {
    const record=this.#record(attemptId);if(!record.nativeId)fail('ownership');
    try{await this.#session.waitForTurn(record.nativeId,timeoutMs);return this.snapshot(attemptId);}
    catch(error){try{this.#error(record,error);}catch{}throw error;}
  }
  async interrupt(attemptId) {
    const record=this.#record(attemptId);if(!record.nativeId)fail('ownership');
    try{await this.#session.interrupt(record.nativeId);return this.snapshot(attemptId);}
    catch(error){try{this.#error(record,error,error.metadata?.dispatch==='not-written'?'not-sent':error.kind==='rpc'||record.observer.snapshot().interrupt.acknowledged?'sent':'unknown');}catch{}throw error;}
  }
  observeProcess(state) {
    if(this.#disposed)fail('ownership');
    const observed=state?.supervision??state??{};
    for(const record of this.#records.values()) {
      this.#flushOutput(record);
      const once=(key,type,fields)=>{if(!record.processSeen.has(key)){this.#emit(record,type,fields,'supervisor');record.processSeen.add(key);}};
      if(observed.parentExited===true)once('runtime-exit','process.exited',{role:'runtime',code:Number.isSafeInteger(observed.code)?observed.code:null,signal:observed.signal==null?'none':'unknown'});
      if(observed.streams&&['stdin','stdout','stderr'].every(key=>typeof observed.streams[key]==='boolean'))once(`runtime-streams-${JSON.stringify(observed.streams)}`,'process.streams-observed',{role:'runtime',streams:{...observed.streams,faulted:false}});
      else if(observed.stdioClosed===true)once('runtime-streams-complete','process.streams-observed',{role:'runtime',streams:{stdin:true,stdout:true,stderr:true,faulted:false}});
      if(observed.helper?.parentExited===true)once('helper-exit','process.exited',{role:'helper',code:Number.isSafeInteger(observed.helper.code)?observed.helper.code:null,signal:'none'});
      if(observed.helperStreams&&['stdinClosed','stdoutClosed','stderrClosed','faulted'].every(key=>typeof observed.helperStreams[key]==='boolean'))once(`helper-streams-${JSON.stringify(observed.helperStreams)}`,'process.streams-observed',{role:'helper',streams:{stdin:observed.helperStreams.stdinClosed,stdout:observed.helperStreams.stdoutClosed,stderr:observed.helperStreams.stderrClosed,faulted:observed.helperStreams.faulted}});
      if(observed.ownershipConfirmed===true&&Number.isSafeInteger(observed.activeCount)) {
        const confirmed=observed.treeTermination==='confirmed';
        if(!confirmed&&record.observer.snapshot().process.tree.termination==='confirmed')continue;
        once(`tree-${confirmed?'confirmed':'unknown'}-${observed.activeCount}`,'process.tree-observed',{ownership:'confirmed',source:'owned-job',termination:confirmed?'confirmed':'unknown',activeCount:observed.activeCount,evidenceRef:'owned-supervision'});
      }
    }
  }
  // Replay consumes only recorded facts; it never dispatches a native request.
  prepareReplayAttempt({attemptId,nativeTurnId,effort}={}) {return this.#prepare({attemptId,nativeTurnId,effort,replay:true}).request.attemptId;}
  replayEvent(event) {this.#onSessionEvent(event);}
  flushOutput(attemptId){this.#flushOutput(this.#record(attemptId));}
  snapshot(attemptId) {return this.#record(attemptId).observer.snapshot();}
  snapshots(){return [...this.#records.values()].map(record=>record.observer.snapshot());}
  get diagnostics(){return {unscopedNotifications:this.#unscoped};}
  dispose(){if(this.#disposed)return;this.#disposed=true;this.#unsubscribe();this.#session.client.removeListener('failure',this.#failureListener);this.#session.client.removeListener('closed',this.#closedListener);}
}
