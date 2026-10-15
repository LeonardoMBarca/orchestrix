// Complementary renderer QA. This mock never ships in the Desktop and proves
// neither Tauri IPC, filesystem permissions nor actual worker execution.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdirSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {defaultSettings} from '../src/protocol.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const output=fileURLToPath(new URL('../../../artifacts/desktop-renderer-qa/',import.meta.url));mkdirSync(output,{recursive:true});
const browser=await chromium.launch({channel:'chrome',headless:true});
const page=await browser.newPage({viewport:{width:1440,height:900},deviceScaleFactor:1});
const errors=[];page.on('pageerror',error=>errors.push(error.message));
try{
  await page.addInitScript(settings=>{
    const saved=sessionStorage.getItem('qa-state');
    const state=saved?JSON.parse(saved):{revision:0,projects:[],conversations:[],details:{},settings,connections:[],eventCursor:{database:'renderer-qa',sequence:0},admissionsSuspended:true};
    const calls=[];let loseNext=false;
    const daemon=()=>({state:{suspended:state.admissionsSuspended,generation:1,cursor:state.eventCursor},workers:[],workerCount:0,workerFactsHistorical:true,activeWorkerId:null,features:['desktop-v1','review-v1']});
    const receipts=new Map();
    const persist=()=>sessionStorage.setItem('qa-state',JSON.stringify(state));
    window.__qa={calls,state,loseNextSend(){loseNext=true;}};
    window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
    window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
      if(command==='connect_daemon')return daemon();
      if(command==='desktop_poll')return {daemon:daemon(),events:[],cursor:state.eventCursor,resyncRequired:false};
      if(command.startsWith('plugin:event|'))return 1;
      if(command==='desktop_request'){
        const request=args.request;calls.push(structuredClone(request));
        if(request.type==='bootstrap')return {...structuredClone(state),daemon:daemon()};
        if(request.type==='conversation')return structuredClone(state.details[request.conversationId]);
        if(request.type==='send-message'){
          if(receipts.has(request.commandId))return receipts.get(request.commandId);
          assertRevision(request);
          let conversation=state.details[request.conversationId];
          if(!conversation){conversation={id:request.conversationId,projectId:request.projectId??null,messages:[],work:[],draft:null,more:false,nextOrdinal:0};state.details[request.conversationId]=conversation;state.conversations.push({id:request.conversationId,title:request.content,projectId:request.projectId??null,messageCount:0});}
          conversation.messages.push({id:`${request.conversationId}-${conversation.messages.length+1}`,role:'user',text:request.content});conversation.nextOrdinal=conversation.messages.length;
          state.conversations.find(c=>c.id===request.conversationId).messageCount=conversation.messages.length;
          state.revision++;state.eventCursor.sequence++;persist();
          const result={replayed:false,result:{disposition:'applied',revision:state.revision,output:null}};receipts.set(request.commandId,result);
          if(loseNext){loseNext=false;throw new Error('response-not-confirmed-query-before-retry');}return result;
        }
        if(request.type==='save-settings'){assertRevision(request);state.settings=request.settings;state.revision++;state.eventCursor.sequence++;persist();return {result:{disposition:'applied',revision:state.revision}};}
        if(request.type==='save-draft'){assertRevision(request);state.details[request.conversationId].draft=request.content;state.revision++;persist();return {result:{disposition:'applied',revision:state.revision}};}
        throw new Error(`Unexpected renderer request: ${request.type}`);
      }
      throw new Error(`Unexpected native command: ${command}`);
    }};
    function assertRevision(request){if(request.expectedRevision!==state.revision)throw new Error('stale-revision-resync');}
  },defaultSettings);
  await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1420/');
  await page.getByRole('heading',{name:'What shall we create today?'}).waitFor();await page.waitForFunction(()=>document.fonts.status==='loaded');
  assert.equal(await page.evaluate(()=>window.__qa.state.conversations.length),0);
  await page.screenshot({path:output+'home-studio.png',fullPage:true});
  const composer=page.getByRole('textbox',{name:'Message',exact:true});
  await composer.fill('Investigate the parser');
  await page.evaluate(()=>window.__qa.loseNextSend());
  await page.getByRole('button',{name:'Send',exact:true}).click();
  await page.getByText('A lost reply remains pending with the original command identity.').waitFor();
  assert.equal(await page.getByRole('button',{name:'Send',exact:true}).isDisabled(),true);
  await page.getByRole('button',{name:'Recheck',exact:true}).click();
  await page.waitForFunction(()=>document.querySelector('textarea[aria-label="Message"]')?.value==='');
  assert.equal(await page.evaluate(()=>window.__qa.state.conversations.length),1);
  assert.equal(await page.evaluate(()=>Object.values(window.__qa.state.details)[0].messages.length),1);
  const sends=await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='send-message'));
  assert.equal(sends.length,2);assert.deepEqual(sends[0],sends[1]);
  await page.reload();await page.getByRole('heading',{name:'What shall we create today?'}).waitFor();
  assert.equal(await page.evaluate(()=>window.__qa.state.conversations.length),1);
  assert.equal(await composer.inputValue(),'');
  await page.getByRole('button',{name:'Settings',exact:true}).click();
  await page.getByRole('dialog').waitFor();await page.screenshot({path:output+'settings-studio.png',fullPage:true});
  await page.getByRole('button',{name:'Orchestrix',exact:true}).click();await page.getByRole('dialog').waitFor({state:'hidden'});
  await page.setViewportSize({width:1000,height:720});
  await page.screenshot({path:output+'home-compact.png',fullPage:true});
  assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  for(const [id,name]of [['studio','Studio'],['institutional','Institutional'],['atelier','Atelier'],['horizon','Horizon'],['deep-black','Deep Black'],['medieval','Medieval'],['forest','Forest'],['dawn','Dawn']]){
    await page.getByRole('button',{name:'Settings',exact:true}).click();
    await page.getByRole('button',{name:'Themes',exact:true}).click();
    await page.getByRole('dialog').getByRole('button',{name,exact:true}).click();
    await page.waitForFunction(theme=>window.__qa.state.settings.theme===theme,id);
    await page.getByRole('button',{name:'Orchestrix',exact:true}).click();
    await page.screenshot({path:output+`theme-${id}.png`,fullPage:true});
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
  }
  await page.getByRole('button',{name:'Settings',exact:true}).click();await page.getByRole('button',{name:'General',exact:true}).click();
  for(const language of ['pt-BR','es','en']){
    await page.locator('.o-settings select').first().selectOption(language);
    await page.waitForFunction(language=>window.__qa.state.settings.language===language,language);
    await page.screenshot({path:output+`language-${language}.png`,fullPage:true});
  }
  assert.deepEqual(errors,[]);
  console.log(JSON.stringify({status:'passed',kind:'renderer-mock-only',checks:['HOME-no-record','lost-reply-original-identity','composer-reconciliation','reload-HOME-history-preserved','modeless-settings','compact-no-horizontal-overflow','eight-themes','three-languages'],screenshots:output}));
}finally{await browser.close();}
