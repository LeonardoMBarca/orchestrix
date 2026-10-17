// Explicit renderer fixture: real DTO shapes, no actual provider or disk effects.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdirSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {defaultSettings} from '../src/protocol.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright'),browser=await chromium.launch({channel:'chrome',headless:true});
const page=await browser.newPage({viewport:{width:1442,height:794}}),errors=[];page.on('pageerror',error=>errors.push(error.message));
try{
 await page.addInitScript(settings=>{
  let sequence=0,suspended=false;const events=[],calls=[];
  const summary={id:'candidate-one',worker_id:'worker-one',source_artifact_revision:'artifact-one',status:'approved',input_base:'base',result_revision:'result',candidate_revision:'candidate-revision',target:{directory:'C:\\fixture\\target'},approval:{expiresMs:Date.now()+900000},checks:[],delivered:false};
  const integration={historical:true,currentVerification:'not-performed',summary,intent:{},phases:[]};
  const worker={worker:{intent:{}},summary:{workerId:'worker-one',status:'verified',details:{checks:[]}},artifact:{id:'artifact-one',revision:'artifact-one',immutable:true,files:[]},reviews:[{id:'review-one',worker_id:'worker-one',artifact_revision:'artifact-one',status:'accepted',result:{summary:'Exact reviewed evidence retained',findings:[]}}],integrations:[summary],historical:true};
  const daemon=()=>({state:{suspended,generation:1,cursor:{database:'edit-candidate',sequence}},workers:[{workerId:'worker-one',status:'verified'}],workerCount:1,workerFactsHistorical:true,activeWorkerId:null,features:['desktop-v1','review-v1','integration-v1']});
  window.__qa={calls,external:'external bytes'};window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
  window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
   if(command==='connect_daemon')return daemon();if(command.startsWith('plugin:event|'))return 1;
   if(command==='desktop_poll')return {daemon:daemon(),cursor:{database:'edit-candidate',sequence},events:events.filter(event=>event.sequence>args.cursor.sequence),resyncRequired:false};
   if(command==='choose_document')return {id:'file-grant',name:'candidate.txt',directory:false};
   if(command==='editor_request'){
    const r=args.request;calls.push(structuredClone(r));
    if(r.type==='open')return {id:'document-one',name:'candidate.txt',text:'original buffer',version:'original-base',origin:'authorized-file'};
    if(r.type==='save'){suspended=true;return {saveError:'save-conflict',affectedWorkers:[],affectedIntegrations:['candidate-one'],integrationRevalidation:[{integrationId:'candidate-one',commandId:'edit-revalidation-one',accepted:false,reason:'daemon-busy'}]};}
   }
   if(command==='desktop_request'){
    const r=args.request;calls.push(structuredClone(r));
    if(r.type==='bootstrap')return {revision:sequence+1,projects:[],conversations:[],settings,connections:[],eventCursor:{database:'edit-candidate',sequence},admissionsSuspended:suspended,daemon:daemon()};
    if(r.type==='inspect-worker')return structuredClone(worker);
    if(r.type==='inspect-integration'){
     const historical=structuredClone(integration);
     if(window.__qa.deferInspect){window.__qa.deferInspect=false;window.__qa.inspectHeld=true;return new Promise(resolve=>{window.__qa.releaseInspect=()=>{window.__qa.inspectHeld=false;resolve(historical);};});}
     return historical;
    }
    if(r.type==='artifact-text')return {text:'',nextOffset:0,more:false,immutable:true};
    if(r.type==='revalidate-integration'){summary.status='invalidated';summary.reason='target-changed';sequence++;events.push({sequence,kind:'command-finished',body:{commandId:r.commandId,workerId:summary.id,state:'finished'}});return {integrationId:summary.id,receipt:{outcome:{state:'finished',status:'invalidated'}}};}
   }
   throw Error(`Unexpected fixture operation ${command}`);
  }};
 },defaultSettings);
 await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1420/');
 await page.getByRole('heading',{name:'What shall we create today?'}).waitFor();
 await page.locator('.o-rail').getByRole('button',{name:'Studio',exact:true}).click();await page.locator('.o-dock').getByRole('button',{name:'Work',exact:true}).click();await page.locator('.o-page>.o-work-card').first().click();
 await page.locator('.o-integration-history button').click();
 await page.getByLabel('I authorize applying this exact candidate to the selected checkout after Core revalidation.',{exact:true}).check();
 await page.evaluate(()=>{window.__qa.deferInspect=true;});
 await page.getByRole('button',{name:'Refresh integration history',exact:true}).click();
 await page.waitForFunction(()=>window.__qa.inspectHeld===true);
 await page.getByRole('button',{name:'File',exact:true}).click();await page.getByRole('menuitem',{name:'Open file',exact:true}).click();
 const buffer=page.getByRole('textbox',{name:'Edit candidate.txt',exact:true});await buffer.fill('my unsaved buffer');await buffer.press('Control+s');
 const conflict=page.locator('.o-editor').getByRole('alert');await conflict.waitFor();
 assert.ok((await conflict.innerText()).startsWith('Save conflict: disk changed. Your buffer is preserved.'));
 assert.equal(await conflict.getByRole('button',{name:'Reload disk version',exact:true}).count(),1);
 assert.equal(await buffer.inputValue(),'my unsaved buffer');
 await page.locator('.o-rail').getByRole('button',{name:'Studio',exact:true}).click();
 await page.getByText('Saved changes require candidate revalidation before approval or application.',{exact:true}).waitFor();
 assert.equal(await page.getByRole('button',{name:'Apply approved candidate',exact:true}).count(),0,'historical refresh cannot restore a stale approval');
 await page.getByText('Exact reviewed evidence retained',{exact:true}).waitFor();
 const revalidate=page.getByRole('button',{name:'Revalidate candidate',exact:true});assert.equal(await revalidate.isEnabled(),true,'deterministic revalidation remains available while admissions are suspended');
 await revalidate.click();await page.getByText('target-changed',{exact:true}).waitFor();
 assert.equal(await page.getByRole('button',{name:'Apply approved candidate',exact:true}).count(),0);
 await page.evaluate(()=>{window.__qa.releaseInspect();});
 await page.evaluate(()=>new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve))));
 assert.equal(await page.getByRole('button',{name:'Apply approved candidate',exact:true}).count(),0,'an Approved reply started before Save and delivered after confirmed invalidation cannot restore authority');
 await page.getByText('target-changed',{exact:true}).waitFor();
 assert.equal(await page.evaluate(()=>window.__qa.calls.find(request=>request.type==='revalidate-integration').commandId),'edit-revalidation-one','retry preserves the original uncertain command identity');
 await page.locator('.o-rail').getByRole('button',{name:'Files',exact:true}).click();assert.equal(await buffer.inputValue(),'my unsaved buffer');assert.equal(await page.evaluate(()=>window.__qa.external),'external bytes');
 assert.deepEqual(errors,[]);console.log('Integration edit regression passed: save conflict preserves buffer, candidate becomes stale, historical refresh cannot approve, original deterministic retry invalidates while suspended.');
}catch(error){
 const output=fileURLToPath(new URL(`../../../artifacts/desktop-renderer-qa/integration-invalidation-failure-${Date.now()}/`,import.meta.url));mkdirSync(output,{recursive:true});
 await page.screenshot({path:output+'screen.png',fullPage:true});console.error(await page.locator('body').ariaSnapshot());throw error;
}finally{await browser.close();}
