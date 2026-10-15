import {reviewMatchesArtifact} from './state.ts';

export function integrationStage(status:string):string{return ({reserved:'Integration request saved',preparing:'Preparing integration',materialized:'Integration worktree ready',checking:'Checking integration',checked:'Integration checks passed','commit-planned':'Result commit planned',integrated:'Result commit recorded',prepared:'Candidate prepared',approved:'Candidate approved',applying:'Applying candidate',applied:'Candidate applied',delivered:'Delivered',blocked:'Blocked',invalidated:'Evidence invalidated',uncertain:'Outcome uncertain'} as Record<string,string>)[status]??status;}

export function integrationPreparationBarriers(records:{worker_id?:string,source_artifact_revision?:string,status?:string,result_revision?:string|null}[],source:{workerId:string,revision:string}):{recordedResult:boolean,recoveryRequired:boolean,candidateExists:boolean}{
 const own=records.filter(record=>record.worker_id===source.workerId);
 return {
  recordedResult:own.some(record=>typeof record.result_revision==='string'&&record.result_revision.length>0),
  recoveryRequired:own.some(record=>['reserved','preparing','materialized','checking','checked','commit-planned','integrated','applying','applied','uncertain'].includes(record.status??'')),
  candidateExists:own.some(record=>record.source_artifact_revision===source.revision&&['prepared','approved','delivered'].includes(record.status??''))
 };
}

export type IntegrationRevalidationPin={commandId:string,integrationId:string,candidateRevision:string,epoch:number,unknownEpoch:number};
export function confirmsIntegrationRevalidation(pin:IntegrationRevalidationPin,event:{commandId?:string,workerId?:string,state?:string},current:{id:string,candidate_revision?:string,status?:string},epoch:number,unknownEpoch:number):boolean{
 return pin.commandId===event.commandId&&pin.integrationId===event.workerId&&event.state==='finished'&&current.id===pin.integrationId&&current.candidate_revision===pin.candidateRevision&&['prepared','approved','invalidated','blocked'].includes(current.status??'')&&pin.epoch===epoch&&pin.unknownEpoch===unknownEpoch;
}

export function integrationFooter(records:{id?:string,worker_id?:string,source_artifact_revision?:string,status?:string,delivered?:boolean}[],source:{workerId:string,revision:string},candidateStale=false):string|null{
 const latest=[...new Map(records.map((record,index)=>[record.id??`unknown-${index}`,record])).values()];
 const exact=latest.filter(record=>record.worker_id===source.workerId&&record.source_artifact_revision===source.revision);
 if(exact.some(record=>record.status==='delivered'&&record.delivered===true))return 'Historical delivery recorded for this exact artifact.';
 if(candidateStale&&exact.some(record=>['prepared','approved'].includes(record.status??'')))return 'Candidate verification required before approval or application.';
 if(exact.some(record=>record.status==='approved'))return 'Candidate approved · Awaiting explicit application';
 if(exact.some(record=>record.status==='prepared'))return 'Candidate prepared · Awaiting explicit approval';
 return null;
}

export function canPrepareIntegration(source:{workerId:string,revision:string,status:string,stale:boolean,reviews:{status:string,worker_id?:string,artifact_revision?:string}[]},blocked:boolean):boolean{
 return !blocked&&!source.stale&&source.status==='verified'&&!!source.revision&&source.reviews.some(review=>review.status==='accepted'&&reviewMatchesArtifact(review,source.workerId,source.revision));
}
export function integrationAction(summary:{worker_id?:string,source_artifact_revision?:string,status?:string,candidate_revision?:string,delivered?:boolean},source:{workerId:string,revision:string},blocked:boolean):'approve'|'apply'|null{
 if(blocked||summary.delivered||summary.worker_id!==source.workerId||summary.source_artifact_revision!==source.revision||!summary.candidate_revision)return null;
 return summary.status==='prepared'?'approve':summary.status==='approved'?'apply':null;
}
