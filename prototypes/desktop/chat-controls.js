/* Session preferences and explicitly selected text references; no runtime execution. */
(() => {
  'use strict';
  const {html,text,t}=OrchestrixI18n;
  const app=()=>window.OrchestrixApp;
  const modes=[['development','Desenvolvimento'],['research','Pesquisa científica'],['documentation','Documentação'],['presentation','Apresentações'],['refactoring','Refatoração'],['improvements','Sugestões de melhorias']];
  const mode=chat=>modes.some(([id])=>id===chat.mode)?chat.mode:'development';
  const attachments=chat=>Array.isArray(chat.attachments)?chat.attachments.filter(file=>typeof file?.id==='string'&&typeof file.path==='string'&&typeof file.text==='string'&&file.text.length<=102400).slice(0,10):[];
  const modelKey=(connectionId,model)=>JSON.stringify([connectionId,model]);
  function choices(state) {
    return state.connections.filter(connection=>connection.lifecycle!=='disconnected'&&!(state.appSettings?.subscriptionOnly!==false&&connection.mode==='api')).flatMap(connection=>(connection.catalog||[]).filter(model=>typeof model.id==='string'&&typeof model.name==='string'&&model.compatibility?.status!=='unsupported'&&!['embedding','image','audio','video','moderation'].includes(model.kind)).map(model=>({key:modelKey(connection.id,model.id),connectionId:connection.id,model:model.id,label:`${model.name} · ${connection.name}`})));
  }
  function selected(state) {
    const preference=state.chat.orchestrator;
    return preference&&typeof preference.connectionId==='string'&&typeof preference.model==='string'?preference:null;
  }
  const contextChat=(state,task)=>state.conversations?.find(chat=>chat.taskIds?.includes(task?.id))||state.chat;
  function sessionConfig(state,task) {
    const chat=contextChat(state,task),preference=selected({...state,chat});
    return {mode:mode(chat),...(preference?{account:preference.connectionId,model:preference.model}:{})};
  }
  function sources(chat) {
    return attachments(chat).map(file=>({...file,reason:t('Documento anexado à sessão')}));
  }
  function controls(state,h) {
    const preference=selected(state),choice=preference&&choices(state).find(item=>item.key===modelKey(preference.connectionId,preference.model));
    const label=preference?choice?.label||t('Modelo indisponível'):t('Modelo automático');
    return html`<div class="chat-attachments" aria-label="Documentos da sessão">${attachments(state.chat).map(file=>html`<span class="chat-attachment">${h.icon('file')}<span title="${h.esc(file.path)}">${h.esc(file.path)}</span><button type="button" data-remove-attachment="${h.esc(file.id)}" aria-label="${h.esc(text`Remover documento ${file.path}`)}">${h.icon('close')}</button></span>`).join('')}</div><div class="composer-context"><div class="composer-tools"><div class="chat-context-picker"><button type="button" id="chat-add-context" class="context-action composer-add" aria-label="Adicionar contexto" title="Adicionar contexto" aria-haspopup="menu" aria-expanded="false" aria-controls="chat-context-menu">${h.icon('plus')}</button><div id="chat-context-menu" class="chat-context-menu" role="menu" aria-label="Adicionar contexto" hidden><button type="button" role="menuitem" data-action="chat-attach-documents">${h.icon('file')}Anexar documentos</button><button type="button" role="menuitem" data-action="session-attach">${h.icon('layers')}Associar a projeto</button><button type="button" role="menuitem" data-action="${state.isLoose?'session-directory':'project-context'}">${h.icon('folder')}${state.isLoose?t('Definir diretório'):t('Contexto do projeto')}</button></div></div><button type="button" class="context-action composer-model" data-action="session-model" aria-label="Modelo orquestrador da sessão" title="${h.esc(label)}">${h.icon('layers')}<span>${h.esc(label)}</span></button><button type="button" class="context-action composer-mode" data-action="session-mode" aria-label="Modo de trabalho">${h.icon('code')}<span>${t(modes.find(([id])=>id===mode(state.chat))[1])}</span></button></div>${!state.isLoose?html`<span class="composer-project" title="${h.esc(state.project)}">${h.icon('folder')}<span>${h.esc(state.project)}</span></span>`:''}</div><input type="file" id="chat-document-input" accept=".txt,.md,.markdown,.mdx,.csv,.json,.yaml,.yml,.toml,.xml,.html,.css,.js,.ts,.tsx,.jsx,.py,.rs,.log" multiple hidden>`;
  }
  function closeMenu(restore=false) {
    const menu=document.getElementById('chat-context-menu');if(!menu||menu.hidden)return;
    menu.hidden=true;document.getElementById('chat-add-context')?.setAttribute('aria-expanded','false');
    if(restore)document.getElementById('chat-add-context')?.focus({preventScroll:true});
  }
  function openModel() {
    const state=app().getState(),h=app().helpers(),preference=selected(state),key=preference?modelKey(preference.connectionId,preference.model):'auto',items=choices(state);
    h.showModal(t('Modelo orquestrador da sessão'),html`<form id="session-model-form"><label class="form-label" for="session-orchestrator">Modelo orquestrador</label><select id="session-orchestrator" name="orchestrator"><option value="auto" ${key==='auto'?'selected':''}>Modelo automático</option>${items.map(item=>html`<option value="${h.esc(item.key)}" ${item.key===key?'selected':''}>${h.esc(item.label)}</option>`).join('')}${preference&&!items.some(item=>item.key===key)?html`<option value="${h.esc(key)}" selected disabled>Modelo indisponível</option>`:''}</select><p class="field-hint">Os modelos vêm dos catálogos das conexões permitidas. Esta preferência vale para novas tentativas; disponibilidade e execução dependem do runtime.</p><div class="form-actions"><button type="button" class="button" data-action="close-modal">Cancelar</button><button type="submit" class="button primary">Salvar modelo da sessão</button></div></form>`);
  }
  function openMode() {
    const state=app().getState(),h=app().helpers();
    h.showModal(t('Modo de trabalho'),html`<form id="session-mode-form"><label class="form-label" for="session-mode">Modo da sessão</label><select id="session-mode" name="mode">${modes.map(([id,label])=>html`<option value="${id}" ${id===mode(state.chat)?'selected':''}>${t(label)}</option>`).join('')}</select><p class="field-hint">O modo acompanha o contexto das novas tarefas. As tentativas existentes mantêm suas escolhas.</p><div class="form-actions"><button type="button" class="button" data-action="close-modal">Cancelar</button><button type="submit" class="button primary">Salvar modo da sessão</button></div></form>`);
  }
  document.addEventListener('click',event=>{
    const button=event.target.closest('button');
    if(button?.id==='chat-add-context') {
      const menu=document.getElementById('chat-context-menu');menu.hidden=!menu.hidden;button.setAttribute('aria-expanded',String(!menu.hidden));
      if(!menu.hidden)menu.querySelector('button').focus({preventScroll:true});return;
    }
    if(button?.dataset.removeAttachment) {
      const state=app().getState();state.chat.attachments=attachments(state.chat).filter(file=>file.id!==button.dataset.removeAttachment);app().helpers().render();document.getElementById('chat-add-context')?.focus({preventScroll:true});return;
    }
    if(button?.dataset.action==='chat-attach-documents'){closeMenu();document.getElementById('chat-document-input').click();return;}
    if(button?.dataset.action==='session-model'){closeMenu();openModel();return;}
    if(button?.dataset.action==='session-mode'){closeMenu();openMode();return;}
    if(!event.target.closest('.chat-context-picker')||button?.matches('[role="menuitem"]'))closeMenu();
  });
  document.addEventListener('keydown',event=>{
    const menu=document.getElementById('chat-context-menu');if(!menu||menu.hidden)return;
    if(event.key==='Escape'){event.preventDefault();closeMenu(true);return;}
    if(!menu.contains(event.target))return;
    const buttons=[...menu.querySelectorAll('button')],index=buttons.indexOf(document.activeElement);
    if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){event.preventDefault();const next=event.key==='Home'?0:event.key==='End'?buttons.length-1:(index+(event.key==='ArrowDown'?1:-1)+buttons.length)%buttons.length;buttons[next].focus();}
    if(event.key==='Tab')closeMenu();
  });
  document.addEventListener('submit',event=>{
    const form=event.target;if(!['session-model-form','session-mode-form'].includes(form.id))return;
    event.preventDefault();const state=app().getState(),h=app().helpers();
    if(form.id==='session-mode-form') {
      const value=new FormData(form).get('mode');if(!modes.some(([id])=>id===value))return;state.chat.mode=value;
    } else {
      const key=new FormData(form).get('orchestrator'),choice=choices(state).find(item=>item.key===key);
      if(key==='auto')delete state.chat.orchestrator;
      else if(choice)state.chat.orchestrator={connectionId:choice.connectionId,model:choice.model};
      else return;
    }
    h.closeModal();h.render();
  });
  document.addEventListener('change',async event=>{
    if(event.target.id!=='chat-document-input')return;
    const files=[...event.target.files],state=app().getState(),composer=state.chat,h=app().helpers();event.target.value='';if(!files.length)return;
    if(files.length+attachments(state.chat).length>10||files.some(file=>file.size>102400)||files.reduce((size,file)=>size+file.size,attachments(state.chat).reduce((size,file)=>size+(file.bytes||0),0))>512000){h.notify(t('Selecione até 10 documentos de texto, com até 100 KB cada e 500 KB no total.'));return;}
    try {
      const loaded=await Promise.all(files.map(async file=>{
        if(!/\.(txt|md|markdown|mdx|csv|json|ya?ml|toml|xml|html|css|[jt]sx?|py|rs|log)$/i.test(file.name))throw new Error('unsupported-document');
        const bytes=new Uint8Array(await file.arrayBuffer()),content=new TextDecoder('utf-8',{fatal:true}).decode(bytes);if(content.includes('\0'))throw new Error('binary-document');
        return {id:`document-${crypto.randomUUID()}`,path:file.name,text:content,bytes:file.size,version:`file:${file.lastModified}:${file.size}`,origin:'user-attachment'};
      }));
      if(app().getState().chat!==composer){h.notify(t('A sessão mudou durante a leitura. Selecione os documentos novamente na sessão desejada.'));return;}
      const current=app().getState().chat,combined=[...attachments(current),...loaded];
      if(combined.length>10||combined.reduce((size,file)=>size+(file.bytes||0),0)>512000){h.notify(t('Selecione até 10 documentos de texto, com até 100 KB cada e 500 KB no total.'));return;}
      current.attachments=combined;h.render();document.getElementById('chat-add-context')?.focus({preventScroll:true});
    } catch {h.notify(t('Não foi possível ler os documentos. Use arquivos de texto em UTF-8.'));}
  });
  document.addEventListener('orchestrix:language-change',()=>{
    const form=document.querySelector('#modal[open] form');
    if(form?.id==='session-model-form'){const value=form.querySelector('select').value;openModel();document.getElementById('session-orchestrator').value=value;}
    if(form?.id==='session-mode-form'){const value=form.querySelector('select').value;openMode();document.getElementById('session-mode').value=value;}
  });
  window.OrchestrixComposer=Object.freeze({controls,sessionConfig,sources,contextChat});
})();
