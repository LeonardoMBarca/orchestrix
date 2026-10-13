import {HarnessError} from './json-rpc.mjs';
import {createHash} from 'node:crypto';
import {isAbsolute,normalize} from 'node:path';

const validIdentity=value=>typeof value==='string'&&value.length>0&&value.length<=512&&!/[\u0000-\u001f]/.test(value);
const samePath=(left,right)=>typeof left==='string'&&typeof right==='string'&&isAbsolute(left)&&isAbsolute(right)&&(process.platform==='win32'?normalize(left).toLowerCase()===normalize(right).toLowerCase():normalize(left)===normalize(right));
const freezePolicy=policy=>Object.freeze(Object.fromEntries(Object.entries(policy).map(([key,value])=>[key,Array.isArray(value)?Object.freeze([...value]):value&&typeof value==='object'?freezePolicy(value):value])));

const catalogRecord=value=>value!==null&&typeof value==='object'&&!Array.isArray(value);
const catalogText=(value,max=512)=>typeof value==='string'&&value.length>0&&value.length<=max&&!/[\u0000-\u001f]/.test(value);
function catalogOptions(value) {
  if(!catalogRecord(value)||Object.keys(value).some(key=>!['pageSize','maxPages','maxModels','timeoutMs'].includes(key)))throw new HarnessError('arguments','Invalid model discovery bounds');
  const options={pageSize:100,maxPages:10,maxModels:256,timeoutMs:15000,...value};
  for(const [key,maximum] of Object.entries({pageSize:100,maxPages:10,maxModels:256,timeoutMs:15000}))if(!Number.isSafeInteger(options[key])||options[key]<1||options[key]>maximum)throw new HarnessError('arguments','Invalid model discovery bounds');
  return options;
}
function catalogModel(value) {
  if(!catalogRecord(value)||!catalogText(value.id)||!catalogText(value.model))return null;
  const rawEfforts=value.supportedReasoningEfforts;
  const efforts=Array.isArray(rawEfforts)?[...new Set(rawEfforts.slice(0,64).map(item=>typeof item==='string'?item:catalogRecord(item)?item.reasoningEffort:null).filter(item=>catalogText(item,128)))]:[];
  const effortsValid=Array.isArray(rawEfforts)&&rawEfforts.length<=64&&rawEfforts.every(item=>catalogText(typeof item==='string'?item:catalogRecord(item)?item.reasoningEffort:null,128));
  const defaultEffort=catalogText(value.defaultReasoningEffort,128)?value.defaultReasoningEffort:null;
  const defaultConsistent=defaultEffort===null||efforts.includes(defaultEffort);
  const tiers=Array.isArray(value.serviceTiers)&&value.serviceTiers.length<=32&&value.serviceTiers.every(tier=>catalogRecord(tier)&&catalogText(tier.id)&&catalogText(tier.name)&&typeof tier.description==='string'&&tier.description.length<=2048)?value.serviceTiers.map(tier=>({id:tier.id,name:tier.name,description:tier.description})):null;
  const modalities=Array.isArray(value.inputModalities)&&value.inputModalities.length<=16&&value.inputModalities.every(item=>catalogText(item,128))?[...new Set(value.inputModalities)]:null;
  return {id:value.id,model:value.model,isDefault:value.isDefault===true,defaultEffort,efforts,
    // Catalog membership is not entitlement or a successful model invocation.
    availability:'unknown',
    reasoning:{support:effortsValid&&defaultConsistent?(efforts.length?'supported':'unsupported'):'unknown',completeness:effortsValid&&defaultConsistent?'complete':Array.isArray(rawEfforts)?'partial':'unknown'},
    serviceTiers:tiers,defaultServiceTier:catalogText(value.defaultServiceTier)?value.defaultServiceTier:null,
    multiAgentVersion:catalogText(value.multiAgentVersion,128)?value.multiAgentVersion:null,inputModalities:modalities};
}
async function discoverModelCatalog(client,options) {
  const byId=new Map(),byModel=new Map(),blockedIds=new Set(),blockedModels=new Set(),cursors=new Set();
  const catalog={source:'model/list',scope:'visible-models',includeHidden:false,completeness:'unknown',pages:0,modelCount:0,reason:'cursor-missing'};
  const deadline=performance.now()+options.timeoutMs;let cursor,ended=false,lossReason=null,stopReason=null;
  for(let page=0;page<options.maxPages;page++) {
    const remaining=deadline-performance.now();if(remaining<1){stopReason='deadline';break;}
    let response;
    try {
      response=await client.request('model/list',{includeHidden:false,limit:options.pageSize,...(cursor!==undefined?{cursor}:{})},Math.min(5000,Math.floor(remaining)));
      if(!catalogRecord(response)||!Array.isArray(response.data))throw new HarnessError('protocol','Invalid model discovery page');
    } catch(error) {
      // Keep the original fail-closed first-read contract. A later page may
      // fail without erasing previously observed, explicitly partial data.
      if(catalog.pages===0)throw error;
      stopReason='read-failed';break;
    }
    if(performance.now()>deadline){stopReason='deadline';break;}
    catalog.pages++;
    if(response.data.length>options.pageSize)stopReason='page-size-exceeded';
    for(const value of response.data.slice(0,options.pageSize)) {
      const model=catalogModel(value);
      if(!model){lossReason??='invalid-model';continue;}
      if(blockedIds.has(model.id)||blockedModels.has(model.model)){lossReason??='conflicting-model';continue;}
      const priorId=byId.get(model.id),priorModel=byModel.get(model.model);
      if(priorId||priorModel) {
        if(priorId===priorModel&&JSON.stringify(priorId)===JSON.stringify(model))continue;
        for(const conflicting of [priorId,priorModel,model].filter(Boolean)) {
          blockedIds.add(conflicting.id);blockedModels.add(conflicting.model);
          byId.delete(conflicting.id);byModel.delete(conflicting.model);
        }
        lossReason??='conflicting-model';continue;
      }
      if(byId.size>=options.maxModels){stopReason='model-limit';break;}
      byId.set(model.id,model);byModel.set(model.model,model);
    }
    if(stopReason)break;
    if(!Object.hasOwn(response,'nextCursor')){stopReason='cursor-missing';break;}
    if(response.nextCursor===null){ended=true;break;}
    if(typeof response.nextCursor!=='string'||response.nextCursor.length>4096||/[\u0000-\u001f]/.test(response.nextCursor)){stopReason='invalid-cursor';break;}
    if(cursors.has(response.nextCursor)){stopReason='cursor-cycle';break;}
    cursors.add(response.nextCursor);cursor=response.nextCursor;
    if(page+1===options.maxPages)stopReason='page-limit';
  }
  const models=[...byId.values()];catalog.modelCount=models.length;
  catalog.reason=lossReason??stopReason??'complete';
  catalog.completeness=ended&&!lossReason?'complete':catalog.pages===0||!lossReason&&stopReason==='cursor-missing'&&catalog.pages===1?'unknown':'partial';
  return {models,catalog};
}

/** Deliberately small experiment. Provider messages never become the Core domain. */
export class CodexSession {
  constructor(client,{attemptId='probe-attempt-1',turnTimeoutMs=45000,interruptTimeoutMs=10000}={}) {
    this.client=client;this.attemptId=attemptId;this.turnTimeoutMs=turnTimeoutMs;this.interruptTimeoutMs=interruptTimeoutMs;
    this.events=[];this.turns=new Map();this.waiters=new Map();this.deltaText=new Map();this.threadId=null;
    this.eventSinks=new Set();
    this.knownTurns=new Set();this.startedTurns=new Set();this.startWaiters=new Map();this.startPending=false;this.earlyNotifications=[];
    this.turnAdmission='idle';this.activeTurnId=null;this.interrupts=new Map();
    this.threadAdmission='idle';this.threadPending=false;this.threadAdmissionObserved=false;this.accountBinding=null;this.confirmedEffort=undefined;
    // The default can never grant writes, including in isolated lifecycle tests.
    this.sandboxPolicy=freezePolicy({type:'readOnly',networkAccess:false});
    client.on('notification',message=>this.observe(message));
    client.on('request',message=>this.handleRequest(message));
    client.on('failure',error=>this.rejectWaiters(error));
    client.on('closed',()=>this.rejectWaiters(new HarnessError('process-exit','Runtime ended before turn confirmation')));
  }
  event(type,fields={}) {
    if(this.events.length>=4096){this.client.fail(new HarnessError('protocol','Runtime event limit exceeded'));return;}
    const event={schemaVersion:1,sequence:this.events.length+1,attemptId:this.attemptId,type,...fields};this.events.push(event);
    for(const sink of [...this.eventSinks])try{sink(event);}catch{this.client.fail(new HarnessError('normalization','Runtime normalization rejected an observation'));}
    return event;
  }
  subscribe(sink) {
    if(typeof sink!=='function'||this.eventSinks.size>=16)throw new HarnessError('contract','Invalid or excessive session event subscribers');
    this.eventSinks.add(sink);return ()=>this.eventSinks.delete(sink);
  }
  async initialize() {
    const result=await this.client.request('initialize',{clientInfo:{name:'orchestrix_probe',title:'Orchestrix runtime experiment',version:'0.1.0'},capabilities:{experimentalApi:false}});
    this.client.notify('initialized');this.event('runtime.initialized');
    return {initialized:Boolean(result),serverInfoObserved:Boolean(result?.userAgent||result?.serverInfo)};
  }
  async discover(bounds={}) {
    const options=catalogOptions(bounds);
    this.accountBinding=null;
    const account=await this.client.request('account/read',{refreshToken:false});
    const type=account?.account?.type??null;
    // Retain only an in-memory comparison hash. No token, email, account ID or
    // binding is returned by discovery or added to reportable events.
    const identity=account?.account;
    const email=typeof identity?.email==='string'&&identity.email.trim()?identity.email.trim():null;
    const accountId=typeof identity?.accountId==='string'&&identity.accountId?identity.accountId:null;
    if(type==='chatgpt'&&(email||accountId))this.accountBinding=createHash('sha256').update(JSON.stringify({type,email,accountId,planType:identity.planType??null})).digest('hex');
    const auth={type:['chatgpt','apiKey','chatgptAuthTokens'].includes(type)?type:type?'other':null,authenticated:Boolean(account?.account),requiresOpenaiAuth:account?.requiresOpenaiAuth===true,subscriptionGate:type==='chatgpt'&&account?.requiresOpenaiAuth===true};
    this.event('connection.observed',{authType:auth.type,authenticated:auth.authenticated});
    const connection={authentication:auth.authenticated?'authenticated':'unauthenticated',billing:type==='chatgpt'?'subscription':type==='apiKey'?'api':'unknown',availability:'unknown',bindingObserved:this.accountBinding!==null};
    if(!auth.subscriptionGate)return {auth,models:[],availability:null,connection,catalog:{source:'model/list',scope:'visible-models',includeHidden:false,completeness:'unknown',pages:0,modelCount:0,reason:'blocked-auth'}};
    const {models,catalog}=await discoverModelCatalog(this.client,options);
    let availability=null;
    try{const result=await this.client.request('account/rateLimits/read',{supportsLunaReserve:false,excludeResetCreditDetails:true},5000);availability=typeof result?.ordinaryUsageAllowed==='boolean'?result.ordinaryUsageAllowed:null;}catch{}
    connection.availability=availability===true?'available':availability===false?'unavailable':'unknown';
    return {auth,models,availability,connection,catalog};
  }
  async inspectEndpoints(cwd) {
    const result=await this.client.request('config/read',{cwd,includeLayers:false});
    const config=result?.config;
    // Unknown flattened config keys may be absent: absence cannot validate the route.
    const validMcpShape=config?.mcp_servers===undefined||(config?.mcp_servers!==null&&typeof config.mcp_servers==='object'&&!Array.isArray(config.mcp_servers));
    const names=validMcpShape?Object.keys(config?.mcp_servers??{}):[];
    this.mcpDisableOverrides=validMcpShape&&names.length<=32&&names.every(name=>/^[A-Za-z0-9_-]{1,80}$/.test(name))?names.map(name=>`mcp_servers.${name}.enabled=false`):null;
    const servers=validMcpShape?Object.values(config?.mcp_servers??{}):[];
    const resources={hooksDisabled:config?.features?.hooks===false,pluginsDisabled:config?.features?.plugins===false,appsDisabled:config?.features?.apps===false,nativeDelegationDisabled:config?.features?.multi_agent===false,notifyDisabled:Array.isArray(config?.notify)&&config.notify.length===0,mcpConfiguredCount:validMcpShape?servers.length:null,mcpDisabled:validMcpShape&&servers.every(server=>server?.enabled===false)};
    const noApiEndpointOverride=config?.openai_base_url===undefined||config.openai_base_url===null||config.openai_base_url==='';
    const windowsSandbox=['mxc','elevated','unelevated'].includes(config?.windows?.sandbox)?config.windows.sandbox:null;
    return {providerOpenai:config?.model_provider==='openai',officialEndpointConfirmed:config?.chatgpt_base_url==='https://chatgpt.com/backend-api/'&&noApiEndpointOverride,windowsSandbox,resources};
  }
  startThread({cwd,model,effort,sandbox='read-only',ephemeral=true}) {
    return this.establishThread('thread/start',{cwd,model,effort,sandbox,ephemeral});
  }
  resumeThread({threadId,cwd,model,effort,sandbox='workspace-write'}) {
    return this.establishThread('thread/resume',{threadId,cwd,model,effort,sandbox,ephemeral:false});
  }
  async establishThread(method,{threadId,cwd,model,effort,sandbox,ephemeral}) {
    if(!['thread/start','thread/resume'].includes(method))throw new HarnessError('arguments','Unsupported thread admission operation');
    if(this.threadId||this.threadPending||this.threadAdmission!=='idle'||this.startPending||this.turnAdmission!=='idle'||this.activeTurnId)throw new HarnessError('contract','Thread admission requires a new idle session');
    if(this.client.failure||this.client.closed)throw this.client.failure||new HarnessError('transport','Runtime is closed');
    if(!validIdentity(cwd)||!isAbsolute(cwd)||!validIdentity(model)||!validIdentity(effort)||!['read-only','workspace-write'].includes(sandbox)||typeof ephemeral!=='boolean'||(method==='thread/resume'&&!validIdentity(threadId)))throw new HarnessError('arguments','Invalid bounded thread configuration');
    const writable=sandbox==='workspace-write';
    const params={cwd,model,modelProvider:'openai',approvalPolicy:'never',approvalsReviewer:'user',sandbox,
      baseInstructions:writable?'This is a bounded disposable-worktree probe. Follow only its prompt. Restrict file changes to the configured workspace. No network, plugins, other sessions or delegation.':'This is a bounded protocol probe. Follow only the probe prompt. Do not use tools or access files, network, plugins, or other sessions.',
      developerInstructions:writable?'Only perform the bounded worktree task. No network, plugins, delegation or access to other projects.':'Respond only to the probe prompt. No tools or delegation. No filesystem access.',
      config:{web_search:'disabled',mcp_servers:{},model_reasoning_effort:effort,...(writable?{sandbox_workspace_write:{network_access:false,writable_roots:[],exclude_tmpdir_env_var:true,exclude_slash_tmp:true}}:{})},
      ...(method==='thread/start'?{ephemeral}:{threadId}),
    };
    this.threadPending=true;this.threadAdmission='starting';this.threadAdmissionObserved=false;
    try {
      const result=await this.client.request(method,params);
      if(this.client.failure||this.client.closed)throw this.client.failure||new HarnessError('transport','Runtime ended during thread admission');
      const resolved=result?.sandbox;
      const roots=resolved?.writableRoots;
      const validRoots=!writable||(Array.isArray(roots)&&roots.length<=32&&roots.every(root=>samePath(root,cwd)));
      const validTmp=!writable||(resolved.excludeTmpdirEnvVar===true&&resolved.excludeSlashTmp===true);
      const validThread=validIdentity(result?.thread?.id)&&(method==='thread/start'||result.thread.id===threadId)&&result.thread.ephemeral===ephemeral&&result.thread.status?.type==='idle'&&(!result.thread.sessionId||result.thread.sessionId===result.thread.id)&&(!result.thread.forkedFromId)&&(!result.thread.turns||Array.isArray(result.thread.turns)&&result.thread.turns.every(turn=>turn?.status!=='inProgress'));
      if(!validThread||!samePath(result.cwd,cwd)||!samePath(result.thread.cwd,cwd)||result.model!==model||result.reasoningEffort!==effort||result.modelProvider!=='openai'||(result.thread.modelProvider!==undefined&&result.thread.modelProvider!=='openai')||result.approvalPolicy!=='never'||result.approvalsReviewer!=='user'||resolved?.type!==(writable?'workspaceWrite':'readOnly')||resolved.networkAccess!==false||!validRoots||!validTmp)throw new HarnessError('policy','Runtime did not confirm requested execution boundaries');
      this.threadId=result.thread.id;this.threadAdmission='owned';this.sandboxPolicy=freezePolicy(structuredClone(resolved));this.confirmedEffort=result.reasoningEffort;
      this.configuration={threadId:this.threadId,ephemeral,requested:{threadId:method==='thread/resume'?threadId:null,cwd,model,effort,provider:'openai',sandbox,networkAccess:false,writableRoots:writable?[]:null,approval:'never',reviewer:'user',ephemeral,excludeTmpdirEnvVar:writable?true:null,excludeSlashTmp:writable?true:null},resolvedConfiguration:{threadId:this.threadId,cwd:result.cwd,model:result.model,effort:result.reasoningEffort,provider:result.modelProvider,sandbox:resolved.type,sandboxPolicy:structuredClone(resolved),networkAccess:resolved.networkAccess,writableRoots:writable?[...roots]:null,approval:result.approvalPolicy,reviewer:result.approvalsReviewer,ephemeral,cwdMatched:true,excludeTmpdirEnvVar:writable?resolved.excludeTmpdirEnvVar:null,excludeSlashTmp:writable?resolved.excludeSlashTmp:null}};
      this.event(method==='thread/resume'?'session.resumed':'session.started',{threadId:this.threadId,model:result.model,sandbox:resolved.type});
      return this.configuration;
    } catch(error) {
      const notSubmitted=error instanceof HarnessError&&['arguments','outbound-limit'].includes(error.kind)&&error.metadata.dispatch==='not-written';
      if((error.kind==='rpc'||notSubmitted)&&!this.threadAdmissionObserved&&!this.client.failure&&!this.client.closed)this.threadAdmission='idle';
      else {this.threadAdmission='uncertain';this.event('attempt.observation.unknown',{phase:method,reason:error.kind||'local-error'});}
      throw error;
    } finally {this.threadPending=false;}
  }
  async startTurn(text,{effort}={}) {
    if(!this.threadId||this.startPending||this.turnAdmission!=='idle')throw new HarnessError('contract','A thread with no active or uncertain turn is required');
    if(this.client.failure||this.client.closed)throw this.client.failure||new HarnessError('transport','Runtime is closed');
    if(this.threadAdmission==='owned'&&effort!==undefined&&effort!==this.confirmedEffort)throw new HarnessError('policy','Turn effort must match the confirmed bounded thread configuration');
    this.startPending=true;this.turnAdmission='starting';
    try {
      const result=await this.client.request('turn/start',{threadId:this.threadId,input:[{type:'text',text,text_elements:[]}],effort:effort??this.confirmedEffort,sandboxPolicy:structuredClone(this.sandboxPolicy),approvalPolicy:'never',approvalsReviewer:'user'});
      if(this.client.failure||this.client.closed)throw this.client.failure||new HarnessError('transport','Runtime ended during turn start');
      if(typeof result?.turn?.id!=='string'||!result.turn.id||result.turn.id.length>512||this.knownTurns.has(result.turn.id))throw new HarnessError('protocol','Runtime did not return a new valid turn ID');
      this.knownTurns.add(result.turn.id);this.activeTurnId=result.turn.id;this.turnAdmission='active';this.startPending=false;
      this.event('attempt.turn.accepted',{turnId:result.turn.id});
      for(const message of this.earlyNotifications.splice(0))this.observe(message);
      return result.turn.id;
    } catch(error) {
      // A local pre-write rejection proves this start was not submitted. Once a
      // transport has failed, even a refused new frame cannot recover admission.
      const notSubmitted=error instanceof HarnessError&&['arguments','outbound-limit'].includes(error.kind)&&error.metadata.dispatch==='not-written';
      if((error.kind==='rpc'||notSubmitted)&&!this.earlyNotifications.length&&!this.client.failure&&!this.client.closed)this.turnAdmission='idle';
      else {this.turnAdmission='uncertain';this.event('attempt.observation.unknown',{phase:'turn-start',reason:error.kind||'local-error'});}
      throw error;
    } finally {this.startPending=false;this.earlyNotifications=[];}
  }
  observe({method,params={}}) {
    if(this.threadPending&&method==='thread/started')this.threadAdmissionObserved=true;
    if(params?.threadId!==this.threadId||!this.threadId)return;
    const turnNotification=['turn/started','turn/completed'].includes(method);
    const deltaNotification=method==='item/agentMessage/delta';
    const turnId=turnNotification?params?.turn?.id:params?.turnId;
    const alternatePresent=turnNotification?Object.hasOwn(params,'turnId'):deltaNotification&&params?.turn!=null&&Object.hasOwn(params.turn,'id');
    const alternateId=turnNotification?params.turnId:params?.turn?.id;
    if(alternatePresent&&alternateId!==turnId){this.client.fail(new HarnessError('protocol','Contradictory turn identities in runtime notification'));return;}
    if((turnNotification||deltaNotification)&&!this.knownTurns.has(turnId)) {
      if(this.startPending) {
        if(this.earlyNotifications.length>=128)this.client.fail(new HarnessError('protocol','Pre-ACK notification limit exceeded'));
        else this.earlyNotifications.push({method,params});
      }
      return;
    }
    if(method==='turn/started') {
      if(this.turns.has(turnId)||this.startedTurns.has(turnId))return;
      this.startedTurns.add(turnId);this.event('attempt.turn.started',{turnId});
      const waiters=this.startWaiters.get(turnId);if(waiters)for(const waiter of [...waiters])waiter.resolve({started:true});
    }
    else if(method==='item/agentMessage/delta') {
      const id=params.turnId;
      const value=(this.deltaText.get(id)||'')+String(params.delta||'');
      if(Buffer.byteLength(value)>65536){this.client.fail(new HarnessError('protocol','Probe response exceeded limit'));return;}
      this.deltaText.set(id,value);
      this.event('response.delta',{turnId:id,bytes:Buffer.byteLength(String(params.delta||''))});
    } else if(method==='turn/completed') {
      const turn=params.turn;
      if(!turn?.id||!['completed','failed','interrupted'].includes(turn.status)){this.client.fail(new HarnessError('protocol','Invalid terminal turn notification'));return;}
      const knownErrors=['usageLimitExceeded','rateLimitExceeded','sessionBudgetExceeded','contextWindowExceeded','serverOverloaded','internalServerError','unauthorized','badRequest','sandboxError'];
      const nativeError=typeof turn.error?.codexErrorInfo==='string'?turn.error.codexErrorInfo:turn.error?.codexErrorInfo?.type;
      const outcome={turnId:turn.id,status:turn.status,errorKind:knownErrors.includes(nativeError)?nativeError:turn.error?'runtime-error':null};
      if(this.turns.has(turn.id)) {
        if(this.turns.get(turn.id).status!==outcome.status)this.client.fail(new HarnessError('protocol','Conflicting terminal turn notification'));
        return;
      }
      this.turns.set(turn.id,outcome);this.event('attempt.turn.ended',outcome);
      if(this.activeTurnId===turn.id){this.activeTurnId=null;this.turnAdmission='idle';}
      const waiters=this.waiters.get(turn.id);if(waiters)for(const waiter of [...waiters])waiter.resolve(outcome);
      const startWaiters=this.startWaiters.get(turn.id);if(startWaiters)for(const waiter of [...startWaiters])waiter.resolve({started:false,terminal:outcome});
    } else this.event('runtime.notification',{method});
  }
  handleRequest({id,method,params={}}) {
    this.event('permission.observed',{requestId:id,kind:method,turnId:params?.turnId??null});
    try {
      if(params?.threadId!==this.threadId)this.client.rejectRequest(id);
      else if(['item/commandExecution/requestApproval','item/fileChange/requestApproval'].includes(method))this.client.reply(id,{decision:'decline'});
      else if(method==='item/permissions/requestApproval')this.client.reply(id,{permissions:{},scope:'turn'});
      else this.client.rejectRequest(id);
      this.event('permission.denied',{requestId:id,kind:method});
    } catch(error){this.client.fail(error);}
  }
  waitForTurn(id,timeoutMs=this.turnTimeoutMs) {
    return this.subscribeTurn(id,timeoutMs).promise;
  }
  waitForTurnStart(id,timeoutMs=this.interruptTimeoutMs) {
    return this.subscribeTurnStart(id,timeoutMs).promise;
  }
  subscribeTurnStart(id,timeoutMs=this.interruptTimeoutMs) {
    const immediate=promise=>({promise,cancel:()=>{}});
    const rejected=error=>({...immediate(Promise.reject(error)),error});
    if(!validIdentity(id)||!this.knownTurns.has(id))return rejected(new HarnessError('contract','Cannot observe the start of an unowned turn'));
    if(!Number.isInteger(timeoutMs)||timeoutMs<1||timeoutMs>2147483647)return rejected(new HarnessError('contract','A bounded positive turn-start timeout is required'));
    if(this.turns.has(id))return immediate(Promise.resolve(this.startedTurns.has(id)?{started:true}:{started:false,terminal:this.turns.get(id)}));
    if(this.client.failure||this.client.closed) {
      if(this.activeTurnId===id)this.turnAdmission='uncertain';
      this.event('attempt.observation.unknown',{turnId:id,phase:'turn-start-observation',reason:'transport-unavailable'});
      return rejected(this.client.failure||new HarnessError('process-exit','Runtime is closed'));
    }
    if(this.startedTurns.has(id))return immediate(Promise.resolve({started:true}));
    const group=this.startWaiters.get(id)||new Set();
    if(group.size+(this.waiters.get(id)?.size||0)>=16)return rejected(new HarnessError('contract','Turn observer limit exceeded'));
    let resolve,reject;
    const promise=new Promise((resolvePromise,rejectPromise)=>{resolve=resolvePromise;reject=rejectPromise;});
    let settled=false;
    const finish=(error,outcome)=>{
      if(settled)return;settled=true;clearTimeout(waiter.timer);group.delete(waiter);
      if(!group.size&&this.startWaiters.get(id)===group)this.startWaiters.delete(id);
      if(error)reject(error);else resolve(outcome);
    };
    const waiter={resolve:outcome=>finish(null,outcome),reject:error=>finish(error)};
    group.add(waiter);this.startWaiters.set(id,group);
    waiter.timer=setTimeout(()=>{
      if(this.activeTurnId===id)this.turnAdmission='uncertain';
      this.event('attempt.observation.unknown',{turnId:id,phase:'turn-start-observation',reason:'timeout'});
      waiter.reject(new HarnessError('timeout','Turn start was not confirmed'));
    },timeoutMs);
    return {promise,cancel:error=>waiter.reject(error??new HarnessError('cancelled','Turn-start observer cancelled'))};
  }
  subscribeTurn(id,timeoutMs) {
    const immediate=promise=>({promise,cancel:()=>{}});
    const rejected=error=>({...immediate(Promise.reject(error)),error});
    if(this.turns.has(id))return immediate(Promise.resolve(this.turns.get(id)));
    if(!this.knownTurns.has(id))return rejected(new HarnessError('contract','Cannot wait for an unowned turn'));
    if(!Number.isInteger(timeoutMs)||timeoutMs<1||timeoutMs>2147483647)return rejected(new HarnessError('contract','A bounded positive turn timeout is required'));
    if(this.client.failure||this.client.closed) {
      this.event('attempt.observation.unknown',{turnId:id,reason:'transport-unavailable'});
      return rejected(this.client.failure||new HarnessError('process-exit','Runtime is closed'));
    }
    const group=this.waiters.get(id)||new Set();
    if(group.size+(this.startWaiters.get(id)?.size||0)>=16)return rejected(new HarnessError('contract','Turn observer limit exceeded'));
    let resolve,reject;
    const promise=new Promise((resolvePromise,rejectPromise)=>{resolve=resolvePromise;reject=rejectPromise;});
    let settled=false;
    const finish=(error,outcome)=>{
      if(settled)return;settled=true;clearTimeout(waiter.timer);group.delete(waiter);
      if(!group.size&&this.waiters.get(id)===group)this.waiters.delete(id);
      if(error)reject(error);else resolve(outcome);
    };
    const waiter={resolve:outcome=>finish(null,outcome),reject:error=>finish(error)};
    group.add(waiter);this.waiters.set(id,group);
    waiter.timer=setTimeout(()=>{
      if(this.activeTurnId===id)this.turnAdmission='uncertain';
      this.event('attempt.observation.unknown',{turnId:id,reason:'timeout'});
      waiter.reject(new HarnessError('timeout','Turn completion was not confirmed'));
    },timeoutMs);
    return {promise,cancel:waiter.reject};
  }
  interrupt(id) {
    if(this.turns.has(id))return Promise.resolve(this.turns.get(id));
    if(!this.threadId||!this.knownTurns.has(id))return Promise.reject(new HarnessError('contract','Cannot interrupt an unowned turn'));
    if(this.interrupts.has(id))return this.interrupts.get(id);
    const operation=this.performInterrupt(id).then(outcome=>{this.interrupts.delete(id);return outcome;},error=>{this.interrupts.delete(id);throw error;});
    this.interrupts.set(id,operation);return operation;
  }
  async performInterrupt(id) {
    const waiter=this.subscribeTurn(id,this.interruptTimeoutMs);
    // Consume a deadline/transport rejection immediately, even while the RPC ACK is pending.
    const terminal=waiter.promise.then(outcome=>({outcome}),error=>({error}));
    if(waiter.error)throw waiter.error;
    try {
      this.event('attempt.interrupt.sent',{turnId:id});
      await this.client.request('turn/interrupt',{threadId:this.threadId,turnId:id},this.interruptTimeoutMs);
      this.event('attempt.interrupt.requested',{turnId:id});
    } catch(error) {
      waiter.cancel(error);
      if(this.turns.has(id))return this.turns.get(id);
      if(this.activeTurnId===id)this.turnAdmission='uncertain';
      this.event('attempt.observation.unknown',{turnId:id,phase:'turn-interrupt',reason:error.kind||'local-error',dispatch:error.metadata?.dispatch==='not-written'?'not-written':'unknown'});
      throw error;
    }
    const result=await terminal;if(result.error)throw result.error;return result.outcome;
  }
  rejectWaiters(error) {
    if(this.activeTurnId&&!this.turns.has(this.activeTurnId)) {
      this.turnAdmission='uncertain';
      if(!this.waiters.has(this.activeTurnId)&&!this.startWaiters.has(this.activeTurnId))this.event('attempt.observation.unknown',{turnId:this.activeTurnId,reason:'transport-unavailable'});
    }
    for(const [id,waiters] of [...this.waiters]) {
      this.event('attempt.observation.unknown',{turnId:id,reason:'transport-unavailable'});
      for(const waiter of [...waiters])waiter.reject(error);
    }
    for(const [id,waiters] of [...this.startWaiters]) {
      this.event('attempt.observation.unknown',{turnId:id,phase:'turn-start-observation',reason:'transport-unavailable'});
      for(const waiter of [...waiters])waiter.reject(error);
    }
  }
}
