/* Same-origin transport for read-only resource discovery; never persists credentials. */
(() => {
  'use strict';
  const failure=code=>Object.assign(new Error(code),{code});
  async function discover(config) {
    if(!/^https?:$/.test(location.protocol))throw failure('host-unavailable');
    const info=await fetch('/api/discovery/session',{cache:'no-store',credentials:'same-origin',signal:config.signal});
    if(!info.ok)throw failure('host-unavailable');
    const session=await info.json();
    if(session.protocolVersion!==1||!/^([a-f0-9]{64})$/.test(session.token))throw failure('host-unavailable');
    const response=await fetch('/api/discovery/catalog',{
      method:'POST',credentials:'same-origin',cache:'no-store',signal:config.signal,
      headers:{'Content-Type':'application/json','X-Orchestrix-Discovery-Token':session.token},
      body:JSON.stringify({provider:config.provider,endpoint:config.endpoint,connectionId:config.connectionId,region:config.region,deployment:config.deployment,authMethod:config.authMethod,apiVersion:config.apiVersion,apiKey:config.credential})
    });
    let result;try{result=await response.json();}catch{throw failure('invalid-response');}
    if(!response.ok)throw failure(result?.error?.code||'discovery-failed');
    return result;
  }
  window.OrchestrixDiscovery=Object.freeze({discover,timeoutMs:16000});
})();
