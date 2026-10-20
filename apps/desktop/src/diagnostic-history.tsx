import {useEffect,useId,useRef,useState} from 'react';
import {diagnosticLabel} from './connection-state';
import {CapacityObservationView} from './capacity-observation';
import {diagnosticByteSize,diagnosticCapturedValidity,diagnosticHistoryPageSize,diagnosticObservationKey,diagnosticScopeValue,diagnosticStorageBlocked} from './diagnostic-history-state';
import {request,type Connection,type DesktopRequest,type DiagnosticHistoryItem,type DiagnosticHistoryPage,type DiagnosticObservation} from './protocol';

type Send=<T>(operation:DesktopRequest)=>Promise<T>;
export function DiagnosticHistory({connection,t,send=request}:{connection:Connection,t:(s:string)=>string,send?:Send}){
 const panelId=useId(),scope=`${connection.id}:${connection.bindingRevision??connection.revision??0}`;
 const[open,setOpen]=useState(false),[page,setPage]=useState<DiagnosticHistoryPage|null>(null),[cursor,setCursor]=useState<string|null>(null),[trail,setTrail]=useState<(string|null)[]>([]),[loading,setLoading]=useState(false),[error,setError]=useState<string|null>(null);
 const[selected,setSelected]=useState<string|null>(null),[detail,setDetail]=useState<{key:string,value:DiagnosticObservation}|null>(null),[detailLoading,setDetailLoading]=useState(false),[detailError,setDetailError]=useState<string|null>(null);
 const[lookupId,setLookupId]=useState(''),[lookupOrdinal,setLookupOrdinal]=useState('');
 const mounted=useRef(false),epoch=useRef(0),pageRead=useRef(0),detailRead=useRef(0),currentScope=useRef(scope);
 const firstPage=useRef<DiagnosticHistoryPage|null>(null),detailHeading=useRef<HTMLHeadingElement|null>(null),focusDetail=useRef(false);
 currentScope.current=scope;
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;epoch.current++;};},[]);
 useEffect(()=>{epoch.current++;pageRead.current++;detailRead.current++;firstPage.current=null;setPage(null);setCursor(null);setTrail([]);setSelected(null);setDetail(null);setError(null);setDetailError(null);setLoading(false);setDetailLoading(false);setLookupId('');setLookupOrdinal('');setOpen(false);},[scope]);
 useEffect(()=>{if(!focusDetail.current||!detailHeading.current)return;focusDetail.current=false;detailHeading.current.focus({preventScroll:true});detailHeading.current.scrollIntoView({block:'nearest'});},[detail]);
 const current=(version:number,key:string)=>mounted.current&&epoch.current===version&&currentScope.current===key;
 async function load(nextCursor:string|null,nextTrail:(string|null)[]){
  const version=epoch.current,key=scope,read=++pageRead.current;setLoading(true);setError(null);
  try{
   const value=await send<DiagnosticHistoryPage>({type:'connection-diagnostic-history',id:connection.id,cursor:nextCursor,limit:diagnosticHistoryPageSize});
   if(!current(version,key)||pageRead.current!==read)return;
   if(value.connectionId!==connection.id)throw Error('diagnostic-history-scope-mismatch');
   if(nextCursor===null)firstPage.current=value;
   setPage(value);setCursor(nextCursor);setTrail(nextTrail);
  }catch(e){if(current(version,key)&&pageRead.current===read)setError(String(e));}
  finally{if(current(version,key)&&pageRead.current===read)setLoading(false);}
 }
 function newer(){
  // A null cursor starts a new high-water view. Reusing the bounded first
  // page avoids doing that implicitly when navigating back from page two.
  if(trail.length===1&&firstPage.current){setPage(firstPage.current);setCursor(null);setTrail([]);setError(null);return;}
  void load(trail.at(-1)??null,trail.slice(0,-1));
 }
 async function inspect(item:DiagnosticHistoryItem){setSelected(diagnosticObservationKey(item));await loadObservation(item.diagnostic.id,item.ordinal,item);}
 async function loadObservation(diagnosticId:string,ordinal?:number,item?:DiagnosticHistoryItem){
  const version=epoch.current,key=scope,read=++detailRead.current;setDetailLoading(true);setDetailError(null);
  try{
   const value=await send<DiagnosticObservation>({type:'connection-diagnostic-observation',id:connection.id,diagnosticId,ordinal});
   if(!current(version,key)||detailRead.current!==read)return;
   if(value.connectionId!==connection.id||value.historical!==true||value.diagnostic.current!==false||value.diagnostic.id!==diagnosticId||(ordinal!==undefined&&value.ordinal!==ordinal)||item&&(diagnosticObservationKey(value)!==diagnosticObservationKey(item)||value.digest!==item.digest))throw Error('diagnostic-observation-scope-mismatch');
   const observationKey=diagnosticObservationKey(value);setSelected(observationKey);
   focusDetail.current=true;setDetail({key:observationKey,value});
  }catch(e){if(current(version,key)&&detailRead.current===read)setDetailError(String(e));}
  finally{if(current(version,key)&&detailRead.current===read)setDetailLoading(false);}
 }
 const time=(value:unknown)=>typeof value==='number'&&Number.isFinite(value)?new Date(value).toLocaleString():t('Not reported');
 const observed=detail?.key===selected?detail.value:null;
 return <section className="o-diagnostic-history" aria-label={t('Diagnostic history')}>
  <button className="o-history-toggle" aria-expanded={open} aria-controls={panelId} onClick={()=>{setOpen(!open);if(!open&&!page&&!loading)void load(null,[]);}}><span aria-hidden="true">{open?'▾':'▸'}</span><strong>{t('Diagnostic history')}</strong><small>{t('Read on demand')}</small></button>
  <div id={panelId} hidden={!open}>
   <p className="o-footnote">{t('History preserves original evidence. Browsing it never contacts the provider, renews validity or authorizes work.')}</p>
   <details className="o-history-lookup"><summary>{t('Find an exact diagnostic')}</summary><p className="o-footnote">{t('Use an evidence reference from a previous task. An empty transition selects the latest recorded transition of that diagnostic, not a different diagnostic.')}</p><div className="o-refresh-validity"><label>{t('Diagnostic ID')}<input value={lookupId} maxLength={256} onChange={event=>setLookupId(event.target.value)}/></label><label>{t('Transition (optional)')}<input type="number" min={1} step={1} value={lookupOrdinal} onChange={event=>setLookupOrdinal(event.target.value)}/></label></div><button className="o-secondary" disabled={detailLoading||!lookupId.trim()||!!lookupOrdinal&&(!Number.isSafeInteger(Number(lookupOrdinal))||Number(lookupOrdinal)<1)} onClick={()=>void loadObservation(lookupId.trim(),lookupOrdinal?Number(lookupOrdinal):undefined)}>{t('Inspect exact diagnostic')}</button></details>
   <div className="o-actions"><button className="o-secondary" disabled={loading} onClick={()=>void load(null,[])}>{t('Refresh history')}</button>{loading&&<span role="status">{t('Loading history…')}</span>}</div>
   {error&&<div className="o-history-error" role="alert"><p>{t('History could not be loaded. Existing evidence and the current selection are preserved.')}</p><details><summary>{t('Technical details')}</summary><code>{error}</code></details></div>}
   {page&&<>
    <section className={`o-history-storage ${diagnosticStorageBlocked(page.storage)?'blocked':''}`} aria-label={t('Diagnostic storage')}>
     <h4>{t('Diagnostic storage')}</h4><dl className="o-connection-facts"><dt>{t('Operational observations')}</dt><dd>{page.storage.operationalRows} / {page.storage.operationalLimitRows}</dd><dt>{t('Archived observations')}</dt><dd>{page.storage.archivedRows}</dd><dt>{t('Total observations')}</dt><dd>{page.storage.totalRows}</dd><dt>{t('Historical storage used')}</dt><dd>{diagnosticByteSize(page.storage.usedBytes)} / {diagnosticByteSize(page.storage.maximumHistoryBytes)}</dd><dt>{t('Reserved for admitted reads')}</dt><dd>{diagnosticByteSize(page.storage.reservedBytes)}</dd></dl>
     <p className="o-footnote">{t('Completed observations leave the bounded operational window without being deleted. These limits describe local diagnostics, not subscription quota.')}</p>
     <p className="o-footnote">{t('A read can record several transitions. These counts describe stored observations, not provider queries.')}</p>
     {typeof page.storage.globalUsedAndReservedBytes==='number'&&typeof page.storage.maximumGlobalHistoryBytes==='number'&&<p className="o-footnote">{t('All connections, used and reserved')}: {diagnosticByteSize(page.storage.globalUsedAndReservedBytes)} / {diagnosticByteSize(page.storage.maximumGlobalHistoryBytes)}</p>}
     {diagnosticStorageBlocked(page.storage)&&<p className="o-history-blocked" role="status">{t('The historical storage budget blocks new reads. Existing history, cancellation and recovery remain available. Inspect the storage details before requesting another read.')}</p>}
     {diagnosticStorageBlocked(page.storage)&&<p className="o-footnote">{t('This release preserves all evidence and cannot increase the budget. A supported storage update is required before new reads can resume.')}</p>}
     {page.storage.blockedReason&&<p className="o-footnote">{t('Storage status')}: {t(storageReason(page.storage.blockedReason))}</p>}
    </section>
    <p className="o-footnote">{t('This page is a stable historical view. New observations appear only after Refresh history.')}</p>
    {!page.items.length?<p className="o-empty">{t('No diagnostic observations have been recorded.')}</p>:<ol className="o-history-list" aria-label={t('Diagnostic observations')}>{page.items.map(item=><li key={diagnosticObservationKey(item)}><button className="o-history-observation" aria-pressed={selected===diagnosticObservationKey(item)} onClick={()=>void inspect(item)}><strong>{t(historicalState(item.diagnostic.state))}</strong><span className="o-history-badge">{t(item.archived?'Archived':'Recent history')}</span><time>{time(item.diagnostic.observedAtMs??item.diagnostic.finishedAtMs??item.diagnostic.startedAtMs)}</time><small>{t('Observation')} #{item.sequence} · {t('Transition')} {item.ordinal}</small>{item.diagnostic.provenance&&<small>{t(diagnosticLabel(item.diagnostic.provenance))}</small>}</button></li>)}</ol>}
    <nav className="o-history-pagination" aria-label={t('History pages')}><button disabled={loading||!trail.length} onClick={newer}>{t('Newer observations')}</button><span>{t('Page')} {trail.length+1}</span><button disabled={loading||!page.nextCursor} onClick={()=>page.nextCursor&&void load(page.nextCursor,[...trail,cursor])}>{t('Older observations')}</button></nav>
   </>}
   {(selected||detailLoading||detailError)&&<section className="o-history-detail" aria-label={t('Historical observation')}>
    <h4 ref={detailHeading} tabIndex={-1}>{t('Historical observation')}</h4>{detailLoading&&<p role="status">{t('Loading observation…')}</p>}
    {detailError&&<div role="alert" className="o-history-error"><p>{t('This exact observation could not be verified. It has not been replaced by a newer report.')}</p><details><summary>{t('Technical details')}</summary><code>{detailError}</code></details></div>}
    {observed&&<>
     <p className="o-history-warning">{t('Historical evidence only. This record does not establish current identity, access, validity or execution permission.')}</p>
     <dl className="o-connection-facts"><dt>{t('Result')}</dt><dd>{t(historicalState(observed.diagnostic.state))}</dd><dt>{t('Storage location')}</dt><dd>{t(observed.archived?'Archived':'Recent history')}</dd><dt>{t('Source')}</dt><dd>{observed.diagnostic.provenance?t(diagnosticLabel(observed.diagnostic.provenance)):t('Not reported')}</dd><dt>{t('Scope')}</dt><dd>{diagnosticScopeValue(observed.diagnostic,'purpose')?t(diagnosticLabel(String(observed.diagnostic.purpose))):t('Not reported')}</dd><dt>{t('Observed at')}</dt><dd>{time(observed.diagnostic.observedAtMs)}</dd><dt>{t('Read started')}</dt><dd>{time(observed.diagnostic.startedAtMs)}</dd><dt>{t('Read finished')}</dt><dd>{time(observed.diagnostic.finishedAtMs)}</dd><dt>{t('Source valid until')}</dt><dd>{time(observed.diagnostic.validUntilMs)}</dd><dt>{t('Effective valid until')}</dt><dd>{time(observed.diagnostic.effectiveValidUntilMs)}</dd><dt>{t('Completeness')}</dt><dd>{typeof observed.diagnostic.complete==='boolean'?t(observed.diagnostic.complete?'Complete':'Partial'):t('Unknown')}</dd><dt>{t('Binding revision')}</dt><dd>{observed.diagnostic.connectionRevision}</dd></dl>
     <CapacityObservationView value={observed.diagnostic.capacity?{observation:observed.diagnostic.capacity,diagnosticId:observed.diagnostic.id,current:false,effectiveValidUntilMs:observed.diagnostic.effectiveValidUntilMs??observed.diagnostic.validUntilMs,previous:false,identityChanged:observed.diagnostic.identityChanged===true}:null} t={t} historical/>
     {!!observed.diagnostic.errorCode&&<p className="o-footnote">{t('Recorded failure')}: {observed.diagnostic.errorCode}</p>}
     {!!diagnosticCapturedValidity(observed.validity?.captured).length&&<details><summary>{t('Captured validity limits')}</summary><p className="o-footnote">{t('These original duration limits never renew the observation. Source deadlines and stricter historical constraints still apply.')}</p><dl className="o-connection-facts">{diagnosticCapturedValidity(observed.validity?.captured).map(value=><span className="o-history-technical-field" key={value.key}><dt>{t(validityLabel(value.key))}</dt><dd>{value.milliseconds/1000} s</dd></span>)}</dl></details>}
     {observed.diagnostic.stages&&<ol className="o-diagnostic-stages">{observed.diagnostic.stages.map((stage,index)=><li key={`${stage.stage}:${index}`}><strong>{t(diagnosticLabel(stage.stage))}</strong><span>{t(diagnosticLabel(stage.state))}</span>{stage.reason&&<small>{stage.reason}</small>}</li>)}</ol>}
     <details><summary>{t('Evidence identifiers')}</summary><dl className="o-connection-facts"><dt>{t('Diagnostic ID')}</dt><dd><code>{observed.diagnostic.id}</code></dd><dt>{t('Observation')}</dt><dd>{observed.sequence}</dd><dt>{t('Transition')}</dt><dd>{observed.ordinal}</dd><dt>SHA-256</dt><dd><code>{observed.digest}</code></dd><dt>{t('Stored evidence size')}</dt><dd>{diagnosticByteSize(observed.bodyBytes)}</dd>{['scopeKey','registrationRevision','daemonGeneration'].map(field=>{const value=diagnosticScopeValue(observed.diagnostic,field);return value?<div className="o-history-technical-field" key={field}><dt>{t(technicalLabel(field))}</dt><dd>{value}</dd></div>:null;})}{observed.validity&&<><dt>{t('Validity envelope hash')}</dt><dd><code>{observed.validity.digest}</code></dd><dt>{t('Refresh configuration revision')}</dt><dd>{observed.validity.refreshRevision??t('Not reported')}</dd><dt>{t('Policy version')}</dt><dd>{observed.validity.policyVersion??t('Not reported')}</dd></>}</dl></details>
    </>}
   </section>}
  </div>
 </section>;
}
function technicalLabel(field:string){return ({scopeKey:'Scope identity',registrationRevision:'Registration revision',daemonGeneration:'Daemon generation'} as Record<string,string>)[field]??field;}
function validityLabel(field:string){return ({identity:'Identity validity (seconds)',configuration:'Configuration validity (seconds)',availability:'Availability validity (seconds)',models:'Models validity (seconds)'} as Record<string,string>)[field]??field;}
function historicalState(state:string){return ['reserved','pending','running','reading','validating'].includes(state)?'Read started':diagnosticLabel(state);}
function storageReason(reason:string){return ({'diagnostic-history-storage-budget-exhausted':'Historical storage budget reached','history-capacity-exhausted':'Historical storage budget reached','history-storage-exhausted':'Historical storage budget reached','operational-capacity-exhausted':'Operational maintenance required'} as Record<string,string>)[reason]??reason;}
