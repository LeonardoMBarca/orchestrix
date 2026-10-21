import {useEffect,useRef,useState} from 'react';
import {request,type DesktopRequest,type GitStatusResult,type RepositoryInspectionResult,type RepositoryPreflight} from './protocol';
import {repositoryBlockerLabel,repositoryOutcome,repositoryReplyCurrent,repositoryResponseSupported} from './repository-inspection-state';

type Send=<T>(operation:DesktopRequest)=>Promise<T>;
export function RepositoryInspection({directoryGrant,disabled,t,onResult,onInspectStart,send=request}:{directoryGrant:string,disabled:boolean,t:(text:string)=>string,onResult:(result:RepositoryInspectionResult|null)=>void,onInspectStart?:()=>void,send?:Send}){
 const[inspection,setInspection]=useState<RepositoryInspectionResult|null>(null),[accepted,setAccepted]=useState(false),[status,setStatus]=useState<{grant:string,value:GitStatusResult}|null>(null),[pending,setPending]=useState<'inspect'|'status'|null>(null),[error,setError]=useState<{grant:string,operation:string,code:string}|null>(null);
 const mounted=useRef(false),epoch=useRef(0),selected=useRef(directoryGrant),callback=useRef(onResult),starting=useRef(onInspectStart);selected.current=directoryGrant;callback.current=onResult;starting.current=onInspectStart;
 useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;epoch.current++;};},[]);
 useEffect(()=>{epoch.current++;setInspection(null);setAccepted(false);setStatus(null);setPending(null);setError(null);callback.current(null);},[directoryGrant]);
 async function load(kind:'inspect'|'status'){
  if(!directoryGrant||disabled||pending)return;
  const grant=directoryGrant,version=++epoch.current;setPending(kind);setError(null);if(kind==='inspect'){setAccepted(false);callback.current(null);starting.current?.();}
  const active=()=>repositoryReplyCurrent(grant,selected.current,version,epoch.current,mounted.current);
  try{
   if(kind==='inspect'){
    const report=await send<RepositoryPreflight>({type:'inspect-repository',directoryGrant:grant});if(!active())return;
    if(!repositoryResponseSupported(report))throw Error('repository-inspection-response-incompatible');
    const result={directoryGrant:grant,report};setInspection(result);setAccepted(true);callback.current(result);
   }else{
    const value=await send<GitStatusResult>({type:'git-status',directoryGrant:grant});if(!active())return;
    if(value.route!=='deterministic'||value.modelCalled!==false||typeof value.output!=='string'||typeof value.truncated!=='boolean')throw Error('git-status-response-incompatible');
    setStatus({grant,value});
   }
  }catch(reason){if(active())setError({grant,operation:kind,code:String(reason)});}
  finally{if(active())setPending(null);}
 }
 const report=inspection?.directoryGrant===directoryGrant?inspection.report:null,git=status?.grant===directoryGrant?status.value:null,problem=error?.grant===directoryGrant?error:null;
 return <section className="o-repository-inspection" aria-label={t('Repository inspection')}>
  <div className="o-task-section-heading"><h2>{t('Repository inspection')}</h2><div className="o-actions"><button className="o-secondary" disabled={!directoryGrant||disabled||pending!==null} onClick={()=>void load('inspect')}>{t('Inspect repository')}</button><button className="o-secondary" disabled={!directoryGrant||disabled||pending!==null} onClick={()=>void load('status')}>{t('Git status')}</button></div></div>
  <p className="o-footnote">{t('These local reads use the selected folder permission and do not call a model, contact a provider or authorize execution.')}</p>
  {!directoryGrant&&<p className="o-footnote">{t('Select a repository folder before inspecting it.')}</p>}
  {directoryGrant&&!report&&!pending&&!problem&&<p className="o-footnote">{t('Inspect this repository to check the worker profile before preparing a task.')}</p>}
  {pending&&<p role="status">{t(pending==='inspect'?'Inspecting repository…':'Reading Git status…')}</p>}
  {problem&&<div className="o-notice" role="alert"><p>{t(problem.operation==='inspect'?'Repository inspection failed. Preparation requires a successful compatible inspection.':'Git status could not be read. Existing inspection and draft are preserved.')}</p><details><summary>{t('Technical details')}</summary><code>{problem.code}</code></details></div>}
  {report&&<>{!accepted&&<p className="o-footnote">{t('Previous inspection is preserved for reference. Inspect again before preparing.')}</p>}<p className="o-repository-outcome" role="status"><strong>{!accepted&&`${t('Previous inspection')}: `}{t(repositoryOutcome(report))}</strong></p>
   {!report.applicationEligible&&<p className="o-notice">{t(report.compatible?'Final application is not eligible in this checkout. A compatible worker may still run; Core revalidates the target before any application.':'Final application is blocked for this repository profile.')}</p>}
   <dl className="o-connection-facts"><dt>{t('Inspected HEAD')}</dt><dd><code>{report.headOid||t('Not reported')}</code></dd><dt>{t('Files inspected')}</dt><dd>{report.inventory.contentInspected} / {report.inventory.files}</dd><dt>{t('Checkout state')}</dt><dd>{t(report.checkout.clean?'Clean checkout':'Local changes observed')}</dd></dl>
   {report.blockers.length>0&&<section className="o-repository-blockers"><h3>{t('Profile restrictions')}</h3><ul>{report.blockers.map((blocker,index)=><li key={`${blocker.code}:${index}`}><strong>{t(repositoryBlockerLabel(blocker.code))} ({blocker.count})</strong><code>{blocker.code}</code>{blocker.examples.length>0&&<ul>{blocker.examples.map((example,item)=><li key={item}><code>{example}</code></li>)}</ul>}</li>)}</ul></section>}
   <details><summary>{t('Inspection details and limits')}</summary><dl className="o-connection-facts"><dt>{t('Repository profile')}</dt><dd>{report.profile}</dd><dt>{t('Pinned base')}</dt><dd><code>{report.baseOid||t('Not reported')}</code></dd><dt>{t('File inventory')}</dt><dd>{report.inventory.files} / {report.limits.files}</dd><dt>{t('Inventory bytes / read budget')}</dt><dd>{report.inventory.bytes} / {report.limits.readBytes} B</dd><dt>{t('Largest file')}</dt><dd>{report.inventory.largestFileBytes} / {report.limits.fileBytes} B</dd>{(['regular','executables','links','submodules','oversized','binary','nonUtf8','secretPattern','excluded','contentSkipped'] as const).map(key=><div className="o-repository-technical-field" key={key}><dt>{t(inventoryLabel(key))}</dt><dd>{report.inventory[key]}</dd></div>)}<dt>{t('Untracked files')}</dt><dd>{report.checkout.untracked}</dd><dt>{t('Ignored paths')}</dt><dd>{report.checkout.ignored}</dd><dt>{t('Completeness')}</dt><dd>{t(report.complete?'Complete':'Partial')}</dd></dl>{report.checkout.status&&<pre className="o-repository-status">{report.checkout.status}</pre>}{report.checkout.truncated&&<p className="o-notice">{t('Status output was limited. This is not a complete inventory.')}</p>}</details>
   {report.inventory.secretPattern>0&&<p className="o-footnote">{t('Sensitive pattern matches are conservative profile restrictions, not proof that these files contain real credentials.')}</p>}
   {report.inventory.contentSkipped>0&&<p className="o-footnote">{t('Content counts cover only files read within the limits. Skipped file contents were not classified.')}</p>}
   <p className="o-footnote">{t('This inspection is an observation, not execution authority. Core checks the repository again before metadata and effects; later edits can change eligibility.')}</p>
  </>}
  {git&&<details className="o-repository-git-status" open><summary>{t('Git status result')}</summary><p className="o-footnote">{t('Deterministic local query — no model called')}</p><pre className="o-repository-status">{git.output||t('No changes reported')}</pre>{git.truncated&&<p className="o-notice">{t('Status output was limited. This is not a complete inventory.')}</p>}</details>}
 </section>;
}
function inventoryLabel(key:string){return ({regular:'Regular files',executables:'Executable files',links:'Links',submodules:'Submodules',oversized:'Oversized files',binary:'Binary files',nonUtf8:'Non-UTF-8 files',secretPattern:'Sensitive patterns',excluded:'Excluded paths',contentSkipped:'Content not inspected'} as Record<string,string>)[key]??key;}
