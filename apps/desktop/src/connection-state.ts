import type {Connection,ConnectionDiagnostic} from './protocol';
import {diagnosticCapacity,capacityUsageLabel} from './capacity-state.ts';

export function selectableConnection(connection:Connection){return connection.enabled!==false&&connection.linked!==false;}
export function diagnosticRunning(report:ConnectionDiagnostic|undefined){return !!report?.latest&&['reserved','pending','running','reading','validating'].includes(report.latest.state);}
export function diagnosticStillCurrent(id:string,revision:number,selection:{id:string,revision:number}|null,generation:number,currentGeneration:number){return !!selection&&selection.id===id&&selection.revision===revision&&generation===currentGeneration;}
export function exactConnectionChoice(connections:Connection[],id:string){return connections.find(connection=>connection.id===id&&selectableConnection(connection));}
export function connectionLabel(connection:Connection){return connection.name??connection.label;}
// Retain a detailed body only for the exact transition returned by the compact
// snapshot. Fresh validity/identity flags win; a new transition needs its own read.
export function mergeDiagnosticProjection(detail:ConnectionDiagnostic,projection:ConnectionDiagnostic|undefined):ConnectionDiagnostic|undefined{
 if(!projection)return undefined;
 const merge=(stored:ConnectionDiagnostic['latest'],fresh:ConnectionDiagnostic['latest'])=>fresh&&stored&&fresh.id===stored.id&&fresh.state===stored.state&&fresh.connectionRevision===stored.connectionRevision?{...stored,...fresh}:fresh;
 return {...detail,...projection,latest:merge(detail.latest,projection.latest),lastSuccessful:merge(detail.lastSuccessful,projection.lastSuccessful)};
}
// A local detail response may arrive after another window advanced diagnostics.
// It can hydrate only the exact transitions still named by the current snapshot.
export function acceptDiagnosticDetail(detail:ConnectionDiagnostic,projection:ConnectionDiagnostic|undefined):ConnectionDiagnostic|null{
 if(!projection)return null;
 const same=(incoming:ConnectionDiagnostic['latest'],fresh:ConnectionDiagnostic['latest'])=>!incoming&&!fresh||!!incoming&&!!fresh&&incoming.id===fresh.id&&incoming.state===fresh.state&&incoming.connectionRevision===fresh.connectionRevision;
 if(!same(detail.latest,projection.latest)||!same(detail.lastSuccessful,projection.lastSuccessful))return null;
 return mergeDiagnosticProjection(detail,projection)??null;
}
// An unhydrated catalog is not an instruction to discard a saved preference.
// This is a display placeholder, never an observed model or admission grant.
export function retainedModelPreference(connections:readonly Connection[],connectionId:string,modelId:string){
 if(!modelId)return null;
 const connection=connections.find(connection=>connection.id===connectionId);
 if(connection?.models?.some(model=>model.id===modelId))return null;
 return {value:`${connectionId}|${modelId}`,connectionId,modelId,connectionLabel:connection?.label??connectionId};
}
export function connectionError(error:string){
 if(error.includes('auth-store-already-linked'))return 'This Codex home is already linked. Use the existing connection.';
 if(error.includes('connection-binding-revision-required')||error.includes('connection-binding-revision-changed'))return 'The connection configuration changed. Select it again and authorize a new preparation.';
 if(error.includes('connection-revision'))return 'The connection was updated in another window. Refresh it before continuing.';
 return error;
}
export function reportedModels(report:ConnectionDiagnostic,connection:Connection){
 const descriptor=report.catalog;
 if(!descriptor||descriptor.connection?.id!==connection.id||descriptor.connection?.revision!==(connection.bindingRevision??connection.revision)||!Array.isArray(descriptor.models))return [];
 return descriptor.models.map((model:any)=>{const presentation=report.catalogLabels?.find(row=>row.id===model.id);return {id:String(model.id),name:presentation?.label??'Model label not reported',efforts:Array.isArray(model.reasoningLevels)?model.reasoningLevels.filter((level:unknown)=>typeof level==='string'):[],nativeEfforts:presentation?.nativeEfforts??[]};});
}
export function diagnosticLabel(value:string){return ({running:'Reading runtime metadata',complete:'Read completed',partial:'Partial response',failed:'Read failed',cancelled:'Cancelled',interrupted:'Interrupted',backoff:'Waiting before another read',coalesced:'Read already in progress','runtime-metadata':'Runtime metadata','official-read-only-metadata':'Official runtime metadata',synthetic:'Synthetic validation',unknown:'Unknown','observed-label':'Limited observed account details',unconfirmed:'Not confirmed',observed:'Observed account details',changed:'Observed account details changed',unlinked:'Unlinked','task-preparation':'Project preparation scope',registration:'Connection scope'} as Record<string,string>)[value]??value;}
export type ResourceFact={id:string,label:string,state:'reported'|'unknown'|'restricted'|'declared'|'unsupported',description:string,value?:string};
export function connectionResourceFacts(report:ConnectionDiagnostic,connection?:Pick<Connection,'id'|'bindingRevision'|'revision'>):ResourceFact[]{
 const catalog=report.catalog,controls=report.controlObservations??[],hasCatalog=Array.isArray(catalog?.models),capacity=diagnosticCapacity(report,connection?.id,connection?.bindingRevision??connection?.revision);
 return [
  {id:'runtime',label:'Installed runtime',state:typeof catalog?.runtimeVersion==='string'?'reported':'unknown',description:'Runtime version observed by the adapter',value:catalog?.runtimeVersion},
  {id:'authentication',label:'Authentication and billing',state:catalog?.connection?.authentication==='authenticated'?'reported':'unknown',description:catalog?.connection?.billing==='subscription'?'Subscription observed; account identity remains limited':'Authentication or billing not reported'},
  {id:'models',label:'Models and reasoning',state:hasCatalog?'reported':'unknown',description:'Catalog presence does not confirm execution access',value:hasCatalog?String(catalog.models.length):undefined},
  {id:'context',label:'Context management',state:catalog?.capabilities?.contextManagement?.support==='runtime-managed'?'declared':'unknown',description:catalog?.capabilities?.contextManagement?.support==='runtime-managed'?'Managed by the runtime; numeric limits are not reported':'Context support not reported'},
  {id:'structured-output',label:'Structured results',state:report.structuredResultSupport===true?'declared':report.structuredResultSupport===false?'unsupported':'unknown',description:report.structuredResultSupport===true?'Declared for this pinned adapter profile; no turn was run':report.structuredResultSupport===false?'Not supported by this adapter profile':'Structured-result support not reported or unavailable'},
  {id:'permissions',label:'Permissions and delegation',state:controls.length?'restricted':'unknown',description:controls.length?'Adapter restrictions are reported; applied permissions require an attempt observation':'Permission controls not reported'},
  {id:'speed',label:'Speed and fast mode',state:'unknown',description:'No speed or fast-mode observation is available'},
  {id:'usage',label:'Quota and usage',state:capacity?'reported':'unknown',description:capacity?'Provider-reported capacity with original date; execution remains subject to admission guards':'No reliable remaining quota or usage was reported',value:capacity?capacityUsageLabel(capacity):undefined},
 ];
}
export function diagnosticNextAction(report:ConnectionDiagnostic,now=Date.now()){
 const run=report.latest;const deadline=run?.effectiveValidUntilMs??run?.validUntilMs;if(!run)return 'Link a runtime and authorize metadata to begin a diagnostic.';
 if(report.identityChanged||run.identityChanged)return 'Review changed observed account details and explicitly confirm the link before new work.';
 if(run.state==='backoff')return 'Wait until the next permitted refresh; retry does not bypass limits.';
 if(run.state==='cancelled')return 'The read was cancelled. No capability was marked unsupported.';
 if(run.state==='coalesced')return 'A matching metadata read is already being observed; wait or recheck when permitted.';
 if((['complete','partial'].includes(run.state)||run.capacity)&&(run.current===false||typeof deadline==='number'&&now>=deadline))return 'The observation is stale. Recheck before preparing new work.';
 if(run.capacity?.ordinaryUsage.state==='reported'&&!run.capacity.ordinaryUsage.value)return 'Included usage is unavailable. Recheck when permitted; local controls and historical evidence remain available.';
 if(run.state==='partial')return 'Some catalog data was not returned. Available observations remain valid; missing resources remain unknown.';
 const code=run.errorCode??'';
 if(/authentication|login/.test(code))return 'Sign in through the official Codex flow, then recheck this link.';
 if(/billing|policy|configuration/.test(code))return 'Review the runtime subscription and configuration; paid API fallback is not allowed.';
 if(/version/.test(code))return 'Select the supported runtime version or relink its executable.';
 if(/ownership|permission/.test(code))return 'Review native access and runtime configuration, then recheck.';
 if(['failed','interrupted'].includes(run.state))return 'The read did not complete. Historical evidence is preserved; recheck when permitted.';
 return diagnosticRunning(report)?'The daemon is reading authorized metadata. No model turn is running.':'Metadata was read; model access and applied settings remain unconfirmed.';
}
