import {randomBytes,timingSafeEqual} from 'node:crypto';
import {discoverProviderCatalog} from '../../tools/provider-discovery/src/index.mjs';

// A read-only host boundary. Credentials live only for the duration of a request.
export function createDiscoveryHandler({discover=discoverProviderCatalog,maxConcurrent=3,timeoutMs=15000}={}) {
  const token=randomBytes(32).toString('hex');
  let active=0;
  const starts=[];
  const json=(res,status,body)=>{
    if(res.destroyed)return;
    res.writeHead(status,{'Content-Type':'application/json; charset=utf-8','Cache-Control':'no-store','X-Content-Type-Options':'nosniff','Cross-Origin-Resource-Policy':'same-origin'});
    res.end(JSON.stringify(body));
  };
  const fail=(res,status,code)=>json(res,status,{error:{code}});
  function originAllowed(req,withOrigin) {
    const address=req.socket.localAddress;
    if(!['127.0.0.1','::1','::ffff:127.0.0.1'].includes(address))return false;
    const host=req.headers.host;
    if(![`127.0.0.1:${req.socket.localPort}`,`localhost:${req.socket.localPort}`].includes(host))return false;
    if(req.headers['sec-fetch-site']&&req.headers['sec-fetch-site']!=='same-origin')return false;
    return withOrigin?req.headers.origin===`http://${host}`:(!req.headers.origin||req.headers.origin===`http://${host}`);
  }
  function authorized(req) {
    const candidate=req.headers['x-orchestrix-discovery-token'];
    return typeof candidate==='string'&&/^[a-f0-9]{64}$/.test(candidate)&&timingSafeEqual(Buffer.from(candidate),Buffer.from(token));
  }
  return async function handle(req,res,pathname) {
    if(!['/api/discovery/session','/api/discovery/catalog'].includes(pathname))return false;
    if(!originAllowed(req,pathname.endsWith('/catalog'))){fail(res,403,'origin-denied');return true;}
    if(pathname.endsWith('/session')) {
      if(req.method!=='GET'){fail(res,405,'method-denied');return true;}
      json(res,200,{token,protocolVersion:1,readOnly:true});return true;
    }
    if(req.method!=='POST'){fail(res,405,'method-denied');return true;}
    if(!authorized(req)){fail(res,403,'session-denied');return true;}
    if(req.headers['content-type']!=='application/json'){fail(res,415,'content-type-denied');return true;}
    const now=Date.now();while(starts[0]<now-60000)starts.shift();
    if(active>=maxConcurrent||starts.length>=30){fail(res,429,'busy');return true;}
    if(Number(req.headers['content-length'])>16384){fail(res,413,'request-too-large');return true;}
    const controller=new AbortController();active++;starts.push(now);
    const abort=()=>{if(!res.writableEnded)controller.abort();};
    req.once('aborted',abort);res.once('close',abort);
    const timer=setTimeout(()=>{controller.abort();if(!req.complete)req.destroy();},timeoutMs);
    let config;
    try {
      let size=0;const chunks=[];
      for await(const chunk of req){size+=chunk.length;if(size>16384){fail(res,413,'request-too-large');return true;}chunks.push(chunk);}
      let input;try{input=JSON.parse(Buffer.concat(chunks).toString('utf8'));}catch{fail(res,400,'invalid-request');return true;}
      if(!input||Array.isArray(input)||!['openai','anthropic','gemini','azure','bedrock','compatible'].includes(input.provider)){fail(res,400,'invalid-request');return true;}
      config={};
      for(const field of ['provider','endpoint','connectionId','region','deployment','authMethod','apiVersion','apiKey']) {
        if(input[field]!==undefined&&input[field]!==null){if(typeof input[field]!=='string'||input[field].length>(field==='apiKey'?8192:400)){fail(res,400,'invalid-request');return true;}config[field]=input[field];}
      }
      // Selecting a loopback URL in the compatible-provider form is the explicit local-backend choice.
      if(config.provider==='compatible')try{const endpoint=new URL(config.endpoint);config.allowLoopbackHTTP=endpoint.protocol==='http:'&&['localhost','127.0.0.1','[::1]'].includes(endpoint.hostname);}catch{}
      const result=await discover(config,{signal:controller.signal,timeoutMs:12000,maxPages:20,maxModels:500,maxResponseBytes:1048576});
      // Refuse accidental credential echoes, including non-model metadata.
      if(config.apiKey&&[config.apiKey,JSON.stringify(config.apiKey).slice(1,-1)].some(marker=>JSON.stringify(result).includes(marker)))throw new Error('unsafe-response');
      json(res,200,result);
    } catch(error) {
      const known=new Set(['authentication-required','access-denied','rate-limited','endpoint-blocked','catalog-unavailable','provider-unavailable','network-error','response-too-large','credential-echo','invalid-config','redirect-blocked','invalid-response','unsupported-auth','timeout','cancelled','limit-exceeded']);
      const code=controller.signal.aborted?'timeout':known.has(error?.code)?error.code:'discovery-failed';
      fail(res,code==='authentication-required'?401:code==='access-denied'?403:code==='rate-limited'?429:502,code);
    } finally {
      if(config)config.apiKey=undefined;
      clearTimeout(timer);req.off('aborted',abort);res.off('close',abort);active--;
    }
    return true;
  };
}
