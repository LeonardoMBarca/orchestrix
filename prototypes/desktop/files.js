/* Lightweight workspace files. File contents stay in memory and never execute. */
(() => {
  'use strict';
  const MAX_FILES=1200, MAX_BYTES=2*1024*1024, MAX_DEPTH=16;
  const state={entries:[],opened:new Map(),active:null,folder:'',native:false,search:'',expanded:new Set(),visible:false,busy:false,preview:false,generation:0};
  let options={},initialized=false;
  const t=value=>window.OrchestrixI18n?.t(value)||value;
  const esc=value=>String(value).replace(/[&<>"']/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const icon=name=>`<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${({folder:'<path d="M3 7V5a1 1 0 0 1 1-1h6l2 3h8a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1Z"/>',file:'<path d="M14 3H5v18h14V8Z"/><path d="M14 3v5h5"/>',chevron:'<path d="m9 5 7 7-7 7"/>',markdown:'<rect x="2" y="5" width="20" height="14" rx="2"/><path d="M5 15V9l3 3 3-3v6m5-6v6m-2-2 2 2 2-2"/>',search:'<circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/>',plus:'<path d="M12 5v14M5 12h14"/>'})[name]||''}</svg>`;
  const notify=message=>options.notify?.(t(message));
  const elements=()=>({toolbar:document.querySelector('#files-toolbar'),tree:document.querySelector('#files-tree'),editor:document.querySelector('#file-editor'),tabs:document.querySelector('#editor-tabs')});
  const activeDocument=()=>state.opened.get(state.active);
  const isDirty=doc=>doc.text!==doc.saved;
  const isMarkdown=path=>/\.(md|markdown|mdown)$/i.test(path);
  function changed(){const doc=activeDocument();options.onEditorChange?.({visible:state.visible,filename:doc?.path||'',dirty:doc?isDirty(doc):false});}
  function sortEntries(){state.entries.sort((a,b)=>(a.kind===b.kind?0:a.kind==='directory'?-1:1)||a.path.localeCompare(b.path,undefined,{numeric:true,sensitivity:'base'}));}
  function renderToolbar(){
    const {toolbar}=elements();if(!toolbar)return;
    toolbar.innerHTML=`<div class="files-folder-heading"><strong title="${esc(state.folder)}">${esc(state.folder||t('Nenhuma pasta aberta'))}</strong><button class="files-icon-button" type="button" data-files-open title="${esc(t('Abrir pasta'))}" aria-label="${esc(t('Abrir pasta'))}">${icon('plus')}</button></div><label class="files-search">${icon('search')}<input type="search" aria-label="${esc(t('Buscar arquivos'))}" placeholder="${esc(t('Buscar arquivos'))}" title="${esc(t('A busca inclui as pastas já carregadas.'))}" value="${esc(state.search)}"></label>${state.folder&&!state.native?`<p class="files-import-note">${esc(t('Arquivos importados. As alterações são salvas como cópias.'))}</p>`:''}`;
  }
  function row(entry,depth){
    const folder=entry.kind==='directory',open=state.expanded.has(entry.path),selected=state.active===entry.path;
    return `<button type="button" class="file-tree-row ${folder?'file-tree-folder':''} ${selected?'is-active':''}" data-file-path="${esc(entry.path)}" data-file-kind="${entry.kind}" style="--file-depth:${depth}" title="${esc(entry.path)}" ${folder?`aria-expanded="${open}"`:`aria-current="${selected?'true':'false'}"`}><span class="file-tree-chevron ${open?'is-open':''}" aria-hidden="true">${folder?icon('chevron'):''}</span><span class="file-tree-icon ${isMarkdown(entry.path)?'is-markdown':''}" aria-hidden="true">${icon(folder?'folder':isMarkdown(entry.path)?'markdown':'file')}</span><span class="file-tree-name">${esc(entry.name)}</span></button>`;
  }
  function renderTree(){
    const {tree}=elements();if(!tree)return;
    if(state.busy){tree.innerHTML=`<p class="files-empty" role="status">${esc(t('Lendo pasta…'))}</p>`;return;}
    if(!state.entries.length){tree.innerHTML=`<div class="files-empty"><span class="files-empty-icon" aria-hidden="true">${icon('folder')}</span><h3>${esc(t('Seus arquivos, por perto'))}</h3><p>${esc(t('Abra uma pasta para consultar documentos e fazer pequenas edições sem sair da conversa.'))}</p><button type="button" class="button" data-files-open>${esc(t('Abrir pasta'))}</button></div>`;return;}
    const query=state.search.trim().toLowerCase();let markup='';
    if(query){markup=state.entries.filter(entry=>entry.kind==='file'&&entry.path.toLowerCase().includes(query)).map(entry=>row({...entry,name:entry.path},0)).join('');}
    else {
      const children=new Map();for(const entry of state.entries){const parent=entry.path.includes('/')?entry.path.slice(0,entry.path.lastIndexOf('/')):'';if(!children.has(parent))children.set(parent,[]);children.get(parent).push(entry);}
      function branch(parent,depth){return (children.get(parent)||[]).map(entry=>row(entry,depth)+(entry.kind==='directory'&&state.expanded.has(entry.path)?branch(entry.path,depth+1):'')).join('');}
      markup=branch('',0);
    }
    tree.innerHTML=markup||`<p class="files-empty">${esc(t('Nenhum arquivo encontrado.'))}</p>`;
  }
  function renderTabs(){
    const {tabs}=elements();if(!tabs)return;
    tabs.hidden=!state.opened.size;
    tabs.innerHTML=`<button type="button" class="editor-tab ${!state.visible?'is-active':''}" data-file-chat>${esc(t('Conversa'))}</button>`+[...state.opened.values()].map(doc=>`<div class="editor-file-tab ${state.visible&&state.active===doc.path?'is-active':''}"><button type="button" class="editor-tab" data-file-tab="${esc(doc.path)}" title="${esc(doc.path)}">${esc(doc.name)}<span class="file-dirty" ${isDirty(doc)?'':'hidden'} aria-label="${esc(t('Alterações não salvas'))}">●</span></button><button type="button" class="editor-tab-close" data-file-close="${esc(doc.path)}" aria-label="${esc(t('Fechar arquivo'))}: ${esc(doc.name)}">×</button></div>`).join('');
  }
  function inlineMarkdown(source){
    // Escape first; deliberately no raw HTML, images, or executable links.
    return esc(source).replace(/`([^`]+)`/g,'<code>$1</code>').replace(/\*\*([^*]+)\*\*/g,'<strong>$1</strong>').replace(/\*([^*]+)\*/g,'<em>$1</em>');
  }
  function markdown(source){
    let fenced=false,code=[],out=[];
    for(const line of source.split(/\r?\n/)){
      if(/^\s*```/.test(line)){if(fenced){out.push(`<pre><code>${esc(code.join('\n'))}</code></pre>`);code=[];}fenced=!fenced;continue;}
      if(fenced){code.push(line);continue;}
      const heading=/^(#{1,6})\s+(.+)$/.exec(line);
      if(heading){out.push(`<h${heading[1].length}>${inlineMarkdown(heading[2])}</h${heading[1].length}>`);continue;}
      if(/^\s*[-*+]\s+/.test(line)){out.push(`<p class="file-preview-list">• ${inlineMarkdown(line.replace(/^\s*[-*+]\s+/,''))}</p>`);continue;}
      if(/^>\s?/.test(line)){out.push(`<blockquote>${inlineMarkdown(line.replace(/^>\s?/,''))}</blockquote>`);continue;}
      if(line.trim())out.push(`<p>${inlineMarkdown(line)}</p>`);
    }
    if(fenced)out.push(`<pre><code>${esc(code.join('\n'))}</code></pre>`);
    return out.join('');
  }
  function renderEditor(){
    const {editor}=elements(),doc=activeDocument();if(!editor||!doc)return;
    editor.innerHTML=`<header class="file-editor-heading"><div><strong title="${esc(doc.path)}">${esc(doc.path)}</strong><span class="file-storage-label">${esc(doc.handle?t('Arquivo na pasta'):t('Cópia importada'))}</span></div><div class="file-editor-actions">${isMarkdown(doc.path)?`<div class="file-view-switch" role="group" aria-label="${esc(t('Visualização do arquivo'))}"><button type="button" data-file-view="edit" class="${!state.preview?'is-active':''}">${esc(t('Editar texto'))}</button><button type="button" data-file-view="preview" class="${state.preview?'is-active':''}">${esc(t('Prévia Markdown'))}</button></div>`:''}<button type="button" class="button file-save-button" data-file-save>${esc(doc.handle?t('Salvar arquivo'):t('Salvar cópia'))}</button></div></header><div class="file-edit-area" ${state.preview?'hidden':''}><div class="file-line-numbers" aria-hidden="true"></div><textarea class="file-text-editor" aria-label="${esc(t('Conteúdo do arquivo'))}" aria-describedby="file-editor-hint" spellcheck="false" autocapitalize="off" autocomplete="off" autocorrect="off" wrap="off"></textarea></div><article class="file-markdown-preview" aria-label="${esc(t('Prévia Markdown'))}" ${state.preview?'':'hidden'}></article><footer class="file-editor-status"><span class="file-cursor-position"></span><span>${esc(isMarkdown(doc.path)?'Markdown':t('Texto simples'))} · UTF-8</span><span class="file-save-status" role="status"></span></footer><p id="file-editor-hint" class="sr-only">${esc(t('Tab insere espaços. Escape permite sair do editor com Tab. Ctrl+M alterna este comportamento.'))}</p>`;
    const input=editor.querySelector('textarea');input.value=doc.text;
    input.selectionStart=doc.selectionStart||0;input.selectionEnd=doc.selectionEnd||0;input.scrollTop=doc.scrollTop||0;
    input.addEventListener('input',()=>{doc.text=input.value;rememberCursor(input,doc);updateEditorStatus();renderTabs();changed();});
    input.addEventListener('select',()=>rememberCursor(input,doc));
    input.addEventListener('keyup',()=>{rememberCursor(input,doc);updateEditorStatus();});
    input.addEventListener('click',()=>{rememberCursor(input,doc);updateEditorStatus();});
    input.addEventListener('scroll',()=>{doc.scrollTop=input.scrollTop;const gutter=editor.querySelector('.file-line-numbers');gutter.scrollTop=input.scrollTop;});
    input.addEventListener('keydown',event=>{
      if(event.key==='Escape'){doc.tabNavigation=true;notify('Tab agora move o foco para fora do editor.');}
      if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='m'){event.preventDefault();doc.tabNavigation=!doc.tabNavigation;notify(doc.tabNavigation?'Tab agora move o foco para fora do editor.':'Tab agora insere espaços no editor.');}
      if(event.key==='Tab'&&!doc.tabNavigation){event.preventDefault();input.setRangeText('  ',input.selectionStart,input.selectionEnd,'end');input.dispatchEvent(new Event('input',{bubbles:true}));}
    });
    editor.querySelector('.file-markdown-preview').innerHTML=markdown(doc.text);
    updateEditorStatus();
  }
  function rememberCursor(input,doc){doc.selectionStart=input.selectionStart;doc.selectionEnd=input.selectionEnd;doc.scrollTop=input.scrollTop;}
  function updateEditorStatus(){
    const {editor}=elements(),doc=activeDocument();if(!editor||!doc)return;
    const input=editor.querySelector('textarea'),position=doc.text.slice(0,input?.selectionStart||0).split('\n');
    editor.querySelector('.file-cursor-position').textContent=`${t('Linha')} ${position.length}, ${t('Coluna')} ${position.at(-1).length+1}`;
    editor.querySelector('.file-save-status').textContent=isDirty(doc)?t('Alterações não salvas'):t('Sem alterações pendentes');
    const lineCount=doc.text.split('\n').length;
    editor.querySelector('.file-line-numbers').textContent=Array.from({length:Math.min(lineCount,10000)},(_,i)=>i+1).join('\n')+(lineCount>10000?'\n…':'');
  }
  async function openFile(path){
    const generation=state.generation;
    try{
      if(!state.opened.has(path)){
        const entry=state.entries.find(item=>item.path===path&&item.kind==='file');if(!entry)return;
        const file=entry.handle?await entry.handle.getFile():entry.file;
        if(file.size>MAX_BYTES){notify('Este arquivo excede o limite de edição de 2 MB.');return;}
        const bytes=new Uint8Array(await file.arrayBuffer());
        if(bytes.subarray(0,8192).includes(0)){notify('Este tipo de arquivo não pode ser editado como texto.');return;}
        let text;try{text=new TextDecoder('utf-8',{fatal:true}).decode(bytes);}catch{notify('Este arquivo não contém texto UTF-8 compatível.');return;}
        if(generation!==state.generation)return;
        state.opened.set(path,{path,name:entry.name,text,saved:text,handle:entry.handle||null,selectionStart:0,selectionEnd:0});
      }
      state.active=path;state.visible=true;state.preview=false;renderTabs();renderTree();changed();renderEditor();
    }catch{notify('Não foi possível abrir o arquivo. Verifique a permissão da pasta.');}
  }
  function mayReplace(){return ![...state.opened.values()].some(isDirty)||window.confirm(t('Há alterações não salvas. Deseja fechar estes arquivos e abrir outra pasta?'));}
  function replaceEntries(entries,folder,native){
    state.generation++;state.entries=entries;state.folder=folder;state.native=native;state.search='';state.expanded=new Set();state.opened.clear();state.active=null;state.visible=false;state.preview=false;sortEntries();renderToolbar();renderTree();renderTabs();changed();
  }
  async function expandFolder(path){
    if(state.expanded.has(path)){state.expanded.delete(path);renderTree();return;}
    const entry=state.entries.find(item=>item.path===path);if(!entry)return;
    state.expanded.add(path);renderTree();
    if(entry.handle&&!entry.loaded&&!entry.loading){
      const generation=state.generation;
      if(path.split('/').length>=MAX_DEPTH){notify('A pasta foi limitada aos primeiros 1.200 itens e 16 níveis.');return;}
      entry.loading=true;
      try{
        for await(const handle of entry.handle.values()){
          if(generation!==state.generation)return;
          if(state.entries.length>=MAX_FILES){notify('A pasta foi limitada aos primeiros 1.200 itens e 16 níveis.');break;}
          const childPath=`${path}/${handle.name}`;
          if(!state.entries.some(item=>item.path===childPath))state.entries.push({path:childPath,name:handle.name,kind:handle.kind,handle});
        }
        entry.loaded=true;sortEntries();
      }catch{notify('Não foi possível abrir a pasta. Verifique a permissão e tente novamente.');}
      finally{entry.loading=false;renderTree();}
    }
  }
  async function openFolder(){
    if(state.busy)return;
    if(typeof window.showDirectoryPicker!=='function'){
      const input=document.createElement('input');input.type='file';input.multiple=true;input.setAttribute('webkitdirectory','');input.hidden=true;document.body.append(input);
      input.addEventListener('change',()=>{importFiles(input.files);input.remove();},{once:true});
      input.addEventListener('cancel',()=>input.remove(),{once:true});input.click();return;
    }
    try{
      const root=await window.showDirectoryPicker({mode:'read'});if(!mayReplace())return;
      state.busy=true;renderTree();const entries=[];let limited=false;
      async function visit(directory){
        for await(const handle of directory.values()){
          if(entries.length>=MAX_FILES){limited=true;return;}
          entries.push({path:handle.name,name:handle.name,kind:handle.kind,handle});
        }
      }
      await visit(root);state.busy=false;replaceEntries(entries,root.name,true);if(limited)notify('A pasta foi limitada aos primeiros 1.200 itens e 16 níveis.');
    }catch(error){state.busy=false;renderTree();if(error.name!=='AbortError')notify('Não foi possível abrir a pasta. Verifique a permissão e tente novamente.');}
  }
  function importFiles(files){
    if(!files?.length||!mayReplace())return false;
    const entries=[],seen=new Set(),list=Array.from(files).slice(0,MAX_FILES);let folder='',limited=files.length>MAX_FILES;
    for(const file of list){
      const raw=file.webkitRelativePath||file.name,parts=raw.split('/').filter(part=>part&&part!=='.'&&part!=='..');
      if(parts.length>1){folder ||=parts.shift();}
      if(parts.length>MAX_DEPTH){limited=true;continue;}
      const path=parts.join('/');if(!path||seen.has(path))continue;
      const absentDirectories=parts.slice(0,-1).map((_,index)=>parts.slice(0,index+1).join('/')).filter(directory=>!seen.has(directory));
      if(entries.length+absentDirectories.length+1>MAX_FILES){limited=true;break;}
      for(let i=1;i<parts.length;i++){const directory=parts.slice(0,i).join('/');if(!seen.has(directory)){seen.add(directory);entries.push({path:directory,name:parts[i-1],kind:'directory'});}}
      seen.add(path);entries.push({path,name:parts.at(-1),kind:'file',file});
    }
    replaceEntries(entries,folder||t('Arquivos importados'),false);if(limited)notify('A pasta foi limitada aos primeiros 1.200 itens e 16 níveis.');return true;
  }
  async function save(){
    const doc=activeDocument();if(!doc||doc.saving)return false;
    doc.saving=true;const text=doc.text;
    try{
      if(doc.handle){
        const permission=typeof doc.handle.queryPermission==='function'?await doc.handle.queryPermission({mode:'readwrite'}):'granted';
        if(permission!=='granted'&&(!doc.handle.requestPermission||await doc.handle.requestPermission({mode:'readwrite'})!=='granted')){notify('A permissão de escrita não foi concedida.');return false;}
        // Avoid silently replacing a file changed outside Orchestrix after it was opened.
        const current=await doc.handle.getFile();if(await current.text()!==doc.saved&&!window.confirm(t('O arquivo foi alterado fora do Orchestrix. Deseja substituir a versão no disco?')))return false;
        const writer=await doc.handle.createWritable();try{await writer.write(text);await writer.close();}catch(error){try{await writer.abort?.();}catch{}throw error;}
      }else{
        const url=URL.createObjectURL(new Blob([text],{type:'text/plain;charset=utf-8'})),link=document.createElement('a');link.href=url;link.download=doc.name;document.body.append(link);link.click();link.remove();setTimeout(()=>URL.revokeObjectURL(url),1000);
      }
      doc.saved=text;renderTabs();updateEditorStatus();changed();notify(doc.handle?'Arquivo salvo.':'Cópia do arquivo preparada para download.');return true;
    }catch{notify('Não foi possível salvar o arquivo. Seu texto continua disponível no editor.');return false;}
    finally{doc.saving=false;}
  }
  function close(path=state.active){
    const doc=state.opened.get(path);if(!doc)return false;
    if(isDirty(doc)&&!window.confirm(t('Descartar as alterações não salvas deste arquivo?')))return false;
    state.opened.delete(path);if(state.active===path){state.active=[...state.opened.keys()].at(-1)||null;state.visible=!!state.active;state.preview=false;}
    renderTabs();renderTree();if(state.active)renderEditor();changed();return true;
  }
  function showChat(){state.visible=false;renderTabs();changed();document.dispatchEvent(new CustomEvent('orchestrix:files-chat'));}
  function showEditor(){if(!state.active)return false;state.visible=true;renderTabs();changed();return true;}
  function init(config={}){
    options=config;if(initialized){renderToolbar();renderTree();renderTabs();return;}initialized=true;
    renderToolbar();renderTree();renderTabs();
    document.addEventListener('click',event=>{
      if(event.target.closest('[data-files-open]')){openFolder();return;}
      const file=event.target.closest('[data-file-path]');if(file){const path=file.dataset.filePath;if(file.dataset.fileKind==='directory')expandFolder(path);else openFile(path);return;}
      const tab=event.target.closest('[data-file-tab]');if(tab){openFile(tab.dataset.fileTab);return;}
      if(event.target.closest('[data-file-chat]')){showChat();return;}
      const closeButton=event.target.closest('[data-file-close]');if(closeButton){close(closeButton.dataset.fileClose);return;}
      if(event.target.closest('[data-file-save]')){save();return;}
      const view=event.target.closest('[data-file-view]');if(view){state.preview=view.dataset.fileView==='preview';renderEditor();}
    });
    document.addEventListener('input',event=>{if(event.target.matches('.files-search input')){state.search=event.target.value;renderTree();}});
    document.addEventListener('keydown',event=>{if(state.visible&&(event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='s'){event.preventDefault();save();}});
    window.addEventListener('beforeunload',event=>{if([...state.opened.values()].some(isDirty)){event.preventDefault();event.returnValue='';}});
    document.addEventListener('orchestrix:language-change',()=>{renderToolbar();renderTree();renderTabs();if(state.active)renderEditor();});
  }
  window.OrchestrixFiles=Object.freeze({init,openFolder,importFiles,openFile,showEditor,showChat,save,closeActive:()=>close(),setSearch(query){state.search=String(query);renderToolbar();renderTree();},getState:()=>({folder:state.folder,native:state.native,active:state.active,visible:state.visible,entries:state.entries.map(({path,name,kind})=>({path,name,kind})),opened:[...state.opened.values()].map(doc=>({path:doc.path,dirty:isDirty(doc)}))})});
})();
