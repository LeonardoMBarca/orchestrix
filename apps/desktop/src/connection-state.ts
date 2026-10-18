import type {Connection,ConnectionDiagnostic} from './protocol';

export function selectableConnection(connection:Connection){return connection.enabled!==false&&connection.linked!==false;}
export function diagnosticRunning(report:ConnectionDiagnostic|undefined){return !!report?.latest&&['reserved','pending','running','reading','validating'].includes(report.latest.state);}
export function diagnosticStillCurrent(id:string,revision:number,selection:{id:string,revision:number}|null,generation:number,currentGeneration:number){return !!selection&&selection.id===id&&selection.revision===revision&&generation===currentGeneration;}
export function exactConnectionChoice(connections:Connection[],id:string){return connections.find(connection=>connection.id===id&&selectableConnection(connection));}
export function connectionLabel(connection:Connection){return connection.name??connection.label;}
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
export function connectionResourceFacts(report:ConnectionDiagnostic):ResourceFact[]{
 const catalog=report.catalog,controls=report.controlObservations??[],hasCatalog=Array.isArray(catalog?.models);
 return [
  {id:'runtime',label:'Installed runtime',state:typeof catalog?.runtimeVersion==='string'?'reported':'unknown',description:'Runtime version observed by the adapter',value:catalog?.runtimeVersion},
  {id:'authentication',label:'Authentication and billing',state:catalog?.connection?.authentication==='authenticated'?'reported':'unknown',description:catalog?.connection?.billing==='subscription'?'Subscription observed; account identity remains limited':'Authentication or billing not reported'},
  {id:'models',label:'Models and reasoning',state:hasCatalog?'reported':'unknown',description:'Catalog presence does not confirm execution access',value:hasCatalog?String(catalog.models.length):undefined},
  {id:'context',label:'Context management',state:catalog?.capabilities?.contextManagement?.support==='runtime-managed'?'declared':'unknown',description:catalog?.capabilities?.contextManagement?.support==='runtime-managed'?'Managed by the runtime; numeric limits are not reported':'Context support not reported'},
  {id:'structured-output',label:'Structured results',state:report.structuredResultSupport===true?'declared':report.structuredResultSupport===false?'unsupported':'unknown',description:report.structuredResultSupport===true?'Declared for this pinned adapter profile; no turn was run':report.structuredResultSupport===false?'Not supported by this adapter profile':'Structured-result support not reported or unavailable'},
  {id:'permissions',label:'Permissions and delegation',state:controls.length?'restricted':'unknown',description:controls.length?'Adapter restrictions are reported; applied permissions require an attempt observation':'Permission controls not reported'},
  {id:'speed',label:'Speed and fast mode',state:'unknown',description:'No speed or fast-mode observation is available'},
  {id:'usage',label:'Quota and usage',state:'unknown',description:'No reliable remaining quota or usage was reported'},
 ];
}
export function diagnosticNextAction(report:ConnectionDiagnostic,now=Date.now()){
 const run=report.latest;if(!run)return 'Link a runtime and authorize metadata to begin a diagnostic.';
 if(report.identityChanged||run.identityChanged)return 'Review changed observed account details and explicitly confirm the link before new work.';
 if(run.state==='backoff')return 'Wait until the next permitted refresh; retry does not bypass limits.';
 if(run.state==='cancelled')return 'The read was cancelled. No capability was marked unsupported.';
 if(run.state==='coalesced')return 'A matching metadata read is already being observed; wait or recheck when permitted.';
 if(['complete','partial'].includes(run.state)&&(run.current===false||typeof run.validUntilMs==='number'&&now>=run.validUntilMs))return 'The observation is stale. Recheck before preparing new work.';
 if(run.state==='partial')return 'Some catalog data was not returned. Available observations remain valid; missing resources remain unknown.';
 const code=run.errorCode??'';
 if(/authentication|login/.test(code))return 'Sign in through the official Codex flow, then recheck this link.';
 if(/billing|policy|configuration/.test(code))return 'Review the runtime subscription and configuration; paid API fallback is not allowed.';
 if(/version/.test(code))return 'Select the supported runtime version or relink its executable.';
 if(/ownership|permission/.test(code))return 'Review native access and runtime configuration, then recheck.';
 if(['failed','interrupted'].includes(run.state))return 'The read did not complete. Historical evidence is preserved; recheck when permitted.';
 return diagnosticRunning(report)?'The daemon is reading authorized metadata. No model turn is running.':'Metadata was read; model access and applied settings remain unconfirmed.';
}
