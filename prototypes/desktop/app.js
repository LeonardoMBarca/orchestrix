/* Product exploration only: every task, account, artifact and transition is simulated. */
(() => {
  'use strict';
  const {html,text,t}=window.OrchestrixI18n;
  const paths = {
    layers:'M12 3 3 8l9 5 9-5-9-5ZM3 12l9 5 9-5M3 16l9 5 9-5',
    inbox:'M4 4h16l2 11v5H2v-5L4 4ZM2 15h6l2 3h4l2-3h6',
    diff:'M9 3H4v18h5M15 3h5v18h-5M8 8h8M12 4v8M8 17h8',
    clock:'M12 8v5l3 2M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0',
    plug:'m8 3 1 4m7-4-1 4M6 7h12v4a6 6 0 0 1-12 0V7ZM12 17v4',
    sliders:'M4 5h16M4 12h16M4 19h16M8 3v4M16 10v4M10 17v4',
    folder:'M3 5h6l2 3h10v12H3V5Z', search:'m21 21-5-5M18 10a8 8 0 1 1-16 0 8 8 0 0 1 16 0',
    chevrons:'m9 7 3-3 3 3m-6 10 3 3 3-3', focus:'M8 3H3v5m13-5h5v5M3 16v5h5m13-5v5h-5',
    plus:'M12 5v14M5 12h14', arrow:'M4 12h16m-6-6 6 6-6 6',
    check:'m5 12 4 4L19 6', circle:'M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0',
    checkCircle:'m8 12 3 3 5-6M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0',
    alert:'M12 3 2 21h20L12 3ZM12 9v5m0 3v.1', pause:'M8 5v14M16 5v14',
    play:'m8 4 12 8-12 8V4Z', stop:'M5 5h14v14H5V5Z', reset:'M3 10a9 9 0 1 1 1 8M3 4v6h6',
    close:'m6 6 12 12M18 6 6 18', code:'m8 7-5 5 5 5m8-10 5 5-5 5m-3-13-2 16',
    file:'M14 2H5v20h14V7l-5-5ZM14 2v5h5', shield:'M12 2 3 6v6c0 5 9 10 9 10s9-5 9-10V6l-9-4Zm-4 10 3 3 5-6',
    branch:'M6 6v12m0-10c10 0 12 0 12 10M9 3a3 3 0 1 1-6 0 3 3 0 0 1 6 0m0 18a3 3 0 1 1-6 0 3 3 0 0 1 6 0m15 0a3 3 0 1 1-6 0 3 3 0 0 1 6 0',
    message:'M3 3h18v14H8l-5 4V3Z', book:'M3 3h7l2 2 2-2h7v17h-7l-2 2-2-2H3V3ZM12 5v17'
  };
  const icon = name => html`<svg viewBox="0 0 24 24" aria-hidden="true"><path d="${paths[name] || paths.circle}"/></svg>`;
  const esc = value => String(value).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const statuses = {permission:'Permissão necessária',review:'Aguardando sua revisão',running:'Em execução',blocked:'Decisão necessária',paused:'Pausada',unknown:'Execução sem confirmação',failed:'Tentativa falhou',ready:'Pronta para iniciar',integrated:'Validada no Run',cancelled:'Cancelada',conflict:'Base alterada'};
  const statusIcons = {permission:'shield',review:'diff',running:'circle',blocked:'alert',paused:'pause',unknown:'alert',failed:'alert',ready:'circle',integrated:'checkCircle',cancelled:'stop',conflict:'branch'};
  const W=window.OrchestrixWorkspace;
  const viewNames = {chat:'Conversa',entry:'Começar',work:'Trabalho',attention:'Precisa de você',review:'Alterações',history:'Histórico',connections:'Conexões',settings:'Preferências'};
  const themes = [
    {id:'studio',name:'Studio',description:'Azul profundo e índigo. A identidade do site no seu workspace. Tema padrão.',colors:['#0B1426','#0C172A','#101D34','#A5ACFF','#AAB8D1','#2A3B5A']},
    {id:'institutional',name:'Institutional',description:'Branco técnico e índigo. Identidade Orchestrix em fundo claro.',colors:['#FAFAFC','#FAFAFC','#FFFFFF','#5154CB','#596273','#E5E7EB']},
    {id:'atelier',name:'Atelier',description:'Papel, terracota e títulos editoriais.',colors:['#f7f6f2','#eeece5','#fffefa','#8b422e','#61635e','#d7d7cc']},
    {id:'horizon',name:'Horizon',description:'Azul profundo e espaço para respirar.',colors:['#0d1726','#09111d','#17283e','#9ccfff','#a4b7cc','#29405b']},
    {id:'deep-black',name:'Deep Black',description:'Preto profundo e acentos neutros.',colors:['#050505','#000000','#191919','#e9e9ee','#afafb5','#303033']},
    {id:'medieval',name:'Medieval',description:'Madeira escura, ouro e tipografia clássica.',colors:['#211b16','#17120d','#362b20','#ddbb72','#c2ae8b','#5c4932']},
    {id:'forest',name:'Forest',description:'Verde profundo e tons de menta.',colors:['#13211e','#0c1715','#22382f','#b5dba4','#a9c0af','#375247']},
    {id:'dawn',name:'Dawn',description:'Luz quente, coral e superfícies suaves.',colors:['#faf7f3','#f2eae1','#fffdf9','#9d4035','#6c615b','#d8c6b9']},
  ];
  function setTheme(id,persist=true) {
    if(!themes.some(theme=>theme.id===id))return;
    document.documentElement.dataset.direction=id;
    document.querySelectorAll('input[name="workspace-theme"]').forEach(input=>{input.checked=input.value===id;});
    if(persist)try{localStorage.setItem('orchestrix-prototype-theme',id);}catch{}
  }
  function appearanceCard() {
    const current=document.documentElement.dataset.direction;
    return html`<section class="settings-card appearance-card" id="appearance" aria-labelledby="appearance-title"><h2 id="appearance-title">Aparência</h2><p>Escolha o ambiente em que você se sente mais confortável. A mudança é imediata e fica salva neste navegador. Studio é o padrão.</p><fieldset class="theme-grid"><legend class="sr-only">Tema do workspace</legend>${themes.map(theme=>html`<label class="theme-card"><input type="radio" name="workspace-theme" value="${theme.id}" ${current===theme.id?'checked':''} aria-label="${theme.name}"><span class="theme-preview" aria-hidden="true" style="${['bg','side','panel','accent','muted','line'].map((key,index)=>`--sample-${key}:${theme.colors[index]}`).join(';')}"><span class="theme-preview-nav"><i></i><i></i><i></i><i></i></span><span class="theme-preview-work"><i></i><i></i><b><span></span><em></em></b><i></i></span></span><strong>${theme.name}${theme.id==='studio'?html`<span class="theme-default">Padrão</span>`:''}</strong><small>${t(theme.description)}</small></label>`).join('')}</fieldset><div class="field-grid"><div><label class="form-label" for="settings-language">Idioma da interface</label><select id="settings-language" data-language-picker>${window.OrchestrixI18n.languages.map(language=>html`<option value="${language.id}" lang="${language.id}" ${window.OrchestrixI18n.language===language.id?'selected':''}>${language.name}</option>`).join('')}</select><p class="field-hint">Idioma salvo neste navegador. Não altera o conteúdo das conversas.</p></div><div><label class="form-label" for="density">Densidade da interface</label><select id="density"><option value="comfortable" ${ui.density==='comfortable'?'selected':''}>Confortável</option><option value="compact" ${ui.density==='compact'?'selected':''}>Compacta</option></select></div><div><label class="form-label" for="text-size">Tamanho do texto</label><select id="text-size"><option value="normal" ${ui.textSize==='normal'?'selected':''}>Normal</option><option value="large" ${ui.textSize==='large'?'selected':''}>Ampliado · 200%</option></select></div></div><p class="field-hint">Tema, densidade, texto e largura dos painéis ficam salvos neste navegador.</p><div class="form-actions">${action(t('Restaurar Studio'),'default-theme')}${action(t('Restaurar layout'),'reset-layout')}</div></section>`;
  }
  const initialTasks = () => [
    {id:'OX-24',title:t('Rotação de refresh tokens'),criteria:[t('Token utilizado deixa de ser válido após a rotação.'),t('Reutilização revoga a família e registra a ocorrência.')],status:'review',role:'Implementação + revisão',account:'Codex · pessoal A',summary:t('Adicionar rotação de refresh tokens e invalidar a família de tokens quando um token já utilizado reaparecer.'),attempt:1,dependency:t('Plano aprovado · etapa 1 de 3')},
    {id:'OX-25',title:t('Revogar sessões ativas'),status:'running',role:'Implementação',account:'Claude · pessoal',summary:t('Permitir encerrar sessões de um usuário e preservar o registro de auditoria.'),attempt:1,dependency:t('Independente de OX-24')},
    {id:'OX-26',title:t('Validar redirecionamento de login'),status:'blocked',role:'Análise',account:'Codex · pessoal B',summary:t('Restringir os destinos de redirecionamento após o login.'),attempt:1,dependency:t('Decisão do responsável pelo projeto')},
    {id:'OX-27',title:t('Documentar a política de sessão'),status:'failed',role:'Documentação',account:'Codex · pessoal A',summary:t('Descrever expiração, revogação e comportamento da rotação de tokens.'),attempt:1,dependency:t('Usar resultados de OX-24 e OX-25')}
  ];
  let state;
  const projects=new Map();
  let guidedPrepared=false;
  let initialConnectionFingerprint='';
  const ui=W.loadUI();W.applyUI(ui);
  const content = document.querySelector('#content');
  const modal = document.querySelector('#modal');
  const mobileLayout=window.matchMedia('(max-width:640px)');
  mobileLayout.addEventListener('change',()=>window.OrchestrixExperience.syncLayout(mobileLayout.matches));
  let toastTimer;
  let opener;
  function resetScenario() {
    projects.clear();guidedPrepared=false;
    state = {view:'chat',tab:'summary',selected:'OX-24',tasks:initialTasks(),project:'orchestrix',projectId:crypto.randomUUID(),chat:W.createProjectChat(),file:'service',revision:1,base:'b9f87d3',approvedRevision:null,events:[
      {text:t('OX-24 · revisão independente concluída'),detail:t('Resultado simulado · tentativa 1 · artefato r1'),icon:'shield'},
      {text:t('OX-26 · solicitação de decisão'),detail:t('Os destinos permitidos ainda não foram definidos.'),icon:'alert'},
      {text:t('OX-25 · implementação iniciada'),detail:t('Claude · conexão de exemplo · worktree separado'),icon:'play'},
      {text:t('Plano de autenticação aprovado'),detail:t('3 etapas · 4 tarefas · dados de demonstração'),icon:'checkCircle'}
    ],connections:[{name:'Codex · pessoal A',provider:'Codex',letter:'C',status:t('Exemplo configurado')},{name:'Codex · pessoal B',provider:'Codex',letter:'C',status:t('Identidade não verificada')},{name:'Claude · pessoal',provider:'Claude Code',letter:'✳',status:t('Exemplo configurado')}],preferences:{routing:'balanced',model:'runtime',thinking:'high',context:'focused',account:'auto',review:'fresh'},comment:'',focus:false};
    state.connections=window.OrchestrixConnections?.initial()||state.connections;
    initialConnectionFingerprint=JSON.stringify(state.connections);
    state.conversations=[state.chat];state.isLoose=false;state.projectOrdinal=1;
    state.chat.exampleVisible=false;
    document.querySelector('#studio-navigation').open=false;
    state.contextSelection=['service','test','policy'];
    state.ui=ui;state.mockScenario=true;state.projectPath='C:\\Projetos\\orchestrix';state.platform='native';state.objective=t('Um login mais seguro');state.aggregate=false;
    state.tasks.forEach((task,index)=>{task.run=`R-${String(8+index).padStart(2,'0')}`;task.dependencies=task.id==='OX-27'?['OX-24','OX-25']:[];task.connectionId=index===1?'claude':index===2?'codex-b':'codex-a';configureAttempt(task);});
    state.hasArtifact=true;state.runStatus='task-review';state.runRevision=1;state.runBase=state.base;state.correctionDraft='';state.checksFailed=false;state.artifactAttempt=1;
    document.querySelector('#app').classList.remove('focus-mode');
    document.querySelector('.focus-toggle').setAttribute('aria-pressed','false');
    render();
  }
  const sessionStorageKey='orchestrix-sessions-v1';
  const testScenario=window.__ORCHESTRIX_TEST_SCENARIO==='seed';
  let persistTimer;
  let persistenceWarning=false;
  const defaultPreferences=()=>({routing:'balanced',model:'runtime',thinking:'auto',context:'focused',account:'auto',review:'fresh'});
  function freshState({project='',path='',context='',loose=true}={}) {
    const chat=W.createProjectChat(t('Nova sessão'),false);
    return {view:'chat',tab:'summary',selected:null,tasks:[],project,projectId:crypto.randomUUID(),projectOrdinal:projects.size+1,projectPath:path,projectContext:context,isLoose:loose,chat,conversations:[chat],file:'service',revision:0,base:'unobserved',approvedRevision:null,events:[],connections:state?.connections||[],preferences:{...defaultPreferences(),...state?.preferences},appSettings:{systemPrompt:'',profileName:'',subscriptionOnly:true,...state?.appSettings},focus:false,contextSelection:[],ui,platform:'native',objective:'',aggregate:false,hasArtifact:false,runStatus:'task-review',runRevision:1,runBase:'unobserved',correctionDraft:'',checksFailed:false,artifactAttempt:1,comment:''};
  }
  function persistSessions() {
    if(!state||testScenario)return;
    rememberComposer();
    projects.set(state.projectId,structuredClone(state));
    const snapshots=[...projects.values()].map(project=>{
      const snapshot=structuredClone(project);delete snapshot.connections;delete snapshot.ui;delete snapshot.appSettings;
      snapshot.view='chat';snapshot.chat.reviewReturn=null;return snapshot;
    });
    try {
      const serialized=JSON.stringify({version:1,activeProjectId:state.projectId,activeSessionId:state.chat.id,projects:snapshots});
      if(serialized.length>2_000_000)throw new Error('Session storage limit');
      localStorage.setItem(sessionStorageKey,serialized);persistenceWarning=false;
    }catch{if(!persistenceWarning){persistenceWarning=true;notify(t('Não foi possível salvar as sessões neste navegador. Exporte conteúdo importante antes de fechar.'));}}
  }
  function schedulePersistence() {clearTimeout(persistTimer);persistTimer=setTimeout(persistSessions,180);}
  function loadSessions() {
    try {
      const raw=localStorage.getItem(sessionStorageKey);if(!raw||raw.length>2_000_000)return false;
      const saved=JSON.parse(raw);if(saved.version!==1||!Array.isArray(saved.projects)||saved.projects.length>100)return false;
      const sessionIds=new Set();
      for(const project of saved.projects){
        if(typeof project.projectId!=='string'||typeof project.project!=='string'||typeof project.projectPath!=='string'||!Array.isArray(project.conversations)||!project.conversations.length||project.conversations.length>100||!Array.isArray(project.tasks)||project.tasks.length>500||!Array.isArray(project.events))return false;
        for(const chat of project.conversations){if(typeof chat.id!=='string'||sessionIds.has(chat.id)||typeof chat.name!=='string'||typeof chat.draft!=='string'||chat.draft.length>2000||!Array.isArray(chat.messages)||!chat.messages.every(message=>typeof message?.text==='string'&&['user','orchestrator'].includes(message.role))||!Array.isArray(chat.taskIds)||!chat.taskIds.every(id=>typeof id==='string'))return false;sessionIds.add(chat.id);}
        if(!project.tasks.every(task=>typeof task?.id==='string'&&typeof task.title==='string'&&typeof task.status==='string'&&task.config&&task.contextPack&&Array.isArray(task.contextPack.sources)&&Array.isArray(task.contextPack.criteria)))return false;
        const chat=project.conversations.find(item=>item.id===project.chat?.id)||project.conversations[0];
        project.chat=chat;project.connections=[];project.ui=ui;project.appSettings={systemPrompt:'',profileName:'',subscriptionOnly:true};projects.set(project.projectId,project);
      }
      const restored=projects.get(saved.activeProjectId)||projects.values().next().value;if(!restored)return false;
      state=restored;state.chat=state.conversations.find(item=>item.id===saved.activeSessionId)||state.chat;state.view='chat';state.focus=false;return true;
    }catch{projects.clear();return false;}
  }
  function reset() {
    if(testScenario){resetScenario();return;}
    projects.clear();state=freshState();render();
  }
  function renderSessions() {
    const root=document.querySelector('#session-list');if(!root)return;
    const entries=[...projects.values()].filter(project=>project.projectId!==state.projectId).concat(state).sort((a,b)=>a.projectOrdinal-b.projectOrdinal);
    const sessionButton=(project,chat)=>html`<button type="button" class="session-item ${chat.id===state.chat.id?'active':''}" data-session-project="${esc(project.projectId)}" data-session-id="${esc(chat.id)}" ${chat.id===state.chat.id?'aria-current="page"':''}>${icon('message')}<span>${esc(sessionLabel(chat))}</span></button>`;
    root.innerHTML=html`<div class="session-toolbar"><button type="button" class="button primary new-session-trigger" data-action="chat-loose">${icon('plus')}Nova sessão</button><button type="button" class="icon-button" data-action="onboarding" aria-label="Novo projeto" title="Novo projeto">${icon('folder')}</button></div><section class="session-section"><h3>Projetos</h3>${entries.filter(project=>!project.isLoose).map(project=>html`<details class="session-project" data-project-id="${esc(project.projectId)}" ${project.projectId===state.projectId||project.expanded!==false?'open':''}><summary>${icon('folder')}<span>${esc(project.project)}</span><small>${project.conversations.length}</small></summary><div class="project-session-actions"><button type="button" class="context-action" data-new-session-project="${esc(project.projectId)}">${icon('plus')}Nova sessão</button></div>${project.conversations.map(chat=>sessionButton(project,chat)).join('')}</details>`).join('')||html`<p class="session-empty">Crie um projeto para reunir sessões.</p>`}</section><section class="session-section"><h3>Sessões recentes</h3>${entries.filter(project=>project.isLoose).reverse().map(project=>project.conversations.map(chat=>sessionButton(project,chat)).join('')).join('')||html`<p class="session-empty">Suas sessões independentes aparecem aqui.</p>`}</section>`;
    root.querySelectorAll('.session-project').forEach(details=>details.addEventListener('toggle',()=>{const project=details.dataset.projectId===state.projectId?state:projects.get(details.dataset.projectId);if(project){project.expanded=details.open;schedulePersistence();}}));
  }
  function selectSession(projectId,chatId) {
    if(projectId!==state.projectId){if(!projects.has(projectId))return;switchProject(projectId);}
    const chat=state.conversations.find(item=>item.id===chatId);if(!chat)return;
    rememberComposer();state.chat=chat;chat.reviewReturn=null;state.view='chat';render();focusComposer();
  }
  function sessionLabel(chat) {return chat.autoName?t('Nova sessão'):chat.name;}
  function projectForm(attach=false) {
    rememberComposer();
    const entries=[...projects.values()].filter(project=>project.projectId!==state.projectId).concat(state).filter(project=>!project.isLoose);
    showModal(attach?t('Vincular ao projeto'):t('Abrir ou criar projeto'),html`${entries.length?html`<section class="existing-projects"><h3>Projetos existentes</h3>${entries.map(project=>html`<button type="button" class="button" ${attach?'data-attach-project':'data-open-project'}="${esc(project.projectId)}">${icon('folder')}${esc(project.project)}</button>`).join('')}</section>`:''}<form id="session-project-form" data-attach="${attach}"><label class="form-label" for="entry-name">Nome do novo projeto</label><input id="entry-name" name="project" required maxlength="50" autocomplete="off"><label class="form-label" for="entry-path">Diretório de trabalho</label><input id="entry-path" name="path" maxlength="180" autocomplete="off" value="${esc(attach?state.projectPath:'')}"><p class="field-hint">Você pode definir o diretório agora ou depois.</p><label class="form-label" for="entry-context">Contexto compartilhado</label><textarea id="entry-context" name="context" rows="3" maxlength="4000"></textarea><div class="form-actions"><button type="button" class="button" data-action="close-modal">Cancelar</button><button type="submit" class="button primary">${attach?t('Criar e vincular sessão'):t('Criar projeto e sessão')}</button></div></form>`);
  }
  function createProject(project,path='',context='',attach=false) {
    if(!project.trim())return;
    if(!attach&&projects.size>=100){notify(t('Você atingiu o limite de 100 espaços de sessão neste navegador.'));return;}
    if(attach){state.project=project.trim();state.projectPath=path.trim();state.projectContext=context.trim();state.isLoose=false;state.expanded=true;}
    else{saveProject();state=freshState({project:project.trim(),path:path.trim(),context:context.trim(),loose:false});}
    state.view='chat';if(modal.open)closeModal();render();window.OrchestrixDock?.activate('sessions');focusComposer();
  }
  function attachToProject(id) {
    const target=projects.get(id);if(!state.isLoose||!target||target.isLoose)return;
    if(target.conversations.length>=100||target.tasks.length+state.tasks.length>500){notify(t('O projeto não tem espaço para outra sessão. Seu conteúdo foi preservado.'));return;}
    rememberComposer();const sourceId=state.projectId,session=state.chat,connections=state.connections,appSettings=state.appSettings;
    if(state.tasks.some(task=>target.tasks.some(other=>other.id===task.id))){notify(t('Não foi possível vincular esta sessão porque os trabalhos têm identificadores duplicados.'));return;}
    const next=structuredClone(target);next.conversations.push(session);next.chat=session;next.tasks.push(...state.tasks);next.events.unshift(...state.events);next.connections=connections;next.appSettings=appSettings;next.ui=ui;next.selected=state.selected;next.expanded=true;next.view='chat';
    // Existing attempts retain their original context, directory and artifact base.
    projects.delete(sourceId);state=next;if(modal.open)closeModal();render();window.OrchestrixDock?.activate('sessions');focusComposer();
  }
  function sessionDirectoryForm() {
    rememberComposer();showModal(t('Diretório da sessão'),html`<form id="session-directory-form"><label class="form-label" for="session-directory">Diretório de trabalho</label><input id="session-directory" name="path" maxlength="180" value="${esc(state.projectPath)}" autocomplete="off" placeholder="Caminho da pasta"><p class="field-hint">Este destino será usado pelas novas tarefas da sessão.</p><div class="form-actions"><button type="submit" class="button primary">Salvar diretório</button></div></form>`);
  }
  function hydrateIcons(root=document) { root.querySelectorAll('[data-icon]').forEach(el => { el.innerHTML=icon(el.dataset.icon); }); }
  function badge(task) {return html`<span class="badge ${task.status}">${esc(t(statuses[task.status]))}</span>`;}
  function action(label, actionName, primary=false, extra='') {return html`<button type="button" class="button${primary?' primary':''}" data-action="${actionName}" ${extra}>${esc(label)}</button>`;}
  function pageHead(eyebrow,title,description,buttons='') {return html`<div class="page-head"><div><div class="eyebrow">${esc(eyebrow)}</div><h1>${esc(title)}</h1><p>${esc(description)}</p></div><div class="head-actions">${buttons}</div></div>`;}
  function notify(text) {const toast=document.querySelector('#toast');toast.textContent=text;toast.classList.add('visible');clearTimeout(toastTimer);toastTimer=setTimeout(()=>toast.classList.remove('visible'),4800);}
  function record(text,detail,iconName='clock') {state.events.unshift({text,detail,icon:iconName});}
  const selectedTask = () => state.tasks.find(taskItem=>taskItem.id===state.selected);
  const artifactTask = () => state.tasks.find(taskItem=>taskItem.id==='OX-24');
  const runTasks=()=>state.tasks.filter(taskItem=>taskItem.run==='R-08');
  const runComplete=()=>!state.aggregate||runTasks().every(taskItem=>taskItem.status==='integrated');
  const resultFileCount=()=>2+(state.aggregate?runTasks().filter(taskItem=>taskItem.id!=='OX-24'&&taskItem.artifact&&taskItem.status==='integrated').length:0);
  function settleRun(run='R-08'){if(run==='R-08'&&state.aggregate&&!['ready','applied'].includes(state.runStatus)&&runComplete()){state.runStatus='ready';state.runRevision++;record(t('R-08 · todas as tarefas validadas'),text`Candidato i${state.runRevision}; revise o diff completo antes da aplicação.`,'branch');}}
  const helpers=()=>({esc,icon,action,pageHead,showModal,closeModal,render,notify,record,modelLabel,statuses,saveProject,projectOptions,prepareProjectConnection,testScenario,studioOpen:document.querySelector('#studio-navigation').open,nextProjectOrdinal:()=>projects.size+1});
  function rememberComposer() {const input=document.querySelector('#chat-message');if(input&&state.view==='chat')state.chat.selection={start:input.selectionStart,end:input.selectionEnd,direction:input.selectionDirection,scroll:input.scrollTop};}
  function saveProject() {rememberComposer();projects.set(state.projectId,structuredClone(state));schedulePersistence();}
  function projectOptions() {
    const entries=[...projects.values()].filter(project=>project.projectId!==state.projectId).concat(state).sort((a,b)=>a.projectOrdinal-b.projectOrdinal);
    return entries.map(project=>({id:project.projectId,label:`${project.project}${entries.filter(other=>other.project===project.project).length>1?text` · espaço ${project.projectOrdinal}`:''}`}));
  }
  function prepareProjectConnection(agent) {
    if(!guidedPrepared&&JSON.stringify(state.connections)===initialConnectionFingerprint){state.connections=window.OrchestrixConnections.initial().filter(connection=>connection.id===agent);guidedPrepared=true;return {connection:state.connections[0],requiresSetup:true};}
    guidedPrepared=true;
    let connection=state.connections.find(item=>item.id===agent);
    if(connection)return {connection,requiresSetup:false};
    connection=window.OrchestrixConnections.initial().find(item=>item.id===agent);state.connections.push(connection);return {connection,requiresSetup:true};
  }
  function switchProject(id) {
    if(id===state.projectId||!projects.has(id))return;
    const connections=state.connections;saveProject();state=structuredClone(projects.get(id));state.connections=connections;state.ui=ui;state.view='chat';state.chat.reviewReturn=null;
    document.querySelector('#app').classList.toggle('focus-mode',state.focus);const focus=document.querySelector('.focus-toggle');focus.setAttribute('aria-pressed',String(state.focus));focus.setAttribute('aria-label',state.focus?t('Sair do modo de foco'):t('Ativar modo de foco'));
    render();focusComposer();
  }
  function newConversation() {if(state.isLoose){looseConversation();return;}if(state.conversations.length>=100){notify(t('Este projeto atingiu o limite de 100 sessões.'));return;}rememberComposer();const conversation=W.createProjectChat(t('Nova sessão'),false);state.conversations.push(conversation);state.chat=conversation;state.view='chat';render();focusComposer();}
  function looseConversation() {
    if(projects.size>=100){notify(t('Você atingiu o limite de 100 espaços de sessão neste navegador.'));return;}
    saveProject();state=freshState();document.querySelector('#app').classList.remove('focus-mode');const focus=document.querySelector('.focus-toggle');focus.setAttribute('aria-pressed','false');focus.setAttribute('aria-label',t('Ativar modo de foco'));render();focusComposer();
  }
  const needingAttention = () => state.tasks.filter(taskItem=>['review','blocked','failed','conflict','unknown','permission'].includes(taskItem.status)||taskItem.needsRead||(taskItem.id==='OX-24'&&taskItem.status==='integrated'&&state.runStatus!=='applied'));
  const modelSupports=(c,model)=>['runtime','favorite'].includes(model)||Boolean(c?.catalog?.some(item=>item.id===model));
  function modelLabel(model,catalog=[]){return model==='runtime'?t('Preferido pelo runtime'):model==='favorite'?t(testScenario?'Favorito do perfil (exemplo)':'Favorito do perfil'):catalog.find(item=>item.id===model)?.name||(testScenario?text`Modelo ${model} (exemplo)`:text`Modelo ${model}`);}
  function modelOptions(model,connectionId){const connections=connectionId==='auto'?state.connections:state.connections.filter(c=>c.id===connectionId);const catalog=connections.flatMap(c=>c.catalog||[]).filter((item,index,items)=>items.findIndex(other=>other.id===item.id)===index);const items=[['runtime','Preferido pelo runtime'],['favorite','Favorito do perfil (exemplo)'],...catalog.map(item=>[item.id,item.name])];if(!items.some(([id])=>id===model))items.push([model,text`${modelLabel(model)} — indisponível nesta conexão`]);return items.map(([id,label])=>option(esc(id),esc(['runtime','favorite'].includes(id)?t(label):label),model)).join('');}
  function configureAttempt(task) {
    if(task.contextPack&&task.configAttempt!==task.attempt){task.attemptHistory||=[];task.attemptHistory.push({number:task.configAttempt,config:structuredClone(task.config),connectionSnapshot:structuredClone(task.connectionSnapshot),contextPack:structuredClone(task.contextPack)});}
    task.configAttempt=task.attempt;task.config={...state.preferences,systemPrompt:state.appSettings?.systemPrompt||''};
    const accounts={'codex-a':'Codex · pessoal A','codex-b':'Codex · pessoal B',claude:'Claude · pessoal'};
    if(accounts[task.config.account])task.account=accounts[task.config.account];
    const candidates=state.connections.filter(c=>window.OrchestrixConnections?.eligible(c)??true);
    const connection=task.config.account==='auto'?(candidates.find(c=>c.id===task.connectionId&&modelSupports(c,task.config.model))||candidates.find(c=>modelSupports(c,task.config.model))||state.connections.find(c=>c.id===task.connectionId)):state.connections.find(c=>c.id===task.config.account);
    if(connection){task.connectionId=connection.id;task.account=connection.name;task.connectionSnapshot={id:connection.id,name:connection.name,provider:connection.provider,status:connection.status,modality:connection.modality,authorizationRecord:connection.identity?.recordId,catalog:structuredClone(connection.catalog||[])};}
    task.contextPack=W.createContext(task,state);
  }
  function createManualTask(title,goal) {
    if(state.tasks.length>=500){notify(t('Este projeto atingiu o limite de 500 trabalhos.'));return null;}
    const allTasks=[...[...projects.values()].flatMap(project=>project.tasks),...state.tasks];
    const number=Math.max(27,...allTasks.map(task=>Number(task.id.slice(3))||0))+1;
    const runNumber=Math.max(11,...allTasks.map(task=>Number(task.run?.slice(2))||0))+1;
    const task={id:`OX-${number}`,title,status:'ready',role:'Implementação',account:t('Nenhuma conta selecionada'),summary:goal,attempt:1,run:`R-${String(runNumber).padStart(2,'0')}`,dependency:t('Definida pelo usuário'),createdByUser:true};
    configureAttempt(task);state.tasks.push(task);state.selected=task.id;state.tab='summary';
    record(text`${task.id} · tarefa criada`,title,'plus');return task;
  }
  function focusComposer() {const input=document.querySelector('#chat-message');if(!input)return;input.focus({preventScroll:true});if(state.chat.selection){const selection=state.chat.selection;input.setSelectionRange(selection.start,selection.end,selection.direction);input.scrollTop=selection.scroll;}input.scrollIntoView({block:'nearest',behavior:'instant'});}
  function chatRecord(text,taskId,note='') {state.chat.messages.push({role:'orchestrator',text,taskId,note});}
  function sendChat(messageText) {
    const message=messageText.trim();if(!message)return;
    state.chat.messages.push({role:'user',text:message});state.chat.draft='';state.chat.selection=null;
    if(state.chat.messages.filter(item=>item.role==='user').length===1){state.chat.name=message.split('\n')[0].slice(0,60);state.chat.autoName=false;}
    const active=state.tasks.find(task=>task.id===state.chat.activeTaskId);
    if(!active){
      const title=message.split('\n')[0].slice(0,100);
      const task=createManualTask(title,message);if(!task){state.chat.draft=message;state.chat.messages.pop();render();return;}state.chat.activeTaskId=task.id;state.chat.taskIds.push(task.id);
      state.chat.messages.push({role:'orchestrator',text:state.connections.length?text`Preparei o trabalho “${title}”. Confira o contexto e a conexão antes de iniciar.`:text`Preparei o trabalho “${title}”. Conecte uma conta para começar.`,plan:W.chatPlan(message),taskId:task.id});
    }else{
      state.chat.feedbackByTask||={};state.chat.feedbackByTask[active.id]=message;
      chatRecord(text`Registrei sua orientação para “${active.title}”. ${active.status==='review'?t('Use Pedir correção no card para revisar e enviar esta instrução em uma nova tentativa.'):active.status==='integrated'?t('O resultado já foi validado. Abra a revisão para conferir sua aplicação; este envio não modifica o resultado aceito.'):active.status==='running'?t('O trabalho continua na mesma tentativa. Confira o resultado antes de solicitar uma correção.'):t('O estado do trabalho foi preservado; use o próximo passo do card para continuar.')}`,active.id,t('Orientação registrada na conversa. Nenhuma tentativa foi iniciada ou alterada por este envio.'));
    }
    state.view='chat';render();focusComposer();
  }
  function chatAction(name,id) {
    if(name==='new'){state.chat.activeTaskId=null;render();focusComposer();notify(t('Descreva o próximo trabalho. Os anteriores continuam disponíveis.'));return;}
    const task=state.tasks.find(item=>item.id===id);if(!task)return;
    state.selected=task.id;
    if(name==='task'){state.tab='summary';navigate('work');window.OrchestrixDock?.activate('work');content.focus({preventScroll:true});return;}
    if(name==='continue'){state.chat.activeTaskId=task.id;chatRecord(text`Vamos continuar “${task.title}”. Escreva sua orientação ou siga o próximo passo do card.`,task.id);render();focusComposer();return;}
    if(name==='correct'){
      const comment=state.chat.draft.trim()||state.chat.feedbackByTask?.[task.id]||'';
      state.chat.reviewReturn=task.id;runAction('review');
      const input=document.querySelector(task.id==='OX-24'?'#correction':'#supplement-correction');
      if(input&&!input.disabled){input.value=comment.slice(0,1500);if(task.id==='OX-24')state.correctionDraft=input.value;input.focus();}
      return;
    }
    if(name==='review'){state.chat.reviewReturn=task.id;runAction('review');return;}
    if(name==='apply'){runAction('approve-independent');return;}
    if(name==='context'){runAction('context');return;}
    const before=task.status;runAction(name==='finish'?(task.id==='OX-24'?'finish-correction':'finish-other'):name);
    if(task.status!==before){
      chatRecord(task.status==='running'?text`A demonstração de “${task.title}” começou. Quando quiser ver o resultado, conclua o trabalho pelo card.`:task.status==='review'?text`O resultado de “${task.title}” está pronto para revisão. Confira o artefato e, se necessário, peça uma correção antes de validar.`:text`O estado de “${task.title}” foi atualizado na demonstração.`,task.id);
      render();
    }
  }
  function render() {
    const focused=document.activeElement;
    const composerSelection=focused?.id==='chat-message'?{start:focused.selectionStart,end:focused.selectionEnd,direction:focused.selectionDirection,scroll:focused.scrollTop}:null;
    const focusKey=content.contains(focused)?['chatAction','action','file','tab','task'].find(key=>focused.dataset[key]):null;
    const focusValue=focusKey?focused.dataset[focusKey]:null;
    const focusTask=focused?.dataset?.chatTask;
    document.querySelectorAll('[data-view]').forEach(el=>{el.classList.toggle('active',el.dataset.view===state.view);if(el.dataset.view===state.view)el.setAttribute('aria-current','page');else el.removeAttribute('aria-current');});
    document.querySelector('#view-name').textContent=t(viewNames[state.view]);
    document.querySelector('#project-name').textContent=state.project||state.chat.name;
    document.querySelector('#breadcrumb-project').textContent=state.project||t('Sessão independente');
    document.querySelector('#work-count').textContent=state.tasks.length;
    document.querySelector('#attention-count').textContent=needingAttention().length;
    const technical=['work','attention','review','history','settings'].includes(state.view);
    if(technical)document.querySelector('#studio-navigation').open=true;
    document.querySelector('#app').dataset.experience=technical?'studio':state.view==='chat'?'conversation':'setup';
    const views={chat:()=>window.OrchestrixExperience.chatView(state,helpers()),entry:()=>window.OrchestrixExperience.entryView(state,helpers()),work:workView,attention:attentionView,review:reviewView,history:historyView,connections:connectionsView,settings:settingsView};
    const scrollTop=content.scrollTop;
    const opened=[...content.querySelectorAll('details[open]')].map(el=>el.className);
    content.innerHTML=views[state.view]();
    content.scrollTop=scrollTop;
    content.querySelectorAll('details').forEach(el=>{if(opened.includes(el.className))el.open=true;});
    window.OrchestrixI18n.localizeStatic();
    hydrateIcons(content);
    window.OrchestrixExperience.syncLayout(mobileLayout.matches);
    if(composerSelection&&state.view==='chat'){const next=content.querySelector('#chat-message');next.focus({preventScroll:true});next.setSelectionRange(composerSelection.start,composerSelection.end,composerSelection.direction);next.scrollTop=composerSelection.scroll;}
    else if(focusKey){const attribute=focusKey.replace(/[A-Z]/g,letter=>`-${letter.toLowerCase()}`);const next=[...content.querySelectorAll(`[data-${attribute}]`)].find(el=>el.dataset[focusKey]===focusValue&&(!focusTask||el.dataset.chatTask===focusTask)&&!el.disabled);(next||content.querySelector('#chat-message')||content).focus({preventScroll:true});}
    renderSessions();schedulePersistence();window.OrchestrixDock?.sync();
  }
  function refreshInterfaceLanguage() {
    rememberComposer();
    const activeId=document.activeElement?.id;
    const openForm=modal.open?modal.querySelector('form'):null;
    const modalFormId=openForm?.id,attach=openForm?.dataset.attach==='true';
    const fields=[...content.querySelectorAll('input[id],textarea[id],select[id]'),...modal.querySelectorAll('input[id],textarea[id],select[id]')].filter(element=>!element.matches('[data-language-picker]')&&element.type!=='file').map(element=>({
      id:element.id,value:element.value,checked:element.checked,
      start:element.selectionStart,end:element.selectionEnd,direction:element.selectionDirection,scroll:element.scrollTop
    }));
    render();
    if(modalFormId==='session-project-form')projectForm(attach);
    else if(modalFormId==='session-directory-form')sessionDirectoryForm();
    else if(modalFormId==='project-context-form')runAction('project-context');
    for(const saved of fields) {
      const element=document.getElementById(saved.id);
      if(!element||!content.contains(element)&&!modal.contains(element))continue;
      element.value=saved.value;
      if(typeof saved.checked==='boolean')element.checked=saved.checked;
      if(typeof saved.start==='number'&&typeof saved.end==='number'&&element.setSelectionRange)element.setSelectionRange(saved.start,saved.end,saved.direction);
      element.scrollTop=saved.scroll;
    }
    document.querySelector('.focus-toggle').setAttribute('aria-label',t(state.focus?'Sair do modo de foco':'Ativar modo de foco'));
    if(activeId)document.getElementById(activeId)?.focus({preventScroll:true});
  }
  function workView() {
    const task=selectedTask();
    if(!task)return pageHead(t('TRABALHO'),t('Comece pela conversa'),t('Envie um pedido para preparar o primeiro trabalho deste espaço.'),action(t('Abrir conversa'),'chat',true))+emptyPanel(t('Nenhum trabalho preparado'),t('Seu pedido ficará associado a esta conversa, com tentativa e contexto próprios.'));
    return pageHead(text`OBJETIVO · ${state.platform==='wsl'?'WSL':t('WINDOWS NATIVO')}`,task.createdByUser?task.title:state.objective,t('Acompanhe o trabalho, entenda as escolhas dos agentes e decida sobre os resultados.'),`${action(t('Ver plano'),'plan')}${action(t('Nova tarefa'),'new-task',true)}`)+html`
      <div class="project-meta"><span>${icon('folder')}${esc(state.projectPath||t('Diretório não definido'))}</span><span>${state.connections.length} ${state.connections.length===1?t('conexão'):t('conexões')}</span></div><div class="panel-toolbar">${action(state.ui.queueCollapsed?t('Mostrar tarefas'):t('Recolher tarefas'),'toggle-queue',false,`aria-controls="worklist" aria-expanded="${!state.ui.queueCollapsed}"`)}</div><div class="workspace ${state.ui.queueCollapsed?'queue-collapsed':''}"><aside class="worklist" id="worklist" aria-label="Tarefas do objetivo"><div class="list-header"><span>TRABALHO DO OBJETIVO</span><span>${state.tasks.length}</span></div>${state.tasks.map(taskItem=>html`<button class="task-item ${taskItem.id===task.id?'selected':''}" data-task="${esc(taskItem.id)}" aria-pressed="${taskItem.id===task.id}"><span class="state-icon state-${taskItem.status}">${icon(statusIcons[taskItem.status])}</span><span><span class="task-id">${esc(taskItem.id)} · ${esc(t(taskItem.role))}</span><span class="task-title">${esc(taskItem.title)}</span><small>${esc(t(statuses[taskItem.status]))}</small></span></button>`).join('')}<p class="list-tip">Cada tarefa preserva suas tentativas, o contexto recebido e as evidências do resultado.</p></aside><button class="panel-resizer" role="separator" aria-label="Largura da fila de tarefas" aria-orientation="vertical" aria-valuemin="220" aria-valuemax="420" aria-valuenow="${state.ui.width}" title="Arraste ou use as setas para redimensionar"></button>
      <section class="task-detail" aria-label="Tarefa selecionada"><div class="detail-head"><div class="detail-meta"><span>${esc(task.id)} · ${esc(task.run)}</span><span>·</span><span>Tentativa ${task.attempt}</span>${badge(task)}</div><div class="detail-title"><h2>${esc(task.title)}</h2><button class="icon-button" data-action="task-info" aria-label="Ver dependências e identidade da tarefa">${icon('branch')}</button></div><div class="tabs" role="tablist" aria-label="Detalhes da tarefa">${[['summary','Resumo'],['code','Código'],['checks','Verificações'],['activity','Atividade']].map(([key,label])=>html`<button role="tab" class="tab" id="tab-${key}" aria-controls="task-panel" aria-selected="${state.tab===key}" tabindex="${state.tab===key?0:-1}" data-tab="${key}">${t(label)}</button>`).join('')}</div></div>
      <div class="detail-body" role="tabpanel" id="task-panel" aria-labelledby="tab-${state.tab}">${taskPanel(task)}</div>
      <details class="context-inspector"><summary>Conta, modelo, raciocínio e contexto</summary>${routing(task)}${action(t('Inspecionar contexto e arquivos'),'context')}</details><div class="detail-footer"><span>${esc(task.account)}<small class="execution-modality">${esc(task.connectionSnapshot?.modality||t('Modalidade desconhecida'))} · snapshot desta tentativa</small></span><div>${taskControls(task)}</div></div></section></div>`;
  }
  function taskControls(task) {
    if(task.status==='permission')return action(t('Revisar permissão'),'permission',true)+action(t('Cancelar'),'cancel');
    if(task.status==='running')return action(t('Pausar'),'pause')+action(t('Cancelar'),'cancel');
    if(task.status==='paused')return action(t('Retomar'),'resume',true)+action(t('Cancelar'),'cancel');
    if(task.status==='unknown')return action(t(testScenario?'Reconciliar demonstração':'Reconciliar execução'),'reconcile',true)+action(t('Cancelar'),'cancel');
    if(task.status==='failed')return action(t('Tentar novamente'),'retry',true);
    if(task.status==='ready')return action(t(testScenario?'Iniciar demonstração':'Iniciar trabalho'),'start',true,state.aggregate&&(task.dependencies||[]).some(id=>state.tasks.find(taskItem=>taskItem.id===id)?.status!=='integrated')?'disabled':'');
    if(task.status==='review')return action(t('Revisar alterações'),'review',true);
    if(task.status==='blocked')return action(t('Responder decisão'),'resolve',true);
    if(task.status==='conflict')return action(t('Revalidar base'),'revalidate',true);
    if(task.status==='integrated'&&task.id!=='OX-24')return action(task.applied?t('Ver Run aplicado'):t('Rever resultado e aplicação'),'review',true);
    if(task.status==='integrated')return action(state.runStatus==='applied'?t('Ver aplicação do Run'):t('Revisar aplicação do Run'),'review',true);
    return html`<span>Tentativa encerrada</span>`;
  }
  function taskPanel(task) {
    const config=task.config;
    if(!testScenario){
      if(state.tab==='code')return emptyPanel(t('O código aparece com o resultado'),t('Esta tarefa ainda não possui um resultado para revisar.'));
      if(state.tab==='checks')return emptyPanel(t('Evidências da tentativa'),t('Verificações e revisão aparecem quando o runtime fornecer um resultado.'));
      if(state.tab==='activity')return timeline(state.events.filter(event=>event.text.includes(task.id)));
      return html`<div class="attention-banner"><span>${icon('play')}</span><div><h3>Pronta para iniciar</h3><p>Confira a conexão e o contexto antes de iniciar este trabalho.</p></div></div><div class="section-label">O QUE ESTA TAREFA DEVE ENTREGAR</div><p class="prose">${esc(task.summary)}</p><ul class="acceptance">${task.contextPack.criteria.map(criterion=>html`<li>${icon('circle')}<span>${esc(criterion)}</span></li>`).join('')}</ul><div class="section-label">ESCOLHAS DA TENTATIVA</div><p class="prose">As preferências solicitadas estão registradas no contexto. Modelo, raciocínio e uso efetivos aguardam confirmação do runtime.</p>`;
    }
    if(state.tab==='code'&&task.id!=='OX-24'&&task.artifact)return html`<p class="code-note">Artefato r${task.artifact.revision} · tentativa produtora ${task.artifact.attempt} · código ilustrativo.</p><div class="context-preview"><h3>${esc(task.artifact.path)}</h3><pre><code>${esc(task.artifact.code)}</code></pre></div>${action(t('Revisar resultado complementar'),'review')}`;
    if(state.tab==='code') return task.id==='OX-24'&&state.hasArtifact?html`<p class="code-note">Artefato r${state.revision} · OX-24 · 2 arquivos. Leitura do resultado; edição leve será avaliada no piloto.</p>${diff(state.file.startsWith('extra-')?'service':state.file)}${action(t('Abrir revisão completa'),'review',false,'style="margin-top:16px"')}`:emptyPanel(t('O código aparece com o resultado'),t('Esta tarefa ainda não possui um artefato verificável na demonstração.'));
    if(state.tab==='checks')return task.id==='OX-24'&&['review','integrated'].includes(task.status)?checks():emptyPanel(t('Evidências da tentativa'),t('Verificações só ficam concluídas quando há um resultado correspondente. Esta tentativa ainda não tem evidências disponíveis.'));
    if(state.tab==='activity')return timeline(state.events.filter(e=>e.text.includes(task.id)).length?state.events.filter(e=>e.text.includes(task.id)):[{text:text`${task.id} · tentativa ${task.attempt}`,detail:t('Registro de exemplo. A execução real ainda não está implementada.'),icon:'clock'}]);
    let banner='';
    if(task.status==='ready')banner=html`<div class="attention-banner"><span>${icon('play')}</span><div><h3>${state.aggregate&&(task.dependencies||[]).some(id=>state.tasks.find(taskItem=>taskItem.id===id)?.status!=='integrated')?t('Aguardando tarefas anteriores'):t('Pronta para iniciar')}</h3><p>${state.aggregate&&(task.dependencies||[]).length?t('Dependências: ')+task.dependencies.map(id=>esc(id)).join(', ')+t('. Os resultados precisam ser validados na branch interna.'):t('Confira a conexão e as fontes antes de iniciar. Nenhuma tentativa é disparada automaticamente.')}</p></div></div>`;
    if(task.status==='permission')banner=html`<div class="attention-banner warning"><span>${icon('shield')}</span><div><h3>${task.permissionDenied?t('Permissão recusada; trabalho aguardando'):t('O agente solicita permissão')}</h3><p>Examine o comando e seu alcance. O protótipo não executa comandos.</p>${action(t('Revisar permissão'),'permission',true)}</div></div>`;
    if(task.status==='review'&&task.id!=='OX-24')banner=html`<div class="attention-banner"><span>${icon('diff')}</span><div><h3>Resultado complementar pronto para revisão</h3><p>Aceitar esta tarefa valida seu artefato na branch interna. A aplicação final continua separada.</p>${action(t('Revisar resultado complementar'),'review',true)}</div></div>`;
    if(task.status==='review'&&task.id==='OX-24')banner=html`<div class="attention-banner"><span>${icon('diff')}</span><div><h3>O resultado está pronto para sua revisão</h3><p>A revisão independente não apontou problemas pendentes neste exemplo. Examine o diff antes de aprovar.</p>${action(t('Revisar 2 arquivos'),'review',true)}</div></div>`;
    if(task.status==='blocked')banner=html`<div class="attention-banner warning"><span>${icon('alert')}</span><div><h3>Quais destinos de login são permitidos?</h3><p>A tarefa precisa dessa decisão para definir a validação. Nenhuma escolha será presumida.</p>${action(t('Responder decisão'),'resolve',true)}</div></div>`;
    if(task.status==='failed')banner=html`<div class="attention-banner warning"><span>${icon('alert')}</span><div><h3>A tentativa encerrou após falha de conexão</h3><p>O encerramento foi confirmado no exemplo (exit 1). O resultado parcial foi preservado. Uma nova tentativa terá sua própria identidade e histórico.</p>${action(t('Tentar novamente'),'retry',true)}</div></div>`;
    if(task.status==='conflict')banner=html`<div class="attention-banner warning"><span>${icon('branch')}</span><div><h3>A base mudou após a revisão</h3><p>A aprovação anterior não vale para esta base. É necessário preparar e revisar um novo resultado.</p>${action(t('Revalidar base na demonstração'),'revalidate',true)}</div></div>`;
    if(task.status==='running')banner=html`<div class="attention-banner"><span>${icon('code')}</span><div><h3>Trabalho em andamento · simulação</h3><p>O agente está na etapa de implementação. Você pode interromper a tentativa e examinar seu histórico.</p>${task.id==='OX-24'?action(state.hasArtifact?t('Concluir correção simulada'):t('Concluir trabalho simulado'),'finish-correction',true):action(t('Concluir resultado complementar simulado'),'finish-other',true)+action(t('Simular perda de sinal'),'signal-loss')}${action(t('Simular pedido de permissão'),'request-permission')}</div></div>`;
    if(task.status==='paused')banner=html`<div class="attention-banner warning"><span>${icon('pause')}</span><div><h3>Tentativa pausada · simulação</h3><p>A simulação preserva a tentativa e seu contexto. O produto real só oferecerá pausa de turno quando o adapter a suportar.</p>${action(t('Retomar demonstração'),'resume',true)}</div></div>`;
    if(task.status==='unknown')banner=html`<div class="attention-banner warning"><span>${icon('alert')}</span><div><h3>${task.cancelRequested?t('Cancelamento sem confirmação'):t('Execução sem confirmação')}</h3><p>Ausência de eventos não confirma execução nem término. A identidade do processo precisa ser reconciliada antes de retomar.</p>${action(t('Reconciliar demonstração'),'reconcile',true)}</div></div>`;
    if(task.status==='integrated'&&task.id!=='OX-24')banner=html`<div class="attention-banner"><span>${icon('branch')}</span><div><h3>Tarefa validada na branch interna de ${esc(task.run)}</h3><p>O artefato mantém sua tentativa e evidências. A aplicação final do Run é uma etapa separada.</p>${action(t('Rever resultado complementar'),'review',true)}</div></div>`;
    if(task.status==='integrated'&&task.id==='OX-24')banner=html`<div class="attention-banner"><span>${icon('branch')}</span><div><h3>Tarefa validada na branch interna de R-08</h3><p>${state.runStatus==='applied'?t('O resultado de R-08 já foi aplicado ao destino na demonstração.'):state.runStatus==='conflict'?t('A base de destino mudou. O candidato do Run precisa de nova validação.'):state.runStatus==='waiting-tasks'?t('Outras tarefas de R-08 continuam abertas; a aplicação final permanece bloqueada.'):state.aggregate?t('Todos os resultados de R-08 foram validados. Revise o candidato final.'):t('R-08 contém apenas esta tarefa e está pronto para revisão da aplicação. Os outros Runs continuam independentes.')}</p>${action(state.runStatus==='applied'?t('Ver resultado aplicado'):t('Revisar resultado do Run'),'review',true)}</div></div>`;
    if(task.id==='OX-24'&&task.status==='review'&&state.checksFailed)banner=html`<div class="attention-banner warning"><span>${icon('alert')}</span><div><h3>Uma verificação falhou neste artefato</h3><p>O teste de revogação retornou exit 1. A tarefa não pode ser validada enquanto a evidência falha estiver pendente.</p>${action(t('Examinar falha e pedir correção'),'review',true)}</div></div>`;
    return html`${banner}<div class="section-label">O QUE ESTA TAREFA DEVE ENTREGAR</div><p class="prose">${esc(task.summary)}</p><ul class="acceptance"><li>${icon((task.status==='review'&&!(task.id==='OX-24'&&state.checksFailed))||task.status==='integrated'?'checkCircle':'circle')}<span>${task.id==='OX-24'?t('Token utilizado deixa de ser válido após a rotação.'):t('Resultado alinhado ao objetivo e ao contexto do projeto.')}</span></li><li>${icon((task.status==='review'&&!(task.id==='OX-24'&&state.checksFailed))||task.status==='integrated'?'checkCircle':'circle')}<span>${task.id==='OX-24'?t('Reutilização revoga a família e registra a ocorrência.'):t('Verificações e revisão vinculadas à tentativa.')}</span></li></ul>${task.id==='OX-24'&&['review','integrated','conflict'].includes(task.status)?html`<div class="section-label">RESULTADO DA TENTATIVA</div><div class="artifact"><span>${icon('branch')}</span><div><strong>Rotação e detecção de reutilização</strong><small>Artefato r${state.revision} · 2 arquivos · revisão simulada</small></div>${action(t('Ver diff'),'review')}</div>`:''}<div class="section-label">ESCOLHA DE EXECUÇÃO <span>DEMONSTRAÇÃO</span></div><p class="prose"><strong>${esc(task.account)}</strong> foi selecionada por disponibilidade simulada e preferência do perfil. Preferência de raciocínio solicitada: ${config.thinking==='high'?t('alto'):config.thinking==='medium'?t('médio'):t('gerenciado pelo runtime')}; contexto solicitado: ${config.context==='focused'?t('focado nos arquivos da tarefa'):t('ampliado para o módulo')}. A execução real registrará o que o runtime confirmou.</p>`;
  }
  function routing(task) {if(!testScenario)return html`<dl class="routing"><div><dt>Conexão da tentativa</dt><dd>${esc(task.account)}</dd></div><div><dt>Modelo solicitado / efetivo</dt><dd>${esc(modelLabel(task.config.model,task.connectionSnapshot?.catalog))} / ${t('desconhecido')}</dd></div><div><dt>Raciocínio solicitado / efetivo</dt><dd>${esc(task.config.thinking)} / ${t('desconhecido')}</dd></div><div><dt>Context Pack da tentativa</dt><dd>${esc(task.contextPack.id)} · ${esc(task.contextPack.version)}</dd></div></dl>`;return html`<dl class="routing"><div><dt>Conexão da tentativa</dt><dd>${esc(task.account)}<small>Conta de exemplo · autenticação não verificada</small></dd></div><div><dt>Modelo solicitado / efetivo</dt><dd>${esc(modelLabel(task.config.model,task.connectionSnapshot?.catalog))} / desconhecido<small>Disponibilidade será descoberta pelo adapter.</small></dd></div><div><dt>Raciocínio solicitado / efetivo</dt><dd>${task.config.thinking==='high'?t('Alto'):task.config.thinking==='medium'?t('Médio'):t('Automático')} / desconhecido<small>O protótipo não executa modelos.</small></dd></div><div><dt>Context Pack da tentativa</dt><dd>${esc(task.contextPack.id)} · ${esc(task.contextPack.version)}<small>Objetivo, critérios, base ${esc(task.contextPack.base)} e ${task.contextPack.sources.length} referências · sessão ${esc(task.contextPack.session)}. Conversas privadas permanecem no runtime.</small></dd></div></dl>`;}
  function checks() {return html`<p class="code-note">Evidências simuladas · OX-24 · artefato r${state.revision}. Não foram executados testes neste projeto.</p><div class="check-list">${[['Typecheck',t('Sem erros · fixture de demonstração')],[t('Verificações do comportamento'),state.checksFailed?'Exit 1 · expected familyIsRevoked(session) to be true':t('Rotação e reutilização · fixture de demonstração')],[t('Revisão independente'),t('Sessão nova · resultado simulado')]].map(([title,description],index)=>html`<div class="check-row"><span>${icon(state.checksFailed&&index===1?'alert':'checkCircle')}</span><div><strong>${title}</strong><small>${description}</small></div><span class="badge ${state.checksFailed&&index===1?'failed':'integrated'}">${state.checksFailed&&index===1?t('Exemplo falhou'):t('Exemplo aprovado')}</span></div>`).join('')}</div>`;}
  function emptyPanel(title,description) {return html`<div class="empty"><span>${icon('file')}</span><h2>${esc(title)}</h2><p>${esc(description)}</p></div>`;}
  function diff(fileId=state.file) {
    const service=[[' ','42','async function rotateToken(token: string) {'],[' ','43','  const stored = await tokens.find(token);'],['-','44','  return issueTokens(stored.userId);'],['+','44','  if (stored.usedAt) {'],['+','45','    await revokeFamily(stored.familyId);'],['+','46',"    throw new TokenReuseError();"],['+','47','  }'],['+','48','  await tokens.markUsed(stored.id);'],['+','49','  return issueTokens(stored.userId, stored.familyId);'],[' ','50','}']];
    const tests=[['+','18',"it('rejects a token that was already used', async () => {"],['+','19','  const session = await createSession();'],['+','20','  await rotateToken(session.refreshToken);'],['+','21','  await expect(rotateToken(session.refreshToken))'],['+','22','    .rejects.toThrow(TokenReuseError);'],['+','23','  expect(await familyIsRevoked(session)).toBe(true);'],['+','24','});']];
    const extra=fileId.startsWith('extra-')?state.tasks.find(taskItem=>taskItem.id===fileId.slice(6))?.artifact:null;
    const rows=extra?extra.code.split('\n').map((text,index)=>['+',String(index+1),text]):fileId==='service'?service:tests;
    return html`<div class="diff"><div class="diff-header">${icon('file')}<span>${extra?esc(extra.path):fileId==='service'?'src/auth/token-service.ts':'src/auth/token-service.test.ts'}</span><span class="counts">${extra?`+${rows.length}`:fileId==='service'?'+6 −1':'+7'}</span></div><div class="diff-lines" aria-label="Diff de exemplo">${rows.map(([sign,line,text])=>html`<div class="diff-line ${sign==='+'?'add':sign==='-'?'remove':''}"><span class="line-no" aria-hidden="true">${line}</span><span class="line-sign">${sign}</span><code>${esc(text)}</code></div>`).join('')}</div></div>`;
  }
  function reviewView() {
    const task=artifactTask();
    if(!state.hasArtifact)return pageHead(t('ALTERAÇÕES'),t('O resultado aparece quando o trabalho termina'),t('Cada artefato preserva sua tarefa, tentativa, base e evidências.'),action(t('Voltar ao trabalho'),'work'))+emptyPanel(t('Ainda não há um resultado para revisar'),t(testScenario?'Inicie e conclua o trabalho simulado. Código e verificações do exemplo aparecerão aqui.':'Os resultados e suas verificações aparecerão aqui.'));
    const taskReview=task.status==='review';
    const canCorrect=['review','integrated'].includes(task.status)&&state.runStatus!=='applied';
    const canApply=task.status==='integrated'&&state.runStatus==='ready'&&runComplete();
    const applied=state.runStatus==='applied';
    const pending=state.aggregate?runTasks().filter(taskItem=>taskItem.status!=='integrated'):[];
    const cta=taskReview?action(t('Validar tarefa no Run…'),'approve-task',true,state.checksFailed?'disabled':''):state.runStatus==='conflict'?action(t('Revalidar candidato do Run'),'revalidate',true):action(applied?t('Run aplicado na demonstração'):t('Revisar aplicação do Run…'),'approve',true,!canApply?'disabled':'');
    return pageHead(taskReview?t('REVISÃO DA TAREFA · OX-24'):t('APLICAÇÃO DO RUN · R-08'),taskReview?t('Examine o resultado da tarefa'):t('Revise o resultado e o destino'),text`Artefato r${state.revision} · produzido pela tentativa ${state.artifactAttempt}${task.status==='running'?text` · tentativa ${task.attempt} em andamento`:''}.`,action(t('Voltar ao trabalho'),'work'))+html`
      <div class="review-layout"><section class="review-main">
        <div class="detail-meta">${badge(task)}<span>Artefato r${state.revision} · dados simulados</span></div>
        <h2>Rotação e detecção de reutilização</h2><p class="code-note" style="margin-top:9px">Código ilustrativo. ${state.aggregate?t('R-08 reúne as quatro tarefas. A aplicação depende de todos os resultados validados.'):t('R-08 contém somente OX-24; os outros trabalhos pertencem a Runs independentes.')}</p>
        <div class="file-switcher" role="group" aria-label="Arquivo do diff"><button data-file="service" aria-pressed="${state.file==='service'}">token-service.ts</button><button data-file="test" aria-pressed="${state.file==='test'}">token-service.test.ts</button>${state.aggregate?runTasks().filter(taskItem=>taskItem.id!=='OX-24'&&taskItem.artifact&&taskItem.status==='integrated').map(taskItem=>html`<button data-file="extra-${esc(taskItem.id)}" aria-pressed="${state.file==='extra-'+taskItem.id}">${esc(taskItem.artifact.path.split('/').pop())}</button>`).join(''):''}</div>${diff()}
        <details class="review-checks" ${state.checksFailed?'open':''}><summary>Verificações e revisão deste artefato</summary>${checks()}${action(t('Contexto da revisão'),'review-context')}</details>
        <div class="comment-box"><h3>Pedir uma correção</h3><p>Seu comentário ficará associado a OX-24 e ao artefato r${state.revision}. Uma correção cria outra tentativa e exige nova revisão.</p><form id="correction-form"><label class="form-label" for="correction">O que precisa mudar?</label><textarea id="correction" name="correction" required maxlength="1500" placeholder="Ex.: preserve o registro de auditoria ao revogar a família…" ${!canCorrect?'disabled':''}>${esc(state.correctionDraft)}</textarea><div class="form-actions"><button class="button" type="submit" ${!canCorrect?'disabled':''}>${icon('message')}Pedir correção</button></div></form></div>
      </section><aside class="review-inspector" aria-label="Identidade do resultado e aprovação"><h3>${taskReview?t('Aceite da tarefa'):t('Aplicação do Run')}</h3><dl class="routing"><div><dt>Objetivo / tarefa / Run</dt><dd>Autenticação / OX-24 / R-08</dd></div><div><dt>Artefato revisado</dt><dd>r${state.revision} · ${artifactHash()}<small>Identificador de exemplo</small></dd></div><div><dt>${taskReview?t('Branch interna do Run'):t('Destino da aplicação')}</dt><dd>${taskReview?`orchestrix/R-08 · ${state.runBase}`:`main · ${state.base}`}<small>${taskReview?t('Validar a tarefa ainda não aplica em main.'):text`Candidato de integração i${state.runRevision} · ${resultFileCount()} arquivos`}</small></dd></div><div><dt>Verificações / revisão</dt><dd>${state.checksFailed?t('Uma verificação falhou'):state.runStatus==='conflict'?state.conflictReason==='candidate'?t('Candidato alterado; revalidar'):t('Revalidar candidato de integração'):t('Exemplos aprovados')}<small>Evidências simuladas, sem execução real</small></dd></div></dl><div class="divider"></div>${state.runStatus==='waiting-tasks'?html`<p class="error-note">Aplicação aguardando ${pending.length} tarefa(s) de R-08: ${pending.map(taskItem=>esc(taskItem.id)).join(', ')}.</p>`:''}${cta}<p>${taskReview?t('Primeiro, aceite o artefato na branch interna do Run. Depois, revise a aplicação do resultado ao destino.'):t('A confirmação vale para este candidato e esta base. Mudanças exigem nova revisão da aplicação.')}</p>${taskReview?action(t('Simular verificação falha'),'fail-check',false,state.checksFailed?'disabled':''):action(t('Simular mudança de base'),'change-base',false,applied?'disabled':'')}</aside></div>`;
  }
  function artifactHash() {return state.revision===1?'a81c4e2':`a81c4e2-r${state.revision}`;}
  function attentionView() {
    const list=needingAttention();
    return pageHead(t('ATENÇÃO'),t('As decisões que movem o trabalho'),t('Cada item explica o motivo da parada e leva diretamente à próxima ação.'))+(list.length?html`<div class="attention-list">${list.map(taskItem=>html`<div class="attention-row"><span>${icon(statusIcons[taskItem.status])}</span><div><div class="detail-meta"><span>${taskItem.id}</span>${badge(taskItem)}</div><h3>${esc(taskItem.title)}</h3><p>${taskItem.needsRead?t('Uma nova evidência foi registrada; abra o trabalho correspondente.'):taskItem.status==='permission'?t('Autorizar ou recusar o comando apresentado.'):taskItem.status==='review'?t('Examine o resultado e decida sobre a aplicação.'):taskItem.status==='failed'?t('Encerramento confirmado (exemplo exit 1). O histórico foi preservado.'):taskItem.status==='unknown'?t('Reconciliar o processo; a execução está sem confirmação.'):taskItem.status==='integrated'?(state.runStatus==='conflict'?t('Revalidar o candidato sobre a nova base.'):t('Revisar a aplicação do resultado de R-08.')):taskItem.status==='conflict'?t('O resultado precisa de revisão sobre a nova base.'):t('Defina os destinos de redirecionamento permitidos.')}</p></div><button class="button" data-task="${taskItem.id}">Abrir tarefa ${icon('arrow')}</button></div>`).join('')}</div>`:emptyPanel(t('Nenhuma decisão pendente'),t('O trabalho pode continuar dentro das preferências definidas. Você ainda pode consultar cada tentativa.')));
  }
  function timeline(events) {return html`<ol class="timeline">${events.map(e=>html`<li><span>${icon(e.icon)}</span><div><strong>${esc(e.text)}</strong><small>${esc(e.detail)}</small></div></li>`).join('')}</ol>`;}
  function historyView() {return pageHead(t('MEMÓRIA DO TRABALHO'),t('Histórico de decisões e resultados'),t('O que mudou, por qual motivo e a qual tarefa ou tentativa o registro pertence.'))+html`<div class="settings-card">${timeline(state.events)}</div>`;}
  function connectionsView() {if(window.OrchestrixConnections)return window.OrchestrixConnections.view(state,helpers());return pageHead(t('SEUS AGENTES'),t('Conexões, com identidade própria'),t('Uma conta pode participar de vários papéis. A autenticação, a sessão e a cota continuam vinculadas à conexão.'),action(t('Adicionar conexão'),'new-connection',true))+html`<div class="connection-list">${state.connections.map((c,i)=>html`<div class="connection-row"><span class="provider-logo" aria-hidden="true">${esc(c.letter)}</span><div><strong>${esc(c.name)}</strong><small>${esc(c.provider)} · conexão de demonstração</small></div><div><span class="badge">${esc(t(c.status))}</span><small>Cota efetiva: desconhecida</small></div><button class="button" data-connection="${i}">Inspecionar</button></div>`).join('')}</div><p class="notice">Essas conexões são exemplos, sem credenciais. Duas contas Codex só terão capacidade independente quando a integração comprovar suas identidades e limites. Antigravity será incluído após o spike de capacidades.</p>`;}
  function option(value,label,current) {return html`<option value="${value}" ${current===value?'selected':''}>${label}</option>`;}
  function settingsView() {const p=state.preferences;return pageHead(t('CONTROL CENTER'),t('Seu jeito de orquestrar'),t('Comece com preferências simples. As decisões de cada tentativa devem continuar visíveis e explicáveis.'))+html`<div class="settings-layout"><form id="settings-form"><div class="settings-card"><h2>Perfil de execução</h2><p>Preferências do projeto para novas tentativas. As opções deste protótipo são simuladas.</p><div class="field-grid"><div><label class="form-label" for="routing">Prioridade de roteamento</label><select id="routing" name="routing">${option('balanced',t('Equilíbrio entre qualidade e disponibilidade'),p.routing)}${option('quality',t('Qualidade, respeitando limites'),p.routing)}${option('availability',t('Disponibilidade, entre opções elegíveis'),p.routing)}</select></div><div><label class="form-label" for="account">Conexão preferida</label><select id="account" name="account">${option('auto',t('Escolher automaticamente'),p.account)}${state.connections.map(c=>option(esc(c.id),esc(c.name),p.account)).join('')}</select></div><div><label class="form-label" for="model">Estratégia de modelo</label><select id="model" name="model">${modelOptions(p.model,p.account)}</select><p class="field-hint">O adapter descobrirá os modelos reais disponíveis.</p></div><div><label class="form-label" for="thinking">Raciocínio preferido</label><select id="thinking" name="thinking">${option('auto',t('Gerenciado pelo runtime'),p.thinking)}${option('medium',t('Médio'),p.thinking)}${option('high',t('Alto'),p.thinking)}</select><p class="field-hint">A escolha efetiva depende do runtime e do modelo.</p></div><div><label class="form-label" for="context">Contexto inicial</label><select id="context" name="context">${option('focused',t('Focado na tarefa'),p.context)}${option('module',t('Ampliado para o módulo'),p.context)}</select><p class="field-hint">Objetivo, critérios e evidências estão sempre presentes.</p></div></div></div><div class="settings-card"><h2>Revisão e aplicação</h2><label class="form-label" for="review">Preferência de revisor</label><select id="review" name="review">${option('fresh',t('Sessão nova no runtime disponível'),p.review)}${option('other',t('Preferir outro provedor, se elegível'),p.review)}</select><p class="field-hint">Uma única conta continua permitindo revisão em sessão separada.</p><p style="margin-top:18px">Aplicação: confirmar resultado e base antes de integrar.</p><div class="form-actions">${action(t('Inspecionar política'),'policy')}<button class="button primary" type="submit">Salvar preferências simuladas</button></div></div></form><aside class="settings-note"><div class="eyebrow">ESCOLHAS EXPLICÁVEIS</div><h2>Automático também deve ser compreensível.</h2><p style="margin-top:14px">Primeiro, o Core filtra as conexões elegíveis por capacidade, permissões e limites. Depois aplica suas preferências.</p><p style="margin-top:14px">Cada tentativa preserva <strong>solicitado, efetivo e desconhecido</strong>. Alterar uma preferência não muda silenciosamente a identidade de uma tentativa existente.</p><p style="margin-top:14px">Políticas avançadas e um grafo de dependências serão acessíveis por contexto, sem ocupar a tela principal.</p></aside></div>`;}
  function showModal(title,body) {if(!modal.open)opener=document.activeElement;modal.classList.toggle('dialog-wide',body.includes('context-pack')||body.includes('conn-inspector')||body.includes('context-preview'));modal.innerHTML=html`<div class="modal-head"><h2 id="modal-title">${esc(title)}</h2><button class="icon-button" data-action="close-modal" aria-label="Fechar">${icon('close')}</button></div><div class="modal-body">${body}</div>`;modal.showModal();}
  function canRestoreFocus(element) {
    return element instanceof HTMLElement&&element!==document.body&&element!==document.documentElement&&
      element.isConnected&&!element.disabled&&!element.closest('[inert]')&&element.getClientRects().length>0&&
      getComputedStyle(element).visibility==='visible';
  }
  function restoreModalFocus() {
    if(modal.open)return;
    // Closing the native dialog can queue its close event after a render has
    // explicitly focused the next screen. Preserve that valid destination.
    if(canRestoreFocus(document.activeElement))return;
    if(canRestoreFocus(opener)){opener.focus({preventScroll:true});return;}
    (content.querySelector('#chat-message')||content).focus({preventScroll:true});
  }
  function closeModal() {modal.close();restoreModalFocus();}
  modal.addEventListener('close',restoreModalFocus);
  modal.addEventListener('click',event=>{if(event.target===modal){const r=modal.getBoundingClientRect();if(event.clientX<r.left||event.clientX>r.right||event.clientY<r.top||event.clientY>r.bottom)closeModal();}});
  function commandMenu() {
    opener=document.activeElement;modal.classList.remove('dialog-wide');
    modal.innerHTML=html`<input class="command-search" id="command-search" aria-label="Buscar ações" placeholder="O que você quer fazer?" autocomplete="off"><h2 id="modal-title" class="sr-only" style="padding:0 20px;font-size:12px;color:var(--muted)">Ações do workspace</h2><div class="command-list" id="command-list"></div>`;
    commands('');modal.showModal();document.querySelector('#command-search').focus();
  }
  const commandItems=[['Conversa','chat','message',''],['Nova sessão','chat-loose','plus',''],['Novo projeto','onboarding','folder',''],['Nova tarefa','new-task','plus','N'],['Precisa de você','attention','inbox',''],['Conexões','connections','plug',''],['Configurações','app-settings','sliders',''],['Alternar modo de foco','focus','focus',''],['Ver atalhos','shortcuts','code','?']];
  function commands(query) {
    const locale=window.OrchestrixI18n.language,needle=query.toLocaleLowerCase(locale);
    const matches=label=>label.toLocaleLowerCase(locale).includes(needle);
    const actions=commandItems.filter(([label])=>matches(t(label))).map(([label,act,ico,key])=>html`<button data-command="${act}">${icon(ico)}${t(label)}${key?html`<kbd>${key}</kbd>`:''}</button>`);
    const entries=[...projects.values()].filter(project=>project.projectId!==state.projectId).concat(state);
    const sessions=entries.flatMap(project=>project.conversations.filter(chat=>matches([sessionLabel(chat),project.project,project.projectPath].join(' '))).map(chat=>html`<button data-search-project="${esc(project.projectId)}" data-search-session="${esc(chat.id)}">${icon('message')}<span>${esc(sessionLabel(chat))}<small>${esc(project.project||t('Sessão independente'))}</small></span></button>`));
    document.querySelector('#command-list').innerHTML=[...sessions,...actions].join('')||html`<p class="empty-result">Nenhuma sessão, projeto ou ação encontrada.</p>`;
  }
  function navigate(view) {rememberComposer();if(view==='settings'){window.OrchestrixSettings?.open('orchestration');return;}if(view!=='review')state.chat.reviewReturn=null;state.view=view;render();}
  function runAction(name) {
    const task=selectedTask();
    if(name==='review'&&task?.id!=='OX-24'&&task?.artifact){W.supplementReview(task,state,helpers());return;}
    if(viewNames[name]) {if(name==='review')state.selected='OX-24';navigate(name);return;}
    if(name==='commands'){commandMenu();return;}
    if(name==='app-settings'){window.OrchestrixSettings?.open('general');return;}
    if(name==='connect-account'){window.OrchestrixConnections.addForm(state,helpers());return;}
    if(name==='session-directory'){sessionDirectoryForm();return;}
    if(name==='session-attach'){projectForm(true);return;}
    if(name==='project-context'){showModal(t('Contexto do projeto'),html`<form id="project-context-form"><label class="form-label" for="project-context">Contexto compartilhado</label><textarea id="project-context" name="context" rows="6" maxlength="4000">${esc(state.projectContext||'')}</textarea><label class="form-label" for="project-directory">Diretório de trabalho</label><input id="project-directory" name="path" maxlength="180" value="${esc(state.projectPath)}"><p class="field-hint">As alterações valem para novas tarefas. Tentativas existentes preservam seu contexto.</p><div class="form-actions"><button class="button primary" type="submit">Salvar contexto</button></div></form>`);return;}
    if(name==='explore-demo'){state.chat.exampleVisible=true;navigate('work');content.focus({preventScroll:true});return;}
    if(name==='chat-new'){if(state.view!=='chat')state.view='chat';chatAction('new');return;}
    if(name==='chat-session-new'){newConversation();return;}
    if(name==='chat-loose'){looseConversation();return;}
    if(name==='appearance'){window.OrchestrixSettings?.open('themes');return;}
    if(name==='default-theme'){setTheme('studio');notify(t('Studio restaurado como tema do workspace.'));return;}
    if(name==='close-modal'){closeModal();return;}
    if(name==='toggle-queue'){state.ui.queueCollapsed=!state.ui.queueCollapsed;render();content.querySelector('[data-action="toggle-queue"]').focus();return;}
    if(name==='reset-layout'){Object.assign(ui,{queueCollapsed:false,width:285,density:'compact',textSize:'normal',textScale:100});W.saveUI(ui);render();notify(t('Layout restaurado.'));return;}
    if(name==='context'){W.contextModal(task,state,helpers());return;}
    if(name==='approve-independent'){W.independentApproval(task,state,helpers());return;}
    if(name==='review-context'){W.reviewContextModal(artifactTask(),state,helpers());return;}
    if(name==='context-next'){if(modal.open)closeModal();W.nextContextForm(state,helpers());return;}
    if(name==='new-event'){const incoming=state.tasks.find(taskItem=>taskItem.id!==state.selected)||task;if(!incoming){notify(t('Prepare um trabalho antes de simular uma evidência.'));return;}incoming.needsRead=true;record(text`${incoming.id} · nova evidência de exemplo`,t('Evento de atenção: a tarefa, o arquivo e o campo em foco permanecem selecionados.'),'clock');document.querySelector('#attention-count').textContent=needingAttention().length;notify(text`Nova evidência de ${incoming.id} em Precisa de você. Seu trabalho permanece em foco.`);return;}
    if(name==='reset'){if(modal.open)closeModal();reset();notify(t('Demonstração reiniciada. Nenhum arquivo foi alterado.'));return;}
    if(name==='focus'){state.focus=!state.focus;document.querySelector('#app').classList.toggle('focus-mode',state.focus);const b=document.querySelector('.focus-toggle');b.setAttribute('aria-pressed',String(state.focus));b.setAttribute('aria-label',state.focus?t('Sair do modo de foco'):t('Ativar modo de foco'));notify(state.focus?t('Modo de foco ativado. Use o mesmo botão para sair.'):t('Navegação e fila de tarefas restauradas.'));return;}
    if(name==='onboarding'){projectForm();return;}
    if(name==='onboarding-legacy'){showModal(t('Seu projeto, seus agentes'),html`<p>O Orchestrix parte de um repositório local. Este estudo usa um projeto fictício e não acessa suas pastas.</p><form id="project-form"><label class="form-label" for="project-input">Nome do projeto de demonstração</label><input id="project-input" name="project" value="${esc(state.project)}" required maxlength="50"><div class="snapshot">1. Abrir projeto<br>2. Conectar uma conta própria<br>3. Definir trabalho e revisar resultados</div><p class="field-hint">A conexão real e a seleção de diretório entram após os spikes do runtime.</p><div class="form-actions"><button class="button primary" type="submit">Abrir demonstração</button></div></form>`);return;}
    if(name==='new-task'){showModal(t('Novo trabalho'),html`<p>Defina o resultado esperado e como você quer revisá-lo.</p><form id="task-form"><label class="form-label" for="task-title">Título da tarefa</label><input id="task-title" name="title" required maxlength="100" placeholder="Título do trabalho"><label class="form-label" for="task-goal">Objetivo e critérios</label><textarea id="task-goal" name="goal" required maxlength="2000" placeholder="Descreva o comportamento esperado e como revisar o resultado…"></textarea><div class="form-actions"><button class="button" type="button" data-action="close-modal">Cancelar</button><button class="button primary" type="submit">${testScenario?t('Criar tarefa simulada'):t('Criar trabalho')}</button></div></form>`);return;}
    if(name==='plan'&&!testScenario){if(!task)return;showModal(t('Plano do trabalho'),html`<div class="snapshot"><strong>${esc(task.title)}</strong><p>${esc(task.summary)}</p><ol>${W.chatPlan(task.summary).map(step=>html`<li>${esc(step)}</li>`).join('')}</ol></div>`);return;}
    if(name==='plan'&&state.tasks.length===1){showModal(t('Objetivo da correção curta'),html`<p>Uma tarefa manual, com objetivo e critérios. Não é necessário definir um plano extenso.</p><div class="snapshot">${esc(task.summary)}<ul><li>Token utilizado deixa de ser válido.</li><li>Reutilização revoga a família e preserva auditoria.</li></ul></div><p>Implementação → evidências → revisão → correção, se necessária → aceite da tarefa → aplicação do Run. São etapas sequenciais na mesma conexão.</p>`);return;}
    if(name==='plan'){showModal(t('Plano do objetivo'),html`<p>Plano ilustrativo aprovado pelo usuário. O primeiro alpha trabalha com tarefas definidas manualmente; o planejador autônomo é uma entrega posterior.</p><ol class="acceptance"><li>${icon('circle')}<span><strong>1. Fortalecer sessões</strong><br>OX-24 rotação · OX-25 revogação, independentes.</span></li><li>${icon('circle')}<span><strong>2. Validar destinos de login</strong><br>OX-26 aguarda uma decisão explícita.</span></li><li>${icon('circle')}<span><strong>3. Documentar o comportamento</strong><br>OX-27 depende dos resultados de OX-24 e OX-25.</span></li></ol><div class="form-actions">${action(t('Voltar ao trabalho'),'close-modal',true)}</div>`);return;}
    if(name==='task-info'){showModal(t('Identidade e dependências'),html`<div class="snapshot">Tarefa: <strong>${esc(task.id)}</strong><br>Tentativa: <strong>${task.attempt}</strong><br>Dependência: ${esc(task.dependency)}<br>Conexão: ${esc(task.account)}<br>Estado: ${esc(t(statuses[task.status]))}</div>${task.attemptHistory?.length?html`<details><summary>Tentativas anteriores preservadas</summary><ul>${task.attemptHistory.map(item=>html`<li>Tentativa ${item.number} · ${esc(item.connectionSnapshot?.name||t('conexão desconhecida'))} · raciocínio solicitado ${esc(item.config.thinking)} · ${esc(item.contextPack.id)}</li>`).join('')}</ul></details>`:''}<p>Encerrar uma tentativa não significa aceitar seu resultado ou aplicar um Run. Cada etapa mantém sua própria evidência.</p>`);return;}
    if(name==='request-permission'&&task.status==='running'){task.status='permission';task.permissionDenied=false;record(text`${task.id} · permissão solicitada`,t('Comando de verificação ilustrativo; aguarda decisão do usuário.'),'shield');render();return;}
    if(name==='permission'&&task.status==='permission'){showModal(t('Permissão para esta tentativa'),html`<p>Solicitação de ${esc(task.id)} · tentativa ${task.attempt} · ${esc(task.account)}. Examine antes de decidir.</p><div class="snapshot">Comando ilustrativo: <code>npm test -- token-service</code><br>Diretório: ${esc(state.projectPath)}<br>Alcance: verificações do comportamento; ferramentas podem gravar caches.<br>Estado: não executado nesta simulação.</div><p>Recusar mantém o trabalho aguardando. Permitir libera somente esta solicitação.</p><div class="form-actions">${action(t('Recusar na demonstração'),'deny-permission')}${action(t('Permitir na demonstração'),'allow-permission',true)}</div>`);return;}
    if(name==='allow-permission'&&task.status==='permission'){task.status='running';task.permissionDenied=false;record(text`${task.id} · permissão concedida na simulação`,t('Mesmo contexto e tentativa. Nenhum comando real foi executado.'),'shield');closeModal();render();return;}
    if(name==='deny-permission'&&task.status==='permission'){task.permissionDenied=true;record(text`${task.id} · permissão recusada`,t('A tarefa continua aguardando; nenhuma escolha presumida.'),'shield');closeModal();render();return;}
    if(name==='pause'&&task.status==='running'){task.status='paused';record(text`${task.id} · pausa solicitada`,t('Transição simulada; o produto real aguardará confirmação do runtime.'),'pause');render();notify(t('Tentativa pausada na demonstração.'));return;}
    if(name==='signal-loss'&&task.status==='running'){task.status='unknown';record(text`${task.id} · sinal de execução perdido`,t('Reconciliar identidade do processo antes de declarar execução ou término.'),'alert');render();notify(t('Estado sem confirmação simulado.'));return;}
    if(name==='reconcile'&&task.status==='unknown'){if(task.cancelRequested){task.status='cancelled';task.cancelRequested=false;record(text`${task.id} · término confirmado após reconciliação`,t('Confirmação simulada da identidade do worker; nenhuma tentativa duplicada.'),'stop');render();notify(t('Cancelamento confirmado na demonstração.'));return;}task.status='running';record(text`${task.id} · processo reconciliado na demonstração`,t('Identidade simulada confirmada; o produto real exigirá evidência do supervisor.'),'play');render();notify(t('Reconciliação simulada; mesma tentativa e conexão.'));return;}
    if(name==='resume'&&task.status==='paused'){task.status='running';task.signalLost=false;record(text`${task.id} · retomada simulada`,t('Mesma tentativa e mesma conexão.'),'play');render();return;}
    if(name==='cancel'){showModal(t('Cancelar esta tentativa?'),html`<p>A operação se refere a ${esc(task.id)}, tentativa ${task.attempt}. O histórico e os artefatos já produzidos permanecem identificados.</p><div class="form-actions">${action(t('Voltar'),'close-modal')}${action(t('Simular cancelamento sem resposta'),'cancel-unknown',false,`data-target="${task.id}"`)}${action(t('Cancelar tentativa simulada'),'confirm-cancel',true,`data-target="${task.id}"`)}</div>`);return;}
    if(name==='cancel-unknown'){task.cancelRequested=true;task.status='unknown';record(text`${task.id} · cancelamento solicitado sem confirmação`,t('Término desconhecido. Reconciliar antes de preparar outra tentativa.'),'alert');closeModal();render();notify(t('Cancelamento sem confirmação. A tentativa permanece sob reconciliação.'));return;}
    if(name==='confirm-cancel'){task.status='cancelled';record(text`${task.id} · tentativa cancelada`,t('Simulação de confirmação de término, sem subprocesso real.'),'stop');closeModal();render();return;}
    if(name==='retry'&&task.status==='failed'){task.attempt++;configureAttempt(task);task.status='ready';record(text`${task.id} · nova tentativa preparada`,text`Tentativa ${task.attempt}; a anterior permanece no histórico.`,'reset');render();notify(t('Uma nova tentativa foi preparada; ainda não está em execução.'));return;}
    if(name==='start'&&task.status==='ready'){if(!testScenario){notify(t('O runtime ainda não está disponível para esta conexão. O pedido e o contexto foram preservados.'));window.OrchestrixSettings?.open('accounts');return;}if(state.aggregate&&(task.dependencies||[]).some(id=>state.tasks.find(taskItem=>taskItem.id===id)?.status!=='integrated')){notify(t('Esta etapa depende de OX-24 e OX-25 validadas no Run. Nenhuma execução foi iniciada.'));return;}const c=state.connections.find(c=>c.id===task.connectionId);if(!c||!window.OrchestrixConnections.eligible(c)||!modelSupports(c,task.config.model)){notify(t('A conexão desta tentativa não está elegível. Inspecione Conexões; o trabalho e seu contexto foram preservados.'));navigate('connections');return;}task.connectionSnapshot={id:c.id,name:c.name,provider:c.provider,status:c.status,authorizationRecord:c.identity.recordId,modality:c.modality,catalog:structuredClone(c.catalog)};task.status='running';record(text`${task.id} · execução simulada iniciada`,text`Tentativa ${task.attempt} · ${task.account}`,'play');render();return;}
    if(name==='resolve'){showModal(t('Definir destinos permitidos'),html`<p>OX-26 precisa de uma decisão do projeto. O exemplo usa caminhos locais como /conta; não há acesso a URLs.</p><form id="decision-form"><label class="form-label" for="destinations">Destinos após o login</label><input id="destinations" name="destinations" required maxlength="250" placeholder="/conta, /configuracoes"><div class="form-actions"><button class="button primary" type="submit">Registrar decisão simulada</button></div></form>`);return;}
    if(name==='finish-other'&&task.id!=='OX-24'&&task.status==='running'){task.artifact=W.createSupplement(task,state);task.status='review';record(text`${task.id} · resultado complementar preparado`,text`Tentativa ${task.attempt} · artefato r${task.artifact.revision} · checks fictícios.`,'shield');render();return;}
    if(name==='finish-correction'&&task.id==='OX-24'&&task.status==='running'){task.status='review';state.revision++;state.hasArtifact=true;state.artifactAttempt=task.attempt;state.checksFailed=false;state.runStatus='task-review';state.approvedRevision=null;record(t('OX-24 · correção e revisão simuladas concluídas'),text`Tentativa ${task.attempt} · novo artefato r${state.revision}`,'shield');render();notify(t('Novo artefato preparado. A revisão anterior não autoriza sua aplicação.'));return;}
    if(name==='fail-check'&&artifactTask().status==='review'){state.checksFailed=true;record(t('OX-24 · verificação simulada falhou'),text`Artefato r${state.revision} · exit 1 · revogação esperada.`,'alert');render();notify(t('Falha de verificação simulada. O aceite da tarefa está bloqueado.'));return;}
    if(name==='approve-task'){const taskItem=artifactTask();if(taskItem.status!=='review'||state.checksFailed)return;showModal(t('Validar esta tarefa no Run?'),html`<p>Aceitar este artefato integra a tarefa na branch interna de R-08. A aplicação em main será uma revisão separada.</p><div class="snapshot">Tarefa: <strong>OX-24</strong> · tentativa ${state.artifactAttempt}<br>Artefato: <code>r${state.revision} · ${artifactHash()}</code><br>Branch interna: <code>orchestrix/R-08 · ${state.runBase}</code></div><div class="form-actions">${action(t('Voltar'),'close-modal')}${action(t('Validar na demonstração'),'confirm-task',true,`data-snapshot="${state.revision}:${state.runBase}"`)}</div>`);return;}
    if(name==='approve'){const taskItem=artifactTask();if(taskItem.status!=='integrated'||state.runStatus!=='ready'||!runComplete())return;const snapshot=`${state.revision}:${state.runRevision}:${state.base}`;showModal(t('Aplicar o resultado de R-08?'),html`<p>Confirme a identidade do resultado e o destino. Esta ação modifica somente os dados em memória da demonstração.</p><div class="snapshot">Run: <strong>R-08</strong> · ${state.aggregate?runTasks().map(taskItem=>esc(taskItem.id)).join(', '):'OX-24'} validada(s)<br>Artefato: <code>r${state.revision} · ${artifactHash()}</code><br>Candidato: <code>i${state.runRevision}</code><br>Destino: <code>main · ${state.base}</code><br>Verificações: exemplos aprovados</div><p id="approval-error" class="error-note" role="alert"></p><div class="form-actions">${action(t('Simular candidato alterado'),'change-candidate')}${action(t('Simular base alterada'),'change-base')}${action(t('Aplicar na demonstração'),'confirm-apply',true,`data-snapshot="${snapshot}"`)}</div>`);return;}
    if(name==='change-candidate'&&state.runStatus==='ready'&&artifactTask().status==='integrated'){state.runRevision++;state.runStatus='conflict';state.conflictReason='candidate';state.approvedRevision=null;record(t('R-08 · candidato alterado'),text`Candidato i${state.runRevision}; confirmação anterior invalidada.`,'branch');render();if(modal.open){document.querySelector('#approval-error').textContent=t('O candidato mudou. Esta confirmação expirou; revalide e revise o resultado.');modal.querySelector('[data-action="confirm-apply"]').disabled=true;}return;}
    if(name==='change-base'&&state.runStatus!=='applied'&&artifactTask().status==='integrated'){state.base=state.base==='b9f87d3'?'c3a91f0':'d71ba25';state.runStatus='conflict';state.conflictReason='base';state.approvedRevision=null;record(t('R-08 · base de destino alterada'),text`Nova base ${state.base}; a aprovação da aplicação foi invalidada.`,'branch');render();if(modal.open&&document.querySelector('#approval-error')){document.querySelector('#approval-error').textContent=t('A base mudou. Esta confirmação expirou; revalide e revise o resultado.');modal.querySelector('[data-action="confirm-apply"]').disabled=true;}else notify(t('Mudança de base simulada. A aplicação está bloqueada até nova revisão.'));return;}
    if(name==='revalidate'&&state.runStatus==='conflict'){state.runRevision++;state.runStatus=runComplete()?'ready':'waiting-tasks';record(t('R-08 · candidato revalidado na demonstração'),text`Candidato i${state.runRevision} · mesma tentativa de agente · base ${state.base}`,'shield');render();notify(t('Novo candidato de integração preparado; revisão da aplicação necessária.'));return;}
    if(name==='new-connection'&&window.OrchestrixConnections){window.OrchestrixConnections.addForm(state,helpers());return;}
    if(name==='new-connection'){showModal(t('Adicionar uma conexão de exemplo'),html`<p>A autenticação real utilizará os mecanismos oficiais de cada runtime. Não informe tokens ou credenciais aqui.</p><form id="connection-form"><label class="form-label" for="provider">Runtime</label><select id="provider" name="provider"><option>Codex</option><option>Claude Code</option></select><label class="form-label" for="connection-name">Nome para reconhecer esta conta</label><input id="connection-name" name="name" required maxlength="70" placeholder="Ex.: Codex · segunda conta"><div class="form-actions"><button class="button primary" type="submit">Adicionar exemplo</button></div></form>`);return;}
    if(name==='policy'){showModal(t(testScenario?'Política do projeto · simulação':'Política do projeto'),html`<p>Preferências respeitam capacidades, permissões e limites do runtime.</p><div class="snapshot">Roteamento: ${esc(state.preferences.routing)}<br>Conta preferida: ${esc(state.preferences.account)}<br>Raciocínio: ${esc(state.preferences.thinking)}<br>Contexto: ${esc(state.preferences.context)}<br>Revisor: ${esc(state.preferences.review)}</div><p>Resultados e destino exigem revisão antes de aplicar alterações.</p>`);return;}
    if(name==='shortcuts'){showModal(t('Atalhos e navegação'),html`<div class="snapshot"><kbd>Ctrl K</kbd> ou <kbd>⌘ K</kbd> · buscar ações<br>${testScenario?html`<kbd>Ctrl Shift E</kbd> · evento simulado sem mover foco<br>`:''}<kbd>N</kbd> · nova tarefa, fora de campos de texto<br><kbd>?</kbd> · esta ajuda<br><kbd>Esc</kbd> · fechar diálogo<br><kbd>Tab</kbd> · navegar por controles<br><kbd>←</kbd> / <kbd>→</kbd> · navegar pelas abas da tarefa</div><p>O modo de foco recolhe navegação e fila. A janela compacta organiza os mesmos controles em sequência.</p>`);}
  }
  document.addEventListener('click',event=>{
    const button=event.target.closest('button');if(!button||button.disabled)return;
    if(button.dataset.searchSession){closeModal();selectSession(button.dataset.searchProject,button.dataset.searchSession);return;}
    if(button.dataset.sessionId){selectSession(button.dataset.sessionProject,button.dataset.sessionId);return;}
    if(button.dataset.newSessionProject){if(button.dataset.newSessionProject!==state.projectId)switchProject(button.dataset.newSessionProject);newConversation();return;}
    if(button.dataset.attachProject){attachToProject(button.dataset.attachProject);return;}
    if(button.dataset.openProject){if(modal.open)closeModal();switchProject(button.dataset.openProject);window.OrchestrixDock?.activate('sessions');return;}
    if(button.dataset.prompt){state.chat.draft=button.dataset.prompt;render();focusComposer();return;}
    if(button.dataset.chatAction){chatAction(button.dataset.chatAction,button.dataset.chatTask);return;}
    if(window.OrchestrixConnections?.click(button,state,helpers()))return;
    if(button.dataset.contextSource){W.contextModal(selectedTask(),state,helpers(),button.dataset.contextSource);modal.querySelector(`[data-context-source="${button.dataset.contextSource}"]`).focus();return;}
    if(button.dataset.view){state.chat.reviewReturn=null;navigate(button.dataset.view);return;}
    if(button.dataset.task){state.chat.reviewReturn=null;state.selected=button.dataset.task;selectedTask().needsRead=false;state.view='work';state.tab='summary';render();return;}
    if(button.dataset.tab){state.tab=button.dataset.tab;render();document.querySelector(`[data-tab="${state.tab}"]`).focus();return;}
    if(button.dataset.file){state.file=button.dataset.file;render();return;}
    if(button.dataset.connection){const c=state.connections[Number(button.dataset.connection)];showModal(t('Identidade da conexão'),html`<div class="snapshot"><strong>${esc(c.name)}</strong><br>Runtime: ${esc(c.provider)}<br>Autenticação: não verificada<br>Modelo disponível: desconhecido<br>Capacidade efetiva: desconhecida<br>Grupo de capacidade: a descobrir</div><p>Configurações separadas não comprovam contas ou cotas independentes. O spike do adapter precisa validar esse comportamento antes da execução.</p>`);return;}
    if(button.dataset.command){const command=button.dataset.command;closeModal();runAction(command);return;}
    if(button.dataset.action==='confirm-independent'){const target=state.tasks.find(taskItem=>taskItem.id===button.dataset.target);if(!target?.artifact||target.run==='R-08'||state.isLoose||target.artifact.base!==state.base||target.status!=='integrated'||target.applied||!target.artifact.checked||button.dataset.snapshot!==`${target.artifact.hash}:${state.base}:${target.run}`){notify(t('A confirmação expirou. Confira o Run, o artefato e a base atuais.'));closeModal();return;}target.applied=true;target.appliedBase=state.base;target.appliedArtifact=target.artifact.hash;record(text`${target.run} · aplicação independente simulada concluída`,text`${target.id} · ${target.artifact.hash} · main · base ${state.base}; não altera R-08.`,'checkCircle');if(state.view==='chat')chatRecord(text`A aplicação de “${target.title}” foi confirmada na demonstração. O resultado e suas tentativas continuam disponíveis. Nenhum arquivo real foi modificado.`,target.id);closeModal();render();return;}
    if(button.dataset.action==='confirm-supplement'){const target=state.tasks.find(taskItem=>taskItem.id===button.dataset.target);if(!target?.artifact||target.status!=='review'||button.dataset.snapshot!==`${target.artifact.hash}:${target.artifact.attempt}`){notify(t('O aceite expirou. Revise o resultado atual.'));return;}target.status='integrated';record(text`${target.id} · resultado complementar validado`,text`Artefato ${target.artifact.hash} · branch interna ${target.run}; sem aplicação no destino.`,'checkCircle');if(state.view==='chat')chatRecord(text`Você validou o resultado de “${target.title}”. ${state.isLoose?t('O destino continua pendente; esta conversa avulsa não pode aplicar o resultado em um repositório.'):t('Use Revisar aplicação para conferir o destino e confirmar a etapa final.')}`,target.id);settleRun(target.run);closeModal();render();return;}
    if(button.dataset.action==='confirm-task'){const taskItem=artifactTask();if(button.dataset.snapshot!==`${state.revision}:${state.runBase}`||taskItem.status!=='review'||state.checksFailed){closeModal();notify(t('O aceite expirou. Confira o artefato e suas evidências.'));return;}taskItem.status='integrated';state.runStatus=state.aggregate&&runTasks().some(other=>other.id!==taskItem.id&&other.status!=='integrated')?'waiting-tasks':'ready';settleRun();record(t('OX-24 · artefato validado na branch de R-08'),text`Artefato r${state.revision} · nenhuma aplicação em main.`,'checkCircle');closeModal();render();notify(t('Tarefa validada no Run. A aplicação no destino ainda precisa de revisão.'));return;}
    if(button.dataset.action==='confirm-apply'){const taskItem=artifactTask();if(button.dataset.snapshot!==`${state.revision}:${state.runRevision}:${state.base}`||taskItem.status!=='integrated'||state.runStatus!=='ready'||!runComplete()){notify(t('A confirmação expirou. Revise o resultado e a base atuais.'));closeModal();return;}state.runStatus='applied';state.approvedRevision=state.revision;record(t('R-08 · aplicação simulada concluída'),text`Candidato i${state.runRevision} · artefato r${state.revision} · main · base ${state.base}`,'checkCircle');closeModal();render();notify(t('Run aplicado somente na demonstração. O repositório não foi alterado.'));return;}
    if(button.dataset.action)runAction(button.dataset.action);
  });
  document.addEventListener('submit',event=>{
    const form=event.target;if(!(form instanceof HTMLFormElement))return;const moduleData=new FormData(form);
    if(form.id==='session-project-form'||form.id==='entry-form'){event.preventDefault();createProject(String(moduleData.get('project')||''),String(moduleData.get('path')||''),String(moduleData.get('context')||''),form.dataset.attach==='true');return;}
    if(form.id==='session-directory-form'){event.preventDefault();state.projectPath=String(moduleData.get('path')||'').trim();closeModal();render();focusComposer();return;}
    if(form.id==='project-context-form'){event.preventDefault();state.projectPath=String(moduleData.get('path')||'').trim();state.projectContext=String(moduleData.get('context')||'').trim();closeModal();render();return;}
    if(window.OrchestrixConnections?.submit(form,moduleData,state,helpers())){event.preventDefault();return;}
    if(form.id==='chat-form'){event.preventDefault();sendChat(String(moduleData.get('message')||''));return;}
    if(form.id==='supplement-correction-form'){event.preventDefault();const task=selectedTask(),comment=String(moduleData.get('correction')||'').trim();if(!comment||task.status!=='review')return;task.attempt++;task.correction=comment;configureAttempt(task);task.status=window.OrchestrixConnections.eligible(state.connections.find(c=>c.id===task.connectionId))&&modelSupports(state.connections.find(c=>c.id===task.connectionId),task.config.model)?'running':'ready';if(state.aggregate&&task.run==='R-08')state.runStatus='waiting-tasks';record(text`${task.id} · correção complementar solicitada`,comment,'message');if(state.view==='chat'){state.chat.messages.push({role:'user',text:text`Correção enviada: ${comment}`});chatRecord(text`A correção de “${task.title}” foi enviada na demonstração, em uma nova tentativa. ${task.status==='running'?t('Conclua esta etapa pelo card e revise o novo resultado.'):t('Inspecione a conexão antes de iniciar pelo card.')}`,task.id);state.chat.draft='';}closeModal();render();return;}
    if(form.id==='context-form'){event.preventDefault();state.contextSelection=moduleData.getAll('source').filter(value=>['service','test','policy'].includes(value));record(t('Contexto futuro atualizado'),t('Snapshots de tentativas existentes foram preservados.'),'book');closeModal();notify(t('Referências salvas para novas tentativas.'));return;}
    if(!['task-form','correction-form','settings-form','decision-form','project-form','connection-form'].includes(form.id))return;event.preventDefault();const data=new FormData(form);
    if(form.id==='task-form'){const title=String(data.get('title')).trim(),goal=String(data.get('goal')).trim();if(!title||!goal)return;createManualTask(title,goal);state.view='work';closeModal();render();notify(t(testScenario?'Tarefa criada em memória, pronta para iniciar a demonstração.':'Trabalho preparado. Confira a conexão e o contexto antes de iniciar.'));}
    if(form.id==='correction-form'){const comment=String(data.get('correction')).trim();if(!comment||!['review','integrated'].includes(artifactTask().status)||state.runStatus==='applied')return;const task=artifactTask(),fromChat=state.chat.reviewReturn===task.id;state.chat.reviewReturn=null;state.comment=comment;state.correctionDraft='';task.attempt++;task.correction=comment;configureAttempt(task);task.status=window.OrchestrixConnections.eligible(state.connections.find(c=>c.id===task.connectionId))&&modelSupports(state.connections.find(c=>c.id===task.connectionId),task.config.model)?'running':'ready';state.runStatus='task-review';state.approvedRevision=null;record(t('OX-24 · correção solicitada'),text`Artefato r${state.revision}: ${comment}`,'message');state.selected='OX-24';state.view=fromChat?'chat':'work';state.tab='summary';if(fromChat){state.chat.draft='';state.chat.messages.push({role:'user',text:text`Correção enviada: ${comment}`});chatRecord(text`A correção de “${task.title}” foi registrada em uma nova tentativa simulada. Confira o próximo passo no card.`,task.id);}render();if(fromChat)focusComposer();notify(t('Correção simulada em uma nova tentativa. O resultado anterior não autoriza aplicação.'));}
    if(form.id==='settings-form'){state.preferences=Object.fromEntries(data);record(t('Preferências do projeto atualizadas'),t('Aplicáveis a novas tentativas na demonstração.'),'sliders');notify(t('Preferências salvas nesta demonstração; tentativas existentes mantêm sua configuração.'));}
    if(form.id==='decision-form'){const answer=String(data.get('destinations')).trim();if(!answer)return;const task=state.tasks.find(taskItem=>taskItem.id==='OX-26');task.status='ready';task.decision=answer;task.contextUpdates||=[];task.contextUpdates.push(structuredClone(task.contextPack));task.contextPack=W.createContext(task,state);task.contextPack.version=`c${task.attempt}.${task.contextUpdates.length}`;record(t('OX-26 · decisão registrada'),answer,'message');closeModal();render();notify(t('Decisão registrada. A tarefa está pronta, sem iniciar automaticamente.'));}
    if(form.id==='project-form'){const project=String(data.get('project')).trim();if(!project)return;state.project=project;state.view='connections';closeModal();render();notify(t('Projeto de exemplo aberto. Nenhuma pasta foi acessada.'));}
    if(form.id==='connection-form'){const name=String(data.get('name')).trim();if(!name)return;const provider=String(data.get('provider'));state.connections.push({name,provider,letter:provider==='Codex'?'C':'✳',status:t('Identidade não verificada')});closeModal();render();notify(t('Conexão de exemplo adicionada, sem autenticação.'));}
  });
  document.addEventListener('input',event=>{if(event.target.id==='entry-path')event.target.setCustomValidity('');if(event.target.id==='command-search')commands(event.target.value);if(event.target.id==='correction')state.correctionDraft=event.target.value;if(event.target.id==='chat-message'){state.chat.draft=event.target.value;schedulePersistence();}});
  document.addEventListener('orchestrix:language-change',refreshInterfaceLanguage);
  document.addEventListener('change',event=>{if(event.target.matches('[data-language-picker]')){window.OrchestrixI18n.setLanguage(event.target.value);return;}if(event.target.id==='chat-project'){switchProject(event.target.value);return;}if(event.target.id==='chat-session'){rememberComposer();const conversation=state.conversations.find(item=>item.id===event.target.value);if(conversation){state.chat=conversation;state.chat.reviewReturn=null;render();focusComposer();}return;}if(event.target.id==='account'){const model=document.querySelector('#model');model.innerHTML=modelOptions(model.value,event.target.value);}if(event.target.matches('input[name="workspace-theme"]'))setTheme(event.target.value);if(event.target.id==='density'){ui.density=event.target.value;W.saveUI(ui);}if(event.target.id==='text-size'){ui.textSize=event.target.value;W.saveUI(ui);}if(event.target.id==='entry-platform'){const path=document.querySelector('#entry-path');path.value=event.target.value==='wsl'?'/home/leo/Meu projeto – sessão':'C:\\Projetos\\Meu projeto – sessão';path.setCustomValidity('');document.querySelector('#entry-path-hint').textContent=event.target.value==='wsl'?t('Caminho Linux no WSL. A seleção deste ambiente é explícita.'):t('Caminho Windows nativo. Espaços e acentos permanecem na localização.');}});
  document.addEventListener('keydown',event=>{
    if(event.target.matches('.panel-resizer')&&['ArrowLeft','ArrowRight','Home','End'].includes(event.key)){event.preventDefault();ui.width=event.key==='Home'?220:event.key==='End'?420:Math.max(220,Math.min(420,ui.width+(event.key==='ArrowRight'?16:-16)));W.saveUI(ui);event.target.setAttribute('aria-valuenow',ui.width);return;}
    if(testScenario&&(event.ctrlKey||event.metaKey)&&event.shiftKey&&event.key.toLowerCase()==='e'){event.preventDefault();runAction('new-event');return;}
    if(event.target.id==='chat-message'&&event.key==='Enter'&&!event.shiftKey&&!event.isComposing&&event.keyCode!==229&&!event.ctrlKey&&!event.metaKey&&!event.altKey){event.preventDefault();event.target.form.requestSubmit();return;}
    const isInput=event.target.matches('input,textarea,select,[contenteditable="true"]');
    if((event.ctrlKey||event.metaKey)&&event.key.toLowerCase()==='k'){event.preventDefault();if(modal.open)closeModal();commandMenu();return;}
    if(event.target.matches('[role="tab"][data-tab]')&&['ArrowLeft','ArrowRight','Home','End'].includes(event.key)){event.preventDefault();const tabs=['summary','code','checks','activity'];const index=tabs.indexOf(state.tab);state.tab=event.key==='Home'?tabs[0]:event.key==='End'?tabs[3]:tabs[(index+(event.key==='ArrowRight'?1:3))%4];render();document.querySelector(`[data-tab="${state.tab}"]`).focus();return;}
    if(event.target.id==='command-search'&&event.key==='ArrowDown'){event.preventDefault();modal.querySelector('.command-list button')?.focus();return;}
    if(modal.open&&event.target.matches('[data-command]')&&['ArrowDown','ArrowUp'].includes(event.key)){event.preventDefault();const buttons=[...modal.querySelectorAll('[data-command]')],index=buttons.indexOf(event.target);buttons[(index+(event.key==='ArrowDown'?1:buttons.length-1))%buttons.length].focus();return;}
    if(!isInput&&!modal.open&&!event.ctrlKey&&!event.metaKey&&!event.altKey){if(event.key.toLowerCase()==='n'){event.preventDefault();runAction('new-task');}if(event.key==='?'){event.preventDefault();runAction('shortcuts');}}
  });
  document.addEventListener('pointerdown',event=>{if(!event.target.matches('.panel-resizer'))return;const target=event.target,start=event.clientX,width=ui.width;target.setPointerCapture(event.pointerId);const move=e=>{ui.width=Math.round(Math.max(220,Math.min(420,width+e.clientX-start)));W.applyUI(ui);target.setAttribute('aria-valuenow',ui.width);};const stop=()=>{target.removeEventListener('pointermove',move);target.removeEventListener('pointerup',stop);target.removeEventListener('pointercancel',stop);W.saveUI(ui);};target.addEventListener('pointermove',move);target.addEventListener('pointerup',stop);target.addEventListener('pointercancel',stop);});
  try{setTheme(localStorage.getItem('orchestrix-prototype-theme')||'studio',false);}catch{setTheme('studio',false);}
  function updateSettings(patch={}) {
    const next={systemPrompt:'',profileName:'',subscriptionOnly:true,...state.appSettings};
    if(typeof patch.systemPrompt==='string')next.systemPrompt=patch.systemPrompt.slice(0,8000);
    if(typeof patch.profileName==='string')next.profileName=patch.profileName.slice(0,80);
    next.subscriptionOnly=patch.subscriptionOnly!==false;
    for(const project of projects.values())project.appSettings={...next};state.appSettings=next;return {...next};
  }
  function savePreferences(patch={}) {
    const allowed={routing:['balanced','quality','availability','efficiency','performance','custom'],thinking:['auto','medium','high'],context:['focused','module'],review:['fresh','other']};
    const next={...state.preferences};
    for(const key of Object.keys(next)){const value=patch[key];if(typeof value==='string'&&value.length<120&&(!allowed[key]||allowed[key].includes(value)))next[key]=value;}
    state.preferences=next;schedulePersistence();return {...next};
  }
  window.OrchestrixApp=Object.freeze({getState:()=>state,helpers,getThemes:()=>themes.map(theme=>({...theme,colors:[...theme.colors]})),setTheme,getUI:()=>ui,saveUI:()=>W.saveUI(ui),getSettings:()=>({...state.appSettings}),updateSettings,savePreferences,openProject:projectForm,newSession:looseConversation,selectSession,createProject,attachToProject,persistSessions,...(testScenario?{resetScenario,prepareScenario(data){W.prepareProject(data,state,{...helpers(),initialTasks,configureAttempt});}}:{})});
  window.addEventListener('pagehide',persistSessions);
  hydrateIcons();if(testScenario)resetScenario();else if(loadSessions())render();else reset();
  document.dispatchEvent(new CustomEvent('orchestrix:app-ready'));
})();
