// Focused renderer geometry QA with an explicitly mocked native boundary.
// This does not establish native Windows/DPI or daemon acceptance.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdirSync,writeFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {defaultSettings} from '../src/protocol.ts';

const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const output=fileURLToPath(new URL(`../../../artifacts/desktop-renderer-qa/settings-nav-${Date.now()}/`,import.meta.url));mkdirSync(output,{recursive:true});
const browser=await chromium.launch({channel:'chrome',headless:true});
const cases=[],started=performance.now();let current;
try{
 for(const viewport of [{width:1440,height:794},{width:900,height:620}])for(const language of ['en','pt-BR','es'])for(const scale of [100,200]){
  const page=await browser.newPage({viewport,deviceScaleFactor:1}),errors=[];page.on('pageerror',error=>errors.push(error.message));
  current={viewport,language,scale,page};
  await page.addInitScript(settings=>{
   const daemon={state:{suspended:true,generation:1,cursor:{database:'settings-nav',sequence:0}},workers:[],workerCount:0,workerFactsHistorical:true,activeWorkerId:null,features:['desktop-v1','review-v1']};
   window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
   window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
    if(command==='connect_daemon')return daemon;
    if(command==='desktop_poll')return {daemon,events:[],cursor:daemon.state.cursor,resyncRequired:false};
    if(command.startsWith('plugin:event|'))return 1;
    if(command==='desktop_request'&&args.request.type==='bootstrap')return {revision:0,projects:[],conversations:[],settings,connections:[],eventCursor:daemon.state.cursor,admissionsSuspended:true,daemon};
    throw Error('Unexpected Settings geometry mock command');
   }};
  },{...defaultSettings,language,textScale:scale});
  await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1420/');
  await page.waitForFunction(({language,scale})=>document.documentElement.lang===language&&getComputedStyle(document.documentElement).fontSize===`${14*scale/100}px`,{language,scale});
  await page.keyboard.press('Control+,');await page.locator('.o-settings').waitFor();await page.waitForFunction(()=>document.fonts.status==='loaded');
  const nav=page.locator('.o-settings-nav'),buttons=nav.getByRole('button'),settings=page.locator('.o-settings'),content=page.locator('.o-settings-content');
  assert.equal(await buttons.count(),6);
  const geometry=await nav.evaluate(element=>{
   const labels=[...element.querySelectorAll('button')].map(button=>{
    const words=[],walker=document.createTreeWalker(button,NodeFilter.SHOW_TEXT);
    while(walker.nextNode())for(const match of walker.currentNode.textContent.matchAll(/\S+/gu)){
     const range=document.createRange();range.setStart(walker.currentNode,match.index);range.setEnd(walker.currentNode,match.index+match[0].length);
     words.push({word:match[0],lines:new Set([...range.getClientRects()].filter(rect=>rect.width>0&&rect.height>0).map(rect=>Math.round(rect.top))).size});
    }
    return {label:button.textContent.trim(),fontSize:parseFloat(getComputedStyle(button).fontSize),words};
   });
   return {width:element.getBoundingClientRect().width,scrollWidth:element.scrollWidth,clientWidth:element.clientWidth,labels};
  });
  if(scale===100)assert(Math.abs(geometry.width-180)<=1,`ordinary labels retain the 180px navigation: ${JSON.stringify({viewport,language,geometry})}`);
  assert(geometry.scrollWidth<=geometry.clientWidth+1,'navigation labels remain fully readable without horizontal clipping');
  for(const label of geometry.labels){assert.equal(label.fontSize,14*scale/100,'text scale must remain unchanged');assert(label.words.length>0);for(const word of label.words)assert.equal(word.lines,1,`Settings label word must remain whole: ${JSON.stringify({viewport,language,scale,label,word})}`);}
  for(let index=0;index<6;index++){
   const button=buttons.nth(index);await button.focus();await page.keyboard.press('Space');await page.waitForFunction(index=>document.querySelectorAll('.o-settings-nav button')[index]?.classList.contains('active'),index);
   assert.equal(await button.evaluate(element=>{const rect=element.getBoundingClientRect(),point=document.elementFromPoint(rect.left+Math.min(rect.width/2,20),rect.top+rect.height/2);return !!point&&(point===element||element.contains(point));}),true,'every navigation option is reachable by keyboard without clipping');
   const last=content.locator('button,input,textarea,select').last();await last.scrollIntoViewIfNeeded();
   assert.equal(await last.evaluate(element=>{const rect=element.getBoundingClientRect(),pane=element.closest('.o-settings-content').getBoundingClientRect();return rect.width>0&&rect.height>0&&rect.top>=pane.top-1&&rect.bottom<=pane.bottom+1&&rect.left>=pane.left-1&&rect.right<=pane.right+1;}),true,'the final control of each Settings section remains reachable by scrolling');
  }
  assert.equal(await settings.evaluate(element=>{const rect=element.getBoundingClientRect();return rect.left>=0&&rect.top>=0&&rect.right<=innerWidth+1&&rect.bottom<=innerHeight+1;}),true,'Settings remains inside the available viewport');
  assert.deepEqual(errors,[]);
  await buttons.first().click();await content.evaluate(element=>{element.scrollTop=0;});
  const name=`${viewport.width}x${viewport.height}-${language}-${scale}`;await page.screenshot({path:output+name+'.png'});
  cases.push({viewport,language,scale,navigationWidth:geometry.width,labels:geometry.labels.map(label=>({text:label.label,fontSize:label.fontSize})),screenshot:name+'.png'});
  console.log(JSON.stringify({case:cases.length,viewport,language,scale,navigationWidth:geometry.width}));
  await page.close();current=null;
 }
 writeFileSync(output+'result.json',JSON.stringify({status:'passed',elapsedMs:performance.now()-started,cases,validationKind:'renderer-geometry-mocked-native',nativeDpiValidated:false},null,2)+'\n');
 console.log(JSON.stringify({status:'passed',cases:cases.length,elapsedMs:performance.now()-started,output}));
}catch(error){
 if(current)await current.page.screenshot({path:output+'failure.png'});
 writeFileSync(output+'result.json',JSON.stringify({status:'failed',elapsedMs:performance.now()-started,cases,failedCase:current?{viewport:current.viewport,language:current.language,scale:current.scale}:null,error:error.stack,validationKind:'renderer-geometry-mocked-native'},null,2)+'\n');
 console.error('Settings navigation evidence: '+output);throw error;
}finally{await browser.close();}
