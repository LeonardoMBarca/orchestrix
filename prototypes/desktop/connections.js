/* Connection configuration. Provider authentication/execution requires a desktop runtime. */
(() => {
  'use strict';
  const {html,text,t}=window.OrchestrixI18n;
  const fixtureControls = () => window.__ORCHESTRIX_TEST_SCENARIO === 'seed';
  const fixtureActions = new Set(['discovery', 'finish-discovery', 'limit', 'expire', 'deny', 'incompatible', 'catalog', 'clear-limit', 'revoke-no-signal']);

  const labels = {
    active: 'Configuração disponível', pending: 'Configuração pendente',
    limited: 'Limite indicado', expired: 'Reautorização necessária',
    denied: 'Acesso negado', disconnected: 'Novos trabalhos bloqueados'
  };
  const modeLabels = {
    'own-plan': 'Assinatura do provedor',
    'api': 'API · cobrança separada'
  };
  const openStatuses = new Set(['running', 'paused', 'unknown', 'blocked']);
  const apiProviders=[
    {id:'openai',name:'OpenAI',endpoint:'https://api.openai.com/v1'},
    {id:'anthropic',name:'Anthropic',endpoint:'https://api.anthropic.com/v1'},
    {id:'gemini',name:'Gemini',endpoint:'https://generativelanguage.googleapis.com/v1beta'},
    {id:'azure',name:'Azure AI',endpoint:''},
    {id:'bedrock',name:'AWS Bedrock',endpoint:''},
    {id:'compatible',name:'OpenAI-compatible',endpoint:''}
  ];
  const runtimeProviders=['Codex','Claude Code','Antigravity'];
  const credentials=new Map();
  const discoveryAdapters=new Map();
  const discoveryJobs=new Map();
  let wizard=null;
  let inspectedId=null;
  let diagnosticsInspectedId=null;
  let latestDiagnosticId=null;
  let diagnosticNoticeTimer=null;
  const providerInfo=id=>apiProviders.find(provider=>provider.id===id);
  const discoveryScope=c=>JSON.stringify([c.apiProvider||c.provider,c.endpoint||'',c.region||'',c.deployment||'',c.authMethod||'api-key']);

  function clearCredentialFields() {document.querySelectorAll('#conn-api-key').forEach(input=>{input.value='';input.removeAttribute('value');});}
  function wizardMode(state,h) {
    clearCredentialFields();
    h.showModal(t('Adicionar conexão'),html`<form id="conn-kind-form" class="conn-form"><fieldset class="context-options"><legend>Como você quer conectar?</legend><label><input type="radio" name="kind" value="own-plan" checked><span><strong>Assinatura ou conta do runtime</strong><small>Use Codex, Claude Code ou Antigravity.</small></span></label><label><input type="radio" name="kind" value="api"><span><strong>Chave de API</strong><small>Configure um provedor com cobrança própria.</small></span></label></fieldset><p class="field-hint">Limites de assinatura nunca ativam uma API automaticamente.</p><div class="form-actions">${button(t('Cancelar'),'wizard-close',null,h)}<button type="submit" class="button primary">Continuar</button></div></form>`);
    const selected=document.querySelector(`#conn-kind-form [value="${wizard.kind==='api'?'api':'own-plan'}"]`);if(selected)selected.checked=true;
  }
  function wizardAccount(state,h) {
    h.showModal(t('Conectar conta do runtime'),html`<form id="conn-account-form" class="conn-form"><label class="form-label" for="conn-runtime">Runtime</label><select id="conn-runtime" name="provider">${runtimeProviders.map(provider=>html`<option value="${h.esc(provider)}">${h.esc(provider)}</option>`).join('')}</select><label class="form-label" for="conn-name">Nome da conexão</label><input id="conn-name" name="name" required maxlength="70" autocomplete="off"><label class="form-label" for="conn-workspace">Workspace</label><input id="conn-workspace" name="workspace" maxlength="70" value="${h.esc(t('Pessoal'))}" autocomplete="off"><p class="field-hint">A autorização e os modelos disponíveis serão confirmados pelo runtime.</p><p class="error-note" id="conn-form-error" role="alert"></p><div class="form-actions">${button(t('Voltar'),'wizard-back',null,h)}<button type="submit" class="button primary">Adicionar conexão</button></div></form>`);
    document.getElementById('conn-runtime').value=wizard.runtimeProvider||runtimeProviders[0];document.getElementById('conn-name').value=wizard.name||'';document.getElementById('conn-workspace').value=wizard.workspace??t('Pessoal');
  }
  function wizardAPI(state,h) {
    const provider=providerInfo(wizard.provider)||apiProviders[0];
    const profile=provider.id==='bedrock'&&wizard.authMethod==='aws-profile';
    h.showModal(t('Conectar API'),html`<form id="conn-api-form" class="conn-form" autocomplete="off"><label class="form-label" for="conn-api-provider">Provedor de API</label><select id="conn-api-provider" name="apiProvider">${apiProviders.map(item=>html`<option value="${item.id}" ${item.id===provider.id?'selected':''}>${h.esc(item.name)}</option>`).join('')}</select><label class="form-label" for="conn-name">Nome da conexão</label><input id="conn-name" name="name" required maxlength="70" value="${h.esc(wizard.name||'')}" autocomplete="off">${['azure','compatible'].includes(provider.id)?html`<label class="form-label" for="conn-api-endpoint">Endpoint da API</label><input id="conn-api-endpoint" name="endpoint" type="url" required maxlength="300" value="${h.esc(wizard.endpoint||'')}" placeholder="https://api.example.com/v1" autocomplete="off">`:''}${provider.id==='azure'?html`<label class="form-label" for="conn-api-deployment">Nome do deployment</label><input id="conn-api-deployment" name="deployment" required maxlength="120" value="${h.esc(wizard.deployment||'')}" autocomplete="off"><p class="field-hint">O deployment identifica o modelo configurado no Azure. A listagem pode exigir permissões de gerenciamento separadas.</p>`:''}${provider.id==='bedrock'?html`<label class="form-label" for="conn-api-region">Região AWS</label><input id="conn-api-region" name="region" required maxlength="40" value="${h.esc(wizard.region||'')}" placeholder="us-east-1" autocomplete="off"><label class="form-label" for="conn-api-auth">Autenticação AWS</label><select id="conn-api-auth" name="authMethod"><option value="api-key" ${!profile?'selected':''}>Chave de API Bedrock</option><option value="aws-profile" ${profile?'selected':''}>Perfil de credenciais AWS</option></select>`:''}${profile?html`<label class="form-label" for="conn-api-profile">Nome do perfil AWS</label><input id="conn-api-profile" name="awsProfile" maxlength="70" value="${h.esc(wizard.awsProfile||'default')}" required autocomplete="off"><p class="field-hint">O runtime desktop usa o perfil AWS configurado no computador. Não informe chaves IAM neste campo.</p>`:html`<label class="form-label" for="conn-api-key">${t(provider.id==='bedrock'?'Chave de API Bedrock':'Chave de API')} · ${h.esc(provider.name)}</label><input id="conn-api-key" name="apiKey" type="password" required minlength="8" maxlength="8192" autocomplete="new-password" spellcheck="false"><p class="field-hint">A chave fica apenas nesta sessão. Reconecte após fechar o aplicativo.</p>`}<label class="conn-consent"><input type="checkbox" name="paidConsent" value="yes" required><span>Autorizo cobrança pela API escolhida. Esta conexão não é um fallback da minha assinatura.</span></label><p class="field-hint">Uso, créditos e capacidades aparecem apenas quando o provedor disponibilizar esses dados.</p><p class="error-note" id="conn-form-error" role="alert"></p><div class="form-actions">${button(t('Voltar'),'wizard-back',null,h)}<button class="button primary" type="submit">${wizard.targetId?t('Atualizar conexão'):t('Adicionar conexão')}</button></div></form>`);
  }
  function cleanEndpoint(value,allowLoopback=false) {
    try{const url=new URL(value);const local=['localhost','127.0.0.1','[::1]'].includes(url.hostname);if(url.username||url.password||/[?#]/.test(value)||/^[a-z][a-z0-9+.-]*:\/\/[^/]*@/i.test(value)||url.protocol!=='https:'&&!(allowLoopback&&local&&url.protocol==='http:'))return null;return url.href.replace(/\/$/,'');}catch{return null;}
  }
  function pendingConnection(name,provider,mode,workspace=t('Pessoal')) {
    return {id:`connection-${crypto.randomUUID()}`,name,provider,letter:provider.slice(0,1),mode,modality:modeLabels[mode],workspace,simulated:false,lifecycle:'pending',status:labels.pending,identity:{owner:null,label:name,email:null,recordId:null,revision:1,validation:'unverified'},planConfirmed:false,consentMode:mode,acceptsNewWork:false,authorizationValid:false,catalog:[],catalogCompatible:null,catalogStatus:t('Catálogo não confirmado'),catalogLoading:false,workerTermination:'Não confirmado',logout:'Não solicitado',revocation:'Não solicitada',pendingRegistration:null,history:[{text:t('Configuração registrada; autorização do provedor pendente.'),revision:1}]};
  }
  function containsCredential(value) {return [...credentials.values()].some(credential=>credential&&String(value).includes(credential));}
  function safeDraft(value,newCredential='') {return containsCredential(value)||newCredential&&String(value).includes(newCredential)?'':String(value||'');}
  function captureWizardDraft() {
    const form=document.getElementById(wizard?.stage==='account'?'conn-account-form':'conn-api-form');if(!form)return;
    const data=new FormData(form),key=String(data.get('apiKey')||'');
    for(const field of ['name','workspace','endpoint','deployment','region','awsProfile'])if(data.has(field))wizard[field]=safeDraft(data.get(field),key);
    if(wizard.stage==='account')wizard.runtimeProvider=String(data.get('provider')||runtimeProviders[0]);
    else wizard.authMethod=String(data.get('authMethod')||'api-key');
    wizard.kind=wizard.stage==='account'?'own-plan':'api';
  }
  function validateModels(result) {
    const serialized=JSON.stringify(result);
    if(!serialized||serialized.length>4000000||containsCredential(serialized)||[...credentials.values()].some(key=>key&&serialized.includes(JSON.stringify(key).slice(1,-1))))throw new Error('invalid-catalog');
    if(!result||!Array.isArray(result.models)||result.models.length>500)throw new Error('invalid-catalog');
    const seen=new Set();return result.models.map(model=>{
      const kind=['deployment','inference-profile'].includes(model?.kind)?model.kind:'model';
      const resourceKey=`${kind}:${model?.id}`;
      if(!model||typeof model.id!=='string'||!model.id.trim()||model.id.length>200||seen.has(resourceKey)||typeof model.name!=='string'||!model.name.trim()||model.name.length>240||containsCredential(model.id)||containsCredential(model.name))throw new Error('invalid-catalog');seen.add(resourceKey);
      const declared=model.capabilities?.reasoning;
      const thinking=declared?.observed===true&&Array.isArray(declared.levels)?declared.levels.filter(value=>typeof value==='string'&&/^[a-z0-9_-]{1,30}$/i.test(value)&&!containsCredential(value)).slice(0,12):[];
      const normalize=cap=>{
        const status=['supported','unsupported','unknown'].includes(cap?.status)?cap.status:'unknown';
        const value={status,observed:cap?.observed===true,evidence:Array.isArray(cap?.evidence)?cap.evidence.slice(0,8):[]};
        for(const field of ['inputTokens','outputTokens','windowTokens'])if(Number.isSafeInteger(cap?.[field])&&cap[field]>0)value[field]=cap[field];
        for(const field of ['modalities','modes','levels'])if(Array.isArray(cap?.[field]))value[field]=cap[field].filter(item=>typeof item==='string'&&item.length<=80).slice(0,16);
        return value;
      };
      const capabilities=Object.fromEntries(['input','output','context','reasoning','streaming','functions','nativeWeb','hostTools','structuredOutput','imageInput','pdfInput'].map(key=>[key,normalize(model.capabilities?.[key])]));
      if(model.capabilities?.reasoning?.effort)capabilities.reasoning.effort=normalize(model.capabilities.reasoning.effort);
      if(thinking.length)capabilities.reasoning={...capabilities.reasoning,observed:true,levels:[...thinking]};
      if(result.schemaVersion!==1)capabilities.reasoning=thinking.length?{observed:true,levels:[...thinking]}:{observed:false};
      const compatibility=['supported','unsupported','unknown'].includes(model.compatibility?.status)?model.compatibility.status:'unknown';
      const runtimeFields=Object.fromEntries(['serviceTiers','defaultServiceTier','defaultReasoningEffort','modelSpecialty','multiAgentVersion','isDefault','hidden','runtimeId','access'].filter(key=>model[key]!==undefined).map(key=>[key,model[key]]));
      return {id:model.id,name:model.name,thinking,kind,capabilities:{...capabilities,tools:capabilities.functions,web:capabilities.nativeWeb},compatibility:{status:compatibility,reason:String(model.compatibility?.reason||'catalog-only').slice(0,160)},source:model.source||'adapter',scope:model.scope||null,methods:Array.isArray(model.methods)?model.methods.slice(0,30):[],baseModel:model.baseModel||null,parameters:model.parameters||null,lifecycle:model.lifecycle||null,owner:model.owner||null,shutdownDate:model.shutdownDate||null,inferenceTypes:model.inferenceTypes||[],members:model.members||[],...runtimeFields};
    });
  }
  function diagnosticResult(c) {return c.catalogObservation?{...c.catalogObservation,models:c.catalog}:null;}
  function updateDiagnostics(c,result,options={}) {
    const engine=window.OrchestrixConnectionDiagnostics;if(!engine||c.simulated)return;
    c.diagnostics=engine.buildReport(c,result,{...options,errorCode:typeof options.error==='string'?options.error:options.errorCode,redactValues:[...credentials.values()]});
  }
  function showDiagnostics(id,state,h) {
    const c=find(id,state);if(!c||c.simulated)return false;
    if(!c.diagnostics||!window.OrchestrixConnectionDiagnostics.isCurrent(c.diagnostics,c))updateDiagnostics(c,diagnosticResult(c),{phase:'completed',error:c.lifecycle==='disconnected'?'disconnected':undefined});
    inspectedId=null;diagnosticsInspectedId=c.id;
    h.showModal(t('Diagnóstico da conexão'),html`<div class="conn-inspector">${window.OrchestrixDiagnosticsView.view(c,h,button)}</div>`);
    return true;
  }
  function diagnosticNotice(c,state,h) {
    if(c.simulated||state.view==='connections')return;
    let notice=document.getElementById('connection-diagnostic-notice');
    if(!notice){notice=document.createElement('aside');notice.id='connection-diagnostic-notice';notice.className='diagnostic-notice';notice.setAttribute('aria-live','polite');document.body.append(notice);}
    notice.dataset.connectionId=c.id;
    notice.innerHTML=html`<div><strong>Diagnóstico concluído</strong><span>${h.esc(c.name)}</span></div>${button(t('Ver diagnóstico'),'diagnostics',c.id,h)}<button type="button" class="icon-button" data-conn-action="dismiss-diagnostics" aria-label="Fechar aviso">×</button>`;
    clearTimeout(diagnosticNoticeTimer);diagnosticNoticeTimer=setTimeout(()=>notice.remove(),20000);
  }
  function cancelDiscovery(c,reason='cancelled') {
    const job=discoveryJobs.get(c.id);if(job){discoveryJobs.delete(c.id);clearTimeout(job.timer);job.controller.abort();}
    c.catalogLoading=false;c.discovery={...c.discovery,status:reason};
    if(c.diagnostics)updateDiagnostics(c,diagnosticResult(c),{phase:'cancelled',runId:c.diagnostics.runId,startedAt:c.diagnostics.startedAt});
  }
  function refreshDiscovery(c,state,h) {
    const modal=document.querySelector('#modal');const focused=modal?.open?document.activeElement:null;const action=focused?.dataset?.connAction;const scroll=modal?.querySelector('.modal-body')?.scrollTop||0;
    h.render();window.OrchestrixSettings?.refresh();
    if(modal?.open&&diagnosticsInspectedId===c.id){showDiagnostics(c.id,state,h);const target=action&&modal.querySelector(`[data-conn-action="${action}"]`);target?.focus({preventScroll:true});const body=modal.querySelector('.modal-body');if(body)body.scrollTop=scroll;}
    else if(modal?.open&&inspectedId===c.id)inspect(c.id,state,h);
  }
  async function discoverCatalog(c,state,h) {
    cancelDiscovery(c);const scope=discoveryScope(c);const adapter=discoveryAdapters.get(scope)||(!c.simulated&&c.mode==='api'&&window.__ORCHESTRIX_TEST_DISCOVERY_ADAPTERS!==true?window.OrchestrixDiscovery:null);
    const startedAt=new Date().toISOString(),runId=crypto.randomUUID();latestDiagnosticId=c.id;
    updateDiagnostics(c,null,{phase:'running',startedAt,runId});
    c.discovery={status:adapter?'loading':'unavailable',scope,source:null};c.catalogLoading=Boolean(adapter);
    if(!adapter){c.catalogObservation=null;updateDiagnostics(c,null,{phase:'completed',startedAt,runId,error:c.mode==='api'?'adapter-unavailable':'authentication-required'});refreshDiscovery(c,state,h);diagnosticNotice(c,state,h);return;}
    const controller=new AbortController(),token=crypto.randomUUID();
    const job={controller,token,timer:null};discoveryJobs.set(c.id,job);
    const timeoutMs=adapter.timeoutMs||10000;
    const timeout=new Promise((_,reject)=>{job.timer=setTimeout(()=>{controller.abort();reject(new Error('timeout'));},timeoutMs);});
    refreshDiscovery(c,state,h);
    try{
      const result=await Promise.race([Promise.resolve().then(()=>adapter.discover({provider:c.apiProvider,endpoint:c.endpoint,connectionId:c.id,apiVersion:c.apiVersion,region:c.region||null,deployment:c.deployment||null,authMethod:c.authMethod,awsProfile:c.awsProfile||null,credential:credentials.get(c.id)||null,signal:controller.signal})),timeout]);
      if(discoveryJobs.get(c.id)!==job||discoveryScope(c)!==scope||c.lifecycle==='disconnected')return;
      if(result.scope?.provider&&result.scope.provider!==c.apiProvider||result.scope?.connectionId&&result.scope.connectionId!==c.id)throw new Error('invalid-catalog');
      c.catalog=validateModels(result);c.catalogCompatible=c.catalog.some(model=>model.compatibility.status==='supported')?true:null;
      c.catalogObservation={schemaVersion:result.schemaVersion,scope:result.scope||null,source:result.source||null,observedAt:result.observedAt||null,status:result.status||'complete',providerCapabilities:result.providerCapabilities||null,features:result.features||null,access:result.access||null,limitations:result.limitations||[]};
      updateDiagnostics(c,diagnosticResult(c),{phase:'completed',startedAt,runId});
      c.discovery={status:result.status==='unavailable'?'unavailable':result.status==='partial'?'partial':'observed',scope,source:result.source||'adapter',observedAt:result.observedAt||new Date().toISOString(),limitations:Array.isArray(result.limitations)?result.limitations.filter(item=>/^[a-z0-9-]{1,80}$/.test(item?.code)).slice(0,20):[],pages:Number.isSafeInteger(result.pages)?result.pages:null};c.catalogStatus=t('Catálogo informado pelo adapter');
    }catch(failure){if(discoveryJobs.get(c.id)!==job)return;c.catalog=[];c.catalogObservation=null;c.catalogCompatible=null;const code=failure?.code;c.discovery={status:controller.signal.aborted?'timeout':code==='host-unavailable'?'host-unavailable':code==='authentication-required'?'authentication-required':code==='access-denied'?'access-denied':code==='rate-limited'||code==='busy'?'rate-limited':'failed',scope,source:null};c.catalogStatus=t('Não foi possível confirmar o catálogo');updateDiagnostics(c,null,{phase:'completed',startedAt,runId,error:c.discovery.status});}
    finally{if(discoveryJobs.get(c.id)===job){clearTimeout(job.timer);discoveryJobs.delete(c.id);c.catalogLoading=false;refreshDiscovery(c,state,h);diagnosticNotice(c,state,h);}}
  }
  function registered(state,h,c) {
    clearCredentialFields();wizard=null;inspectedId=null;diagnosticsInspectedId=null;
    h.closeModal();h.render();h.notify(t('Configuração salva; autenticação e acesso ainda precisam ser confirmados.'));
    void discoverCatalog(c,state,h);
  }
  function sameName(state,name,id) {return (state.connections||[]).some(c=>c.id!==id&&c.name.toLocaleLowerCase()===name.toLocaleLowerCase());}
  function validLabel(value,max=70) {return Boolean(value&&value.length<=max&&!/[\u0000-\u001f\u007f]/.test(value));}
  function safeConnectionMetadata(form,values,h,newCredential='') {
    const unsafe=value=>containsCredential(value)||Boolean(newCredential&&String(value).includes(newCredential));
    if(!values.some(unsafe))return true;
    // Validate while old credentials are still retained; rotation must not erase the redaction reference.
    for(const input of form.elements)if(input.name!=='apiKey'&&unsafe(input.value)){input.value='';input.removeAttribute('value');}
    if(wizard)for(const field of ['name','endpoint','deployment','region','awsProfile','workspace'])if(unsafe(wizard[field]||''))wizard[field]='';
    error(t('Mantenha a chave apenas no campo seguro de credenciais.'),h);return false;
  }
  function saveAPI(form,data,state,h) {
    if(!wizard||wizard.stage!=='api')return true;
    const id=String(data.get('apiProvider')||'');const provider=providerInfo(id);
    const name=String(data.get('name')||'').trim();
    const key=String(data.get('apiKey')||'');
    const authMethod=id==='bedrock'?String(data.get('authMethod')||''):'api-key';
    const endpoint=['azure','compatible'].includes(id)?cleanEndpoint(String(data.get('endpoint')||'').trim(),id==='compatible'):provider?.endpoint;
    const deployment=id==='azure'?String(data.get('deployment')||'').trim():null;
    const region=id==='bedrock'?String(data.get('region')||'').trim():null;
    const awsProfile=authMethod==='aws-profile'?String(data.get('awsProfile')||'').trim():null;
    if(!safeConnectionMetadata(form,[name,String(data.get('endpoint')||''),deployment||'',region||'',awsProfile||''],h,authMethod==='api-key'&&key.length>=8?key:''))return true;
    if(!provider||id!==wizard.provider||!validLabel(name)||sameName(state,name,wizard.targetId)) {error(t('Escolha um provedor e um nome válido e único para a conexão.'),h);return true;}
    if(data.get('paidConsent')!=='yes') {error(t('Confirme a cobrança desta API antes de salvar a conexão.'),h);return true;}
    if(['azure','compatible'].includes(id)&&!endpoint) {error(t('Informe um endpoint HTTPS válido, sem credenciais, parâmetros ou fragmentos. HTTP é aceito apenas para serviços locais compatíveis.'),h);return true;}
    if(id==='azure'&&(!deployment||!/^[A-Za-z0-9][A-Za-z0-9_.-]{0,119}$/.test(deployment))) {error(t('Informe um nome de deployment válido para o Azure.'),h);return true;}
    if(id==='bedrock'&&(!/^[a-z]{2}(?:-[a-z]+){1,3}-\d$/.test(region||'')||!['api-key','aws-profile'].includes(authMethod))) {error(t('Informe uma região AWS e um método de autenticação válidos.'),h);return true;}
    if(authMethod==='aws-profile'&&!/^[A-Za-z0-9_.@-]{1,70}$/.test(awsProfile||'')) {error(t('Informe o nome do perfil AWS configurado no computador.'),h);return true;}
    if(authMethod==='api-key'&&(key.length<8||key.length>8192||/\s|[\u0000-\u001f\u007f]/.test(key))) {error(t('Informe uma chave de API válida, sem espaços ou quebras de linha.'),h);return true;}
    let c=wizard.targetId?find(wizard.targetId,state):null;
    if(wizard.targetId&&!c) {clearCredentialFields();error(t('Conexão não encontrada.'),h);return true;}
    if(c){cancelDiscovery(c);credentials.delete(c.id);c.identity={...c.identity,revision:c.identity.revision+1};}
    else {c=pendingConnection(name,provider.name,'api');(state.connections||=[]).push(c);}
    Object.assign(c,{name,provider:provider.name,letter:provider.name.slice(0,1),apiProvider:id,endpoint:id==='bedrock'?`https://bedrock-runtime.${region}.${region.startsWith('cn-')?'amazonaws.com.cn':'amazonaws.com'}`:endpoint,deployment,region,authMethod,awsProfile,paidConsent:true,credentialPresent:authMethod==='api-key',credentialStorage:authMethod==='api-key'?'memory':'runtime-profile',lifecycle:'pending',status:labels.pending,acceptsNewWork:false,authorizationValid:false,catalog:[],catalogCompatible:null,catalogStatus:t('Catálogo não confirmado'),discovery:{status:'unavailable'}});
    c.identity={...c.identity,label:name,validation:'unverified'};
    if(authMethod==='api-key')credentials.set(c.id,key);
    clearCredentialFields();
    h.record(text`${name} · configuração de API salva`,t('Cobrança de API autorizada explicitamente; acesso, modelos e cotas aguardam confirmação.'),'plug');
    registered(state,h,c);return true;
  }
  function configureAPI(c,state,h) {
    wizard={stage:'api',provider:c.apiProvider,name:c.name,targetId:c.id,endpoint:c.endpoint||'',deployment:c.deployment||'',region:c.region||'',authMethod:c.authMethod||'api-key',awsProfile:c.awsProfile||''};
    inspectedId=null;wizardAPI(state,h);return true;
  }

  function exampleCatalog(provider) {
    const codex = provider === 'Codex';
    return [{id: codex ? 'codex-example' : 'claude-example',
      name: codex ? 'Modelo Codex' : 'Modelo Claude',
      thinking: ['medium', 'high'], simulated: true}];
  }

  function makeConnection(id, name, provider, workspace, capacityGroup, active) {
    return {
      id, name, provider, letter: provider === 'Codex' ? 'C' : '✳',
      lifecycle: active ? 'active' : 'pending', status: labels[active ? 'active' : 'pending'],
      simulated: true, mode: 'own-plan', modality: modeLabels['own-plan'], workspace,
      capacityGroup, capacity: 'Desconhecida; grupos distintos não comprovam cotas independentes',
      identity: {owner: `demo-owner-${id}`, label: name,
        email: provider === 'Codex' ? 'pessoa@example.invalid' : 'outra-pessoa@example.invalid',
        recordId: active ? `demo-${id}-r1` : null, revision: active ? 1 : 0,
        validation: active ? 'validated-simulated' : 'pending'},
      planConfirmed: active, consentMode: active ? 'own-plan' : null,
      acceptsNewWork: active, authorizationValid: active, catalog: exampleCatalog(provider), catalogCompatible: true,
      catalogStatus: 'Acesso ao modelo não verificado',
      workerTermination: 'Não confirmado', logout: 'Não solicitado', revocation: 'Não solicitada',
      pendingRegistration: active ? null : {revision: 1, mode: 'own-plan'},
      history: [{text: active ? t('Configuração inicial registrada; autorização do provedor não verificada.') :
        t('Registro aguarda confirmação de configuração.'), revision: active ? 1 : 0}]
    };
  }

  function initial() {
    return [
      makeConnection('codex-a', t('Codex · pessoal A'), 'Codex', t('Pessoal A'), 'demo-capacity-a', true),
      makeConnection('codex-b', t('Codex · pessoal B'), 'Codex', t('Pessoal B'), 'demo-capacity-b', false),
      makeConnection('claude', t('Claude · pessoal'), 'Claude Code', t('Pessoal'), 'demo-capacity-claude', true)
    ];
  }

  function eligible(c) {
    return Boolean(c && c.simulated && c.lifecycle === 'active' && c.acceptsNewWork && c.authorizationValid &&
      c.identity?.validation === 'validated-simulated' && !c.catalogLoading && c.catalogCompatible !== false &&
      (c.mode !== 'own-plan' || c.planConfirmed));
  }

  const find = (id, state) => (state.connections || []).find(c => c.id === id);
  const related = (c, state) => (state.tasks || []).filter(task => task.connectionId === c.id ||
    (!task.connectionId && task.account === c.name));
  const button = (text, action, id, h, primary = false) =>
    html`<button type="button" class="button conn-button${primary ? ' primary' : ''}" data-conn-action="${h.esc(action)}"${id ? ` data-conn-id="${h.esc(id)}"` : ''}>${h.esc(text)}</button>`;
  const fact = (label, value, h) => html`<div><dt>${h.esc(label)}</dt><dd>${h.esc(value)}</dd></div>`;
  const status = (c, h) => html`<span class="badge conn-status ${c.lifecycle === 'active' ? 'ready' : 'blocked'}">${h.esc(t(c.catalogLoading?'Catálogo pendente':labels[c.lifecycle]))}</span>`;
  function connectionFact(value) {
    switch(value) {
      case 'Não confirmado':return t('Não confirmado');
      case 'Não solicitado':return t('Não solicitado');
      case 'Não solicitada':return t('Não solicitada');
      case 'Não confirmado; nenhuma tentativa foi encerrada':return t('Não confirmado; nenhuma tentativa foi encerrada');
      case 'Não confirmada · sem resposta do provedor':return t('Não confirmada · sem resposta do provedor');
      default:return value;
    }
  }

  function change(c, lifecycle, message, detail, h) {
    c.lifecycle = lifecycle;
    c.status = labels[lifecycle];
    c.acceptsNewWork = lifecycle === 'active';
    c.history.unshift({text:message, revision: c.identity.revision});
    h.record(text`${c.name} · ${message}`, text`${detail} · tentativas anteriores preservadas.`, 'plug');
  }

  function view(state, h) {
    const latest=(state.connections||[]).find(c=>c.id===latestDiagnosticId&&!c.simulated);
    return h.pageHead(t('CONEXÕES'), t('Contas distintas, escolhas claras'),
      t('Escolha a conexão para novos trabalhos. Sessões existentes preservam sua conta, configuração e contexto.'),
      button(t('Adicionar conexão'), 'add', null, h, true)) +
      (latest&&window.OrchestrixDiagnosticsView?window.OrchestrixDiagnosticsView.card(latest,h,button):'')+
      html`<div class="connection-list conn-list">${(state.connections || []).map(c => {
        const tasks = related(c, state);
        return html`<article class="connection-row conn-row" aria-label="${h.esc(c.name)}">
          <span class="provider-logo" aria-hidden="true">${h.esc(c.letter)}</span>
          <div><strong>${h.esc(c.name)}</strong><small>${h.esc(c.provider)} · ${h.esc(c.workspace)}</small>
            <small>${h.esc(t(modeLabels[c.mode]))}</small>${!c.simulated?html`<small>${h.esc(window.OrchestrixResources.summary(c))}</small>`:''}</div>
          <div>${status(c, h)}<small>${eligible(c) ? t('Configuração elegível para novo trabalho') : t('Indisponível para nova tentativa')}</small>
            <small>${tasks.length} ${tasks.length === 1 ? t('tentativa vinculada') : t('tentativas vinculadas')} · capacidade não informada</small></div>
          <div class="conn-row-actions">${!c.simulated?button(t('Ver diagnóstico'),'diagnostics',c.id,h):''}${button(t('Inspecionar conexão'), 'inspect', c.id, h)}</div></article>`;
      }).join('')}</div>
      <p class="notice conn-note">Cada conexão mantém sua identidade e suas preferências. Mais sessões não aumentam a cota, e limites não ativam cobrança por API automaticamente.</p>`;
  }

  function catalogView(c, state, h) {
    if(!c.simulated)return window.OrchestrixResources.view(c,h,button);
    if(c.catalogLoading)return html`<div class="attention-banner" role="status" aria-live="polite"><div><h3>Aguardando catálogo</h3><p>Modelos e acesso ainda não foram confirmados. Novos trabalhos aguardam; os existentes preservam sua configuração.</p></div></div>`;
    const p = state.preferences || {};
    const requestedModel = p.model === 'runtime' ? t('Preferência do runtime') :
      p.model === 'favorite' ? t('Favorito do perfil') : p.model || t('Não definido');
    const requestedThinking = {auto: t('Gerenciado pelo runtime'), medium: t('Médio'), high: t('Alto')}[p.thinking] || p.thinking || t('Não definido');
    return html`<h3>Escolhas para a próxima tentativa</h3><dl class="routing conn-facts">
      ${fact(t('Modelo solicitado'), requestedModel, h)}${fact(t('Raciocínio solicitado'), requestedThinking, h)}
      ${fact(t('Catálogo'), t(c.catalogCompatible?'Acesso ao modelo não verificado':'Escolha incompatível com o catálogo'), h)}${fact(t('Modelo efetivo'), t('Não informado'), h)}</dl>
      <ul class="conn-catalog">${c.catalog.map(model => html`<li><strong>${h.esc(model.id==='codex-example'?t('Modelo Codex'):model.id==='claude-example'?t('Modelo Claude'):model.name)}</strong>
        <small>Raciocínio configurado: ${h.esc(model.thinking.map(v => v === 'high' ? t('alto') : t('médio')).join(', '))}; acesso não verificado.</small></li>`).join('')}</ul>
      ${c.catalogCompatible ? html`<p class="field-hint">Configuração compatível com o catálogo. A disponibilidade do modelo depende da autorização do provedor.</p>` :
        html`<p class="error-note" role="status">Escolha incompatível com o catálogo desta conexão. Ajuste a preferência ou selecione outra conexão para um novo trabalho. Nenhum fallback para API foi realizado.</p>`}`;
  }

  function sessionsView(c, state, h) {
    const tasks = related(c, state);
    return html`<h3>Tentativas vinculadas</h3>${tasks.length ? html`<ul class="conn-sessions">${tasks.map(task =>
      html`<li><strong>${h.esc(task.id)} · tentativa ${h.esc(task.attempt || 1)}</strong>
        <small>${h.esc(task.title)} · ${h.esc(h.statuses?.[task.status]?t(h.statuses[task.status]):task.status)}</small>
        <small>Sessão/contexto próprios; conta e configuração da tentativa preservadas.</small></li>`).join('')}</ul>` :
      html`<p class="field-hint">Nenhuma tentativa vinculada.</p>`}
      <p class="field-hint">Outra sessão nesta conexão pode realizar revisão, com contexto separado. Isso não cria outra conta nem capacidade independente.</p>`;
  }

  function inspect(id, state, h) {
    const c = find(id, state);
    if (!c) { h.notify(t('Conexão não encontrada.')); return false; }
    const identity = c.identity.validation === 'validated-simulated' ? t('Registro confirmado · autorização não verificada') : t('Confirmação de registro pendente');
    inspectedId=c.id;diagnosticsInspectedId=null;
    const next = !c.simulated&&c.mode==='api'?button(t('Atualizar credenciais'),'configure-api',c.id,h,true):c.lifecycle === 'pending' ? button(t('Confirmar configuração'), 'authorize', c.id, h, true) :
      ['expired', 'denied', 'disconnected'].includes(c.lifecycle) ? button(t('Reconfigurar conexão'), 'reconnect', c.id, h, true) : '';
    h.showModal(text`${c.name} · conexão`, html`<div class="conn-inspector">${status(c, h)}
      <dl class="routing conn-facts">${fact(t('Registro estável'), c.id, h)}${fact(t('Runtime'), c.provider, h)}
        ${fact(t('Identidade'), identity, h)}${fact(t('Nome da conexão'), c.name, h)}
        ${fact(t('Revisão do registro'), c.identity.recordId ? `r${c.identity.revision}` : t('Ainda não confirmado'), h)}
        ${fact(t('Modalidade'), t(modeLabels[c.mode]), h)}${fact(t('Workspace'), c.workspace, h)}
        ${fact(t('Grupo de capacidade'), t('Não informado'), h)}${fact(t('Capacidade'), t('Não informado'), h)}${c.mode==='api'&&!c.simulated?fact(t('Endpoint da API'),c.endpoint,h)+(c.deployment?fact(t('Nome do deployment'),c.deployment,h):'')+(c.region?fact(t('Região AWS'),c.region,h):'')+fact(t('Credenciais'),c.authMethod==='aws-profile'?t('Perfil AWS no runtime desktop'):c.credentialPresent?t('Chave mantida apenas nesta sessão'):t('Reconexão necessária'),h):''}</dl>
      <p class="field-hint">A configuração identifica a conexão; a autorização e o acesso do provedor precisam de confirmação própria.</p>
      <div class="form-actions conn-actions">${next}${button(t('Consultar uso'), 'usage', c.id, h)}${!c.simulated?button(t('Ver diagnóstico'),'diagnostics',c.id,h):''}</div>
      ${catalogView(c, state, h)}${sessionsView(c, state, h)}
      ${fixtureControls() ? html`<details class="conn-scenarios"><summary>Diagnóstico da conexão</summary><p class="field-hint">Apenas a elegibilidade de novos trabalhos muda. Nenhuma tentativa existente é retomada, interrompida ou transferida.</p>
        <div class="form-actions conn-actions">${button(c.catalogLoading?t('Confirmar registro de catálogo'):t('Definir catálogo pendente'),c.catalogLoading?'finish-discovery':'discovery',c.id,h)}${button(t('Marcar limite'), 'limit', c.id, h)}${button(t('Requerer nova autorização'), 'expire', c.id, h)}
          ${button(t('Marcar acesso negado'), 'deny', c.id, h)}${button(c.catalogCompatible ? t('Marcar catálogo incompatível') : t('Restaurar catálogo'), c.catalogCompatible ? 'incompatible' : 'catalog', c.id, h)}</div></details>` : ''}
      <details class="conn-history"><summary>Histórico de registros e decisões</summary><ol>${c.history.map(entry =>
        html`<li>${h.esc(entry.text)} <small>· registro r${h.esc(entry.revision)}</small></li>`).join('')}</ol></details>
      <h3>Encerramento e confirmação</h3><dl class="routing conn-facts">${fact(t('Novos trabalhos'), c.acceptsNewWork ? t('Permitidos se elegíveis') : t('Bloqueados'), h)}
        ${fact(t('Término de workers'), connectionFact(c.workerTermination), h)}${fact(t('Saída da conta'), connectionFact(c.logout), h)}${fact(t('Revogação remota'), connectionFact(c.revocation), h)}</dl>
      <div class="form-actions conn-actions">${c.lifecycle === 'disconnected' ? (fixtureControls() ? button(t('Registrar revogação sem resposta'), 'revoke-no-signal', c.id, h) : '') :
        button(t('Desconectar para novos trabalhos'), 'disconnect', c.id, h)}</div></div>`);
    return true;
  }

  function addForm(state,h) {
    if(fixtureControls()&&!window.__ORCHESTRIX_TEST_API_WIZARD)return legacyAddForm(state,h);
    wizard={stage:'kind',provider:'openai',authMethod:'api-key',name:''};inspectedId=null;diagnosticsInspectedId=null;
    wizardMode(state,h);return true;
  }
  function legacyAddForm(state, h) {
    h.showModal(t('Adicionar conexão'), html`<p>Crie um registro para organizar a conexão. A autorização do provedor é uma etapa separada; este formulário não recebe credenciais.</p>
      <form id="conn-add-form" class="conn-form">
        <label class="form-label" for="conn-provider">Runtime</label><select id="conn-provider" name="provider"><option>Codex</option><option>Claude Code</option></select>
        <label class="form-label" for="conn-name">Nome para reconhecer a conta</label><input id="conn-name" name="name" required maxlength="70" placeholder="Nome para identificar esta conexão">
        <label class="form-label" for="conn-workspace">Workspace</label><input id="conn-workspace" name="workspace" required maxlength="70" placeholder="Pessoal ou equipe">
        <label class="form-label" for="conn-mode">Origem de autorização e cobrança</label><select id="conn-mode" name="mode"><option value="own-plan">Assinatura do provedor</option><option value="api">API · cobrança separada</option></select>
        <p class="field-hint">Confirme a origem de cobrança para este registro. API é uma escolha separada; limites de assinatura nunca a selecionam automaticamente.</p>
        <p class="error-note" id="conn-form-error" role="alert"></p><div class="form-actions">${button(t('Cancelar'), 'close', null, h)}<button class="button primary" type="submit">Adicionar registro pendente</button></div></form>`);
    return true;
  }

  function authorization(c, state, h) {
    if (!c.pendingRegistration) c.pendingRegistration = {revision: c.identity.revision + 1, mode: c.mode};
    const candidate = c.pendingRegistration;
    const needsConsent = c.consentMode !== candidate.mode || (candidate.mode === 'own-plan' && !c.planConfirmed);
    const confirmation = candidate.mode === 'own-plan' ? t('Quero usar uma assinatura do provedor nesta conexão.') :
      t('Quero configurar uma API com cobrança separada para esta conexão.');
    h.showModal(t('Confirmar configuração da conexão'), html`<p>Revise o registro e a origem de cobrança. Esta confirmação salva a configuração; a autorização do provedor não é verificada aqui. Trabalhos anteriores conservam seu registro e contexto.</p>
      <div class="snapshot"><strong>${h.esc(c.name)}</strong><br>${h.esc(c.workspace)}<br>${h.esc(t(modeLabels[candidate.mode]))}<br>Registro proposto r${h.esc(candidate.revision)} · configuração</div>
      <form id="conn-confirm-form" class="conn-form"><input type="hidden" name="connectionId" value="${h.esc(c.id)}">
        <label class="form-label" for="conn-identity-result">Conferência da conta declarada</label><select id="conn-identity-result" name="identityResult"><option value="same">A conta declarada corresponde a este registro</option><option value="different">A conta declarada não corresponde — impedir uso</option></select>
        ${needsConsent ? html`<label class="conn-consent"><input type="checkbox" name="confirmMode" value="yes" required> <span>${h.esc(confirmation)}</span></label>` :
          html`<p class="field-hint">Esta modalidade já foi confirmada neste registro.</p>`}
        <p class="field-hint">Confirmar o registro não comprova assinatura, acesso a modelos ou cota disponível.</p>
        <p class="error-note" id="conn-form-error" role="alert"></p><div class="form-actions">${button(t('Voltar à conexão'), 'inspect', c.id, h)}<button class="button primary" type="submit">Salvar configuração</button></div></form>`);
    return true;
  }

  function error(message, h) {
    const element = document.getElementById('conn-form-error');
    if (element) element.textContent = message;
    h.notify(message);
  }

  function click(el, state, h) {
    const act = el.dataset.connAction;
    if (!act) return false;
    if (fixtureActions.has(act) && !fixtureControls()) return true;
    if (act === 'add') return addForm(state, h);
    if(act==='dismiss-diagnostics'){document.getElementById('connection-diagnostic-notice')?.remove();clearTimeout(diagnosticNoticeTimer);return true;}
    if(act==='wizard-close') {clearCredentialFields();wizard=null;h.closeModal();return true;}
    if(act==='wizard-back') {captureWizardDraft();clearCredentialFields();wizard.stage='kind';wizardMode(state,h);return true;}
    if (act === 'close') { h.closeModal(); return true; }
    const c = find(el.dataset.connId, state);
    if (!c) { h.notify(t('Conexão não encontrada.')); return true; }
    if (act === 'inspect') return inspect(c.id, state, h);
    if(act==='diagnostics')return showDiagnostics(c.id,state,h);
    if(act==='recheck-diagnostics') {
      if(c.lifecycle==='disconnected'||c.mode==='api'&&c.authMethod==='api-key'&&!credentials.has(c.id)){
        updateDiagnostics(c,null,{phase:'completed',error:c.lifecycle==='disconnected'?'disconnected':'credentials-missing'});refreshDiscovery(c,state,h);h.notify(t('Reconexão necessária'));return true;
      }
      if(!c.simulated)void discoverCatalog(c,state,h);return true;
    }
    if(act==='cancel-diagnostics'){cancelDiscovery(c);refreshDiscovery(c,state,h);return true;}
    if(act==='configure-api')return configureAPI(c,state,h);
    if(act==='retry-catalog') {if(c.lifecycle==='disconnected'||c.authMethod==='api-key'&&!credentials.has(c.id)){h.notify(t('Reconexão necessária'));return true;}if(c.mode==='api'&&!c.simulated)void discoverCatalog(c,state,h);return true;}
    if(act==='cancel-catalog') {cancelDiscovery(c);refreshDiscovery(c,state,h);return true;}
    if(['authorize','reconnect'].includes(act)&&!c.simulated) {
      if(c.mode==='api')return configureAPI(c,state,h);
      h.notify(t('A autenticação desta conta será realizada pelo runtime desktop.'));return true;
    }
    if (act === 'authorize') return authorization(c, state, h);
    if (act === 'reconnect') {
      c.pendingRegistration = {revision: c.identity.revision + 1, mode: c.mode};
      c.authorizationValid = false;
      change(c, 'pending', t('Nova autorização pendente'), t('Registro anterior preservado'), h);
      h.render(); return authorization(c, state, h);
    }
    if (act === 'usage') {
      h.showModal(t('Uso da conexão'), html`<p>${h.esc(c.name)} usa ${h.esc(t(modeLabels[c.mode]))}. Consulte o gerenciamento de uso do provedor quando ele estiver disponível.</p>
        <div class="snapshot">Capacidade disponível: não informada.<br>Mais sessões/hosts não comprovam uma franquia adicional.</div>
        <p>Um limite permite aguardar, ajustar a próxima execução ou escolher outra conexão elegível. O histórico e a origem de cobrança permanecem explícitos.</p>
        <div class="form-actions">${button(t('Voltar à conexão'), 'inspect', c.id, h)}${c.lifecycle === 'limited' && fixtureControls() ? button(t('Registrar capacidade disponível'), 'clear-limit', c.id, h, true) : ''}</div>`);
      return true;
    }
    if (act === 'disconnect') {
      const active = related(c, state).filter(task => openStatuses.has(task.status));
      h.showModal(t('Bloquear novos trabalhos'), html`<p>A conexão ${h.esc(c.name)} deixará de aceitar novas tentativas. As ${active.length} tentativas pendentes continuam vinculadas à identidade original.</p>
        <p>Esta ação não confirma término dos workers, saída da conta ou revogação no provedor. Cada confirmação precisa de evidência própria.</p>
        <div class="form-actions">${button(t('Voltar à conexão'), 'inspect', c.id, h)}${button(t('Bloquear novos trabalhos'), 'confirm-disconnect', c.id, h, true)}</div>`);
      return true;
    }
    if(act==='discovery'||act==='finish-discovery'){c.catalogLoading=act==='discovery';c.status=c.catalogLoading?t('Catálogo pendente'):labels[c.lifecycle];h.record(text`${c.name} · descoberta de catálogo ${c.catalogLoading?t('pendente'):t('concluída')}`,t('Registro de catálogo atualizado; acesso do provedor não verificado.'),'plug');h.render();inspect(c.id,state,h);return true;}
    if (act === 'confirm-disconnect') {
      cancelDiscovery(c);credentials.delete(c.id);c.credentialPresent=false;
      c.pendingRegistration = null;
      c.authorizationValid = false;
      change(c, 'disconnected', t('Novos trabalhos bloqueados'), t('Sem confirmação de término, logout ou revogação'), h);
      c.catalogObservation=null;updateDiagnostics(c,null,{phase:'completed',error:'disconnected'});
      c.workerTermination = 'Não confirmado; nenhuma tentativa foi encerrada';
      c.logout = 'Não solicitado'; c.revocation = 'Não solicitada';
    } else if (act === 'revoke-no-signal') {
      if (c.lifecycle !== 'disconnected') return true;
      c.revocation = 'Não confirmada · sem resposta do provedor';
      c.history.unshift({text: t('Revogação sem confirmação; não tratada como sucesso.'), revision: c.identity.revision});
      h.record(text`${c.name} · revogação não confirmada`, t('Sem confirmação remota; workers e histórico preservados.'), 'alert');
    } else if (act === 'limit') {
      change(c, 'limited', t('Limite observado'), t('Capacidade não informada; sem fallback para API'), h);
    } else if (act === 'expire') {
      c.authorizationValid = false;
      change(c, 'expired', t('Autorização expirada'), t('Nova autorização necessária antes de novos trabalhos'), h);
    } else if (act === 'deny') {
      c.authorizationValid = false;
      change(c, 'denied', t('Acesso negado'), t('Sem retry silencioso ou troca de modalidade'), h);
    } else if (act === 'clear-limit') {
      if (c.lifecycle !== 'limited') return true;
      const validated = c.authorizationValid && c.identity.validation === 'validated-simulated' && (c.mode !== 'own-plan' || c.planConfirmed);
      change(c, validated ? 'active' : 'pending', t('Capacidade marcada como disponível'), t('Elegibilidade reavaliada; nenhuma tentativa retomada automaticamente'), h);
    } else if (act === 'incompatible' || act === 'catalog') {
      c.catalogCompatible = act === 'catalog';
      c.catalogStatus = c.catalogCompatible ? t('Acesso ao modelo não verificado') : t('Escolha incompatível com o catálogo');
      c.history.unshift({text: c.catalogCompatible ? t('Registro de catálogo restaurado.') : t('Incompatibilidade identificada; sem fallback pago.'), revision: c.identity.revision});
      h.record(text`${c.name} · catálogo ${c.catalogCompatible ? t('compatível') : t('incompatível')}`, t('Somente novas escolhas afetadas; tentativas preservadas.'), 'plug');
    } else return false;
    h.render(); inspect(c.id, state, h);
    return true;
  }

  function submit(form, data, state, h) {
    if(form.id==='conn-kind-form') {
      const kind=String(data.get('kind')||'');if(!wizard||wizard.stage!=='kind'||!['own-plan','api'].includes(kind))return true;
      wizard.kind=kind;wizard.stage=kind==='api'?'api':'account';if(kind==='api')wizardAPI(state,h);else wizardAccount(state,h);return true;
    }
    if(form.id==='conn-account-form') {
      if(!wizard||wizard.stage!=='account')return true;
      const name=String(data.get('name')||'').trim(),provider=String(data.get('provider')||''),workspace=String(data.get('workspace')||'').trim()||t('Pessoal');
      if(!safeConnectionMetadata(form,[name,workspace],h))return true;
      if(!validLabel(name)||!validLabel(workspace)||!runtimeProviders.includes(provider)||sameName(state,name)) {error(t('Escolha um runtime e um nome válido e único para a conexão.'),h);return true;}
      const c=pendingConnection(name,provider,'own-plan',workspace);(state.connections||=[]).push(c);h.record(text`${name} · conta registrada`,t('Conta registrada; autenticação, modelos e cotas aguardam o runtime.'),'plug');registered(state,h,c);return true;
    }
    if(form.id==='conn-api-form')return saveAPI(form,data,state,h);
    if (form.id === 'conn-add-form') {
      if(!fixtureControls())return true;
      const name = String(data.get('name') || '').trim();
      const workspace = String(data.get('workspace') || '').trim();
      const provider = String(data.get('provider') || '');
      const mode = String(data.get('mode') || '');
      if(!safeConnectionMetadata(form,[name,workspace],h))return true;
      if (!name || name.length > 70 || !workspace || workspace.length > 70 ||
          !['Codex', 'Claude Code'].includes(provider) || !Object.hasOwn(modeLabels, mode)) {
        error(t('Preencha nome, workspace, runtime e modalidade válidos.'), h); return true;
      }
      if ((state.connections || []).some(c => c.name.toLocaleLowerCase('pt-BR') === name.toLocaleLowerCase('pt-BR'))) {
        error(t('Escolha outro nome para distinguir as conexões.'), h); return true;
      }
      state.connections ||= [];
      let sequence = (state.nextConnectionSequence || 0) + 1;
      while (find(`demo-connection-${sequence}`, state)) sequence++;
      state.nextConnectionSequence = sequence;
      const c = makeConnection(`demo-connection-${sequence}`, name, provider, workspace, `demo-capacity-${sequence}`, false);
      c.mode = mode; c.modality = modeLabels[mode]; c.pendingRegistration.mode = mode;
      state.connections.push(c);
      h.record(text`${c.name} · registro pendente adicionado`, t('Registro pendente; autorização do provedor não verificada. Conexões e tentativas em uso preservadas.'), 'plug');
      h.closeModal(); h.render(); h.notify(t('Registro pendente criado. Confirme a configuração desta conexão.'));
      return true;
    }
    if (form.id !== 'conn-confirm-form') return false;
    const c = find(String(data.get('connectionId') || ''), state);
    if(c&&!c.simulated)return true;
    if (!c || c.lifecycle !== 'pending' || !c.pendingRegistration) {
      error(t('Este pedido não está mais pendente. Inspecione a conexão e revise sua configuração.'), h); return true;
    }
    const candidate = c.pendingRegistration;
    const needsConsent = c.consentMode !== candidate.mode || (candidate.mode === 'own-plan' && !c.planConfirmed);
    if (needsConsent && data.get('confirmMode') !== 'yes') {
      error(t('Confirme a modalidade indicada para salvar este registro.'), h); return true;
    }
    if (data.get('identityResult') !== 'same') {
      c.pendingRegistration = null;
      c.authorizationValid = false;
      change(c, 'denied', t('Identidade diferente; ativação impedida'), t('Registro anterior e tentativas conservados'), h);
      h.closeModal(); h.render(); h.notify(t('A identidade não corresponde à conta indicada. Nenhuma tentativa foi transferida.'));
      return true;
    }
    c.identity = {...c.identity, recordId: `demo-${c.id}-r${candidate.revision}`, revision: candidate.revision, validation: 'validated-simulated'};
    c.mode = candidate.mode; c.modality = modeLabels[c.mode];
    c.authorizationValid = true;
    c.consentMode = c.mode; c.planConfirmed = c.mode === 'own-plan'; c.pendingRegistration = null;
    c.catalogStatus = c.catalogCompatible ? t('Acesso ao modelo não verificado') : t('Escolha incompatível com o catálogo');
    change(c, 'active', text`Registro r${c.identity.revision} confirmado na configuração`, t('Configuração registrada; autorização, cota e acesso do provedor não verificados'), h);
    h.closeModal(); h.render(); h.notify(c.catalogCompatible ? t('Configuração disponível para novos trabalhos. Autorização do provedor não verificada.') :
      t('Registro validado; escolha incompatível ainda impede uma nova tentativa. Nenhum fallback pago.'));
    return true;
  }

  document.addEventListener('change',event=>{
    if(!wizard||wizard.stage!=='api'||!['conn-api-provider','conn-api-auth'].includes(event.target?.id))return;
    const app=window.OrchestrixApp;if(!app)return;
    const key=document.getElementById('conn-api-key')?.value||'';
    const name=safeDraft(document.getElementById('conn-name')?.value||'',key);
    const region=safeDraft(document.getElementById('conn-api-region')?.value||'',key);
    clearCredentialFields();
    if(event.target.id==='conn-api-provider')wizard={...wizard,provider:event.target.value,name,endpoint:'',deployment:'',region:'',awsProfile:'',authMethod:'api-key'};
    else wizard={...wizard,name,region,authMethod:event.target.value,awsProfile:''};
    wizardAPI(app.getState(),app.helpers());
  });
  document.addEventListener('orchestrix:language-change',()=>{
    const app=window.OrchestrixApp;if(!app)return;
    const notice=document.getElementById('connection-diagnostic-notice');
    if(notice){const c=find(notice.dataset.connectionId,app.getState());if(app.getState().view==='connections'||!c)notice.remove();else diagnosticNotice(c,app.getState(),app.helpers());}
    if(!document.getElementById('modal')?.open)return;
    if(!wizard){if(diagnosticsInspectedId)showDiagnostics(diagnosticsInspectedId,app.getState(),app.helpers());else if(inspectedId&&document.querySelector('#modal .conn-inspector'))inspect(inspectedId,app.getState(),app.helpers());return;}
    if(wizard.stage==='kind'){
      const kind=document.querySelector('#conn-kind-form [name="kind"]:checked')?.value;
      wizardMode(app.getState(),app.helpers());const input=document.querySelector(`#conn-kind-form [name="kind"][value="${kind==='api'?'api':'own-plan'}"]`);if(input)input.checked=true;
    }
    if(wizard.stage==='account'){
      const values=new FormData(document.getElementById('conn-account-form'));
      wizardAccount(app.getState(),app.helpers());for(const name of ['provider','name','workspace']){const input=document.querySelector(`#conn-account-form [name="${name}"]`);if(input)input.value=safeDraft(values.get(name)||'');}
    }
    if(wizard.stage==='api'){
      const values=new FormData(document.getElementById('conn-api-form'));const key=String(values.get('apiKey')||'');
      for(const field of ['name','endpoint','deployment','region','awsProfile'])wizard[field]=safeDraft(values.get(field)||'',key);
      wizardAPI(app.getState(),app.helpers());const input=document.getElementById('conn-api-key');if(input)input.value=key;const consent=document.querySelector('#conn-api-form [name="paidConsent"]');if(consent)consent.checked=values.get('paidConsent')==='yes';
    }
  });
  document.addEventListener('DOMContentLoaded',()=>document.getElementById('modal')?.addEventListener('close',()=>{if(document.getElementById('modal')?.open)return;clearCredentialFields();wizard=null;inspectedId=null;diagnosticsInspectedId=null;}));
  window.addEventListener('pagehide',()=>{
    for(const input of document.querySelectorAll('#modal input'))if(containsCredential(input.value)){input.value='';input.removeAttribute('value');}
    clearCredentialFields();wizard=null;credentials.clear();clearTimeout(diagnosticNoticeTimer);document.getElementById('connection-diagnostic-notice')?.remove();
    for(const c of window.OrchestrixApp?.getState().connections||[])if(!c.simulated&&c.mode==='api'){c.credentialPresent=false;cancelDiscovery(c);}
    for(const job of discoveryJobs.values()){clearTimeout(job.timer);job.controller.abort();}discoveryJobs.clear();
  });
  const testing=window.__ORCHESTRIX_TEST_DISCOVERY_ADAPTERS===true?{
    setDiscoveryAdapter(scope,discover){
      if(!providerInfo(scope?.provider)||typeof discover!=='function')throw new TypeError('Invalid discovery adapter');
      discoveryAdapters.set(discoveryScope({apiProvider:scope.provider,endpoint:scope.endpoint,region:scope.region,deployment:scope.deployment,authMethod:scope.authMethod}),{discover,timeoutMs:Math.max(20,Math.min(10000,Number(scope.timeoutMs)||10000))});
    }
  }:{};
  window.OrchestrixConnections = Object.freeze({initial, view, inspect, showDiagnostics, addForm, click, submit, eligible,...testing});
})();
