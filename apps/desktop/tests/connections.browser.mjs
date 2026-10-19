// Renderer-only regression using actual connection DTO shapes. No login,
// metadata/provider process or native grant is executed by this mock.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {defaultSettings} from '../src/protocol.ts';
import {connectionTranslations} from '../src/connection-locales.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const browser=await chromium.launch({channel:'chrome',headless:true});
const page=await browser.newPage({viewport:{width:1442,height:794}});
const errors=[];page.on('pageerror',error=>errors.push(error.message));
async function assertConsentAdjacency(context){
 const geometry=await page.locator('.o-connection-detail label.o-checkbox').evaluate(label=>{
  const input=label.querySelector('input'),control=input.getBoundingClientRect();
  const text=[...label.childNodes].find(node=>node.nodeType===Node.TEXT_NODE&&node.textContent.trim());
  if(!text)throw Error('The consent label must expose its text beside the checkbox');
  const index=text.textContent.search(/\S/u),range=document.createRange();range.setStart(text,index);range.setEnd(text,index+1);
  const first=range.getBoundingClientRect(),style=getComputedStyle(label);
  return {controlWidth:control.width,textGap:first.left-control.right,labelFont:parseFloat(style.fontSize)};
 });
 assert(geometry.controlWidth>0&&geometry.controlWidth<=24,`consent control must retain its intrinsic width: ${JSON.stringify({context,geometry})}`);
 assert(geometry.textGap>=0&&geometry.textGap<=16,`consent text must be adjacent to its checkbox: ${JSON.stringify({context,geometry})}`);
 return geometry;
}
try{
 await page.addInitScript(settings=>{
  let sequence=0,grant=0;const calls=[],events=[];
  const connection={id:'connection-one',revision:1,bindingRevision:1,name:'My Codex',label:'My Codex',provider:'Codex',mode:'subscription',status:'observed',linked:true,enabled:true,selected:true,identityAssurance:'observed-label',identityStatus:'observed',runtimeVersion:'0.162.0-alpha.17.2',models:[]};
  let report={latest:{id:'diagnostic-one',state:'running',connectionRevision:1,registrationRevision:1,startedAtMs:Date.now(),current:true,purpose:'registration'},catalog:null,history:[]};
  const state={revision:1,settings,projects:[{id:'project-one',name:'My project'}],conversations:[{id:'conversation-one',title:'Prepare arithmetic',projectId:'project-one',messageCount:1}],connections:[],eventCursor:{database:'connections-test',sequence:0},admissionsSuspended:false};
  const daemon=()=>({state:{generation:1,suspended:false,cursor:{database:'connections-test',sequence}},workers:[],activeWorkerId:null,features:['desktop-v1','review-v1','connections-v1']});
  const emit=()=>{sequence++;events.push({sequence,kind:'command-finished',body:{commandId:`changed-${sequence}`,state:'finished'}});};
  window.__qa={calls,state,connection,finish(){report={latest:{...report.latest,state:'partial',complete:false,observedAtMs:Date.now(),validUntilMs:Date.now()+60000,provenance:'official-read-only-metadata',stages:[{stage:'runtime-metadata',state:'partial'}]},catalog:{runtimeVersion:'0.162.0-alpha.17.2',connection:{id:connection.id,revision:connection.bindingRevision,authentication:'authenticated',billing:'subscription'},models:[{id:'model-opaque',reasoningLevels:['low','maximum']}],capabilities:{contextManagement:{support:'runtime-managed'}}},catalogLabels:[{id:'model-opaque',label:'Readable provider model',nativeEfforts:['low','xhigh']}],controlObservations:['control:native-delegation-disabled'],structuredResultSupport:true,history:[]};connection.diagnostics=report;emit();},rename(){connection.name=connection.label='Renamed';connection.revision++;emit();},rebind(){connection.bindingRevision++;connection.revision++;emit();},language(language,scale){state.settings.language=language;state.settings.textScale=scale;emit();}};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
  window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
   if(command.startsWith('plugin:event|'))return 1;
   if(command==='connect_daemon')return daemon();
   if(command==='desktop_poll')return {daemon:daemon(),cursor:{database:'connections-test',sequence},events:events.filter(e=>e.sequence>args.cursor.sequence),resyncRequired:false};
   if(command==='choose_document')return {id:`grant-${++grant}`,name:`Authorized folder ${grant}`,directory:true};
   if(command==='desktop_request'){
    const r=args.request;calls.push(structuredClone(r));
    if(r.type==='bootstrap')return {...structuredClone(state),connections:state.connections.map(()=>structuredClone(connection)),eventCursor:{database:'connections-test',sequence},daemon:daemon()};
    if(r.type==='detect-codex-candidates')return {candidates:[{id:'opaque-executable',kind:'executable',location:'C:/fixture/codex.exe',source:'native-path'},{id:'opaque-home',kind:'home',location:'C:/fixture/codex-home',source:'native-home'}]};
    if(r.type==='inspect-runtime-version')return {version:'0.162.0-alpha.17.2',supported:true,executableSha256:'a'.repeat(64),inferenceUsed:false,authenticationRead:false};
    if(r.type==='link-connection'){connection.diagnostics=report;state.connections=[connection];return {id:connection.id,revision:1,bindingRevision:1,linked:true,diagnosticId:report.latest.id,metadataRequested:true};}
    if(r.type==='connection-diagnostics')return structuredClone(report);
    // Persisted status projection only: opening the controls must not start a
    // provider read or implicitly authorize recurring metadata.
    if(r.type==='connection-refresh-status')return {refresh:[{connectionId:connection.id,refreshRevision:0,requested:{schemaVersion:1,mode:'manual',intervalMs:300000,validityMs:{identity:300000,configuration:900000,availability:60000,models:900000},recurringConsent:false},effective:{intervalMs:300000,validityMs:{identity:60000,configuration:60000,availability:60000,models:60000}},reason:'manual-refresh-only',nextOpportunityAtMs:null,recurringAuthorized:false,sourceTtlMs:60000,policyValidityMs:{identity:300000,configuration:900000,availability:60000,models:900000}}]};
    if(r.type==='diagnose-connection'){report.latest={...report.latest,id:'diagnostic-two',state:'running'};connection.diagnostics=report;return {diagnosticId:'diagnostic-two'};}
    if(r.type==='cancel-connection-diagnostic'){report.latest={...report.latest,state:'cancelled'};connection.diagnostics=report;return {cancelled:true};}
    if(r.type==='conversation')return {id:'conversation-one',projectId:'project-one',messages:[{id:'message-one',ordinal:1,role:'user',text:'Fix arithmetic'}],draft:'',work:[],more:false,nextOrdinal:1};
    if(r.type==='prepare-task')return {id:'prepared-exact',task:{connection_revision:r.draft.connectionRevision,objective:r.draft.objective,checks:[]}};
    if(r.type==='save-draft')return {};
   }
   throw Error(`Unexpected native mock command ${command} ${args.request?.type}`);
  }};
 },defaultSettings);
 await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1421/');
 await page.getByRole('heading',{name:'What shall we create today?'}).waitFor();
 await page.getByRole('button',{name:/^Connect account/}).first().click();
 const dialog=page.getByRole('dialog',{name:'Connect account',exact:true});
 await dialog.getByRole('button',{name:/^API /}).click();
 await dialog.getByText('API adapters are not available in this build. No API key is requested or saved, and Subscription Only never falls back to paid APIs.').waitFor();
 assert.equal(await dialog.locator('input[type=password]').count(),0);
 await dialog.getByRole('button',{name:'Back',exact:true}).click();
 await dialog.getByRole('button',{name:/^Subscription /}).click();
 await dialog.getByRole('button',{name:/^Codex /}).click();
 await dialog.getByRole('button',{name:'C:/fixture/codex.exe',exact:true}).click();
 await dialog.getByRole('button',{name:'C:/fixture/codex-home',exact:true}).click();
 assert.equal(await dialog.getByRole('button',{name:'Link and diagnose',exact:true}).isDisabled(),true);
 await dialog.getByRole('button',{name:'Check runtime version',exact:true}).click();
 await dialog.getByText('0.162.0-alpha.17.2 · Supported version',{exact:true}).waitFor();
 await dialog.getByLabel('Allow a bounded metadata read for this connection, without a model turn.',{exact:true}).check();
 await dialog.getByRole('button',{name:'Link and diagnose',exact:true}).click();
 await page.locator('.o-connection-detail').waitFor();
 await assertConsentAdjacency('normal English 100%');
 await page.getByRole('status').filter({hasText:'Diagnostic state: Reading runtime metadata'}).waitFor();
 const linked=await page.evaluate(()=>window.__qa.calls.find(r=>r.type==='link-connection'));
 assert.equal(linked.executableRef,'opaque-executable');assert.equal(linked.homeRef,'opaque-home');assert.equal(linked.allowMetadataRead,true);
 assert.equal(await page.evaluate(()=>window.__qa.calls.some(r=>r.type==='submit-task')),false);
 await page.evaluate(()=>window.__qa.finish());
 await page.getByRole('status').filter({hasText:'Diagnostic state: Partial response'}).waitFor();
 assert.match(await page.locator('.o-resource-facts').innerText(),/Models and reasoning[\s\S]*Reported[\s\S]*Quota and usage[\s\S]*Unknown/);
 await page.getByText('Models and reported reasoning levels (1)',{exact:true}).click();
 await page.getByText('Readable provider model',{exact:true}).waitFor();
 assert.match(await page.locator('.o-connection-models').innerText(),/Orchestrix reasoning choices: low · maximum[\s\S]*Native provider levels: low · xhigh/);
 const detail=page.locator('.o-connection-detail');
 await detail.getByLabel('Allow a bounded metadata read for this connection, without a model turn.',{exact:true}).check();
 await detail.getByRole('button',{name:'Recheck connection',exact:true}).click();
 await detail.getByRole('button',{name:'Cancel diagnostic',exact:true}).click();
 assert.equal(await page.evaluate(()=>window.__qa.calls.find(r=>r.type==='cancel-connection-diagnostic').diagnosticId),'diagnostic-two');
 await page.keyboard.press('Escape');
 await page.locator('.o-rail').getByRole('button',{name:'Chat',exact:true}).click();
 await page.locator('.o-session-row').filter({hasText:'Prepare arithmetic'}).click();
 await page.getByRole('button',{name:'Prepare task',exact:true}).click();
 await page.getByRole('textbox',{name:'Acceptance criteria',exact:true}).fill('Arithmetic passes');
 await page.getByRole('button',{name:'Repository',exact:true}).click();await page.getByRole('button',{name:'Worktree folder',exact:true}).click();
 await page.getByLabel('Allow a bounded runtime metadata read in this project before preparing, without a model turn.',{exact:true}).check();
 await page.getByRole('button',{name:'Prepare task',exact:true}).click();
 await page.getByRole('heading',{name:'Review prepared request',exact:true}).waitFor();
 assert.equal(await page.evaluate(()=>window.__qa.calls.find(r=>r.type==='prepare-task').draft.connectionRevision),1);
 await page.evaluate(()=>window.__qa.rename());
 await page.getByRole('combobox',{name:'Connection',exact:true}).getByRole('option',{name:'Renamed',exact:true}).waitFor({state:'attached'});
 assert.equal(await page.locator('.o-task-authorization').count(),1,'rename does not change the binding pin');
 await page.evaluate(()=>window.__qa.rebind());
 await page.waitForFunction(()=>!document.querySelector('.o-task-authorization'));
 assert.equal(await page.getByRole('combobox',{name:'Connection',exact:true}).inputValue(),'');
 assert.equal(await page.getByLabel('Allow a bounded runtime metadata read in this project before preparing, without a model turn.',{exact:true}).isChecked(),false);
 await page.keyboard.press('Control+,');
 for(const language of ['en','pt-BR','es'])for(const scale of [100,200]){
  await page.setViewportSize({width:900,height:620});await page.evaluate(({language,scale})=>window.__qa.language(language,scale),{language,scale});
  await page.waitForFunction(({language,scale})=>document.documentElement.lang===language&&getComputedStyle(document.documentElement).fontSize===`${14*scale/100}px`,{language,scale});
  const translate=key=>language==='en'?key:connectionTranslations[key]?.[language==='pt-BR'?0:1]??key;
  await page.getByRole('region',{name:translate('Connection details'),exact:true}).waitFor();
  const consentGeometry=await assertConsentAdjacency({language,scale,width:900,height:620});
  assert.equal(consentGeometry.labelFont,14*scale/100,'consent text must preserve the requested font scale');
  const geometry=await page.locator('.o-settings-content').evaluate(element=>({width:element.clientWidth,scrollWidth:element.scrollWidth,font:parseFloat(getComputedStyle(element).fontSize)}));
  assert(geometry.scrollWidth<=geometry.width+1,`connections content must not clip horizontally: ${JSON.stringify({language,scale,geometry})}`);
  await page.getByText(translate('Authentication ownership'),{exact:true}).waitFor();
  await page.locator('.o-connection-detail').getByRole('button',{name:translate('Recheck connection'),exact:true}).scrollIntoViewIfNeeded();
 }
 assert.deepEqual(errors,[]);console.log('PASS connection renderer onboarding, metadata states, exact cancellation, readable model/effort and binding-pin regressions');
}finally{await browser.close();}
