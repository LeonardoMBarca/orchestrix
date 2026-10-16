import type {ConversationWork,WorkerSummary} from './protocol';

const stages:Record<string,[string,string]>={
 reserved:['Request saved','The request is persisted. Runtime execution has not been confirmed.'],
 preparing:['Preparing work','Preparing the authorized isolated worktree.'],
 'supervisor-ready':['Supervisor ready','The owned supervisor is ready. Runtime dispatch has not been confirmed.'],
 'dispatch-planned':['Runtime dispatch planned','The dispatch intent is recorded. A runtime response is not yet confirmed.'],
 'runtime-observed':['Runtime observation recorded','A runtime observation was recorded. Artifact capture and checks may still be pending.'],
 checking:['Checking artifact','Running the authorized checks against the captured artifact.'],
 verified:['Checks passed','The captured artifact passed its configured checks. Independent review is a separate step.'],
 'needs-work':['Checks need attention','Inspect the failed checks before requesting review.'],
 'no-checks':['Captured without checks','No passing test result is claimed for this artifact.'],
 failed:['Work failed','Inspect the recorded failure before taking another action.'],
 cancelled:['Cancelled','The cancellation result is recorded. Inspect any preserved artifacts.'],
 uncertain:['Outcome uncertain','Inspect recovery evidence before admitting more work.'],
 blocked:['Blocked','Inspect the recorded reason and recovery actions.'],
 invalidated:['Evidence invalidated','The artifact needs new verification before review.'],
 'inspection-required':['Inspection required','Inspect recovery evidence before admitting more work.'],
 'set-aside':['Set aside','This attempt was explicitly set aside. It was not delivered.']
};
export function workerStage(status:string):{title:string,description:string}{const value=stages[status];return value?{title:value[0],description:value[1]}:{title:status,description:'No confirmed execution state is available.'};}

// Conversation links are chronological attempts, not separate Tasks. Their
// order and identifiers do not establish an implementation/correction role.
export function conversationAttempt(work:ConversationWork,index:number,workers:WorkerSummary[]){
 const observed=work.workerId?workers.find(worker=>worker.workerId===work.workerId):undefined;
 return {number:index+1,workerId:work.workerId,status:observed?.status,statusLabel:observed?workerStage(observed.status).title:'Not yet observed'};
}
