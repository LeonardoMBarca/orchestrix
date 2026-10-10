import {test,expect} from '@playwright/test';
import {openSettings} from './navigation.mjs';

const secret='synthetic-diagnostics-credential-private';
const token='d'.repeat(64);
const connection=page=>page.evaluate(()=>OrchestrixApp.getState().connections.at(-1));
function catalog(request,{id='diagnostic-model',name='Diagnostic model',status='complete',access=false}={}) {
  return {schemaVersion:1,status,scope:{connectionId:request.connectionId,provider:request.provider,endpoint:request.endpoint,region:request.region,deployment:request.deployment,authMethod:request.authMethod},
    source:{kind:'provider-metadata',operation:'models.list'},observedAt:'2026-10-10T19:10:00.000Z',
    models:[{id,name,source:{kind:'provider-metadata',operation:'models.list'},...(access?{access:{status:'supported',observed:true}}:{}),capabilities:{
      input:{status:'supported',observed:true,modalities:['text']},output:{status:'supported',observed:true,modalities:['text']},
      reasoning:{status:'supported',observed:true,levels:['low','high']},context:{status:'supported',observed:true,inputTokens:128000,outputTokens:8192},
      functions:{status:'unsupported',observed:true},streaming:{status:'supported',observed:true},nativeWeb:{status:'unknown',observed:false}
    }}],limitations:status==='partial'?[{code:'page-limit-reached'}]:[]};
}
async function transport(page,respond) {
  const requests=[];
  await page.route('**/api/discovery/session',route=>route.fulfill({contentType:'application/json',body:JSON.stringify({protocolVersion:1,token,readOnly:true})}));
  await page.route('**/api/discovery/catalog',async route=>{
    const body=route.request().postDataJSON();requests.push(body);
    const response=await respond(body,requests.length);
    try{await route.fulfill({status:response.status||200,contentType:'application/json',body:JSON.stringify(response.body)});}catch(error){if(!route.request().isNavigationRequest()&&!/closed|cancel|aborted|Invalid InterceptionId/i.test(error.message))throw error;}
  });
  return requests;
}
async function register(page,{name='Diagnostics API',key=secret,fromChat=false}={}) {
  if(!fromChat)await page.locator('[data-view="connections"]').click();
  await openSettings(page,'accounts');await page.locator('[data-settings-add-account]').click();
  await page.locator('#conn-kind-form [name="kind"][value="api"]').check();await page.locator('#conn-kind-form button[type="submit"]').click();
  await page.locator('#conn-api-provider').selectOption('openai');await page.locator('#conn-name').fill(name);await page.locator('#conn-api-key').fill(key);await page.locator('[name="paidConsent"]').check();
  await page.locator('#conn-api-form button[type="submit"]').click();await expect(page.locator('#modal')).not.toBeVisible();
}
async function report(page,id) {await openSettings(page,'accounts');await page.locator(`[data-settings-diagnostics="${id}"]`).click();await expect(page.locator('#connection-diagnostics')).toBeVisible();return page.locator('#connection-diagnostics');}
async function privacy(page,keys=[secret]) {
  await page.evaluate(()=>OrchestrixApp.persistSessions());
  const surfaces=await page.evaluate(()=>({state:JSON.stringify(OrchestrixApp.getState()),storage:JSON.stringify({...localStorage}),dom:document.documentElement.outerHTML,passwords:[...document.querySelectorAll('input[type="password"]')].map(x=>x.value)}));
  for(const key of [...keys,token])for(const value of [surfaces.state,surfaces.storage,surfaces.dom,...surfaces.passwords])expect(value).not.toContain(key);
}

test('registration automatically reports real progress and keeps catalog support separate from verified access',async ({page})=>{
  let finish;const pending=new Promise(resolve=>finish=resolve);
  const requests=await transport(page,async request=>{await pending;return {body:catalog(request)};});await page.goto('/');await register(page);
  await expect.poll(()=>requests.length).toBe(1);
  await expect.poll(async()=> (await connection(page)).diagnostics.phase).toBe('running');
  expect(await page.evaluate(()=>typeof window.__ORCHESTRIX_TEST_DISCOVERY_ADAPTERS)).toBe('undefined');
  const current=await connection(page),running=await report(page,current.id);
  await expect(running).toContainText('Checking connection');await expect(running.getByRole('progressbar',{name:'Diagnostic progress'})).toBeVisible();
  await expect(running.locator('[data-diagnostic-check="catalog"]')).toHaveAttribute('data-check-status','pending');
  await page.screenshot({path:'artifacts/connection-diagnostics-running-desktop.png'});
  finish();await expect.poll(async()=> (await connection(page)).diagnostics.phase).toBe('completed');
  await expect(running).toContainText('Diagnostic complete');await expect(running.getByRole('progressbar')).toHaveCount(0);
  const c=await connection(page);expect(c.diagnostics.scope).toMatchObject({connectionId:c.id,provider:'openai',revision:c.identity.revision});expect(c.diagnostics.summary.totalChecks).toBe(20);
  expect(c.diagnostics.summary.resources).toEqual({available:0,unavailable:0,unknown:1});
  await expect(running.locator('[data-diagnostic-feature="reasoning"]').first()).toContainText('Supported');
  await expect(running.locator('[data-diagnostic-feature="reasoning"]').first()).toContainText('Not verified');
  await expect(running.locator('[data-diagnostic-feature="functionCalling"]').first()).toContainText('Unsupported');
  await expect(running.locator('[data-diagnostic-feature="functionCalling"]').first()).toContainText('Unavailable');
  await expect(running.locator('[data-diagnostic-feature="nativeMultiAgent"]').first()).toContainText('Not applicable');
  await expect(running.locator('[data-diagnostic-feature="fullAccess"]').first()).toContainText('Requires user permission');
  await expect(running).toContainText('Disabled by policy');expect(c.authorizationValid).toBe(false);expect(c.acceptsNewWork).toBe(false);
  await page.locator('#modal').evaluate(element=>element.scrollTop=0);await page.screenshot({path:'artifacts/connection-diagnostics-completed-desktop.png'});
  const model=running.locator('[data-diagnostic-model="diagnostic-model"]');await model.locator('summary').click();
  await expect(model.locator('[data-diagnostic-feature="context"]')).toContainText('128,000');await expect(model.locator('[data-diagnostic-feature="reasoning"]')).toContainText('low, high');
  await expect(model).toContainText('models.list');await model.scrollIntoViewIfNeeded();await page.screenshot({path:'artifacts/connection-diagnostics-model-details-desktop.png'});
  await privacy(page);expect(await page.evaluate(()=>OrchestrixI18n.missing())).toEqual([]);
});

test('a chat draft remains usable during a background diagnostic and receives a nonblocking notice',async ({page})=>{
  let finish;const pending=new Promise(resolve=>finish=resolve);const requests=await transport(page,async request=>{await pending;return {body:catalog(request)};});await page.goto('/');
  const composer=page.locator('#chat-message');await composer.fill('Keep this unsent draft while checking my account.');const session=await page.evaluate(()=>OrchestrixApp.getState().chat.id);
  await register(page,{fromChat:true});await expect.poll(()=>requests.length).toBe(1);await page.evaluate(()=>OrchestrixSettings.close());
  await expect(composer).toHaveValue('Keep this unsent draft while checking my account.');expect(await page.evaluate(()=>OrchestrixApp.getState().view)).toBe('chat');
  await composer.press('End');await composer.press('!');finish();await expect.poll(async()=> (await connection(page)).diagnostics.phase).toBe('completed');
  await expect(page.locator('#modal')).not.toBeVisible();await expect(page.locator('#connection-diagnostic-notice')).toContainText('Diagnostic complete');
  await expect(composer).toHaveValue('Keep this unsent draft while checking my account.!');expect(await page.evaluate(()=>OrchestrixApp.getState().chat.id)).toBe(session);
  await page.locator('#connection-diagnostic-notice [data-conn-action="dismiss-diagnostics"]').click();await expect(page.locator('#connection-diagnostic-notice')).toHaveCount(0);await privacy(page);
});

test('rechecking is scoped to the selected account and cancellation rejects late results',async ({page})=>{
  let finish,lateReturned=false;const pending=new Promise(resolve=>finish=resolve);
  const requests=await transport(page,async (request,index)=>{if(index===1)return {body:catalog(request)};await pending;lateReturned=true;return {body:catalog(request,{id:'late-model'})};});await page.goto('/');await register(page);
  await expect.poll(async()=> (await connection(page)).diagnostics.phase).toBe('completed');const before=await connection(page),view=await report(page,before.id);
  await view.locator('[data-conn-action="recheck-diagnostics"]').click();await expect.poll(()=>requests.length).toBe(2);
  await expect.poll(async()=> (await connection(page)).diagnostics.phase).toBe('running');expect(requests[1]).toMatchObject({connectionId:before.id,endpoint:before.endpoint,apiKey:secret});
  await view.locator('[data-conn-action="cancel-diagnostics"]').click();await expect(view).toContainText('Diagnostic cancelled');
  finish();await expect.poll(()=>lateReturned).toBe(true);await expect.poll(async()=> (await connection(page)).diagnostics.phase).toBe('cancelled');
  expect((await connection(page)).diagnostics.models).toEqual([]);await expect(view.locator('[data-diagnostic-model="late-model"]')).toHaveCount(0);await privacy(page);
});

test('two accounts at the same endpoint retain distinct diagnostic scopes and resource observations',async ({page})=>{
  const second='synthetic-other-diagnostics-credential-private';
  const requests=await transport(page,request=>({body:catalog(request,{id:request.apiKey===secret?'first-resource':'second-resource'})}));await page.goto('/');await register(page,{name:'First API'});
  await expect.poll(async()=> (await connection(page)).diagnostics.phase).toBe('completed');const first=await connection(page);await register(page,{name:'Second API',key:second});
  await expect.poll(async()=> (await connection(page)).diagnostics.phase).toBe('completed');const latest=await connection(page);
  expect(first.diagnostics.scopeKey).not.toBe(latest.diagnostics.scopeKey);expect(first.diagnostics.models.map(x=>x.id)).toEqual(['first-resource']);expect(latest.diagnostics.models.map(x=>x.id)).toEqual(['second-resource']);
  const firstView=await report(page,first.id);await expect(firstView.locator('[data-diagnostic-model="first-resource"]')).toHaveCount(1);await expect(firstView.locator('[data-diagnostic-model="second-resource"]')).toHaveCount(0);
  await page.locator('#modal [data-action="close-modal"]').click();const secondView=await report(page,latest.id);await expect(secondView.locator('[data-diagnostic-model="second-resource"]')).toHaveCount(1);
  expect(requests.map(x=>x.apiKey)).toEqual([secret,second]);await privacy(page,[secret,second]);
});

for(const failure of [{code:'authentication-required',status:401,message:'Confirm provider authentication'},{code:'access-denied',status:403,message:'This credential cannot list resources.'},{code:'rate-limited',status:429,message:'The provider rate-limited'}])test(`diagnostic explains ${failure.code} without granting access and supports retry`,async ({page})=>{
  const requests=await transport(page,(request,index)=>index===1?{status:failure.status,body:{error:{code:failure.code,message:`unsafe ${secret}`}}}:{body:catalog(request)});await page.goto('/');await register(page);
  await expect.poll(async()=> (await connection(page)).diagnostics.phase).toBe('completed');const c=await connection(page),view=await report(page,c.id);
  await view.locator('.diagnostic-checklist summary').click();await expect(view.locator('[data-diagnostic-check="catalog"]')).toHaveAttribute('data-check-status','failed');await expect(view.locator('[data-diagnostic-check="catalog"]')).toContainText(failure.message);
  expect(c.diagnostics.models).toEqual([]);expect(c.authorizationValid).toBe(false);await privacy(page);
  await view.locator('[data-conn-action="recheck-diagnostics"]').click();await expect.poll(async()=> (await connection(page)).discovery.status).toBe('observed');await expect(view.locator('[data-diagnostic-model="diagnostic-model"]')).toHaveCount(1);expect(requests).toHaveLength(2);await privacy(page);
});

test('runtime configuration reports pending authentication instead of invented resources or permissions',async ({page})=>{
  await page.goto('/');await page.locator('[data-view="connections"]').click();await openSettings(page,'accounts');await page.locator('[data-settings-add-account]').click();await page.locator('#conn-kind-form button[type="submit"]').click();
  await page.locator('#conn-runtime').selectOption('Codex');await page.locator('#conn-name').fill('Personal Codex');await page.locator('#conn-account-form button[type="submit"]').click();await expect(page.locator('#modal')).not.toBeVisible();
  await expect.poll(async()=> (await connection(page)).diagnostics.phase).toBe('completed');const c=await connection(page),view=await report(page,c.id);
  await view.locator('.diagnostic-checklist summary').click();await expect(view.locator('[data-diagnostic-check="identity"]')).toHaveAttribute('data-check-status','blocked');await expect(view.locator('[data-diagnostic-check="identity"]')).toContainText('authentication have not been confirmed');
  expect(c.diagnostics.models).toEqual([]);await expect(view.locator('[data-diagnostic-feature="usage"]').first()).toContainText('Not verified');await expect(view.locator('[data-diagnostic-feature="fullAccess"]').first()).toContainText('Requires user permission');await expect(view.getByRole('progressbar')).toHaveCount(0);expect(c.authorizationValid).toBe(false);
});

test('scoped quota and API credit observations are displayed without turning missing access into availability',async ({page})=>{
  await transport(page,request=>{const result=catalog(request);result.features={
    usage:{status:'unknown',observed:true,details:{usedPercent:34,remainingPercent:66,resetAt:'2026-10-11T02:30:00.000Z'}},
    credits:{status:'unknown',observed:true,details:{credit:{balance:18.75,currency:'USD'}}}
  };return {body:result};});await page.goto('/');await register(page);await expect.poll(async()=> (await connection(page)).diagnostics.phase).toBe('completed');
  const c=await connection(page),view=await report(page,c.id),usage=view.locator('[data-diagnostic-feature="usage"]'),credits=view.locator('[data-diagnostic-feature="credits"]');
  expect(c.diagnostics.features.usage).toMatchObject({support:'unknown',available:'unknown',observed:true,details:{usedPercent:34,remainingPercent:66}});
  await expect(usage).toContainText('34%');await expect(usage).toContainText('66%');await expect(usage).toContainText('Quota reset');await expect(usage).toContainText('Not verified');
  await expect(credits).toContainText('18.75');await expect(credits).toContainText('USD');await expect(credits).toContainText('Credits reported by the source');await expect(view.getByRole('progressbar')).toHaveCount(0);expect(c.authorizationValid).toBe(false);await privacy(page);
});

test('reports translate in EN/PT/ES and reflow at 200% on a narrow viewport',async ({page})=>{
  await transport(page,request=>({body:catalog(request,{status:'partial'})}));await page.goto('/');await register(page);await expect.poll(async()=> (await connection(page)).diagnostics.phase).toBe('completed');const c=await connection(page);
  for(const [language,title] of [['en','Connection diagnostics'],['pt-BR','Diagnóstico da conexão'],['es','Diagnóstico de la conexión']]){
    await page.evaluate(id=>OrchestrixI18n.setLanguage(id),language);const view=await report(page,c.id);await expect(view.getByRole('heading',{name:title,exact:true})).toBeVisible();
    expect((await connection(page)).diagnostics.scopeKey).toBe(c.diagnostics.scopeKey);expect(await page.evaluate(()=>OrchestrixI18n.missing())).toEqual([]);await page.locator('#modal [data-action="close-modal"]').click();
  }
  await page.evaluate(()=>OrchestrixI18n.setLanguage('en'));await page.setViewportSize({width:390,height:844});await openSettings(page,'general');
  await page.locator('#settings-text-size').focus();await page.locator('#settings-text-size').press('End');await page.evaluate(()=>OrchestrixSettings.close());const view=await report(page,c.id);
  await expect(view).toContainText('Diagnostic complete');const bounds=await view.evaluate(element=>({width:element.clientWidth,scroll:element.scrollWidth,page:document.documentElement.scrollWidth,viewport:innerWidth}));expect(bounds.scroll).toBeLessThanOrEqual(bounds.width+1);expect(bounds.page).toBeLessThanOrEqual(bounds.viewport+1);
  await page.locator('#modal').evaluate(element=>element.scrollTop=0);await page.screenshot({path:'artifacts/connection-diagnostics-mobile-200.png'});await privacy(page);
});
