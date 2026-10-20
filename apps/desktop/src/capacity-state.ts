import type {CapacityField,CapacityObservation,CapacityWindow,Connection,ConnectionDiagnostic,DiagnosticRun,PreparedCapacity} from './protocol';

export type CapacityPresentation={observation:CapacityObservation,diagnosticId?:string,current:boolean,effectiveValidUntilMs?:number|null,previous:boolean,identityChanged:boolean};
export function capacityFieldLabel(field:CapacityField<unknown>|undefined){
 return field?.state==='null'?'Not provided (null)':field?.state==='invalid'?'Uninterpretable value':field?.state==='reported'?'Reported':'Not reported';
}
export function capacityPercent(window:CapacityWindow){
 const used=window.usedPercent;
 return window.state==='reported'&&used?.state==='reported'&&Number.isInteger(used.value)&&used.value>=0&&used.value<=100?used.value:null;
}
// A historical allowance or a passed reset timestamp is never current authority.
export function capacityIsCurrent(value:CapacityPresentation,now=Date.now()){
 return value.current&&!value.identityChanged&&Number.isFinite(value.observation.observedAtMs)&&now>=value.observation.observedAtMs&&typeof value.effectiveValidUntilMs==='number'&&now<value.effectiveValidUntilMs;
}
export function diagnosticCapacity(report:ConnectionDiagnostic|undefined,connectionId?:string,bindingRevision?:number):CapacityPresentation|null{
 const latest=report?.latest;
 const selected:DiagnosticRun|undefined|null=latest?.capacity?latest:report?.lastSuccessful?.capacity?report.lastSuccessful:null;
 const observation=selected?.capacity;
 if(!observation||observation.schemaVersion!==1)return null;
 if(connectionId!==undefined&&observation.connectionId!==connectionId||bindingRevision!==undefined&&observation.connectionRevision!==bindingRevision)return null;
 return {observation,diagnosticId:selected.id,current:selected.current===true,effectiveValidUntilMs:selected.effectiveValidUntilMs??selected.validUntilMs,previous:selected!==latest,identityChanged:report?.identityChanged===true||selected.identityChanged===true};
}
export function preparedCapacity(value:PreparedCapacity|undefined,connectionId:string,bindingRevision?:number):CapacityPresentation|null{
 if(!value||value.observation.connectionId!==connectionId||bindingRevision!==undefined&&value.observation.connectionRevision!==bindingRevision)return null;
 return {...value,previous:false};
}
// Resolve the same explicit connection used by the composer/task. Only an
// automatic choice may use the local default; an unresolved preference stays unknown.
export function selectedCapacity(connections:readonly Connection[],selection:{connectionId?:string|null,bindingRevision?:number,prepared?:PreparedCapacity|null}={}):CapacityPresentation|null{
 const eligible=(connection:Connection)=>connection.enabled!==false&&connection.linked!==false;
 const connection=selection.connectionId!==undefined&&selection.connectionId!==null?connections.find(value=>value.id===selection.connectionId):connections.find(value=>value.selected&&eligible(value))??connections.find(eligible);
 if(!connection||!eligible(connection))return null;
 const revision=connection.bindingRevision??connection.revision;
 if(revision===undefined||selection.bindingRevision!==undefined&&selection.bindingRevision!==revision)return null;
 const observed=diagnosticCapacity(connection.diagnostics,connection.id,revision);
 const pinned=preparedCapacity(selection.prepared??undefined,connection.id,revision);
 const latest=connection.diagnostics?.latest;
 if(pinned&&latest&&latest.id!==pinned.diagnosticId&&latest.connectionRevision===revision&&['complete','partial'].includes(latest.state)&&!latest.capacity)return null;
 // A newer read supersedes the older display snapshot, regardless of its scope.
 // Values are never combined, and the selected evidence keeps its original lease.
 let selected=pinned&&(!observed||pinned.observation.observedAtMs>observed.observation.observedAtMs)?pinned:observed;
 if(selected&&latest&&latest.id===selected.diagnosticId&&latest.connectionRevision===revision){
  const deadline=latest.effectiveValidUntilMs??latest.validUntilMs;
  selected={...selected,current:selected.current&&latest.current===true,effectiveValidUntilMs:typeof deadline==='number'&&typeof selected.effectiveValidUntilMs==='number'?Math.min(deadline,selected.effectiveValidUntilMs):selected.effectiveValidUntilMs};
 }
 return selected&&(connection.identityStatus==='changed'||connection.diagnostics?.identityChanged===true)?{...selected,identityChanged:true}:selected;
}
export function capacityUsageLabel(value:CapacityPresentation|null,now=Date.now()){
 if(!value)return 'Capacity not reported';
 if(!capacityIsCurrent(value,now))return 'Historical capacity observation';
 return value.observation.ordinaryUsage.state==='reported'?(value.observation.ordinaryUsage.value?'Included usage allowed':'Included usage unavailable'):'Usage permission unknown';
}
export function capacityResetReached(window:CapacityWindow,now=Date.now()){
 return window.resetsAtSeconds.state==='reported'&&Number.isFinite(window.resetsAtSeconds.value)&&now>=window.resetsAtSeconds.value*1000;
}
