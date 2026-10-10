import {isIP} from 'node:net';
import {lookup} from 'node:dns/promises';
import {request as requestHTTPS} from 'node:https';
import {request as requestHTTP} from 'node:http';
import {Readable} from 'node:stream';

const DEFAULTS=Object.freeze({openai:'https://api.openai.com/v1',anthropic:'https://api.anthropic.com/v1',gemini:'https://generativelanguage.googleapis.com/v1beta'});
const PROVIDERS=new Set(['openai','anthropic','gemini','azure','bedrock','compatible']);
const DOCS=Object.freeze({
  openai:'https://developers.openai.com/api/reference/resources/models/methods/list',
  anthropic:'https://platform.claude.com/docs/en/api/models/list',
  gemini:'https://ai.google.dev/api/models',
  azure:'https://learn.microsoft.com/en-us/rest/api/microsoftfoundry/accountmanagement/deployments/list?view=rest-microsoftfoundry-accountmanagement-2025-06-01',
  azureModels:'https://learn.microsoft.com/en-us/rest/api/microsoft-foundry/azureopenai/models',
  bedrock:'https://docs.aws.amazon.com/bedrock/latest/APIReference/API_ListFoundationModels.html',
  bedrockKey:'https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys-reference.html'
});
const MESSAGES=Object.freeze({
  'invalid-config':'The catalog configuration is invalid.',
  'endpoint-blocked':'This endpoint is not allowed for this provider.',
  'redirect-blocked':'Catalog redirects are not allowed.',
  'authentication-required':'The catalog request requires valid credentials.',
  'access-denied':'This credential cannot access the catalog.',
  'catalog-unavailable':'The catalog operation is unavailable for this endpoint.',
  'rate-limited':'The provider rate-limited this catalog request.',
  'provider-unavailable':'The provider could not complete the catalog request.',
  'network-error':'The catalog request could not reach the provider.',
  'invalid-response':'The provider returned an invalid catalog response.',
  'response-too-large':'The catalog response exceeded the configured size limit.',
  'credential-echo':'The provider returned credential material in catalog metadata.',
  'timeout':'The catalog request timed out.',
  'cancelled':'The catalog request was cancelled.'
});
export class ProviderDiscoveryError extends Error {
  constructor(code,{provider=null,httpStatus=null}={}) {
    super(MESSAGES[code]||MESSAGES['invalid-response']);this.name='ProviderDiscoveryError';
    this.code=Object.hasOwn(MESSAGES,code)?code:'invalid-response';this.provider=PROVIDERS.has(provider)?provider:null;
    this.httpStatus=Number.isInteger(httpStatus)&&httpStatus>=100&&httpStatus<=599?httpStatus:null;
    this.retryable=['rate-limited','provider-unavailable','network-error','timeout'].includes(this.code);
  }
  toJSON(){return {name:this.name,code:this.code,message:this.message,provider:this.provider,httpStatus:this.httpStatus,retryable:this.retryable};}
}
const fail=(code,provider,httpStatus)=>{throw new ProviderDiscoveryError(code,{provider,httpStatus});};
const recoverableCatalogError=code=>['authentication-required','access-denied','catalog-unavailable','rate-limited','provider-unavailable','network-error','timeout'].includes(code);
const unknown=()=>({status:'unknown',observed:false,evidence:[]});
const integer=value=>Number.isSafeInteger(value)&&value>0?value:null;
function safeString(value,max=240){if(typeof value!=='string'||!value.trim()||value.length>max||/[\u0000-\u001f\u007f]/.test(value))fail('invalid-response');return value;}
function bound(value,fallback,min,max){if(value===undefined)return fallback;if(!Number.isInteger(value)||value<min||value>max)fail('invalid-config');return value;}
function secret(value,required=true){if(!required&&value===undefined)return null;if(typeof value!=='string'||value.length<8||value.length>8192||/\s|[\u0000-\u001f\u007f]/.test(value))fail('invalid-config');return value;}
function publicText(value,secrets,max=240){const text=safeString(value,max);if(secrets.some(key=>key&&text.includes(key)))fail('credential-echo');return text;}
function loopback(host){return ['localhost','127.0.0.1','[::1]','::1'].includes(host);}
function privateHost(host){
  if(/(?:^|\.)(?:local|internal|localhost)$/.test(host)||host==='metadata.google.internal')return true;
  const raw=host.replace(/^\[|\]$/g,'');
  if(isIP(raw)===4){const [a,b]=raw.split('.').map(Number);return a===0||a===10||a===127||a===169&&b===254||a===172&&b>=16&&b<=31||a===192&&b===168||a===100&&b>=64&&b<=127||a>=224;}
  if(isIP(raw)===6)return raw==='::'||raw==='::1'||/^f[cd]|^fe[89ab]|^ff|^::ffff:/i.test(raw);
  return false;
}
async function pinnedFetch(url,options,allowLoopback=false){
  const parsed=new URL(url),host=parsed.hostname.replace(/^\[|\]$/g,'');
  const addresses=isIP(host)?[{address:host,family:isIP(host)}]:await lookup(host,{all:true});
  if(!addresses.length||addresses.some(value=>privateHost(value.address)&&!(allowLoopback&&loopback(parsed.hostname)&&loopback(value.address))))fail('endpoint-blocked');
  if(options.signal?.aborted)throw new DOMException('Cancelled','AbortError');
  const pinned=addresses.find(value=>value.family===4)||addresses[0];
  return new Promise((resolve,reject)=>{
    const request=(parsed.protocol==='https:'?requestHTTPS:requestHTTP)(parsed,{
      method:options.method,headers:options.headers,signal:options.signal,
      // Validate once, pin that address, and preserve the original TLS server name.
      lookup:(_hostname,lookupOptions,callback)=>lookupOptions?.all?callback(null,[pinned]):callback(null,pinned.address,pinned.family)
    },response=>{
      const headers=new Headers();for(const [name,value] of Object.entries(response.headers))if(value!==undefined)headers.set(name,Array.isArray(value)?value.join(', '):value);
      resolve(new Response([204,205,304].includes(response.statusCode)?null:Readable.toWeb(response),{status:response.statusCode,headers}));
    });
    request.once('error',reject);request.end();
  });
}
function endpointFor(config){
  const provider=config.provider;let raw=config.endpoint||DEFAULTS[provider];
  if(provider==='bedrock'){
    if(!/^[a-z]{2}(?:-[a-z]+){1,3}-\d$/.test(config.region||''))fail('invalid-config',provider);
    raw=`https://bedrock.${config.region}.${config.region.startsWith('cn-')?'amazonaws.com.cn':'amazonaws.com'}`;
  }
  if(typeof raw!=='string'||raw.length>500||/[?#]/.test(raw)||/^[a-z][a-z0-9+.-]*:\/\/[^/]*@/i.test(raw))fail('endpoint-blocked',provider);
  let url;try{url=new URL(raw);}catch{fail('endpoint-blocked',provider);}
  if(url.username||url.password||url.search||url.hash||url.port&&url.port!=='443'&&provider!=='compatible')fail('endpoint-blocked',provider);
  const host=url.hostname.toLowerCase();
  if(url.protocol!=='https:'&&!(provider==='compatible'&&config.allowLoopbackHTTP===true&&url.protocol==='http:'&&loopback(host)))fail('endpoint-blocked',provider);
  if(provider==='openai'&&(host!=='api.openai.com'||url.pathname.replace(/\/$/,'')!=='/v1'))fail('endpoint-blocked',provider);
  if(provider==='anthropic'&&(host!=='api.anthropic.com'||url.pathname.replace(/\/$/,'')!=='/v1'))fail('endpoint-blocked',provider);
  if(provider==='gemini'&&(host!=='generativelanguage.googleapis.com'||!['/v1beta','/v1'].includes(url.pathname.replace(/\/$/,''))))fail('endpoint-blocked',provider);
  if(provider==='azure'&&(!/^[a-z0-9-]+\.(?:openai\.azure\.com|cognitiveservices\.azure\.com|services\.ai\.azure\.com)$/.test(host)||!['','/','/openai/v1','/openai/v1/'].includes(url.pathname)))fail('endpoint-blocked',provider);
  if(provider==='compatible'&&privateHost(host)&&!(config.allowLoopbackHTTP===true&&loopback(host)))fail('endpoint-blocked',provider);
  return url.href.replace(/\/$/,'');
}
function evidence(ctx,path,kind='provider-metadata'){
  return {kind,operation:ctx.operation,field:path,observedAt:ctx.observedAt,scope:ctx.scope,documentation:ctx.documentation?{url:ctx.documentation,consultedAt:'2026-10-10',apiVersion:ctx.scope.apiVersion}:null};
}
function booleanCapability(value,ctx,path){return typeof value==='boolean'?{status:value?'supported':'unsupported',observed:true,evidence:[evidence(ctx,path)]}:unknown();}
function resource(id,name,ctx,kind='model',source='provider-metadata'){
  return {id:publicText(id,ctx.secrets,200),name:publicText(name,ctx.secrets,240),kind,scope:{...ctx.scope,resourceId:id},source:{kind:source,operation:ctx.operation,observedAt:ctx.observedAt},observedAt:ctx.observedAt,
    compatibility:{status:'unknown',reason:'Catalog access does not establish text generation or execution compatibility.'},
    authentication:'unknown',entitlement:'unknown',capabilities:{input:unknown(),output:unknown(),context:unknown(),reasoning:unknown(),streaming:unknown(),functions:unknown(),nativeWeb:unknown(),hostTools:unknown()}};
}
function contextCapability(input,output,ctx,inputPath,outputPath){
  const inputTokens=integer(input),outputTokens=integer(output);if(inputTokens===null&&outputTokens===null)return unknown();
  return {status:'supported',observed:true,inputTokens,outputTokens,windowTokens:null,evidence:[...(inputTokens?[evidence(ctx,inputPath)]:[]),...(outputTokens?[evidence(ctx,outputPath)]:[])]};
}
function modalities(values,ctx,path){
  if(!Array.isArray(values)||!values.length)return unknown();
  const names=values.slice(0,12).map(value=>publicText(value,ctx.secrets,30).toLowerCase());
  return {status:'supported',observed:true,modalities:[...new Set(names)],evidence:[evidence(ctx,path)]};
}
function compatibility(model){
  const {input,output}=model.capabilities;
  if(input.observed&&output.observed){const text=input.modalities.includes('text')&&output.modalities.includes('text');model.compatibility={status:text?'supported':'unsupported',reason:text?'Provider metadata reports text input and output; execution remains unverified.':'Provider metadata does not report text input and output together.'};}
  return model;
}
function openAIModel(item,ctx){
  const model=resource(item.id,item.display_name||item.name||item.id,ctx);
  if(item.owned_by!==undefined)model.owner=publicText(item.owned_by,ctx.secrets,160);
  if(Number.isSafeInteger(item.created))model.created=item.created;
  if(item.shutdown_date!==undefined&&item.shutdown_date!==null)model.shutdownDate=publicText(item.shutdown_date,ctx.secrets,40);
  return model;
}
function anthropicModel(item,ctx){
  const model=resource(item.id,item.display_name||item.id,ctx),caps=item.capabilities;
  model.capabilities.context=contextCapability(item.max_input_tokens,item.max_tokens,ctx,'max_input_tokens','max_tokens');
  if(caps&&typeof caps==='object'){
    const thinking=booleanCapability(caps.thinking?.supported,ctx,'capabilities.thinking.supported');
    const effort=booleanCapability(caps.effort?.supported,ctx,'capabilities.effort.supported');
    const levels=['low','medium','high','xhigh','max'].filter(level=>caps.effort?.[level]?.supported===true);
    const modes=thinking.status==='supported'?['adaptive','enabled','disabled'].filter(mode=>caps.thinking?.types?.[mode]?.supported===true):[];
    model.capabilities.reasoning={...thinking,levels:effort.status==='supported'?levels:[],modes,effort};
    model.thinking=effort.status==='supported'?levels:[];
    model.capabilities.nativeWeb=booleanCapability(caps.server_tools?.web_search?.supported,ctx,'capabilities.server_tools.web_search.supported');
    model.capabilities.nativeCodeExecution=booleanCapability(caps.server_tools?.code_execution?.supported,ctx,'capabilities.server_tools.code_execution.supported');
    model.capabilities.structuredOutput=booleanCapability(caps.structured_outputs?.supported,ctx,'capabilities.structured_outputs.supported');
    model.capabilities.citations=booleanCapability(caps.citations?.supported,ctx,'capabilities.citations.supported');
    model.capabilities.batch=booleanCapability(caps.batch?.supported,ctx,'capabilities.batch.supported');
    model.capabilities.imageInput=booleanCapability(caps.image_input?.supported,ctx,'capabilities.image_input.supported');
    model.capabilities.pdfInput=booleanCapability(caps.pdf_input?.supported,ctx,'capabilities.pdf_input.supported');
  }
  if(['active','deprecated','retired'].includes(item.lifecycle))model.lifecycle=item.lifecycle;
  return model;
}
function geminiModel(item,ctx){
  const model=resource(item.name,item.displayName||item.name,ctx);
  model.capabilities.context=contextCapability(item.inputTokenLimit,item.outputTokenLimit,ctx,'inputTokenLimit','outputTokenLimit');
  model.capabilities.reasoning=booleanCapability(item.thinking,ctx,'thinking');model.thinking=[];
  if(Array.isArray(item.supportedGenerationMethods)){
    model.methods=item.supportedGenerationMethods.slice(0,30).map(value=>publicText(value,ctx.secrets,80));
    if(!model.methods.includes('generateContent'))model.compatibility={status:'unsupported',reason:'The listed methods do not include generateContent.'};
  }
  for(const field of ['temperature','maxTemperature','topP','topK'])if(typeof item[field]==='number'&&Number.isFinite(item[field])&&item[field]>=0){model.parameters||={};model.parameters[field]={value:item[field],evidence:[evidence(ctx,field)]};}
  return model;
}
function bedrockModel(item,ctx){
  const model=resource(item.modelId,item.modelName||item.modelId,ctx);
  model.capabilities.input=modalities(item.inputModalities,ctx,'inputModalities');model.capabilities.output=modalities(item.outputModalities,ctx,'outputModalities');
  model.capabilities.streaming=booleanCapability(item.responseStreamingSupported,ctx,'responseStreamingSupported');
  if(item.providerName)model.owner=publicText(item.providerName,ctx.secrets,160);
  if(Array.isArray(item.inferenceTypesSupported))model.inferenceTypes=item.inferenceTypesSupported.slice(0,12).map(value=>publicText(value,ctx.secrets,60));
  if(item.modelLifecycle?.status)model.lifecycle=publicText(item.modelLifecycle.status,ctx.secrets,40);
  return compatibility(model);
}
function azureModel(item,ctx){
  const model=resource(item.name,item.name,ctx,'deployment');
  const declared=item.properties?.model;
  if(declared&&typeof declared==='object'){model.baseModel={name:publicText(declared.name,ctx.secrets,200)};if(declared.version)model.baseModel.version=publicText(declared.version,ctx.secrets,80);if(declared.format)model.baseModel.format=publicText(declared.format,ctx.secrets,60);}
  if(item.properties?.provisioningState)model.provisioningState=publicText(item.properties.provisioningState,ctx.secrets,50);
  return model;
}
function bedrockProfile(item,ctx){
  const model=resource(item.inferenceProfileId,item.inferenceProfileName||item.inferenceProfileId,ctx,'inference-profile');
  if(item.status)model.lifecycle=publicText(item.status,ctx.secrets,40);
  if(item.type)model.profileType=publicText(item.type,ctx.secrets,40);
  if(Array.isArray(item.models))model.members=item.models.slice(0,100).map(member=>publicText(member.modelArn,ctx.secrets,500));
  return model;
}
function assertNoSecret(value,secrets){if(secrets.some(key=>key&&JSON.stringify(value).includes(key)))fail('credential-echo');}
function httpError(status,provider){
  const code=status===401?'authentication-required':status===403?'access-denied':status===404||status===405?'catalog-unavailable':status===429?'rate-limited':status>=500?'provider-unavailable':status>=300&&status<400?'redirect-blocked':'invalid-response';fail(code,provider,status);
}
async function boundedJSON(response,limit,total,ctx){
  const length=Number(response.headers?.get('content-length'));if(Number.isFinite(length)&&length>limit)fail('response-too-large',ctx.scope.provider);
  const type=response.headers?.get('content-type');if(type&&!/json/i.test(type))fail('invalid-response',ctx.scope.provider);
  if(!response.body?.getReader)fail('invalid-response',ctx.scope.provider);
  const reader=response.body.getReader(),chunks=[];let bytes=0;
  try{while(true){const next=await reader.read();if(next.done)break;bytes+=next.value.byteLength;total.bytes+=next.value.byteLength;if(bytes>limit||total.bytes>total.limit)fail('response-too-large',ctx.scope.provider);chunks.push(next.value);}}
  catch(failure){try{await reader.cancel();}catch{}throw failure;}
  finally{reader.releaseLock();}
  const merged=new Uint8Array(bytes);let offset=0;for(const chunk of chunks){merged.set(chunk,offset);offset+=chunk.byteLength;}
  try{const data=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(merged));if(!data||Array.isArray(data)||typeof data!=='object')fail('invalid-response');return data;}catch(failure){if(failure instanceof ProviderDiscoveryError)throw failure;fail('invalid-response',ctx.scope.provider);}
}

/** Read-only catalog discovery. Credentials are transient arguments and never returned or logged. */
export async function discoverProviderCatalog(config,options={}) {
  if(!config||!PROVIDERS.has(config.provider))fail('invalid-config');
  const provider=config.provider,apiKey=secret(config.apiKey,!(provider==='azure'&&config.managementToken||provider==='bedrock'&&config.authMethod==='aws-profile'));
  const managementToken=secret(config.managementToken,false),secrets=[apiKey,managementToken].filter(Boolean);
  const endpoint=endpointFor(config),observedAt=new Date().toISOString();
  const scope={provider,endpoint,region:config.region||null,deployment:config.deployment||null,connectionId:config.connectionId||null,apiVersion:provider==='anthropic'?config.apiVersion||'2023-06-01':provider==='gemini'?endpoint.split('/').at(-1):provider==='azure'?managementToken?config.apiVersion||'2025-06-01':'v1':'catalog'};
  if((provider==='anthropic'||provider==='azure'&&managementToken)&&!/^\d{4}-\d{2}-\d{2}$/.test(scope.apiVersion))fail('invalid-config',provider);
  for(const value of [scope.region,scope.deployment,scope.connectionId])if(value!==null)publicText(value,secrets,200);assertNoSecret(scope,secrets);
  const ctx={scope,observedAt,secrets,documentation:DOCS[provider]||null,operation:provider==='azure'?'Deployments.List':provider==='bedrock'?'ListFoundationModels':'models.list'};
  const result={schemaVersion:1,scope,source:{kind:'provider-metadata',operation:ctx.operation,documentation:ctx.documentation},observedAt,status:'complete',models:[],limitations:[],authentication:'unknown',entitlement:'unknown',usage:'unknown',credits:'unknown'};
  if(provider==='bedrock'&&config.authMethod==='aws-profile'){
    result.status='unavailable';result.limitations.push({code:'aws-signer-required',documentation:DOCS.bedrockKey});return result;
  }
  let nextURL=`${endpoint}/models`,headers={Accept:'application/json'};
  let azureManagement=false,bedrockProfiles=false;
  if(provider==='azure'){
    const fields=[config.subscriptionId,config.resourceGroup,config.accountName];
    if(managementToken&&fields.every(value=>typeof value==='string'&&/^[A-Za-z0-9_.()-]{1,128}$/.test(value))){
      azureManagement=true;fields.forEach(value=>publicText(value,secrets,128));
      nextURL=`https://management.azure.com/subscriptions/${encodeURIComponent(fields[0])}/resourceGroups/${encodeURIComponent(fields[1])}/providers/Microsoft.CognitiveServices/accounts/${encodeURIComponent(fields[2])}/deployments?api-version=${encodeURIComponent(scope.apiVersion)}`;headers.Authorization=`Bearer ${managementToken}`;
    }else {
      if(managementToken)fail('invalid-config',provider);
      if(!apiKey)fail('invalid-config',provider);
      const base=endpoint.endsWith('/openai/v1')?endpoint:`${endpoint}/openai/v1`;nextURL=`${base}/models`;headers['api-key']=apiKey;
      ctx.operation='OpenAI.ListModels';ctx.documentation=DOCS.azureModels;
      result.source={kind:'provider-metadata',operation:ctx.operation,documentation:ctx.documentation};
    }
  }else if(provider==='anthropic'){
    headers['x-api-key']=apiKey;headers['anthropic-version']=scope.apiVersion;nextURL+='?limit=100';
  }else if(provider==='gemini'){headers['x-goog-api-key']=apiKey;nextURL+='?pageSize=100';}
  else {headers.Authorization=`Bearer ${apiKey}`;if(provider==='bedrock')nextURL=`${endpoint}/foundation-models`;}
  const fetchImpl=options.fetchImpl||((url,requestOptions)=>pinnedFetch(url,requestOptions,config.allowLoopbackHTTP===true));if(typeof fetchImpl!=='function')fail('invalid-config',provider);
  const timeoutMs=bound(options.timeoutMs,10000,20,60000),maxPages=bound(options.maxPages,20,1,100),maxModels=bound(options.maxModels,2000,1,10000),maxResponseBytes=bound(options.maxResponseBytes,2*1024*1024,100,16*1024*1024);
  const total={bytes:0,limit:bound(options.maxTotalBytes,8*1024*1024,100,64*1024*1024)};
  const controller=new AbortController();let timedOut=false;const external=options.signal;
  if(external&&typeof external.addEventListener!=='function')fail('invalid-config',provider);
  const abort=()=>controller.abort();if(external?.aborted)fail('cancelled',provider);external?.addEventListener('abort',abort,{once:true});
  const timer=setTimeout(()=>{timedOut=true;controller.abort();},timeoutMs);
  let rejectAbort;const interrupted=new Promise((_,reject)=>rejectAbort=reject);controller.signal.addEventListener('abort',()=>rejectAbort(new ProviderDiscoveryError(timedOut?'timeout':'cancelled',{provider})),{once:true});
  const seenURLs=new Set(),seenIds=new Set();let pages=0,completedPages=0;
  const baseURL=new URL(nextURL),allowedPath=baseURL.pathname;
  const load=async url=>{
    const parsed=new URL(url);if(parsed.origin!==baseURL.origin||parsed.pathname!==allowedPath&&!(provider==='bedrock'&&parsed.pathname==='/inference-profiles')||parsed.username||parsed.password||parsed.hash)fail('endpoint-blocked',provider);
    const response=await fetchImpl(parsed.href,{method:'GET',headers,redirect:'manual',signal:controller.signal});
    if(response.redirected||response.status>=300&&response.status<400)fail('redirect-blocked',provider,response.status);
    if(!response.ok)httpError(response.status,provider);
    if(response.url&&new URL(response.url).origin!==baseURL.origin)fail('redirect-blocked',provider);
    return boundedJSON(response,maxResponseBytes,total,ctx);
  };
  try{
    while(nextURL){
      if(pages>=maxPages){result.status='partial';result.limitations.push({code:'page-limit-reached'});break;}
      if(seenURLs.has(nextURL))fail('invalid-response',provider);seenURLs.add(nextURL);pages++;
      const page=await Promise.race([load(nextURL),interrupted]);
      const items=provider==='gemini'?page.models:provider==='azure'&&azureManagement?page.value:provider==='bedrock'?bedrockProfiles?page.inferenceProfileSummaries:page.modelSummaries:page.data;
      if(!Array.isArray(items)||items.length>10000)fail('invalid-response',provider);
      for(const item of items){
        if(!item||typeof item!=='object'||Array.isArray(item))fail('invalid-response',provider);
        const model=provider==='anthropic'?anthropicModel(item,ctx):provider==='gemini'?geminiModel(item,ctx):provider==='azure'&&azureManagement?azureModel(item,ctx):provider==='bedrock'?bedrockProfiles?bedrockProfile(item,ctx):bedrockModel(item,ctx):openAIModel(item,ctx);
        const resourceKey=`${model.kind}:${model.id}`;
        assertNoSecret(model,secrets);if(seenIds.has(resourceKey))continue;
        if(result.models.length===maxModels){result.status='partial';result.limitations.push({code:'model-limit-reached'});nextURL=null;break;}
        seenIds.add(resourceKey);result.models.push(model);
      }
      completedPages++;
      if(result.status==='partial')break;
      nextURL=null;
      if(provider==='anthropic'&&page.has_more===true){const cursor=publicText(page.last_id,secrets,200);const url=new URL(`${endpoint}/models`);url.searchParams.set('limit','100');url.searchParams.set('after_id',cursor);nextURL=url.href;}
      if(provider==='gemini'&&page.nextPageToken){const cursor=publicText(page.nextPageToken,secrets,2048);const url=new URL(`${endpoint}/models`);url.searchParams.set('pageSize','100');url.searchParams.set('pageToken',cursor);nextURL=url.href;}
      if(provider==='azure'&&azureManagement&&page.nextLink)nextURL=publicText(page.nextLink,secrets,2048);
      if(provider==='bedrock'&&config.includeInferenceProfiles!==false){
        if(!bedrockProfiles){bedrockProfiles=true;ctx.operation='ListInferenceProfiles';ctx.documentation='https://docs.aws.amazon.com/bedrock/latest/APIReference/API_ListInferenceProfiles.html';nextURL=`${endpoint}/inference-profiles?maxResults=1000`;}
        else if(page.nextToken){const cursor=publicText(page.nextToken,secrets,2048);const url=new URL(`${endpoint}/inference-profiles`);url.searchParams.set('maxResults','1000');url.searchParams.set('nextToken',cursor);nextURL=url.href;}
      }
      if(['openai','compatible'].includes(provider)&&page.has_more===true){result.status='partial';result.limitations.push({code:'undocumented-pagination'});}
    }
    if(provider==='azure'&&!azureManagement&&config.deployment){
      const manual=resource(config.deployment,config.deployment,ctx,'deployment','user-declaration');manual.availability='unverified';
      if(!seenIds.has(`deployment:${manual.id}`)){
        if(result.models.length<maxModels)result.models.push(manual);
        else {result.status='partial';if(!result.limitations.some(item=>item.code==='model-limit-reached'))result.limitations.push({code:'model-limit-reached'});}
      }
      result.limitations.push({code:'azure-deployment-unverified',documentation:DOCS.azureModels});
    }
    result.pages=pages;assertNoSecret(result,secrets);return result;
  }catch(failure){
    const safeFailure=failure instanceof ProviderDiscoveryError?failure:new ProviderDiscoveryError(controller.signal.aborted?timedOut?'timeout':'cancelled':'network-error',{provider});
    if(completedPages&&recoverableCatalogError(safeFailure.code)){
      result.status='partial';result.limitations.push({code:safeFailure.code});
      if(provider==='bedrock'&&bedrockProfiles&&['access-denied','catalog-unavailable'].includes(safeFailure.code))result.limitations.push({code:'bedrock-profile-catalog-unavailable'});
      result.pages=pages;assertNoSecret(result,secrets);return result;
    }
    if(provider==='azure'&&config.deployment&&['authentication-required','access-denied','catalog-unavailable'].includes(safeFailure.code)){
      result.models=[resource(config.deployment,config.deployment,ctx,'deployment','user-declaration')];result.models[0].availability='unverified';result.status='unavailable';result.limitations.push({code:safeFailure.code},{code:'azure-deployment-unverified',documentation:ctx.documentation});result.pages=pages;assertNoSecret(result,secrets);return result;
    }
    throw safeFailure;
  }
  finally{clearTimeout(timer);external?.removeEventListener('abort',abort);controller.abort();}
}

// Kept separate so a future trusted AWS signer can normalize native catalog responses.
export function normalizeBedrockCatalog(payload,scope,{observedAt=new Date().toISOString()}={}){
  if(!Array.isArray(payload?.modelSummaries)||payload.modelSummaries.length>2000)fail('invalid-response','bedrock');
  const normalizedScope={provider:'bedrock',endpoint:endpointFor({provider:'bedrock',region:scope.region}),region:scope.region,deployment:scope.deployment||null,connectionId:scope.connectionId||null,apiVersion:'catalog'};
  const ctx={scope:normalizedScope,observedAt,secrets:[],documentation:DOCS.bedrock,operation:'ListFoundationModels'};
  return payload.modelSummaries.map(item=>bedrockModel(item,ctx));
}

function boundedMetadata(value,secrets,depth=0){
  if(depth>4)fail('invalid-response');
  if(value===null||typeof value==='boolean'||typeof value==='number'&&Number.isFinite(value))return value;
  if(typeof value==='string')return publicText(value,secrets,500);
  if(Array.isArray(value)&&value.length<=32)return value.map(item=>boundedMetadata(item,secrets,depth+1));
  if(value&&typeof value==='object'&&Object.keys(value).length<=32){const result={};for(const [key,entry] of Object.entries(value)){if(['__proto__','constructor','prototype'].includes(key))fail('invalid-response');result[publicText(key,secrets,80)]=boundedMetadata(entry,secrets,depth+1);}return result;}
  fail('invalid-response');
}
/** Catalog only through a caller-owned, already initialized runtime request transport. */
export async function discoverRuntimeCatalog(config,options={}){
  if(typeof config?.request!=='function')fail('invalid-config');
  const secrets=Array.isArray(options.redactValues)?options.redactValues.map(value=>secret(value)):[];
  const observedAt=new Date().toISOString();
  const scope={provider:'codex-runtime',connectionId:config.connectionId?publicText(config.connectionId,secrets,200):null,runtimeVersion:config.runtimeVersion?publicText(config.runtimeVersion,secrets,80):null};
  const ctx={scope,observedAt,secrets,documentation:'https://developers.openai.com/codex/app-server',operation:'model/list'};
  const maxPages=bound(options.maxPages,20,1,100),maxModels=bound(options.maxModels,2000,1,10000),timeoutMs=bound(options.timeoutMs,10000,20,60000);
  const controller=new AbortController(),signal=options.signal;let timedOut=false;
  if(signal&&typeof signal.addEventListener!=='function')fail('invalid-config');
  const abort=()=>controller.abort();if(signal?.aborted)fail('cancelled');signal?.addEventListener('abort',abort,{once:true});
  const timer=setTimeout(()=>{timedOut=true;controller.abort();},timeoutMs);
  let rejectAbort;const interrupted=new Promise((_,reject)=>rejectAbort=reject);controller.signal.addEventListener('abort',()=>rejectAbort(new ProviderDiscoveryError(timedOut?'timeout':'cancelled')),{once:true});
  const result={schemaVersion:1,scope,source:{kind:'runtime-observation',operation:'model/list',documentation:ctx.documentation},observedAt,status:'complete',models:[],providerCapabilities:null,limitations:[],authentication:'unknown',entitlement:'unknown',usage:'unknown',credits:'unknown'};
  const seenCursors=new Set(),seenIds=new Set();let cursor=null,pages=0;
  const request=(method,params)=>Promise.race([Promise.resolve().then(()=>config.request(method,params,{signal:controller.signal})),interrupted]);
  try{
    do{
      if(pages===maxPages){result.status='partial';result.limitations.push({code:'page-limit-reached'});break;}
      const page=await request('model/list',{cursor,limit:100,includeHidden:config.includeHidden!==false});pages++;
      if(!Array.isArray(page?.data)||page.data.length>1000)fail('invalid-response');
      for(const item of page.data){
        const model=resource(item.model,item.displayName||item.display_name||item.model,ctx);model.source.kind='runtime-observation';model.runtimeId=item.id?publicText(item.id,secrets,200):model.id;
        if(seenIds.has(model.id))continue;
        if(result.models.length===maxModels){result.status='partial';result.limitations.push({code:'model-limit-reached'});break;}
        model.capabilities.input=modalities(item.inputModalities,ctx,'inputModalities');model.capabilities.input.evidence.forEach(entry=>entry.kind='runtime-observation');
        if(Array.isArray(item.supportedReasoningEfforts)){
          const levels=item.supportedReasoningEfforts.slice(0,30).map(value=>publicText(typeof value==='string'?value:value?.reasoningEffort,secrets,40));
          model.capabilities.reasoning={status:levels.length?'supported':'unsupported',observed:true,levels:[...new Set(levels)],defaultLevel:item.defaultReasoningEffort?publicText(item.defaultReasoningEffort,secrets,40):null,evidence:[evidence(ctx,'supportedReasoningEfforts','runtime-observation')]};model.thinking=[...model.capabilities.reasoning.levels];
        }
        if(item.defaultReasoningEffort!==undefined&&item.defaultReasoningEffort!==null)model.defaultReasoningEffort={value:publicText(item.defaultReasoningEffort,secrets,40),evidence:[evidence(ctx,'defaultReasoningEffort','runtime-observation')]};
        for(const field of ['serviceTiers','defaultServiceTier','modelSpecialty','multiAgentVersion'])if(item[field]!==undefined)model[field]={value:boundedMetadata(item[field],secrets),evidence:[evidence(ctx,field,'runtime-observation')]};
        if(typeof item.isDefault==='boolean')model.isDefault=item.isDefault;
        if(typeof item.hidden==='boolean')model.hidden=item.hidden;
        assertNoSecret(model,secrets);seenIds.add(model.id);result.models.push(model);
      }
      if(result.status==='partial')break;
      cursor=page.nextCursor===null||page.nextCursor===undefined?null:publicText(page.nextCursor,secrets,2000);
      if(cursor&&seenCursors.has(cursor))fail('invalid-response');if(cursor)seenCursors.add(cursor);
    }while(cursor);
    try{
      const response=await request('modelProvider/capabilities/read',{});const caps=response;
      const providerScope={...scope,modelProvider:config.modelProvider?publicText(config.modelProvider,secrets,80):null};
      const providerContext={...ctx,scope:providerScope,operation:'modelProvider/capabilities/read'};
      result.providerCapabilities={scope:providerScope,source:'runtime-observation',observedAt,...Object.fromEntries(['webSearch','imageGeneration','namespaceTools'].map(field=>[field,booleanCapability(caps?.[field],providerContext,field)]))};
      for(const field of ['webSearch','imageGeneration','namespaceTools'])result.providerCapabilities[field].evidence.forEach(entry=>entry.kind='runtime-observation');
    }catch(failure){if(controller.signal.aborted||failure instanceof ProviderDiscoveryError)throw failure;result.limitations.push({code:'runtime-provider-capabilities-unavailable'});}
    result.pages=pages;assertNoSecret(result,secrets);return result;
  }catch(failure){
    const safeFailure=failure instanceof ProviderDiscoveryError?failure:new ProviderDiscoveryError(controller.signal.aborted?timedOut?'timeout':'cancelled':'provider-unavailable');
    if(pages&&recoverableCatalogError(safeFailure.code)){result.status='partial';result.pages=pages;result.limitations.push({code:safeFailure.code});assertNoSecret(result,secrets);return result;}
    throw safeFailure;
  }
  finally{clearTimeout(timer);signal?.removeEventListener('abort',abort);}
}
