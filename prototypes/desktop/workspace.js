/* D1 interaction fixtures. No repository or private conversation is read. */
(() => {
  'use strict';
  const referenceFiles = [
    {id:'service',path:'src/auth/token-service.ts',version:'b9f87d3:42–50',reason:'Implementação diretamente afetada',text:'async function rotateToken(token: string) {\n  const stored = await tokens.find(token);\n  return issueTokens(stored.userId);\n}'},
    {id:'test',path:'src/auth/token-service.test.ts',version:'b9f87d3:18–24',reason:'Critérios verificáveis da rotação',text:"it('rotates an active session token', async () => {\n  const session = await createSession();\n  expect(await rotateToken(session.refreshToken)).toBeDefined();\n});"},
    {id:'policy',path:'docs/session-policy.md',version:'fixture-policy-v1',reason:'Decisões do projeto sobre sessões',text:'# Session policy\n\nA reutilização deve revogar a família de tokens.\nPreservar uma entrada de auditoria para a ocorrência.'},
    {id:'module',path:'src/auth/session-service.ts',version:'fixture-module-v1',reason:'Módulo ampliado solicitado no perfil',text:'export async function revokeFamily(familyId: string) {\n  await sessions.revokeByFamily(familyId);\n}'}
  ];
  function loadUI() {
    let saved={};try{saved=JSON.parse(localStorage.getItem('orchestrix-prototype-layout')||'{}')||{};}catch{}
    return {queueCollapsed:false,width:Math.max(220,Math.min(420,Number(saved.width)||285)),density:saved.density==='compact'?'compact':'comfortable',textSize:saved.textSize==='large'?'large':'normal'};
  }
  function applyUI(ui) {
    document.documentElement.dataset.density=ui.density;
    document.documentElement.dataset.textSize=ui.textSize;
    document.documentElement.style.setProperty('--worklist-width',`${ui.width}px`);
  }
  function saveUI(ui) {applyUI(ui);try{localStorage.setItem('orchestrix-prototype-layout',JSON.stringify({width:ui.width,density:ui.density,textSize:ui.textSize}));}catch{}}
  function createContext(task,state) {
    const selected=state.contextSelection||['service','test','policy'];
    const files=referenceFiles.filter(file=>selected.includes(file.id)||(task.config.context==='module'&&file.id==='module')).map(file=>({...file}));
    return {id:`CP-${task.id}-${task.attempt}`,version:`c${task.attempt}`,base:state.base,objective:task.summary,correction:task.correction||'',decision:task.decision||'',criteria:task.criteria||['Resultado alinhado ao objetivo.','Evidências e revisão vinculadas ao artefato.'],sources:files,session:`S-${task.id}-${task.attempt}`,connectionId:task.connectionId,model:task.config.model,thinking:task.config.thinking};
  }
  function contextModal(task,state,h,sourceId) {
    const pack=task.contextPack;
    const file=pack.sources.find(source=>source.id===sourceId)||pack.sources[0];
    h.showModal('Contexto e arquivos da tentativa',`<div class="project-meta"><span class="badge">${h.esc(pack.id)} · ${h.esc(pack.version)}</span><span>${h.esc(task.id)} · tentativa ${task.attempt} · base ${h.esc(pack.base)}</span></div><p>Snapshot enviado nesta demonstração. Cada fonte mostra sua origem, versão e motivo de inclusão. Nenhuma pasta foi lida.</p><div class="snapshot"><strong>Objetivo</strong><p>${h.esc(pack.objective)}</p>${pack.correction?`<strong>Instrução desta correção</strong><p>${h.esc(pack.correction)}</p>`: ''}${pack.decision?`<strong>Decisão incorporada</strong><p>${h.esc(pack.decision)}</p>`: ''}<strong>Critérios</strong><ul>${pack.criteria.map(text=>`<li>${h.esc(text)}</li>`).join('')}</ul></div><div class="context-pack"><div class="context-sources" role="group" aria-label="Fontes do contexto">${pack.sources.map(source=>`<button class="context-source" data-context-source="${h.esc(source.id)}" aria-pressed="${source.id===file?.id}"><strong>${h.esc(source.path)}</strong><small>${h.esc(source.version)} · ${h.esc(source.reason)}</small></button>`).join('')}</div>${file?`<section class="context-preview" aria-label="Conteúdo da fonte"><h3>${h.esc(file.path)}</h3><p>${h.esc(file.version)} · ${file.text.length} caracteres · exemplo integral</p><pre><code>${h.esc(file.text)}</code></pre></section>`:'<p>Nenhuma referência de código foi incluída; objetivo e critérios continuam presentes.</p>'}</div><p class="field-hint">Sessão ${h.esc(pack.session)} · conversa privada do runtime fora deste pacote. Efetivo e consumo de tokens: desconhecidos.</p><div class="form-actions">${h.action('Preparar contexto para nova tentativa','context-next')}${h.action('Voltar','close-modal')}</div>`);
  }
  function nextContextForm(state,h) {
    const selected=state.contextSelection||['service','test','policy'];
    h.showModal('Contexto da próxima tentativa',`<p>Escolha as referências para novas tentativas. O pacote já associado à tentativa atual permanece com sua versão original.</p><form id="context-form"><fieldset class="context-options"><legend>Referências de exemplo</legend>${referenceFiles.filter(file=>file.id!=='module').map(file=>`<label><input type="checkbox" name="source" value="${file.id}" ${selected.includes(file.id)?'checked':''}><span><strong>${h.esc(file.path)}</strong><small>${h.esc(file.reason)}</small></span></label>`).join('')}</fieldset><p class="field-hint">Objetivo, critérios e base sempre acompanham a tentativa. O perfil ampliado acrescenta session-service.ts.</p><div class="form-actions"><button class="button primary" type="submit">Salvar contexto futuro</button></div></form>`);
  }
  function reviewContextModal(task,state,h) {
    const producer=task.attemptHistory?.find(item=>item.number===state.artifactAttempt);
    const pack=producer?.contextPack||task.contextPack;
    const account=producer?.connectionSnapshot?.name||task.account;
    const config=producer?.config||task.config;
    h.showModal('Contexto da revisão',`<div class="project-meta"><span class="badge">CP-REVIEW-${h.esc(task.id)}-r${state.revision}</span><span>S-REVIEW-${h.esc(task.id)}-r${state.revision}</span></div><p>Manifesto ilustrativo da sessão revisora. Ela usa a mesma conexão ${h.esc(account)} com contexto separado; não significa outra conta ou mais cota.</p><dl class="routing"><div><dt>Modelo solicitado / efetivo</dt><dd>${h.esc(h.modelLabel(config.model,producer?.connectionSnapshot?.catalog||task.connectionSnapshot?.catalog))} / desconhecido</dd></div><div><dt>Origem e diversidade</dt><dd>Runtime da conexão · diversidade de modelo não comprovada</dd></div></dl><div class="snapshot"><strong>Fontes desta revisão</strong><ul><li>Objetivo e critérios de ${h.esc(pack.id)} · ${h.esc(pack.version)}</li><li>Diff do artefato r${state.revision} · tentativa produtora ${state.artifactAttempt} · base ${h.esc(state.runBase)}</li><li>Evidências de r${state.revision} · ${state.checksFailed?'verificação falha pendente':'checks e revisão fictícios'}</li></ul><p>Conversas privadas do implementador não são transportadas. Tokens efetivamente enviados: desconhecidos.</p></div><div class="form-actions">${h.action('Voltar ao resultado','close-modal')}</div>`);
  }
  function createSupplement(task,state) {
    const fixtures={
      'OX-25':{path:'src/auth/session-service.ts',code:"export async function revokeActiveSessions(userId: string) {\n  await sessions.revokeForUser(userId);\n  await audit.record('session.revoked', { userId });\n}"},
      'OX-26':{path:'src/auth/redirect-policy.ts',code:"const allowedDestinations = new Set(['/conta', '/configuracoes']);\nexport function safeDestination(path: string) {\n  return allowedDestinations.has(path) ? path : '/conta';\n}"},
      'OX-27':{path:'docs/session-policy.md',code:'# Session policy\n\nTokens utilizados não podem ser reutilizados.\nRevogar a família e registrar a ocorrência na auditoria.\nDestinos de login: /conta e /configuracoes.'}
    };
    const fixture=fixtures[task.id]||{path:'docs/requested-change.md',code:`# ${task.title}\n\n${task.summary}\n\nRepresentação do resultado pretendido; código real ainda não foi executado.`};
    if(task.id==='OX-26'&&task.decision){const destinations=task.decision.split(',').map(value=>value.trim()).filter(value=>value.startsWith('/'));fixture.code=`const allowedDestinations = new Set(${JSON.stringify(destinations)});\nexport function safeDestination(path: string) {\n  return allowedDestinations.has(path) ? path : ${JSON.stringify(destinations[0]||'/conta')};\n}`;}
    if(task.id==='OX-27'&&state){const decision=state.tasks.find(t=>t.id==='OX-26')?.decision;if(decision)fixture.code=fixture.code.replace('Destinos de login: /conta e /configuracoes.',`Destinos de login: ${decision}.`);}
    const revision=(task.artifact?.revision||0)+1;
    return {...fixture,revision,attempt:task.attempt,hash:`demo-${task.id}-r${revision}`,base:task.contextPack.base,correction:task.correction||'',checked:true};
  }
  function supplementReview(task,state,h) {
    const artifact=task.artifact;
    h.showModal(`Revisão da tarefa ${task.id}`,`<div class="project-meta"><span class="badge">${h.esc(task.id)} · ${h.esc(task.run)}</span><span>Artefato r${artifact.revision} · tentativa produtora ${artifact.attempt} · ${h.esc(artifact.hash)}</span></div><p>${h.esc(task.summary)}</p><div class="context-preview"><h3>${h.esc(artifact.path)}</h3><p>Resultado complementar ilustrativo · acréscimos de exemplo · base ${h.esc(artifact.base)}</p><pre><code>${h.esc(artifact.code)}</code></pre></div><div class="snapshot">Critérios: alinhamento ao objetivo e evidências vinculadas.<br>Verificações e revisão: fixtures aprovadas, sem execução real.<br>Branch interna: orchestrix/${h.esc(task.run)}.<br>Aceitar esta tarefa não aplica o Run no destino.</div>${task.status==='review'?`<form id="supplement-correction-form"><label class="form-label" for="supplement-correction">Correção deste resultado</label><textarea id="supplement-correction" name="correction" required maxlength="1500"></textarea><div class="form-actions"><button type="submit" class="button">Pedir correção desta tarefa</button></div></form>`:''}<div class="form-actions">${h.action('Voltar','close-modal')}${task.status==='review'?h.action('Validar resultado complementar','confirm-supplement',true,`data-target="${h.esc(task.id)}" data-snapshot="${h.esc(artifact.hash)}:${artifact.attempt}"`):'<span class="badge integrated">Tarefa validada no Run</span>'}${task.status==='integrated'&&task.run!=='R-08'?(task.applied?`<span class="badge integrated">Run ${h.esc(task.run)} aplicado · ${h.esc(task.appliedBase)}</span>`:h.action('Revisar aplicação deste Run','approve-independent',true)):''}</div>`);
  }
  function entryView(state,h) {
    return h.pageHead('COMEÇAR','Um projeto, um resultado','Abra o trabalho com uma conexão e amplie a orquestração quando precisar.',h.action('Voltar ao workspace','work'))+`<div class="entry-layout"><section class="entry-card settings-card"><div class="eyebrow">PROJETO DE DEMONSTRAÇÃO</div><h2>Preparar seu espaço</h2><p>Este percurso não acessa pastas nem autentica contas. Use os exemplos para avaliar a experiência.</p><form id="entry-form"><label class="form-label" for="entry-name">Nome do projeto</label><input id="entry-name" name="project" value="orchestrix" required maxlength="50"><label class="form-label" for="entry-platform">Ambiente de execução</label><select id="entry-platform" name="platform"><option value="native">Windows nativo</option><option value="wsl">WSL · Linux</option></select><label class="form-label" for="entry-path">Caminho do repositório · exemplo</label><input id="entry-path" name="path" value="C:\\Projetos\\Meu projeto – sessão" required maxlength="180"><p class="field-hint" id="entry-path-hint">Caminhos com espaços e acentos são exibidos como uma única localização.</p><label class="form-label" for="entry-template">Trabalho inicial</label><select id="entry-template" name="template"><option value="short">Correção curta · 1 tarefa</option><option value="feature">Feature guiada · 4 tarefas</option></select><div class="snapshot"><strong>Exemplo: rotação de refresh tokens</strong><p>Invalidar tokens utilizados, revogar a família ao detectar reutilização e preservar auditoria. O resultado será código ilustrativo.</p></div><label class="form-label" for="entry-agent">Conexão inicial</label><select id="entry-agent" name="agent"><option value="codex-a">Codex · conta própria de exemplo</option><option value="claude">Claude Code · conta própria de exemplo</option></select><p class="field-hint">Uma conexão permite implementar e revisar em sessões separadas. Duas sessões não aumentam a cota.</p><div class="form-actions"><button class="button primary" type="submit">Preparar projeto de exemplo</button></div></form></section><aside class="entry-steps settings-note"><div class="eyebrow">SEU PERCURSO</div><h2>Da intenção ao resultado revisado.</h2><ol class="acceptance"><li>${h.icon('folder')}<span><strong>1. Projeto e conexão</strong><br>Identidade visível, sem configurar uma frota.</span></li><li>${h.icon('book')}<span><strong>2. Objetivo e contexto</strong><br>Critérios e fontes antes de iniciar.</span></li><li>${h.icon('code')}<span><strong>3. Trabalho e atenção</strong><br>Decisões, limites e falhas com próxima ação.</span></li><li>${h.icon('diff')}<span><strong>4. Revisar e corrigir</strong><br>Diff e evidências associados ao artefato.</span></li><li>${h.icon('checkCircle')}<span><strong>5. Aplicar com controle</strong><br>Tarefa aceita no Run, depois candidato revisado no destino.</span></li></ol><p style="margin-top:24px">A demo atual continua disponível até você preparar este cenário. Temas e preferências visuais permanecem salvos.</p></aside></div>`;
  }
  function independentApproval(task,state,h) {
    const artifact=task.artifact;
    if(task.run==='R-08'||task.status!=='integrated'||!artifact?.checked||task.applied)return;
    h.showModal(`Aplicar o resultado de ${task.run}?`,`<p>Confira o resultado, sua identidade e o destino. Esta confirmação modifica somente dados da demonstração.</p><div class="snapshot">Run: <strong>${h.esc(task.run)}</strong> · ${h.esc(task.id)} validada<br>Artefato: <code>${h.esc(artifact.hash)}</code> · tentativa produtora ${artifact.attempt}<br>Candidato: <code>i${artifact.revision}</code> · ${h.esc(artifact.path)}<br>Destino: <code>main · ${h.esc(state.base)}</code><br>Evidências: exemplos aprovados, sem execução real.</div><div class="form-actions">${h.action('Voltar','close-modal')}${h.action('Aplicar este Run na demonstração','confirm-independent',true,`data-target="${h.esc(task.id)}" data-snapshot="${h.esc(artifact.hash)}:${h.esc(state.base)}:${h.esc(task.run)}"`)}</div>`);
  }
  function prepareProject(data,state,h) {
    const project=String(data.get('project')).trim(),path=String(data.get('path')).trim(),platform=String(data.get('platform'));
    if(!project||!path)return;
    const input=document.querySelector('#entry-path');
    if(platform==='wsl'&&!path.startsWith('/')){input.setCustomValidity('No WSL, use um caminho Linux, por exemplo /home/leo/meu-projeto.');input.reportValidity();return;}
    if(platform==='native'&&!/^[a-zA-Z]:[\\/]/.test(path)){input.setCustomValidity('No Windows nativo, use um caminho como C:\\Projetos\\Meu projeto.');input.reportValidity();return;}
    state.project=project;state.projectPath=path;state.platform=platform;
    const agent=String(data.get('agent'));
    state.connections=window.OrchestrixConnections.initial().filter(c=>c.id===agent);
    const connection=state.connections[0];
    Object.assign(connection,{lifecycle:'pending',status:'Autorização pendente · simulação',acceptsNewWork:false,authorizationValid:false,planConfirmed:false,consentMode:null,pendingRegistration:{revision:1,mode:'own-plan'}});
    Object.assign(connection.identity,{recordId:null,revision:0,validation:'pending'});
    state.tasks=h.initialTasks();state.aggregate=data.get('template')==='feature';
    if(!state.aggregate)state.tasks=state.tasks.slice(0,1);
    state.preferences.account='auto';state.base='b9f87d3';state.contextSelection=['service','test','policy'];
    state.tasks.forEach(t=>{t.status=t.id==='OX-26'?'blocked':'ready';t.run='R-08';t.dependencies=t.id==='OX-27'?['OX-24','OX-25']:[];t.connectionId=agent;h.configureAttempt(t);});
    state.selected='OX-24';state.objective=state.aggregate?'Uma autenticação mais segura':'Corrigir rotação de tokens';state.view='work';state.tab='summary';
    state.revision=0;state.hasArtifact=false;state.runBase=state.base;state.runRevision=1;state.runStatus='task-review';state.artifactAttempt=1;state.checksFailed=false;state.correctionDraft='';state.comment='';state.file='service';state.approvedRevision=null;state.events=[];
    h.record('Projeto de demonstração preparado',`${project} · ${platform==='wsl'?'WSL':'Windows nativo'} · uma conexão; nenhuma pasta foi acessada.`,'folder');
    h.render();h.notify('Projeto preparado. Inspecione o contexto antes de iniciar.');
    window.OrchestrixConnections.inspect(connection.id,state,h);
  }
  window.OrchestrixWorkspace={loadUI,applyUI,saveUI,createContext,contextModal,nextContextForm,reviewContextModal,createSupplement,supplementReview,independentApproval,entryView,prepareProject};
})();
