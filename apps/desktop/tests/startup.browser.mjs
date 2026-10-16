// Renderer regression only: mocked native bridge, never a native readiness proof.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdir,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {defaultSettings} from '../src/protocol.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const browser=await chromium.launch({channel:'chrome',headless:true});
try {
 for(const mode of ['delayed','blocked','failed','owner-gone']) {
  const page=await browser.newPage({viewport:{width:900,height:620}});
  console.log(`Startup renderer mode: ${mode}`);
  try {
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(({mode,settings})=>{
   const calls=[];let observations=0,ready=false;
   const daemon=()=>({state:{suspended:true,generation:1,cursor:{database:'startup-renderer',sequence:0}},workers:[],activeWorkerId:null});
   const pending=()=>({startup:{state:mode==='delayed'?'recovering':mode==='owner-gone'?'blocked':mode,phase:mode==='delayed'?'worker-recovery':null,phaseSource:mode==='delayed'?'own-startup-report':'unknown',hostElapsedMs:['blocked','owner-gone'].includes(mode)?15000:1000,daemonElapsedMs:null,automaticFollowUp:mode==='delayed',followUpBudgetMs:15000,explicitConnectAvailable:mode==='owner-gone'}});
   window.__startupQa={calls,release(){ready=true;}};
   window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
   window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
    calls.push(command==='desktop_request'?args.request.type:command);
    if(command.startsWith('plugin:event|'))return 1;
    if(command==='connect_daemon')return ready?daemon():pending();
    if(command==='observe_daemon_startup'){observations++;return ready?daemon():pending();}
    if(command==='desktop_request'&&args.request.type==='bootstrap'){if(!ready)throw Error('bootstrap-before-readiness');return {revision:0,projects:[],conversations:[],settings,connections:[],eventCursor:{database:'startup-renderer',sequence:0},admissionsSuspended:true,daemon:daemon()};}
    if(command==='desktop_poll')return {daemon:daemon(),cursor:{database:'startup-renderer',sequence:0},events:[],resyncRequired:false};
    throw Error(`Unexpected startup fixture command ${command}`);
   }};
  },{mode,settings:defaultSettings});
  await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1420/');
  if(mode==='delayed') {
   await page.getByRole('status').getByText('Recovering saved work…',{exact:true}).waitFor();
   assert.equal(await page.evaluate(()=>window.__startupQa.calls.includes('bootstrap')),false);
   await page.evaluate(()=>window.__startupQa.release());
   await page.getByText('Core API v1',{exact:true}).waitFor();
   assert.equal(await page.evaluate(()=>window.__startupQa.calls.filter(c=>c==='connect_daemon').length),1);
   assert.ok(await page.evaluate(()=>window.__startupQa.calls.filter(c=>c==='observe_daemon_startup').length)>=1);
  } else {
   await page.getByRole('button',{name:'Check again',exact:true}).waitFor();
   // Wait across multiple follow-up intervals: failed/blocked must stay bounded.
   await new Promise(r=>setTimeout(r,1200));
   assert.equal(await page.evaluate(()=>window.__startupQa.calls.filter(c=>c==='observe_daemon_startup').length),0);
   assert.equal(await page.evaluate(()=>window.__startupQa.calls.includes('bootstrap')),false);
   await page.evaluate(()=>window.__startupQa.release());
   await page.getByRole('button',{name:mode==='owner-gone'?'Connect to daemon':'Check again',exact:true}).click();
   await page.getByText('Core API v1',{exact:true}).waitFor();
   assert.equal(await page.evaluate(()=>window.__startupQa.calls.filter(c=>c==='connect_daemon').length),['failed','owner-gone'].includes(mode)?2:1);
  }
  assert.deepEqual(errors,[]);
  } catch(error) {
   console.error(`Startup renderer failed in ${mode}:`,error);
   const dir=new URL('../../../artifacts/diagnostics/',import.meta.url);await mkdir(dir,{recursive:true});
   const base=new URL(`startup-renderer-${mode}-${Date.now()}`,dir);
   await writeFile(new URL(base.href+'.json'),JSON.stringify({mode,errors,state:await page.evaluate(()=>({calls:window.__startupQa.calls,text:document.body.innerText,buttons:[...document.querySelectorAll('button')].map(b=>({text:b.innerText,disabled:b.disabled,box:b.getBoundingClientRect().toJSON()}))}))},null,2));
   await page.screenshot({path:fileURLToPath(new URL(base.href+'.png')),timeout:5000}).catch(e=>console.error('Failure capture unavailable:',e.message));
   throw error;
  } finally {await page.close();}
 }
 console.log('Startup renderer passed: no bootstrap before handshake, finite follow-up, blocked observation-only check, explicit failed retry.');
} finally { await browser.close(); }
