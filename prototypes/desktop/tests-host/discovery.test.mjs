import test from 'node:test';
import assert from 'node:assert/strict';
import {createServer, request as httpRequest} from 'node:http';
import {once} from 'node:events';
import {setTimeout as delay} from 'node:timers/promises';
import {readFileSync} from 'node:fs';
import {runInNewContext} from 'node:vm';
import {createDiscoveryHandler} from '../host-discovery.mjs';

// Every provider transport is injected. HTTP requests target only an ephemeral
// loopback server; these tests neither read credentials nor contact providers.
const fixtureKey = 'unit-test-key-not-a-provider-credential';
const catalog = () => ({schemaVersion:1,status:'complete',models:[],limitations:[],usage:'unknown',credits:'unknown'});
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve=yes; reject=no; });
  return {promise,resolve,reject};
};

async function fixture(t, discover=async()=>catalog(), options={}) {
  const calls=[], uncaught=[];
  const handler=createDiscoveryHandler({
    ...options,
    discover:async(config, bounds)=>{
      calls.push({config:{...config},bounds});
      return discover(config,bounds);
    }
  });
  const server=createServer((req,res)=>{
    const pathname=new URL(req.url,'http://127.0.0.1').pathname;
    Promise.resolve(handler(req,res,pathname)).then(handled=>{
      if(!handled){res.writeHead(404);res.end();}
    }).catch(error=>{
      // Keep a regression from crashing the test runner. Tests require this list
      // to remain empty and still observe an unexpected 500 when the host throws.
      uncaught.push(error);
      if(!res.destroyed&&!res.writableEnded){res.writeHead(500,{'Content-Type':'application/json'});res.end('{"error":{"code":"uncaught-handler"}}');}
    });
  });
  server.listen(0,'127.0.0.1');
  await once(server,'listening');
  const port=server.address().port, base=`http://127.0.0.1:${port}`;
  t.after(async()=>{
    server.closeAllConnections();
    await new Promise(resolve=>server.close(resolve));
  });
  async function request({method='GET',path='/api/discovery/session',headers={},body}={}) {
    return new Promise((resolve,reject)=>{
      const req=httpRequest(`${base}${path}`,{method,headers,agent:false},res=>{
        const chunks=[];
        res.on('data',chunk=>chunks.push(chunk));
        res.on('end',()=>{
          const text=Buffer.concat(chunks).toString('utf8');
          let json;try{json=JSON.parse(text);}catch{}
          resolve({status:res.statusCode,headers:res.headers,text,json});
        });
        res.on('error',reject);
      });
      req.on('error',reject);
      if(Array.isArray(body))for(const chunk of body)req.write(chunk);
      else if(body!==undefined)req.write(body);
      req.end();
    });
  }
  const session=await request();
  assert.equal(session.status,200);
  const token=session.json.token;
  const headers={'Origin':base,'Sec-Fetch-Site':'same-origin','Content-Type':'application/json','X-Orchestrix-Discovery-Token':token};
  const post=(input={provider:'openai',apiKey:fixtureKey},extra={})=>request({method:'POST',path:'/api/discovery/catalog',headers:{...headers,...extra.headers},body:extra.body??JSON.stringify(input)});
  return {base,port,server,request,post,session,token,headers,calls,uncaught};
}

test('same-origin session supplies a fresh opaque token with non-cacheable headers',async t=>{
  const one=await fixture(t), two=await fixture(t);
  assert.match(one.token,/^[a-f0-9]{64}$/);
  assert.notEqual(one.token,two.token);
  assert.equal(one.session.json.protocolVersion,1);
  assert.equal(one.session.json.readOnly,true);
  assert.equal(one.session.headers['cache-control'],'no-store');
  assert.equal(one.session.headers['x-content-type-options'],'nosniff');
  assert.equal(one.session.headers['cross-origin-resource-policy'],'same-origin');
  assert.equal(one.session.headers['access-control-allow-origin'],undefined);
  assert.equal(one.calls.length,0);
});

test('valid read catalog uses only the injected discover function and bounded options',async t=>{
  const host=await fixture(t);
  const response=await host.post({provider:'anthropic',apiKey:fixtureKey,region:'unused-region',apiVersion:'2023-06-01',password:'must-not-forward',dangerous:'ignored'});
  assert.equal(response.status,200);
  assert.deepEqual(response.json,catalog());
  assert.equal(host.calls.length,1);
  assert.equal(host.calls[0].config.provider,'anthropic');
  assert.equal(host.calls[0].config.apiKey,fixtureKey);
  assert.equal(host.calls[0].config.apiVersion,'2023-06-01');
  assert.equal(host.calls[0].config.password,undefined);
  assert.equal(host.calls[0].config.dangerous,undefined);
  assert.ok(host.calls[0].bounds.signal instanceof AbortSignal);
  assert.ok(host.calls[0].bounds.maxPages>0);
  assert.ok(host.calls[0].bounds.maxModels>0);
  assert.ok(host.calls[0].bounds.maxResponseBytes>0);
  assert.equal(response.text.includes(fixtureKey),false);
  assert.deepEqual(host.uncaught,[]);
});

test('forged Host is rejected before session or provider discovery',async t=>{
  const host=await fixture(t);
  for(const path of ['/api/discovery/session','/api/discovery/catalog']){
    const response=await host.request({path,method:path.endsWith('catalog')?'POST':'GET',headers:{...host.headers,Host:`attacker.invalid:${host.port}`,Origin:`http://attacker.invalid:${host.port}`},body:'{}'});
    assert.equal(response.status,403);
    assert.deepEqual(response.json,{error:{code:'origin-denied'}});
  }
  assert.equal(host.calls.length,0);
});

test('foreign Origin and cross-site fetch metadata cannot retrieve session or catalog',async t=>{
  const host=await fixture(t);
  for(const headers of [{Origin:'https://attacker.invalid'},{'Sec-Fetch-Site':'cross-site'},{'Sec-Fetch-Site':'same-site'}]){
    const session=await host.request({headers});
    assert.equal(session.status,403);
    const result=await host.post(undefined,{headers});
    assert.equal(result.status,403);
  }
  assert.equal(host.calls.length,0);
});

test('catalog requires an explicit matching Origin even with a valid token',async t=>{
  const host=await fixture(t);
  const response=await host.request({method:'POST',path:'/api/discovery/catalog',headers:{'Content-Type':'application/json','X-Orchestrix-Discovery-Token':host.token},body:'{}'});
  assert.equal(response.status,403);
  assert.equal(response.json.error.code,'origin-denied');
  assert.equal(host.calls.length,0);
});

test('localhost with its exact same origin is accepted on the loopback listener',async t=>{
  const host=await fixture(t);
  const response=await host.post(undefined,{headers:{Host:`localhost:${host.port}`,Origin:`http://localhost:${host.port}`}});
  assert.equal(response.status,200);
  assert.equal(host.calls.length,1);
});

test('missing, malformed, foreign-session and incorrect CSRF tokens are rejected',async t=>{
  const host=await fixture(t), other=await fixture(t);
  for(const token of ['', 'wrong', 'f'.repeat(64), other.token]){
    const response=await host.post(undefined,{headers:{'X-Orchestrix-Discovery-Token':token}});
    assert.equal(response.status,403);
    assert.deepEqual(response.json,{error:{code:'session-denied'}});
  }
  const missing=await host.request({method:'POST',path:'/api/discovery/catalog',headers:{Origin:host.base,'Content-Type':'application/json'},body:'{}'});
  assert.equal(missing.status,403);
  assert.equal(host.calls.length,0);
  assert.deepEqual(host.uncaught,[]);
});

test('a 64-character non-ASCII CSRF token is denied without timingSafeEqual throwing',async t=>{
  const host=await fixture(t);
  const response=await host.post(undefined,{headers:{'X-Orchestrix-Discovery-Token':'é'.repeat(64)}});
  assert.equal(response.status,403);
  assert.deepEqual(response.json,{error:{code:'session-denied'}});
  assert.deepEqual(host.uncaught,[]);
  assert.equal((await host.post()).status,200);
});

test('unsupported methods and media types never invoke discovery',async t=>{
  const host=await fixture(t);
  const session=await host.request({method:'POST',headers:{Origin:host.base},body:'{}'});
  assert.equal(session.status,405);
  const catalogGet=await host.request({path:'/api/discovery/catalog',headers:host.headers});
  assert.equal(catalogGet.status,405);
  const wrongType=await host.post(undefined,{headers:{'Content-Type':'text/plain'}});
  assert.equal(wrongType.status,415);
  assert.equal(host.calls.length,0);
});

test('malformed JSON and invalid config types fail before the adapter',async t=>{
  const host=await fixture(t);
  for(const body of ['{', 'null', '[]','"openai"',JSON.stringify({provider:'unsupported'}),JSON.stringify({provider:'openai',apiKey:7}),JSON.stringify({provider:'openai',endpoint:'a'.repeat(401)}),JSON.stringify({provider:'openai',apiKey:'a'.repeat(8193)})]){
    const response=await host.post(undefined,{body});
    assert.equal(response.status,400);
    assert.deepEqual(response.json,{error:{code:'invalid-request'}});
  }
  assert.equal(host.calls.length,0);
});

test('declared and chunked bodies exceeding 16 KiB are rejected',async t=>{
  const host=await fixture(t);
  const declared=await host.post(undefined,{headers:{'Content-Length':'16385'},body:'x'.repeat(16385)});
  assert.equal(declared.status,413);
  assert.equal(declared.json.error.code,'request-too-large');
  const chunked=await host.post(undefined,{headers:{'Transfer-Encoding':'chunked'},body:['x'.repeat(9000),'y'.repeat(9000)]});
  assert.equal(chunked.status,413);
  assert.equal(chunked.json.error.code,'request-too-large');
  assert.equal(host.calls.length,0);
  assert.deepEqual(host.uncaught,[]);
});

test('credential echoes in non-model metadata and object keys never reach the client',async t=>{
  for(const makeResult of [key=>({...catalog(),scope:{endpoint:`https://${key}.invalid`}}),key=>({...catalog(),metadata:{nested:[key]}}),key=>({...catalog(),[key]:true})]){
    const host=await fixture(t,async config=>makeResult(config.apiKey));
    const response=await host.post();
    assert.equal(response.status,502);
    assert.equal(response.text.includes(fixtureKey),false);
    assert.deepEqual(response.json,{error:{code:'discovery-failed'}});
  }
});

test('credential echo detection also handles JSON-escaped key characters',async t=>{
  const key='fixture-"-\\-credential';
  const host=await fixture(t,async config=>({...catalog(),metadata:{credential:config.apiKey}}));
  const response=await host.post({provider:'openai',apiKey:key});
  assert.equal(response.status,502);
  assert.deepEqual(response.json,{error:{code:'discovery-failed'}});
  assert.equal(response.text.includes('fixture-'),false);
});

test('errors expose only a permitted code and never provider message, stack or secrets',async t=>{
  for(const [code,status] of [['authentication-required',401],['access-denied',403],['rate-limited',429],['arbitrary-error-code',502]]){
    const host=await fixture(t,async()=>{throw Object.assign(new Error(`provider says ${fixtureKey}`),{code,metadata:{key:fixtureKey}});});
    const response=await host.post();
    assert.equal(response.status,status);
    assert.deepEqual(response.json,{error:{code:code==='arbitrary-error-code'?'discovery-failed':code}});
    assert.equal(response.text.includes(fixtureKey),false);
    assert.equal(response.json.error.message,undefined);
    assert.equal(response.json.error.stack,undefined);
    assert.deepEqual(host.uncaught,[]);
  }
});

test('concurrency rejection does not invoke another adapter and a completed request releases its slot',async t=>{
  const started=deferred(), release=deferred();let count=0;
  const host=await fixture(t,async()=>{
    if(++count===1){started.resolve();await release.promise;}
    return catalog();
  },{maxConcurrent:1});
  const first=host.post();
  await started.promise;
  const busy=await host.post();
  assert.equal(busy.status,429);
  assert.equal(busy.json.error.code,'busy');
  assert.equal(host.calls.length,1);
  release.resolve();
  assert.equal((await first).status,200);
  assert.equal((await host.post()).status,200);
  assert.equal(host.calls.length,2);
});

test('closing the HTTP request cancels the injected discovery and releases its slot',async t=>{
  const started=deferred(), cancelled=deferred();let count=0;
  const host=await fixture(t,async(config,{signal})=>{
    if(++count>1)return catalog();
    started.resolve(signal);
    return new Promise((resolve,reject)=>signal.addEventListener('abort',()=>{
      cancelled.resolve();reject(Object.assign(new Error('unit cancellation'),{code:'cancelled'}));
    },{once:true}));
  },{maxConcurrent:1});
  const req=httpRequest(`${host.base}/api/discovery/catalog`,{method:'POST',headers:host.headers,agent:false});
  req.on('error',()=>{});
  req.end(JSON.stringify({provider:'openai',apiKey:fixtureKey}));
  const signal=await started.promise;
  req.destroy();
  await Promise.race([cancelled.promise,delay(2000).then(()=>{throw new Error('HTTP close did not cancel discovery');})]);
  assert.equal(signal.aborted,true);
  // The abort listener releases its promise before the host finally block.
  await new Promise(resolve=>setImmediate(resolve));
  assert.equal((await host.post()).status,200);
  assert.deepEqual(host.uncaught,[]);
});

test('adapter rejection releases a concurrency slot',async t=>{
  let count=0;
  const host=await fixture(t,async()=>{if(++count===1)throw new Error('private provider detail');return catalog();},{maxConcurrent:1});
  assert.equal((await host.post()).status,502);
  assert.equal((await host.post()).status,200);
});

test('discovery attempts are bounded to 30 per rolling minute',async t=>{
  const host=await fixture(t);
  for(let index=0;index<30;index++)assert.equal((await host.post()).status,200);
  const response=await host.post();
  assert.equal(response.status,429);
  assert.equal(response.json.error.code,'busy');
  assert.equal(host.calls.length,30);
});

test('unrelated routes fall through without session or provider work',async t=>{
  const host=await fixture(t);
  const response=await host.request({path:'/assets/any-file'});
  assert.equal(response.status,404);
  assert.equal(host.calls.length,0);
});

test('a stalled upload is closed by the body deadline and releases its slot', {timeout:5000},async t=>{
  const host=await fixture(t,async()=>catalog(),{maxConcurrent:1});
  t.mock.timers.enable({apis:['setTimeout']});
  const closed=deferred();
  const req=httpRequest(`${host.base}/api/discovery/catalog`,{method:'POST',headers:{...host.headers,'Transfer-Encoding':'chunked'},agent:false});
  req.on('error',error=>closed.resolve(error));
  req.on('close',()=>closed.resolve('closed'));
  req.write('{"provider":"openai",');
  await new Promise(resolve=>setImmediate(resolve));
  const busy=await host.post();
  assert.equal(busy.status,429);
  assert.equal(host.calls.length,0);
  t.mock.timers.tick(15001);
  await closed.promise;
  t.mock.timers.reset();
  await new Promise(resolve=>setImmediate(resolve));
  assert.equal((await host.post()).status,200);
  assert.equal(host.calls.length,1);
  assert.deepEqual(host.uncaught,[]);
});

const clientSource=readFileSync(new URL('../discovery-client.js',import.meta.url),'utf8');
function browserClient(fetch,protocol='http:'){
  const window={};
  runInNewContext(clientSource,{window,fetch,location:{protocol},localStorage:{setItem(){throw new Error('Discovery must not persist a key');}}});
  return window.OrchestrixDiscovery;
}

test('browser transport forwards connection scope, API version, credential and cancellation only to the same-origin host',async()=>{
  const requests=[], result=catalog(), controller=new AbortController();
  const client=browserClient(async(url,options)=>{
    requests.push({url,options});
    return {ok:true,json:async()=>requests.length===1?{protocolVersion:1,token:'a'.repeat(64),readOnly:true}:result};
  });
  assert.equal(await client.discover({provider:'anthropic',connectionId:'scope-fixture',apiVersion:'2023-06-01',credential:fixtureKey,signal:controller.signal}),result);
  assert.deepEqual(requests.map(entry=>entry.url),['/api/discovery/session','/api/discovery/catalog']);
  assert.equal(requests[0].options.signal,controller.signal);
  assert.equal(requests[1].options.signal,controller.signal);
  assert.equal(requests[0].options.credentials,'same-origin');
  assert.equal(requests[1].options.credentials,'same-origin');
  assert.equal(requests[1].options.cache,'no-store');
  assert.equal(requests[1].options.method,'POST');
  assert.equal(requests[1].options.headers['X-Orchestrix-Discovery-Token'],'a'.repeat(64));
  assert.deepEqual(JSON.parse(requests[1].options.body),{provider:'anthropic',connectionId:'scope-fixture',apiVersion:'2023-06-01',apiKey:fixtureKey});
});

test('file entry never makes discovery requests and reports an unavailable host',async()=>{
  let fetches=0;
  const client=browserClient(async()=>{fetches++;throw new Error('Unexpected request');},'file:');
  await assert.rejects(client.discover({provider:'openai',credential:fixtureKey}),error=>error.code==='host-unavailable');
  assert.equal(fetches,0);
});

test('browser rejects invalid session protocol or token before transmitting a credential',async()=>{
  for(const session of [{protocolVersion:2,token:'a'.repeat(64)},{protocolVersion:1,token:'bad'},{protocolVersion:1,token:'é'.repeat(64)}]){
    let fetches=0;
    const client=browserClient(async()=>{fetches++;return {ok:true,json:async()=>session};});
    await assert.rejects(client.discover({provider:'openai',credential:fixtureKey}),error=>error.code==='host-unavailable');
    assert.equal(fetches,1);
  }
});

test('browser reports typed host errors and rejects malformed catalog JSON',async()=>{
  for(const badJSON of [false,true]){
    let fetches=0;
    const client=browserClient(async()=>{
      if(++fetches===1)return {ok:true,json:async()=>({protocolVersion:1,token:'a'.repeat(64)})};
      return {ok:false,json:async()=>{if(badJSON)throw new SyntaxError('private transport detail');return {error:{code:'access-denied'}};}};
    });
    await assert.rejects(client.discover({provider:'openai',credential:fixtureKey}),error=>error.code===(badJSON?'invalid-response':'access-denied'));
    assert.equal(fetches,2);
  }
});
