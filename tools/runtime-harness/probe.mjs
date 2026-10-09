#!/usr/bin/env node
import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {fileURLToPath,pathToFileURL} from 'node:url';
import {join,resolve,dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {mkdtemp,mkdir,writeFile,readdir,readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {JsonRpcProcess,HarnessError} from './src/json-rpc.mjs';
import {CodexSession} from './src/codex-session.mjs';

const execute=promisify(execFile);
const root=dirname(fileURLToPath(import.meta.url));
export const SUPPORTED_CODEX_VERSION='0.162.0-alpha.2';
const EXPECTED='ORCHESTRIX_PROBE_OK';

/** Keep only OS/auth-location variables. Never transfer API keys or endpoint overrides. */
export function runtimeEnvironment(source=process.env) {
  const allowed=new Set(['PATH','PATHEXT','SYSTEMROOT','WINDIR','COMSPEC','TEMP','TMP','TMPDIR','USERPROFILE','HOME','HOMEDRIVE','HOMEPATH','APPDATA','LOCALAPPDATA','PROGRAMDATA','PROGRAMFILES','PROGRAMFILES(X86)','COMMONPROGRAMFILES','PROCESSOR_ARCHITECTURE','NUMBER_OF_PROCESSORS','CODEX_HOME','LANG','LC_ALL']);
  return Object.fromEntries(Object.entries(source).filter(([key])=>allowed.has(key.toUpperCase())));
}

function safeLabel(value) {return typeof value==='string'&&/^[\w.:-]{1,100}$/.test(value)?value:'unknown';}
function reportEvents(events) {
  const ids=new Map();
  const localId=(kind,id)=>{const key=`${kind}:${id}`;if(!ids.has(key))ids.set(key,`${kind}-${ids.size+1}`);return ids.get(key);};
  return events.map(event=>Object.fromEntries(Object.entries(event).map(([key,value])=>[key,['threadId','turnId','requestId'].includes(key)&&value!=null?localId(key,value):value])));
}
async function workspaceSnapshot(cwd) {
  const files=(await readdir(cwd,{recursive:true,withFileTypes:true})).filter(entry=>entry.isFile());
  const entries=[];
  for(const entry of files) {
    const path=join(entry.parentPath,entry.name);
    entries.push([path.slice(cwd.length),createHash('sha256').update(await readFile(path)).digest('hex')]);
  }
  return JSON.stringify(entries.sort(([left],[right])=>left.localeCompare(right)));
}

/** Metadata is default. Real inference requires --live AND all subscription gates. */
export async function runProbe({mode='fake',scenario='success',binary=process.platform==='win32'?'codex.exe':'codex',live=false,requestTimeoutMs=15000,turnTimeoutMs=45000}={}) {
  const started=Date.now();
  const report={schemaVersion:1,date:new Date().toISOString(),experiment:'M0/runtime-contract',mode,scenario:mode==='fake'?safeLabel(scenario):null,transport:'stdio',liveRequested:live,liveStarted:false,subscriptionGate:false,status:'starting',checks:[],limits:['One existing auth store; independent accounts not tested','Parent process exit is observed; process tree termination is unknown','Requested/resolved settings are not execution telemetry','Read-only policy is not a Windows sandbox certification'],events:[]};
  let client,session,cwd,before;
  try {
    cwd=await mkdtemp(join(tmpdir(),'orchestrix-runtime-'));
    await execute('git',['init','--quiet'],{cwd,windowsHide:true,timeout:10000});
    await writeFile(join(cwd,'AGENTS.md'),'This is an isolated Orchestrix protocol probe. Do not use tools, read files, write files, run commands, use plugins, or delegate. Answer the prompt only.\n');
    await writeFile(join(cwd,'probe.txt'),'Orchestrix disposable fixture. Never import a personal project.\n');
    before=await workspaceSnapshot(cwd);
    report.workspace={disposable:true,gitInitialized:true,retainedForInspection:true};
    const env=runtimeEnvironment();
    let command,args;
    if(mode==='fake') {
      command=process.execPath;args=[join(root,'fixtures/fake-app-server.mjs'),'--scenario',scenario];report.runtimeVersion='synthetic-fixture/1';
    } else {
      command=binary;
      const version=await execute(command,['--version'],{cwd,env,windowsHide:true,timeout:10000});
      report.runtimeVersion=version.stdout.match(/codex-cli ([\w.-]+)/)?.[1]??'unknown';
      if(report.runtimeVersion!==SUPPORTED_CODEX_VERSION)throw new HarnessError('version','Executable version has not been validated by this experiment');
      args=['--no-daemon','-c','model_provider="openai"','-c','openai_base_url=""','-c','chatgpt_base_url="https://chatgpt.com/backend-api/"','-c','analytics.enabled=false','-c','features.hooks=false','-c','features.plugins=false','-c','features.apps=false','-c','notify=[]','app-server','--listen','stdio://'];
    }
    client=new JsonRpcProcess({command,args,cwd,env,requestTimeoutMs});
    session=new CodexSession(client,{turnTimeoutMs});
    await session.initialize();report.checks.push('initialize/initialized');
    let discovery=await session.discover();
    report.auth=discovery.auth;report.availability=discovery.availability;
    report.models=discovery.models.map(model=>({model:safeLabel(model.model),isDefault:model.isDefault,efforts:model.efforts.map(safeLabel)}));
    let endpoints=await session.inspectEndpoints(cwd);report.endpoints=endpoints;
    report.subscriptionGate=discovery.auth.subscriptionGate&&discovery.availability===true&&endpoints.providerOpenai&&endpoints.officialEndpointConfirmed;
    if(!discovery.auth.subscriptionGate) {report.status='blocked-auth';return report;}
    if(discovery.availability!==true) {report.status='blocked-availability';return report;}
    if(!endpoints.providerOpenai||!endpoints.officialEndpointConfirmed) {report.status='blocked-endpoint';return report;}
    report.checks.push('subscription-auth','ordinary-usage-available','official-endpoint-resolved');
    if(mode==='codex'&&!live){report.status='metadata-passed';return report;}
    if(mode==='codex'&&!endpoints.resources.mcpDisabled&&session.mcpDisableOverrides) {
      // Start owns no session yet. Restart with per-server launch overrides before MCP prewarm.
      const overrides=session.mcpDisableOverrides.flatMap(value=>['-c',value]);
      report.bootstrapProcess=await client.stop();report.mcpOverridesCount=session.mcpDisableOverrides.length;
      args.splice(args.indexOf('app-server'),0,...overrides);
      client=new JsonRpcProcess({command,args,cwd,env,requestTimeoutMs});
      session=new CodexSession(client,{turnTimeoutMs});await session.initialize();
      discovery=await session.discover();endpoints=await session.inspectEndpoints(cwd);
      report.auth=discovery.auth;report.availability=discovery.availability;report.endpoints=endpoints;
      report.subscriptionGate=discovery.auth.subscriptionGate&&discovery.availability===true&&endpoints.providerOpenai&&endpoints.officialEndpointConfirmed;
      if(!report.subscriptionGate){report.status='blocked-after-restart';return report;}
      report.checks.push('isolated-restart-gates-reconfirmed');
    }
    if(mode==='codex'&&Object.entries(endpoints.resources).some(([key,value])=>key!=='mcpConfiguredCount'&&value!==true)) {report.status='blocked-extensions';return report;}
    const model=discovery.models.find(value=>value.isDefault)||discovery.models[0];
    if(!model)throw new HarnessError('capability','Runtime did not announce a model');
    const effort=model.efforts.includes('low')?'low':model.efforts.includes('minimal')?'minimal':model.defaultEffort;
    if(!model.efforts.includes(effort))throw new HarnessError('capability','Runtime did not announce a supported reasoning effort');
    const configuration=await session.startThread({cwd,model:model.model,effort});
    if(configuration.resolvedConfiguration.model!==model.model)throw new HarnessError('policy','Requested model was not resolved');
    report.configuration=configuration;report.checks.push('thread-boundaries-resolved');
    report.liveStarted=mode==='codex';
    const id=await session.startTurn(`Reply exactly ${EXPECTED}. Do not use tools, read or modify files, execute commands, access network, use plugins, or delegate.`,{effort});
    if(mode==='fake'&&['hang','slow'].includes(scenario)) {
      const cancellation=await session.interrupt(id);
      report.cancellation={requestAcknowledged:true,terminalStatus:cancellation.status,interruptedConfirmed:cancellation.status==='interrupted'};
      report.status=cancellation.status==='interrupted'?'passed':'cancellation-not-proven';return report;
    }
    const outcome=await session.waitForTurn(id);
    report.firstTurn={status:outcome.status,errorKind:outcome.errorKind,expectedTextMatched:(session.deltaText.get(id)||'').trim()===EXPECTED};
    if(outcome.status!=='completed') {report.status='turn-failed';return report;}
    if(mode==='codex'&&!report.firstTurn.expectedTextMatched) {report.status='unexpected-response';return report;}
    report.checks.push('turn-terminal-observed');
    // A second, bounded turn tests interruption. No tool/command is requested.
    if(mode==='codex') {
      const cancelledId=await session.startTurn('Without tools, files, network, or delegation, output integers from 1 to 5000 separated by spaces. This is a cancellation probe.',{effort});
      const cancellation=await session.interrupt(cancelledId);
      report.cancellation={requestAcknowledged:true,terminalStatus:cancellation.status,interruptedConfirmed:cancellation.status==='interrupted'};
      if(cancellation.status!=='interrupted'){report.status='cancellation-not-proven';return report;}
      report.checks.push('interrupt-terminal-observed');
    }
    report.status='passed';
    return report;
  } catch(error) {
    report.status='failed';report.error={kind:error instanceof HarnessError?error.kind:'local-error',method:error instanceof HarnessError?error.metadata?.method??null:null,code:Number.isSafeInteger(error.metadata?.code)?error.metadata.code:null};
    return report;
  } finally {
    if(session)report.events=reportEvents(session.events);
    if(client) {
      try{
        report.process=await client.stop();
        if(['passed','metadata-passed'].includes(report.status)&&(report.process.code!==0||client.failure))report.status='runtime-exit-failed';
      }catch{report.process={parentExited:false,treeTermination:'unknown'};report.status='termination-unconfirmed';}
      report.transportMetrics={messages:client.messages,stderrBytesDrained:client.stderrBytes};
    }
    if(cwd&&before) {
      try{report.workspace.filesUnchanged=before===await workspaceSnapshot(cwd);}catch{report.workspace.filesUnchanged=null;}
      if(report.workspace.filesUnchanged===false)report.status='workspace-changed';
    }
    report.durationMs=Date.now()-started;
  }
}

async function main() {
  let options={mode:'fake'},output;
  const args=process.argv.slice(2);
  for(let index=0;index<args.length;index++) {
    if(args[index]==='--fake')options.mode='fake';
    else if(args[index]==='--codex')options.mode='codex';
    else if(args[index]==='--live')options.live=true;
    else if(args[index]==='--scenario')options.scenario=args[++index];
    else if(args[index]==='--binary')options.binary=args[++index];
    else if(args[index]==='--output')output=resolve(args[++index]);
    else if(args[index]==='--help'){process.stdout.write('node probe.mjs --fake [--scenario success] | --codex [--live] [--binary path] [--output path]\nReal mode defaults to metadata; --live runs only after positive subscription, availability and endpoint gates.\n');return;}
    else throw new HarnessError('arguments','Unknown probe argument');
  }
  const report=await runProbe(options);
  const target=output||join(root,'artifacts',`probe-${options.mode}-${Date.now()}.json`);
  await mkdir(dirname(target),{recursive:true});await writeFile(target,`${JSON.stringify(report,null,2)}\n`);
  process.stdout.write(`${JSON.stringify({status:report.status,runtimeVersion:report.runtimeVersion,subscriptionGate:report.subscriptionGate,liveStarted:report.liveStarted,checks:report.checks,report:target})}\n`);
  process.exitCode=['passed','metadata-passed'].includes(report.status)?0:1;
}
if(process.argv[1]&&import.meta.url===pathToFileURL(resolve(process.argv[1])).href)main().catch(()=>{process.stderr.write('Probe failed locally; no raw runtime data was printed.\n');process.exitCode=1;});
