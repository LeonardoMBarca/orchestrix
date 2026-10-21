// Explicit renderer-only synthetic boundary. No daemon, Git, native grant or provider.
import {useState} from 'react';
import {createRoot} from 'react-dom/client';
import {TaskForm} from '../src/task-form';
import {translate,type Language} from '../src/i18n';
import type {DesktopRequest,PreparedTask,RepositoryPreflight,TaskDraft} from '../src/protocol';
import '../src/styles.css';
const root=createRoot(document.getElementById('repository-fixture')!),calls:DesktopRequest[]=[];
let language:Language='en',scenario='compatible',defer=false,release:(()=>void)|null=null,changeGrant:((value:string)=>void)|null=null;
const initial:TaskDraft={conversationId:'synthetic-conversation',projectId:'synthetic-project',repositoryGrant:'grant-a',worktreeGrant:'worktree-grant',connectionId:'synthetic-connection',connectionRevision:1,objective:'Preserve this draft while inspecting',acceptanceCriteria:['A local check is never a model call'],contextFiles:[],checks:[],strategy:'',preset:'',model:'',effort:'',correctionBudget:1,fallback:'compatible'};
function preflight(grant:string):RepositoryPreflight{
 const blocked=scenario==='blocked',head=(grant==='grant-b'?'b':'a').repeat(40);
 return {schemaVersion:1,profile:'regular-utf8-v1',baseOid:head,headOid:head,compatible:!blocked,applicationEligible:scenario!=='dirty'&&!blocked,complete:scenario!=='partial',inventory:{files:19,bytes:5000,largestFileBytes:1024,regular:16,executables:1,links:1,submodules:1,oversized:0,binary:blocked?1:0,nonUtf8:0,secretPattern:0,excluded:2,contentInspected:17,contentSkipped:2},checkout:{clean:scenario!=='dirty',status:scenario==='dirty'?' M src/changed-file.ts':'',truncated:false,ignored:0,untracked:0},blockers:blocked?[{code:'binary-content',count:1,examples:['assets/a-very-long-file-name-used-to-exercise-wrapping-without-truncating-the-observed-path-and-its-extension.png']},{code:'future-unrecognized-blocker-code',count:1,examples:['a'.repeat(190)+'/file.txt']}]:[],limits:{files:4096,fileBytes:1048576,readBytes:67108864},modelCalled:false,authority:'inspection-only-revalidate-before-effects'};
}
async function send(operation:DesktopRequest){
 calls.push(structuredClone(operation));
 if(operation.type==='inspect-repository'){
  if(scenario==='fail')throw Error('synthetic-inspection-failure');
  const report=preflight(operation.directoryGrant);if(scenario==='wrong-shape')(report as any).modelCalled=true;
  if(defer){defer=false;return new Promise(resolve=>{release=()=>{release=null;resolve(report);};});}
  return report;
 }
 if(operation.type==='git-status')return {route:'deterministic',output:' M src/example.ts\n?? docs/a-new-document-with-a-long-filename-for-layout.md\n',truncated:scenario==='truncated-status',modelCalled:false};
 if(operation.type==='prepare-task')return {id:'synthetic-preparation',task:{objective:operation.draft.objective,connection_revision:1},warning:'Explicitly synthetic renderer preparation. No Core or runtime was invoked.'};
 throw Error('unexpected-renderer-only-operation');
}
(window as any).__TAURI_INTERNALS__={invoke:async(command:string,args:{request:DesktopRequest})=>{if(command!=='desktop_request')throw Error('unexpected-native-command');return send(args.request);}};
function Fixture(){
 const[task,setTask]=useState(initial),[prepared,setPrepared]=useState<PreparedTask|null>(null),[busy,setBusy]=useState(false),[runtimeConsent,setRuntimeConsent]=useState(false),[checkConsent,setCheckConsent]=useState(false),[error,setError]=useState('');
 changeGrant=value=>{setTask(current=>({...current,repositoryGrant:value}));setPrepared(null);};
 const t=(text:string)=>translate(language,text);
 return <><h1>Explicit synthetic repository inspection fixture</h1>{error&&<p role="alert">{error}</p>}<TaskForm task={task} setTask={setTask} prepared={prepared} setPrepared={setPrepared} grants={[{id:'grant-a',name:'Synthetic repository A',directory:true},{id:'grant-b',name:'Synthetic repository B',directory:true},{id:'worktree-grant',name:'Synthetic worktree parent',directory:true}]} setGrants={()=>{}} connections={[{id:'synthetic-connection',bindingRevision:1,label:'Explicit synthetic connection',provider:'Codex',mode:'subscription',status:'observed'}]} projects={[{id:'synthetic-project',name:'Synthetic project'}]} runtimeConsent={runtimeConsent} checkConsent={checkConsent} setRuntimeConsent={setRuntimeConsent} setCheckConsent={setCheckConsent} busy={busy} setBusy={setBusy} t={t} onError={setError} onSubmit={async()=>{}} suspended={false} onResume={async()=>{}} onAttach={async()=>{}} onCreateProject={()=>{}} onLoadCatalog={async()=>{}}/></>;
}
function render(){root.render(<Fixture/>);}
(window as any).__repositoryQa={calls,scenario(value:string){scenario=value;},defer(){defer=true;},pending(){return release!==null;},release(){release?.();},grant(value:string){changeGrant?.(value);},language(value:Language,scale:number){language=value;document.documentElement.lang=value;document.documentElement.style.fontSize=`${14*scale/100}px`;render();}};
render();
