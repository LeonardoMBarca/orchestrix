#!/usr/bin/env node
// OX-001 experiment: existing subscription, disposable Git fixture, explicit live opt-in.
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {join,resolve,dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {mkdtemp,mkdir,writeFile,readFile,readdir,realpath} from 'node:fs/promises';
import {createHash,randomUUID} from 'node:crypto';
import {JsonRpcProcess,HarnessError} from './src/json-rpc.mjs';
import {CodexSession} from './src/codex-session.mjs';
import {runtimeEnvironment,fixtureGitEnvironment,codexLaunchArguments,supportsCodexVersion,reportEvents,reportConfiguration,observeCancellation} from './probe.mjs';

const execute=promisify(execFile),root=dirname(fileURLToPath(import.meta.url));
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const git=async (fixture,cwd,args)=>(await execute('git',['-c',`core.hooksPath=${fixture.hooks}`,'-c','commit.gpgsign=false','-c','core.autocrlf=false',...args],{cwd,env:fixtureGitEnvironment(),windowsHide:true,timeout:15000,maxBuffer:1024*1024})).stdout;
const checkSource=`import {readFile} from 'node:fs/promises';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const source=await readFile(new URL('../src/add.mjs',import.meta.url),'utf8');
// Only a bounded arithmetic expression is executable in this disposable probe.
const match=source.match(/^\\s*export\\s+function\\s+add\\s*\\(a,\\s*b\\)\\s*\\{\\s*return\\s+([ab0-9.+*/()\\s-]+);?\\s*\\}\\s*$/);
if(!match||match[1].length>128)throw new Error('Fixture code outside the permitted arithmetic grammar');
for(const [a,b] of [[2,3],[0,0],[-4,2],[0.5,0.25],[12,-7]]) {
  assert.equal(vm.runInNewContext(match[1],{a,b},{timeout:100}),a+b);
}
console.log(JSON.stringify({passed:true,cases:5}));
`;

export async function trackedSnapshot(path) {
  const entries=[];
  async function scan(directory,prefix='') {
    for(const entry of await readdir(directory,{withFileTypes:true})) {
      if(entry.name==='.git')continue;
      const relative=prefix+entry.name,absolute=join(directory,entry.name);
      if(entry.isSymbolicLink())throw new HarnessError('artifact','Symlink outside fixture contract');
      if(entry.isDirectory())await scan(absolute,relative+'/');
      else if(entry.isFile())entries.push({path:relative,sha256:sha(await readFile(absolute))});
    }
  }
  await scan(path);return entries.sort((a,b)=>a.path.localeCompare(b.path));
}

export async function createWorktreeFixture() {
  // Windows TEMP can use an 8.3 alias while persisted runtime cwd uses its long
  // name. Pin one filesystem-resolved root before Git or the runtime sees it.
  const folder=await realpath(await mkdtemp(join(tmpdir(),'orchestrix-worktree-')));
  const fixture={folder,main:join(folder,'main'),worktree:join(folder,'attempt'),hooks:join(folder,'empty-hooks')};
  await Promise.all([mkdir(fixture.main),mkdir(fixture.hooks)]);
  await mkdir(join(fixture.main,'src'));await mkdir(join(fixture.main,'checks'));
  await writeFile(join(fixture.main,'src/add.mjs'),'export function add(a, b) { return a - b; }\n');
  await writeFile(join(fixture.main,'checks/add-check.mjs'),checkSource);
  await writeFile(join(fixture.main,'AGENTS.md'),'Disposable Orchestrix harness fixture. Fix only src/add.mjs. Do not modify checks, instructions or Git state. No installs, network, plugins, delegation or reads outside this worktree. Preserve the function signature and the arithmetic-only implementation.\n');
  await git(fixture,fixture.main,['init','--quiet','--initial-branch=main']);
  await git(fixture,fixture.main,['add','--all']);
  await git(fixture,fixture.main,['-c','user.name=Orchestrix Fixture','-c','user.email=fixture@orchestrix.invalid','commit','--quiet','-m','Disposable arithmetic fixture']);
  fixture.base=(await git(fixture,fixture.main,['rev-parse','HEAD'])).trim();
  fixture.baseline=await trackedSnapshot(fixture.main);
  fixture.baselineCheck=await runFixtureCheck(fixture.main);
  requireFailingBaseline(fixture.baselineCheck);
  await git(fixture,fixture.main,['worktree','add','--quiet','--detach',fixture.worktree,fixture.base]);
  return fixture;
}

export function requireFailingBaseline(check) {
  if(check.passed!==false||check.exitCode!==1||check.failureKind!=='assertion-failed')throw new HarnessError('fixture','Baseline arithmetic assertion failure was not confirmed');
}

export async function runFixtureCheck(cwd) {
  try {
    const result=await execute(process.execPath,['checks/add-check.mjs'],{cwd,windowsHide:true,timeout:5000,maxBuffer:8192,env:runtimeEnvironment()});
    const value=JSON.parse(result.stdout.trim());return {passed:value.passed===true&&value.cases===5,exitCode:0,cases:value.cases};
  } catch(error) {return {passed:false,exitCode:Number.isInteger(error.code)?error.code:null,cases:0,
    failureKind:error.code===1&&/ERR_ASSERTION/.test(String(error.stderr||''))?'assertion-failed':error.killed?'timeout':'checker-error'};}
}

export async function inspectAttempt(fixture) {
  const mainUnchanged=JSON.stringify(await trackedSnapshot(fixture.main))===JSON.stringify(fixture.baseline)&&
    (await git(fixture,fixture.main,['rev-parse','HEAD'])).trim()===fixture.base&&!(await git(fixture,fixture.main,['status','--porcelain'])).trim();
  const baseMatched=(await git(fixture,fixture.worktree,['rev-parse','HEAD'])).trim()===fixture.base;
  const status=(await git(fixture,fixture.worktree,['status','--porcelain=v1','--untracked-files=all'])).trim();
  const paths=(await git(fixture,fixture.worktree,['diff','--name-only',fixture.base])).trim().split('\n').filter(Boolean);
  const before=await trackedSnapshot(fixture.worktree);
  const diff=await git(fixture,fixture.worktree,['diff','--binary',fixture.base]);
  const expectedOnly=baseMatched&&mainUnchanged&&status==='M src/add.mjs'&&paths.length===1&&paths[0]==='src/add.mjs'&&
    before.length===fixture.baseline.length&&before.every(entry=>entry.path==='src/add.mjs'||fixture.baseline.some(old=>old.path===entry.path&&old.sha256===entry.sha256));
  // Never execute a checker changed by an agent, or arbitrary generated JavaScript.
  const checks=expectedOnly?await runFixtureCheck(fixture.worktree):{passed:false,exitCode:null,cases:0,notRun:true};
  const checksSameSnapshot=JSON.stringify(before)===JSON.stringify(await trackedSnapshot(fixture.worktree));
  return {passed:expectedOnly&&checks.passed&&checksSameSnapshot,mainUnchanged,baseMatched,expectedOnly,checksSameSnapshot,base:fixture.base,paths,diffSha256:sha(diff),checks,sources:before,diff};
}

export async function connect(binary,cwd,requestTimeoutMs,{windowsSandbox}={}) {
  const args=codexLaunchArguments({windowsSandbox});let client,session;
  try {
    async function launch() {
      client=new JsonRpcProcess({command:binary,args,cwd,env:runtimeEnvironment(),requestTimeoutMs});
      session=new CodexSession(client,{turnTimeoutMs:90000});await session.initialize();
      const discovery=await session.discover(),endpoints=await session.inspectEndpoints(cwd);
      if(!discovery.auth.subscriptionGate||discovery.availability!==true||!endpoints.providerOpenai||!endpoints.officialEndpointConfirmed)throw new HarnessError('subscription-gate','Subscription/availability/official endpoint not confirmed');
      if(windowsSandbox&&endpoints.windowsSandbox!==windowsSandbox)throw new HarnessError('policy','Requested Windows sandbox implementation was not confirmed');
      return {discovery,endpoints};
    }
    let metadata=await launch();
    if(!metadata.endpoints.resources.mcpDisabled&&session.mcpDisableOverrides) {
      args.splice(args.indexOf('app-server'),0,...session.mcpDisableOverrides.flatMap(value=>['-c',value]));
      const stopped=await client.stop();if(!stopped.parentExited||stopped.code!==0||client.failure)throw new HarnessError('process-exit','Bootstrap exit not confirmed');
      metadata=await launch();
    }
    if(Object.entries(metadata.endpoints.resources).some(([key,value])=>key!=='mcpConfiguredCount'&&value!==true))throw new HarnessError('extensions','Extensions are not confirmed disabled');
    return {client,session,...metadata};
  } catch(error) {if(client)await client.stop();throw error;}
}

export async function runWorktreeProbe({binary=process.platform==='win32'?'codex.exe':'codex',live=false,windowsSandbox,requestTimeoutMs=15000,artifactDirectory=join(root,'artifacts')}={}) {
  const started=Date.now(),report={schemaVersion:1,date:new Date().toISOString(),experiment:'OX-001/worktree-resume',liveRequested:live,liveStarted:false,status:'starting',checks:[],events:[],processes:[],limits:['Disposable fixture only; not the production worker or SQLite recovery','Runtime conversational persistence remains owned by Codex','Parent exits observed; Windows process-tree containment integration is still pending','No CLI exec comparison or independent account/quota proof','Sandbox resolution is not a Windows filesystem/network enforcement certification']};
  let fixture,connection;const events=[];
  try {
    if(windowsSandbox!==undefined&&(process.platform!=='win32'||windowsSandbox!=='mxc'))throw new HarnessError('arguments','The explicit MXC selection requires Windows');
    report.requestedWindowsSandbox=windowsSandbox??null;
    fixture=await createWorktreeFixture();report.workspace={disposable:true,retainedForInspection:true,base:fixture.base,baselineCheck:fixture.baselineCheck};
    const version=await execute(binary,['--version'],{cwd:fixture.worktree,windowsHide:true,timeout:10000,env:runtimeEnvironment()});
    report.runtimeVersion=version.stdout.match(/codex-cli ([\w.-]+)/)?.[1]??'unknown';
    if(!supportsCodexVersion(report.runtimeVersion))throw new HarnessError('version','Runtime version not schema-reviewed');
    connection=await connect(binary,fixture.worktree,requestTimeoutMs,{windowsSandbox});
    report.auth=connection.discovery.auth;report.availability=connection.discovery.availability;report.endpoints=connection.endpoints;
    report.checks.push('subscription-gates-confirmed','extensions-disabled','disposable-worktree','baseline-check-failed');
    if(!live){report.status='metadata-passed';return report;}
    const model=connection.discovery.models.find(model=>model.isDefault)||connection.discovery.models[0];
    const effort=model?.efforts.includes('low')?'low':model?.efforts.includes('minimal')?'minimal':model?.defaultEffort;
    if(!model||!model.efforts.includes(effort)||!connection.session.accountBinding)throw new HarnessError('capability','Model/effort/account binding not observed');
    const accountBinding=connection.session.accountBinding;
    const options={cwd:fixture.worktree,model:model.model,effort,sandbox:'workspace-write',ephemeral:false};
    report.configuration=reportConfiguration(await connection.session.startThread(options));const threadId=connection.session.threadId,marker=`ORCHESTRIX_${randomUUID()}`;
    report.liveStarted=true;
    const id=await connection.session.startTurn(`Remember this marker for a later continuation: ${marker}. Fix only src/add.mjs so add(a, b) returns the sum for integers and decimals. Preserve the function signature and an arithmetic-only return expression. Do not change checks or AGENTS.md, create other files, install anything, use network, delegate or commit. You may inspect the fixture and change that one file inside this worktree.`,{effort});
    const first=await connection.session.waitForTurn(id);report.firstTurn={status:first.status,errorKind:first.errorKind};
    if(first.status!=='completed'){report.status='turn-not-completed';return report;}
    const attempt=await inspectAttempt(fixture);await mkdir(artifactDirectory,{recursive:true});
    const patchName=`worktree-attempt-${Date.now()}.patch`;await writeFile(join(artifactDirectory,patchName),attempt.diff);delete attempt.diff;
    report.attempt={...attempt,patch:patchName};if(!attempt.passed){report.status='artifact-check-failed';return report;}
    report.checks.push('diff-attempt-bound','checks-same-snapshot','main-checkout-preserved');
    events.push(...connection.session.events);const stopped=await connection.client.stop();report.processes.push(stopped);
    if(!stopped.parentExited||stopped.code!==0||connection.client.failure)throw new HarnessError('process-exit','First app-server exit unconfirmed');
    connection=null;connection=await connect(binary,fixture.worktree,requestTimeoutMs,{windowsSandbox});
    if(connection.session.accountBinding!==accountBinding)throw new HarnessError('connection-binding','Observed account metadata changed');
    if(!connection.discovery.models.some(value=>value.model===model.model&&value.efforts.includes(effort)))throw new HarnessError('capability','Resumed model/effort no longer announced');
    report.resumedConfiguration=reportConfiguration(await connection.session.resumeThread({threadId,...options}));
    const resumedId=await connection.session.startTurn('Reply only with the exact marker I asked you to remember earlier. Do not use tools or modify files.',{effort});
    const resumed=await connection.session.waitForTurn(resumedId);
    report.resume={serverRestarted:true,sameThread:connection.session.threadId===threadId,sameObservedAccount:true,status:resumed.status,priorMarkerMatched:(connection.session.deltaText.get(resumedId)||'').trim()===marker};
    if(resumed.status!=='completed'||!report.resume.priorMarkerMatched) {report.status='resume-not-proven';return report;}
    const afterResume=await inspectAttempt(fixture);report.resume.artifactUnchanged=afterResume.diffSha256===attempt.diffSha256&&JSON.stringify(afterResume.sources)===JSON.stringify(attempt.sources);
    if(!afterResume.passed||!report.resume.artifactUnchanged){report.status='resume-artifact-changed';return report;}
    report.checks.push('same-thread-resumed-after-restart','prior-runtime-context-observed');
    const cancelId=await connection.session.startTurn('Without tools, files, network or delegation, output integers from 1 to 5000 separated by spaces. This turn is intended for immediate interruption.',{effort});
    report.cancellation=await observeCancellation(connection.session,cancelId);
    if(!report.cancellation.interruptedConfirmed){report.status='cancellation-not-proven';return report;}
    const finalAttempt=await inspectAttempt(fixture);
    report.cancellation.artifactUnchanged=finalAttempt.passed&&finalAttempt.diffSha256===attempt.diffSha256&&JSON.stringify(finalAttempt.sources)===JSON.stringify(attempt.sources);
    if(!report.cancellation.artifactUnchanged){report.status='cancel-artifact-changed';return report;}
    report.checks.push('resumed-turn-interrupted','artifact-preserved-after-cancellation');report.status='passed';return report;
  } catch(error) {report.status='failed';report.error={kind:error instanceof HarnessError?error.kind:'local-error',method:error.metadata?.method??null,code:Number.isSafeInteger(error.metadata?.code)?error.metadata.code:null};return report;
  } finally {
    if(connection) {
      events.push(...connection.session.events);
      try {const stopped=await connection.client.stop();report.processes.push(stopped);if(['passed','metadata-passed'].includes(report.status)&&(!stopped.parentExited||stopped.code!==0||connection.client.failure))report.status='runtime-exit-failed';}
      catch {report.status='termination-unconfirmed';}
    }
    report.events=reportEvents(events).map((event,index)=>({...event,sequence:index+1}));
    if(fixture) {
      try {report.workspace.mainUnchanged=JSON.stringify(await trackedSnapshot(fixture.main))===JSON.stringify(fixture.baseline);if(!report.workspace.mainUnchanged)report.status='main-checkout-changed';}
      catch {report.workspace.mainUnchanged=null;report.status='artifact-observation-failed';}
    }
    report.durationMs=Date.now()-started;
  }
}

async function main() {
  let binary,live=false,output,windowsSandbox;const args=process.argv.slice(2);
  for(let index=0;index<args.length;index++) {
    if(args[index]==='--live')live=true;
    else if(args[index]==='--binary'&&args[index+1])binary=args[++index];
    else if(args[index]==='--windows-sandbox'&&args[index+1])windowsSandbox=args[++index];
    else if(args[index]==='--output'&&args[index+1])output=resolve(args[++index]);
    else if(args[index]==='--help'){process.stdout.write('node worktree-probe.mjs [--live] [--windows-sandbox mxc] [--binary path] [--output path]\nDefault: metadata only. Live: three bounded turns on an existing subscription, disposable worktree edit/checks, app-server restart/resume and interruption. Explicit MXC requires Windows and confirmed configuration; no setup or API fallback.\n');return;}
    else throw new HarnessError('arguments','Unknown or incomplete option');
  }
  const target=output||join(root,'artifacts',`worktree-probe-${Date.now()}.json`);
  const report=await runWorktreeProbe({binary,live,windowsSandbox,artifactDirectory:dirname(target)});
  await mkdir(dirname(target),{recursive:true});await writeFile(target,JSON.stringify(report,null,2)+'\n');
  process.stdout.write(JSON.stringify({status:report.status,runtimeVersion:report.runtimeVersion,liveStarted:report.liveStarted,checks:report.checks,report:target})+'\n');
  process.exitCode=['passed','metadata-passed'].includes(report.status)?0:1;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href)main().catch(()=>{process.stderr.write('Worktree probe failed locally; raw runtime data was not printed.\n');process.exitCode=1;});
