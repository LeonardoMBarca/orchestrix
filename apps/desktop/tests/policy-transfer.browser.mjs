// Actual renderer, explicitly mocked native boundary. This does not validate
// native file dialogs, daemon persistence, authentication or provider behavior.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdir,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {defaultSettings} from '../src/protocol.ts';
import {controlTranslations} from '../src/ox016-locales.ts';

const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const output=fileURLToPath(new URL(`../../../artifacts/policy-transfer-renderer/${Date.now()}/`,import.meta.url));
await mkdir(output,{recursive:true});
const browser=await chromium.launch({channel:'chrome',headless:true});
const page=await browser.newPage({viewport:{width:1440,height:900}});
const errors=[];page.on('pageerror',error=>errors.push(error.message));
const t=(key,language='en')=>language==='en'?key:controlTranslations[key]?.[language==='pt-BR'?0:1]??key;
const checks=[];
try {
 await page.addInitScript(settings=>{
  let sequence=0,importNumber=0,choice='cancel',saveChoice=false,failApply=false,customDefinitions=false,holdCatalog=false,deferredCatalog=null;
  const imports=new Map();
  const calls=[],events=[];
  // IDs/names deliberately coincide with source aliases/hints. None may auto-map.
  const connection={id:'connection-1',label:'connection-1',name:'connection-1',revision:7,bindingRevision:4,provider:'Codex',mode:'subscription',status:'observed',enabled:true,linked:true,selected:true,models:[{id:'local-model',name:'Reported local model',efforts:['low','high']}]};
  const state={revision:10,settings,projects:[],conversations:[],connections:[connection],eventCursor:{database:'portable-renderer',sequence:0},admissionsSuspended:false};
  const daemon=()=>({state:{generation:2,suspended:false,cursor:{database:'portable-renderer',sequence}},workers:[],workerCount:0,workerFactsHistorical:true,activeWorkerId:null,features:['desktop-v1','connections-v1','policy-portability-v1']});
  const emit=()=>{sequence++;events.push({sequence,kind:'command-finished',body:{commandId:`policy-${sequence}`,state:'finished'}});};
  function preview(mappings,custom=false){
   const c=mappings.connections.find(c=>c.alias==='connection-1'),m=mappings.models.find(m=>m.alias==='model-1');
   const issues=[];
   if(!c)issues.push({code:'portable-connection-unresolved',reference:'connection-1'});
   if(!m?.modelId)issues.push({code:'portable-model-unresolved',reference:'model-1'});
   if(c&&(c.connectionId!==connection.id||c.bindingRevision!==connection.bindingRevision))issues.push({code:'connection-binding-revision-changed',reference:'connection-1'});
   if(m?.modelId&&m.modelId!=='local-model')issues.push({code:'portable-model-not-observed',reference:'model-1'});
   if(custom&&(!mappings.mode||mappings.mode.id!==state.settings.policy.mode.id||mappings.mode.version!==state.settings.policy.mode.version))issues.push({code:'portable-definition-remapping-required',reference:'mode'});
   if(custom&&(!mappings.profile||mappings.profile.id!==state.settings.policy.profile.id||mappings.profile.version!==state.settings.policy.profile.version))issues.push({code:'portable-definition-remapping-required',reference:'profile'});
   return {schemaVersion:1,expectedRevision:state.revision,documentDigest:'document-fixture',previewDigest:`preview-${state.revision}-${c?.connectionId??''}-${m?.modelId??''}-${mappings.mode?.id??''}-${mappings.profile?.id??''}`,canApply:issues.length===0,issues,changes:['Supported policy and appearance only','No credentials or execution authority'],connectionReferences:[{alias:'connection-1'}],modelReferences:[{alias:'model-1',sourceModelId:'local-model',connectionAlias:'connection-1'}],definitionReferences:{mode:{id:custom?'source-custom-mode':'development',version:custom?2:1},profile:{id:custom?'source-custom-profile':'implementation-engineer',version:custom?3:1}},mappings:structuredClone(mappings)};
  }
  const report=()=>({latest:{id:'historical-read',state:'complete',current:true,connectionRevision:4},catalog:{connection:{id:connection.id,revision:4},models:[{id:'local-model',availability:'available',reasoningLevels:['low','high']}]},catalogLabels:[{id:'local-model',label:'Reported local model',nativeEfforts:['low','high']}]});
  window.__qa={calls,state,setChoice(value){choice=value;},setDefinitions(custom){customDefinitions=custom;},setSave(value){saveChoice=value;},failApply(){failApply=true;},holdCatalog(){holdCatalog=true;},hasDeferredCatalog(){return deferredCatalog!==null;},releaseCatalog(){const release=deferredCatalog;deferredCatalog=null;release?.(report());},language(language,textScale){state.settings.language=language;state.settings.textScale=textScale;emit();}};
  window.__TAURI_EVENT_PLUGIN_INTERNALS__={unregisterListener(){}};
  window.__TAURI_INTERNALS__={transformCallback(){return 1;},unregisterCallback(){},async invoke(command,args={}){
   if(command.startsWith('plugin:event|'))return 1;
   if(command==='connect_daemon')return daemon();
   if(command==='desktop_poll')return {daemon:daemon(),cursor:{database:'portable-renderer',sequence},events:events.filter(e=>e.sequence>args.cursor.sequence),resyncRequired:false};
   if(command==='choose_policy_import'){
    calls.push({native:command});
    if(choice==='cancel')return null;
    if(choice==='invalid')throw Error('portable-version');
    const importRef=`opaque-import-${++importNumber}`;imports.set(importRef,customDefinitions);
    return {importRef,name:'policy-fixture.json',preview:preview({connections:[],models:[],mode:null,profile:null},customDefinitions)};
   }
   if(command==='save_policy_export'){calls.push({native:command,...args});return saveChoice?{saved:true}:null;}
   if(command==='desktop_request'){
    const r=args.request;calls.push(structuredClone(r));
    if(r.type==='bootstrap')return {...structuredClone(state),daemon:daemon(),eventCursor:{database:'portable-renderer',sequence}};
    if(r.type==='preview-policy-import'){if(!imports.has(r.importRef))throw Error('released-import');return preview(r.mappings,imports.get(r.importRef));}
    if(r.type==='release-policy-import'){imports.delete(r.importRef);return {released:true};}
    if(r.type==='connection-diagnostics'){if(holdCatalog){holdCatalog=false;return new Promise(resolve=>{deferredCatalog=resolve;});}return report();}
    if(r.type==='prepare-policy-export')return {exportRef:'opaque-export',summary:{credentialsIncluded:false,executionAuthorityIncluded:false},document:JSON.stringify({schemaVersion:1,format:'orchestrix-policy',policy:{strategy:'performance'},connectionReferences:[{alias:'connection-1'}]})};
    if(r.type==='apply-policy-import'){
     if(!r.confirm)throw Error('explicit-confirmation-required');
     if(failApply){failApply=false;state.revision++;emit();throw Error('stale-revision-resync');}
     const p=preview(r.mappings,imports.get(r.importRef));
     if(!p.canApply||p.previewDigest!==r.expectedPreviewDigest||r.expectedRevision!==state.revision)throw Error('stale-preview');
     state.settings.theme='horizon';state.settings.policy.revision++;state.settings.policy.mode_defaults={strategy:'performance',connection_id:connection.id,model_id:'local-model'};state.revision++;emit();
     return {result:{disposition:'applied',revision:state.revision},replayed:false};
    }
    throw Error(`Unexpected renderer operation: ${r.type}`);
   }
   throw Error(`Unexpected native command: ${command}`);
  }};
 },defaultSettings);
 await page.goto(process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1421/');
 await page.getByRole('heading',{name:'What shall we create today?'}).waitFor();
 await page.getByRole('button',{name:'Settings',exact:true}).click();
 await page.getByRole('button',{name:'Orchestration',exact:true}).click();
 const transfer=()=>page.getByRole('region',{name:'Policy portability',exact:true});
 const source=()=>transfer().locator('.o-transfer-preview').filter({has:page.getByRole('heading',{name:/^Import preview/})});
 const baseline=await page.evaluate(()=>structuredClone(window.__qa.state.settings));
 await transfer().getByRole('button',{name:'Choose policy file',exact:true}).click();
 assert.equal(await transfer().locator('.o-transfer-preview').count(),0);
 assert.deepEqual(await page.evaluate(()=>window.__qa.state.settings),baseline);
 checks.push('native-file-cancel-preserves-state');

 async function chooseAndMap({custom=false}={}){
  await page.evaluate(custom=>{window.__qa.setChoice('valid');window.__qa.setDefinitions(custom);},custom);
  await transfer().getByRole('button',{name:'Choose policy file',exact:true}).click();
  await source().waitFor();
  const localConnection=source().getByRole('combobox',{name:/^Local connection/});
  const localModel=source().getByRole('combobox',{name:'Local model',exact:true});
  assert.equal(await localConnection.inputValue(),'','matching source alias/local ID must not authorize a mapping');
  assert.equal(await localModel.inputValue(),'','matching model hint/local ID must not auto-select a model');
  assert.equal(await localModel.isDisabled(),true);
  await localConnection.selectOption('connection-1');
  await localModel.waitFor();
  await localModel.selectOption('local-model');
  const consent=source().getByRole('checkbox');
  await consent.waitFor();
  await page.waitForFunction(custom=>document.querySelector('.o-policy-transfer input[type=checkbox]')?.disabled===custom,custom);
  assert.equal(await consent.isChecked(),false);
  assert.equal(await source().getByRole('button',{name:'Confirm import',exact:true}).isDisabled(),true);
  return consent;
 }
 let consent=await chooseAndMap();
 const firstPreview=await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='preview-policy-import').at(-1));
 assert.deepEqual(firstPreview.mappings.connections,[{alias:'connection-1',connectionId:'connection-1',bindingRevision:4}]);
 assert.deepEqual(firstPreview.mappings.models,[{alias:'model-1',connectionAlias:'connection-1',modelId:'local-model'}]);
 await consent.check();await source().getByRole('button',{name:'Cancel import',exact:true}).click();
 await source().waitFor({state:'hidden'});
 assert.deepEqual(await page.evaluate(()=>window.__qa.state.settings),baseline);
 assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='apply-policy-import').length),0);
 assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='release-policy-import').length),1);
 checks.push('explicit-remapping-and-consent','cancel-preview-without-apply');

 consent=await chooseAndMap();await consent.check();await page.evaluate(()=>window.__qa.failApply());
 await source().getByRole('button',{name:'Confirm import',exact:true}).click();
 await page.getByText(/stale-revision-resync/).waitFor();
 assert.deepEqual(await page.evaluate(()=>window.__qa.state.settings),baseline);
 await source().getByRole('button',{name:'Refresh preview',exact:true}).click();
 await page.waitForFunction(()=>!document.querySelector('.o-policy-transfer input[type=checkbox]')?.disabled);
 assert.equal(await source().getByRole('checkbox').isChecked(),false,'refreshed preview requires a new exact confirmation');
 await source().getByRole('checkbox').check();await source().getByRole('button',{name:'Confirm import',exact:true}).click();
 await transfer().getByRole('status').filter({hasText:'Policy imported'}).waitFor();
 const after=await page.evaluate(()=>window.__qa.state.settings);
 assert.equal(after.theme,'horizon');assert.equal(after.policy.mode_defaults.connection_id,'connection-1');assert.equal(after.policy.mode_defaults.model_id,'local-model');
 const applied=await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='apply-policy-import'));
 assert.equal(applied.length,2);assert.notEqual(applied[0].expectedRevision,applied[1].expectedRevision);assert.notEqual(applied[0].expectedPreviewDigest,applied[1].expectedPreviewDigest);
 checks.push('stale-revision-preserves-settings','refresh-requires-new-confirmation','successful-import-uses-exact-preview');

 await page.evaluate(()=>window.__qa.setChoice('invalid'));
 await transfer().getByRole('button',{name:'Choose policy file',exact:true}).click();
 await page.getByText(/portable-version/).waitFor();
 assert.deepEqual(await page.evaluate(()=>window.__qa.state.settings),after);
 await transfer().getByRole('button',{name:'Prepare export',exact:true}).click();
 const exported=transfer().locator('.o-transfer-preview').filter({has:page.getByRole('heading',{name:'Export preview',exact:true})});
 await exported.waitFor();await exported.getByRole('button',{name:'Save export file',exact:true}).click();
 assert.equal(await exported.count(),1,'cancelled native save retains export preview');
 assert.deepEqual(await page.evaluate(()=>window.__qa.state.settings),after);
 await exported.getByRole('button',{name:'Cancel',exact:true}).click();await exported.waitFor({state:'hidden'});
 await transfer().getByRole('button',{name:'Prepare export',exact:true}).click();await page.evaluate(()=>window.__qa.setSave(true));await exported.getByRole('button',{name:'Save export file',exact:true}).click();
 await transfer().getByRole('status').filter({hasText:'Policy exported'}).waitFor();
 checks.push('invalid-file-preserves-settings','export-native-save-cancel','export-explicit-save');

 const customConsent=await chooseAndMap({custom:true});assert.equal(await customConsent.isDisabled(),true);
 await source().getByText('source-custom-mode · v2',{exact:true}).waitFor();
 const mode=source().getByRole('combobox',{name:'Destination action mode',exact:true}),profile=source().getByRole('combobox',{name:'Destination executor profile',exact:true});
 assert.equal(await mode.inputValue(),'');assert.equal(await profile.inputValue(),'');
 await mode.selectOption('current');await profile.selectOption('current');
 await page.waitForFunction(()=>!document.querySelector('.o-policy-transfer input[type=checkbox]')?.disabled);
 const definitions=await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='preview-policy-import').at(-1).mappings);
 assert.deepEqual(definitions.mode,{id:'development',version:1});assert.deepEqual(definitions.profile,{id:'implementation-engineer',version:1});
 assert.equal(await source().getByRole('checkbox').isChecked(),false);
 await source().getByRole('button',{name:'Cancel import',exact:true}).click();
 checks.push('custom-mode-and-profile-require-explicit-local-remapping');

 await page.evaluate(()=>{window.__qa.setChoice('valid');window.__qa.setDefinitions(false);window.__qa.holdCatalog();});
 await transfer().getByRole('button',{name:'Choose policy file',exact:true}).click();await source().waitFor();
 await source().getByRole('combobox',{name:/^Local connection/}).selectOption('connection-1');
 await page.waitForFunction(()=>window.__qa.hasDeferredCatalog());
 assert.equal(await source().getByRole('combobox',{name:/^Local connection/}).isDisabled(),true,'catalog loading immediately fences further remapping');
 assert.equal(await source().getByRole('checkbox').isChecked(),false);
 await page.getByRole('button',{name:'General',exact:true}).click();await transfer().waitFor({state:'hidden'});
 await page.getByRole('button',{name:'Orchestration',exact:true}).click();
 await transfer().getByRole('button',{name:'Choose policy file',exact:true}).click();await source().waitFor();
 const previewsBefore=await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='preview-policy-import').length);
 await page.evaluate(()=>window.__qa.releaseCatalog());
 assert.equal(await source().getByRole('combobox',{name:/^Local connection/}).inputValue(),'');
 assert.equal(await page.evaluate(()=>window.__qa.calls.filter(c=>c.type==='preview-policy-import').length),previewsBefore,'late closed-panel catalog response cannot preview or resurrect its old mapping');
 await source().getByRole('button',{name:'Cancel import',exact:true}).click();
 checks.push('late-catalog-response-cannot-resurrect-closed-import');

 await chooseAndMap();
 const geometries=[];
 for(const language of ['en','pt-BR','es'])for(const scale of [100,200])for(const width of [1440,900]){
  await page.setViewportSize({width,height:width===900?620:900});
  await page.evaluate(({language,scale})=>window.__qa.language(language,scale),{language,scale});
  await page.waitForFunction(({language,scale})=>document.documentElement.lang===language&&getComputedStyle(document.documentElement).fontSize===`${14*scale/100}px`,{language,scale});
  const region=page.getByRole('region',{name:t('Policy portability',language),exact:true});
  await region.getByRole('button',{name:t('Refresh preview',language),exact:true}).scrollIntoViewIfNeeded();
  await region.getByRole('checkbox').focus();await page.keyboard.press('Space');await page.keyboard.press('Space');
  const box=await page.locator('.o-settings-content').evaluate(element=>({width:element.clientWidth,scrollWidth:element.scrollWidth,scrollHeight:element.scrollHeight,clientHeight:element.clientHeight,rootFont:getComputedStyle(document.documentElement).fontSize}));
  assert(box.scrollWidth<=box.width+1,`policy content must reflow without horizontal clipping: ${JSON.stringify({language,scale,width,box})}`);
  assert.equal(box.rootFont,`${14*scale/100}px`);
  const confirm=region.getByRole('button',{name:t('Confirm import',language),exact:true});await confirm.scrollIntoViewIfNeeded();
  const bounds=await confirm.boundingBox();assert(bounds&&bounds.width>0&&bounds.x>=0&&bounds.x+bounds.width<=width+1,`confirmation remains reachable: ${JSON.stringify({language,scale,width,bounds})}`);
  geometries.push({language,scale,viewportWidth:width,...box});
  if(width===900)await page.screenshot({path:`${output}${language}-${scale}-compact.png`});
 }
 const calls=await page.evaluate(()=>window.__qa.calls);
 const unauthorized=calls.filter(c=>['diagnose-connection','configure-connection-refresh','prepare-task','submit-task','request-review','authorize-correction','control','link-connection'].includes(c.type));
 assert.deepEqual(unauthorized,[],'portability must not ask for metadata refresh, login or execution');
 assert.deepEqual(errors,[]);
 checks.push('three-languages-two-scales-two-widths','keyboard-confirmation-and-reachable-controls','no-diagnostic-login-or-runtime-side-effect');
 const result={status:'passed',kind:'renderer-mock-only',checks,geometries,source:'policy-transfer.browser.mjs',screenshots:output};
 await writeFile(`${output}result.json`,JSON.stringify(result,null,2));console.log(JSON.stringify(result));
}finally{await browser.close();}
