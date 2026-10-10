import {test,expect} from '@playwright/test';

const secret='private-test-key-not-for-storage';
const connection=page=>page.evaluate(()=>OrchestrixApp.getState().connections.at(-1));
async function begin(page,kind='api') {
  await page.evaluate(()=>OrchestrixConnections.addForm(OrchestrixApp.getState(),OrchestrixApp.helpers()));
  await page.locator(`#conn-kind-form [name="kind"][value="${kind}"]`).check();
  await page.locator('#conn-kind-form button[type="submit"]').click();
}
async function fillAPI(page,{provider='openai',name='My API',endpoint='',deployment='',region=''}={}) {
  await page.locator('#conn-api-provider').selectOption(provider);await page.locator('#conn-name').fill(name);
  if(endpoint)await page.locator('#conn-api-endpoint').fill(endpoint);
  if(deployment)await page.locator('#conn-api-deployment').fill(deployment);
  if(region)await page.locator('#conn-api-region').fill(region);
  await page.locator('#conn-api-key').fill(secret);await page.locator('[name="paidConsent"]').check();
}
async function save(page) {await page.locator('#conn-api-form button[type="submit"]').click();await expect(page.locator('#modal')).not.toBeVisible();}
async function inspect(page) {await page.evaluate(()=>{const c=OrchestrixApp.getState().connections.at(-1);OrchestrixConnections.inspect(c.id,OrchestrixApp.getState(),OrchestrixApp.helpers());});}
async function adapter(page,{provider='openai',endpoint='https://api.openai.com/v1',behavior='success',timeoutMs=10000,region='',deployment='',authMethod='api-key'}={}) {
  await page.evaluate(options=>{
    OrchestrixConnections.setDiscoveryAdapter(options,async request=>{
      window.discoveryCalls=(window.discoveryCalls||0)+1;
      window.discoveryScopeReceived={provider:request.provider,endpoint:request.endpoint,region:request.region,deployment:request.deployment,authMethod:request.authMethod,awsProfile:request.awsProfile,keyMatches:request.credential==='private-test-key-not-for-storage'};
      if(options.behavior==='error')throw new Error(`provider echoed ${request.credential}`);
      if(options.behavior==='slow')await new Promise(resolve=>window.finishDiscovery=resolve);
      if(options.behavior==='timeout')await new Promise(()=>{});
      return {models:[{id:'precise-model',name:'Precise model',capabilities:{reasoning:{observed:true,levels:['low','high']}}},{id:'basic-model',name:'Basic model',thinking:['high']}]};
    });
  },{provider,endpoint,behavior,timeoutMs,region,deployment,authMethod});
}

test.beforeEach(async ({page})=>{await page.addInitScript(()=>window.__ORCHESTRIX_TEST_DISCOVERY_ADAPTERS=true);await page.goto('/');});

test('connection type comes first and runtime accounts have their own providers',async ({page})=>{
  await page.evaluate(()=>OrchestrixConnections.addForm(OrchestrixApp.getState(),OrchestrixApp.helpers()));
  await expect(page.getByRole('group',{name:'How would you like to connect?'})).toBeVisible();await expect(page.locator('#conn-api-provider,#conn-runtime')).toHaveCount(0);
  await page.locator('#conn-kind-form button[type="submit"]').click();
  expect(await page.locator('#conn-runtime option').allTextContents()).toEqual(['Codex','Claude Code','Antigravity']);await expect(page.locator('input[type="password"]')).toHaveCount(0);
  await page.locator('#conn-runtime').selectOption('Antigravity');await page.locator('#conn-name').fill('My runtime');await page.locator('#conn-account-form button[type="submit"]').click();
  const c=await connection(page);expect([c.provider,c.mode,c.simulated,c.authorizationValid,c.catalog.length]).toEqual(['Antigravity','own-plan',false,false,0]);
  await inspect(page);await page.getByRole('button',{name:'Confirm configuration',exact:true}).click();expect((await connection(page)).authorizationValid).toBe(false);
});

test('API registration keeps credentials private and does not invent authentication, quota or catalogs',async ({page})=>{
  await begin(page);expect(await page.locator('#conn-api-provider option').allTextContents()).toEqual(['OpenAI','Anthropic','Gemini','Azure AI','AWS Bedrock','OpenAI-compatible']);
  await fillAPI(page);await expect(page.locator('#conn-api-key')).toHaveAttribute('type','password');await save(page);
  const c=await connection(page);expect(c).toMatchObject({apiProvider:'openai',mode:'api',paidConsent:true,simulated:false,authorizationValid:false,acceptsNewWork:false,catalog:[],discovery:{status:'unavailable'}});
  expect(await page.evaluate(()=>OrchestrixConnections.eligible(OrchestrixApp.getState().connections.at(-1)))).toBe(false);
  const surfaces=await page.evaluate(()=>({state:JSON.stringify(OrchestrixApp.getState()),storage:JSON.stringify({...localStorage}),dom:document.documentElement.outerHTML,values:[...document.querySelectorAll('input[type="password"]')].map(input=>input.value)}));
  for(const value of [surfaces.state,surfaces.storage,surfaces.dom,...surfaces.values])expect(value).not.toContain(secret);
  await inspect(page);await expect(page.getByRole('dialog')).toContainText('has not provided a catalog');await expect(page.getByRole('dialog')).not.toContainText(/100%|authenticated|authentication verified/i);
  await page.getByRole('button',{name:'Update credentials',exact:true}).click();await expect(page.locator('#conn-api-key')).toHaveValue('');
});

test('provider and AWS auth changes clear incompatible credentials and fields',async ({page})=>{
  await begin(page);await fillAPI(page,{provider:'azure',endpoint:'https://my-resource.openai.azure.com',deployment:'research'});
  await page.locator('#conn-api-provider').selectOption('anthropic');await expect(page.locator('#conn-api-key')).toHaveValue('');await expect(page.locator('#conn-api-deployment,#conn-api-endpoint')).toHaveCount(0);await expect(page.locator('[name="paidConsent"]')).not.toBeChecked();
  await page.locator('#conn-api-provider').selectOption('azure');await expect(page.locator('#conn-api-deployment')).toHaveValue('');await expect(page.locator('#conn-api-endpoint')).toHaveValue('');
  await page.locator('#conn-api-provider').selectOption('bedrock');await page.locator('#conn-api-region').fill('us-east-1');await page.locator('#conn-api-key').fill(secret);await page.locator('#conn-api-auth').selectOption('aws-profile');
  await expect(page.locator('#conn-api-key')).toHaveCount(0);await expect(page.locator('#conn-api-region')).toHaveValue('us-east-1');await expect(page.locator('#conn-api-profile')).toHaveValue('default');
  await page.locator('#conn-api-auth').selectOption('api-key');await expect(page.locator('#conn-api-key')).toHaveValue('');
});

test('Azure requires deployment and rejects secret-bearing endpoints; billing consent is mandatory',async ({page})=>{
  await begin(page);await fillAPI(page,{provider:'azure',endpoint:'https://user:secret@resource.openai.azure.com',deployment:'code'});
  await page.locator('#conn-api-form button[type="submit"]').click();await expect(page.locator('#conn-form-error')).toContainText('without credentials');expect(await page.evaluate(()=>OrchestrixApp.getState().connections.length)).toBe(0);
  await page.locator('#conn-api-endpoint').fill('https://resource.openai.azure.com');await page.locator('[name="paidConsent"]').uncheck();
  await page.locator('#conn-api-form').evaluate(form=>{form.noValidate=true;form.requestSubmit();});await expect(page.locator('#conn-form-error')).toContainText('Confirm API billing');
  await page.locator('[name="paidConsent"]').check();await page.locator('#conn-api-deployment').fill('bad deployment');await page.locator('#conn-api-form button[type="submit"]').click();await expect(page.locator('#conn-form-error')).toContainText('valid Azure deployment');
  await page.locator('#conn-api-deployment').fill('research-deployment');await save(page);expect(await connection(page)).toMatchObject({endpoint:'https://resource.openai.azure.com',deployment:'research-deployment',authorizationValid:false});
});

test('custom endpoints reject remote HTTP and trailing queries while allowing localhost HTTPS-independent services',async ({page})=>{
  await begin(page);await fillAPI(page,{provider:'compatible',endpoint:'http://remote.example.com/v1'});
  for(const endpoint of ['http://remote.example.com/v1','https://remote.example.com/v1?','https://remote.example.com/v1#','https://@remote.example.com/v1']){
    await page.locator('#conn-api-endpoint').fill(endpoint);await page.locator('#conn-api-form button[type="submit"]').click();await expect(page.locator('#conn-form-error')).toContainText('valid HTTPS endpoint');
  }
  await page.locator('#conn-api-endpoint').fill('http://127.0.0.1:11434/v1');await save(page);expect((await connection(page)).endpoint).toBe('http://127.0.0.1:11434/v1');
});

test('Bedrock supports a desktop AWS profile without asking for browser IAM credentials',async ({page})=>{
  await begin(page);await page.locator('#conn-api-provider').selectOption('bedrock');await page.locator('#conn-name').fill('Regional research');await page.locator('#conn-api-region').fill('us-west-2');await page.locator('#conn-api-auth').selectOption('aws-profile');await page.locator('#conn-api-profile').fill('research');await page.locator('[name="paidConsent"]').check();await save(page);
  expect(await connection(page)).toMatchObject({apiProvider:'bedrock',region:'us-west-2',authMethod:'aws-profile',awsProfile:'research',credentialPresent:false,endpoint:'https://bedrock-runtime.us-west-2.amazonaws.com',catalog:[]});
});

test('discovery is scoped and keeps basic model metadata separate from observed reasoning',async ({page})=>{
  await adapter(page);await begin(page);await fillAPI(page);await save(page);await expect.poll(async()=> (await connection(page)).discovery.status).toBe('observed');
  const c=await connection(page);expect(c.catalog[0].capabilities.reasoning).toEqual({observed:true,levels:['low','high']});expect(c.catalog[1].capabilities.reasoning).toEqual({observed:false});expect(c.catalog[1].thinking).toEqual([]);expect(c.authorizationValid).toBe(false);
  expect(await page.evaluate(()=>window.discoveryScopeReceived)).toMatchObject({provider:'openai',endpoint:'https://api.openai.com/v1',keyMatches:true});
  await inspect(page);await expect(page.getByRole('dialog')).toContainText('Precise model');await expect(page.getByRole('dialog')).toContainText('Reasoning capabilities not reported.');
  await page.getByRole('button',{name:'Close',exact:true}).click();await begin(page);await fillAPI(page,{provider:'anthropic',name:'Other provider'});await save(page);expect((await connection(page)).discovery.status).toBe('unavailable');expect(await page.evaluate(()=>window.discoveryCalls)).toBe(1);
});

test('discovery errors never expose provider echoes and can be retried',async ({page})=>{
  await adapter(page,{behavior:'error'});await begin(page);await fillAPI(page);await save(page);await expect.poll(async()=> (await connection(page)).discovery.status).toBe('failed');await inspect(page);
  await expect(page.getByRole('dialog')).toContainText('Could not retrieve models');await expect(page.locator('body')).not.toContainText(secret);expect(JSON.stringify(await connection(page))).not.toContain(secret);
  await adapter(page);await page.getByRole('button',{name:'Check models',exact:true}).click();await expect(page.getByRole('dialog')).toContainText('Precise model');expect((await connection(page)).discovery.status).toBe('observed');
});

test('adapter metadata echoing a credential is rejected before entering state or the interface',async ({page})=>{
  await page.evaluate(()=>OrchestrixConnections.setDiscoveryAdapter({provider:'openai',endpoint:'https://api.openai.com/v1'},request=>({models:[{id:'echo',name:`Unsafe echo ${request.credential}`}]})));
  await begin(page);await fillAPI(page);await save(page);await expect.poll(async()=> (await connection(page)).discovery.status).toBe('failed');await inspect(page);
  expect((await connection(page)).catalog).toEqual([]);expect(JSON.stringify(await connection(page))).not.toContain(secret);await expect(page.locator('body')).not.toContainText(secret);
});

test('retained credentials cannot enter another connection metadata or old-key rotation history',async ({page})=>{
  await begin(page);await fillAPI(page);await save(page);
  const before=await page.evaluate(()=>JSON.stringify(OrchestrixApp.getState()));
  await begin(page,'own-plan');await page.locator('#conn-name').fill(secret);await page.locator('#conn-account-form button[type="submit"]').click();await expect(page.locator('#conn-name')).toHaveValue('');
  await page.locator('#conn-name').fill('Other runtime');await page.locator('#conn-workspace').fill(secret);await page.locator('#conn-account-form button[type="submit"]').click();
  await expect(page.locator('#conn-form-error')).toContainText('secure credential field');await expect(page.locator('#conn-workspace')).toHaveValue('');expect(await page.evaluate(()=>JSON.stringify(OrchestrixApp.getState()))).toBe(before);
  await page.getByRole('button',{name:'Close',exact:true}).click();await begin(page);await fillAPI(page,{provider:'compatible',name:'Other API',endpoint:`https://api.example.com/${secret}`});await page.locator('#conn-api-key').fill('another-private-test-key');await page.locator('#conn-api-form button[type="submit"]').click();
  await expect(page.locator('#conn-form-error')).toContainText('secure credential field');await expect(page.locator('#conn-api-endpoint')).toHaveValue('');expect(await page.evaluate(()=>OrchestrixApp.getState().connections.length)).toBe(1);
  await page.getByRole('button',{name:'Close',exact:true}).click();await inspect(page);await page.getByRole('button',{name:'Update credentials',exact:true}).click();await page.locator('#conn-name').fill(secret);await page.locator('#conn-api-key').fill('replacement-private-test-key');await page.locator('[name="paidConsent"]').check();await page.locator('#conn-api-form button[type="submit"]').click();
  await expect(page.locator('#conn-form-error')).toContainText('secure credential field');await expect(page.locator('#conn-name')).toHaveValue('');expect((await connection(page)).identity.revision).toBe(1);expect((await connection(page)).name).toBe('My API');
  const surfaces=await page.evaluate(()=>({state:JSON.stringify(OrchestrixApp.getState()),storage:JSON.stringify({...localStorage}),dom:document.documentElement.outerHTML}));for(const value of Object.values(surfaces))expect(value).not.toContain(secret);
});

test('Back preserves valid provider-specific metadata and runtime drafts while clearing keys',async ({page})=>{
  await begin(page);await fillAPI(page,{provider:'azure',name:'Azure draft',endpoint:'https://draft.azure.com',deployment:'draft-model'});await page.getByRole('button',{name:'Back',exact:true}).click();
  await expect(page.locator('#conn-kind-form [name="kind"][value="api"]')).toBeChecked();await page.locator('#conn-kind-form button[type="submit"]').click();await expect(page.locator('#conn-api-provider')).toHaveValue('azure');await expect(page.locator('#conn-name')).toHaveValue('Azure draft');await expect(page.locator('#conn-api-endpoint')).toHaveValue('https://draft.azure.com');await expect(page.locator('#conn-api-deployment')).toHaveValue('draft-model');await expect(page.locator('#conn-api-key')).toHaveValue('');await expect(page.locator('[name="paidConsent"]')).not.toBeChecked();
  await page.getByRole('button',{name:'Close',exact:true}).click();await begin(page,'own-plan');await page.locator('#conn-runtime').selectOption('Claude Code');await page.locator('#conn-name').fill('Runtime draft');await page.locator('#conn-workspace').fill('Research');await page.getByRole('button',{name:'Back',exact:true}).click();await page.locator('#conn-kind-form button[type="submit"]').click();await expect(page.locator('#conn-runtime')).toHaveValue('Claude Code');await expect(page.locator('#conn-name')).toHaveValue('Runtime draft');await expect(page.locator('#conn-workspace')).toHaveValue('Research');
});

test('two accounts sharing an endpoint keep concurrent discovery results separate',async ({page})=>{
  await page.evaluate(()=>{
    window.pendingCatalogs={};
    OrchestrixConnections.setDiscoveryAdapter({provider:'openai',endpoint:'https://api.openai.com/v1'},request=>new Promise(resolve=>{
      const label=request.credential==='private-test-key-not-for-storage'?'A':'B';window.pendingCatalogs[label]=()=>resolve({models:[{id:`model-${label}`,name:`Model ${label}`}]});
    }));
  });
  await begin(page);await fillAPI(page,{name:'API A'});await save(page);await begin(page);await fillAPI(page,{name:'API B'});await page.locator('#conn-api-key').fill('independent-second-test-key');await save(page);
  await page.evaluate(()=>window.pendingCatalogs.B());await expect.poll(async()=> (await connection(page)).discovery.status).toBe('observed');
  expect(await page.evaluate(()=>OrchestrixApp.getState().connections.map(c=>[c.name,c.discovery.status,c.catalog.map(model=>model.id)]))).toEqual([['API A','loading',[]],['API B','observed',['model-B']]]);
  await page.evaluate(()=>window.pendingCatalogs.A());await expect.poll(()=>page.evaluate(()=>OrchestrixApp.getState().connections[0].discovery.status)).toBe('observed');expect(await page.evaluate(()=>OrchestrixApp.getState().connections.map(c=>c.catalog[0].id))).toEqual(['model-A','model-B']);
});

test('credential rotation and disconnect discard earlier pending catalog responses',async ({page})=>{
  await page.evaluate(()=>{
    window.pendingRequests=[];
    OrchestrixConnections.setDiscoveryAdapter({provider:'openai',endpoint:'https://api.openai.com/v1'},request=>new Promise(resolve=>{
      const revision=window.pendingRequests.length+1;window.pendingRequests.push({signal:request.signal,finish:()=>resolve({models:[{id:`revision-${revision}`,name:`Revision ${revision}`} ]})});
    }));
  });
  await begin(page);await fillAPI(page);await save(page);await inspect(page);await page.getByRole('button',{name:'Update credentials',exact:true}).click();await page.locator('#conn-api-key').fill('replacement-private-test-key');await page.locator('[name="paidConsent"]').check();await save(page);
  expect(await page.evaluate(()=>window.pendingRequests[0].signal.aborted)).toBe(true);await page.evaluate(()=>window.pendingRequests[0].finish());expect((await connection(page)).catalog).toEqual([]);expect((await connection(page)).identity.revision).toBe(2);
  await page.evaluate(()=>window.pendingRequests[1].finish());await expect.poll(async()=> (await connection(page)).discovery.status).toBe('observed');expect((await connection(page)).catalog[0].id).toBe('revision-2');
  await inspect(page);await page.getByRole('button',{name:'Check models',exact:true}).click();await page.getByRole('button',{name:'Disconnect for new work',exact:true}).click();await page.getByRole('button',{name:'Block new work',exact:true}).click();expect(await page.evaluate(()=>window.pendingRequests[2].signal.aborted)).toBe(true);await page.evaluate(()=>window.pendingRequests[2].finish());
  expect(await connection(page)).toMatchObject({lifecycle:'disconnected',credentialPresent:false,catalogLoading:false,discovery:{status:'cancelled'}});expect((await connection(page)).catalog[0].id).toBe('revision-2');
});

test('leaving the page clears credentials and cancels pending discovery',async ({page})=>{
  await adapter(page,{behavior:'slow'});await begin(page);await fillAPI(page);await save(page);await page.evaluate(()=>window.dispatchEvent(new Event('pagehide')));expect(await connection(page)).toMatchObject({credentialPresent:false,catalogLoading:false,discovery:{status:'cancelled'}});
  await page.evaluate(()=>window.finishDiscovery());expect((await connection(page)).catalog).toEqual([]);await inspect(page);await page.getByRole('button',{name:'Check models',exact:true}).click();expect(await page.evaluate(()=>window.discoveryCalls)).toBe(1);await expect(page.locator('#toast')).toContainText('Reconnect required');
});

test('provider change, Back, cancel and disconnect do not reuse a credential',async ({page})=>{
  await adapter(page);await begin(page);await fillAPI(page);await page.getByRole('button',{name:'Back',exact:true}).click();await page.locator('#conn-kind-form [name="kind"][value="api"]').check();await page.locator('#conn-kind-form button[type="submit"]').click();await expect(page.locator('#conn-api-key')).toHaveValue('');
  await fillAPI(page);await page.getByRole('button',{name:'Close',exact:true}).click();await expect(page.locator('#conn-api-key')).toHaveValue('');await begin(page);await expect(page.locator('#conn-api-key')).toHaveValue('');await fillAPI(page);await save(page);await expect.poll(async()=> (await connection(page)).discovery.status).toBe('observed');
  await inspect(page);await page.getByRole('button',{name:'Disconnect for new work',exact:true}).click();await page.getByRole('button',{name:'Block new work',exact:true}).click();expect((await connection(page)).credentialPresent).toBe(false);
  await page.getByRole('button',{name:'Check models',exact:true}).click();expect(await page.evaluate(()=>window.discoveryCalls)).toBe(1);
});

test('cancelling discovery ignores late responses',async ({page})=>{
  await adapter(page,{behavior:'slow'});await begin(page);await fillAPI(page);await save(page);await inspect(page);await expect(page.getByRole('button',{name:'Cancel request',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Cancel request',exact:true}).click();await page.evaluate(()=>window.finishDiscovery());
  expect(await connection(page)).toMatchObject({catalogLoading:false,catalog:[],discovery:{status:'cancelled'}});await expect(page.getByRole('dialog')).toContainText('Late results will be ignored');
});

test('timeout is recoverable and subscription settings remain separate from API consent',async ({page})=>{
  await adapter(page,{behavior:'timeout',timeoutMs:40});await begin(page);await fillAPI(page);await save(page);await expect.poll(async()=> (await connection(page)).discovery.status).toBe('timeout');await inspect(page);await expect(page.getByRole('dialog')).toContainText('longer than expected');
  expect(await page.evaluate(()=>OrchestrixApp.getSettings().subscriptionOnly)).toBe(true);expect((await connection(page)).paidConsent).toBe(true);expect((await connection(page)).acceptsNewWork).toBe(false);
});

for(const language of ['es','pt-BR'])test(`connection wizard preserves fields and translates in ${language}`,async ({page})=>{
  await begin(page);await fillAPI(page,{provider:'azure',name:'My original label',endpoint:'https://my.azure.com',deployment:'original'});await page.evaluate(language=>OrchestrixI18n.setLanguage(language),language);
  await expect(page.locator('#conn-name')).toHaveValue('My original label');await expect(page.locator('#conn-api-key')).toHaveValue(secret);await expect(page.locator('#conn-api-deployment')).toHaveValue('original');await expect(page.locator('[name="paidConsent"]')).toBeChecked();
  expect(await page.evaluate(()=>OrchestrixI18n.missing())).toEqual([]);
});

for(const language of ['en','pt-BR','es'])test(`every API provider form has complete copy in ${language}`,async ({page})=>{
  await page.evaluate(language=>OrchestrixI18n.setLanguage(language),language);await begin(page);
  for(const provider of ['openai','anthropic','gemini','azure','bedrock','compatible']){
    await page.evaluate(()=>OrchestrixI18n.clearMissing());await page.locator('#conn-api-provider').selectOption(provider);expect(await page.evaluate(()=>OrchestrixI18n.missing())).toEqual([]);
    if(provider==='bedrock'){await expect(page.locator('#conn-api-region')).toHaveAttribute('placeholder','us-east-1');await page.locator('#conn-api-auth').selectOption('aws-profile');expect(await page.evaluate(()=>OrchestrixI18n.missing())).toEqual([]);}
  }
});
