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
  const calls=[];const daemon=()=>({state:{suspended:false,generation:1,cursor:{database:'worker-shape',sequence}},workers:[{workerId,status:'verified'}],workerCount:1,workerFactsHistorical:true,activeWorkerId:null,features:['desktop-v1','review-v1']});
  window.__qa={calls,finishReview(summary='Rejected exact artifact',emit=true){detail.reviews=[{id:'review-distinct-identity',worker_id:workerId,artifact_revision:revision,status:'rejected',result:{summary,findings:[{id:'finding-one',path:'fixture.txt',line_start:1,line_end:1,severity:'warning',explanation:'finding-one requires correction',evidence:'exact artifact evidence'}]}}];if(emit){sequence++;events.push({sequence,kind:'command-finished',body:{commandId:'review-command',workerId:'review-distinct-identity',state:'finished'}});}}};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
  window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
   if(command==='connect_daemon')return daemon();
   if(command==='desktop_poll')return {daemon:daemon(),cursor:{database:'worker-shape',sequence},events:events.filter(event=>event.sequence>args.cursor.sequence),resyncRequired:false};
   if(command.startsWith('plugin:event|'))return 1;
   if(command==='desktop_request'){
    const r=args.request;calls.push(structuredClone(r));
    if(r.type==='bootstrap')return {revision:0,projects:[],conversations:[],settings,connections:[],eventCursor:{database:'worker-shape',sequence},admissionsSuspended:false,daemon:daemon()};
    if(r.type==='inspect-worker')return structuredClone(detail);
    if(r.type==='artifact-text')return {text:'immutable fixture text',nextOffset:22,more:false,immutable:true};
   }
   throw Error('Unexpected mocked native command');
  }};
 },defaultSettings);
 await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1420/');
 await page.getByRole('heading',{name:'What shall we create today?'}).waitFor();
 await page.locator('.o-rail').getByRole('button',{name:'Studio',exact:true}).click();
 await page.locator('.o-dock').getByRole('button',{name:'Work',exact:true}).click();
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
 assert.deepEqual(errors,[]);
 process.stdout.write('Worker evidence renderer regression passed: compact preservation, distinct review identity, bounded refresh, Files return.\n');
} finally {await browser.close();}
