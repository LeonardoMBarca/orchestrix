// Renderer regression only: real DTO shapes, explicitly mocked native boundary.
// This does not establish Tauri, provider or execution acceptance.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {defaultSettings} from '../src/protocol.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const browser=await chromium.launch({channel:'chrome',headless:true});
const page=await browser.newPage({viewport:{width:1440,height:900}});
const errors=[];page.on('pageerror',error=>errors.push(error.message));
try {
 await page.addInitScript(settings=>{
  const workerId='worker-real-shape',revision='exact-artifact-hash';let sequence=0;const events=[];
  const detail={worker:{intent:{id:workerId,policy:{}},status:'verified'},summary:{workerId,status:'verified',details:{artifactRevision:revision,checks:[{id:'fixture-check',result:'passed',exit_code:0}]}},artifact:{id:'artifact-id',revision,immutable:true,files:[{path:'fixture.txt'}]},reviews:[],historical:true};
  const calls=[];let active=null,suspended=false,visibleWorkers=true;const daemon=()=>({state:{suspended,generation:1,cursor:{database:'worker-shape',sequence}},workers:visibleWorkers?[{workerId,status:'verified'}]:[],workerCount:visibleWorkers?1:0,workerFactsHistorical:true,activeWorkerId:active,features:['desktop-v1','review-v1']});
  window.__qa={calls,setActivity(id,paused,showWorkers=true){active=id;suspended=paused;visibleWorkers=showWorkers;},finishReview(summary='Rejected exact artifact',emit=true){detail.reviews=[{id:'review-distinct-identity',worker_id:workerId,artifact_revision:revision,status:'rejected',result:{summary,findings:[{id:'finding-one',path:'fixture.txt',line_start:1,line_end:1,severity:'warning',explanation:'finding-one requires correction',evidence:'exact artifact evidence'}]}}];if(emit){sequence++;events.push({sequence,kind:'command-finished',body:{commandId:'review-command',workerId:'review-distinct-identity',state:'finished'}});}}};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
  window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
   if(command==='connect_daemon')return daemon();
   if(command==='desktop_poll')return {daemon:daemon(),cursor:{database:'worker-shape',sequence},events:events.filter(event=>event.sequence>args.cursor.sequence),resyncRequired:false};
   if(command.startsWith('plugin:event|'))return 1;
   if(command==='desktop_request'){
    const r=args.request;calls.push(structuredClone(r));
    if(r.type==='bootstrap')return {revision:0,projects:[],conversations:[],settings,connections:[],eventCursor:{database:'worker-shape',sequence},admissionsSuspended:false,daemon:daemon()};
    if(r.type==='inspect-worker')return structuredClone(detail);
    if(r.type==='recovery')return {has_blocking_cases:true,ready_for_admission:false,cases:[{worker_id:'uncertain-work',status:'uncertain',remote_outcome:'completed',local_tree_stopped:true,streams_closed:true,inspection_required:true,set_aside_eligible:true},{worker_id:'unknown-work',status:'uncertain',set_aside_eligible:false}],reviews:[],integrations:[{id:'integration-uncertain',worker_id:'worker-real-shape',run_id:'run-one',task_id:'task-one',source_artifact_revision:revision,status:'uncertain',input_base:'base-commit',result_revision:null,candidate_revision:null,checks:null,reason:'git-inspection-required',delivered:false,profile:'single-code-task-clean-target-v1',target:{directory:'C:/fixture/target',head:'base-commit'},guardian:{authority:'never-render-ownership-authority'}}]};
    if(r.type==='artifact-text')return {text:'immutable fixture text',nextOffset:22,more:false,immutable:true};
   }
   throw Error('Unexpected mocked native command');
  }};
 },defaultSettings);
 await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1420/');
 await page.getByRole('heading',{name:'What shall we create today?'}).waitFor();
 await page.evaluate(()=>window.__qa.setActivity('admitted-request',true,false));
 await page.waitForFunction(()=>document.querySelector('.o-footer')?.textContent.includes('Work in progress'));
 assert.match(await page.locator('.o-footer').innerText(),/Work in progress.*New work suspended/);
 assert.doesNotMatch(await page.locator('.o-footer').innerText(),/Ready when you are/);
 await page.locator('.o-rail').getByRole('button',{name:'Studio',exact:true}).click();
 await page.locator('.o-dock').getByRole('button',{name:'Work',exact:true}).click();
 await page.getByRole('status').filter({hasText:'Waiting for the first work observation?'}).waitFor();
 assert.equal(await page.getByText('No active execution',{exact:true}).count(),0);
 await page.evaluate(()=>window.__qa.setActivity(null,false));
 await page.waitForFunction(()=>document.querySelector('.o-footer')?.textContent.includes('Ready when you are'));
 await page.locator('.o-page .o-work-card').first().click();
 await page.getByRole('cell',{name:'fixture-check',exact:true}).waitFor();
 await page.getByRole('cell',{name:'passed',exact:true}).waitFor();
 await page.waitForTimeout(2700);
 assert.equal(await page.getByRole('cell',{name:'passed',exact:true}).count(),1,'compact snapshot must retain detailed check rows');
 const inspections=()=>page.evaluate(()=>window.__qa.calls.filter(r=>r.type==='inspect-worker').length);
 const before=await inspections();await page.waitForTimeout(1400);assert.equal(await inspections(),before,'ordinary polls must not restore full worker details');
 await page.evaluate(()=>window.__qa.finishReview());
 await page.getByText('Rejected exact artifact',{exact:true}).waitFor();
 await page.getByText('finding-one requires correction',{exact:true}).waitFor();
 await page.getByRole('heading',{name:'Independent review',exact:true}).waitFor();
 const correction=await page.getByRole('button',{name:'Authorize correction',exact:true}).boundingBox(),reviewCard=await page.locator('.o-review-card').boundingBox();
 assert(correction.width<reviewCard.width*.7,'correction action must not stretch across the review card');
 assert.equal(await page.getByText('Changes requested',{exact:true}).count(),1);
 assert.equal(await inspections(),before+1,'review-ID completion must refresh the selected source worker exactly once');
 assert.equal(await page.getByRole('cell',{name:'0',exact:true}).count(),1);
 await page.locator('.o-rail').getByRole('button',{name:'Files',exact:true}).click();
 await page.evaluate(()=>window.__qa.finishReview('Updated while Files was active',false));
 await page.waitForTimeout(1500);
 const hidden=await inspections();
 await page.locator('.o-rail').getByRole('button',{name:'Studio',exact:true}).click();
 await page.getByText('Updated while Files was active',{exact:true}).waitFor();
 assert.equal(await inspections(),hidden+1,'returning to Studio refreshes details without a per-poll restore');
 assert.equal(await page.getByRole('cell',{name:'passed',exact:true}).count(),1);
 await page.locator('.o-dock').getByRole('button',{name:'Recovery',exact:true}).click();
 await page.getByRole('heading',{name:'Recovery',exact:true}).waitFor();
 const cases=page.locator('.o-review-card').filter({has:page.getByRole('heading',{name:'Work recovery',exact:true})});
 assert.equal(await cases.count(),2);
 assert.match(await cases.first().innerText(),/completed[\s\S]*Stopped[\s\S]*Closed[\s\S]*Required/);
 assert.match(await cases.nth(1).innerText(),/Unknown/,'missing recovery observations must remain unknown');
 assert.equal(await page.locator('.o-json:visible').count(),0,'recovery evidence must start collapsed without hiding its summary or actions');
 assert.equal(await page.getByRole('button',{name:'Set aside',exact:true}).count(),1,'only Core-eligible cases offer set aside');
 await page.getByText('Set aside releases the local admission barrier and preserves this uncertain result. It does not retry or discard the work.',{exact:true}).waitFor();
 const integration=page.locator('.o-integration-recovery');
 assert.equal(await integration.count(),1,'blocking integration must not disappear behind completed worker recovery cards');
 assert.match(await integration.innerText(),/Integration recovery[\s\S]*Outcome uncertain[\s\S]*git-inspection-required[\s\S]*worker-real-shape[\s\S]*exact-artifact-hash[\s\S]*C:\/fixture\/target/);
 assert.equal(await integration.getByRole('button').count(),0,'integration recovery cannot invent SetAside, retry or apply actions');
 await page.getByText('New work is blocked until the recovery cases are resolved.',{exact:true}).waitFor();
 await integration.getByText('Preserve the integration worktree and its evidence. This view does not retry, apply or discard the result.',{exact:true}).waitFor();
 await integration.getByText('Show evidence',{exact:true}).click();
 assert.match(await integration.locator('.o-json:visible').innerText(),/"status": "uncertain"/);
 assert.doesNotMatch(await integration.textContent(),/guardian|authority|never-render/,'ownership bindings must not enter visible or collapsed evidence');
 assert.deepEqual(errors,[]);
 process.stdout.write('Worker evidence renderer regression passed: compact preservation, distinct review identity, bounded refresh, Files return, blocking integration recovery.\n');
} finally {await browser.close();}
