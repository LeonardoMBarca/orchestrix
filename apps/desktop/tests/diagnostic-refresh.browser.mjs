// Renderer-only QA of periodic diagnostic controls. The native/daemon boundary
// is explicit mock data; no authenticated metadata read or timer is executed.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdir,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {defaultSettings} from '../src/protocol.ts';
import {controlTranslations} from '../src/ox016-locales.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const output=fileURLToPath(new URL(`../../../artifacts/diagnostic-refresh-renderer/${Date.now()}/`,import.meta.url));await mkdir(output,{recursive:true});
const browser=await chromium.launch({channel:'chrome',headless:true});
const page=await browser.newPage({viewport:{width:1440,height:900}});
const errors=[];page.on('pageerror',error=>errors.push(error.message));
const t=(key,language='en')=>language==='en'?key:controlTranslations[key]?.[language==='pt-BR'?0:1]??key;
const checks=[];
try{
 await page.addInitScript(settings=>{
  let sequence=0,holdStatusId=null,deferredStatus=null,holdSaveId=null,deferredSave=null;const calls=[],events=[];
  const requested={schemaVersion:1,mode:'manual',intervalMs:300000,validityMs:{identity:300000,configuration:900000,availability:60000,models:900000},recurringConsent:false};
  const observed=Date.now()-30000;
  let report={latest:{id:'synthetic-original-observation',state:'complete',current:false,complete:true,connectionRevision:3,registrationRevision:5,observedAtMs:observed,validUntilMs:observed+60000,effectiveValidUntilMs:observed+10000,provenance:'synthetic',purpose:'registration',stages:[{stage:'runtime-metadata',state:'complete'}]},catalog:{runtimeVersion:'synthetic-rust-1',connection:{id:'refresh-account',revision:3,authentication:'authenticated',billing:'subscription'},models:[]},lastSuccessfulAtMs:observed,history:[]};
  const connection={id:'refresh-account',revision:5,bindingRevision:3,label:'Synthetic refresh account',name:'Synthetic refresh account',provider:'Codex',mode:'subscription',status:'observed',enabled:true,linked:true,selected:true,identityAssurance:'synthetic',identityStatus:'observed',runtimeVersion:'synthetic-rust-1',diagnostics:report,models:[]};
  const secondConnection={...connection,id:'refresh-account-b',revision:9,bindingRevision:6,label:'Second synthetic account',name:'Second synthetic account',selected:false,diagnostics:{...report,latest:{...report.latest,id:'second-observation',connectionRevision:6,registrationRevision:9},catalog:{connection:{id:'refresh-account-b',revision:6},models:[]}}};
  const state={revision:8,settings,projects:[],conversations:[],connections:[connection,secondConnection],eventCursor:{database:'refresh-renderer',sequence:0},admissionsSuspended:true};
  let refresh={connectionId:connection.id,refreshRevision:0,requested,effective:{intervalMs:300000,validityMs:{identity:60000,configuration:60000,availability:10000,models:60000}},reason:'manual',nextOpportunityAtMs:null,recurringAuthorized:false,sourceTtlMs:60000,policyValidityMs:{identity:300000,configuration:900000,availability:10000,models:900000},limits:{globalConcurrentReads:1,perConnectionConcurrentReads:1,minimumIntervalMs:60000,maximumIntervalMs:86400000,metadataDeadlineMs:15000,maximumBackoffMs:300000,retainedDiagnosticRows:512,remainingDiagnosticRows:509},consentScope:'registration-directory-binding-runtime',activeSource:null};
  const secondRefresh={...structuredClone(refresh),connectionId:secondConnection.id,refreshRevision:2,requested:{...structuredClone(requested),intervalMs:420000},reason:'second-account-status'};
  const daemon=()=>({state:{generation:1,suspended:true,cursor:{database:'refresh-renderer',sequence}},workers:[],workerCount:0,workerFactsHistorical:true,activeWorkerId:null,features:['desktop-v1','connections-v1']});
  const emit=()=>{sequence++;events.push({sequence,kind:'command-finished',body:{commandId:`refresh-change-${sequence}`,state:'finished'}});};
  window.__qa={calls,state,refresh:()=>refresh,report:()=>report,language(language,textScale){state.settings.language=language;state.settings.textScale=textScale;emit();},reason(reason){refresh.reason=reason;},passive(){refresh.reason='passive-status-updated';report={...report,latest:{...report.latest,id:'passive-observation',state:'complete'}};connection.diagnostics=report;emit();},changeConfiguration(){refresh.refreshRevision++;refresh.requested={...refresh.requested,intervalMs:33000};refresh.reason='configuration-revision-updated';report={...report,latest:{...report.latest,id:'configuration-observation'}};connection.diagnostics=report;emit();},deferStatus(id){holdStatusId=id;},hasDeferredStatus(){return deferredStatus!==null;},releaseStatus(){const release=deferredStatus;deferredStatus=null;release?.();},deferSave(id){holdSaveId=id;},hasDeferredSave(){return deferredSave!==null;},releaseSave(){const release=deferredSave;deferredSave=null;release?.();}};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
  window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
   if(command.startsWith('plugin:event|'))return 1;
   if(command==='connect_daemon')return daemon();
   if(command==='desktop_poll')return {daemon:daemon(),cursor:{database:'refresh-renderer',sequence},events:events.filter(e=>e.sequence>args.cursor.sequence),resyncRequired:false};
   if(command==='desktop_request'){
    const r=args.request;calls.push(structuredClone(r));
    if(r.type==='bootstrap')return {...structuredClone(state),daemon:daemon(),eventCursor:{database:'refresh-renderer',sequence}};
    if(r.type==='connection-diagnostics')return structuredClone(r.id===secondConnection.id?secondConnection.diagnostics:report);
    if(r.type==='connection-refresh-status'){const value={refresh:[structuredClone(r.id===secondConnection.id?secondRefresh:refresh)]};if(holdStatusId===r.id){holdStatusId=null;return new Promise(resolve=>{deferredStatus=()=>resolve(value);});}return value;}
    if(r.type==='configure-connection-refresh'){
     if(r.expectedRevision!==connection.revision||r.expectedRefreshRevision!==refresh.refreshRevision)throw Error('refresh-revision-conflict');
     if(r.settings.mode==='automatic'&&!r.settings.recurringConsent)throw Error('recurring-metadata-not-authorized');
     refresh={...refresh,requested:structuredClone(r.settings),refreshRevision:refresh.refreshRevision+1,recurringAuthorized:r.settings.mode==='automatic'&&r.settings.recurringConsent,effective:{intervalMs:Math.max(60000,r.settings.intervalMs),validityMs:{identity:Math.min(60000,r.settings.validityMs.identity),configuration:Math.min(60000,r.settings.validityMs.configuration),availability:Math.min(10000,r.settings.validityMs.availability),models:Math.min(60000,r.settings.validityMs.models)}},nextOpportunityAtMs:r.settings.mode==='automatic'?Date.now()+60000:null,reason:r.settings.mode==='automatic'?'interval':'manual'};
     emit();const value={refresh:structuredClone(refresh)};if(holdSaveId===r.id){holdSaveId=null;return new Promise(resolve=>{deferredSave=()=>resolve(value);});}return value;
    }
    if(r.type==='diagnose-connection'){report={...report,latest:{...report.latest,id:'explicit-manual-diagnostic',state:'running',current:false}};connection.diagnostics=report;emit();return {diagnosticId:'explicit-manual-diagnostic'};}
    if(r.type==='cancel-connection-diagnostic'){if(r.diagnosticId!==report.latest.id)throw Error('wrong-diagnostic');report={...report,latest:{...report.latest,state:'cancelled'}};connection.diagnostics=report;emit();return {cancelled:true};}
    throw Error(`Unexpected refresh renderer operation ${r.type}`);
   }
   throw Error(`Unexpected native refresh command ${command}`);
  }};
 },defaultSettings);
 await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1421/');
 await page.getByRole('heading',{name:'What shall we create today?'}).waitFor();await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByRole('button',{name:'Accounts',exact:true}).click();
 const refresh=()=>page.getByRole('region',{name:'Diagnostic refresh',exact:true});await refresh().waitFor();
 await refresh().getByRole('combobox',{name:'Refresh mode',exact:true}).waitFor();
 assert.equal(await refresh().getByRole('combobox',{name:'Refresh mode',exact:true}).inputValue(),'manual');
 assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='configure-connection-refresh').length),0);
 const original=await page.evaluate(()=>structuredClone(window.__qa.report().latest));
 checks.push('manual-default-no-recurring-authority');
 await refresh().getByRole('combobox',{name:'Refresh mode',exact:true}).selectOption('automatic');
 const consent=refresh().getByRole('checkbox');assert.equal(await consent.isChecked(),false);assert.equal(await refresh().getByRole('button',{name:'Save refresh settings',exact:true}).isDisabled(),true);
 await refresh().getByRole('spinbutton',{name:'Requested interval (seconds)',exact:true}).fill('1');
 await refresh().getByText('Evidence validity by class',{exact:true}).click();
 await refresh().getByRole('spinbutton',{name:'Availability validity (seconds)',exact:true}).fill('5');
 await consent.focus();await page.keyboard.press('Space');
 await refresh().getByRole('button',{name:'Save refresh settings',exact:true}).click();
 await refresh().getByRole('button',{name:'Disable recurring reads',exact:true}).waitFor();
 const configured=await page.evaluate(()=>window.__qa.calls.find(c=>c.type==='configure-connection-refresh'));
 assert.equal(configured.expectedRevision,5);assert.equal(configured.expectedRefreshRevision,0);assert.equal(configured.settings.intervalMs,1000);assert.equal(configured.settings.recurringConsent,true);assert.equal(configured.settings.validityMs.availability,5000);
 const effective=await page.evaluate(()=>structuredClone(window.__qa.refresh()));assert.equal(effective.effective.intervalMs,60000);assert.equal(effective.effective.validityMs.availability,5000);
 assert.equal(effective.sourceTtlMs,60000);assert.deepEqual(await page.evaluate(()=>window.__qa.report().latest),original,'settings do not renew or rewrite a historical observation');
 await refresh().getByText('Inspect limits',{exact:true}).click();const limits=JSON.parse(await refresh().locator('.o-json').innerText());assert.equal(limits.requested.intervalMs,1000);assert.equal(limits.effective.intervalMs,60000);
 assert.deepEqual(limits.limits,effective.limits,'inspect limits presents the exact backend projection, not values inferred from requested settings');
 assert.equal(limits.limits.minimumIntervalMs,60000);assert.equal(limits.limits.metadataDeadlineMs,15000);assert.equal(limits.limits.retainedDiagnosticRows,512);assert.equal(limits.limits.remainingDiagnosticRows,509,'remaining history capacity is the observed backend value, not the total limit');
 assert.equal(limits.consentScope,'registration-directory-binding-runtime');assert.equal(limits.activeSource,null,'an idle projection must not invent an active read source');
 checks.push('explicit-binding-consent','requested-vs-effective-cadence-and-class-validity','source-observation-not-renewed','backend-limits-and-history-are-observed-projections');

 const beforePoll=await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='connection-refresh-status').length);
 // Two visual poll cycles: they may read compact state, never request diagnostics.
 await page.waitForTimeout(2600);
 assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='connection-refresh-status').length),beforePoll,'no refresh-status timer per window');
 assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='diagnose-connection').length),0,'enabling cadence does not call Diagnose from the renderer');
 const detail=page.getByRole('region',{name:'Connection details',exact:true});assert.equal(await detail.getByRole('button',{name:'Recheck connection',exact:true}).isDisabled(),true,'recurring consent does not authorize a separate one-off read');
 await refresh().getByRole('button',{name:'Disable recurring reads',exact:true}).click();
 await refresh().getByRole('combobox',{name:'Refresh mode',exact:true}).getByRole('option',{name:'Manual only',exact:true}).waitFor({state:'attached'});
 await page.waitForFunction(()=>window.__qa.refresh().requested.mode==='manual');
 const configurations=await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='configure-connection-refresh'));
 assert.equal(configurations.length,2);assert.equal(configurations[1].settings.mode,'manual');assert.equal(configurations[1].settings.recurringConsent,false);
 assert.equal(await page.evaluate(()=>window.__qa.calls.some(c=>c.type==='cancel-connection-diagnostic')),false);
 checks.push('no-window-owned-refresh-timer','disable-recurrence-is-not-cancel');

 await detail.getByRole('checkbox',{name:'Allow a bounded metadata read for this connection, without a model turn.',exact:true}).check();await detail.getByRole('button',{name:'Recheck connection',exact:true}).click();
 await detail.getByRole('button',{name:'Cancel diagnostic',exact:true}).click();
 const cancellation=await page.evaluate(()=>window.__qa.calls.find(c=>c.type==='cancel-connection-diagnostic'));assert.equal(cancellation.diagnosticId,'explicit-manual-diagnostic');assert.equal(await page.evaluate(()=>window.__qa.refresh().requested.mode),'manual');
 checks.push('one-off-recheck-has-separate-consent','exact-round-cancel-keeps-cadence-distinct');
 await page.evaluate(()=>window.__qa.reason('backoff'));
 await refresh().getByRole('button',{name:'Refresh status',exact:true}).click();
 await refresh().getByText('Waiting after a temporary failure',{exact:true}).waitFor();assert.deepEqual(await page.evaluate(()=>window.__qa.report().latest.observedAtMs),original.observedAtMs);
 checks.push('status-read-no-provider-query');

 const geometries=[];
 for(const language of ['en','pt-BR','es'])for(const scale of [100,200])for(const width of [1440,900]){
  await page.setViewportSize({width,height:width===900?620:900});await page.evaluate(({language,scale})=>window.__qa.language(language,scale),{language,scale});
  await page.waitForFunction(({language,scale})=>document.documentElement.lang===language&&getComputedStyle(document.documentElement).fontSize===`${14*scale/100}px`,{language,scale});
  const region=page.getByRole('region',{name:t('Diagnostic refresh',language),exact:true});
  await region.getByRole('combobox',{name:t('Refresh mode',language),exact:true}).selectOption('automatic');
  const checkbox=region.getByRole('checkbox');await checkbox.scrollIntoViewIfNeeded();await checkbox.focus();await page.keyboard.press('Space');assert.equal(await checkbox.isChecked(),true);await page.keyboard.press('Space');
  const save=region.getByRole('button',{name:t('Save refresh settings',language),exact:true});await save.scrollIntoViewIfNeeded();
  const bounds=await save.boundingBox();assert(bounds&&bounds.x>=0&&bounds.x+bounds.width<=width+1,`refresh save remains reachable: ${JSON.stringify({language,scale,width,bounds})}`);
  const box=await page.locator('.o-settings-content').evaluate(e=>({width:e.clientWidth,scrollWidth:e.scrollWidth,rootFont:getComputedStyle(document.documentElement).fontSize}));assert(box.scrollWidth<=box.width+1,`refresh content reflows: ${JSON.stringify({language,scale,width,box})}`);assert.equal(box.rootFont,`${14*scale/100}px`);geometries.push({language,scale,viewportWidth:width,...box});
  if(width===900)await page.screenshot({path:`${output}${language}-${scale}-compact.png`});
 }
 const calls=await page.evaluate(()=>window.__qa.calls);assert.equal(calls.filter(c=>c.type==='configure-connection-refresh').length,2,'editing translations/scale does not save recurring authority');assert.equal(calls.filter(c=>c.type==='diagnose-connection').length,1,'only the separately authorized one-off read was requested');
 assert.deepEqual(calls.filter(c=>['submit-task','prepare-task','request-review','authorize-correction','link-connection','control'].includes(c.type)),[]);assert.deepEqual(errors,[]);
 checks.push('three-languages-two-scales-two-widths','keyboard-consent-and-responsive-controls','no-runtime-or-admission-effects');
 // Scope races are driven through the normal account selection and controls.
 // Only the mocked local projection is delayed, never provider metadata.
 await page.setViewportSize({width:1440,height:900});await page.evaluate(()=>window.__qa.language('en',100));
 await page.waitForFunction(()=>document.documentElement.lang==='en'&&getComputedStyle(document.documentElement).fontSize==='14px');
 await refresh().getByRole('combobox',{name:'Refresh mode',exact:true}).selectOption('manual');
 await refresh().getByRole('spinbutton',{name:'Requested interval (seconds)',exact:true}).fill('17');
 await page.evaluate(()=>window.__qa.passive());await refresh().getByText('passive-status-updated',{exact:true}).waitFor();
 assert.equal(await refresh().getByRole('spinbutton',{name:'Requested interval (seconds)',exact:true}).inputValue(),'17','passive diagnostic updates preserve unsaved configuration');
 await refresh().getByRole('button',{name:'Refresh status',exact:true}).click();
 assert.equal(await refresh().getByRole('spinbutton',{name:'Requested interval (seconds)',exact:true}).inputValue(),'1','explicit refresh reloads the saved configuration');
 await refresh().getByRole('spinbutton',{name:'Requested interval (seconds)',exact:true}).fill('19');await page.evaluate(()=>window.__qa.changeConfiguration());await refresh().getByText('configuration-revision-updated',{exact:true}).waitFor();
 assert.equal(await refresh().getByRole('spinbutton',{name:'Requested interval (seconds)',exact:true}).inputValue(),'33','a new configuration revision replaces an obsolete draft');
 checks.push('passive-observation-preserves-draft','explicit-refresh-reloads-draft','configuration-revision-reloads-draft');

 await page.evaluate(()=>window.__qa.deferStatus('refresh-account'));await refresh().getByRole('button',{name:'Refresh status',exact:true}).click();await page.waitForFunction(()=>window.__qa.hasDeferredStatus());
 await page.getByRole('button',{name:/^Second synthetic account/}).click();await refresh().getByText('second-account-status',{exact:true}).waitFor();await page.evaluate(()=>window.__qa.releaseStatus());await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
 assert.equal(await refresh().getByRole('spinbutton',{name:'Requested interval (seconds)',exact:true}).inputValue(),'420','late account A projection cannot replace account B draft');
 assert.equal(await refresh().getByRole('combobox',{name:'Refresh mode',exact:true}).inputValue(),'manual');
 checks.push('late-status-response-cannot-cross-account-selection');

 await page.getByRole('button',{name:/^Synthetic refresh account/}).click();await refresh().getByText('configuration-revision-updated',{exact:true}).waitFor();
 await refresh().getByRole('spinbutton',{name:'Requested interval (seconds)',exact:true}).fill('44');await page.evaluate(()=>window.__qa.deferSave('refresh-account'));await refresh().getByRole('button',{name:'Save refresh settings',exact:true}).click();await page.waitForFunction(()=>window.__qa.hasDeferredSave());
 await page.getByRole('button',{name:/^Second synthetic account/}).click();await refresh().getByText('second-account-status',{exact:true}).waitFor();
 const aReads=await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='connection-refresh-status'&&c.id==='refresh-account').length);
 await page.evaluate(()=>window.__qa.releaseSave());await page.waitForFunction(()=>!document.querySelector('.o-diagnostic-refresh select')?.disabled);await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
 assert.equal(await refresh().getByRole('spinbutton',{name:'Requested interval (seconds)',exact:true}).inputValue(),'420');
 assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='connection-refresh-status'&&c.id==='refresh-account').length),aReads,'a save completing after selection changed cannot start an account A follow-up read');
 checks.push('save-followup-cannot-cross-account-selection');

 await page.evaluate(()=>window.__qa.deferStatus('refresh-account-b'));await refresh().getByRole('button',{name:'Refresh status',exact:true}).click();await page.waitForFunction(()=>window.__qa.hasDeferredStatus());
 await page.getByRole('button',{name:'General',exact:true}).click();await refresh().waitFor({state:'hidden'});await page.getByRole('button',{name:'Accounts',exact:true}).click();await refresh().locator('dd').getByText('Manual only',{exact:true}).waitFor();await page.evaluate(()=>window.__qa.releaseStatus());await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
 assert.equal(await refresh().getByRole('spinbutton',{name:'Requested interval (seconds)',exact:true}).inputValue(),'44','an unmounted account B read cannot overwrite the reopened account A controls');
 const finalCalls=await page.evaluate(()=>window.__qa.calls);assert.equal(finalCalls.filter(c=>c.type==='configure-connection-refresh').length,3);assert.equal(finalCalls.filter(c=>c.type==='diagnose-connection').length,1);assert.deepEqual(finalCalls.filter(c=>['submit-task','prepare-task','request-review','authorize-correction','link-connection','control'].includes(c.type)),[]);assert.deepEqual(errors,[]);
 checks.push('unmounted-status-response-ignored');
 const result={status:'passed',kind:'renderer-mock-only',checks,geometries,screenshots:output};await writeFile(`${output}result.json`,JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}finally{await browser.close();}
