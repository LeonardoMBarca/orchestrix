/* Modeless application settings. Provider telemetry is displayed only when observed. */
(() => {
  'use strict';
  const I=window.OrchestrixI18n;
  const {html,t}=I;
  const storageKey='orchestrix-app-preferences';
  const esc=value=>String(value??'').replace(/[&<>"']/g,char=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[char]));
  const clean=(value,max)=>typeof value==='string'?value.replace(/\u0000/g,'').slice(0,max):'';
  function validate(value) {return {profileName:clean(value?.profileName,80),systemPrompt:clean(value?.systemPrompt,4000),subscriptionOnly:value?.subscriptionOnly!==false};}
  let saved=validate(null);
  try {saved=validate(JSON.parse(localStorage.getItem(storageKey)||'{}'));} catch {}
  let draft={...saved};
  let api={};
  let panel;
  let activeTab='general';
  let opener;
  let drag;
  let observer;
  let saving=false;
  let orchestrationDraft;
  const tabs=[['general','Geral'],['accounts','Contas'],['themes','Temas'],['conversation','Conversa e respostas'],['orchestration','Orquestração'],['layout','Layout dos painéis']];
  const state=()=>api.getState?.()||{};
  const ui=()=>api.getUI?.()||{};
  const validFocus=element=>element instanceof HTMLElement&&element.isConnected&&!element.disabled&&element.getClientRects().length>0&&!element.closest('[inert]');
  const option=(value,label,current)=>html`<option value="${esc(value)}" ${value===current?'selected':''}>${esc(label)}</option>`;
  const icons={close:'m6 6 12 12M18 6 6 18',drag:'M5 8h14M5 12h14M5 16h14',general:'M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8ZM4 21v-2a8 8 0 0 1 16 0v2',accounts:'m8 3 1 4m7-4-1 4M6 7h12v4a6 6 0 0 1-12 0V7ZM12 17v4',themes:'M12 3a9 9 0 1 0 0 18h2a2 2 0 0 0 2-2c0-1-1-2-1-3s1-2 2-2h1a3 3 0 0 0 3-3 9 9 0 0 0-9-8ZM7 8h.1M12 6h.1M17 9h.1M6 13h.1',conversation:'M3 4h18v13H9l-6 4V4ZM7 8h10M7 12h7',orchestration:'M12 3 3 8l9 5 9-5-9-5ZM3 12l9 5 9-5M3 16l9 5 9-5',layout:'M3 4h18v16H3V4ZM8 4v16M8 9h13'};
  const icon=name=>`<svg viewBox="0 0 24 24" aria-hidden="true"><path d="${icons[name]||icons.drag}"/></svg>`;
  function scaleControl(current) {
    const value=typeof current.textScale==='number'?current.textScale:current.textSize==='large'?200:100;
    return html`<div class="settings-text-scale"><div class="settings-scale-heading"><label for="settings-text-size">Tamanho do texto</label><output id="settings-text-size-value" for="settings-text-size">${value}%</output></div><input type="range" id="settings-text-size" min="80" max="200" step="5" value="${value}" data-settings-scale aria-valuetext="${value}%" style="--scale-progress:${(value-80)/120*100}%"><div class="settings-scale-marks"><span>80%</span><button type="button" data-settings-reset-scale>Restaurar 100%</button><span>200%</span></div><p class="settings-description settings-small">Ajuste o tamanho do texto sem alterar suas conversas.</p></div>`;
  }
  function general() {
    const current=ui();
    return html`<h3>Seu perfil</h3><p class="settings-description">Personalize sua experiência sem interromper a conversa.</p><form data-settings-form="profile"><label for="settings-profile-name">Nome de exibição</label><input id="settings-profile-name" name="profileName" autocomplete="nickname" maxlength="80" value="${esc(draft.profileName)}" placeholder="Como devemos chamar você?"><div class="settings-two-columns"><div><label for="settings-language">Idioma da interface</label><select id="settings-language" data-settings-language>${I.languages.map(language=>option(language.id,language.name,I.language)).join('')}</select></div><div><label for="settings-density">Densidade da interface</label><select id="settings-density" data-settings-ui="density">${option('compact',t('Compacta'),current.density||'compact')}${option('comfortable',t('Confortável'),current.density||'compact')}</select></div></div>${scaleControl(current)}<div class="settings-actions"><button class="button primary" type="submit">Salvar perfil</button></div></form>`;
  }
  // The adapter must identify telemetry as observed and name its origin. Fixtures never qualify.
  function observedUsage(connection) {
    if(connection.simulated)return null;
    const record=api.getAccountUsage?.(connection)||connection.usage;
    return record?.observed===true&&['provider','runtime'].includes(record.source)?record:null;
  }
  function date(value) {
    if(typeof value!=='string'||value.length>80)return null;
    const parsed=new Date(value);
    return Number.isFinite(parsed.getTime())?new Intl.DateTimeFormat(I.language,{dateStyle:'medium',timeStyle:'short'}).format(parsed):null;
  }
  function usage(connection) {
    const record=observedUsage(connection);
    const percent=record?.usedPercent;
    const known=typeof percent==='number'&&Number.isFinite(percent)&&percent>=0&&percent<=100;
    const reset=date(record?.resetAt);
    const credit=record?.credit;
    function money(value) {
      if(!credit||typeof value!=='number'||!Number.isFinite(value)||value<0||typeof credit.currency!=='string'||!/^[A-Z]{3}$/.test(credit.currency))return null;
      if(Intl.supportedValuesOf&&!Intl.supportedValuesOf('currency').includes(credit.currency))return null;
      try{return new Intl.NumberFormat(I.language,{style:'currency',currency:credit.currency}).format(value);}catch{return null;}
    }
    const remaining=money(credit?.remaining),spent=money(credit?.spent);
    return html`<dl class="settings-account-facts"><div><dt>Limite utilizado</dt><dd>${known?`${new Intl.NumberFormat(I.language,{maximumFractionDigits:1}).format(percent)}%`:t('Não informado')}${known?html`<progress max="100" value="${percent}" aria-label="Limite utilizado"></progress>`:''}</dd></div><div><dt>Renovação do limite</dt><dd>${reset?esc(reset):t('Não informado')}</dd></div>${connection.mode==='api'?html`<div><dt>Crédito utilizado</dt><dd>${spent?esc(spent):t('Não informado')}</dd></div><div><dt>Crédito disponível</dt><dd>${remaining?esc(remaining):t('Não informado')}</dd></div>`:''}</dl>`;
  }
  function accounts() {
    const connections=state().connections||[];
    return html`<div class="settings-section-heading"><h3>Suas contas</h3><button class="button" type="button" data-settings-add-account>Adicionar conta</button></div><p class="settings-description">Veja as conexões disponíveis e o uso informado pelo provedor.</p>${connections.length?html`<div class="settings-account-list">${connections.map(connection=>{
      const verified=!connection.simulated&&connection.authorizationValid===true&&connection.lifecycle==='active';
      return html`<article class="settings-account"><header><div><strong>${esc(connection.name||connection.provider||t('Conta'))}</strong><small>${esc(connection.provider||t('Provedor não informado'))}</small></div><span class="settings-account-state">${verified?t('Conectada'):t('Indisponível')}</span></header><p class="settings-account-mode">${connection.mode==='api'?t('API · cobrança separada'):t('Assinatura do provedor')}</p>${usage(connection)}${!connection.simulated?html`<div class="settings-resource-summary"><span role="status">${esc(window.OrchestrixResources.summary(connection))}</span><div class="settings-connection-actions"><button class="button" type="button" data-settings-diagnostics="${esc(connection.id)}">Ver diagnóstico</button><button class="button" type="button" data-settings-resources="${esc(connection.id)}">Ver recursos</button></div></div>`:''}</article>`;
    }).join('')}</div>`:html`<div class="settings-empty"><h4>Nenhuma conta conectada</h4><p>Conecte uma assinatura ou configure uma API para disponibilizar seus agentes.</p></div>`}<section class="settings-policy"><strong>Subscription Only</strong><p>O uso de assinaturas não ativa APIs pagas automaticamente. Cada conexão de API exige autorização explícita para cobrança separada.</p></section>`;
  }
  function themes() {
    const themes=api.getThemes?.()||[];
    const current=document.documentElement.dataset.direction||'studio';
    return html`<h3>Escolha seu ambiente</h3><p class="settings-description">O tema muda imediatamente. Studio é a identidade padrão do Orchestrix.</p><fieldset class="settings-theme-grid"><legend class="sr-only">Tema do workspace</legend>${themes.map(theme=>html`<label class="settings-theme"><input type="radio" name="settings-theme" value="${esc(theme.id)}" ${current===theme.id?'checked':''} aria-label="${esc(theme.name)}"><span class="settings-theme-sample" aria-hidden="true" style="${(theme.colors||[]).map((color,index)=>/^#[0-9a-f]{3,8}$/i.test(color)?`--sample-${index}:${color}`:'').join(';')}"><i></i><b></b><em></em></span><strong>${esc(theme.name)}${theme.id==='studio'?html`<small>Padrão</small>`:''}</strong><span>${esc(t(theme.description||''))}</span></label>`).join('')}</fieldset><div class="settings-actions"><button class="button" type="button" data-settings-restore-theme>Restaurar Studio</button></div>`;
  }
  function conversation() {
    return html`<h3>Como seus agentes respondem</h3><p class="settings-description">Defina o tom e as preferências que acompanharão suas próximas tarefas.</p><form data-settings-form="conversation"><label for="settings-system-prompt">Instruções pessoais</label><textarea id="settings-system-prompt" name="systemPrompt" maxlength="4000" rows="9" placeholder="Por exemplo: use um tom direto, explique decisões importantes e prefira respostas curtas.">${esc(draft.systemPrompt)}</textarea><p class="settings-description settings-small">Estas instruções complementam os objetivos e as regras de segurança de cada tarefa. Alterações valem para novas tentativas.</p><div class="settings-actions"><button class="button primary" type="submit">Salvar instruções</button></div></form>`;
  }
  const orchestrationFields={routing:['Prioridade de roteamento',[['balanced','Equilíbrio entre qualidade e disponibilidade'],['quality','Qualidade, respeitando limites'],['availability','Disponibilidade, entre opções elegíveis']]],thinking:['Raciocínio preferido',[['auto','Gerenciado pelo runtime'],['medium','Médio'],['high','Alto']]],context:['Contexto inicial',[['focused','Focado na tarefa'],['module','Ampliado para o módulo']]],review:['Preferência de revisor',[['fresh','Sessão nova no runtime disponível'],['other','Preferir outro provedor, se elegível']]]};
  function orchestration() {
    orchestrationDraft||={...(state().preferences||{})};
    const preferences=orchestrationDraft;
    const connections=state().connections||[];
    const modelConnections=preferences.account==='auto'?connections:connections.filter(connection=>connection.id===preferences.account);
    const models=modelConnections.flatMap(connection=>connection.catalog||[]).filter((model,index,items)=>items.findIndex(other=>other.id===model.id)===index);
    const choices=[['runtime',t('Preferido pelo runtime')],['favorite',t('Favorito do perfil')],...models.map(model=>[model.id,model.name])];
    if(preferences.model&&!choices.some(([id])=>id===preferences.model))choices.push([preferences.model,preferences.model]);
    return html`<h3>Seu jeito de orquestrar</h3><p class="settings-description">Uma conta ou várias, com escolhas ajustadas ao trabalho.</p><form data-settings-form="orchestration"><div class="settings-two-columns">${Object.entries(orchestrationFields).map(([key,[label,items]])=>html`<div><label for="settings-routing-${key}">${t(label)}</label><select id="settings-routing-${key}" name="${key}">${items.map(([value,source])=>option(value,t(source),preferences[key])).join('')}</select></div>`).join('')}<div><label for="settings-routing-account">Conexão preferida</label><select id="settings-routing-account" name="account">${option('auto',t('Escolher automaticamente'),preferences.account)}${connections.map(connection=>option(connection.id,connection.name,preferences.account)).join('')}</select></div><div><label for="settings-routing-model">Estratégia de modelo</label><select id="settings-routing-model" name="model">${choices.map(([value,label])=>option(value,label,preferences.model)).join('')}</select></div></div><p class="settings-description settings-small">Preferências orientam novas tentativas. As escolhas efetivas respeitam as capacidades e os limites de cada runtime.</p><div class="settings-actions"><button class="button" type="button" data-action="policy">Inspecionar política</button><button class="button primary" type="submit">Salvar orquestração</button></div></form>`;
  }
  function layout() {
    const dock=api.dock||window.OrchestrixDock;
    const current=dock?.getLayout?.()||{};
    const position=id=>current.positions?.[id]||(typeof current[id]==='string'?current[id]:current[id]?.zone)||current.panels?.[id]?.zone||({navigation:'right',sessions:'left',work:'right'}[id]);
    return html`<h3>Organize seu espaço</h3><p class="settings-description">Arraste os títulos dos painéis ou escolha onde cada um fica. Painéis no mesmo lado compartilham abas.</p><div class="settings-layout-options">${[['navigation','Navegação'],['sessions','Sessões e projetos'],['work','Trabalho']].map(([id,label])=>html`<div><label for="settings-dock-${id}">${t(label)}</label><select id="settings-dock-${id}" data-settings-dock="${id}" ${dock?.move?'':'disabled'}>${[['left','Esquerda'],['right','Direita']].map(([value,source])=>option(value,t(source),position(id))).join('')}</select></div>`).join('')}</div><div class="settings-actions"><button class="button" type="button" data-settings-restore-layout ${dock?.restore?'':'disabled'}>Restaurar layout</button></div>`;
  }
  const views={general,accounts,themes,conversation,orchestration,layout};
  function render() {
    if(!panel)return;
    const focused=panel.contains(document.activeElement)?document.activeElement:null;
    const selection=focused&&typeof focused.selectionStart==='number'?{start:focused.selectionStart,end:focused.selectionEnd,direction:focused.selectionDirection}:null;
    const focusedId=focused?.id;
    const focusedTab=focused?.dataset.settingsTab;
    const scroll=panel.querySelector('.floating-settings-body')?.scrollTop||0;
    panel.setAttribute('aria-label',t('Configurações do Orchestrix'));
    panel.innerHTML=html`<header class="floating-settings-header"><div class="settings-drag-handle" role="button" tabindex="0" aria-label="Mover janela de configurações" title="Arraste para mover; use as setas para ajustar a posição">${icon('drag')}<h2>Configurações</h2></div><button class="icon-button" type="button" data-settings-close aria-label="Fechar configurações">${icon('close')}</button></header><div class="floating-settings-content"><nav class="floating-settings-tabs" aria-label="Seções das configurações">${tabs.map(([id,label])=>html`<button type="button" id="settings-tab-${id}" data-settings-tab="${id}" aria-current="${id===activeTab?'page':'false'}">${icon(id)}<span>${t(label)}</span><i aria-hidden="true"></i></button>`).join('')}</nav><section class="floating-settings-body" aria-labelledby="settings-tab-${activeTab}">${views[activeTab]()}<p class="settings-save-status" role="status" aria-live="polite">${saving?t('Preferências salvas'):''}</p></section></div>`;
    panel.querySelector('.floating-settings-body').scrollTop=scroll;
    const target=focusedId?panel.querySelector(`#${CSS.escape(focusedId)}`):focusedTab?panel.querySelector(`[data-settings-tab="${focusedTab}"]`):null;
    if(target){target.focus({preventScroll:true});if(selection&&target.setSelectionRange)target.setSelectionRange(selection.start,selection.end,selection.direction);}
    bound();
  }
  function bound() {
    if(!panel||panel.hidden||drag)return;
    const rect=panel.getBoundingClientRect();
    const margin=8;
    const left=Math.max(margin,Math.min(innerWidth-rect.width-margin,rect.left));
    const top=Math.max(margin,Math.min(innerHeight-rect.height-margin,rect.top));
    if(Math.abs(rect.left-left)>.5)panel.style.left=`${left}px`;
    if(Math.abs(rect.top-top)>.5)panel.style.top=`${top}px`;
  }
  function setScale(value) {
    const current=ui();current.textScale=Math.max(80,Math.min(200,Math.round(Number(value)/5)*5));
    api.saveUI?.(current);
    const range=panel?.querySelector('#settings-text-size'),output=panel?.querySelector('#settings-text-size-value');
    if(range){range.value=String(current.textScale);range.setAttribute('aria-valuetext',`${current.textScale}%`);range.style.setProperty('--scale-progress',`${(current.textScale-80)/120*100}%`);}
    if(output)output.textContent=`${current.textScale}%`;
    bound();
  }
  function ensurePanel() {
    if(panel)return;
    panel=document.createElement('section');panel.id='floating-settings';panel.className='floating-settings';panel.hidden=true;
    panel.setAttribute('role','dialog');panel.setAttribute('aria-modal','false');
    document.body.append(panel);
    panel.addEventListener('click',event=>{
      if(event.target.closest('[data-settings-close]')){close();return;}
      const tab=event.target.closest('[data-settings-tab]');
      if(tab){activeTab=tab.dataset.settingsTab;saving=false;render();return;}
      const diagnostic=event.target.closest('[data-settings-diagnostics]');
      if(diagnostic){window.OrchestrixConnections.showDiagnostics(diagnostic.dataset.settingsDiagnostics,state(),api.helpers());return;}
      const resources=event.target.closest('[data-settings-resources]');
      if(resources){window.OrchestrixConnections.inspect(resources.dataset.settingsResources,state(),api.helpers());return;}
      if(event.target.closest('[data-settings-add-account]')){
        if(api.connectAccount)api.connectAccount();
        else if(api.helpers&&window.OrchestrixConnections?.addForm)window.OrchestrixConnections.addForm(state(),api.helpers());
      }
      if(event.target.closest('[data-settings-restore-layout]')){(api.dock||window.OrchestrixDock)?.restore?.();render();}
      if(event.target.closest('[data-settings-restore-theme]')){api.setTheme?.('studio');render();}
      if(event.target.closest('[data-settings-reset-scale]'))setScale(100);
    });
    panel.addEventListener('input',event=>{
      if(event.target.id==='settings-profile-name')draft.profileName=clean(event.target.value,80);
      if(event.target.id==='settings-system-prompt')draft.systemPrompt=clean(event.target.value,4000);
      if(event.target.matches('[data-settings-scale]'))setScale(event.target.value);
    });
    panel.addEventListener('change',event=>{
      if(event.target.matches('[data-settings-language]')){I.setLanguage(event.target.value);return;}
      if(event.target.name==='settings-theme'){api.setTheme?.(event.target.value);return;}
      if(event.target.matches('[data-settings-scale]')){setScale(event.target.value);return;}
      if(event.target.dataset.settingsUi){const current=ui();current[event.target.dataset.settingsUi]=event.target.value;api.saveUI?.(current);}
      if(event.target.dataset.settingsDock)(api.dock||window.OrchestrixDock)?.move?.(event.target.dataset.settingsDock,event.target.value);
      if(event.target.closest('[data-settings-form="orchestration"]')){
        orchestrationDraft[event.target.name]=event.target.value;
        if(event.target.name==='account')render();
      }
    });
    panel.addEventListener('submit',event=>{
      const form=event.target;
      if(!form.matches('[data-settings-form]'))return;
      event.preventDefault();event.stopPropagation();
      if(form.dataset.settingsForm==='orchestration'){
        const changes=Object.fromEntries(new FormData(form));
        const valid={};
        for(const [key,value] of Object.entries(changes))if(orchestrationFields[key]?.[1].some(([id])=>id===value))valid[key]=value;
        if(changes.account==='auto'||state().connections?.some(connection=>connection.id===changes.account))valid.account=changes.account;
        if(['runtime','favorite'].includes(changes.model)||state().connections?.some(connection=>connection.catalog?.some(model=>model.id===changes.model)))valid.model=changes.model;
        if(api.savePreferences)api.savePreferences(valid);else Object.assign(state().preferences||{},valid);
      }else{
        saved=validate(draft);draft={...saved};
        try{localStorage.setItem(storageKey,JSON.stringify(saved));}catch{}
        api.updateSettings?.({...saved});
      }
      saving=true;panel.querySelector('.settings-save-status').textContent=t('Preferências salvas');
    });
    panel.addEventListener('keydown',event=>{
      if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();return;}
      if(event.target.matches('.settings-drag-handle')&&['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(event.key)){
        event.preventDefault();const rect=panel.getBoundingClientRect();const step=event.shiftKey?32:12;
        panel.style.left=`${rect.left+(event.key==='ArrowRight'?step:event.key==='ArrowLeft'?-step:0)}px`;
        panel.style.top=`${rect.top+(event.key==='ArrowDown'?step:event.key==='ArrowUp'?-step:0)}px`;bound();
      }
    });
    panel.addEventListener('pointerdown',event=>{
      const handle=event.target.closest('.settings-drag-handle');if(!handle||event.button!==0)return;
      const rect=panel.getBoundingClientRect();drag={id:event.pointerId,x:event.clientX,y:event.clientY,left:rect.left,top:rect.top};
      handle.setPointerCapture(event.pointerId);panel.classList.add('is-dragging');event.preventDefault();
    });
    panel.addEventListener('pointermove',event=>{
      if(!drag||event.pointerId!==drag.id)return;
      const rect=panel.getBoundingClientRect();
      panel.style.left=`${Math.max(8,Math.min(innerWidth-rect.width-8,drag.left+event.clientX-drag.x))}px`;
      panel.style.top=`${Math.max(8,Math.min(innerHeight-rect.height-8,drag.top+event.clientY-drag.y))}px`;
    });
    const finish=()=>{drag=null;panel.classList.remove('is-dragging');bound();};
    panel.addEventListener('pointerup',finish);panel.addEventListener('pointercancel',finish);panel.addEventListener('lostpointercapture',finish);
    if(window.ResizeObserver){observer=new ResizeObserver(bound);observer.observe(panel);}
    window.addEventListener('resize',bound);
  }
  function open(tab='general') {
    ensurePanel();
    const reopening=panel.hidden;
    if(reopening){opener=document.activeElement;panel.hidden=false;}
    activeTab=Object.hasOwn(views,tab)?tab:'general';saving=false;render();
    if(reopening){
      const margin=innerWidth<=640?16:48;
      panel.style.width=`${Math.min(940,innerWidth-margin)}px`;panel.style.height=`${Math.min(720,innerHeight-24)}px`;
      const rect=panel.getBoundingClientRect();panel.style.left=`${Math.max(8,(innerWidth-rect.width)/2)}px`;panel.style.top=`${Math.max(8,(innerHeight-rect.height)/2)}px`;bound();
    }
    panel.querySelector(`[data-settings-tab="${activeTab}"]`).focus({preventScroll:true});
  }
  function close(restoreFocus=true) {
    if(!panel||panel.hidden)return;
    const restore=panel.contains(document.activeElement);panel.hidden=true;drag=null;
    if(restoreFocus&&restore&&validFocus(opener))opener.focus({preventScroll:true});
  }
  function configure(options) {
    api={...api,...options};api.updateSettings?.({...saved});
    if(panel&&!panel.hidden)render();
  }
  function connectApp() {if(window.OrchestrixApp)configure(window.OrchestrixApp);}
  document.addEventListener('orchestrix:app-ready',connectApp);
  document.addEventListener('orchestrix:language-change',()=>{if(panel&&!panel.hidden)render();});
  document.addEventListener('orchestrix:panel-move',()=>{if(panel&&!panel.hidden&&activeTab==='layout')render();});
  document.addEventListener('close',event=>{if(event.target instanceof HTMLDialogElement&&panel&&!panel.hidden&&activeTab==='accounts')render();},true);
  document.addEventListener('pointerdown',event=>{
    if(!panel||panel.hidden||panel.contains(event.target)||document.querySelector('#modal')?.open)return;
    close(false);
  },true);
  window.OrchestrixSettings=Object.freeze({configure,open,close,refresh:()=>{if(panel&&!panel.hidden)render();},getPreferences:()=>({...saved}),get isOpen(){return Boolean(panel&&!panel.hidden);}});
  connectApp();
})();
