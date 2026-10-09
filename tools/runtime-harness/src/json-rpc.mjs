import {spawn} from 'node:child_process';
import {EventEmitter} from 'node:events';

export class HarnessError extends Error {
  constructor(kind, message, metadata={}) {
    super(message);this.name='HarnessError';this.kind=kind;this.metadata=metadata;
  }
}

/** A bounded JSONL stdio client. It owns only the subprocess it spawned. */
export class JsonRpcProcess extends EventEmitter {
  constructor({command,args=[],cwd,env=process.env,requestTimeoutMs=10000,maxMessageBytes=1024*1024,exitDrainTimeoutMs=250}) {
    super();
    if(!Number.isSafeInteger(exitDrainTimeoutMs)||exitDrainTimeoutMs<1||exitDrainTimeoutMs>10000)throw new HarnessError('arguments','Invalid exit drain timeout');
    this.pending=new Map();this.serverRequests=new Set();this.nextId=1;
    this.requestTimeoutMs=requestTimeoutMs;this.maxMessageBytes=maxMessageBytes;
    this.buffer='';this.decoder=new TextDecoder('utf-8',{fatal:true});
    this.closed=false;this.parentExited=false;this.spawned=false;this.failure=null;this.stderrBytes=0;this.messages=0;this.exit=null;
    this.exitDrainTimeoutMs=exitDrainTimeoutMs;this.exitDrainTimer=null;this.stopPromise=null;
    this.closedPromise=new Promise(resolve=>{this.resolveClosed=resolve;});
    this.child=spawn(command,args,{cwd,env,stdio:['pipe','pipe','pipe'],shell:false,windowsHide:true});
    this.pid=this.child.pid;
    this.child.on('spawn',()=>{this.spawned=true;});
    this.child.stdout.on('data',chunk=>this.consume(chunk));
    this.child.stderr.on('data',chunk=>{this.stderrBytes+=chunk.length;});
    this.child.stdin.on('error',()=>{if(!this.parentExited&&!this.closed)this.fail(new HarnessError('transport','Unable to write to runtime stdin'));});
    for(const stream of [this.child.stdout,this.child.stderr])stream.on('error',()=>{if(!this.closed)this.fail(new HarnessError('transport','Unable to read runtime output'));});
    this.child.on('error',()=>this.fail(new HarnessError(this.spawned?'transport':'launch',this.spawned?'Runtime process operation failed':'Runtime process could not be launched')));
    this.child.stdout.on('end',()=>{
      if(this.failure)return;
      try{this.buffer+=this.decoder.decode();}catch{this.fail(new HarnessError('protocol','Incomplete UTF-8 stream'));return;}
      if(this.buffer.trim())this.fail(new HarnessError('protocol','Runtime ended with an incomplete JSONL message'));
    });
    this.child.on('exit',(code,signal)=>{
      this.parentExited=true;
      if(this.closed){this.exit={...this.exit,code,signal,parentExited:true};return;}
      this.exit={code,signal,parentExited:true,treeTermination:'unknown',stdioClosed:false,observationDetached:false};
      // Buffered responses may still arrive. A descendant holding the inherited
      // pipes must not keep request waiters or this observer alive indefinitely.
      this.exitDrainTimer=setTimeout(()=>this.detachTransport(),this.exitDrainTimeoutMs);
    });
    this.child.on('close',(code,signal)=>this.finishTransport({code,signal,stdioClosed:true,observationDetached:false}));
  }

  finishTransport({code=this.exit?.code??null,signal=this.exit?.signal??null,stdioClosed=false,observationDetached=true}={}) {
    if(this.closed)return;
    clearTimeout(this.exitDrainTimer);
    this.closed=true;this.exit={code,signal,parentExited:this.parentExited,treeTermination:'unknown',stdioClosed,observationDetached};
    const reason=this.failure||new HarnessError('process-exit','Runtime observation ended',{code,signal,parentExited:this.parentExited});
    this.rejectPending(reason);this.serverRequests.clear();this.resolveClosed(this.exit);this.emit('closed',this.exit);
  }

  detachTransport() {
    // Destroy only our pipe endpoints. This is not a process-tree termination.
    this.finishTransport();
    for(const stream of [this.child.stdin,this.child.stdout,this.child.stderr])stream.destroy();
    this.child.unref();
  }

  consume(chunk) {
    if(this.failure||this.closed)return;
    try{this.buffer+=this.decoder.decode(chunk,{stream:true});}catch{this.fail(new HarnessError('protocol','Runtime emitted invalid UTF-8'));return;}
    let newline;
    while((newline=this.buffer.indexOf('\n'))!==-1) {
      const line=this.buffer.slice(0,newline).replace(/\r$/,'');this.buffer=this.buffer.slice(newline+1);
      if(Buffer.byteLength(line)>this.maxMessageBytes){this.fail(new HarnessError('protocol','Runtime message exceeded limit'));return;}
      if(!line.trim())continue;
      let message;
      try{message=JSON.parse(line);}catch{this.fail(new HarnessError('protocol','Runtime emitted malformed JSON'));return;}
      if(!message||typeof message!=='object'||Array.isArray(message)){this.fail(new HarnessError('protocol','Runtime message must be an object'));return;}
      this.messages++;
      if(Object.hasOwn(message,'method')) {
        if(typeof message.method!=='string'||!/^[-\w./]{1,120}$/.test(message.method)){this.fail(new HarnessError('protocol','Invalid method envelope'));return;}
        if(Object.hasOwn(message,'id')) {
          if(!validId(message.id)||this.serverRequests.has(message.id)){this.fail(new HarnessError('protocol','Invalid or duplicate runtime request ID'));return;}
          this.serverRequests.add(message.id);this.emit('request',message);
        } else this.emit('notification',message);
      } else {
        if(!validId(message.id)||Object.hasOwn(message,'result')===Object.hasOwn(message,'error')){this.fail(new HarnessError('protocol','Invalid runtime response envelope'));return;}
        const pending=this.pending.get(message.id);
        if(!pending){this.emit('diagnostic',{kind:'late-or-unknown-response'});continue;}
        this.pending.delete(message.id);clearTimeout(pending.timer);
        if(Object.hasOwn(message,'error')) {
          pending.reject(new HarnessError('rpc','Runtime rejected request',{method:pending.method,code:Number.isSafeInteger(message.error?.code)?message.error.code:null}));
        } else pending.resolve(message.result);
      }
      if(this.failure||this.closed)return;
    }
    if(Buffer.byteLength(this.buffer)>this.maxMessageBytes)this.fail(new HarnessError('protocol','Runtime message exceeded limit'));
  }

  write(message) {
    if(this.closed||this.failure||this.parentExited)throw this.failure||new HarnessError(this.parentExited?'process-exit':'transport','Runtime is unavailable');
    this.child.stdin.write(`${JSON.stringify(message)}\n`);
  }
  request(method,params={},timeoutMs=this.requestTimeoutMs) {
    if(this.closed||this.failure||this.parentExited)return Promise.reject(this.failure||new HarnessError(this.parentExited?'process-exit':'transport','Runtime is unavailable'));
    const id=this.nextId++;
    return new Promise((resolve,reject)=>{
      const timer=setTimeout(()=>{this.pending.delete(id);reject(new HarnessError('timeout','Runtime request timed out',{method}));},timeoutMs);
      this.pending.set(id,{resolve,reject,timer,method});
      try{this.write({id,method,params});}catch(error){clearTimeout(timer);this.pending.delete(id);reject(error);}
    });
  }
  notify(method,params={}) {this.write({method,params});}
  reply(id,result) {
    if(!this.serverRequests.delete(id))throw new HarnessError('protocol','Cannot answer an unknown runtime request');
    this.write({id,result});
  }
  rejectRequest(id) {
    if(!this.serverRequests.delete(id))throw new HarnessError('protocol','Cannot answer an unknown runtime request');
    this.write({id,error:{code:-32601,message:'Client capability unavailable in this probe'}});
  }
  rejectPending(reason) {for(const pending of this.pending.values()){clearTimeout(pending.timer);pending.reject(reason);}this.pending.clear();}
  fail(reason) {
    if(this.failure)return;
    this.failure=reason;this.rejectPending(reason);this.emit('failure',reason);
    if(!this.closed&&!this.parentExited)this.child.kill();
  }
  stop(timeoutMs=3000) {
    if(!Number.isSafeInteger(timeoutMs)||timeoutMs<1)return Promise.reject(new HarnessError('arguments','Invalid stop timeout'));
    this.stopPromise??=Promise.resolve().then(()=>this.stopOwnedProcess(timeoutMs));
    return this.stopPromise;
  }
  async waitForClose(timeoutMs) {
    let timer;
    const result=await Promise.race([this.closedPromise,new Promise(resolve=>{timer=setTimeout(()=>resolve(null),timeoutMs);})]);
    clearTimeout(timer);return result;
  }
  async stopOwnedProcess(timeoutMs) {
    if(this.closed)return this.exit;
    if(!this.parentExited)this.child.stdin.end();
    const exited=await this.waitForClose(timeoutMs);
    if(exited)return exited;
    if(this.parentExited)return this.closedPromise;
    this.child.kill();
    const killed=await this.waitForClose(timeoutMs);
    if(killed)return killed;
    if(this.parentExited)return this.closedPromise;
    this.detachTransport();
    throw new HarnessError('termination','Could not confirm runtime parent exit',this.exit);
  }
}

function validId(id) {return typeof id==='string'||(typeof id==='number'&&Number.isSafeInteger(id));}
