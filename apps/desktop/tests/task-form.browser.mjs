// Renderer-only interaction regression. Native permissions/execution require
// the separate Windows fixture proof; this mock never ships in the app.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdirSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {defaultSettings} from '../src/protocol.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const browser=await chromium.launch({channel:'chrome',headless:true});
const page=await browser.newPage({viewport:{width:1442,height:794}});
const errors=[];page.on('pageerror',error=>errors.push(error.message));
try{
 await page.addInitScript(settings=>{
  const conversation={id:'conversation-one',title:'Fix arithmetic',projectId:null,messageCount:1};
  const detail={...conversation,messages:[{id:'message-one',ordinal:1,role:'user',text:'Fix arithmetic'}],draft:'',work:[],nextOrdinal:1,more:false};
  const other={id:'conversation-two',title:'Another conversation',projectId:'project-one',messageCount:1},otherDetail={...other,messages:[{id:'message-two',ordinal:1,role:'user',text:'Explain the parser'}],draft:'',work:[],nextOrdinal:1,more:false};
  const state={revision:1,projects:[{id:'project-one',name:'Arithmetic'}],conversations:[conversation,other],settings,connections:[{id:'connection-one',label:'Fixture account',provider:'synthetic',mode:'subscription',status:'historical'}],eventCursor:{database:'task-form-test',sequence:0},admissionsSuspended:true};
  const calls=[];let grantNumber=0,preparationNumber=0,releasePrepare=null;
  const daemon=()=>({state:{suspended:state.admissionsSuspended,generation:1,cursor:state.eventCursor},workers:[],workerCount:0,workerFactsHistorical:true,activeWorkerId:null});
  window.__qa={calls,state,holdPrepare:false,finishPrepare(){releasePrepare?.();}};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
  window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
   if(command==='connect_daemon')return daemon();
   if(command==='desktop_poll')return {daemon:daemon(),events:[],cursor:state.eventCursor,resyncRequired:false};
   if(command.startsWith('plugin:event|'))return 1;
   if(command==='choose_document'){grantNumber++;return {id:`grant-${grantNumber}`,name:`Authorized folder ${grantNumber}`,directory:args.kind==='folder'};}
   if(command==='editor_request'){calls.push(structuredClone(args.request));const r=args.request;if(r.type==='create')return {id:`document-${grantNumber}`,name:r.name,text:'',version:'version-one',origin:'authorized-file',affectedWorkers:[]};if(r.type==='release')return {};throw Error('Unexpected editor command');}
   if(command==='desktop_request'){
    const r=args.request;calls.push(structuredClone(r));
    if(r.type==='bootstrap')return {...structuredClone(state),daemon:daemon()};
    if(r.type==='conversation')return structuredClone(r.conversationId===other.id?otherDetail:detail);
    if(r.type==='attach-conversation'){state.revision++;conversation.projectId=r.projectId;detail.projectId=r.projectId;return {applied:true};}
    if(r.type==='prepare-task'){if(window.__qa.holdPrepare)await new Promise(resolve=>{releasePrepare=resolve;});preparationNumber++;return {id:`prepared-${preparationNumber}`,task:{objective:r.draft.objective,repository_directory:'C:\\fixture\\repository',authorized_worktree_directory:'C:\\fixture\\worktrees',checks:[]}};}
    if(r.type==='control'&&r.action==='resume'){state.admissionsSuspended=false;return {state:'finished'};}
    if(r.type==='submit-task')return {state:'accepted'};
   }
   throw Error(`Unexpected native command ${command}`);
  }};
 },defaultSettings);
 await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1420/');
 await page.getByRole('heading',{name:'What shall we create today?'}).waitFor();
 await page.locator('.o-session-row').filter({hasText:'Fix arithmetic'}).click();
 await page.getByRole('button',{name:'Prepare task',exact:true}).click();
 await page.getByText('This conversation is independent. Attach it to a project before preparing a task.').waitFor();
 assert.equal(await page.getByRole('button',{name:'Prepare task',exact:true}).isDisabled(),true);
 await page.getByRole('combobox',{name:'Project',exact:true}).selectOption('project-one');
 await page.getByRole('button',{name:'Attach conversation',exact:true}).click();
 await page.locator('.o-task-attach').waitFor({state:'hidden'});
 await page.getByRole('textbox',{name:'Acceptance criteria',exact:true}).fill('2 + 2 is 4');
 // The surrounding native field labels name these buttons in the accessibility tree.
 await page.getByRole('button',{name:'Repository',exact:true}).click();
 await page.getByRole('button',{name:'Worktree folder',exact:true}).click();
 const objective=page.getByRole('textbox',{name:'Objective',exact:true});
 assert.ok((await objective.boundingBox()).height<100,'compact fields must override inherited 112px minimum');
 await page.getByRole('button',{name:'Prepare task',exact:true}).click();
 await page.getByRole('heading',{name:'Review prepared request',exact:true}).waitFor();
 assert.equal(await page.locator('.o-prepared-details').getAttribute('open'),null);
 await page.getByLabel('I authorize one runtime turn using my subscription.',{exact:true}).check();
 assert.equal(await page.getByRole('button',{name:'Authorize and submit',exact:true}).isDisabled(),true);
 await page.getByRole('button',{name:'Resume admissions',exact:true}).click();
 await page.getByRole('button',{name:'Authorize and submit',exact:true}).waitFor();
 assert.equal(await page.getByRole('button',{name:'Authorize and submit',exact:true}).isEnabled(),true);
 assert.equal(await page.evaluate(()=>window.__qa.calls.filter(call=>call.type==='submit-task').length),0,'Resume is not submission consent');
 await objective.fill('Fix arithmetic and document it');
 assert.equal(await page.locator('.o-task-authorization').count(),0,'editing withdraws prepared request and consent');
 await page.getByRole('button',{name:'Prepare task',exact:true}).click();
 await page.getByRole('heading',{name:'Review prepared request',exact:true}).waitFor();
 assert.equal(await page.getByLabel('I authorize one runtime turn using my subscription.',{exact:true}).isChecked(),false);
 await page.getByRole('button',{name:'Add check',exact:true}).click();
 const remove=page.locator('.o-check-card').getByRole('button',{name:'Remove',exact:true});
 assert.ok((await remove.boundingBox()).width<150,'Remove uses natural action width');
 await remove.click();
 await page.getByRole('button',{name:'File',exact:true}).click();await page.getByRole('menuitem',{name:'New file',exact:true}).click();
 await page.getByRole('dialog',{name:'New file',exact:true}).waitFor();
 assert.equal(await page.getByRole('combobox',{name:'Destination folder',exact:true}).inputValue(),'','multiple grants never select the first implicitly');
 await page.getByRole('textbox',{name:'File name',exact:true}).fill('notes.md');
 await page.getByRole('combobox',{name:'Destination folder',exact:true}).selectOption('grant-2');
 await page.getByRole('button',{name:'Create file',exact:true}).click();
 await page.getByRole('dialog',{name:'New file',exact:true}).waitFor({state:'hidden'});
 assert.equal(await page.evaluate(()=>window.__qa.calls.find(call=>call.type==='create').grant),'grant-2');
 await page.locator('.o-dock').getByRole('button',{name:'New file',exact:true}).click();
 assert.equal(await page.getByRole('combobox',{name:'Destination folder',exact:true}).inputValue(),'grant-2','active authorized file grant is the explicit default');
 await page.getByRole('dialog',{name:'New file',exact:true}).getByRole('button',{name:'Cancel',exact:true}).click();
 await page.locator('.o-dock.left .o-dock-grip').click();
 await page.locator('.o-docking-overlay').getByRole('button',{name:'Right',exact:true}).click();
 const filesTab=page.getByRole('tab',{name:'Files',exact:true}),sessionsTab=page.getByRole('tab',{name:'Sessions',exact:true});
 await filesTab.press('End');await page.waitForFunction(()=>document.activeElement?.id==='dock-tab-right-sessions');
 assert.equal(await sessionsTab.getAttribute('aria-selected'),'true');
 await sessionsTab.press('Home');await page.waitForFunction(()=>document.activeElement?.id==='dock-tab-right-navigation');
 assert.equal(await filesTab.getAttribute('aria-selected'),'true');
 await filesTab.press('Shift+ArrowLeft');await page.waitForFunction(()=>document.activeElement?.id==='dock-grip-left');
 assert.equal(await page.locator('.o-dock').count(),2);assert.equal(await page.getByRole('tab').count(),0,'returning Navigation leaves one independent panel on each side');
 await page.locator('.o-session-row').filter({hasText:'Fix arithmetic'}).click();await page.getByRole('button',{name:'Prepare task',exact:true}).click();
 await page.getByRole('textbox',{name:'Acceptance criteria',exact:true}).fill('Arithmetic remains correct');
 await page.getByRole('button',{name:'Repository',exact:true}).click();await page.getByRole('button',{name:'Worktree folder',exact:true}).click();
 await page.evaluate(()=>{window.__qa.holdPrepare=true;});await page.getByRole('button',{name:'Prepare task',exact:true}).click();
 await page.locator('.o-session-row').filter({hasText:'Another conversation'}).click();await page.getByRole('button',{name:'Prepare task',exact:true}).click();
 assert.equal(await page.getByRole('textbox',{name:'Objective',exact:true}).inputValue(),'Explain the parser');
 await page.evaluate(()=>{window.__qa.holdPrepare=false;window.__qa.finishPrepare();});
 await page.waitForFunction(()=>!document.querySelector('.o-task-fields')?.disabled);
 assert.equal(await page.locator('.o-task-authorization').count(),0,'a delayed preparation from the unmounted conversation must not authorize the new task');
 await page.getByRole('textbox',{name:'Acceptance criteria',exact:true}).fill('Explain the parser precisely');
 await page.getByRole('button',{name:'Repository',exact:true}).click();await page.getByRole('button',{name:'Worktree folder',exact:true}).click();await page.getByRole('button',{name:'Prepare task',exact:true}).click();
 await page.getByRole('heading',{name:'Review prepared request',exact:true}).waitFor();
 assert.equal(await page.locator('.o-task-summary dd').first().innerText(),'Explain the parser');
 assert.equal(await page.getByLabel('I authorize one runtime turn using my subscription.',{exact:true}).isChecked(),false);
 assert.deepEqual(errors,[]);
 console.log('Task form renderer regression passed: contextual attach, compact inputs, inspectable request, consent invalidation, explicit resume, selected file destination and dock keyboard focus.');
}catch(error){
 const output=fileURLToPath(new URL('../../../artifacts/desktop-renderer-qa/task-form-failure/',import.meta.url));mkdirSync(output,{recursive:true});
 await page.screenshot({path:output+'screen.png',fullPage:true});
 console.error(await page.locator('.o-task-form').ariaSnapshot().catch(()=>page.locator('body').innerText()));
 throw error;
}finally{await browser.close();}
