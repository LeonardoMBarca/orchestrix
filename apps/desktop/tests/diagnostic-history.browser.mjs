// Renderer-only regression: real React/CSS, synthetic local API responses.
// This does not prove SQLite retention, daemon pagination or native dialogs.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdir,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {diagnosticHistoryTranslations} from '../src/diagnostic-history-locales.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url));
const {chromium}=require('playwright');
const browser=await chromium.launch({channel:'chrome',headless:true}),page=await browser.newPage({viewport:{width:1440,height:900}}),errors=[],checks=[];
page.on('pageerror',error=>errors.push(error.message));
const output=fileURLToPath(new URL(`../../../artifacts/diagnostic-history-renderer/${Date.now()}/`,import.meta.url));await mkdir(output,{recursive:true});
const t=(key,language)=>language==='en'?key:diagnosticHistoryTranslations[key]?.[language==='pt'?0:1]??key;
const history=()=>page.getByRole('region',{name:'Diagnostic history',exact:true}),items=()=>page.locator('.o-history-observation');
try{
 await page.goto(`${process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1421'}/tests/diagnostic-history.fixture.html`);
 const toggle=history().getByRole('button',{name:/^Diagnostic history/});await toggle.waitFor();
 assert.equal(await page.evaluate(()=>window.__historyQa.calls.length),0,'opening the surrounding screen performs no history/provider query');
 await toggle.focus();await page.keyboard.press('Space');await items().first().waitFor();
 assert.equal(await items().count(),20);assert.match(await items().first().innerText(),/#600/);
 assert.equal(await page.evaluate(()=>window.__historyQa.calls[0].limit),20);
 checks.push('lazy-keyboard-open-bounded-page');

 await items().first().click();await page.getByRole('region',{name:'Historical observation',exact:true}).waitFor();
 await page.getByText('Historical evidence only. This record does not establish current identity, access, validity or execution permission.',{exact:true}).waitFor();
 assert.equal(await page.getByText('large-descriptor-canary',{exact:true}).count(),0,'large descriptors are not dumped into the view');
 const selected=await page.evaluate(()=>window.__historyQa.calls.at(-1));assert.equal(selected.type,'connection-diagnostic-observation');assert.equal(selected.diagnosticId,'synthetic-round-300');assert.equal(selected.ordinal,2);
 await page.getByText('Evidence identifiers',{exact:true}).click();await page.getByText('synthetic-round-300',{exact:true}).waitFor();
 checks.push('exact-diagnostic-transition-detail-no-current-authority');

 await history().getByRole('button',{name:'Older observations',exact:true}).click();await page.getByText('Page 2',{exact:true}).waitFor();assert.equal(await items().count(),20);assert.match(await items().first().innerText(),/#580/);
 await page.evaluate(()=>window.__historyQa.arrive());
 await history().getByRole('button',{name:'Newer observations',exact:true}).click();await page.getByText('Page 1',{exact:true}).waitFor();assert.match(await items().first().innerText(),/#600/,'cursor retains the original high-water mark after arrivals');
 await page.getByText('synthetic-round-300',{exact:true}).waitFor();
 const beforePassive=await page.evaluate(()=>window.__historyQa.calls.length);await page.evaluate(()=>window.__historyQa.passive());await page.waitForTimeout(1100);assert.equal(await page.evaluate(()=>window.__historyQa.calls.length),beforePassive,'passive refresh never queries history again');
 const refresh=history().getByRole('button',{name:'Refresh history',exact:true});await refresh.click();await page.waitForFunction(()=>document.querySelector('.o-history-observation')?.textContent.includes('#601'));
 assert.equal(await refresh.evaluate(element=>element===document.activeElement),true,'manual refresh retains keyboard focus');await page.getByText('synthetic-round-300',{exact:true}).waitFor();
 checks.push('stable-high-water-arrivals-explicit-refresh-no-poll','selection-and-focus-preserved');

 await page.evaluate(()=>window.__historyQa.fail());await refresh.click();await page.getByRole('alert').filter({hasText:'History could not be loaded.'}).waitFor();assert.match(await items().first().innerText(),/#601/);await page.getByText('synthetic-round-300',{exact:true}).waitFor();
 await refresh.click();await page.getByRole('alert').waitFor({state:'hidden'});
 // Seek oldest by the normal bounded pager; no direct DB or state editing.
 let pages=1;while(await history().getByRole('button',{name:'Older observations',exact:true}).isEnabled()){
  await history().getByRole('button',{name:'Older observations',exact:true}).click();pages++;await page.getByText(`Page ${pages}`,{exact:true}).waitFor();assert(await items().count()<=20);
 }
 assert.equal(pages,31);assert.match(await items().last().innerText(),/#1/);assert.match(await items().last().innerText(),/Archived/);
 await items().last().click();await page.getByText('synthetic-round-1',{exact:true}).waitFor({state:'attached'});await page.getByText('Evidence identifiers',{exact:true}).click();await page.getByText('synthetic-round-1',{exact:true}).waitFor();const oldest=await page.evaluate(()=>window.__historyQa.calls.at(-1));assert.equal(oldest.diagnosticId,'synthetic-round-1');assert.equal(oldest.ordinal,1);
 await page.getByText('Captured validity limits',{exact:true}).click();await page.getByText('These original duration limits never renew the observation. Source deadlines and stricter historical constraints still apply.',{exact:true}).waitFor();
 checks.push('failed-page-preserves-page-and-detail','600-plus-records-oldest-exact-archived-inspection');
 await page.getByText('Find an exact diagnostic',{exact:true}).click();await page.getByRole('textbox',{name:'Diagnostic ID',exact:true}).fill('synthetic-round-200');await page.getByRole('spinbutton',{name:'Transition (optional)',exact:true}).fill('1');await page.getByRole('button',{name:'Inspect exact diagnostic',exact:true}).click();await page.waitForFunction(()=>document.querySelector('.o-history-detail')?.textContent.includes('synthetic-round-200'));
 assert.equal((await page.evaluate(()=>window.__historyQa.calls.at(-1))).ordinal,1);await page.getByRole('textbox',{name:'Diagnostic ID',exact:true}).fill('not-a-known-diagnostic');await page.getByRole('button',{name:'Inspect exact diagnostic',exact:true}).click();await page.getByRole('alert').filter({hasText:'This exact observation could not be verified.'}).waitFor();assert.equal(await page.locator('.o-history-detail').getByText('synthetic-round-200',{exact:true}).count(),1,'failed lookup preserves previous exact selection');
 await page.getByRole('textbox',{name:'Diagnostic ID',exact:true}).fill('synthetic-round-200');await page.getByRole('spinbutton',{name:'Transition (optional)',exact:true}).fill('');await page.getByRole('button',{name:'Inspect exact diagnostic',exact:true}).click();await page.getByRole('alert').waitFor({state:'hidden'});assert.equal((await page.evaluate(()=>window.__historyQa.calls.at(-1))).ordinal,undefined);await page.locator('.o-history-detail').getByText('Read completed',{exact:true}).waitFor();
 checks.push('explicit-diagnostic-id-lookup-optional-transition-and-miss-preserves-selection');

 // Revisions fence delayed local responses; they do not authenticate a binding.
 await page.evaluate(()=>window.__historyQa.defer());await refresh.click();await page.waitForFunction(()=>window.__historyQa.hasDeferred());await page.evaluate(()=>window.__historyQa.rebind());await history().getByRole('button',{name:/^Diagnostic history/}).waitFor();await page.evaluate(()=>window.__historyQa.release());await page.waitForTimeout(100);
 assert.equal(await history().locator('.o-history-list:visible').count(),0,'late previous-binding page cannot reopen or populate the new binding');
 await history().getByRole('button',{name:/^Diagnostic history/}).click();await items().first().waitFor();
 await page.evaluate(()=>window.__historyQa.corrupt(true));await items().first().click();await page.getByRole('alert').filter({hasText:'This exact observation could not be verified.'}).waitFor();assert.equal(await page.getByText('Evidence identifiers',{exact:true}).count(),0,'mismatched exact evidence is never displayed as verified');await page.evaluate(()=>window.__historyQa.corrupt(false));await items().first().click();await page.getByText('Evidence identifiers',{exact:true}).waitFor();
 await page.evaluate(()=>window.__historyQa.block());await refresh.click();await page.getByText('The historical storage budget blocks new reads. Existing history, cancellation and recovery remain available. Inspect the storage details before requesting another read.',{exact:true}).waitFor();
 assert.equal(await history().getByRole('button',{name:'Older observations',exact:true}).isEnabled(),true,'local inspection remains available after storage exhaustion');
 checks.push('late-binding-response-rejected','digest-mismatch-explicit-error','budget-block-does-not-block-history');
 await page.getByText('Evidence identifiers',{exact:true}).click();await page.getByText('a'.repeat(64),{exact:true}).waitFor();

 const geometry=[];
 for(const language of ['en','pt','es'])for(const scale of [100,200])for(const width of [1440,900]){
  await page.setViewportSize({width,height:width===900?620:900});await page.evaluate(({language,scale})=>window.__historyQa.language(language,scale),{language,scale});
  const region=page.getByRole('region',{name:t('Diagnostic history',language),exact:true});await region.waitFor();
  const older=region.getByRole('button',{name:t('Older observations',language),exact:true});await older.scrollIntoViewIfNeeded();await older.focus();assert.equal(await older.evaluate(element=>element===document.activeElement),true);
  const bounds=await region.evaluate(element=>({width:element.clientWidth,scrollWidth:element.scrollWidth,rootFont:getComputedStyle(document.documentElement).fontSize}));assert(bounds.scrollWidth<=bounds.width+1,`history reflows without horizontal clipping: ${JSON.stringify({language,scale,width,bounds})}`);assert.equal(bounds.rootFont,`${14*scale/100}px`);
  const detailBounds=await region.locator('.o-history-detail').evaluate(element=>({width:element.clientWidth,scrollWidth:element.scrollWidth}));assert(detailBounds.scrollWidth<=detailBounds.width+1,`long scope and evidence hashes reflow: ${JSON.stringify({language,scale,width,detailBounds})}`);
  geometry.push({language,scale,width,...bounds,detailBounds});if(width===900){await page.screenshot({path:`${output}${language}-${scale}-compact.png`});await region.locator('.o-history-storage').evaluate(element=>element.scrollIntoView({block:'start'}));await page.screenshot({path:`${output}${language}-${scale}-storage.png`});await region.locator('.o-history-detail details').last().evaluate(element=>element.scrollIntoView({block:'start'}));await page.screenshot({path:`${output}${language}-${scale}-identifiers.png`});}
 }
 const calls=await page.evaluate(()=>window.__historyQa.calls);assert(calls.every(call=>['connection-diagnostic-history','connection-diagnostic-observation'].includes(call.type)));assert.deepEqual(errors,[]);
 checks.push('three-languages-two-scales-two-widths-keyboard-reflow','no-metadata-inference-auth-or-worker-effects');
 await writeFile(`${output}result.json`,JSON.stringify({checks,geometry,requestCount:calls.length,errors,native:false,persistence:false,providerRead:false},null,2));
 console.log(`PASS ${checks.length} diagnostic history renderer groups; screenshots ${output}`);
}finally{await browser.close();}
