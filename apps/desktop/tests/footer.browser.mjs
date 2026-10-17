// Focused renderer geometry regression with an explicitly mocked native boundary.
// This does not establish native Windows/DPI or daemon acceptance.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdirSync,writeFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {defaultSettings} from '../src/protocol.ts';

const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const viewport={width:900,height:620},scale=200;
const labels={
 en:{active:'Work in progress',suspended:'New work suspended',quota:'Quota not reported'},
 'pt-BR':{active:'Trabalho em andamento',suspended:'Novos trabalhos suspensos',quota:'Cota não informada'},
 es:{active:'Trabajo en curso',suspended:'Nuevos trabajos suspendidos',quota:'Cuota no informada'},
};
const output=fileURLToPath(new URL(`../../../artifacts/desktop-renderer-qa/footer-${Date.now()}/`,import.meta.url));
mkdirSync(output,{recursive:true});
const browser=await chromium.launch({channel:'chrome',headless:true});
const cases=[],started=performance.now();let current;
try{
 for(const language of Object.keys(labels))for(const active of [false,true]){
  const page=await browser.newPage({viewport,deviceScaleFactor:1}),errors=[];
  page.on('pageerror',error=>errors.push(error.message));current={page,language,active};
  await page.addInitScript(({settings,active})=>{
   const daemon={state:{suspended:true,generation:1,cursor:{database:'footer-geometry',sequence:0}},workers:[],workerCount:active?1:0,workerFactsHistorical:true,activeWorkerId:active?'footer-active-worker':null,activeCancellable:active,activeOperationKind:active?'worker':null,features:['desktop-v1','review-v1']};
   window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
   window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
    if(command==='connect_daemon')return daemon;
    if(command==='desktop_poll')return {daemon,events:[],cursor:daemon.state.cursor,resyncRequired:false};
    if(command.startsWith('plugin:event|'))return 1;
    if(command==='desktop_request'&&args.request.type==='bootstrap')return {revision:0,projects:[],conversations:[],settings,connections:[],eventCursor:daemon.state.cursor,admissionsSuspended:true,daemon};
    throw Error('Unexpected footer geometry mock command');
   }};
  },{settings:{...defaultSettings,language,textScale:scale},active});
  await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1420/');
  await page.waitForFunction(({language,scale})=>document.documentElement.lang===language&&getComputedStyle(document.documentElement).fontSize===`${14*scale/100}px`,{language,scale});
  const footer=page.locator('.o-footer');
  const expectedStatus=active?`${labels[language].active} · ${labels[language].suspended}`:labels[language].suspended;
  await page.waitForFunction(expected=>document.querySelector('.o-footer>span')?.textContent===expected,expectedStatus);
  await page.waitForFunction(()=>document.fonts.status==='loaded');
  const geometry=await footer.evaluate(element=>{
   const rect=element.getBoundingClientRect(),style=getComputedStyle(element);
   const bounds=rect=>({left:rect.left,top:rect.top,right:rect.right,bottom:rect.bottom,width:rect.width,height:rect.height});
   const spans=[...element.querySelectorAll(':scope>span')].map(span=>{
    const textRects=[],walker=document.createTreeWalker(span,NodeFilter.SHOW_TEXT);
    while(walker.nextNode())if(walker.currentNode.textContent.trim()){
     const range=document.createRange();range.selectNodeContents(walker.currentNode);
     textRects.push(...[...range.getClientRects()].filter(rect=>rect.width>0&&rect.height>0).map(bounds));
    }
    return {text:span.textContent,bounds:bounds(span.getBoundingClientRect()),textRects,fontSize:parseFloat(getComputedStyle(span).fontSize),scrollWidth:span.scrollWidth,clientWidth:span.clientWidth,scrollHeight:span.scrollHeight,clientHeight:span.clientHeight};
   });
   return {bounds:bounds(rect),fontSize:parseFloat(style.fontSize),flexShrink:style.flexShrink,scrollWidth:element.scrollWidth,clientWidth:element.clientWidth,scrollHeight:element.scrollHeight,clientHeight:element.clientHeight,content:bounds(document.querySelector('.o-content').getBoundingClientRect()),spans};
  });
  assert.deepEqual(geometry.spans.map(span=>span.text),[expectedStatus,`Subscription Only · ${labels[language].quota}`],'all footer status and billing text must remain present');
  assert(Math.abs(geometry.fontSize-14*scale/100*.77)<.1,'the selected text scale remains unchanged');
  assert.equal(geometry.flexShrink,'0','the footer must retain its measured height when content grows');
  assert(geometry.bounds.height>28,'the footer expands beyond the original fixed height at 200%');
  assert(geometry.bounds.left>=0&&geometry.bounds.right<=viewport.width+1&&geometry.bounds.top>=0&&geometry.bounds.bottom<=viewport.height+1,'the complete footer remains within the viewport');
  assert(geometry.scrollWidth<=geometry.clientWidth+1&&geometry.scrollHeight<=geometry.clientHeight+1,'the footer has no clipped or overflowing content');
  assert(geometry.content.height>0&&geometry.content.bottom<=geometry.bounds.top+1,'scrollable content retains space above the footer');
  for(const span of geometry.spans){
   assert(Math.abs(span.fontSize-geometry.fontSize)<.1,'footer spans must not reduce the font');
   assert(span.textRects.length>0,'each footer label has visible text');
   assert(span.scrollWidth<=span.clientWidth+1&&span.scrollHeight<=span.clientHeight+1,'each footer label wraps without overflow');
   for(const rect of [span.bounds,...span.textRects])assert(rect.left>=geometry.bounds.left-1&&rect.right<=geometry.bounds.right+1&&rect.top>=geometry.bounds.top-1&&rect.bottom<=geometry.bounds.bottom+1,`complete label text must fit inside the footer: ${JSON.stringify({language,active,span,rect,footer:geometry.bounds})}`);
  }
  assert.deepEqual(errors,[]);
  const name=`900x620-${language}-200-${active?'active-suspended':'suspended'}`;
  await page.screenshot({path:output+name+'.png'});
  cases.push({language,active,viewport,scale,geometry,screenshot:name+'.png'});
  console.log(JSON.stringify({case:cases.length,language,active,footerHeight:geometry.bounds.height}));
  await page.close();current=null;
 }
 writeFileSync(output+'result.json',JSON.stringify({status:'passed',elapsedMs:performance.now()-started,cases,validationKind:'renderer-geometry-mocked-native',nativeDpiValidated:false},null,2)+'\n');
 console.log(JSON.stringify({status:'passed',cases:cases.length,elapsedMs:performance.now()-started,output}));
}catch(error){
 if(current)await current.page.screenshot({path:output+'failure.png'});
 writeFileSync(output+'result.json',JSON.stringify({status:'failed',elapsedMs:performance.now()-started,cases,failedCase:current?{language:current.language,active:current.active}:null,error:error.stack,validationKind:'renderer-geometry-mocked-native'},null,2)+'\n');
 console.error('Footer geometry evidence: '+output);throw error;
}finally{await browser.close();}
