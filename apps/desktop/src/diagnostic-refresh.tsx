import {useEffect,useRef,useState} from 'react';
import {request,commandId,type Connection,type DesktopRequest,type RefreshSettings,type RefreshStatus} from './protocol';
const defaults:RefreshSettings={schemaVersion:1,mode:'manual',intervalMs:300000,validityMs:{identity:300000,configuration:900000,availability:60000,models:900000},recurringConsent:false};
const reasonLabels:Record<string,string>={manual:'Manual only',running:'Reading metadata',interval:'Waiting for configured interval',backoff:'Waiting after a temporary failure','authorization-required':'Read authorization required','reconfirmation-required':'Identity reconfirmation required','connection-disabled':'Connection disabled','history-capacity-exhausted':'Diagnostic history limit reached','foreground-read':'User-requested read in progress'};
export function DiagnosticRefresh({connection,t,busy,onMutate,onError}:{connection:Connection,t:(s:string)=>string,busy:boolean,onMutate:(r:DesktopRequest,label:string)=>Promise<unknown|undefined>,onError:(s:string)=>void}){
 const scope=JSON.stringify([connection.id,connection.revision??0,connection.bindingRevision??0]);
 const[observed,setObserved]=useState<{scope:string,value:RefreshStatus}|null>(null),[draft,setDraft]=useState<RefreshSettings>(defaults),[consent,setConsent]=useState(false),[loading,setLoading]=useState(false),[saving,setSaving]=useState(false);
 const mounted=useRef(false),selectedScope=useRef(scope),initializedScope=useRef<string|null>(null),selectionEpoch=useRef(0),readEpoch=useRef(0),configurationRevision=useRef<number|null>(null);
 selectedScope.current=scope;
 const status=observed?.scope===scope?observed.value:null;
 function selected(version:number,key:string){return mounted.current&&selectionEpoch.current===version&&selectedScope.current===key;}
 async function load(replaceDraft=false){
  const version=selectionEpoch.current,read=++readEpoch.current,key=scope,id=connection.id;setLoading(true);
  try{
   const value=await request<{refresh:RefreshStatus[]}>({type:'connection-refresh-status',id});
   if(!selected(version,key)||read!==readEpoch.current)return;
   const row=value.refresh.find(r=>r.connectionId===id);
   if(row){
    setObserved({scope:key,value:row});
    if(replaceDraft||configurationRevision.current!==row.refreshRevision){setDraft(row.requested);setConsent(false);}
    configurationRevision.current=row.refreshRevision;
   }
  }catch(e){if(selected(version,key)&&read===readEpoch.current)onError(String(e));}
  finally{if(selected(version,key)&&read===readEpoch.current)setLoading(false);}
 }
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;selectionEpoch.current++;readEpoch.current++;};},[]);
 useEffect(()=>{
  const changed=initializedScope.current!==scope;
  if(changed){initializedScope.current=scope;selectionEpoch.current++;readEpoch.current++;configurationRevision.current=null;setObserved(null);setConsent(false);setSaving(false);}
  void load(changed);
 },[scope,connection.diagnostics?.latest?.id,connection.diagnostics?.latest?.state]);
 async function save(manual=false){
  if(!status||loading||saving)return;const version=selectionEpoch.current,key=scope;
  const settings={...draft,mode:manual?'manual':draft.mode,recurringConsent:manual?false:draft.mode==='automatic'&&(consent||status.recurringAuthorized)} as RefreshSettings;
  setSaving(true);
  try{
   const result=await onMutate({type:'configure-connection-refresh',commandId:commandId(),id:connection.id,expectedRevision:connection.revision??0,expectedRefreshRevision:status.refreshRevision,settings},'configure-connection-refresh');
   if(selected(version,key)&&result!==undefined)await load(true);
  }catch(e){if(selected(version,key))onError(String(e));}
  finally{if(selected(version,key))setSaving(false);}
 }
 const disabled=busy||loading||saving||connection.enabled===false||connection.linked===false;
 return <section className="o-diagnostic-refresh" aria-label={t('Diagnostic refresh')}><h3>{t('Diagnostic refresh')}</h3><p className="o-footnote">{t('Refresh interval and evidence validity are different. Source and mandatory policy limits always apply.')}</p>
  {!status?<p role="status">{t('Loading refresh settings…')}</p>:<>
   <label>{t('Refresh mode')}<select value={draft.mode} disabled={disabled} onChange={e=>{setDraft(d=>({...d,mode:e.target.value as RefreshSettings['mode']}));setConsent(false);}}><option value="manual">{t('Manual only')}</option><option value="automatic">{t('Automatic metadata')}</option></select></label>
   <label>{t('Requested interval (seconds)')}<input type="number" min={1} max={86400} disabled={disabled} value={draft.intervalMs/1000} onChange={e=>setDraft(d=>({...d,intervalMs:Number(e.target.value)*1000}))}/></label>
   <details><summary>{t('Evidence validity by class')}</summary><div className="o-refresh-validity">{(['identity','configuration','availability','models'] as const).map(key=><label key={key}>{t({identity:'Identity validity (seconds)',configuration:'Configuration validity (seconds)',availability:'Availability validity (seconds)',models:'Models validity (seconds)'}[key])}<input type="number" min={1} max={900} disabled={disabled} value={draft.validityMs[key]/1000} onChange={e=>setDraft(d=>({...d,validityMs:{...d.validityMs,[key]:Number(e.target.value)*1000}}))}/></label>)}</div></details>
   {draft.mode==='automatic'&&!status.recurringAuthorized&&<label className="o-checkbox"><input type="checkbox" checked={consent} disabled={disabled} onChange={e=>setConsent(e.target.checked)}/>{t('Allow recurring metadata reads for this connection and registration scope. No model turns or login.')}</label>}
   <div className="o-actions"><button disabled={disabled||draft.mode==='automatic'&&!consent&&!status.recurringAuthorized} onClick={()=>void save()}>{t('Save refresh settings')}</button>{status.recurringAuthorized&&<button disabled={busy||loading||saving} onClick={()=>void save(true)}>{t('Disable recurring reads')}</button>}<button disabled={busy||loading||saving} onClick={()=>void load(true)}>{t('Refresh status')}</button></div>
   <dl className="o-connection-facts"><dt>{t('Recurring reads')}</dt><dd>{t(status.recurringAuthorized?'Authorized for this binding':'Not authorized')}</dd><dt>{t('Refresh status')}</dt><dd>{t(reasonLabels[status.reason]??status.reason)}</dd><dt>{t('Effective interval (seconds)')}</dt><dd>{status.effective ? status.effective.intervalMs/1000 : t('Not reported')}</dd><dt>{t('Next opportunity')}</dt><dd>{status.nextOpportunityAtMs?new Date(status.nextOpportunityAtMs).toLocaleString():t('Not reported')}</dd><dt>{t('Source validity ceiling')}</dt><dd>{(status.sourceTtlMs??60000)/1000}s</dd><dt>{t('Effective settings')}</dt><dd><details><summary>{t('Inspect limits')}</summary><pre className="o-json">{JSON.stringify({reason:status.reason,requested:status.requested,effective:status.effective,policy:status.policyValidityMs},null,2)}</pre></details></dd></dl>
   <p className="o-footnote">{t('Task suspension does not suspend authorized metadata reads. Task directories require their own validation; quota remains unknown when not reported.')}</p>
  </>}
 </section>;
}
