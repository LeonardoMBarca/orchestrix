// Renderer-only regression using the existing synthetic repository fixture.
// No daemon, metadata, credential, worker or native permission is exercised.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdir,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {createServer} from 'vite';
import {connectionTranslations} from '../src/connection-locales.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const output=fileURLToPath(new URL(`../../../artifacts/task-preparation-error-renderer/${Date.now()}/`,import.meta.url));
await mkdir(output,{recursive:true});
const server=process.env.ORCHESTRIX_RENDERER_URL?null:await createServer({root:fileURLToPath(new URL('../',import.meta.url)),logLevel:'error',server:{host:'127.0.0.1',port:0}});
await server?.listen();
const url=process.env.ORCHESTRIX_RENDERER_URL??`http://127.0.0.1:${server.httpServer.address().port}`;
const browser=await chromium.launch({channel:'chrome',headless:true});
const page=await browser.newPage({viewport:{width:900,height:680}}),errors=[],checks=[];
page.on('pageerror',error=>errors.push(error.message));
try{
 await page.goto(`${url}/tests/repository-inspection.fixture.html`);
 await page.getByRole('button',{name:'Inspect repository',exact:true}).click();
 await page.getByText('Compatible with the worker profile',{exact:true}).waitFor();
 const prepare=page.locator('.o-task-fields>.o-actions>.o-primary');
 assert(await prepare.isEnabled());
 await page.evaluate(()=>window.__repositoryQa.failPreparation('connection-identity-reconfirmation-required'));
 const message='Observed account details changed. Review the connection in Connections and explicitly confirm the link before preparing new work. This does not prove that the account switched.';
 for(const language of ['en','pt','es']){
  await page.evaluate(language=>window.__repositoryQa.language(language,200),language);
  const before=await page.evaluate(()=>window.__repositoryQa.calls.filter(call=>call.type==='prepare-task').length);
  await prepare.click();
  const expected=language==='en'?message:connectionTranslations[message][language==='pt'?0:1];
  const alert=page.getByRole('alert').filter({hasText:expected});await alert.waitFor();
  assert.equal(await alert.innerText(),expected);
  await page.waitForFunction(()=>document.querySelector('.o-task-fields')?.disabled===false);
  assert.equal(await page.evaluate(()=>window.__repositoryQa.calls.filter(call=>call.type==='prepare-task').length),before+1,'one explicit click must not retry preparation');
  assert.equal(await page.locator('.o-task-goals textarea').first().inputValue(),'Preserve this draft while inspecting');
  assert.equal(await page.locator('.o-task-authorization').count(),0,'a rejected preparation grants no execution consent');
  assert.equal(await page.locator('.o-task-fields input[type=checkbox]').last().isChecked(),false,'failure must not grant metadata consent');
  // Inspect the warning itself at the left origin. The isolated fixture does not
  // certify the full Desktop shell or preserve a browser's horizontal scroll.
  await alert.evaluate(element=>{element.scrollIntoView({block:'center',inline:'start',behavior:'instant'});window.scrollTo({left:0,top:window.scrollY,behavior:'instant'});});
  const bounds=await alert.evaluate(element=>{const rect=element.getBoundingClientRect();return {width:element.clientWidth,scrollWidth:element.scrollWidth,left:rect.left,right:rect.right,viewport:innerWidth};});
  assert(bounds.scrollWidth<=bounds.width+1&&bounds.left>=0&&bounds.right<=bounds.viewport+1,'localized error must wrap within the viewport at the selected text scale');
  await alert.screenshot({path:`${output}${language}-200-900.png`});
  checks.push({language,scale:200,width:900,manualPreparations:before+1,localizedError:true,draftPreserved:true});
 }
 const calls=await page.evaluate(()=>window.__repositoryQa.calls);
 assert.equal(calls.filter(call=>call.type==='prepare-task').length,3);
 assert(calls.every(call=>['inspect-repository','prepare-task'].includes(call.type)),'no recheck, identity confirmation or worker submission is introduced');
 assert.deepEqual(errors,[]);
 await writeFile(`${output}result.json`,JSON.stringify({checks,calls,errors,kind:'renderer-native-boundary-mock',native:false,providerRead:false},null,2));
 console.log(JSON.stringify({passed:checks.length,checks,output}));
}catch(error){await page.screenshot({path:`${output}failure.png`});throw error;}finally{await browser.close();await server?.close();}
