// Renderer-only regression using the real additive API shapes. No inference,
// filesystem permission, Git or native-application acceptance is claimed here.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdirSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {defaultSettings} from '../src/protocol.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const browser=await chromium.launch({channel:'chrome',headless:true});
const page=await browser.newPage({viewport:{width:1442,height:794}}),errors=[];
page.on('pageerror',error=>errors.push(error.message));
try{
 await page.addInitScript(settings=>{
  let sequence=0,grant=0,lose=true,suspended=false;const calls=[],events=[],receipts=new Map(),workerId='worker-one',revision='artifact-one';
  const summary={id:'integration-one',worker_id:workerId,source_artifact_revision:revision,status:'prepared',input_base:'input-commit',result_revision:'result-commit',candidate_revision:'candidate-one',target:{directory:'C:\\fixture\\repository',head:'input-commit'},checks:[{id:'check-one',result:'passed',exit_code:0,cwd:'C:\\fixture\\integration\\worktree'}],delivered:false};
  const worker={worker:{intent:{authorized_checks:[{id:'check-one',executable:'C:\\fixture\\check.exe',arguments:['--verify'],executable_sha256:'a'.repeat(64),timeout_ms:30000,authorization_ref:'original-consent',authorized:true}]}},summary:{workerId,status:'verified',details:{checks:[{id:'check-one',result:'passed',exit_code:0}]}},artifact:{id:'artifact-one',revision,immutable:true,files:[]},reviews:[{id:'review-one',worker_id:workerId,artifact_revision:revision,status:'accepted',result:{summary:'Exact artifact accepted',findings:[]}}],integrations:[],historical:true};
  const integration={historical:true,currentVerification:'not-performed',summary,intent:{},phases:[]};
  const daemon=()=>({state:{suspended,generation:1,cursor:{database:'integration-qa',sequence}},workers:[{workerId,status:worker.summary.status}],workerCount:1,workerFactsHistorical:true,activeWorkerId:null,features:['desktop-v1','review-v1','integration-v1']});
  const complete=commandId=>{sequence++;events.push({sequence,kind:'command-finished',body:{commandId,workerId:summary.id,state:'finished'}});};
  window.__qa={calls,worker,integration,suspend(){suspended=true;}};window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
  window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
   if(command==='connect_daemon')return daemon();
   if(command==='desktop_poll')return {daemon:daemon(),cursor:{database:'integration-qa',sequence},events:events.filter(event=>event.sequence>args.cursor.sequence),resyncRequired:false};
   if(command.startsWith('plugin:event|'))return 1;
   if(command==='choose_document')return {id:`grant-${++grant}`,name:`Native folder ${grant}`,directory:true};
   if(command==='desktop_request'){
    const r=args.request;calls.push(structuredClone(r));
    if(r.type==='bootstrap')return {revision:sequence+1,projects:[],conversations:[],settings,connections:[],eventCursor:{database:'integration-qa',sequence},admissionsSuspended:suspended,daemon:daemon()};
    if(r.type==='control'&&r.action==='resume'){suspended=false;return {state:'finished'};}
    if(r.type==='inspect-worker')return structuredClone(worker);
    if(r.type==='artifact-text')return {text:'',more:false,nextOffset:0,immutable:true};
    if(r.type==='inspect-integration')return structuredClone(integration);
    if(['prepare-integration','approve-integration','apply-integration'].includes(r.type)){
     if(receipts.has(r.commandId))return structuredClone(receipts.get(r.commandId));
     if(r.type==='prepare-integration')worker.integrations=[summary];
     if(r.type==='approve-integration'){summary.status='approved';summary.approval={expiresMs:Date.now()+900000};}
     if(r.type==='apply-integration'){summary.status='delivered';summary.delivered=true;worker.summary.status='invalidated';}
     const result={integrationId:summary.id,receipt:{state:'accepted'}};receipts.set(r.commandId,result);complete(r.commandId);
     if(r.type==='prepare-integration'&&lose){lose=false;throw Error('daemon-command-not-confirmed');}
     return structuredClone(result);
    }
   }
   throw Error('Unexpected mocked native command');
  }};
 },defaultSettings);
 await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1420/');
 await page.getByRole('heading',{name:'What shall we create today?'}).waitFor();
 await page.locator('.o-rail').getByRole('button',{name:'Studio',exact:true}).click();await page.locator('.o-dock').getByRole('button',{name:'Work',exact:true}).click();await page.locator('.o-page>.o-work-card').first().click();
 await page.getByRole('heading',{name:'Prepare integration candidate',exact:true}).waitFor();
 const target=page.getByRole('button',{name:'Target checkout',exact:true}),parent=page.getByRole('button',{name:'Integration worktree parent',exact:true});
 assert.equal((await target.innerText()).trim(),'Select original checkout');assert.equal((await parent.innerText()).trim(),'Select integration folder');
 await target.click();await parent.click();
 await page.getByLabel('I authorize these exact checks in the isolated integration worktree.',{exact:true}).check();
 await page.getByRole('button',{name:'Prepare integration candidate',exact:true}).click();
 await page.getByText('A lost reply remains pending with the original command identity.',{exact:true}).waitFor();
 await page.getByRole('button',{name:'Recheck',exact:true}).click();
 await page.getByRole('heading',{name:'Integration candidate',exact:true}).waitFor();
 const preparations=await page.evaluate(()=>window.__qa.calls.filter(r=>r.type==='prepare-integration'));assert.equal(preparations.length,2);assert.deepEqual(preparations[0],preparations[1]);
 assert.equal(await page.getByRole('button',{name:'Apply approved candidate',exact:true}).count(),0);
 const approvalConsent=page.getByLabel('I approve this exact candidate for 15 minutes. This does not apply it.',{exact:true});
 await approvalConsent.check();await page.evaluate(()=>window.__qa.suspend());
 await page.getByRole('button',{name:'Resume admissions',exact:true}).waitFor();
 assert.equal(await page.getByRole('button',{name:'Approve exact candidate',exact:true}).isDisabled(),true);
 await page.getByRole('button',{name:'Resume admissions',exact:true}).click();
 await page.waitForFunction(()=>[...document.querySelectorAll('button')].some(button=>button.textContent==='Approve exact candidate'&&!button.disabled));
 assert.equal(await approvalConsent.isChecked(),true,'resuming admissions preserves the exact-candidate consent');
 await page.getByRole('button',{name:'Approve exact candidate',exact:true}).click();
 await page.getByRole('button',{name:'Apply approved candidate',exact:true}).waitFor();assert.equal(await page.getByRole('button',{name:'Apply approved candidate',exact:true}).isDisabled(),true);
 assert.equal(await page.evaluate(()=>window.__qa.calls.filter(r=>r.type==='apply-integration').length),0);
 await page.getByLabel('I authorize applying this exact candidate to the selected checkout after Core revalidation.',{exact:true}).check();await page.getByRole('button',{name:'Apply approved candidate',exact:true}).click();
 await page.getByRole('heading',{name:'Delivered to checkout',exact:true}).waitFor();
 assert.equal(await page.locator('.o-worker-delivery').innerText(),'Historical delivery recorded for this exact artifact.');
 assert.equal(await page.getByRole('button',{name:'Apply approved candidate',exact:true}).count(),0);
 assert.deepEqual(errors,[]);console.log('Integration renderer regression passed: exact reviewed source, native folder grants, original lost-reply identity, separate candidate approval/application and durable delivery.');
}catch(error){
 const output=fileURLToPath(new URL('../../../artifacts/desktop-renderer-qa/integration-flow-failure/',import.meta.url));mkdirSync(output,{recursive:true});
 await page.screenshot({path:output+'screen.png',fullPage:true});
 console.error(await page.locator('.o-integration').ariaSnapshot().catch(()=>page.locator('body').innerText()));
 throw error;
}finally{await browser.close();}
