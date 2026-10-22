// Renderer-only regression. Synthetic responses do not prove Git, grants or daemon behavior.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdir,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {repositoryTranslations} from '../src/repository-locales.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url)),{chromium}=require('playwright');
const browser=await chromium.launch({channel:'chrome',headless:true}),page=await browser.newPage({viewport:{width:1440,height:900}}),errors=[],checks=[];
page.on('pageerror',error=>errors.push(error.message));
const output=fileURLToPath(new URL(`../../../artifacts/repository-inspection-renderer/${Date.now()}/`,import.meta.url));await mkdir(output,{recursive:true});
const t=(text,language)=>language==='en'?text:repositoryTranslations[text]?.[language==='pt'?0:1]??text;
const region=()=>page.getByRole('region',{name:'Repository inspection',exact:true}),inspect=()=>region().getByRole('button',{name:'Inspect repository',exact:true}),prepare=()=>page.getByRole('button',{name:'Prepare task',exact:true});
const scenario=async value=>page.evaluate(value=>window.__repositoryQa.scenario(value),value);
const privacy='Pattern matches in preserved files do not prove real credentials and do not authorize their content as context or edits. Selected text still requires privacy validation.';
async function shownGeometry(view){
 await page.waitForFunction(()=>document.fonts.status==='loaded');
 await view.evaluate(element=>{
  element.scrollIntoView({block:'start',inline:'nearest',behavior:'instant'});
  return new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
 });
 return view.evaluate(element=>{
  const r=element.getBoundingClientRect(),visible=document.elementFromPoint(Math.min(innerWidth-1,r.left+8),Math.min(innerHeight-1,Math.max(1,r.top+8)));
  return {width:element.clientWidth,scrollWidth:element.scrollWidth,left:r.left,right:r.right,top:r.top,bottom:r.bottom,scrollX,scrollY,viewport:innerWidth,viewportHeight:innerHeight,documentWidth:document.documentElement.scrollWidth,visibleTarget:visible!==null&&element.contains(visible)};
 });
}
try{
 await page.goto(`${process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1428'}/tests/repository-inspection.fixture.html`);await inspect().waitFor();assert.equal(await page.evaluate(()=>window.__repositoryQa.calls.length),0);assert(await prepare().isDisabled());
 await inspect().focus();await page.keyboard.press('Enter');await region().getByText('Compatible with the worker profile',{exact:true}).waitFor();assert(await prepare().isEnabled());assert.deepEqual(await page.evaluate(()=>window.__repositoryQa.calls[0]),{type:'inspect-repository',directoryGrant:'grant-a'});checks.push('explicit-keyboard-read-native-grant-before-preparation');
 await scenario('dirty');await inspect().click();await region().getByText('Final application is not eligible in this checkout. A compatible worker may still run; Core revalidates the target before any application.',{exact:true}).waitFor();assert(await prepare().isEnabled());checks.push('application-ineligible-does-not-block-compatible-worker');
 await region().getByRole('button',{name:'Git status',exact:true}).click();await region().getByText('Deterministic local query — no model called',{exact:true}).waitFor();assert.match(await region().locator('.o-repository-status').last().innerText(),/src\/example\.ts/);assert(await prepare().isEnabled());checks.push('git-status-remains-a-separate-local-query');
 await scenario('blocked');await inspect().click();await region().getByText('Repository blocked for this profile',{exact:true}).waitFor();assert(await prepare().isDisabled());await region().getByText('Final application is blocked for this repository profile.',{exact:true}).waitFor();assert.equal(await region().getByText('Final application is not eligible in this checkout. A compatible worker may still run; Core revalidates the target before any application.',{exact:true}).count(),0);await region().getByText('Binary content (1)',{exact:true}).waitFor();await region().getByText('future-unrecognized-blocker-code',{exact:true}).waitFor();checks.push('blocked-inventory-visible-no-fabricated-reason');
 await scenario('partial');await inspect().click();await region().getByText('Inspection incomplete',{exact:true}).waitFor();assert(await prepare().isDisabled());
 await scenario('compatible');await inspect().click();await prepare().waitFor({state:'visible'});assert(await prepare().isEnabled());await scenario('fail');await inspect().click();await region().getByRole('alert').waitFor();assert(await prepare().isDisabled());await region().getByText('Previous inspection: Compatible with the worker profile',{exact:true}).waitFor();assert.equal(await page.getByRole('textbox',{name:'Objective',exact:true}).inputValue(),'Preserve this draft while inspecting');checks.push('partial-and-failed-inspection-never-release-preparation','failure-retains-observation-and-draft');
 await scenario('compatible');await page.evaluate(()=>window.__repositoryQa.defer());await inspect().click();await page.waitForFunction(()=>window.__repositoryQa.pending());await page.getByRole('textbox',{name:'Objective',exact:true}).fill('A changed draft survives the pending read');await page.evaluate(()=>window.__repositoryQa.grant('grant-b'));await inspect().waitFor({state:'visible'});await page.evaluate(()=>window.__repositoryQa.release());await page.waitForTimeout(50);assert.equal(await region().getByText('Compatible with the worker profile',{exact:true}).count(),0);assert(await prepare().isDisabled());await inspect().click();await region().getByText('b'.repeat(40),{exact:true}).first().waitFor();assert(await prepare().isEnabled());assert.equal(await page.getByRole('textbox',{name:'Objective',exact:true}).inputValue(),'A changed draft survives the pending read');checks.push('late-old-grant-response-rejected-with-draft-preserved');
 await scenario('wrong-shape');await inspect().click();await region().getByRole('alert').waitFor();assert(await prepare().isDisabled());await scenario('compatible');await inspect().click();await region().getByRole('alert').waitFor({state:'hidden'});await prepare().click();await page.getByRole('region',{name:'Review prepared request',exact:true}).waitFor();const call=await page.evaluate(()=>window.__repositoryQa.calls.at(-1));assert.equal(call.type,'prepare-task');assert.equal(call.draft.repositoryGrant,'grant-b');assert.equal(call.draft.objective,'A changed draft survives the pending read');checks.push('incompatible-response-cannot-grant-readiness','prepared-draft-retains-the-exact-selected-grant');
 await scenario('truncated-status');await region().getByRole('button',{name:'Git status',exact:true}).click();await region().getByText('Status output was limited. This is not a complete inventory.',{exact:true}).waitFor();await scenario('blocked');await inspect().click();await region().getByText('Repository blocked for this profile',{exact:true}).waitFor();
 await scenario('compatible');await page.getByRole('checkbox',{name:'Preserve complete base with an explicit editable file list',exact:true}).check();assert(await prepare().isDisabled());await inspect().click();await region().getByText('Select editable files to complete compatibility',{exact:true}).waitFor();assert(await prepare().isDisabled());await page.getByRole('textbox',{name:'Editable files (one repository-relative path per line)',exact:true}).fill('docs/guide.md');assert(await prepare().isDisabled());await inspect().click();await region().getByText('Compatible with the opaque-base profile',{exact:true}).waitFor();assert(await prepare().isEnabled());await region().getByText('Preserved bytes are not included in context or review automatically. Runtime tools may still read files materialized in the worktree; this is not a read sandbox.',{exact:true}).waitFor();assert.deepEqual(await page.evaluate(()=>window.__repositoryQa.calls.at(-1).editablePaths),['docs/guide.md']);await page.getByRole('textbox',{name:'Editable files (one repository-relative path per line)',exact:true}).fill('docs/other.md');assert(await prepare().isDisabled());await inspect().click();assert(await prepare().isEnabled());checks.push('opaque-profile-selection-required-and-scope-change-withdraws-readiness','preservation-counts-and-runtime-read-boundary-visible');
 const geometry=[];
 for(const language of ['en','pt','es'])for(const scale of [100,200])for(const width of [1440,900]){
  await page.setViewportSize({width,height:width===900?680:900});
  await page.evaluate(({language,scale})=>window.__repositoryQa.language(language,scale),{language,scale});
  const view=page.getByRole('region',{name:t('Repository inspection',language),exact:true});await view.waitFor();
  const detail=view.getByText(t('Inspection details and limits',language),{exact:true});
  await detail.focus();await page.keyboard.press('Enter');
  const bounds=await shownGeometry(view),context=JSON.stringify({language,scale,requestedWidth:width,bounds});
  assert(bounds.scrollWidth<=bounds.width+1,context);
  assert(bounds.scrollX===0&&bounds.documentWidth<=width+1&&bounds.left>=0&&bounds.right<=width+1,context);
  assert(bounds.top>=-1&&bounds.top<bounds.viewportHeight&&bounds.bottom>0&&bounds.visibleTarget,context);
  const notice=await view.locator('.o-base-preservation').evaluate(element=>{const title=element.querySelector('strong').getBoundingClientRect(),p=element.querySelector('p').getBoundingClientRect();return {titleBottom:title.bottom,firstTop:p.top};});
  assert(notice.firstTop>=notice.titleBottom,JSON.stringify({language,scale,width,notice}));
  assert.equal(await page.evaluate(()=>getComputedStyle(document.documentElement).fontSize),`${14*scale/100}px`);
  const privacyNotice=view.getByText(t(privacy,language),{exact:true});await privacyNotice.waitFor();
  await detail.focus();assert(await detail.evaluate(element=>element===document.activeElement));
  await shownGeometry(view);await page.screenshot({path:`${output}${language}-${scale}-${width}.png`});
  if(width===900&&scale===200){
   await privacyNotice.evaluate(element=>element.scrollIntoView({block:'center',inline:'nearest',behavior:'instant'}));
   await page.screenshot({path:`${output}${language}-${scale}-${width}-privacy.png`});
   const privacyBounds=await privacyNotice.boundingBox();
   assert(privacyBounds.x>=0&&privacyBounds.x+privacyBounds.width<=width+1&&privacyBounds.y>=0&&privacyBounds.y+privacyBounds.height<=bounds.viewportHeight+1,JSON.stringify({language,privacyBounds}));
  }
  await detail.focus();await page.keyboard.press('Enter');geometry.push({language,scale,requestedWidth:width,...bounds});
 }
 const calls=await page.evaluate(()=>window.__repositoryQa.calls);assert(calls.every(call=>['inspect-repository','git-status','prepare-task'].includes(call.type)));assert.equal(calls.filter(call=>call.type==='prepare-task').length,1);assert.deepEqual(errors,[]);checks.push('three-languages-two-scales-two-widths-keyboard-reflow','no-automatic-provider-authentication-or-worker-effects');await writeFile(`${output}result.json`,JSON.stringify({checks,geometry,calls,errors,native:false,persistence:false,providerRead:false},null,2));console.log(`PASS ${checks.length} repository inspection renderer groups; screenshots ${output}`);
}finally{await browser.close();}
