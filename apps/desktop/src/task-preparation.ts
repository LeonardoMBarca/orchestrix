import type {TaskDraft} from './protocol';

// Preparation and consent belong to one immutable request, not its form identity.
export function taskFingerprint(task:TaskDraft):string{return JSON.stringify(task);}
export function preparationStillCurrent(expected:string,current:TaskDraft):boolean{return expected===taskFingerprint(current);}

export function activeFileDestination(grants:{id:string,directory:boolean}[],activeGrant?:string):string {
 return grants.find(grant=>grant.id===activeGrant&&grant.directory)?.id??'';
}

export function dockTab(items:('navigation'|'sessions')[],current:'navigation'|'sessions',key:string):'navigation'|'sessions'|null {
 if(!items.length)return null;
 if(key==='Home')return items[0];
 if(key==='End')return items.at(-1)!;
 if(key!=='ArrowLeft'&&key!=='ArrowRight')return null;
 const index=Math.max(0,items.indexOf(current));
 return items[(index+(key==='ArrowRight'?1:items.length-1))%items.length];
}
