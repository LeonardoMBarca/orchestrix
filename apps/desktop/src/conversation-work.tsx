import {conversationAttempt} from './worker-presentation';
import type {ConversationWork,WorkerSummary} from './protocol';

export function ConversationWorkCards({work,workers,onInspect,t}:{work:ConversationWork[],workers:WorkerSummary[],onInspect:(workerId:string)=>void,t:(text:string)=>string}){
 if(!work.length)return null;
 return <div className="o-conversation-work">{work.map((link,index)=>{
  const attempt=conversationAttempt(link,index,workers),title=t('Attempt {number}').replace('{number}',String(attempt.number));
  return <section className="o-work-card o-conversation-attempt" key={link.attemptId??link.workerId??link.runId} aria-label={title}>
   <header><h3>{title}</h3><span className="o-status">{t(attempt.statusLabel)}</span></header>
   <p className="o-footnote">{t('Historical observation')}</p>
   <details className="o-worker-identity"><summary>{t('Attempt identity')}</summary><dl className="o-attempt-identity">
    <dt>{t('Run')}</dt><dd>{link.runId}</dd>
    {link.taskId&&<><dt>{t('Task')}</dt><dd>{link.taskId}</dd></>}
    {link.attemptId&&<><dt>{t('Attempt')}</dt><dd>{link.attemptId}</dd></>}
    {link.workerId&&<><dt>{t('Worker')}</dt><dd>{link.workerId}</dd></>}
    {link.revision&&<><dt>{t('Recorded revision')}</dt><dd>{link.revision}</dd></>}
   </dl></details>
   <div className="o-actions"><button className="o-secondary" disabled={!attempt.workerId} aria-label={`${t('Show evidence')} · ${title}`} onClick={()=>{if(attempt.workerId)onInspect(attempt.workerId);}}>{t('Show evidence')}</button></div>
  </section>;
 })}</div>;
}
