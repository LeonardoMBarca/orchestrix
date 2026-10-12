import {HarnessError} from './json-rpc.mjs';
import {createHash} from 'node:crypto';
import {isAbsolute,normalize} from 'node:path';

const validIdentity=value=>typeof value==='string'&&value.length>0&&value.length<=512&&!/[\u0000-\u001f]/.test(value);
const samePath=(left,right)=>typeof left==='string'&&typeof right==='string'&&isAbsolute(left)&&isAbsolute(right)&&(process.platform==='win32'?normalize(left).toLowerCase()===normalize(right).toLowerCase():normalize(left)===normalize(right));
const freezePolicy=policy=>Object.freeze(Object.fromEntries(Object.entries(policy).map(([key,value])=>[key,Array.isArray(value)?Object.freeze([...value]):value&&typeof value==='object'?freezePolicy(value):value])));

/** Deliberately small experiment. Provider messages never become the Core domain. */
export class CodexSession {
  constructor(client,{attemptId='probe-attempt-1',turnTimeoutMs=45000,interruptTimeoutMs=10000}={}) {
    this.client=client;this.attemptId=attemptId;this.turnTimeoutMs=turnTimeoutMs;this.interruptTimeoutMs=interruptTimeoutMs;
    this.events=[];this.turns=new Map();this.waiters=new Map();this.deltaText=new Map();this.threadId=null;
    this.knownTurns=new Set();this.startPending=false;this.earlyNotifications=[];
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
    const event={schemaVersion:1,sequence:this.events.length+1,attemptId:this.attemptId,type,...fields};this.events.push(event);return event;
  }
  async initialize() {
    const result=await this.client.request('initialize',{clientInfo:{name:'orchestrix_probe',title:'Orchestrix runtime experiment',version:'0.1.0'},capabilities:{experimentalApi:false}});
    this.client.notify('initialized');this.event('runtime.initialized');
    return {initialized:Boolean(result),serverInfoObserved:Boolean(result?.userAgent||result?.serverInfo)};
  }
  async discover() {
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
    if(!auth.subscriptionGate)return {auth,models:[],availability:null};
    const response=await this.client.request('model/list',{includeHidden:false,limit:100});
    const models=(response?.data||[]).filter(model=>model&&typeof model.model==='string').map(model=>({
      id:String(model.id),model:model.model,isDefault:Boolean(model.isDefault),
      defaultEffort:model.defaultReasoningEffort??null,
      efforts:(model.supportedReasoningEfforts||[]).map(value=>typeof value==='string'?value:value.reasoningEffort).filter(value=>typeof value==='string'),
    }));
    let availability=null;
    try{const result=await this.client.request('account/rateLimits/read',{supportsLunaReserve:false,excludeResetCreditDetails:true},5000);availability=typeof result?.ordinaryUsageAllowed==='boolean'?result.ordinaryUsageAllowed:null;}catch{}
    return {auth,models,availability};
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
    return {providerOpenai:config?.model_provider==='openai',officialEndpointConfirmed:config?.chatgpt_base_url==='https://chatgpt.com/backend-api/'&&noApiEndpointOverride,resources};
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
    if(method==='turn/started')this.event('attempt.turn.started',{turnId:params.turn?.id});
    else if(method==='item/agentMessage/delta') {
      const id=params.turnId;
      const value=(this.deltaText.get(id)||'')+String(params.delta||'');
      if(Buffer.byteLength(value)>65536){this.client.fail(new HarnessError('protocol','Probe response exceeded limit'));return;}
      this.deltaText.set(id,value);
      this.event('response.delta',{turnId:id,bytes:Buffer.byteLength(String(params.delta||''))});
    } else if(method==='turn/completed') {
      const turn=params.turn;
      if(!turn?.id||!['completed','failed','interrupted'].includes(turn.status)){this.client.fail(new HarnessError('protocol','Invalid terminal turn notification'));return;}
      const knownErrors=['usageLimitExceeded','contextWindowExceeded','serverOverloaded','internalServerError','unauthorized','badRequest','sandboxError'];
      const outcome={turnId:turn.id,status:turn.status,errorKind:knownErrors.includes(turn.error?.codexErrorInfo)?turn.error.codexErrorInfo:turn.error?'runtime-error':null};
      if(this.turns.has(turn.id)) {
        if(this.turns.get(turn.id).status!==outcome.status)this.client.fail(new HarnessError('protocol','Conflicting terminal turn notification'));
        return;
      }
      this.turns.set(turn.id,outcome);this.event('attempt.turn.ended',outcome);
      if(this.activeTurnId===turn.id){this.activeTurnId=null;this.turnAdmission='idle';}
      const waiters=this.waiters.get(turn.id);if(waiters)for(const waiter of [...waiters])waiter.resolve(outcome);
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
    if(group.size>=16)return rejected(new HarnessError('contract','Turn observer limit exceeded'));
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
      await this.client.request('turn/interrupt',{threadId:this.threadId,turnId:id},this.interruptTimeoutMs);
      this.event('attempt.interrupt.requested',{turnId:id});
    } catch(error) {
      waiter.cancel(error);
      if(this.turns.has(id))return this.turns.get(id);
      if(this.activeTurnId===id)this.turnAdmission='uncertain';
      this.event('attempt.observation.unknown',{turnId:id,phase:'turn-interrupt',reason:error.kind||'local-error'});
      throw error;
    }
    const result=await terminal;if(result.error)throw result.error;return result.outcome;
  }
  rejectWaiters(error) {
    if(this.activeTurnId&&!this.turns.has(this.activeTurnId)) {
      this.turnAdmission='uncertain';
      if(!this.waiters.has(this.activeTurnId))this.event('attempt.observation.unknown',{turnId:this.activeTurnId,reason:'transport-unavailable'});
    }
    for(const [id,waiters] of [...this.waiters]) {
      this.event('attempt.observation.unknown',{turnId:id,reason:'transport-unavailable'});
      for(const waiter of [...waiters])waiter.reject(error);
    }
  }
}
