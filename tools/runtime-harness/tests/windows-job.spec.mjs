// Optional native Windows proof; never starts a coding agent or uses credentials.
import test from 'node:test';
import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {mkdtemp,copyFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join,dirname} from 'node:path';
import {fileURLToPath} from 'node:url';

const root=join(dirname(fileURLToPath(import.meta.url)),'..');
const options={skip:process.platform!=='win32',timeout:30000};
const delay=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const alive=pid=>{try{process.kill(pid,0);return true;}catch(error){if(error.code==='ESRCH')return false;throw error;}};
async function until(check,timeoutMs=4000) {
  const deadline=Date.now()+timeoutMs;
  while(!check()) {
    if(Date.now()>=deadline)throw new Error('Owned fixture did not reach the expected state before deadline');
    await delay(20);
  }
}
function fixtureEnvironment() {
  const keys=new Set(['PATH','PATHEXT','SYSTEMROOT','WINDIR','COMSPEC','TEMP','TMP','USERPROFILE','APPDATA','LOCALAPPDATA']);
  return Object.fromEntries(Object.entries(process.env).filter(([key])=>keys.has(key.toUpperCase())));
}

class Probe {
  constructor({fixture=join(root,'fixtures/windows-job-tree.mjs'),binary=process.execPath,lifetimeMs=10000,failureStage,pauseOnFlood=false}={}) {
    this.messages=[];this.finished=false;this.parseError=false;this.stderrBytes=0;this.bytes=0;
    const command=join(process.env.SystemRoot,'System32','WindowsPowerShell','v1.0','powershell.exe');
    const args=['-NoLogo','-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',join(root,'windows/job-probe.ps1'),
      '-NodePath',binary,'-FixturePath',fixture,'-LifetimeMs',String(lifetimeMs)];
    if(failureStage)args.push('-FailureStage',failureStage);
    this.child=spawn(command,args,{shell:false,windowsHide:true,env:fixtureEnvironment(),stdio:['pipe','pipe','pipe']});
    let buffer='';
    this.child.stdout.setEncoding('utf8');
    this.child.stdout.on('data',chunk=>{
      this.bytes+=Buffer.byteLength(chunk);
      if(this.bytes>1024*1024){this.parseError=true;this.child.kill();return;}
      buffer+=chunk;
      let boundary;
      while((boundary=buffer.indexOf('\n'))!==-1) {
        const line=buffer.slice(0,boundary).replace(/\r$/,'');buffer=buffer.slice(boundary+1);
        if(!line.trim())continue;
        try {
          const message=JSON.parse(line);this.messages.push(message);
          if(pauseOnFlood&&message.type==='output.flood.start') {
            this.child.stdout.pause();this.floodPaused=true;this.floodStarted=Date.now();
          }
        }catch{this.parseError=true;}
      }
    });
    this.child.stderr.on('data',chunk=>{this.stderrBytes+=chunk.length;});
    this.child.stdin.on('error',()=>{});
    this.exited=new Promise(resolve=>this.child.once('exit',(code,signal)=>resolve({code,signal})));
    this.closed=new Promise(resolve=>{
      this.child.once('error',()=>{this.finished=true;resolve({launchFailed:true});});
      this.child.once('close',(code,signal)=>{
        this.finished=true;this.partialFrameObserved=Boolean(buffer.trim());
        if(this.partialFrameObserved&&!pauseOnFlood)this.parseError=true;
        resolve({code,signal});
      });
    });
  }
  async next(type,predicate=()=>true,timeoutMs=10000) {
    const deadline=Date.now()+timeoutMs;
    while(true) {
      if(this.parseError)throw new Error('Probe output violated JSONL or size limits');
      const index=this.messages.findIndex(message=>message.type===type&&predicate(message));
      if(index!==-1)return this.messages.splice(index,1)[0];
      if(this.finished)throw new Error(`Probe closed before ${type}; diagnostic bytes: ${this.stderrBytes}`);
      if(Date.now()>=deadline)throw new Error(`Probe timed out before ${type}`);
      await delay(20);
    }
  }
  send(message) {this.child.stdin.write(`${JSON.stringify(message)}\n`);}
  async tree() {
    const ready=await this.next('ready');
    assert.equal(ready.jobConfigured,true);assert.equal(ready.rootInJob,true);assert.equal(ready.resumed,true);
    assert.equal(ready.atomicAssignment,true);assert.equal(ready.jobHandleInherited,false);
    assert.equal(ready.watchdogArmed,true);
    const tree=await this.next('fixture.ready');
    assert.equal(tree.rootPid,ready.rootPid);assert.equal(tree.ownerId,ready.ownerId);
    assert.equal(tree.rootInJob,true);assert.equal(tree.descendantInJob,true);
    assert.notEqual(tree.descendantPid,tree.rootPid);assert.equal(tree.lifetimeMs,20000);
    assert.equal(alive(tree.rootPid),true);assert.equal(alive(tree.descendantPid),true);
    return tree;
  }
  async stop() {
    this.send({type:'stop'});
    const outcome=await this.next('stopped');
    assert.equal(outcome.activeCount,0);assert.equal(outcome.treeTermination,'confirmed');
    assert.equal(outcome.reason,'requested');assert.equal(outcome.rootStarted,true);assert.equal(outcome.rootExited,true);
    const exit=await this.closed;assert.equal(exit.code,0);assert.equal(this.parseError,false);
    return outcome;
  }
  async waitForClose(timeoutMs) {
    let timer;
    try{return await Promise.race([this.closed,new Promise(resolve=>{timer=setTimeout(resolve,timeoutMs);})]);}
    finally{clearTimeout(timer);}
  }
  async cleanup() {
    this.child.stdout.resume();
    if(!this.finished)this.child.stdin.end();
    await this.waitForClose(4000);
    if(!this.finished)this.child.kill();
    await this.waitForClose(4000);
  }
}
function start(t,settings) {
  const probe=new Probe(settings);t.after(()=>probe.cleanup());return probe;
}

test('native Job contains a detached descendant and paths with spaces and accents',options,async t=>{
  // Retained for inspection; never recursively deletes a computed Windows path.
  const directory=await mkdtemp(join(tmpdir(),'orchestrix job São Paulo '));
  const fixture=join(directory,'windows-job-tree.mjs');
  await copyFile(join(root,'fixtures/windows-job-tree.mjs'),fixture);
  const probe=start(t,{fixture});const tree=await probe.tree();
  probe.send({type:'inspect',pid:tree.descendantPid});
  const membership=await probe.next('inspect',message=>message.pid===tree.descendantPid);
  assert.equal(membership.inOwnedJob,true);
  await probe.stop();await until(()=>!alive(tree.rootPid)&&!alive(tree.descendantPid));
});

test('root exit does not release its still-associated descendant',options,async t=>{
  const probe=start(t);const tree=await probe.tree();probe.send({type:'exit-root'});
  assert.equal((await probe.next('root.exit')).code,0);await until(()=>!alive(tree.rootPid));
  assert.equal(alive(tree.descendantPid),true);
  probe.send({type:'inspect',pid:tree.descendantPid});
  assert.equal((await probe.next('inspect',message=>message.pid===tree.descendantPid)).inOwnedJob,true);
  await probe.stop();await until(()=>!alive(tree.descendantPid));
});

test('abrupt exit of the owning helper closes the Job and terminates known members',options,async t=>{
  const probe=start(t);const tree=await probe.tree();
  assert.equal(probe.child.kill(),true);await probe.closed;
  await until(()=>!alive(tree.rootPid)&&!alive(tree.descendantPid));
  assert.equal(probe.messages.some(message=>message.type==='stopped'),false);
});

test('controller EOF closes only its owned Job',options,async t=>{
  const probe=start(t);const tree=await probe.tree();probe.child.stdin.end();
  const outcome=await probe.next('stopped');
  assert.equal(outcome.activeCount,0);assert.equal(outcome.treeTermination,'confirmed');
  assert.equal(outcome.reason,'stdin-eof');
  assert.equal((await probe.closed).code,0);
  await until(()=>!alive(tree.rootPid)&&!alive(tree.descendantPid));
});

test('helper deadline terminates members before fixture safety expiry',options,async t=>{
  // Allow synthetic startup under a busy native host, retaining a clear margin
  // below the fixture's separate 20-second safety lifetime.
  const probe=start(t,{lifetimeMs:4000});const tree=await probe.tree();const started=Date.now();
  const outcome=await probe.next('stopped');
  assert.equal(outcome.activeCount,0);assert.equal(outcome.treeTermination,'confirmed');
  assert.equal(outcome.reason,'deadline');
  assert.ok(Date.now()-started<tree.lifetimeMs/2);
  assert.equal((await probe.closed).code,2);
  await until(()=>!alive(tree.rootPid)&&!alive(tree.descendantPid));
});

test('controlled failure before resume cleans the suspended process without running fixture',options,async t=>{
  const probe=start(t,{failureStage:'before-resume'});const exit=await probe.closed;
  assert.notEqual(exit.code,0);assert.equal(probe.parseError,false);
  assert.equal(probe.messages.some(message=>message.type==='ready'||message.type==='fixture.ready'),false);
  assert.equal(probe.messages.find(message=>message.type==='error')?.category,'injected-before-resume');
  const stopped=probe.messages.find(message=>message.type==='stopped');
  assert.ok(stopped);assert.equal(stopped.activeCount,0);assert.equal(stopped.treeTermination,'confirmed');
  assert.equal(stopped.rootStarted,true);assert.equal(stopped.rootExited,true);
});

test('stopping one Job leaves an unrelated owned process alive',options,async t=>{
  const outsider=spawn(process.execPath,['-e','setTimeout(()=>process.exit(0),20000)'],{
    stdio:'ignore',shell:false,windowsHide:true,env:fixtureEnvironment()
  });
  t.after(()=>{if(outsider.exitCode===null&&outsider.signalCode===null)outsider.kill();});
  const probe=start(t);const tree=await probe.tree();assert.equal(alive(outsider.pid),true);
  probe.send({type:'inspect',pid:outsider.pid});
  assert.equal((await probe.next('inspect',message=>message.pid===outsider.pid)).inOwnedJob,false);
  await probe.stop();await until(()=>!alive(tree.rootPid)&&!alive(tree.descendantPid));
  assert.equal(alive(outsider.pid),true);
});

test('missing executable fails without claiming a root was started',options,async t=>{
  const probe=start(t,{binary:join(root,'windows','nonexistent-owned-fixture','node.exe')});
  const exit=await probe.closed;assert.notEqual(exit.code,0);assert.equal(probe.parseError,false);
  assert.equal(probe.messages.some(message=>message.type==='ready'||message.type==='fixture.ready'),false);
  assert.equal(probe.messages.find(message=>message.type==='error')?.category,'launch');
  const stopped=probe.messages.find(message=>message.type==='stopped');
  assert.ok(stopped);assert.equal(stopped.rootStarted,false);assert.equal(stopped.rootExited,false);
});

test('independent watchdog terminates the owned Job and blocked writer before fixture safety expiry',options,async t=>{
  const probe=start(t,{lifetimeMs:4000,pauseOnFlood:true});const tree=await probe.tree();
  probe.send({type:'output-flood'});
  const pressure=await probe.next('output.flood.start');
  assert.equal(pressure.frameCount,4096);assert.equal(pressure.paddingCharacters,4096);
  assert.equal(probe.floodPaused,true);
  await delay(200);
  assert.equal(alive(tree.rootPid),true);assert.equal(alive(tree.descendantPid),true);
  // The planned 16 MiB cannot have been delivered to this paused observer.
  // Exit code 3 additionally requires the native watchdog to observe an active
  // Console write after it confirmed Job emptiness and the root handle exit.
  assert.ok(probe.bytes<pressure.frameCount*pressure.paddingCharacters);
  await until(()=>!alive(tree.rootPid)&&!alive(tree.descendantPid),6000);
  const membersElapsed=Date.now()-probe.floodStarted;
  assert.ok(membersElapsed<tree.lifetimeMs/2);
  await until(()=>probe.child.exitCode!==null,3000);
  const helperElapsed=Date.now()-probe.floodStarted;
  const exit=await probe.exited;
  assert.equal(exit.code,3);assert.equal(exit.signal,null);
  assert.ok(Date.now()-probe.floodStarted<tree.lifetimeMs/2);
  assert.equal(probe.messages.some(message=>message.type==='stopped'),false);
  // Process exit and pipe close are separate. Drain only after proving exit;
  // an abruptly cut final JSONL frame is allowed solely in this loss-of-output proof.
  probe.child.stdout.resume();
  assert.equal((await probe.closed).code,3);assert.equal(probe.parseError,false);
  assert.equal(probe.messages.some(message=>message.type==='stopped'),false);
  assert.ok(probe.messages.filter(message=>message.type==='output.flood').length<pressure.frameCount);
  t.diagnostic(JSON.stringify({configuredDeadlineMs:4000,membersExitedMs:membersElapsed,helperExitedMs:helperElapsed,
    observedFloodFrames:probe.messages.filter(message=>message.type==='output.flood').length,
    observedBytes:probe.bytes,partialFinalFrame:probe.partialFrameObserved}));
});
