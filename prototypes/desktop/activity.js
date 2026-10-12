/* App navigation and menus. The activity rail is independent of the content docks. */
(() => {
  'use strict';
  const {t}=OrchestrixI18n;
  const orderKey='orchestrix-activity-order-v1';
  const defaults=['chat','files','studio','connections'];
  const names={chat:'Conversa',files:'Arquivos',studio:'Studio',connections:'Conexões'};
  let api={},active='chat',drag=null,ignoreClick=false,menuKind=null,menuOpener=null,menuActivity=null,lastEditable=null;
  const app=document.getElementById('app');
  const rail=document.getElementById('activity-items');
  const popup=document.getElementById('app-menu-popup');
  const mobile=()=>matchMedia('(max-width:800px)').matches;
  const dock=()=>window.OrchestrixDock;
  function workspaceExpanded(layout=dock().getLayout()){
    const zone=layout.positions.navigation;
    // A selected rail activity may be behind Sessions or Work in a shared dock.
    return layout.active[zone]==='navigation'&&!layout.collapsed[zone]&&(!mobile()||app.dataset.mobileDock===zone);
  }
  function rememberEditable(event) {
    const element=event?.target||document.activeElement;
    if(element?.matches('textarea,input:not([type]),input:is([type="text"],[type="search"],[type="tel"],[type="url"],[type="password"]),[contenteditable="true"]'))lastEditable={element,start:element.selectionStart,end:element.selectionEnd};
  }
  function editableTarget(){
    const visible=element=>element?.isConnected&&!element.disabled&&!element.closest('[hidden],[inert]')&&element.getClientRects().length>0;
    if(visible(lastEditable?.element))return lastEditable.element;
    const fallback=document.querySelector(app.classList.contains('editing-file')?'.file-text-editor':'#chat-message');
    return visible(fallback)?fallback:null;
  }
  function order(){return [...rail.querySelectorAll('[data-activity]')].map(button=>button.dataset.activity);}
  function saveOrder(){try{localStorage.setItem(orderKey,JSON.stringify(order()));}catch{}}
  function setOrder(ids,persist=true){
    const focused=document.activeElement;
    for(const id of ids)if(defaults.includes(id))rail.append(rail.querySelector(`[data-activity="${id}"]`));
    if(focused?.isConnected&&rail.contains(focused))focused.focus({preventScroll:true});
    if(persist)saveOrder();
  }
  function reorder(id,offset){
    const ids=order(),index=ids.indexOf(id),next=Math.max(0,Math.min(ids.length-1,index+offset));
    if(index<0||index===next)return;
    ids.splice(index,1);ids.splice(next,0,id);setOrder(ids);
  }
  function setActive(id){
    if(!defaults.includes(id))return;
    active=id;
    dock().setAvailable('navigation',id!=='chat');
    for(const button of rail.querySelectorAll('[data-activity]')){
      const selected=button.dataset.activity===id;button.classList.toggle('active',selected);
      button.setAttribute('aria-description',t('Arraste para ordenar. Use Alt com as setas para mover a atividade.'));
      if(selected)button.setAttribute('aria-current','page');else button.removeAttribute('aria-current');
      button.setAttribute('aria-expanded',String(selected&&id!=='chat'&&workspaceExpanded()));
    }
    document.querySelectorAll('[data-activity-pane]').forEach(pane=>{pane.hidden=pane.dataset.activityPane!==id;});
    const label=document.querySelector('#navigation-panel .dock-panel-handle>span:nth-child(2)');
    if(label){label.removeAttribute('data-i18n');label.textContent=t(names[id]);}
    document.querySelectorAll('[data-toggle-dock]').forEach(button=>{
      button.hidden=!dock().hasPanels(button.dataset.toggleDock);
      button.setAttribute('aria-pressed',String(mobile()?app.dataset.mobileDock===button.dataset.toggleDock:!dock().isCollapsed(button.dataset.toggleDock)));
    });
  }
  function closeMobile(){delete app.dataset.mobileDock;setActive(active);}
  function openPrimary(){
    dock()?.activate('navigation');
    if(mobile())app.dataset.mobileDock=dock().getLayout().positions.navigation;
  }
  function select(id,{toggle=false}={}){
    if(!defaults.includes(id))return;
    api.rememberComposer?.();
    const wasActive=active===id,wasExpanded=workspaceExpanded(),zone=dock().getLayout().positions.navigation;
    setActive(id);
    if(id==='chat'){
      window.OrchestrixFiles?.showChat();
      if(api.getState().view!=='chat')api.navigate('chat');
      else api.focusComposer?.();
      closeMobile();return;
    }else if(id==='studio'){document.getElementById('studio-navigation').open=true;window.OrchestrixFiles?.showChat();api.navigate('work');}
    else if(id==='connections'){window.OrchestrixFiles?.showChat();api.navigate('connections');}
    if(toggle&&wasActive&&wasExpanded){
      if(mobile())closeMobile();
      else dock().collapse(zone);
    }else openPrimary();
    setActive(id);
  }
  function sync(state=api.getState?.()){
    if(!state)return;
    if(!window.OrchestrixFiles?.getState().visible){
      if(state.view==='connections')setActive('connections');
      else if(['work','attention','review','history'].includes(state.view))setActive('studio');
      else if(['connections','studio'].includes(active))setActive('chat');
    }
    const label=state.chat?.autoName?t('Nova sessão'):state.chat?.name||t('Nova sessão');
    document.getElementById('titlebar-workspace').textContent=state.project||label;
    document.querySelector('.profile-initial').textContent=(state.appSettings?.profileName||'O').trim().slice(0,1).toUpperCase()||'O';
    const root=document.getElementById('pane-connections');root.replaceChildren();
    for(const connection of state.connections||[]){
      const button=document.createElement('button');button.type='button';button.className='pane-connection';button.dataset.view='connections';
      const name=document.createElement('strong');name.textContent=connection.name||connection.provider;
      const detail=document.createElement('span');detail.textContent=connection.provider||t('Conta');button.append(name,detail);root.append(button);
    }
    if(!root.children.length){const empty=document.createElement('p');empty.className='pane-description';empty.textContent=t('Nenhuma conta conectada');root.append(empty);}
    OrchestrixI18n.localizeStatic(document.querySelector('.app-titlebar'));setActive(active);
  }
  function editorChanged({visible}){
    if(visible){api.rememberComposer?.();setActive('files');closeMobile();}
    document.getElementById('content').hidden=visible;
    document.getElementById('file-editor').hidden=!visible;
    app.classList.toggle('editing-file',visible);
  }
  const menuItems={
    file:()=>[['Novo arquivo','new-file','Ctrl N'],['Abrir arquivo','open-file','Ctrl O'],['Abrir pasta','open-folder','Ctrl Shift O']],
    edit:()=>[['Desfazer','undo','Ctrl Z'],['Refazer','redo','Ctrl Shift Z'],'-',['Recortar','cut','Ctrl X'],['Copiar','copy','Ctrl C'],['Colar','paste','Ctrl V'],'-',['Selecionar tudo','selectAll','Ctrl A']],
    view:()=>[['Conversa','chat'],['Arquivos','files'],['Studio','studio'],['Conexões','connections'],'-',['Alternar painel esquerdo','toggle-left','Ctrl B'],['Alternar painel direito','toggle-right','Ctrl Alt B'],['Modo de foco','focus'],'-',['Restaurar layout','restore-layout']],
    help:()=>[['Ajuda e tutoriais','help'],['Atalhos de teclado','shortcuts'],['Conheça o Orchestrix','website'],['GitHub','github']],
    activity:()=>[['Mover atividade para cima','activity-up'],['Mover atividade para baixo','activity-down'],'-',['Mover workspace para a esquerda','move-left'],['Mover workspace para a direita','move-right']]
  };
  function closeMenu(restore=false){
    popup.hidden=true;popup.replaceChildren();document.querySelectorAll('[data-app-menu]').forEach(button=>button.setAttribute('aria-expanded','false'));
    if(restore&&menuOpener?.isConnected)menuOpener.focus({preventScroll:true});menuKind=null;menuOpener=null;
  }
  function openMenu(kind,opener,activity=null){
    if(menuKind===kind&&menuOpener===opener&&!popup.hidden){closeMenu(true);return;}
    rememberEditable();closeMenu();menuKind=kind;menuOpener=opener;menuActivity=activity;
    for(const item of menuItems[kind]()){
      if(item==='-'){popup.append(document.createElement('hr'));continue;}
      const [label,command,key]=item,button=document.createElement('button');button.type='button';button.setAttribute('role','menuitem');button.dataset.menuCommand=command;
      const text=document.createElement('span');text.textContent=label==='Studio'||label==='GitHub'?label:t(label);button.append(text);
      if(key){const shortcut=document.createElement('kbd');shortcut.textContent=key;button.append(shortcut);}
      if(['undo','redo','cut','copy','paste','selectAll'].includes(command)){
        const target=editableTarget();button.disabled=!target||(target.readOnly&&['undo','redo','cut','paste'].includes(command));
      }
      if(command==='activity-up')button.disabled=order()[0]===activity;
      if(command==='activity-down')button.disabled=order().at(-1)===activity;
      if(command.startsWith('toggle-'))button.disabled=!dock().hasPanels(command.slice(7));
      if(command.startsWith('move-'))button.disabled=activity==='chat';
      popup.append(button);
    }
    popup.hidden=false;opener?.setAttribute('aria-expanded','true');
    const rect=opener.getBoundingClientRect();popup.style.top=`${Math.min(innerHeight-popup.offsetHeight-8,rect.bottom+4)}px`;popup.style.left=`${Math.max(8,Math.min(rect.left,innerWidth-popup.offsetWidth-8))}px`;
    popup.querySelector('button:not(:disabled)')?.focus({preventScroll:true});
  }
  function toggleZone(zone){
    if(mobile()){
      if(app.dataset.mobileDock===zone)closeMobile();else {dock().expand(zone);app.dataset.mobileDock=zone;}
      sync();
    }else dock().toggle(zone);
  }
  async function edit(command){
    const saved=lastEditable,element=editableTarget();
    if(!element)return;
    element.focus({preventScroll:true});if(saved?.element===element&&element.setSelectionRange&&saved.start!==null)element.setSelectionRange(saved.start,saved.end);
    if(command==='selectAll'){if(element.select)element.select();else document.execCommand('selectAll');return;}
    if(command==='paste'){
      try{const value=await navigator.clipboard.readText();if(element.setRangeText){element.setRangeText(value,element.selectionStart,element.selectionEnd,'end');element.dispatchEvent(new Event('input',{bubbles:true}));}else document.execCommand('insertText',false,value);}
      catch{api.notify(t('Use Ctrl+V ou Command+V para colar no campo selecionado.'));}return;
    }
    document.execCommand(command);
  }
  function execute(command){
    const activity=menuActivity;closeMenu();
    if(defaults.includes(command)){select(command);return;}
    if(['undo','redo','cut','copy','paste','selectAll'].includes(command)){edit(command);return;}
    if(command==='open-folder'){select('files');window.OrchestrixFiles.openFolder();return;}
    if(command==='open-file'){select('files');window.OrchestrixFiles.pickFile();return;}
    if(command==='new-file'){select('files');window.OrchestrixFiles.newFile();return;}
    if(command==='focus'){api.runAction('focus');closeMobile();return;}
    if(command==='restore-layout'){dock().restore();setOrder(defaults);closeMobile();return;}
    if(command.startsWith('toggle-')){toggleZone(command.slice(7));return;}
    if(command.startsWith('activity-')){reorder(activity,command==='activity-up'?-1:1);return;}
    if(command.startsWith('move-')){select(activity);dock().move('navigation',command.slice(5));return;}
    if(command==='shortcuts'){api.runAction('shortcuts');return;}
    const links={help:'help.html',website:'website.html',github:'https://github.com/LeonardoMBarca/orchestrix'};
    if(links[command])window.open(links[command],'_blank','noopener,noreferrer');
  }
  function endDrag(){if(drag?.button)drag.button.classList.remove('is-reordering');rail.querySelectorAll('.reorder-target').forEach(button=>button.classList.remove('reorder-target'));drag=null;}
  function init(options){
    api=options;
    try{const saved=JSON.parse(localStorage.getItem(orderKey));if(Array.isArray(saved)&&saved.length===defaults.length&&new Set(saved).size===defaults.length&&saved.every(id=>defaults.includes(id)))setOrder(saved,false);}catch{}
    rail.querySelectorAll('[data-activity]').forEach(button=>{button.setAttribute('aria-keyshortcuts','Alt+ArrowUp Alt+ArrowDown');button.setAttribute('aria-description',t('Arraste para ordenar. Use Alt com as setas para mover a atividade.'));});
    window.OrchestrixFiles.init({notify:api.notify,getState:api.getState,onEditorChange:editorChanged});
    document.addEventListener('focusin',rememberEditable);
    document.addEventListener('focusout',rememberEditable);
    document.addEventListener('pointerdown',event=>{
      if(event.target.closest('#app-menubar'))rememberEditable();
      if(!popup.hidden&&!event.target.closest('#app-menu-popup,[data-app-menu]'))closeMenu();
      const button=event.target.closest('#activity-items>[data-activity]');if(!button||event.button!==0)return;
      drag={button,startX:event.clientX,startY:event.clientY,pointer:event.pointerId,active:false,target:null};button.setPointerCapture(event.pointerId);
    });
    document.addEventListener('pointermove',event=>{
      if(!drag||drag.pointer!==event.pointerId)return;
      if(!drag.active&&Math.hypot(event.clientX-drag.startX,event.clientY-drag.startY)>7){drag.active=true;drag.button.classList.add('is-reordering');}
      if(!drag.active)return;
      const target=[...rail.children].find(button=>{const r=button.getBoundingClientRect();return event.clientY>=r.top&&event.clientY<=r.bottom;});drag.target=target;
      rail.querySelectorAll('[data-activity]').forEach(button=>button.classList.toggle('reorder-target',button===target&&button!==drag.button));
    });
    document.addEventListener('pointerup',event=>{
      if(!drag||drag.pointer!==event.pointerId)return;
      if(drag.active){const ids=order(),from=ids.indexOf(drag.button.dataset.activity),to=ids.indexOf(drag.target?.dataset.activity);if(to>=0&&to!==from){ids.splice(from,1);ids.splice(to,0,drag.button.dataset.activity);setOrder(ids);}ignoreClick=true;setTimeout(()=>{ignoreClick=false;},0);}endDrag();
    });
    document.addEventListener('pointercancel',endDrag);addEventListener('blur',endDrag);
    document.addEventListener('contextmenu',event=>{const button=event.target.closest('#activity-items>[data-activity]');if(button){event.preventDefault();openMenu('activity',button,button.dataset.activity);}});
    document.addEventListener('click',event=>{
      const activity=event.target.closest('[data-activity]');if(activity){if(!ignoreClick)select(activity.dataset.activity,{toggle:rail.contains(activity)});return;}
      const menu=event.target.closest('[data-app-menu]');if(menu){openMenu(menu.dataset.appMenu,menu);return;}
      const command=event.target.closest('[data-menu-command]');if(command){execute(command.dataset.menuCommand);return;}
      const toggle=event.target.closest('[data-toggle-dock]');if(toggle){toggleZone(toggle.dataset.toggleDock);return;}
      if(event.target.closest('[data-close-mobile]')){closeMobile();return;}
      if(event.target.closest('[data-activity-profile]')){window.OrchestrixSettings.open('general');return;}
      const settings=event.target.closest('[data-shell-settings]');if(settings)window.OrchestrixSettings.open(settings.dataset.shellSettings);
    });
    document.addEventListener('keydown',event=>{
      const activity=event.target.closest('#activity-items>[data-activity]');
      if(activity&&event.altKey&&['ArrowUp','ArrowDown'].includes(event.key)){event.preventDefault();reorder(activity.dataset.activity,event.key==='ArrowUp'?-1:1);return;}
      if(activity&&event.key==='F10'&&event.shiftKey){event.preventDefault();openMenu('activity',activity,activity.dataset.activity);return;}
      if(!popup.hidden){
        if(event.key==='Escape'){event.preventDefault();closeMenu(true);return;}
        if(['ArrowDown','ArrowUp','Home','End'].includes(event.key)){const buttons=[...popup.querySelectorAll('button:not(:disabled)')],current=buttons.indexOf(document.activeElement),next=event.key==='Home'?0:event.key==='End'?buttons.length-1:(current+(event.key==='ArrowDown'?1:buttons.length-1))%buttons.length;event.preventDefault();buttons[next]?.focus();return;}
        if(['ArrowLeft','ArrowRight'].includes(event.key)&&menuKind!=='activity'){const buttons=[...document.querySelectorAll('[data-app-menu]')],index=buttons.indexOf(menuOpener),next=buttons[(index+(event.key==='ArrowRight'?1:buttons.length-1))%buttons.length];event.preventDefault();openMenu(next.dataset.appMenu,next);return;}
      }
      const menu=event.target.closest('[data-app-menu]');if(menu&&event.key==='ArrowDown'){event.preventDefault();openMenu(menu.dataset.appMenu,menu);return;}
      if(event.key==='Escape'){closeMobile();endDrag();}
      if((event.ctrlKey||event.metaKey)&&!event.shiftKey&&event.key===','){event.preventDefault();window.OrchestrixSettings.open('general');}
      if((event.ctrlKey||event.metaKey)&&event.shiftKey&&event.key.toLowerCase()==='e'){event.preventDefault();select('files');}
      if((event.ctrlKey||event.metaKey)&&!event.shiftKey&&event.key.toLowerCase()==='b'){event.preventDefault();toggleZone(event.altKey?'right':'left');}
      if((event.ctrlKey||event.metaKey)&&!event.shiftKey&&!event.altKey&&event.key.toLowerCase()==='n'){event.preventDefault();execute('new-file');}
      if((event.ctrlKey||event.metaKey)&&!event.altKey&&event.key.toLowerCase()==='o'){event.preventDefault();execute(event.shiftKey?'open-folder':'open-file');}
    });
    document.addEventListener('orchestrix:files-chat',()=>{if(api.getState().view!=='chat')api.navigate('chat');setActive('chat');closeMobile();api.focusComposer?.();});
    document.addEventListener('orchestrix:layout-change',()=>setActive(active));
    document.addEventListener('orchestrix:panel-move',({detail})=>{
      if(mobile()&&detail.wasActive&&app.dataset.mobileDock===detail.from){app.dataset.mobileDock=detail.zone;setActive(active);}
    });
    document.addEventListener('orchestrix:language-change',()=>{closeMenu();sync();});
    addEventListener('resize',()=>{closeMenu();if(!mobile())closeMobile();});
    sync();
  }
  window.OrchestrixShell=Object.freeze({init,sync,select,closeMobile,getActivity:()=>active,getOrder:order});
})();
