// Integrated renderer regression with an explicitly mocked native boundary.
// This does not certify daemon persistence, authenticated metadata or execution.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdir,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {defaultSettings} from '../src/protocol.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const output=fileURLToPath(new URL(`../../../artifacts/configuration-integration-renderer/${Date.now()}/`,import.meta.url));
await mkdir(output,{recursive:true});
const browser=await chromium.launch({channel:'chrome',headless:true});
const page=await browser.newPage({viewport:{width:1440,height:950}}),errors=[],checks=[];
page.on('pageerror',error=>errors.push(error.message));
try {
 await page.addInitScript(settings=>{
  const calls=[];let catalogReads=0,releaseCatalog=null,grantNumber=0;
  const implementation={id:'implementation-engineer',version:2,editRevision:0,origin:'native',archived:false,name:'Implementation Engineer',role:'implementation',instructions:'Use observable evidence',outputLanguage:'inherit',conventions:[],specialty:'Code',preferences:{},requirements:{subscription_only:true}};
  const reviewer={...implementation,id:'independent-reviewer',name:'Independent Reviewer',role:'review',requirements:{subscription_only:true,read_only:true}};
  const development={id:'development',name:'Development',version:2,editRevision:0,origin:'native',archived:false,purpose:'Develop code',instructions:'Preserve evidence',outputs:['Patch'],criteria:['Checks and review'],preferences:{},requirements:{subscription_only:true,require_review:true},roles:[{role:'implementation',profile:{id:implementation.id,version:2}},{role:'review',profile:{id:reviewer.id,version:2}}],executionSupport:'manual-worker'};
  const local={...development,id:'local-focused',name:'Newer local definition',origin:'local',version:1,base:{id:'development',version:2}};
  const oldCatalog={schemaVersion:1,revision:1,modes:[development],profiles:[implementation,reviewer],scopes:[]};
  const newCatalog={...oldCatalog,revision:2,modes:[development,local]};
  const connection={id:'account-a',label:'Synthetic account',name:'Synthetic account',provider:'Codex',mode:'subscription',status:'observed',revision:1,bindingRevision:1,selected:true,models:[{id:'model-a',name:'Synthetic model',efforts:['low','high']}],enabled:true,linked:true};
  const project={id:'project-a',repositoryId:'repository-a',name:'Configuration project',directory:'C:/owned-fixture/repository'};
  const conversation={id:'conversation-a',projectId:project.id,title:'Inspect configuration task',messageCount:1};
  const daemon=()=>({state:{suspended:false,generation:1,cursor:{database:'configuration-integration',sequence:0}},workers:[],workerCount:0,workerFactsHistorical:true,activeWorkerId:null,features:['desktop-v1','connections-v1','review-v1','configuration-v1']});
  window.__configurationIntegration={calls,get catalogReads(){return catalogReads;},get held(){return releaseCatalog!==null;},releaseOldCatalog(){const callback=releaseCatalog;releaseCatalog=null;callback?.(structuredClone(oldCatalog));}};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
  window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
   if(command.startsWith('plugin:event|'))return 1;
   if(command==='connect_daemon')return daemon();
   if(command==='desktop_poll')return {daemon:daemon(),cursor:daemon().state.cursor,events:[],resyncRequired:false};
   if(command==='choose_document'){calls.push({native:command,kind:args.kind});return {id:`directory-${++grantNumber}`,name:grantNumber===1?'Authorized repository':'Authorized worktrees',directory:true};}
   if(command!=='desktop_request')throw Error(`Unexpected mocked native command: ${command}`);
   const request=args.request;calls.push(structuredClone(request));
   if(request.type==='bootstrap')return {revision:2,settings,projects:[project],conversations:[conversation],connections:[connection],eventCursor:daemon().state.cursor,admissionsSuspended:false,daemon:daemon()};
   if(request.type==='configurations'&&request.request.type==='catalog'){
    catalogReads++;if(catalogReads===1)return new Promise(resolve=>{releaseCatalog=resolve;});return structuredClone(newCatalog);
   }
   if(request.type==='conversation')return {...conversation,messages:[{id:'message-one',ordinal:1,text:'Implement the authorized fixture change'}],draft:'',work:[]};
   if(request.type==='connection-diagnostics')return {latest:{state:'complete',current:true,connectionRevision:1},catalog:{connection:{id:connection.id,revision:1},models:[{id:'model-a',reasoningLevels:['low','high']}]},catalogLabels:[{id:'model-a',label:'Synthetic model',nativeEfforts:['low','high']}]};
   if(request.type==='prepare-task'){
    const draft=request.draft,preferences={strategy:draft.strategy,model_id:draft.model,reasoning:draft.effort,fallback:draft.fallback,connection_id:draft.connectionId};
    const policy={...structuredClone(settings.policy),mode:draft.mode,profile:draft.profile,layers:[{scope:'task',reference:'prepared-task-snapshot',revision:3,preferences,requirements:{}}]};
    return {id:'prepared-configuration',task:{objective:draft.objective,connection_revision:1,policy,repository_directory:'C:/owned-fixture/repository',authorized_worktree_directory:'C:/owned-fixture/worktrees',checks:[]},configuration:{schemaVersion:1,revision:2,policy,definitions:{mode:draft.mode,profile:draft.profile,reviewProfile:{id:'independent-reviewer',version:2}},decision:{preferences,origins:Object.fromEntries(Object.keys(preferences).map(key=>[key,'prepared-task-snapshot@task'])),requirements:{subscription_only:true,require_review:true},route:{kind:'blocked',value:['no-eligible-runtime']}},fingerprint:'synthetic-prepared-fingerprint',warnings:[]}};
   }
   throw Error(`Unexpected mocked request: ${request.type}`);
  }};
 },structuredClone(defaultSettings));
 await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1420/');
 await page.getByRole('heading',{name:'What shall we create today?',exact:true}).waitFor();
 await page.waitForFunction(()=>window.__configurationIntegration.held);
 await page.locator('.o-rail').getByRole('button',{name:'Settings',exact:true}).click();
 await page.locator('.o-settings-nav').getByRole('button',{name:'Modes & executor profiles',exact:true}).click();
 await page.locator('.o-configuration-editor').getByRole('combobox',{name:'Definition',exact:true}).locator('option[value="local-focused"]').waitFor({state:'attached'});
 await page.getByRole('button',{name:'Close settings',exact:true}).click();
 const composerMode=page.locator('.o-composer-tools').getByRole('combobox',{name:'Action mode',exact:true});
 assert.equal(await composerMode.locator('option[value="local-focused@1"]').count(),1);
 await page.evaluate(()=>window.__configurationIntegration.releaseOldCatalog());
 await page.waitForTimeout(80);
 assert.equal(await composerMode.locator('option[value="local-focused@1"]').count(),1,'late initial Catalog must not replace the newer editor catalog');
 await composerMode.selectOption('local-focused@1');
 assert.equal(await composerMode.inputValue(),'local-focused@1');
 checks.push('late-root-catalog-does-not-erase-newer-editor-definition');
 await page.locator('.o-session-row').filter({hasText:'Inspect configuration task'}).click();
 await page.getByRole('button',{name:'Prepare task',exact:true}).click();
 const form=page.locator('.o-task-form');
 assert.equal(await form.getByRole('combobox',{name:'Action mode',exact:true}).isEnabled(),true);
 assert.equal(await form.getByRole('combobox',{name:'Executor profile',exact:true}).isEnabled(),true);
 await form.locator('button').filter({hasText:'Select repository'}).click();
 await form.locator('button').filter({hasText:'Grant worktree folder'}).click();
 await form.getByRole('textbox',{name:'Acceptance criteria',exact:true}).fill('Fixture check passes');
 await form.getByRole('combobox',{name:'Resource strategy',exact:true}).selectOption('performance');
 await form.getByRole('combobox',{name:'Model',exact:true}).selectOption('model-a');
 await form.getByRole('combobox',{name:'Reasoning effort',exact:true}).selectOption('high');
 await form.getByRole('checkbox',{name:'Allow compatible runtime defaults when model access is unconfirmed',exact:true}).check();
 await form.getByRole('button',{name:'Prepare task',exact:true}).click();
 const preview=form.getByRole('region',{name:'Effective configuration preview',exact:true});await preview.waitFor();
 for(const [label,expected] of [['Resource strategy','performance'],['Model','model-a'],['Reasoning','high'],['Fallback','compatible'],['Connection','account-a']]){
  const field=preview.locator('.o-configuration-resolution>section').filter({has:page.getByRole('heading',{name:label,exact:true})});
  assert.equal(await field.locator('dt').first().innerText(),'Requested preference');
  assert.equal(await field.locator('dd').first().innerText(),expected,`${label} comes from the prepared Task layer, not an empty preferences object`);
  assert.equal(await field.locator('dd').last().innerText(),'prepared-task-snapshot@task');
 }
 const preparations=await page.evaluate(()=>window.__configurationIntegration.calls.filter(call=>call.type==='prepare-task'));
 assert.equal(preparations.length,1);assert.equal(preparations[0].draft.mode.id,'local-focused');assert.equal(preparations[0].draft.profile.version,2);
 checks.push('prepared-task-layer-is-shown-as-requested-preferences');
 const calls=await page.evaluate(()=>window.__configurationIntegration.calls);
 assert.equal(calls.some(call=>['submit-task','diagnose-connection','request-review','authorize-correction'].includes(call.type)),false,'this renderer regression grants no execution or authenticated reads');
 assert.deepEqual(errors,[]);await preview.locator('.o-configuration-resolution').scrollIntoViewIfNeeded();await page.screenshot({path:output+'prepared-task.png'});
 await writeFile(output+'result.json',JSON.stringify({passed:true,kind:'renderer-native-boundary-mock',checks,catalogReads:await page.evaluate(()=>window.__configurationIntegration.catalogReads),errors},null,2));
 console.log(JSON.stringify({passed:checks.length,checks,output}));
}catch(error){await page.screenshot({path:output+'failure.png'});throw error;}finally{await browser.close();}
