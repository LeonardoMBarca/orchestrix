// Actual renderer/CSS with a read-only daemon mock. No native/runtime proof.
import {createRequire} from 'node:module';
import {mkdirSync,writeFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import assert from 'node:assert/strict';
import {defaultSettings} from '../src/protocol.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const before=process.argv.includes('--record-before');
const browser=await chromium.launch({channel:'chrome',headless:true});
const out=new URL(`../../../artifacts/ox016-refresh-portability/home/run-${Date.now()}/`,import.meta.url);mkdirSync(out,{recursive:true});
const rows=[];
try{
 for(const width of before?[900]:[900,1440])for(const scale of before?[200]:[100,200])for(const language of before?['en']:['en','pt-BR','es']){
  const page=await browser.newPage({viewport:{width,height:794}}),errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(settings=>{
   const state={revision:0,settings,projects:[],conversations:[],connections:[],eventCursor:{database:'home-test',sequence:0},admissionsSuspended:true};
   const daemon={state:{generation:1,suspended:true,cursor:state.eventCursor},workers:[],activeWorkerId:null,features:['desktop-v1','connections-v1']};
   window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
   window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
    if(command.startsWith('plugin:event|'))return 1;
    if(command==='connect_daemon')return daemon;
    if(command==='desktop_poll')return {daemon,cursor:state.eventCursor,events:[],resyncRequired:false};
    if(command==='desktop_request'&&args.request.type==='bootstrap')return {...state,daemon};
    throw Error('Unexpected mock call '+command+' '+args.request?.type);
   }};
  },{...defaultSettings,language,textScale:scale});
  await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1421/');
  await page.locator('.o-home-heading').waitFor();
  await page.getByRole('navigation').first().getByRole('button').nth(3).click();
  await page.locator('.o-dock.left').waitFor();
  const saved=await page.evaluate(()=>localStorage.getItem('orchestrix-desktop-layout'));
  await page.locator('.o-composer textarea').fill('Draft survives responsive reflow');
  await page.locator('.o-content').evaluate(element=>element.scrollTo(0,0));
  const geometry=await page.locator('.o-home').evaluate(home=>{
   const box=e=>{const r=e.getBoundingClientRect();return {top:r.top,bottom:r.bottom,left:r.left,right:r.right,width:r.width,height:r.height};};
   return {home:box(home),heading:box(home.querySelector('.o-home-heading')),title:box(home.querySelector('h1')),cards:box(home.querySelector('.o-start-cards')),suggestions:box(home.querySelector('.o-suggestions')),composer:box(home.querySelector('.o-composer')),scrollWidth:document.documentElement.scrollWidth,viewport:innerWidth};
  });
  const label=`${before?'before':'after'}-${language}-${scale}-${width}`;
  await page.screenshot({path:fileURLToPath(new URL(label+'.png',out))});
  rows.push({label,geometry});
  if(!before){
   assert(geometry.title.bottom<=geometry.cards.top,JSON.stringify({label,geometry}));
   assert(geometry.cards.bottom<=geometry.suggestions.top+1);
   assert(geometry.suggestions.bottom<=geometry.composer.top+1);
   assert(geometry.scrollWidth<=geometry.viewport,`horizontal overflow ${label}`);
   await page.locator('.o-composer textarea').scrollIntoViewIfNeeded();
   await page.locator('.o-composer textarea').focus();
   assert.equal(await page.locator('.o-composer textarea').inputValue(),'Draft survives responsive reflow');
   await page.setViewportSize({width:width===900?1440:900,height:794});
   assert.equal(await page.evaluate(()=>localStorage.getItem('orchestrix-desktop-layout')),saved,'reflow must not write another layout preference');
   assert.equal(await page.locator('.o-composer textarea').inputValue(),'Draft survives responsive reflow');
   assert.deepEqual(errors,[]);
  }
  await page.close();
 }
 writeFileSync(new URL(before?'before.json':'after.json',out),JSON.stringify(rows,null,2));
 console.log(JSON.stringify({before,cases:rows.length,rows}));
}finally{await browser.close();}
