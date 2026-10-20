import {useId} from 'react';
import {capacityFieldLabel,capacityIsCurrent,capacityPercent,capacityResetReached,capacityUsageLabel,type CapacityPresentation} from './capacity-state';
import type {CapacityBucket,CapacityField,CapacityWindow} from './protocol';

type Props={value:CapacityPresentation|null,t:(text:string)=>string,historical?:boolean,preparation?:boolean,now?:number};
export function CapacityObservationView({value,t,historical=false,preparation=false,now=Date.now()}:Props){
 const id=useId(),effective=value?{...value,current:historical?false:value.current}:null;
 const observation=effective?.observation,current=effective?capacityIsCurrent(effective,now):false;
 const time=(milliseconds:unknown)=>typeof milliseconds==='number'&&Number.isFinite(milliseconds)?new Date(milliseconds).toLocaleString():t('Not reported');
 return <section className="o-capacity" aria-labelledby={id}>
  <header className="o-capacity-heading"><h4 id={id}>{t('Subscription capacity')}</h4><span className={`o-capacity-status ${current&&observation?.ordinaryUsage.state==='reported'&&!observation.ordinaryUsage.value?'unavailable':''}`}>{t(capacityUsageLabel(effective,now))}</span></header>
  {!observation?<p className="o-footnote">{t('No capacity observation was recorded. Missing values do not mean zero, unlimited access or an independent quota.')}</p>:<>
   <p className="o-footnote">{t('These are provider observations, not a guarantee that a task, its review and corrections can finish.')}</p>
   {historical&&<p className="o-capacity-warning">{t('Historical capacity only. This record cannot authorize work or confirm a renewal.')}</p>}
   {effective.previous&&<p className="o-capacity-warning">{t('The latest read did not update capacity. The previous observation keeps its original date and values.')}</p>}
   {!current&&!historical&&<p className="o-capacity-warning">{t('Capacity is historical or unconfirmed. A new authorized observation is required before relying on it.')}</p>}
   {!current&&!historical&&observation.ordinaryUsage.state==='reported'&&!observation.ordinaryUsage.value&&<p className="o-capacity-warning">{t('The last observed permission denied included usage. New work remains blocked until a new authorized read explicitly allows it.')}</p>}
   {current&&observation.ordinaryUsage.state==='reported'&&!observation.ordinaryUsage.value&&<p className="o-capacity-warning">{t('The provider does not currently allow ordinary included usage. This does not identify which window caused the restriction. Local controls remain available; no paid API fallback is used.')}</p>}
   <dl className="o-connection-facts"><dt>{t('Reported use permission')}</dt><dd>{observation.ordinaryUsage.state==='reported'?t(observation.ordinaryUsage.value?'Allowed at observation':'Unavailable at observation'):t(capacityFieldLabel(observation.ordinaryUsage))}</dd><dt>{t('Observed at')}</dt><dd>{time(observation.observedAtMs)}</dd><dt>{t('Effective valid until')}</dt><dd>{time(effective.effectiveValidUntilMs)}</dd><dt>{t('Observation validity')}</dt><dd>{t(current?'Current':'Historical or unknown')}</dd><dt>{t('Source')}</dt><dd>{t(observation.synthetic?'Synthetic validation':'Official runtime metadata')}</dd><dt>{t('Completeness')}</dt><dd>{t(observation.completeness==='complete'?'Complete':'Partial')}</dd></dl>
   {preparation&&<p className="o-footnote">{t('Preparation and admission revalidate the selected connection in the task scope. A connection-level observation does not confirm a model-specific quota.')}</p>}
   {observation.buckets.length?<div className="o-capacity-buckets">{observation.buckets.map((bucket,index)=><CapacityBucketView key={`${bucket.reference}:${index}`} bucket={bucket} t={t} now={now}/>)}</div>:<p className="o-footnote">{t('Quantitative windows were not reported. Usage permission is a separate fact.')}</p>}
   <p className="o-footnote">{t('Reported windows may overlap and are never added together. Separate connections, models or sessions do not prove independent capacity.')}</p>
   <details className="o-capacity-details"><summary>{t('Capacity evidence details')}</summary><dl className="o-connection-facts"><dt>{t('Connection')}</dt><dd>{observation.connectionId}</dd><dt>{t('Binding revision')}</dt><dd>{observation.connectionRevision}</dd><dt>{t('Runtime version')}</dt><dd>{observation.runtimeVersion}</dd><dt>{t('Source')}</dt><dd><code>{observation.source}</code></dd><dt>{t('Coverage')}</dt><dd>{t('Reported subset; missing resources remain unknown')}</dd>{effective.diagnosticId&&<><dt>{t('Diagnostic ID')}</dt><dd><code>{effective.diagnosticId}</code></dd></>}<dt>{t('Capacity groups')}</dt><dd>{observation.capacityGroupIds.length?observation.capacityGroupIds.join(' · '):t('No shared capacity group was established')}</dd></dl>
    <p className="o-footnote">{t('Tokens, currency, API credits, concurrency and remaining tasks cannot be inferred from these percentages. Diagnostic storage budgets are separate local limits.')}</p>
    {observation.issues.length>0&&<><h5>{t('Observation limitations')}</h5><ul>{observation.issues.map((issue,index)=><li key={`${issue}:${index}`}><code>{issue}</code></li>)}</ul></>}
    {observation.bucketView==='multi'&&observation.legacyBucket&&<details><summary>{t('Legacy view of the same read')}</summary><p className="o-footnote">{t('Preserved for provenance. This legacy view is not additional capacity and is not added to the resource windows.')}</p><CapacityBucketView bucket={observation.legacyBucket} t={t} now={now}/></details>}
   </details>
  </>}
 </section>;
}
function CapacityBucketView({bucket,t,now}:{bucket:CapacityBucket,t:(text:string)=>string,now:number}){
 const field=(value:CapacityField<string>)=>value.state==='reported'?value.value:t(capacityFieldLabel(value));
 return <section className="o-capacity-bucket">
  <h5>{bucket.name.state==='reported'?bucket.name.value:t('Reported resource')}</h5><p className="o-footnote o-capacity-resource">{t('Resource reference')}: <code>{bucket.reference}</code></p>
  {bucket.reachedReason.state==='reported'&&<p className="o-capacity-warning">{t('Provider restriction reason')}: <code>{bucket.reachedReason.value}</code></p>}
  <div className="o-capacity-windows">{bucket.windows.map(window=><CapacityWindowView key={window.slot} window={window} t={t} now={now}/>)}</div>
  <details className="o-capacity-details"><summary>{t('Resource scope details')}</summary><dl className="o-connection-facts"><dt>{t('Reported limit identifier')}</dt><dd>{field(bucket.limitId)}</dd><dt>{t('Reported name')}</dt><dd>{field(bucket.name)}</dd><dt>{t('Model display alias')}</dt><dd>{field(bucket.modelDisplayAlias)}</dd><dt>{t('Provider restriction reason')}</dt><dd>{field(bucket.reachedReason)}</dd></dl><p className="o-footnote">{t('A display alias is not proof that this limit applies only to the selected model.')}</p></details>
 </section>;
}
function CapacityWindowView({window,t,now}:{window:CapacityWindow,t:(text:string)=>string,now:number}){
 const percentage=capacityPercent(window),used=window.usedPercent;
 const remaining=percentage!==null&&window.remainingPercent?.origin==='derived'&&window.remainingPercent.basis==='100-minus-used-percent'&&window.remainingPercent.value===100-percentage?window.remainingPercent.value:null;
 const reset=window.resetsAtSeconds.state==='reported'?new Date(window.resetsAtSeconds.value*1000).toLocaleString():t(capacityFieldLabel(window.resetsAtSeconds));
 return <section className="o-capacity-window"><h6>{t(window.slot==='primary'?'Primary window':'Secondary window')}</h6>
  {window.state!=='reported'?<p className="o-footnote">{t(capacityFieldLabel({state:window.state}))}</p>:<>
   <dl className="o-connection-facts"><dt>{t('Used in reported window')}</dt><dd>{used.state==='reported'?`${used.value}%`:t(capacityFieldLabel(used))}</dd>{remaining!==null&&<><dt>{t('Remaining percentage (derived)')}</dt><dd>{remaining}%</dd></>}<dt>{t('Window duration')}</dt><dd>{window.durationMinutes.state==='reported'?`${window.durationMinutes.value} ${t('minutes')}`:t(capacityFieldLabel(window.durationMinutes))}</dd><dt>{t('Reported renewal time')}</dt><dd>{reset}</dd></dl>
   {percentage!==null&&<progress className="o-capacity-progress" max={100} value={percentage} aria-label={t('Used in reported window')}>{percentage}%</progress>}
   {used.state==='reported'&&percentage===null&&<p className="o-footnote">{t('The reported percentage is outside the usable range. Its original value is preserved without a bar or derived balance.')}</p>}
   {remaining!==null&&<p className="o-footnote">{t('Remaining percentage is derived as 100 minus reported use, within this window only.')}</p>}
   {capacityResetReached(window,now)&&<p className="o-capacity-warning">{t('The reported renewal time has passed. Restoration is not confirmed; recheck when permitted.')}</p>}
  </>}
 </section>;
}
