// Renderer-only synthetic evidence. This entry point is not part of main.tsx;
// it never creates a daemon, a database, a runtime or authenticated metadata.
import {createRoot} from 'react-dom/client';
import {DiagnosticHistory} from '../src/diagnostic-history';
import {translate,type Language} from '../src/i18n';
import type {Connection,DesktopRequest,DiagnosticHistoryItem,DiagnosticHistoryPage} from '../src/protocol';
import '../src/styles.css';
const root=createRoot(document.getElementById('history-fixture')!),calls:DesktopRequest[]=[];
const connection:Connection={id:'history-account',bindingRevision:1,revision:1,label:'Synthetic history account',provider:'Codex',mode:'subscription',status:'observed'};
const rows:DiagnosticHistoryItem[]=Array.from({length:600},(_,index)=>{const sequence=index+1;return {sequence,ordinal:sequence%2?1:2,archived:sequence<=100,digest:sequence.toString(16).padStart(64,'0'),bodyBytes:2048,diagnostic:{id:`synthetic-round-${Math.ceil(sequence/2)}`,state:sequence%2?'running':'complete',connectionRevision:1,purpose:'registration',provenance:'synthetic',observedAtMs:1700000000000+sequence,startedAtMs:1700000000000+sequence-100,finishedAtMs:1700000000000+sequence+100,validUntilMs:1700000060000+sequence,effectiveValidUntilMs:1700000005000+sequence,complete:true,current:false,descriptor:{notForDisplay:'large-descriptor-canary'}}};});
let language:Language='en',budgetBlocked=false,failPage=false,corruptDetail=false;
let deferPage=false,heldPage:((value:DiagnosticHistoryPage)=>void)|null=null,heldValue:DiagnosticHistoryPage|null=null;
async function send<T>(operation:DesktopRequest):Promise<T>{calls.push(structuredClone(operation));
 if(operation.type==='connection-diagnostic-history'){
  if(failPage){failPage=false;throw Error('synthetic-history-read-failed');}
  const cursor=operation.cursor?JSON.parse(operation.cursor):null,highWater=cursor?.highWater??rows.at(-1)!.sequence,before=cursor?.before??highWater+1;
  const found=rows.filter(row=>row.sequence<=highWater&&row.sequence<before).reverse().slice(0,operation.limit??25);
  const value:DiagnosticHistoryPage={connectionId:operation.id,items:structuredClone(found),nextCursor:found.at(-1)!.sequence>1?JSON.stringify({highWater,before:found.at(-1)!.sequence}):null,highWaterSequence:highWater,storage:{operationalRows:500,archivedRows:rows.length-500,totalRows:rows.length,usedBytes:budgetBlocked?268435456:rows.length*2048,reservedBytes:4096,operationalLimitRows:512,maximumHistoryBytes:268435456,globalUsedAndReservedBytes:rows.length*2048+4096,maximumGlobalHistoryBytes:1073741824,storageBlocked:budgetBlocked}};
  if(deferPage){deferPage=false;return await new Promise<T>(resolve=>{heldPage=value=>resolve(value as T);heldValue=value;});}return value as T;
 }
 if(operation.type==='connection-diagnostic-observation'){
  const row=rows.findLast(row=>row.diagnostic.id===operation.diagnosticId&&(operation.ordinal===undefined||row.ordinal===operation.ordinal));if(!row)throw Error('synthetic-history-not-found');
  return {...structuredClone(row),digest:corruptDetail?'f'.repeat(64):row.digest,diagnostic:{...row.diagnostic,scopeKey:'a'.repeat(64),registrationRevision:1,daemonGeneration:1},connectionId:operation.id,historical:true,validity:{captured:{identity:5000,availability:5000,configuration:5000,models:5000},digest:'e'.repeat(64),refreshRevision:2,policyVersion:3}} as T;
 }
 throw Error(`Unexpected renderer-only history operation ${operation.type}`);
}
function render(){root.render(<div className="o-settings-content" style={{maxWidth:800,margin:'0 auto'}}><DiagnosticHistory connection={{...connection}} t={text=>translate(language,text)} send={send}/></div>);}
(window as any).__historyQa={calls,rows,language(value:Language,scale=100){language=value;document.documentElement.lang=value;document.documentElement.style.fontSize=`${14*scale/100}px`;render();},arrive(){rows.push({...structuredClone(rows.at(-1)!),sequence:rows.length+1,ordinal:1,diagnostic:{...rows.at(-1)!.diagnostic,id:`arriving-round-${rows.length+1}`,state:'running'}});render();},passive(){connection.diagnostics={latest:rows.at(-1)!.diagnostic};render();},block(){budgetBlocked=true;},fail(){failPage=true;},corrupt(value:boolean){corruptDetail=value;},defer(){deferPage=true;},hasDeferred(){return heldPage!==null;},release(){const callback=heldPage;heldPage=null;if(heldValue)callback?.(heldValue);},rebind(){connection.bindingRevision!++;connection.revision!++;render();}};
render();
