import test from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {once} from 'node:events';
import {discoverProviderCatalog,discoverRuntimeCatalog,ProviderDiscoveryError} from '../src/index.mjs';

const key='synthetic-provider-key-for-tests';
const config=provider=>({provider,apiKey:key});
const json=value=>new Response(JSON.stringify(value),{headers:{'content-type':'application/json'}});
function sequence(...responses){const calls=[];const fetchImpl=async (url,options)=>{calls.push({url,options});const response=responses.shift();if(response instanceof Error)throw response;if(typeof response==='function')return response(url,options);assert.ok(response,'Unexpected provider request');return response;};return {fetchImpl,calls};}
async function rejects(code,operation){await assert.rejects(operation,error=>error instanceof ProviderDiscoveryError&&error.code===code&&!JSON.stringify(error).includes(key)&&!error.message.includes(key));}

test('OpenAI uses a read-only authenticated catalog and preserves unknown capabilities',async()=>{
  const transport=sequence(json({data:[{id:'any-provider-id',owned_by:'provider',created:123},{id:'other-id'}]}));
  const result=await discoverProviderCatalog(config('openai'),transport);
  assert.equal(transport.calls[0].url,'https://api.openai.com/v1/models');assert.equal(transport.calls[0].options.method,'GET');assert.equal(transport.calls[0].options.headers.Authorization,`Bearer ${key}`);assert.equal(transport.calls[0].options.redirect,'manual');
  assert.equal(result.status,'complete');assert.equal(result.models.length,2);assert.equal(result.authentication,'unknown');assert.equal(result.entitlement,'unknown');assert.equal(result.usage,'unknown');
  for(const model of result.models)for(const capability of Object.values(model.capabilities))assert.equal(capability.status,'unknown');
  assert.ok(!JSON.stringify(result).includes(key));assert.equal(result.models[0].compatibility.status,'unknown');
});

test('Anthropic consumes all cursor pages and only explicit thinking, effort and native web metadata',async()=>{
  const transport=sequence(json({data:[{id:'one',display_name:'First',max_input_tokens:200000,max_tokens:32000,capabilities:{thinking:{supported:true,types:{adaptive:{supported:true},disabled:{supported:true}}},effort:{supported:true,low:{supported:true},high:{supported:true}},server_tools:{supported:true,web_search:{supported:false}},structured_outputs:{supported:true},image_input:{supported:true}}}],has_more:true,last_id:'one'}),json({data:[{id:'two',capabilities:null}],has_more:false,last_id:'two'}));
  const result=await discoverProviderCatalog(config('anthropic'),transport);
  assert.equal(result.pages,2);assert.equal(new URL(transport.calls[1].url).searchParams.get('after_id'),'one');assert.equal(transport.calls[0].options.headers['x-api-key'],key);assert.equal(transport.calls[0].options.headers['anthropic-version'],'2023-06-01');
  const model=result.models[0];assert.equal(model.capabilities.context.inputTokens,200000);assert.equal(model.capabilities.context.outputTokens,32000);assert.deepEqual(model.capabilities.reasoning.levels,['low','high']);assert.deepEqual(model.capabilities.reasoning.modes,['adaptive','disabled']);assert.equal(model.capabilities.nativeWeb.status,'unsupported');assert.equal(model.capabilities.functions.status,'unknown');assert.equal(model.capabilities.streaming.status,'unknown');assert.equal(result.models[1].capabilities.reasoning.status,'unknown');assert.equal(model.capabilities.context.evidence[0].kind,'provider-metadata');
});

test('Gemini paginates with header authentication and keeps tools and missing thinking unknown',async()=>{
  const transport=sequence(json({models:[{name:'models/first',inputTokenLimit:1000,outputTokenLimit:300,thinking:false,supportedGenerationMethods:['generateContent'],temperature:1,topK:40}],nextPageToken:'cursor'}),json({models:[{name:'models/embedding',supportedGenerationMethods:['embedContent']}] }));
  const result=await discoverProviderCatalog(config('gemini'),transport);
  assert.equal(transport.calls[0].options.headers['x-goog-api-key'],key);assert.ok(!transport.calls[0].url.includes(key));assert.equal(new URL(transport.calls[1].url).searchParams.get('pageToken'),'cursor');assert.equal(result.models[0].capabilities.reasoning.status,'unsupported');assert.equal(result.models[0].capabilities.input.status,'unknown');assert.equal(result.models[0].capabilities.nativeWeb.status,'unknown');assert.equal(result.models[1].compatibility.status,'unsupported');assert.equal(result.models[1].capabilities.reasoning.status,'unknown');
});

test('Azure resource key lists v1 models without requiring ARM and configured deployment stays unverified',async()=>{
  const transport=sequence(json({data:[{id:'base-model'}]}));
  const result=await discoverProviderCatalog({...config('azure'),endpoint:'https://resource.openai.azure.com',deployment:'my-deployment'},transport);
  assert.equal(transport.calls[0].url,'https://resource.openai.azure.com/openai/v1/models');assert.equal(transport.calls[0].options.headers['api-key'],key);assert.equal(transport.calls[0].options.headers.Authorization,undefined);
  assert.equal(result.models[0].kind,'model');assert.equal(result.models[1].kind,'deployment');assert.equal(result.models[1].source.kind,'user-declaration');assert.equal(result.models[1].availability,'unverified');assert.equal(result.models[1].compatibility.status,'unknown');
  assert.equal(result.scope.apiVersion,'v1');assert.equal(result.source.documentation,'https://learn.microsoft.com/en-us/rest/api/microsoft-foundry/azureopenai/models');
});

test('Azure catalog permission failure preserves only a manually declared unverified deployment',async()=>{
  const result=await discoverProviderCatalog({...config('azure'),endpoint:'https://resource.openai.azure.com/openai/v1',deployment:'configured'},sequence(new Response('',{status:403})));
  assert.equal(result.status,'unavailable');assert.deepEqual(result.models.map(model=>[model.id,model.source.kind]),[['configured','user-declaration']]);assert.ok(result.limitations.some(value=>value.code==='access-denied'));assert.equal(result.authentication,'unknown');
});

test('Azure deployment enumeration uses separately supplied management authorization and guarded nextLink',async()=>{
  const token='synthetic-management-token';const url='https://management.azure.com/subscriptions/sub/resourceGroups/group/providers/Microsoft.CognitiveServices/accounts/account/deployments?api-version=2025-06-01';
  const transport=sequence(json({value:[{name:'deployment-one',properties:{model:{name:'base',version:'2026',format:'OpenAI'},provisioningState:'Succeeded'}}],nextLink:`${url}&skip=one`}),json({value:[{name:'deployment-two'}]}));
  const result=await discoverProviderCatalog({provider:'azure',endpoint:'https://resource.openai.azure.com',managementToken:token,subscriptionId:'sub',resourceGroup:'group',accountName:'account'},transport);
  assert.equal(result.pages,2);assert.equal(transport.calls[0].options.headers.Authorization,`Bearer ${token}`);assert.equal(result.models[0].baseModel.name,'base');assert.equal(result.models[0].authentication,'unknown');assert.ok(!JSON.stringify(result).includes(token));
  await rejects('endpoint-blocked',()=>discoverProviderCatalog({provider:'azure',endpoint:'https://resource.openai.azure.com',managementToken:token,subscriptionId:'sub',resourceGroup:'group',accountName:'account'},sequence(json({value:[],nextLink:'https://attacker.example/deployments'}))));
});

test('Bedrock bearer discovery uses control host, modalities and streaming then paginated inference profiles',async()=>{
  const transport=sequence(json({modelSummaries:[{modelId:'foundation-one',modelName:'Foundation',inputModalities:['TEXT','IMAGE'],outputModalities:['TEXT'],responseStreamingSupported:true,providerName:'Vendor',inferenceTypesSupported:['ON_DEMAND'],modelLifecycle:{status:'ACTIVE'}}]}),json({inferenceProfileSummaries:[{inferenceProfileId:'profile-one',inferenceProfileName:'Profile',type:'SYSTEM_DEFINED',models:[{modelArn:'arn:aws:bedrock:us-east-1::foundation-model/foundation-one'}]}],nextToken:'next-profile'}),json({inferenceProfileSummaries:[]}));
  const result=await discoverProviderCatalog({...config('bedrock'),region:'us-east-1',authMethod:'api-key'},transport);
  assert.equal(transport.calls[0].url,'https://bedrock.us-east-1.amazonaws.com/foundation-models');assert.equal(transport.calls[0].options.headers.Authorization,`Bearer ${key}`);assert.equal(new URL(transport.calls[2].url).searchParams.get('nextToken'),'next-profile');assert.equal(result.models[0].capabilities.streaming.status,'supported');assert.equal(result.models[0].compatibility.status,'supported');assert.equal(result.models[0].capabilities.functions.status,'unknown');assert.equal(result.models[1].kind,'inference-profile');assert.equal(result.models[1].capabilities.input.status,'unknown');assert.equal(result.entitlement,'unknown');
});

test('Bedrock profile permission failure yields a partial foundation catalog and AWS profile requires a signer',async()=>{
  const result=await discoverProviderCatalog({...config('bedrock'),region:'us-east-1'},sequence(json({modelSummaries:[{modelId:'one'}]}),new Response('',{status:403})));
  assert.equal(result.status,'partial');assert.equal(result.models.length,1);assert.ok(result.limitations.some(value=>value.code==='bedrock-profile-catalog-unavailable'));
  const unavailable=await discoverProviderCatalog({provider:'bedrock',region:'us-east-1',authMethod:'aws-profile'},{fetchImpl:()=>assert.fail('Must not impersonate IAM signing')});assert.equal(unavailable.status,'unavailable');assert.equal(unavailable.limitations[0].code,'aws-signer-required');
});

test('custom protocol lists only its explicitly configured endpoint without OpenAI capability inference',async()=>{
  const transport=sequence(json({data:[{id:'custom',context_window:123456,tools:true}]}));const result=await discoverProviderCatalog({...config('compatible'),endpoint:'https://custom.example/openai/v1'},transport);
  assert.equal(transport.calls[0].url,'https://custom.example/openai/v1/models');assert.equal(result.models[0].capabilities.context.status,'unknown');assert.equal(result.models[0].capabilities.functions.status,'unknown');
});

test('provider endpoint, userinfo, query, redirects and private network policies are enforced',async()=>{
  for(const endpoint of ['https://attacker.example/v1','http://api.openai.com/v1','https://user@api.openai.com/v1','https://api.openai.com/v1?','https://api.openai.com/v1#'])await rejects('endpoint-blocked',()=>discoverProviderCatalog({...config('openai'),endpoint},{fetchImpl:()=>assert.fail('No request should be dispatched')}));
  for(const endpoint of ['http://localhost:9000/v1','https://169.254.169.254/v1','https://10.0.0.1/v1','https://[::ffff:127.0.0.1]/v1','https://service.internal/v1'])await rejects('endpoint-blocked',()=>discoverProviderCatalog({...config('compatible'),endpoint},{fetchImpl:()=>assert.fail('No blocked endpoint request')}));
  const transport=sequence(json({data:[]}));await discoverProviderCatalog({...config('compatible'),endpoint:'http://127.0.0.1:9000/v1',allowLoopbackHTTP:true},transport);assert.equal(transport.calls[0].url,'http://127.0.0.1:9000/v1/models');
  await rejects('redirect-blocked',()=>discoverProviderCatalog(config('openai'),sequence(new Response('',{status:302,headers:{location:'https://attacker.example'}}))));
});

test('errors redact credentials, never include provider bodies, and classify status failures safely',async()=>{
  for(const [status,code] of [[401,'authentication-required'],[403,'access-denied'],[404,'catalog-unavailable'],[429,'rate-limited'],[503,'provider-unavailable']])await rejects(code,()=>discoverProviderCatalog(config('openai'),sequence(new Response(`echo ${key}`,{status}))));
  await rejects('network-error',()=>discoverProviderCatalog(config('openai'),sequence(new Error(`echo ${key}`))));
  await rejects('credential-echo',()=>discoverProviderCatalog(config('openai'),sequence(json({data:[{id:'one',name:`echo ${key}`}]}))));
  await rejects('credential-echo',()=>discoverProviderCatalog({...config('openai'),connectionId:key},sequence(json({data:[]}))));
});

test('response, total bytes, pages, model limits and repeated cursors are bounded',async()=>{
  await rejects('response-too-large',()=>discoverProviderCatalog(config('openai'),{...sequence(json({data:[{id:'x'.repeat(200)}]})),maxResponseBytes:100}));
  await rejects('invalid-response',()=>discoverProviderCatalog(config('openai'),sequence(json({data:null}))));
  const partial=await discoverProviderCatalog(config('anthropic'),{...sequence(json({data:[{id:'one'}],has_more:true,last_id:'one'})),maxPages:1});assert.equal(partial.status,'partial');assert.equal(partial.limitations[0].code,'page-limit-reached');
  const bounded=await discoverProviderCatalog(config('openai'),{...sequence(json({data:[{id:'one'},{id:'two'}]})),maxModels:1});assert.equal(bounded.models.length,1);assert.equal(bounded.status,'partial');
  await rejects('invalid-response',()=>discoverProviderCatalog(config('anthropic'),sequence(json({data:[{id:'one'}],has_more:true,last_id:'cursor'}),json({data:[{id:'two'}],has_more:true,last_id:'cursor'}))));
});

test('timeouts and external cancellation terminate even a noncooperative injected fetch',async()=>{
  await rejects('timeout',()=>discoverProviderCatalog(config('openai'),{fetchImpl:()=>new Promise(()=>{}),timeoutMs:20}));
  const controller=new AbortController();const operation=discoverProviderCatalog(config('openai'),{fetchImpl:()=>new Promise(()=>{}),signal:controller.signal});controller.abort();await rejects('cancelled',()=>operation);
});

test('default transport pins an explicitly allowed local catalog and never follows its redirects',async()=>{
  const calls=[];const server=createServer((request,response)=>{calls.push({method:request.method,path:request.url,authorization:request.headers.authorization});response.setHeader('Content-Type','application/json');if(request.url.startsWith('/redirect/')){response.writeHead(302,{Location:'http://169.254.169.254/'});response.end('{}');}else response.end(JSON.stringify({data:[{id:'local-model'}]}));});
  server.listen(0,'127.0.0.1');await once(server,'listening');
  try{
    const endpoint=`http://127.0.0.1:${server.address().port}/v1`;const result=await discoverProviderCatalog({...config('compatible'),endpoint,allowLoopbackHTTP:true});assert.equal(result.models[0].id,'local-model');assert.deepEqual(calls[0],{method:'GET',path:'/v1/models',authorization:`Bearer ${key}`});
    await rejects('redirect-blocked',()=>discoverProviderCatalog({...config('compatible'),endpoint:endpoint.replace('/v1','/redirect/v1'),allowLoopbackHTTP:true}));assert.equal(calls.length,2);
  }finally{server.closeAllConnections();await new Promise(resolve=>server.close(resolve));}
});

test('total byte and streaming body limits remain enforced across pages and body timeout',async()=>{
  const first={data:[{id:'one'}],has_more:true,last_id:'one'},second={data:[{id:'x'.repeat(100)}],has_more:false};
  await rejects('response-too-large',()=>discoverProviderCatalog(config('anthropic'),{...sequence(json(first),json(second)),maxTotalBytes:150}));
  const stream=new ReadableStream({start(controller){controller.enqueue(new TextEncoder().encode('{"data":'));}});
  await rejects('timeout',()=>discoverProviderCatalog(config('openai'),{...sequence(new Response(stream,{headers:{'content-type':'application/json'}})),timeoutMs:20}));
});

test('runtime helper preserves all pages, reasoning, modalities and optional metadata without starting execution',async()=>{
  const calls=[];const request=async(method,params)=>{
    calls.push({method,params});
    if(method==='modelProvider/capabilities/read')return {webSearch:true,imageGeneration:false,namespaceTools:true};
    if(params.cursor)return {data:[{id:'runtime-b',model:'model-b',supportedReasoningEfforts:[]}],nextCursor:null};
    return {data:[{id:'runtime-a',model:'model-a',displayName:'Model A',inputModalities:['text','image'],supportedReasoningEfforts:[{reasoningEffort:'low'},{reasoningEffort:'high'}],defaultReasoningEffort:'low',serviceTiers:['default','fast'],defaultServiceTier:'default',hidden:true,modelSpecialty:'code',multiAgentVersion:'2'}],nextCursor:'cursor'};
  };
  const result=await discoverRuntimeCatalog({request,connectionId:'connected-runtime',runtimeVersion:'0.162.0-alpha.2',modelProvider:'openai'});
  assert.equal(result.pages,2);assert.equal(result.models.length,2);assert.deepEqual(result.models[0].capabilities.reasoning.levels,['low','high']);assert.deepEqual(result.models[0].capabilities.input.modalities,['text','image']);assert.deepEqual(result.models[0].serviceTiers.value,['default','fast']);assert.equal(result.models[1].capabilities.reasoning.status,'unsupported');
  assert.equal(result.providerCapabilities.webSearch.status,'supported');assert.equal(result.providerCapabilities.imageGeneration.status,'unsupported');assert.equal(result.providerCapabilities.scope.modelProvider,'openai');assert.equal(result.models[0].capabilities.nativeWeb.status,'unknown');assert.equal(result.authentication,'unknown');assert.deepEqual(calls.map(call=>call.method),['model/list','model/list','modelProvider/capabilities/read']);
  assert.equal(calls[0].params.includeHidden,true);assert.equal(result.models[0].hidden,true);assert.equal(result.models[0].defaultServiceTier.value,'default');assert.equal(result.models[0].defaultReasoningEffort.value,'low');
});

test('later transient failures retain completed catalog pages but unsafe responses remain fatal',async()=>{
  const first=()=>json({data:[{id:'observed'}],has_more:true,last_id:'observed'});
  for(const [status,code] of [[403,'access-denied'],[429,'rate-limited'],[503,'provider-unavailable']]){
    const result=await discoverProviderCatalog(config('anthropic'),sequence(first(),new Response(`echo ${key}`,{status})));
    assert.equal(result.status,'partial');assert.equal(result.models[0].id,'observed');assert.equal(result.limitations[0].code,code);assert.equal(result.authentication,'unknown');assert.ok(!JSON.stringify(result).includes(key));
  }
  const network=await discoverProviderCatalog(config('anthropic'),sequence(first(),new Error(key)));assert.equal(network.limitations[0].code,'network-error');
  const timeout=await discoverProviderCatalog(config('anthropic'),{...sequence(first(),()=>new Promise(()=>{})),timeoutMs:20});assert.equal(timeout.status,'partial');assert.equal(timeout.limitations[0].code,'timeout');
  await rejects('credential-echo',()=>discoverProviderCatalog(config('anthropic'),sequence(first(),json({data:[{id:key}],has_more:false}))));
  await rejects('invalid-response',()=>discoverProviderCatalog(config('anthropic'),sequence(first(),json({data:null}))));
  await rejects('redirect-blocked',()=>discoverProviderCatalog(config('anthropic'),sequence(first(),new Response('',{status:302}))));
  const controller=new AbortController();const operation=discoverProviderCatalog(config('anthropic'),{...sequence(first(),()=>{controller.abort();return new Promise(()=>{});}),signal:controller.signal});await rejects('cancelled',()=>operation);
});

test('resource limits cover manual declarations and distinct catalog resource kinds',async()=>{
  const azure=await discoverProviderCatalog({...config('azure'),endpoint:'https://resource.openai.azure.com',deployment:'declared'}, {...sequence(json({data:[{id:'observed'}]})),maxModels:1});
  assert.equal(azure.models.length,1);assert.equal(azure.status,'partial');assert.ok(azure.limitations.some(item=>item.code==='model-limit-reached'));
  const resources=await discoverProviderCatalog({...config('bedrock'),region:'us-east-1'}, sequence(json({modelSummaries:[{modelId:'same-id'},{modelId:'same-id'}]}),json({inferenceProfileSummaries:[{inferenceProfileId:'same-id'}]})));
  assert.deepEqual(resources.models.map(item=>[item.kind,item.id]),[['model','same-id'],['inference-profile','same-id']]);
});

test('runtime later failures preserve observations and explicit hidden opt-out while cancellation is fatal',async()=>{
  const calls=[];const request=async(method,params)=>{calls.push(params);if(params.cursor)throw new Error(key);return {data:[{model:'one',defaultReasoningEffort:'medium'}],nextCursor:'next'};};
  const result=await discoverRuntimeCatalog({request,includeHidden:false});assert.equal(result.status,'partial');assert.equal(result.models[0].capabilities.reasoning.status,'unknown');assert.equal(result.models[0].defaultReasoningEffort.value,'medium');assert.equal(calls[0].includeHidden,false);assert.equal(result.limitations[0].code,'provider-unavailable');
  const controller=new AbortController();await rejects('cancelled',()=>discoverRuntimeCatalog({request:async()=>{controller.abort();return new Promise(()=>{});}},{signal:controller.signal}));
  await rejects('credential-echo',()=>discoverRuntimeCatalog({modelProvider:key,request:async method=>method==='model/list'?{data:[{model:'one'}],nextCursor:null}:{webSearch:true,imageGeneration:false,namespaceTools:false}},{redactValues:[key]}));
});

test('runtime metadata remains unknown when absent and RPC failures do not leak payloads',async()=>{
  const result=await discoverRuntimeCatalog({request:async method=>{if(method==='model/list')return {data:[{model:'one'}],nextCursor:null};throw new Error(`unknown method echoed ${key}`);}});
  assert.equal(result.models[0].capabilities.reasoning.status,'unknown');assert.equal(result.providerCapabilities,null);assert.equal(result.limitations[0].code,'runtime-provider-capabilities-unavailable');
  await rejects('credential-echo',()=>discoverRuntimeCatalog({request:async()=>({data:[{model:key}],nextCursor:null})},{redactValues:[key]}));
  await rejects('timeout',()=>discoverRuntimeCatalog({request:()=>new Promise(()=>{})},{timeoutMs:20}));
});
