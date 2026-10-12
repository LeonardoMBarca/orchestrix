/* Movable workspace panels. Layout is a UI preference, independent of execution. */
(() => {
  'use strict';
  const {t}=OrchestrixI18n;
  const zones=['left','right'];
  const names={navigation:'Workspace',sessions:'Sessões',work:'Trabalho'};
  const storageKey='orchestrix-panel-layout-v3';
  const initial=()=>({version:3,positions:{navigation:'left',sessions:'right',work:'right'},active:{left:'navigation',right:'sessions'},sizes:{left:224,right:280},collapsed:{left:false,right:false}});
  const boundedSize=size=>Math.max(210,Math.min(440,size));
  let layout=initial();
  try {
    const saved=JSON.parse(localStorage.getItem(storageKey)||localStorage.getItem('orchestrix-panel-layout-v2')||localStorage.getItem('orchestrix-panel-layout-v1')||'null');
    const oldDefault=saved?.version===1&&saved.positions?.navigation==='left'&&saved.positions?.sessions==='right'&&saved.positions?.work==='right';
    const untouchedV2=saved?.version===2&&saved.positions?.navigation==='right'&&saved.positions?.sessions==='left'&&saved.positions?.work==='right'&&saved.active?.left==='sessions'&&saved.active?.right==='navigation'&&saved.sizes?.left===280&&saved.sizes?.right===224;
    if([1,2,3].includes(saved?.version)&&!oldDefault&&!untouchedV2) {
      for(const id of Object.keys(names))if(zones.includes(saved.positions?.[id]))layout.positions[id]=saved.positions[id];
      for(const zone of zones) {
        if(Object.hasOwn(names,saved.active?.[zone]))layout.active[zone]=saved.active[zone];
        const size=Number(saved.sizes?.[zone]);
        if(Number.isFinite(size))layout.sizes[zone]=boundedSize(size);
        if(saved.version===3)layout.collapsed[zone]=saved.collapsed?.[zone]===true;
      }
    }
  } catch {}
  const app=document.getElementById('app');
  const targets=document.getElementById('dock-drop-targets');
  const menu=document.getElementById('dock-position-menu');
  const available={navigation:false,sessions:true,work:false};
  let dragging=null;
  let menuOpener=null;
  const panel=id=>document.getElementById(`${id}-panel`);
  const save=()=>{try{localStorage.setItem(storageKey,JSON.stringify(layout));}catch{}};
  const zonePanels=zone=>Object.keys(names).filter(id=>layout.positions[id]===zone&&available[id]);
  function applySize(zone,transientSize) {
    const container=document.querySelector(`[data-dock-zone="${zone}"]`);
    const ids=zonePanels(zone),collapsed=layout.collapsed[zone],visible=ids.length>0&&!collapsed;
    container.classList.toggle('is-collapsed',collapsed);
    app.classList.toggle(`has-${zone}`,visible);
    app.style.setProperty(`--dock-${zone}`,`${visible?(transientSize??layout.sizes[zone]):0}px`);
    const tabs=container.querySelector('.dock-tabs');if(tabs)tabs.hidden=collapsed;
    for(const id of Object.keys(names).filter(id=>layout.positions[id]===zone))panel(id).hidden=!visible||!available[id]||layout.active[zone]!==id;
    const resizer=container.querySelector('.dock-resizer');
    if(resizer){resizer.setAttribute('aria-valuenow',String(visible?Math.round(transientSize??layout.sizes[zone]):0));resizer.tabIndex=ids.length?0:-1;}
  }
  function apply() {
    const focused=document.activeElement;
    for(const zone of zones) {
      const container=document.querySelector(`[data-dock-zone="${zone}"]`);
      const ids=zonePanels(zone);
      for(const id of Object.keys(names).filter(id=>layout.positions[id]===zone))if(panel(id).parentElement!==container)container.append(panel(id));
      container.querySelector('.dock-tabs')?.remove();
      if(!ids.includes(layout.active[zone]))layout.active[zone]=ids[0]||null;
      container.classList.toggle('has-panels',ids.length>0);
      container.dataset.activePanel=layout.active[zone]||'';
      if(ids.length>1) {
        const tablist=document.createElement('div');
        tablist.className='dock-tabs';tablist.setAttribute('role','tablist');tablist.setAttribute('aria-label',t('Painéis do workspace'));
        for(const id of ids) {
          const button=document.createElement('button');
          button.type='button';button.className='dock-tab';button.dataset.dockTab=id;button.id=`dock-tab-${id}`;
          button.setAttribute('role','tab');button.setAttribute('aria-controls',`${id}-panel`);
          button.setAttribute('aria-selected',String(layout.active[zone]===id));button.tabIndex=layout.active[zone]===id?0:-1;
          button.textContent=t(names[id]);button.title=t('Arraste para reposicionar ou use as setas com Shift');
          tablist.append(button);
        }
        container.prepend(tablist);
      }
      for(const id of Object.keys(names).filter(id=>layout.positions[id]===zone)) {
        const element=panel(id);
        element.hidden=!available[id]||layout.active[zone]!==id;
        if(ids.length>1){element.setAttribute('role','tabpanel');element.setAttribute('aria-labelledby',`dock-tab-${id}`);}
        else {element.removeAttribute('role');element.removeAttribute('aria-labelledby');}
      }
      let resizer=container.querySelector('.dock-resizer');
      if(!resizer) {
        resizer=document.createElement('button');resizer.type='button';resizer.className='dock-resizer';resizer.dataset.resizeDock=zone;
        resizer.setAttribute('role','separator');resizer.setAttribute('aria-orientation','vertical');container.append(resizer);
      }
      resizer.setAttribute('aria-label',t(zone==='left'?'Largura do painel esquerdo':'Largura do painel direito'));
      resizer.setAttribute('aria-valuemin','0');resizer.setAttribute('aria-valuemax','440');
      applySize(zone);
    }
    if(focused!==document.body&&document.contains(focused)&&!focused.closest('[hidden]'))focused.focus({preventScroll:true});
    OrchestrixI18n.localizeStatic();
    document.dispatchEvent(new CustomEvent('orchestrix:layout-change',{detail:{layout:structuredClone(layout),available:{...available}}}));
  }
  function activate(id,focus=false) {
    if(!Object.hasOwn(names,id)||!available[id])return false;
    const zone=layout.positions[id];layout.active[zone]=id;layout.collapsed[zone]=false;apply();save();
    if(focus)document.getElementById(`dock-tab-${id}`)?.focus({preventScroll:true});
    return true;
  }
  function move(id,zone) {
    if(!Object.hasOwn(names,id)||!zones.includes(zone))return false;
    const focused=document.activeElement;
    layout.positions[id]=zone;layout.active[zone]=id;layout.collapsed[zone]=false;apply();save();
    if(focused&&panel(id).contains(focused)&&!focused.closest('[hidden]'))focused.focus({preventScroll:true});
    document.dispatchEvent(new CustomEvent('orchestrix:panel-move',{detail:{id,zone}}));
    return true;
  }
  function collapse(zone) {
    if(!zones.includes(zone)||!zonePanels(zone).length)return false;
    layout.collapsed[zone]=true;apply();save();return true;
  }
  function expand(zone) {
    if(!zones.includes(zone)||!zonePanels(zone).length)return false;
    layout.collapsed[zone]=false;apply();save();return true;
  }
  function toggle(zone) {
    if(!zones.includes(zone))return false;
    return layout.collapsed[zone]?expand(zone):collapse(zone);
  }
  function setAvailable(id,value) {
    if(!Object.hasOwn(names,id)||available[id]===Boolean(value))return;
    available[id]=Boolean(value);apply();
  }
  function sync() {
    const work=document.querySelector('#content .chat-work');
    const body=document.getElementById('work-panel-body');
    body.replaceChildren();
    available.work=Boolean(work);
    if(work)body.append(work);
    apply();
  }
  function closeMenu(restore=false) {
    const opener=menuOpener;menu.hidden=true;menu.replaceChildren();menuOpener=null;
    if(!dragging?.active||dragging.resize)document.body.classList.remove('panel-placement-open');
    if(restore&&opener?.isConnected&&!opener.closest('[hidden]'))opener.focus({preventScroll:true});
  }
  function openMenu(id,opener) {
    closeMenu();menuOpener=opener;
    for(const zone of zones) {
      const button=document.createElement('button');button.type='button';button.textContent=t({left:'Mover para a esquerda',right:'Mover para a direita'}[zone]);
      button.dataset.movePanel=id;button.dataset.moveZone=zone;button.disabled=layout.positions[id]===zone;menu.append(button);
    }
    menu.hidden=false;
    document.body.classList.add('panel-placement-open');
    menu.querySelector('button:not(:disabled)')?.focus();
  }
  function dropZone(x,y) {
    return [...targets.children].find(element=>{const r=element.getBoundingClientRect();return x>=r.left&&x<=r.right&&y>=r.top&&y<=r.bottom;})?.dataset.dropZone;
  }
  function endDrag() {
    targets.hidden=true;targets.querySelectorAll('.drop-active').forEach(element=>element.classList.remove('drop-active'));
    document.body.classList.remove('dragging-panel');dragging=null;
    if(menu.hidden)document.body.classList.remove('panel-placement-open');
  }
  document.addEventListener('pointerdown',event=>{
    const handle=event.target.closest('.dock-panel-handle,[data-dock-tab]');
    const resize=event.target.closest('[data-resize-dock]');
    if(event.button!==0||(!handle&&!resize))return;
    event.preventDefault();
    const id=handle?.dataset.dockTab||handle?.closest('[data-panel]')?.dataset.panel;
    const zone=resize?.dataset.resizeDock;
    dragging={id,resize:zone,x:event.clientX,y:event.clientY,startSize:resize?(layout.collapsed[zone]?0:layout.sizes[zone]):0,expandedSize:resize?layout.sizes[zone]:0,startCollapsed:resize?layout.collapsed[zone]:false,handle:handle||resize,pointer:event.pointerId,active:false};
    dragging.handle.setPointerCapture?.(event.pointerId);
  });
  document.addEventListener('pointermove',event=>{
    if(!dragging||event.pointerId!==dragging.pointer)return;
    const dx=event.clientX-dragging.x,dy=event.clientY-dragging.y;
    if(!dragging.active&&Math.hypot(dx,dy)>6){dragging.active=true;closeMenu();document.body.classList.add('dragging-panel');if(!dragging.resize){targets.hidden=false;document.body.classList.add('panel-placement-open');}}
    if(!dragging.active)return;
    event.preventDefault();
    if(dragging.resize) {
      const zone=dragging.resize;
      const size=Math.round(Math.max(0,Math.min(440,dragging.startSize+(zone==='left'?dx:-dx))));
      dragging.currentSize=size;layout.collapsed[zone]=size<=80;
      applySize(zone,size);
    } else {
      const zone=dropZone(event.clientX,event.clientY);
      [...targets.children].forEach(element=>element.classList.toggle('drop-active',element.dataset.dropZone===zone));
    }
  },{passive:false});
  document.addEventListener('pointerup',event=>{
    if(!dragging||event.pointerId!==dragging.pointer)return;
    const current=dragging;
    if(current.active&&current.resize){if(!layout.collapsed[current.resize])layout.sizes[current.resize]=boundedSize(current.currentSize);save();apply();}
    else if(current.active){const zone=dropZone(event.clientX,event.clientY);if(zone)move(current.id,zone);}
    else if(current.handle.matches('[data-dock-tab]'))activate(current.id,true);
    else if(!current.resize)openMenu(current.id,current.handle);
    endDrag();
  });
  function cancelDrag() {
    if(dragging?.resize){layout.sizes[dragging.resize]=dragging.expandedSize;layout.collapsed[dragging.resize]=dragging.startCollapsed;apply();}
    endDrag();
  }
  document.addEventListener('pointercancel',cancelDrag);
  document.addEventListener('dragstart',event=>{if(event.target.closest('.dock-panel-handle'))event.preventDefault();});
  document.addEventListener('click',event=>{
    const choice=event.target.closest('[data-move-panel]');
    if(choice){const id=choice.dataset.movePanel;move(id,choice.dataset.moveZone);closeMenu();panel(id).querySelector('.dock-panel-handle').focus();}
    else if(!menu.hidden&&!event.target.closest('.dock-position-menu,.dock-panel-handle'))closeMenu();
  });
  document.addEventListener('keydown',event=>{
    const handle=event.target.closest('.dock-panel-handle,[data-dock-tab]');
    const resize=event.target.closest('[data-resize-dock]');
    if(event.key==='Escape'){if(dragging)cancelDrag();if(!menu.hidden){event.preventDefault();closeMenu(true);}return;}
    if(resize) {
      const zone=resize.dataset.resizeDock;
      if(event.key==='Home'||event.key==='End'){event.preventDefault();(event.key==='Home'?collapse:expand)(zone);resize.focus({preventScroll:true});return;}
      const change={ArrowLeft:zone==='right'?20:-20,ArrowRight:zone==='right'?-20:20,ArrowUp:20,ArrowDown:-20}[event.key];
      if(change!==undefined){event.preventDefault();if(layout.collapsed[zone]){if(change>0)layout.collapsed[zone]=false;}else layout.sizes[zone]=boundedSize(layout.sizes[zone]+change);apply();save();resize.focus({preventScroll:true});}return;
    }
    if(!handle)return;
    const id=handle.dataset.dockTab||handle.closest('[data-panel]').dataset.panel;
    const zone={ArrowLeft:'left',ArrowRight:'right'}[event.key];
    if(zone&&(event.shiftKey||!handle.dataset.dockTab)){event.preventDefault();move(id,zone);panel(id).querySelector('.dock-panel-handle').focus();return;}
    if(event.key==='Enter'||event.key===' '){event.preventDefault();if(handle.dataset.dockTab)activate(id,true);else openMenu(id,handle);return;}
    if(handle.dataset.dockTab&&['ArrowLeft','ArrowRight','Home','End'].includes(event.key)) {
      const tabs=[...handle.parentElement.querySelectorAll('[data-dock-tab]')],index=tabs.indexOf(handle);
      const next=event.key==='Home'?0:event.key==='End'?tabs.length-1:(index+(event.key==='ArrowRight'?1:-1)+tabs.length)%tabs.length;
      event.preventDefault();activate(tabs[next].dataset.dockTab,true);
    }
  });
  document.addEventListener('orchestrix:language-change',()=>{closeMenu();apply();});
  addEventListener('blur',()=>{if(dragging)cancelDrag();});
  window.OrchestrixDock=Object.freeze({move,activate,sync,collapse,expand,toggle,setAvailable,hasPanels:zone=>zones.includes(zone)&&zonePanels(zone).length>0,isCollapsed:zone=>zones.includes(zone)&&layout.collapsed[zone],getLayout:()=>structuredClone(layout),restore:()=>{layout=initial();apply();save();}});
  apply();
})();
