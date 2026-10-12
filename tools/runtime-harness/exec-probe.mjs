#!/usr/bin/env node
// OX-001 comparison experiment, not a production adapter. Live is explicit opt-in.
import {spawn,execFile} from 'node:child_process';
import {EventEmitter} from 'node:events';
import {promisify} from 'node:util';
import {dirname,join,resolve,isAbsolute} from 'node:path';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {mkdir,writeFile} from 'node:fs/promises';
import {randomUUID} from 'node:crypto';
import {HarnessError} from './src/json-rpc.mjs';
import {cleanWindowsJobExit} from './src/windows-job-process.mjs';
import {runtimeEnvironment,codexLaunchArguments,supportsCodexVersion} from './probe.mjs';
import {createWorktreeFixture,inspectAttempt,trackedSnapshot,connect} from './worktree-probe.mjs';

const execute=promisify(execFile),root=dirname(fileURLToPath(import.meta.url));
const validId=value=>typeof value==='string'&&value.length>0&&value.length<=512&&!/[\u0000-\u001f]/.test(value);
const boundedMs=value=>Number.isInteger(value)&&value>=1&&value<=2147483647;
const safeLabel=value=>typeof value==='string'&&/^[\w.:-]{1,100}$/.test(value)?value:'unknown';
const knownItemTypes=new Set(['agent_message','reasoning','command_execution','file_change','mcp_tool_call','web_search','todo_list','error']);

/** One CLI invocation owns one stream, one thread and at most one turn. No raw logs survive. */
export class ExecJsonProcess extends EventEmitter {
  constructor({command,args=[],cwd,env=runtimeEnvironment(),expectedThreadId=null,timeoutMs=90000,startTimeoutMs=15000,maxMessageBytes=1024*1024,maxResponseBytes=65536,maxEvents=4096,spawnProcess}={}) {
    super();
    if(!boundedMs(timeoutMs)||!boundedMs(startTimeoutMs)||[maxMessageBytes,maxResponseBytes,maxEvents].some(value=>!Number.isSafeInteger(value)||value<1)||expectedThreadId!==null&&!validId(expectedThreadId))throw new HarnessError('arguments','Invalid bounded CLI observation configuration');
    this.expectedThreadId=expectedThreadId;this.maxMessageBytes=maxMessageBytes;this.maxResponseBytes=maxResponseBytes;this.maxEvents=maxEvents;
    this.threadId=null;this.started=false;this.terminal=null;this.failure=null;this.closed=false;this.parentExited=false;
    this.messages=0;this.stderrBytes=0;this.responseBytes=0;this.unknownEvents=0;this.items=new Map();this.itemCounts=Object.create(null);this.agentText='';this.usage=null;
    this.buffer='';this.decoder=new TextDecoder('utf-8',{fatal:true});this.events=[];this.startWaiters=new Set();this.stopPromise=null;this.cancelRequested=false;
    this.closedPromise=new Promise(resolveClosed=>{this.resolveClosed=resolveClosed;});
    this.child=spawnProcess?spawnProcess({command,args,cwd,env}):spawn(command,args,{cwd,env,stdio:['pipe','pipe','pipe'],shell:false,windowsHide:true});
    this.child.stdout.on('data',chunk=>this.consume(chunk));
    this.child.stderr.on('data',chunk=>{this.stderrBytes+=chunk.length;if(this.stderrBytes>8*1024*1024)this.fail(new HarnessError('protocol','CLI stderr drain limit exceeded'));});
    this.child.stdin.on('error',()=>{if(!this.closed)this.fail(new HarnessError('transport','CLI stdin failed'));});
    for(const stream of [this.child.stdout,this.child.stderr])stream.on('error',()=>{if(!this.closed)this.fail(new HarnessError('transport','CLI output failed'));});
    this.child.stdout.on('end',()=>{
      if(this.failure)return;
      try{this.buffer+=this.decoder.decode();}catch{this.fail(new HarnessError('protocol','CLI ended with incomplete UTF-8'));return;}
      if(this.buffer.trim())this.fail(new HarnessError('protocol','CLI ended with incomplete JSONL'));
    });
    this.child.on('error',()=>this.fail(new HarnessError('launch','CLI process unavailable')));
    this.child.on('exit',(code,signal)=>{
      this.parentExited=true;this.exit={code,signal,parentExited:true,treeTermination:'unknown',stdioClosed:false};
      if(!this.closed)this.drainTimer=setTimeout(()=>this.finish(false),250);
    });
    this.child.on('close',(code,signal)=>{this.exit={...this.exit,code,signal,parentExited:this.parentExited};this.finish(true);});
    const armDeadlines=()=>{
      if(this.closed||this.failure)return;
      this.deadline=setTimeout(()=>this.fail(new HarnessError('timeout','CLI invocation did not terminate before its deadline')),timeoutMs);
      if(!this.started&&!this.terminal)this.startDeadline=setTimeout(()=>this.fail(new HarnessError('timeout','CLI turn start was not observed')),startTimeoutMs);
    };
    // Bridge startup has its own bounded deadline. Turn deadlines begin only once
    // the runtime, rather than its helper process, is ready inside the owned Job.
    if(this.child.ready)this.child.ready.then(armDeadlines,()=>this.fail(new HarnessError('launch','Owned Windows Job launch failed')));
    else this.child.once('spawn',armDeadlines);
    // No input is accepted after the bounded prompt passed in argv.
    this.child.stdin.end();
  }
  event(type,fields={}) {
    if(this.events.length>=this.maxEvents){this.fail(new HarnessError('protocol','CLI event limit exceeded'));return;}
    this.events.push({sequence:this.events.length+1,type,...fields});
  }
  consume(chunk) {
    if(this.failure||this.closed)return;
    try{this.buffer+=this.decoder.decode(chunk,{stream:true});}catch{this.fail(new HarnessError('protocol','CLI emitted invalid UTF-8'));return;}
    let newline;
    while((newline=this.buffer.indexOf('\n'))!==-1) {
      const line=this.buffer.slice(0,newline).replace(/\r$/,'');this.buffer=this.buffer.slice(newline+1);
      if(Buffer.byteLength(line)>this.maxMessageBytes){this.fail(new HarnessError('protocol','CLI frame limit exceeded'));return;}
      if(!line.trim())continue;
      let value;try{value=JSON.parse(line);}catch{this.fail(new HarnessError('protocol','CLI emitted malformed JSON'));return;}
      try{this.observe(value);}catch(error){this.fail(error instanceof HarnessError?error:new HarnessError('protocol','Invalid CLI event'));}
      if(this.failure)return;
    }
    if(Buffer.byteLength(this.buffer)>this.maxMessageBytes)this.fail(new HarnessError('protocol','CLI frame limit exceeded'));
  }
  observe(value) {
    if(!value||typeof value!=='object'||Array.isArray(value)||typeof value.type!=='string'||!/^[-\w.]{1,100}$/.test(value.type))throw new HarnessError('protocol','Invalid CLI event envelope');
    if(++this.messages>this.maxEvents)throw new HarnessError('protocol','CLI event limit exceeded');
    if(value.type==='thread.started') {
      if(!validId(value.thread_id)||this.threadId!==null||this.expectedThreadId!==null&&value.thread_id!==this.expectedThreadId)throw new HarnessError('protocol','CLI thread identity did not match its invocation');
      this.threadId=value.thread_id;this.event('session.observed',{sameExpectedThread:this.expectedThreadId===null?null:true});return;
    }
    if(value.type==='turn.started') {
      if(!this.threadId||this.started||this.terminal)throw new HarnessError('protocol','CLI turn start out of order');
      this.started=true;clearTimeout(this.startDeadline);this.event('attempt.turn.started');
      for(const waiter of [...this.startWaiters])waiter.resolve({started:true});return;
    }
    if(value.type==='turn.completed'||value.type==='turn.failed') {
      if(!this.threadId||this.terminal||value.type==='turn.completed'&&!this.started)throw new HarnessError('protocol','CLI terminal out of order');
      if(value.type==='turn.completed') {
        const usage=value.usage;
        if(!usage||['input_tokens','cached_input_tokens','output_tokens'].some(key=>!Number.isSafeInteger(usage[key])||usage[key]<0)||(usage.reasoning_output_tokens!==undefined&&(!Number.isSafeInteger(usage.reasoning_output_tokens)||usage.reasoning_output_tokens<0)))throw new HarnessError('protocol','Invalid CLI usage counters');
        this.usage=Object.fromEntries(['input_tokens','cached_input_tokens','output_tokens','reasoning_output_tokens'].filter(key=>usage[key]!==undefined).map(key=>[key,usage[key]]));
      }
      this.terminal={status:value.type==='turn.completed'?'completed':'failed',errorKind:value.type==='turn.failed'?'runtime-error':null};
      clearTimeout(this.startDeadline);this.event('attempt.turn.ended',this.terminal);
      for(const waiter of [...this.startWaiters])waiter.resolve({started:false,terminal:this.terminal});return;
    }
    if(value.type.startsWith('item.')) {
      if(!['item.started','item.updated','item.completed'].includes(value.type)) {this.unknownEvents++;this.event('runtime.unknown-event');return;}
      if(!this.started||this.terminal||!validId(value.item?.id)||typeof value.item.type!=='string'||!/^[-\w]{1,100}$/.test(value.item.type))throw new HarnessError('protocol','Invalid CLI item event');
      const old=this.items.get(value.item.id);
      if(old&&old.type!==value.item.type||old?.completed)throw new HarnessError('protocol','Contradictory CLI item identity');
      if(this.items.size>=1024&&!old)throw new HarnessError('protocol','CLI item limit exceeded');
      this.items.set(value.item.id,{type:value.item.type,completed:value.type==='item.completed'});
      if(value.type==='item.completed') {
        const kind=knownItemTypes.has(value.item.type)?value.item.type:'unknown';
        this.itemCounts[kind]=(this.itemCounts[kind]||0)+1;
        if(value.item.type==='agent_message') {
          if(typeof value.item.text!=='string')throw new HarnessError('protocol','Invalid CLI agent message');
          this.responseBytes+=Buffer.byteLength(value.item.text);if(this.responseBytes>this.maxResponseBytes)throw new HarnessError('protocol','CLI response limit exceeded');
          this.agentText=value.item.text;
        }
      }
      this.event('runtime.item',{kind:knownItemTypes.has(value.item.type)?value.item.type:'unknown',completed:value.type==='item.completed'});return;
    }
    if(value.type==='error') {
      // Runtime error text can include paths, prompts or credentials. Keep none.
      this.event('runtime.error');throw new HarnessError('runtime-error','CLI reported an error');
    }
    this.unknownEvents++;this.event('runtime.unknown-event');
  }
  waitForStart(timeoutMs=15000) {
    if(!boundedMs(timeoutMs))return Promise.reject(new HarnessError('arguments','Invalid CLI start observer timeout'));
    if(this.failure)return Promise.reject(this.failure);
    if(this.started)return Promise.resolve({started:true});
    if(this.terminal)return Promise.resolve({started:false,terminal:this.terminal});
    if(this.closed)return Promise.reject(new HarnessError('process-exit','CLI closed before observed start'));
    if(this.startWaiters.size>=16)return Promise.reject(new HarnessError('contract','CLI observer limit exceeded'));
    return new Promise((resolveStart,rejectStart)=>{
      let settled=false;
      const finish=(error,value)=>{if(settled)return;settled=true;clearTimeout(waiter.timer);this.startWaiters.delete(waiter);error?rejectStart(error):resolveStart(value);};
      const waiter={resolve:value=>finish(null,value),reject:error=>finish(error)};
      waiter.timer=setTimeout(()=>{this.fail(new HarnessError('timeout','CLI turn start was not confirmed'));},timeoutMs);
      this.startWaiters.add(waiter);
    });
  }
  fail(error) {
    if(this.failure||this.closed)return;this.failure=error;this.event('attempt.observation.unknown',{reason:error.kind||'local-error'});
    for(const waiter of [...this.startWaiters])waiter.reject(error);
    void this.stop().catch(()=>{});
  }
  finish(stdioClosed) {
    if(this.closed)return;this.closed=true;clearTimeout(this.deadline);clearTimeout(this.startDeadline);clearTimeout(this.drainTimer);
    if(!this.failure&&!this.cancelRequested&&!this.terminal){this.failure=new HarnessError('terminal-missing','CLI exited without a terminal event');this.event('attempt.observation.unknown',{reason:'terminal-missing'});}
    for(const waiter of [...this.startWaiters])waiter.reject(this.failure||new HarnessError('process-exit','CLI ended before turn start'));
    this.exit={...this.exit,parentExited:this.parentExited,stdioClosed,treeTermination:this.exit?.treeTermination??'unknown'};
    if(!stdioClosed)for(const stream of [this.child.stdin,this.child.stdout,this.child.stderr])stream.destroy();
    this.resolveClosed(this.result());this.emit('closed',this.exit);
  }
  result() {
    return {passed:this.closed&&!this.failure&&!this.cancelRequested&&this.terminal?.status==='completed'&&this.exit?.code===0&&this.parentExited&&this.exit?.stdioClosed===true,threadObserved:Boolean(this.threadId),sameExpectedThread:this.expectedThreadId===null?null:this.threadId===this.expectedThreadId,startObserved:this.started,terminalStatus:this.terminal?.status??null,errorKind:this.failure?.kind??this.terminal?.errorKind??(this.closed&&this.exit?.code!==0?'process-exit':null),usage:this.usage,items:{...this.itemCounts},unknownEvents:this.unknownEvents,process:this.exit??null,events:[...this.events],metrics:{messages:this.messages,stderrBytesDrained:this.stderrBytes,responseBytes:this.responseBytes}};
  }
  wait() {return this.closedPromise;}
  async cancel({startTimeoutMs=15000,graceMs=1000}={}) {
    if(this.closed||this.terminal)return {requested:false,startObserved:this.started,terminalStatus:this.terminal?.status??null,turnInterruptedConfirmed:false};
    const start=await this.waitForStart(startTimeoutMs);
    if(!start.started||this.terminal)return {requested:false,startObserved:start.started,terminalStatus:this.terminal?.status??null,turnInterruptedConfirmed:false};
    this.cancelRequested=true;this.event('attempt.process-stop.requested');
    const process=await this.stop({graceMs});
    const job=process.supervision;
    return {requested:true,startObserved:true,processTerminationConfirmed:job?.ownershipConfirmed===true&&job.parentExited===true&&job.treeTermination==='confirmed'&&job.activeCount===0&&job.stdioClosed===true&&job.helper?.parentExited===true&&job.helper?.stdioClosed===true&&job.helper?.code===0&&job.helperStreams?.faulted===false&&job.helperStreams.stdinClosed===true&&job.helperStreams.stdoutClosed===true&&job.helperStreams.stderrClosed===true,turnInterruptedConfirmed:false,terminalStatus:this.terminal?.status??null};
  }
  stop({graceMs=1000}={}) {
    if(!Number.isSafeInteger(graceMs)||graceMs<1||graceMs>10000)return Promise.reject(new HarnessError('arguments','Invalid CLI stop grace period'));
    if(this.stopPromise)return this.stopPromise;
    this.stopPromise=(async()=>{
      let supervision;
      if(this.child.stop){supervision=await this.child.stop(graceMs);if(!this.closed)this.finish(false);}
      else if(!this.closed) {
        this.child.kill();
        let timer;await Promise.race([this.closedPromise,new Promise(resolveDelay=>{timer=setTimeout(resolveDelay,graceMs);})]);clearTimeout(timer);
        if(!this.closed){this.child.kill('SIGKILL');this.finish(false);}
      }
      if(supervision)this.exit={...this.exit,supervision};
      return this.exit??{parentExited:false,treeTermination:'unknown',supervision};
    })();return this.stopPromise;
  }
}

/** Flags apply to both initial exec and resume; no --last, paid API or bypass route. */
export function execArguments({cwd,model,effort,prompt,threadId=null,sandbox='workspace-write',windowsSandbox,mcpDisableOverrides=[]}={}) {
  if(!validId(cwd)||!isAbsolute(cwd)||!validId(model)||!validId(effort)||typeof prompt!=='string'||!prompt.trim()||Buffer.byteLength(prompt)>8192||!['read-only','workspace-write'].includes(sandbox)||(threadId!==null&&!/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(threadId)))throw new HarnessError('arguments','Invalid bounded CLI invocation');
  if(!Array.isArray(mcpDisableOverrides)||mcpDisableOverrides.length>32||mcpDisableOverrides.some(value=>typeof value!=='string'||!/^mcp_servers\.[A-Za-z0-9_-]{1,80}\.enabled=false$/.test(value)))throw new HarnessError('arguments','Invalid bounded MCP disable overrides');
  const flags=codexLaunchArguments({windowsSandbox});flags.splice(flags.indexOf('app-server'));
  return [...flags,...mcpDisableOverrides.flatMap(value=>['-c',value]),'-c','web_search="disabled"','-c',`model=${JSON.stringify(model)}`,'-c',`model_reasoning_effort=${JSON.stringify(effort)}`,'-c','approval_policy="never"','-c','approvals_reviewer="user"','-c',`sandbox_mode=${JSON.stringify(sandbox)}`,'-c','sandbox_workspace_write.network_access=false','-c','sandbox_workspace_write.writable_roots=[]','-c','sandbox_workspace_write.exclude_tmpdir_env_var=true','-c','sandbox_workspace_write.exclude_slash_tmp=true','-C',cwd,'exec',...(threadId?['resume',threadId]:[]),'--json',prompt];
}

async function supervisedFactory(windowsJob) {
  if(!windowsJob)return undefined;
  if(process.platform!=='win32')throw new HarnessError('arguments','Windows Job supervision requires Windows');
  const {spawnWindowsJobProcess}=await import('./src/windows-job-process.mjs');
  return options=>spawnWindowsJobProcess(options);
}

/** Metadata default; at most two opt-in CLI invocations, with fresh gates before each. */
export async function runExecProbe({binary=process.platform==='win32'?'codex.exe':'codex',live=false,windowsJob=process.platform==='win32'&&live,windowsSandbox,requestTimeoutMs=15000,turnTimeoutMs=90000,maxInvocations=2,artifactDirectory=join(root,'artifacts')}={}) {
  const report={schemaVersion:1,date:new Date().toISOString(),experiment:'OX-001/exec-json-comparison',liveRequested:live===true,liveStarted:false,invocations:0,status:'starting',checks:[],processes:[],cancellation:{dedicatedLiveInvocation:false,reason:'Two-invocation budget is reserved for edit and exact resume; timeout cleanup is not a successful turn interruption'},limits:['CLI events do not acknowledge resolved model/effort/sandbox configuration','CLI cancellation by owned process supervision does not prove turn.interrupted','Metadata uses app-server read-only queries; no fallback billing','Observed account binding covers metadata connections, not an atomic CLI auth lock','Runtime conversation history remains owned by Codex; not Orchestrix memory']};
  let fixture,connection,invocation;let accountBinding;
  try {
    if(typeof live!=='boolean'||typeof windowsJob!=='boolean'||!boundedMs(requestTimeoutMs)||!boundedMs(turnTimeoutMs))throw new HarnessError('arguments','Explicit boolean opt-in and bounded deadlines are required');
    if(!Number.isInteger(maxInvocations)||maxInvocations<1||maxInvocations>2)throw new HarnessError('arguments','Only one or two bounded CLI invocations are allowed');
    if(windowsSandbox!==undefined&&(process.platform!=='win32'||windowsSandbox!=='mxc'))throw new HarnessError('arguments','Explicit MXC requires Windows');
    if(live&&process.platform==='win32'&&!windowsJob)throw new HarnessError('supervision','Windows live CLI requires owned Job supervision');
    fixture=await createWorktreeFixture();report.workspace={disposable:true,retainedForInspection:true,base:fixture.base,baselineCheck:fixture.baselineCheck};
    const version=await execute(binary,['--version'],{cwd:fixture.worktree,env:runtimeEnvironment(),windowsHide:true,timeout:10000,maxBuffer:8192});
    report.runtimeVersion=version.stdout.match(/codex-cli ([\w.-]+)/)?.[1]??'unknown';
    if(!supportsCodexVersion(report.runtimeVersion))throw new HarnessError('version','CLI version not reviewed');
    async function observeGates() {
      connection=await connect(binary,fixture.worktree,requestTimeoutMs,{windowsSandbox,windowsJob});
      const metadata=connection;
      report.processes.push(...(metadata.bootstrapProcesses??[]).map(state=>({surface:'metadata-bootstrap',...state})));
      const model=metadata.discovery.models.find(value=>value.isDefault)||metadata.discovery.models[0];
      const effort=model?.efforts.includes('low')?'low':model?.efforts.includes('minimal')?'minimal':model?.defaultEffort;
      if(!model||!model.efforts.includes(effort)||!metadata.session.accountBinding)throw new HarnessError('capability','Model, effort or observable account identity missing');
      if(accountBinding&&accountBinding!==metadata.session.accountBinding)throw new HarnessError('connection-binding','Observed account metadata changed');
      accountBinding=metadata.session.accountBinding;
      report.auth=metadata.discovery.auth;report.availability=metadata.discovery.availability;report.endpoints=metadata.endpoints;
      const stop=await metadata.client.stop();report.processes.push({surface:'metadata-app-server',...stop});
      if(!stop.parentExited||stop.code!==0||metadata.client.failure||windowsJob&&!cleanWindowsJobExit(stop))throw new HarnessError('process-exit','Metadata process exit not confirmed');
      connection=null;return {model:model.model,effort,mcpDisableOverrides:metadata.session.mcpDisableOverrides??[]};
    }
    const selected=await observeGates();report.checks.push('subscription-gates-confirmed','extensions-disabled','observed-account-binding','baseline-check-failed');
    report.configuration={requested:{model:safeLabel(selected.model),effort:safeLabel(selected.effort),provider:'openai',sandbox:'workspace-write',networkAccess:false,approval:'never',reviewer:'user',ephemeral:false},resolvedConfiguration:{observed:false,reason:'exec-json-does-not-return-resolved-configuration'}};
    if(!live){report.status='metadata-passed';return report;}
    const spawnProcess=await supervisedFactory(windowsJob);const marker=`ORCHESTRIX_${randomUUID()}`;
    const prompt=`Remember this marker for a later continuation: ${marker}. Fix only src/add.mjs so add(a, b) returns the sum for integers and decimals. Preserve the function signature and arithmetic-only return expression. Do not change checks or AGENTS.md, create files, install, use network, delegate, commit, or read outside this worktree.`;
    async function executeInvocation({prompt,threadId=null,sandbox='workspace-write'}) {
      if(report.invocations>=maxInvocations)throw new HarnessError('budget','CLI invocation budget exhausted');
      const args=execArguments({cwd:fixture.worktree,...selected,prompt,threadId,sandbox,windowsSandbox});
      report.invocations++;report.liveStarted=true;
      invocation=new ExecJsonProcess({command:binary,args,cwd:fixture.worktree,env:runtimeEnvironment(),expectedThreadId:threadId,timeoutMs:turnTimeoutMs,spawnProcess});
      const result=await invocation.wait();const id=invocation.threadId,text=invocation.agentText;
      const stopped=await invocation.stop();result.process={...result.process,...stopped};
      if(windowsJob&&!cleanWindowsJobExit(stopped.supervision))result.passed=false;
      report.processes.push({surface:'exec-cli',...result.process});invocation=null;
      return {result,id,text};
    }
    const first=await executeInvocation({prompt});report.firstInvocation=first.result;
    const attempt=await inspectAttempt(fixture);await mkdir(artifactDirectory,{recursive:true});const patch=`exec-attempt-${Date.now()}.patch`;await writeFile(join(artifactDirectory,patch),attempt.diff);delete attempt.diff;report.attempt={...attempt,patch};
    if(!first.result.passed||!attempt.passed){report.status=first.result.passed?'artifact-check-failed':'exec-not-completed';return report;}
    report.checks.push('cli-terminal-and-exit-observed','diff-attempt-bound','checks-same-snapshot','main-checkout-preserved');
    if(maxInvocations<2){report.status='edit-passed-resume-not-requested';return report;}
    const refreshed=await observeGates();if(refreshed.model!==selected.model||refreshed.effort!==selected.effort)throw new HarnessError('capability','Selected model/effort changed before resume');
    selected.mcpDisableOverrides=refreshed.mcpDisableOverrides;
    const resumed=await executeInvocation({threadId:first.id,prompt:'Reply only with the exact marker I asked you to remember earlier. No tools, files, network or delegation.'});
    report.resume={invocation:resumed.result,sameThread:resumed.id===first.id,sameObservedAccount:true,priorMarkerMatched:resumed.text.trim()===marker};
    const after=await inspectAttempt(fixture);report.resume.artifactUnchanged=after.passed&&after.diffSha256===attempt.diffSha256&&JSON.stringify(after.sources)===JSON.stringify(attempt.sources);
    report.status=resumed.result.passed&&report.resume.sameThread&&report.resume.priorMarkerMatched&&report.resume.artifactUnchanged?'passed':'resume-not-proven';
    if(report.status==='passed')report.checks.push('exact-cli-thread-resumed','prior-runtime-context-observed');return report;
  } catch(error) {report.status='failed';report.error={kind:error instanceof HarnessError?error.kind:'local-error'};return report;}
  finally {
    if(connection)try{report.processes.push({surface:'metadata-app-server',...await connection.client.stop()});}catch{report.status='termination-unconfirmed';}
    if(invocation)try{report.processes.push({surface:'exec-cli',...await invocation.stop()});}catch{report.status='termination-unconfirmed';}
    if(fixture)try{report.workspace.mainUnchanged=JSON.stringify(await trackedSnapshot(fixture.main))===JSON.stringify(fixture.baseline);if(!report.workspace.mainUnchanged)report.status='main-checkout-changed';}catch{report.status='artifact-observation-failed';}
  }
}

async function main() {
  const options={};let output;const args=process.argv.slice(2);
  for(let index=0;index<args.length;index++) {
    if(args[index]==='--live')options.live=true;
    else if(args[index]==='--windows-job')options.windowsJob=true;
    else if(args[index]==='--binary'&&args[index+1])options.binary=args[++index];
    else if(args[index]==='--windows-sandbox'&&args[index+1])options.windowsSandbox=args[++index];
    else if(args[index]==='--max-invocations'&&args[index+1])options.maxInvocations=Number(args[++index]);
    else if(args[index]==='--output'&&args[index+1])output=resolve(args[++index]);
    else if(args[index]==='--help'){process.stdout.write('node exec-probe.mjs [--live] [--windows-job] [--windows-sandbox mxc] [--max-invocations 1|2] [--binary path] [--output path]\nDefault: metadata only. Live: at most two bounded CLI invocations against a disposable worktree and existing subscription; no paid fallback.\n');return;}
    else throw new HarnessError('arguments','Unknown or incomplete option');
  }
  const target=output||join(root,'artifacts',`exec-probe-${Date.now()}.json`);options.artifactDirectory=dirname(target);
  const report=await runExecProbe(options);await mkdir(dirname(target),{recursive:true});await writeFile(target,JSON.stringify(report,null,2)+'\n');
  process.stdout.write(JSON.stringify({status:report.status,runtimeVersion:report.runtimeVersion,liveStarted:report.liveStarted,invocations:report.invocations,report:target})+'\n');
  process.exitCode=['passed','metadata-passed','edit-passed-resume-not-requested'].includes(report.status)?0:1;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href)main().catch(()=>{process.stderr.write('CLI comparison failed locally; raw runtime data was not printed.\n');process.exitCode=1;});
