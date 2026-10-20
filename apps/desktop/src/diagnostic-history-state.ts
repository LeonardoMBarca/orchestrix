import type {DiagnosticHistoryItem,DiagnosticStorage} from './protocol';

export const diagnosticHistoryPageSize=20;
export function diagnosticObservationKey(item:Pick<DiagnosticHistoryItem,'sequence'|'ordinal'>){return `${item.sequence}:${item.ordinal}`;}
export function diagnosticStorageBlocked(storage:DiagnosticStorage){
 return storage.storageBlocked===true||storage.usedBytes+storage.reservedBytes>=storage.maximumHistoryBytes||(typeof storage.globalUsedAndReservedBytes==='number'&&typeof storage.maximumGlobalHistoryBytes==='number'&&storage.globalUsedAndReservedBytes>=storage.maximumGlobalHistoryBytes);
}
export function diagnosticByteSize(value:number){
 if(!Number.isFinite(value)||value<0)return '—';
 if(value<1024)return `${value} B`;
 const exponent=Math.min(Math.floor(Math.log(value)/Math.log(1024)),3);
 return `${(value/1024**exponent).toFixed(1)} ${['B','KiB','MiB','GiB'][exponent]}`;
}
// Only scalar, named evidence fields are presented. Catalogs and other large
// descriptors remain at the host boundary instead of becoming a JSON dump.
export function diagnosticScopeValue(run:Record<string,unknown>,field:string){
 const value=run[field];
 return typeof value==='string'||typeof value==='number'?String(value):null;
}
export function diagnosticCapturedValidity(captured:Record<string,unknown>|undefined){
 const candidate=captured?.validity??captured;
 if(!candidate||typeof candidate!=='object'||Array.isArray(candidate))return [];
 const record=candidate as Record<string,unknown>;
 return (['identity','configuration','availability','models'] as const).flatMap(key=>typeof record[key]==='number'?[{key,milliseconds:record[key] as number}]:[]);
}
