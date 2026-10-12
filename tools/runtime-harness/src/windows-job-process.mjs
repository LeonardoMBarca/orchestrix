import {spawn} from 'node:child_process';
import {EventEmitter} from 'node:events';
import {createServer} from 'node:net';
import {Writable,PassThrough} from 'node:stream';
import {randomBytes} from 'node:crypto';
import {access,realpath} from 'node:fs/promises';
import {isAbsolute,join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {HarnessError} from './errors.mjs';
import {runtimeEnvironment} from './runtime-environment.mjs';

const bridge=join(dirname(fileURLToPath(import.meta.url)),'..','windows','job-bridge.ps1');
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});promise.catch(()=>{});return {promise,resolve,reject};};
const envValue=(env,key)=>Object.entries(env).find(([name])=>name.toUpperCase()===key)?.[1];

/** Resolve a native executable without cmd.exe, PATHEXT scripts or implicit cwd lookup. */
export async function resolveWindowsExecutable(command,env) {
  if(typeof command!=='string'||!command.toLowerCase().endsWith('.exe'))throw new HarnessError('arguments','A native .exe is required by the experimental Windows bridge');
  const candidates=isAbsolute(command)?[command]:(envValue(env,'PATH')||'').split(';').filter(value=>value.trim()&&isAbsolute(value.replace(/^"|"$/g,''))).map(value=>join(value.replace(/^"|"$/g,''),command));
  for(const candidate of candidates)try{await access(candidate);return await realpath(candidate);}catch{}
  throw new HarnessError('launch','Native executable could not be resolved');
}

/** Experimental owned-Job subprocess. Control JSONL never shares runtime byte pipes. */
export class WindowsJobProcess extends EventEmitter {
  constructor({command,args=[],cwd,env=process.env,lifetimeMs=300000,startupTimeoutMs=10000}={}) {
    super();
    if(process.platform!=='win32')throw new HarnessError('platform','Windows Job containment requires Windows');
    if(!Number.isSafeInteger(lifetimeMs)||lifetimeMs<1000||lifetimeMs>300000||!Number.isSafeInteger(startupTimeoutMs)||startupTimeoutMs<1||startupTimeoutMs>10000)throw new HarnessError('arguments','Invalid bridge deadline');
    if(!Array.isArray(args)||args.length>256||args.some(value=>typeof value!=='string'||value.includes('\0'))||typeof cwd!=='string'||!isAbsolute(cwd))throw new HarnessError('arguments','Invalid native launch specification');
    this.command=command;this.args=args;this.cwd=cwd;this.env=runtimeEnvironment(env);this.lifetimeMs=lifetimeMs;this.startupTimeoutMs=startupTimeoutMs;
    this.pid=undefined;this.runtimeExited=false;this.runtimeCode=null;this.failure=null;this.stopPromise=null;
    this.helperExited=false;this.helperClosed=false;this.helperCode=null;this.stopped=null;this.streamState=null;this.ownerId=null;this.controlSequence=0;this.controlRecords=0;
    this.servers=[];this.sockets={};this.rawEnded={stdin:false,stdout:false,stderr:false};this.inspections=new Map();
    this.readyDeferred=deferred();this.ready=this.readyDeferred.promise;this.inputDeferred=deferred();this.finished=deferred();this.rawDrained=deferred();
    this.stdout=new PassThrough({highWaterMark:65536});this.stderr=new PassThrough({highWaterMark:65536});
    for(const stream of [this.stdout,this.stderr])stream.on('end',()=>queueMicrotask(()=>this.maybeClose()));
    const owner=this;
    this.stdin=new Writable({highWaterMark:65536,write(chunk,encoding,callback){owner.inputDeferred.promise.then(socket=>{if(socket.destroyed)return callback(new HarnessError('transport','Runtime input pipe closed'));socket.write(chunk,callback);},callback);},final(callback){owner.inputDeferred.promise.then(socket=>{socket.end(()=>{owner.rawEnded.stdin=true;owner.maybeClose();callback();});},callback);}});
    // Errors are additionally observed by the protocol/CLI owner; avoid an unhandled
    // stream error if an asynchronous launch fails before it attaches listeners.
    for(const stream of [this.stdin,this.stdout,this.stderr])stream.on('error',()=>{});
    this.startTimer=setTimeout(()=>this.fail(new HarnessError('launch-timeout','Windows bridge did not become ready within its startup deadline')),startupTimeoutMs);
    queueMicrotask(()=>this.launch().catch(error=>this.fail(error instanceof HarnessError?error:new HarnessError('launch','Windows bridge could not be launched'))));
  }
  async makePipe(name,role) {
    const server=createServer(socket=>{
      if(this.sockets[role]||this.failure||this.stopping){socket.destroy();return;}
      this.sockets[role]=socket;server.close();socket.on('error',()=>{if(!this.stopped&&!this.failure)this.fail(new HarnessError('transport','Runtime byte pipe failed'));});
      if(role==='stdin'){socket.on('close',()=>{this.rawEnded.stdin=true;this.maybeClose();});this.inputDeferred.resolve(socket);}
      else {
        const target=this[role];socket.on('data',chunk=>{if(!target.write(chunk))socket.pause();});target.on('drain',()=>socket.resume());
        socket.on('end',()=>{this.rawEnded[role]=true;target.end();this.maybeClose();});
        target.on('close',()=>socket.destroy());
      }
    });
    this.servers.push(server);
    await new Promise((resolve,reject)=>{server.once('error',reject);server.listen(name,resolve);});
    if(this.failure||this.stopping){server.close();throw this.failure||new HarnessError('cancelled','Launch cancelled before admission');}
  }
  async launch() {
    const executable=await resolveWindowsExecutable(this.command,this.env);
    if(this.failure||this.stopping)return;
    const nonce=randomBytes(24).toString('hex');
    const pipes=Object.fromEntries(['stdin','stdout','stderr'].map(role=>[role,`\\\\.\\pipe\\orchestrix-${nonce}-${role}`]));
    await Promise.all(Object.entries(pipes).map(([role,name])=>this.makePipe(name,role)));
    if(this.failure||this.stopping)return;
    const systemRoot=envValue(this.env,'SYSTEMROOT');
    if(!systemRoot||!isAbsolute(systemRoot))throw new HarnessError('launch','SystemRoot is required for the Windows helper');
    this.helper=spawn(join(systemRoot,'System32','WindowsPowerShell','v1.0','powershell.exe'),['-NoLogo','-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',bridge],{env:this.env,cwd:this.cwd,windowsHide:true,shell:false,stdio:['pipe','pipe','pipe']});
    this.helperStderrBytes=0;
    this.helper.stderr.on('data',chunk=>{this.helperStderrBytes+=chunk.length;});
    this.helper.stdout.on('data',chunk=>this.consumeControl(chunk));
    this.helper.stdout.on('end',()=>{
      try {this.controlBuffer=(this.controlBuffer||'')+(this.controlDecoder?.decode()||'');if(this.controlBuffer.trim())throw new Error();}
      catch {this.controlFaulted=true;this.stopped=null;if(!this.failure)this.fail(new HarnessError('helper-protocol','Incomplete helper control record'));}
    });
    this.helper.stdin.on('error',()=>{if(!this.helperExited&&!this.stopped&&!this.failure)this.fail(new HarnessError('helper-transport','Helper control input failed'));});
    this.helper.on('error',()=>this.fail(new HarnessError('launch','Windows helper process failed')));
    this.helper.on('exit',code=>{this.helperExited=true;this.helperCode=code;});
    this.helper.on('close',()=>{
      this.helperClosed=true;clearTimeout(this.startTimer);
      if(!this.stopped&&!this.failure)this.fail(new HarnessError('helper-exit','Helper exited without confirming its owned Job'));
      if(this.streamState?.faulted&&!this.failure)this.fail(new HarnessError('helper-transport','Helper byte stream failed'));
      if(!this.ownerId)this.readyDeferred.reject(this.failure||new HarnessError('launch','Helper exited before ready'));
      this.inputDeferred.reject(this.failure||new HarnessError('transport','Helper closed'));
      for(const server of this.servers)server.close();
      // Control may close before bytes/EOF on the three independent raw pipes.
      // Preserve a bounded drain; helper exit must not truncate a final response.
      this.rawDrainTimer=setTimeout(()=>this.detachRaw(),250);
      this.finished.resolve();this.maybeClose();
    });
    const specification=JSON.stringify({type:'launch',executable,args:this.args,cwd:this.cwd,env:this.env,pipes,lifetimeMs:this.lifetimeMs});
    if(Buffer.byteLength(specification)>131072)throw new HarnessError('arguments','Native launch specification exceeded its byte budget');
    this.helper.stdin.write(`${specification}\n`);
  }
  consumeControl(chunk) {
    if(this.controlFaulted)return;
    try {
      this.controlDecoder??=new TextDecoder('utf-8',{fatal:true});this.controlBuffer=(this.controlBuffer||'')+this.controlDecoder.decode(chunk,{stream:true});
      let newline;
      while((newline=this.controlBuffer.indexOf('\n'))!==-1) {
        const line=this.controlBuffer.slice(0,newline).replace(/\r$/,'');this.controlBuffer=this.controlBuffer.slice(newline+1);
        if(!line||Buffer.byteLength(line)>8192||++this.controlRecords>4096)throw new Error();
        const record=JSON.parse(line);
        if(record.schemaVersion!==1||record.sequence!==this.controlSequence+1||typeof record.ownerId!=='string'||!/^[a-fA-F0-9-]{16,64}$/.test(record.ownerId)||(this.ownerId&&record.ownerId!==this.ownerId))throw new Error();
        this.controlSequence=record.sequence;
        this.onControl(record);
      }
      if(Buffer.byteLength(this.controlBuffer)>8192)throw new Error();
    } catch {this.controlFaulted=true;this.stopped=null;this.fail(new HarnessError('helper-protocol','Invalid helper control stream'));}
  }
  onControl(record) {
    switch(record.type) {
      case 'ready':
        if(this.ownerId||!Number.isSafeInteger(record.rootPid)||record.rootPid<1||record.rootInJob!==true||record.atomicAssignment!==true||record.resumed!==true||record.jobConfigured!==true||record.jobHandleInherited!==false||record.watchdogArmed!==true)throw new Error();
        this.ownerId=record.ownerId;this.pid=record.rootPid;clearTimeout(this.startTimer);this.emit('spawn');this.readyDeferred.resolve(this);break;
      case 'root.exit':
        if(!this.ownerId||this.runtimeExited||!Number.isSafeInteger(record.code)||!Number.isSafeInteger(record.activeCount)||record.activeCount<0)throw new Error();
        this.runtimeExited=true;this.runtimeCode=record.code;this.emit('exit',record.code,null);this.maybeClose();break;
      case 'inspect': {
        if(!this.ownerId||!Number.isSafeInteger(record.pid)||typeof record.inOwnedJob!=='boolean'||!Number.isSafeInteger(record.activeCount))throw new Error();
        const pending=this.inspections.get(record.pid);if(pending){this.inspections.delete(record.pid);clearTimeout(pending.timer);pending.resolve({inOwnedJob:record.inOwnedJob,activeCount:record.activeCount});}break;
      }
      case 'streams.closed':
        if(!['stdinClosed','stdoutClosed','stderrClosed','faulted'].every(key=>typeof record[key]==='boolean'))throw new Error();
        this.streamState={stdinClosed:record.stdinClosed,stdoutClosed:record.stdoutClosed,stderrClosed:record.stderrClosed,faulted:record.faulted};break;
      case 'stopped':
        if(!['confirmed','unknown'].includes(record.treeTermination)||!['graceful','forced'].includes(record.termination)||typeof record.rootExited!=='boolean'||typeof record.rootStarted!=='boolean'||!(record.activeCount===null||Number.isSafeInteger(record.activeCount)))throw new Error();
        if(record.treeTermination==='confirmed'&&(record.activeCount!==0||(record.rootStarted&&(!this.ownerId||!record.rootExited||!this.runtimeExited))))throw new Error();
        this.stopped={treeTermination:record.rootStarted?record.treeTermination:'unknown',termination:record.termination,activeCount:record.activeCount,reason:record.reason};break;
      case 'error':
        this.fail(new HarnessError('native-launch','Windows helper reported a native failure',{category:typeof record.category==='string'&&/^[\w-]{1,60}$/.test(record.category)?record.category:'unknown',nativeError:Number.isSafeInteger(record.nativeError)?record.nativeError:null}));break;
      default:throw new Error();
    }
    // Only sanitized state is observable here, never runtime payloads/control paths.
    this.emit('supervision',this.supervision());
  }
  maybeClose() {
    if(this.rawEnded.stdout&&this.rawEnded.stderr&&this.stdout.readableEnded&&this.stderr.readableEnded){clearTimeout(this.rawDrainTimer);this.rawDrained.resolve(true);}
    if(this.closeEmitted||!this.runtimeExited||!this.rawEnded.stdout||!this.rawEnded.stderr||!this.stdout.readableEnded||!this.stderr.readableEnded)return;
    this.closeEmitted=true;this.emit('close',this.runtimeCode,null);
  }
  detachRaw() {
    clearTimeout(this.rawDrainTimer);
    for(const socket of Object.values(this.sockets))socket.destroy();
    for(const stream of [this.stdout,this.stderr])if(!stream.destroyed&&!stream.writableEnded)stream.end();
  }
  supervision() {
    return {code:this.runtimeCode,signal:null,parentExited:this.runtimeExited,treeTermination:this.stopped?.treeTermination??'unknown',stdioClosed:this.rawEnded.stdout&&this.rawEnded.stderr&&this.rawEnded.stdin&&this.stdout.readableEnded&&this.stderr.readableEnded,
      streams:{...this.rawEnded},helperStreams:this.streamState,helper:{code:this.helperCode,parentExited:this.helperExited,stdioClosed:this.helperClosed},
      termination:this.stopped?.termination??'unknown',activeCount:this.stopped?.activeCount??null,ownershipConfirmed:!!this.ownerId};
  }
  fail(error) {
    if(this.failure)return;this.failure=error;clearTimeout(this.startTimer);this.readyDeferred.reject(error);this.inputDeferred.reject(error);
    for(const pending of this.inspections.values()){clearTimeout(pending.timer);pending.reject(error);}this.inspections.clear();
    this.emit('error',error);void this.stop(1).catch(()=>{});
  }
  control(message) {
    if(!this.helper||this.helperExited||this.helper.stdin.destroyed)return false;
    const line=`${JSON.stringify(message)}\n`;if(Buffer.byteLength(line)>8192||this.helper.stdin.writableLength>16384)return false;
    return this.helper.stdin.write(line);
  }
  async inspect(pid,timeoutMs=1000) {
    if(!Number.isSafeInteger(pid)||pid<1||!Number.isSafeInteger(timeoutMs)||timeoutMs<1||timeoutMs>10000)throw new HarnessError('arguments','Invalid membership inspection');
    await this.ready;
    if(this.stopping||this.helperClosed)throw new HarnessError('transport','Owned Job observation has ended');
    if(this.inspections.has(pid)||this.inspections.size>=16)throw new HarnessError('contract','Membership observer limit exceeded');
    return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{this.inspections.delete(pid);reject(new HarnessError('timeout','Membership inspection timed out'));},timeoutMs);this.inspections.set(pid,{resolve,reject,timer});this.control({type:'inspect',pid});});
  }
  kill() {void this.stop(1).catch(()=>{});return true;}
  unref() { /* Observation detachment must not relinquish owned-Job cleanup. */ }
  stop(graceMs=3000) {
    if(!Number.isSafeInteger(graceMs)||graceMs<1||graceMs>10000)return Promise.reject(new HarnessError('arguments','Invalid Job stop grace period'));
    this.stopPromise??=this.stopOwned(graceMs);return this.stopPromise;
  }
  terminate() {this.stopPromise??=this.stopOwned(1,true);return this.stopPromise;}
  async waitFinished(ms) {
    let timer;const result=await Promise.race([this.finished.promise.then(()=>true),new Promise(resolve=>{timer=setTimeout(()=>resolve(false),ms);})]);clearTimeout(timer);return result;
  }
  async stopOwned(graceMs,force=false) {
    this.stopping=true;
    if(!this.helper){const cancelled=this.failure||new HarnessError('cancelled','Launch cancelled before admission');this.readyDeferred.reject(cancelled);this.inputDeferred.reject(cancelled);this.finished.resolve();}
    // During Add-Type/pipe setup no ownership ACK exists yet. Stop the helper we
    // spawned immediately; a queued control command must not admit a child later.
    if(this.helper&&!this.ownerId&&!this.helperExited){this.helper.kill();await this.waitFinished(1000);}
    // An explicit forced proof must not send runtime EOF first: a graceful EOF
    // exit would otherwise be confused with exercising TerminateJobObject.
    if(!force&&!this.stdin.destroyed&&!this.stdin.writableEnded)this.stdin.end();
    if(this.helper&&(force||!(await this.waitFinished(graceMs)))) {
      this.control({type:'stop'});
      if(!(await this.waitFinished(4500))) {
        // This PID is the helper we spawned. Its non-inherited Job handle closes
        // on death. Without a stopped record we retain unknown, never invent proof.
        if(!this.helperExited)this.helper.kill();
        await this.waitFinished(1000);
      }
    }
    clearTimeout(this.startTimer);
    if(this.helperClosed){let timer;await Promise.race([this.rawDrained.promise,new Promise(resolve=>{timer=setTimeout(resolve,250);})]);clearTimeout(timer);}
    for(const server of this.servers)try{server.close();}catch{}
    this.detachRaw();
    this.stdin.destroy();
    for(const stream of [this.stdout,this.stderr])if(!stream.destroyed&&!stream.writableEnded)stream.end();
    return this.supervision();
  }
}
export const spawnWindowsJobProcess=options=>new WindowsJobProcess(options);
export function cleanWindowsJobExit(state,expectedRuntimeCode=0) {
  return state?.ownershipConfirmed===true&&state.parentExited===true&&state.code===expectedRuntimeCode&&state.treeTermination==='confirmed'&&state.activeCount===0&&state.stdioClosed===true&&state.helper?.parentExited===true&&state.helper?.stdioClosed===true&&state.helper?.code===0&&state.helperStreams?.faulted===false&&state.helperStreams.stdinClosed===true&&state.helperStreams.stdoutClosed===true&&state.helperStreams.stderrClosed===true;
}
