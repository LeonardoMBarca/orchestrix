// Real React/CSS, explicitly synthetic local facts; not native or provider proof.
import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {mkdir,writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {capacityTranslations} from '../src/capacity-locales.ts';
import {diagnosticHistoryTranslations} from '../src/diagnostic-history-locales.ts';
const require=createRequire(new URL('../../../prototypes/desktop/package.json',import.meta.url)),{chromium}=require('playwright');
const browser=await chromium.launch({channel:'chrome',headless:true}),page=await browser.newPage({viewport:{width:1440,height:900}}),errors=[],checks=[];
page.on('pageerror',error=>errors.push(error.message));
const output=fileURLToPath(new URL(`../../../artifacts/capacity-renderer/${Date.now()}/`,import.meta.url));await mkdir(output,{recursive:true});
const t=(key,language)=>language==='en'?key:(capacityTranslations[key]??diagnosticHistoryTranslations[key])?.[language==='pt'?0:1]??key;
const current=()=>page.locator('#capacity-fixture>.o-capacity').first();
const scenario=async name=>{await page.evaluate(name=>window.__capacityQa.scenario(name),name);};
try{
 await page.goto(`${process.env.ORCHESTRIX_RENDERER_URL??'http://127.0.0.1:1428'}/tests/capacity.fixture.html`);await current().getByText('Included usage allowed',{exact:true}).waitFor();
 assert.equal(await current().locator('progress:visible').count(),2);assert.deepEqual(await current().locator('progress:visible').evaluateAll(elements=>elements.map(element=>element.value)),[0,72]);
 await current().getByText('0%',{exact:true}).first().waitFor();await current().getByText('100%',{exact:true}).first().waitFor();
 assert.equal(await page.evaluate(()=>window.__capacityQa.calls.length),0);checks.push('zero-is-observed-two-windows-no-extra-read');

 await scenario('denied');await current().getByText('Included usage unavailable',{exact:true}).waitFor();await current().getByText('The provider does not currently allow ordinary included usage. This does not identify which window caused the restriction. Local controls remain available; no paid API fallback is used.',{exact:true}).waitFor();
 await scenario('no-quantities');await current().getByText('Included usage allowed',{exact:true}).waitFor();assert.equal(await current().locator('progress:visible').count(),0);await current().getByText('Quantitative windows were not reported. Usage permission is a separate fact.',{exact:true}).waitFor();
 await scenario('null');await current().getByText('Usage permission unknown',{exact:true}).waitFor();
 await scenario('invalid');await current().getByText('120%',{exact:true}).waitFor();assert.equal(await current().locator('progress:visible').count(),1);await current().getByText('The reported percentage is outside the usable range. Its original value is preserved without a bar or derived balance.',{exact:true}).waitFor();
 checks.push('denial-without-cause-invention','permission-only-null-and-out-of-range-are-distinct');

 await scenario('failed');await current().getByText('Included usage allowed',{exact:true}).waitFor();await current().getByText('The latest read did not update capacity. The previous observation keeps its original date and values.',{exact:true}).waitFor();await current().getByText('0%',{exact:true}).first().waitFor();
 await scenario('reset');await current().getByText('Historical capacity observation',{exact:true}).waitFor();await current().getByText('The last observed permission denied included usage. New work remains blocked until a new authorized read explicitly allows it.',{exact:true}).waitFor();assert.equal(await current().getByText('The reported renewal time has passed. Restoration is not confirmed; recheck when permitted.',{exact:true}).count(),3);
 await scenario('legacy');await current().getByText('Capacity not reported',{exact:true}).waitFor();assert.equal(await current().locator('progress:visible').count(),0);
 checks.push('failed-read-keeps-old-values','reset-does-not-restore','legacy-stays-unknown');

 await scenario('normal');await current().getByText('Included usage allowed',{exact:true}).waitFor();const capacityDetails=current().getByText('Capacity evidence details',{exact:true});await capacityDetails.focus();await page.keyboard.press('Enter');await current().getByText('No shared capacity group was established',{exact:true}).waitFor();assert.equal(await current().getByText('Legacy view of the same read',{exact:true}).count(),1);
 const history=page.getByRole('region',{name:'Diagnostic history',exact:true});await history.getByRole('button',{name:/^Diagnostic history/}).click();await history.getByRole('button',{name:/Read completed/}).click();const detail=page.getByRole('region',{name:'Historical observation',exact:true});await detail.getByText('Historical capacity only. This record cannot authorize work or confirm a renewal.',{exact:true}).waitFor();assert.equal(await detail.getByText('Included usage allowed',{exact:true}).count(),0);checks.push('historical-capacity-original-read-no-authority','keyboard-details-and-legacy-no-double-count');

 await page.evaluate(()=>window.__capacityQa.task());await page.getByText('Prepared capacity evidence',{exact:true}).click();const prepared=page.locator('.o-task-authorization .o-capacity');await prepared.getByText('Included usage allowed',{exact:true}).waitFor();await prepared.getByText('Capacity evidence details',{exact:true}).click();await prepared.getByText('synthetic-capacity-diagnostic',{exact:true}).waitFor();await page.getByText('Connection observation',{exact:true}).click();await page.locator('.o-task-fields .o-capacity').getByText('Included usage allowed',{exact:true}).waitFor();checks.push('selected-connection-and-prepared-evidence-separated');

 const geometry=[];
 for(const language of ['en','pt','es'])for(const scale of [100,200])for(const width of [1440,900]){
  await page.setViewportSize({width,height:width===900?680:900});await page.evaluate(({language,scale})=>window.__capacityQa.language(language,scale),{language,scale});await current().getByRole('heading',{name:t('Subscription capacity',language),exact:true}).waitFor();
  await current().evaluate(element=>element.scrollIntoView({block:'start'}));
  const bounds=await page.locator('.o-capacity').evaluateAll(elements=>elements.filter(element=>element.getClientRects().length>0).map(element=>({width:element.clientWidth,scrollWidth:element.scrollWidth})));
  assert(bounds.every(value=>value.scrollWidth<=value.width+1),`reflow without clipping: ${JSON.stringify({language,scale,width,bounds})}`);assert.equal(await page.evaluate(()=>getComputedStyle(document.documentElement).fontSize),`${14*scale/100}px`);
  const evidence=current().getByText(t('Capacity evidence details',language),{exact:true});await evidence.focus();assert(await evidence.evaluate(element=>element===document.activeElement));
  const legacy=current().getByText(t('Legacy view of the same read',language),{exact:true});await legacy.scrollIntoViewIfNeeded();await legacy.focus();await page.keyboard.press('Enter');assert.equal(await current().locator('progress:visible').count(),4,'legacy shown separately, not summed');await page.keyboard.press('Enter');assert.equal(await current().locator('progress:visible').count(),2);
  await current().evaluate(element=>element.scrollIntoView({block:'start'}));await page.screenshot({path:`${output}${language}-${scale}-${width}-summary.png`});await current().locator('.o-capacity-window').first().evaluate(element=>element.scrollIntoView({block:'start'}));await page.screenshot({path:`${output}${language}-${scale}-${width}-window.png`});
  geometry.push({language,scale,width,bounds});
 }
 const calls=await page.evaluate(()=>window.__capacityQa.calls);assert(calls.every(call=>['connection-diagnostic-history','connection-diagnostic-observation'].includes(call.type)));assert.deepEqual(errors,[]);checks.push('three-languages-two-scales-two-widths-keyboard-reflow','no-provider-inference-worker-or-authorization-effects');
 await writeFile(`${output}result.json`,JSON.stringify({checks,geometry,calls,errors,native:false,persistence:false,providerRead:false},null,2));console.log(`PASS ${checks.length} capacity renderer groups; screenshots ${output}`);
}finally{await browser.close();}
