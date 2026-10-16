import {reviewMatchesArtifact} from './state.ts';

export function integrationStage(status:string):string{return ({reserved:'Integration request saved',preparing:'Preparing integration',materialized:'Integration worktree ready',checking:'Checking integration',checked:'Integration checks passed','commit-planned':'Result commit planned',integrated:'Result commit recorded',prepared:'Candidate prepared',approved:'Candidate approved',applying:'Applying candidate',applied:'Candidate applied',delivered:'Delivered',blocked:'Blocked',invalidated:'Evidence invalidated',uncertain:'Outcome uncertain'} as Record<string,string>)[status]??status;}

// Recovery receives historical summaries, not permission to expose future
// guardian/binding fields or to retry an integration. Keep evidence explicit.
export function integrationRecoveryEvidence(input:Record<string,unknown>){
 const text=(value:unknown)=>typeof value==='string'?value:undefined;
 const target=input.target&&typeof input.target==='object'?input.target as Record<string,unknown>:{};
 return {
  id:text(input.id),worker_id:text(input.worker_id),run_id:text(input.run_id),task_id:text(input.task_id),
  source_artifact_revision:text(input.source_artifact_revision),status:text(input.status),reason:input.reason===null?null:text(input.reason),
  input_base:text(input.input_base),result_revision:input.result_revision===null?null:text(input.result_revision),candidate_revision:input.candidate_revision===null?null:text(input.candidate_revision),
  delivered:typeof input.delivered==='boolean'?input.delivered:undefined,profile:text(input.profile),
  target:{directory:text(target.directory),common_git_dir:text(target.common_git_dir),symbolic_ref:text(target.symbolic_ref),head:text(target.head),index_sha256:text(target.index_sha256),working_sha256:text(target.working_sha256)},
  checks:input.checks===null?null:Array.isArray(input.checks)?input.checks.map((report:Record<string,unknown>)=>({id:text(report.id),artifact_revision:text(report.artifact_revision),result:text(report.result),exit_code:typeof report.exit_code==='number'?report.exit_code:null,command_hash:text(report.command_hash),executable_hash:text(report.executable_hash),tree_confirmed:typeof report.tree_confirmed==='boolean'?report.tree_confirmed:undefined,streams_closed:typeof report.streams_closed==='boolean'?report.streams_closed:undefined})):undefined
 };
}

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
