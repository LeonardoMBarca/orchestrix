import type {Activity,Side} from './protocol';
export type Layout={navigation:Side,sessions:Side,leftWidth:number,rightWidth:number,activeLeft:'navigation'|'sessions',activeRight:'navigation'|'sessions',activityOrder:Activity[]};
export const initialLayout:Layout={navigation:'left',sessions:'right',leftWidth:224,rightWidth:280,activeLeft:'navigation',activeRight:'sessions',activityOrder:['chat','files','studio','connections']};
export function movePanel(layout:Layout,panel:'navigation'|'sessions',side:Side):Layout{return {...layout,[panel]:side,[side==='left'?'activeLeft':'activeRight']:panel};}
export function selectActivity(layout:Layout,activity:Activity):Layout{if(activity==='chat')return layout;const side=layout.navigation;return {...layout,[side==='left'?'activeLeft':'activeRight']:'navigation',[side==='left'?'leftWidth':'rightWidth']:Math.max(layout[side==='left'?'leftWidth':'rightWidth'],side==='left'?224:260)};}
export function resizeDock(layout:Layout,side:Side,width:number):Layout{return {...layout,[side==='left'?'leftWidth':'rightWidth']:width<90?0:Math.max(160,Math.min(480,width))};}
export function moveActivity(layout:Layout,activity:Activity,direction:number):Layout{const order=[...layout.activityOrder],a=order.indexOf(activity),b=a+direction;if(b<0||b>=order.length)return layout;[order[a],order[b]]=[order[b],order[a]];return {...layout,activityOrder:order};}
export function uniqueEvents<T extends {sequence:number}>(old:T[],incoming:T[]):T[]{const result=new Map(old.map(e=>[e.sequence,e]));for(const event of incoming)result.set(event.sequence,event);return [...result.values()].sort((a,b)=>a.sequence-b.sequence).slice(-200);}
export function sessionForFirstMessage(content:string,existing:string|null,id:()=>string):string|null{return content.trim()?existing??id():null;}

export type RevalidationPin={commandId:string,workerId:string,revision:string,epoch:number};
export function confirmsRevalidation(pin:RevalidationPin,event:{commandId?:string,workerId?:string,state?:string},current:{workerId:string,revision:string,status:string},epoch:number):boolean {
 return pin.commandId===event.commandId&&pin.workerId===event.workerId&&event.state==='finished'&&pin.workerId===current.workerId&&pin.revision===current.revision&&current.status==='verified'&&pin.epoch===epoch;
}
export function reviewMatchesArtifact(review:{worker_id?:string,artifact_revision?:string},workerId:string,revision:string):boolean{return review.worker_id===workerId&&review.artifact_revision===revision;}

// Daemon snapshots intentionally omit details. They update status without
// erasing exact-version checks, reviews or immutable manifests already inspected.
export function mergeCompactWorker<T extends {summary:{workerId:string,[key:string]:unknown}}>(detail:T,compact:{workerId:string,status:string}):T {
 if(detail.summary.workerId!==compact.workerId)return detail;
 return {...detail,summary:{...detail.summary,...compact}};
}
