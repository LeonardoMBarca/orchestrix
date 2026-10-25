// Focused renderer check, with a mocked native boundary and headless Chrome.
// It never opens the native Desktop, help browser, daemon or provider account.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdir,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createServer} from 'vite';
import {defaultSettings} from '../src/protocol.ts';

const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const output=fileURLToPath(new URL(`../../../artifacts/activity-icons-renderer/${Date.now()}/`,import.meta.url));
await mkdir(output,{recursive:true});
const server=await createServer({root:fileURLToPath(new URL('../',import.meta.url)),logLevel:'error',server:{host:'127.0.0.1',port:0}});
await server.listen();
const url=`http://127.0.0.1:${server.httpServer.address().port}`;
let browser;
const cases=[],started=performance.now();
try{
 browser=await chromium.launch({channel:'chrome',headless:true});
 for(const viewport of [{width:1440,height:794},{width:900,height:620}])for(const language of ['en','pt-BR','es'])for(const scale of [100,200]){
  const page=await browser.newPage({viewport}),errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(settings=>{
   const daemon={state:{suspended:true,generation:1,cursor:{database:'activity-icons-fixture',sequence:0}},workers:[],workerCount:0,workerFactsHistorical:true,activeWorkerId:null,features:['desktop-v1','review-v1']};
   window.__iconQa={calls:[]};
   window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
   window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
    window.__iconQa.calls.push({command,request:args.request});
    if(command==='connect_daemon')return daemon;
    if(command==='desktop_poll')return {daemon,events:[],cursor:daemon.state.cursor,resyncRequired:false};
    if(command.startsWith('plugin:event|')||command==='open_help')return 1;
    if(command==='desktop_request'&&args.request.type==='bootstrap')return {revision:0,projects:[],conversations:[],settings,connections:[],eventCursor:daemon.state.cursor,admissionsSuspended:true,daemon};
    if(command==='desktop_request'&&args.request.type==='configurations'&&args.request.request.type==='catalog')return {schemaVersion:1,revision:0,modes:[],profiles:[],scopes:[]};
    throw Error('Unexpected icon fixture operation');
   }};
  },{...defaultSettings,language,textScale:scale});
  await page.goto(url);
  await page.waitForFunction(({language,scale})=>document.documentElement.lang===language&&getComputedStyle(document.documentElement).fontSize===`${14*scale/100}px`,{language,scale});
  const controls=page.locator('.o-rail-bottom>button'),help=controls.nth(0),settings=controls.nth(1);
  await controls.first().waitFor();
  const name=`${viewport.width}-${language}-${scale}`;
  await page.locator('.o-rail-bottom').screenshot({path:output+name+'.png'});
  for(const button of [help,settings]){
   assert(await button.getAttribute('aria-label'),'icon buttons retain accessible names');
   const bounds=await button.evaluate(element=>{
    const rect=element.getBoundingClientRect(),svg=element.querySelector('svg'),icon=svg.getBoundingClientRect(),style=getComputedStyle(svg),box=svg.getBBox();
    return {left:rect.left,right:rect.right,top:rect.top,bottom:rect.bottom,width:rect.width,height:rect.height,iconWidth:icon.width,iconHeight:icon.height,centerX:(icon.left+icon.right-rect.left-rect.right)/2,centerY:(icon.top+icon.bottom-rect.top-rect.bottom)/2,fill:style.fill,stroke:style.stroke,lineWidth:parseFloat(style.strokeWidth),box:{x:box.x,y:box.y,width:box.width,height:box.height}};
   });
   assert.equal(bounds.iconWidth,23);assert.equal(bounds.iconHeight,23);
   assert(Math.abs(bounds.centerX)<.5&&Math.abs(bounds.centerY)<.5,'glyph stays centered in its hit area');
   assert(bounds.width>=40&&bounds.height>=40,'help and Settings keep their full hit areas');
   assert(bounds.left>=0&&bounds.right<=viewport.width&&bounds.top>=0&&bounds.bottom<=viewport.height,'both buttons remain reachable');
   assert.equal(bounds.fill,'none');assert.notEqual(bounds.stroke,'none');
   assert(bounds.box.x-bounds.lineWidth/2>=0&&bounds.box.y-bounds.lineWidth/2>=0&&bounds.box.x+bounds.box.width+bounds.lineWidth/2<=24&&bounds.box.y+bounds.box.height+bounds.lineWidth/2<=24,'paint stays within the viewBox');
  }
  // Raster symmetry detects an accidentally malformed gear without pinning a
  // particular SVG path string. The inner ring must also remain unobstructed.
  const gear=await settings.locator('svg').evaluate(async svg=>{
   const clone=svg.cloneNode(true);clone.setAttribute('width','240');clone.setAttribute('height','240');clone.setAttribute('style','width:240px;height:240px;fill:none;stroke:white;stroke-width:1.6;stroke-linecap:round;stroke-linejoin:round');
   const img=new Image();img.src='data:image/svg+xml,'+encodeURIComponent(new XMLSerializer().serializeToString(clone));await img.decode();
   const canvas=document.createElement('canvas');canvas.width=canvas.height=240;
   const context=canvas.getContext('2d');context.drawImage(img,0,0);const pixels=context.getImageData(0,0,240,240).data;
   let horizontal=0,vertical=0,total=0;
   for(let y=0;y<240;y++)for(let x=0;x<240;x++){
    const alpha=pixels[(y*240+x)*4+3];total+=alpha;
    horizontal+=Math.abs(alpha-pixels[(y*240+239-x)*4+3]);vertical+=Math.abs(alpha-pixels[((239-y)*240+x)*4+3]);
   }
   const ringAlpha=[[85,120],[155,120],[120,85],[120,155]].map(([x,y])=>pixels[(y*240+x)*4+3]);
   return {horizontal:horizontal/total,vertical:vertical/total,centerAlpha:pixels[(120*240+120)*4+3],ringAlpha};
  });
  assert(gear.horizontal<.02&&gear.vertical<.02,`gear must be centered and symmetric: ${JSON.stringify(gear)}`);
  assert.equal(gear.centerAlpha,0,'the gear opening remains clear');
  assert(gear.ringAlpha.every(alpha=>alpha>200),'a visible central ring surrounds the opening');
  await settings.focus();await page.keyboard.press('Enter');await page.locator('.o-settings').waitFor();
  await page.keyboard.press('Escape');await page.locator('.o-settings').waitFor({state:'hidden'});
  assert(await settings.evaluate(element=>element===document.activeElement),'closing Settings restores keyboard focus');
  await help.focus();await page.keyboard.press('Enter');
  const calls=await page.evaluate(()=>window.__iconQa.calls);
  assert.equal(calls.filter(call=>call.command==='open_help').length,1,'Help uses only its dedicated native command');
  assert(calls.every(call=>call.command!=='desktop_request'||['bootstrap','configurations'].includes(call.request.type)),'icon actions do not prepare tasks or query metadata');
  assert.deepEqual(errors,[]);
  cases.push({viewport,language,scale,gear,screenshot:name+'.png'});await page.close();
 }
 await writeFile(output+'result.json',JSON.stringify({status:'passed',kind:'headless-renderer-mocked-native',native:false,providerReads:0,cases,elapsedMs:performance.now()-started},null,2)+'\n');
 console.log(JSON.stringify({passed:cases.length,output,elapsedMs:performance.now()-started}));
}catch(error){
 await writeFile(output+'result.json',JSON.stringify({status:'failed',kind:'headless-renderer-mocked-native',native:false,cases,error:String(error),elapsedMs:performance.now()-started},null,2)+'\n');
 console.error('Icon renderer evidence: '+output);throw error;
}finally{await browser?.close();await server.close();}
