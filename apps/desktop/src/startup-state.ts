// Native observations describe availability; only the daemon handshake grants Ready.
export type StartupObservation={state:'initializing'|'recovering'|'blocked'|'failed',phase?:string|null,phaseSource:string,hostElapsedMs:number,daemonElapsedMs?:number|null,automaticFollowUp:boolean,followUpBudgetMs:number,explicitConnectAvailable?:boolean};
const phases=new Set(['ownership','database','restoration','worker-recovery','review-recovery','integration-recovery','pending-commands','ipc-binding','ipc-available']);
export function startupObservation(value:unknown):StartupObservation|null {
 const s=(value as {startup?:StartupObservation}|null)?.startup;
 if(!s)return null;
 if(s.explicitConnectAvailable!==undefined&&typeof s.explicitConnectAvailable!=='boolean')throw new Error('invalid-startup-observation');
 if(!['initializing','recovering','blocked','failed'].includes(s.state)||typeof s.automaticFollowUp!=='boolean'||!Number.isFinite(s.hostElapsedMs)||s.hostElapsedMs<0||!Number.isFinite(s.followUpBudgetMs)||s.followUpBudgetMs<15000||s.followUpBudgetMs>300000||typeof s.phaseSource!=='string'||s.phase!=null&&!phases.has(s.phase))throw new Error('invalid-startup-observation');
 return {...s,automaticFollowUp:s.automaticFollowUp&&(s.state==='initializing'||s.state==='recovering')&&s.hostElapsedMs<s.followUpBudgetMs};
}
export function startupLabel(s:StartupObservation):string {
 return s.state==='recovering'?'Recovering saved work…':s.state==='blocked'?'Startup is not yet confirmed':s.state==='failed'?'Daemon startup failed':'Initializing Core…';
}
