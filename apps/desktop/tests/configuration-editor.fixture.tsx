// Isolated browser fixture, never included in the normal Desktop entry point.
// It exercises renderer behavior, not persistence, the daemon or a provider.
import {createRoot} from 'react-dom/client';
import {ConfigurationEditor} from '../src/configuration-editor';
import type {ConfigurationCatalog,ConfigurationRequest,ConfigurationPreview} from '../src/configuration-types';
import type {Language} from '../src/i18n';
import '../src/styles.css';
const implementation={id:'implementation-engineer',version:2,editRevision:1,origin:'native' as const,archived:false,name:'Implementation Engineer',role:'implementation' as const,instructions:'Prefer small changes',outputLanguage:'inherit',conventions:['Use local conventions'],specialty:'Software',preferences:{},requirements:{subscription_only:true}};
const review={...implementation,id:'independent-reviewer',name:'Independent Reviewer',role:'review' as const,instructions:'Inspect exact evidence',requirements:{subscription_only:true,read_only:true}};
const nativeMode=(id:string,name:string)=>({id,version:2,editRevision:1,origin:'native' as const,archived:false,name,purpose:`${name} purpose`,instructions:'Native instructions',roles:[{role:'implementation' as const,profile:{id:implementation.id,version:2}},{role:'review' as const,profile:{id:review.id,version:2}}],outputs:['Patch'],criteria:['Checks and review'],preferences:{},requirements:{subscription_only:true,require_review:true},executionSupport:(['development','refactoring','documentation'].includes(id)?'manual-worker':'artifact-flow-unproven') as 'manual-worker'|'artifact-flow-unproven'});
const catalog:ConfigurationCatalog={schemaVersion:1,revision:1,modes:[nativeMode('development','Development'),nativeMode('research','Research'),nativeMode('documentation','Documentation'),nativeMode('presentations','Presentations'),nativeMode('refactoring','Refactoring'),nativeMode('improvement-review','Improvement Review')],profiles:[implementation,review],scopes:[]};
const calls:ConfigurationRequest[]=[],receipts=new Map<string,unknown>(),history=new Map<string,unknown>();
let language:Language='en',active=true,failSave=false,loseSave=false,holdPreview=false,releasePreview:((value:ConfigurationPreview)=>void)|null=null;
const projects=[{id:'project-a',name:'Project A'},{id:'project-b',name:'Project B'}];
const connections=[{id:'account-a',label:'Account A',name:'Account A',provider:'Codex',mode:'subscription',status:'observed',models:[{id:'model-a',name:'Reported model',efforts:['low','high']}]},{id:'account-b',label:'Account B',provider:'Codex',mode:'subscription',status:'observed',models:[{id:'model-a',name:'Homonym on another account'}]}];
async function send<T>(request:ConfigurationRequest):Promise<T>{calls.push(structuredClone(request));
 if(request.type==='catalog')return structuredClone(catalog) as T;
 if(request.type==='get'){const key=`${request.kind}:${request.id}:${request.version}`,found=history.get(key)??(request.kind==='mode'?catalog.modes:catalog.profiles).find(row=>row.id===request.id&&row.version===request.version);if(!found)throw Error('definition-version-not-found');return structuredClone(found) as T;}
 if(request.type==='preview'){
  const mode=request.modeDraft??catalog.modes.find(row=>row.id===request.mode.id)!,profile=request.profileDraft??catalog.profiles.find(row=>row.id===request.profile.id)!;
  const preferences={strategy:'balanced',...mode.preferences,...request.scopeDraft?.preferences,...profile.preferences};
  const value:ConfigurationPreview={schemaVersion:1,revision:catalog.revision,policy:{layers:catalog.scopes},definitions:{mode:{id:mode.id,version:mode.version},profile:{id:profile.id,version:profile.version},reviewProfile:mode.roles.find(row=>row.role==='review')!.profile},fingerprint:'mock-preview',warnings:['No eligibility evidence is implied by a saved definition.'],decision:{preferences,origins:{strategy:'mode-defaults@system'},requirements:{subscription_only:true,read_only:profile.role==='review'},route:{kind:'blocked',value:['mock-no-execution']}}};
  if(holdPreview){holdPreview=false;return new Promise(resolve=>{releasePreview=value=>resolve(value as T);(window as any).__configurationQa.heldValue=value;});}return value as T;
 }
 if(receipts.has(request.commandId))return structuredClone(receipts.get(request.commandId)) as T;
 if(failSave){failSave=false;catalog.revision++;catalog.modes[0]={...catalog.modes[0],version:3,editRevision:2,instructions:'Saved by another window'};throw Error('stale-revision');}
 if(request.expectedRevision!==catalog.revision)throw Error('stale-revision');
 if(request.type==='save-mode'||request.type==='save-profile'){
  const rows=request.type==='save-mode'?catalog.modes:catalog.profiles;
  const index=rows.findIndex(row=>row.id===request.definition.id),previous=rows[index];
  if(previous)history.set(`${request.type==='save-mode'?'mode':'profile'}:${previous.id}:${previous.version}`,structuredClone(previous));
  const next={...structuredClone(request.definition),version:(previous?.version??0)+1,editRevision:(previous?.editRevision??0)+1};
  if(index>=0)(rows as any)[index]=next;else (rows as any).push(next);
 }else if(request.type==='save-scope'){
  const index=catalog.scopes.findIndex(row=>row.scope===request.scope.scope&&(row.projectId??'')===(request.scope.projectId??''));
  const previous=catalog.scopes[index],next={...structuredClone(request.scope),version:(previous?.version??0)+1,editRevision:(previous?.editRevision??0)+1};if(index>=0)catalog.scopes[index]=next;else catalog.scopes.push(next);
 }else{
  const rows=request.kind==='mode'?catalog.modes:catalog.profiles,index=rows.findIndex(row=>row.id===request.id),previous=rows[index];
  history.set(`${request.kind}:${previous.id}:${previous.version}`,structuredClone(previous));(rows as any)[index]={...previous,version:previous.version+1,editRevision:previous.editRevision+1,...(request.type==='archive'?{archived:true}:{instructions:'Native instructions'})};
 }
 catalog.revision++;const result={receipt:{revision:catalog.revision},replayed:false};receipts.set(request.commandId,result);if(loseSave){loseSave=false;throw Error('fixture-response-lost');}return result as T;
}
const root=createRoot(document.getElementById('configuration-fixture')!);
function render(){root.render(<div className="o-settings-content" style={{maxWidth:'920px',margin:'0 auto',fontSize:'inherit'}}><div hidden={!active}><ConfigurationEditor language={language} projects={projects} projectId="project-a" connections={connections} send={send} active={active}/></div></div>);}
(window as any).__configurationQa={calls,catalog,setLanguage(value:Language,scale=100){language=value;document.documentElement.style.fontSize=`${14*scale/100}px`;render();},setModels(id:string,models:unknown[]){const connection=connections.find(value=>value.id===id);if(connection)(connection as any).models=models;render();},setActive(value:boolean){active=value;render();},conflict(){failSave=true;},loseSave(){loseSave=true;},holdPreview(){holdPreview=true;},releasePreview(){const callback=releasePreview;releasePreview=null;callback?.((window as any).__configurationQa.heldValue);},hasHeldPreview(){return releasePreview!==null;}};
render();
