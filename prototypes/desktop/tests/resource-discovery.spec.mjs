import {test,expect} from '@playwright/test';
import {openSettings} from './navigation.mjs';

const apiKey='synthetic-browser-discovery-private-key';
const sessionToken='a'.repeat(64);
const unknown={status:'unknown',observed:false};
const catalog=({status='complete',id='research-model',name='Research model',limitations=[]}={})=>({
  schemaVersion:1,status,observedAt:'2026-10-10T18:30:00.000Z',pages:1,
  source:{kind:'provider-metadata',operation:'models.list'},limitations,
  models:[{id,name,compatibility:{status:'supported',reason:'Provider metadata reports text input and output; execution is unverified.'},
    source:{kind:'provider-metadata',operation:'models.list'},
    capabilities:{
      input:{status:'supported',observed:true,modalities:['text']},
      output:{status:'supported',observed:true,modalities:['text']},
      reasoning:{status:'supported',observed:true,levels:['low','high']},
      context:{status:'supported',observed:true,inputTokens:128000,outputTokens:8192},
      functions:{status:'supported',observed:true},nativeWeb:{...unknown},hostTools:{...unknown}
    }}]
});
const currentConnection=page=>page.evaluate(()=>OrchestrixApp.getState().connections.at(-1));

test('models and inference profiles with the same ID remain distinct catalog resources',async ({page})=>{
  const response=catalog({id:'shared-resource'});
  response.models[0].kind='model';
  response.models.push({...response.models[0],name:'Regional inference profile',kind:'inference-profile'});
  await mockDiscovery(page,[{body:response}]);await page.goto('/');await registerAPI(page);
  await expect.poll(async()=> (await currentConnection(page)).discovery.status).toBe('observed');
  expect((await currentConnection(page)).catalog.map(model=>[model.kind,model.id])).toEqual([['model','shared-resource'],['inference-profile','shared-resource']]);
  const resources=await inspectResources(page,(await currentConnection(page)).id);
  await expect(resources.locator('[data-resource-model="shared-resource"]')).toHaveCount(2);
  await assertCredentialPrivacy(page);
});

async function mockDiscovery(page,responses) {
  const sessions=[],requests=[];
  await page.route('**/api/discovery/session',async route=>{
    sessions.push({method:route.request().method()});
    await route.fulfill({status:200,contentType:'application/json',body:JSON.stringify({protocolVersion:1,token:sessionToken,readOnly:true})});
  });
  await page.route('**/api/discovery/catalog',async route=>{
    requests.push({method:route.request().method(),headers:route.request().headers(),body:route.request().postDataJSON()});
    const response=await (typeof responses==='function'?responses(requests.at(-1),requests.length):responses[Math.min(requests.length-1,responses.length-1)]);
    await route.fulfill({status:response.status||200,contentType:'application/json',body:JSON.stringify(response.body)});
  });
  return {sessions,requests};
}

async function registerAPI(page,{name='Research API',provider='openai',key=apiKey,endpoint='',deployment=''}={}) {
  await page.locator('[data-view="connections"]').click();
  await page.locator('#content [data-conn-action="add"]').click();
  await page.locator('#conn-kind-form [name="kind"][value="api"]').check();
  await page.locator('#conn-kind-form button[type="submit"]').click();
  await page.locator('#conn-api-provider').selectOption(provider);
  await page.locator('#conn-name').fill(name);
  if(endpoint)await page.locator('#conn-api-endpoint').fill(endpoint);
  if(deployment)await page.locator('#conn-api-deployment').fill(deployment);
  await page.locator('#conn-api-key').fill(key);
  await page.locator('[name="paidConsent"]').check();
  await page.locator('#conn-api-form button[type="submit"]').click();
  await expect(page.locator('#modal')).not.toBeVisible();
}

async function inspectResources(page,id) {
  await openSettings(page,'accounts');
  await page.locator(`[data-settings-resources="${id}"]`).click();
  await expect(page.locator('#connection-resources')).toBeVisible();
  return page.locator('#connection-resources');
}

async function assertCredentialPrivacy(page,keys=[apiKey]) {
  await page.evaluate(()=>OrchestrixApp.persistSessions());
  const surfaces=await page.evaluate(()=>({state:JSON.stringify(OrchestrixApp.getState()),storage:JSON.stringify({...localStorage}),
    dom:document.documentElement.outerHTML,passwordValues:[...document.querySelectorAll('input[type="password"]')].map(input=>input.value)}));
  for(const key of [...keys,sessionToken])for(const value of [surfaces.state,surfaces.storage,surfaces.dom,...surfaces.passwordValues])expect(value).not.toContain(key);
}

test('saving an API automatically performs the HTTP discovery flow and exposes rich resources from Settings',async ({page})=>{
  let finish;const pending=new Promise(resolve=>{finish=resolve;});
  const transport=await mockDiscovery(page,[pending]);await page.goto('/');
  expect(await page.evaluate(()=>typeof window.__ORCHESTRIX_TEST_DISCOVERY_ADAPTERS)).toBe('undefined');
  expect(await page.evaluate(()=>typeof OrchestrixConnections.setDiscoveryAdapter)).toBe('undefined');
  await registerAPI(page);
  await expect.poll(()=>transport.requests.length).toBe(1);
  expect(transport.sessions).toEqual([{method:'GET'}]);
  expect(transport.requests[0]).toMatchObject({method:'POST',headers:{'x-orchestrix-discovery-token':sessionToken},
    body:{provider:'openai',endpoint:'https://api.openai.com/v1',authMethod:'api-key',apiKey}});
  expect(await currentConnection(page)).toMatchObject({catalogLoading:true,discovery:{status:'loading'},authorizationValid:false,acceptsNewWork:false});
  await assertCredentialPrivacy(page);
  finish({body:catalog()});
  await expect.poll(async()=> (await currentConnection(page)).discovery.status).toBe('observed');
  const connection=await currentConnection(page);
  expect(connection.catalog[0]).toMatchObject({thinking:['low','high'],capabilities:{context:{inputTokens:128000},functions:{status:'supported'},nativeWeb:unknown}});
  expect(connection.authorizationValid).toBe(false);expect(connection.acceptsNewWork).toBe(false);
  expect(await page.evaluate(()=>OrchestrixConnections.eligible(OrchestrixApp.getState().connections.at(-1)))).toBe(false);
  expect(await page.evaluate(()=>OrchestrixApp.getState().tasks)).toEqual([]);
  const resources=await inspectResources(page,connection.id);await expect(resources).toContainText('Catalog updated');
  const model=resources.locator('[data-resource-model="research-model"]');await model.locator('summary').click();
  await expect(model.locator('[data-capability="reasoning"]')).toContainText('low, high');
  await expect(model.locator('[data-capability="context"]')).toContainText('128,000');
  await expect(model.locator('[data-capability="context"]')).toContainText('8,192');
  await expect(model.locator('[data-capability="functions"] .resource-value')).toHaveClass(/resource-supported/);
  await expect(model.locator('[data-capability="nativeWeb"]')).toContainText('Not reported');
  await expect(model.locator('[data-capability="hostTools"] .resource-value')).toHaveClass(/resource-unknown/);
  await expect(model).toContainText('models.list');
  await expect(resources.locator('progress')).toHaveCount(0);
  await assertCredentialPrivacy(page);
  await resources.evaluate(element=>element.scrollIntoView({block:'start'}));
  await page.screenshot({path:'artifacts/api-resources-observed-desktop.png'});
  await page.getByRole('button',{name:'Close',exact:true}).click();
  await page.setViewportSize({width:390,height:844});
  await page.evaluate(()=>{OrchestrixApp.getUI().textScale=200;OrchestrixApp.saveUI();OrchestrixSettings.close();});
  await openSettings(page,'accounts');
  await expect(page.locator('[data-settings-resources]')).toBeVisible();
  await page.screenshot({path:'artifacts/api-resources-accounts-mobile-200.png'});
});

test('partial catalogs preserve known metadata and disclose pagination limitations and unknown capabilities',async ({page})=>{
  const result=catalog({status:'partial',limitations:[{code:'page-limit-reached'}]});
  result.models.push({id:'catalog-only',name:'Catalog only',compatibility:{status:'unknown'},capabilities:{reasoning:{...unknown},context:{...unknown},functions:{...unknown},nativeWeb:{...unknown}}});
  await mockDiscovery(page,[{body:result}]);await page.goto('/');await registerAPI(page);
  await expect.poll(async()=> (await currentConnection(page)).discovery.status).toBe('partial');
  const connection=await currentConnection(page),resources=await inspectResources(page,connection.id);
  expect(connection.discovery).toMatchObject({pages:1,source:{operation:'models.list'},limitations:[{code:'page-limit-reached'}]});
  await expect(resources).toContainText('Partial catalog');await expect(resources.locator('.resource-limitation')).toContainText('page-limit-reached');
  await expect(resources.locator('[data-resource-model]')).toHaveCount(2);
  const unknownModel=resources.locator('[data-resource-model="catalog-only"]');await unknownModel.locator('summary').click();
  for(const capability of ['reasoning','context','functions','nativeWeb']) {
    await expect(unknownModel.locator(`[data-capability="${capability}"]`)).toContainText('Not reported');
    await expect(unknownModel.locator(`[data-capability="${capability}"] .resource-value`)).toHaveClass(/resource-unknown/);
  }
  expect(connection.catalog[1].thinking).toEqual([]);expect(connection.catalog[1].compatibility.status).toBe('unknown');
  await assertCredentialPrivacy(page);
});

for(const error of [
  {code:'authentication-required',status:401,message:'Check the credential'},
  {code:'access-denied',status:403,message:'This credential cannot list resources.'},
  {code:'rate-limited',status:429,message:'The provider rate-limited'}
])test(`typed ${error.code} is explained and can be retried without echoing provider details`,async ({page})=>{
  const transport=await mockDiscovery(page,[{status:error.status,body:{error:{code:error.code,message:`Unsafe provider echo ${apiKey}`}}},{body:catalog()}]);
  await page.goto('/');await registerAPI(page);
  await expect.poll(async()=> (await currentConnection(page)).discovery.status).toBe(error.code);
  const connection=await currentConnection(page),resources=await inspectResources(page,connection.id);
  await expect(resources).toContainText(error.message);
  expect(connection.catalog).toEqual([]);expect(connection.authorizationValid).toBe(false);
  await assertCredentialPrivacy(page);
  await resources.getByRole('button',{name:'Check models',exact:true}).click();
  await expect.poll(async()=> (await currentConnection(page)).discovery.status).toBe('observed');
  await expect(resources).toContainText('Research model');expect(transport.requests).toHaveLength(2);expect(transport.sessions).toHaveLength(2);
  for(const request of transport.requests)expect(request.body).toMatchObject({apiKey,provider:'openai'});
  expect((await currentConnection(page)).authorizationValid).toBe(false);
  await assertCredentialPrivacy(page);
});

test('a credential echoed in non-model discovery metadata is rejected before UI or history',async ({page})=>{
  const result=catalog();result.source.operation=`unsafe-${apiKey}`;
  await mockDiscovery(page,[{body:result}]);await page.goto('/');await registerAPI(page);
  await expect.poll(async()=> (await currentConnection(page)).discovery.status).toBe('failed');
  const connection=await currentConnection(page);expect(connection.catalog).toEqual([]);
  const resources=await inspectResources(page,connection.id);await expect(resources.locator('[data-resource-model]')).toHaveCount(0);
  await assertCredentialPrivacy(page);
});

test('two API accounts on the same endpoint keep HTTP credentials and observed resources separate',async ({page})=>{
  const secondKey='synthetic-second-account-discovery-private-key';
  const transport=await mockDiscovery(page,request=>({body:catalog(request.body.apiKey===apiKey?{id:'first-account-model',name:'First account resource'}:{id:'second-account-model',name:'Second account resource'})}));
  await page.goto('/');await registerAPI(page,{name:'First API'});
  await expect.poll(async()=> (await currentConnection(page)).discovery.status).toBe('observed');const first=await currentConnection(page);
  await registerAPI(page,{name:'Second API',key:secondKey});
  await expect.poll(async()=> (await currentConnection(page)).discovery.status).toBe('observed');const second=await currentConnection(page);
  expect(first.id).not.toBe(second.id);expect(transport.requests.map(request=>request.body.apiKey)).toEqual([apiKey,secondKey]);
  expect(transport.requests.map(request=>request.body.endpoint)).toEqual(['https://api.openai.com/v1','https://api.openai.com/v1']);
  expect(await page.evaluate(()=>OrchestrixApp.getState().connections.map(connection=>connection.catalog.map(model=>model.id)))).toEqual([['first-account-model'],['second-account-model']]);
  const resources=await inspectResources(page,first.id);await expect(resources).toContainText('First account resource');await expect(resources).not.toContainText('Second account resource');
  await assertCredentialPrivacy(page,[apiKey,secondKey]);
});

test('resource capabilities translate in EN/PT/ES without changing the discovered catalog',async ({page})=>{
  await mockDiscovery(page,[{body:catalog()}]);await page.goto('/');await registerAPI(page);
  await expect.poll(async()=> (await currentConnection(page)).discovery.status).toBe('observed');const original=await currentConnection(page);
  const names={en:'Not reported','pt-BR':'Não informado',es:'No informado'};
  for(const language of ['en','pt-BR','es']) {
    await page.evaluate(language=>OrchestrixI18n.setLanguage(language),language);
    const resources=await inspectResources(page,original.id),model=resources.locator('[data-resource-model="research-model"]');
    await model.locator('summary').click();await expect(model.locator('[data-capability="nativeWeb"]')).toContainText(names[language]);
    await expect(model.locator('[data-capability="reasoning"]')).toContainText('low, high');
    expect((await currentConnection(page)).catalog).toEqual(original.catalog);
    expect(await page.evaluate(()=>OrchestrixI18n.missing())).toEqual([]);
    await page.getByRole('button',{name:language==='en'?'Close':language==='pt-BR'?'Fechar':'Cerrar',exact:true}).click();
  }
  await assertCredentialPrivacy(page);
});
