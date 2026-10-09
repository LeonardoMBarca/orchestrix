/* D1 study only. No credentials, provider requests, OAuth or runtime execution. */
(() => {
  'use strict';

  const labels = {
    active: 'Disponível · simulação', pending: 'Autorização pendente · simulação',
    limited: 'Limite observado · simulação', expired: 'Autorização expirada · simulação',
    denied: 'Acesso negado · simulação', disconnected: 'Novos trabalhos bloqueados · simulação'
  };
  const modeLabels = {
    'own-plan': 'Plano próprio do provedor · simulação',
    'api': 'API com cobrança separada · simulação'
  };
  const openStatuses = new Set(['running', 'paused', 'unknown', 'blocked']);

  function exampleCatalog(provider) {
    const codex = provider === 'Codex';
    return [{id: codex ? 'codex-example' : 'claude-example',
      name: codex ? 'Modelo Codex de exemplo' : 'Modelo Claude de exemplo',
      thinking: ['medium', 'high'], simulated: true}];
  }

  function makeConnection(id, name, provider, workspace, capacityGroup, active) {
    return {
      id, name, provider, letter: provider === 'Codex' ? 'C' : '✳',
      lifecycle: active ? 'active' : 'pending', status: labels[active ? 'active' : 'pending'],
      simulated: true, mode: 'own-plan', modality: modeLabels['own-plan'], workspace,
      capacityGroup, capacity: 'Desconhecida; grupos distintos não comprovam cotas independentes',
      identity: {owner: `demo-owner-${id}`, label: `${name} (identidade fictícia)`,
        email: provider === 'Codex' ? 'pessoa@example.invalid' : 'outra-pessoa@example.invalid',
        recordId: active ? `demo-${id}-r1` : null, revision: active ? 1 : 0,
        validation: active ? 'validated-simulated' : 'pending'},
      planConfirmed: active, consentMode: active ? 'own-plan' : null,
      acceptsNewWork: active, authorizationValid: active, catalog: exampleCatalog(provider), catalogCompatible: true,
      catalogStatus: 'Catálogo ilustrativo; acesso real desconhecido',
      workerTermination: 'Não confirmado', logout: 'Não solicitado', revocation: 'Não solicitada',
      pendingRegistration: active ? null : {revision: 1, mode: 'own-plan'},
      history: [{text: active ? 'Registro inicial validado e modalidade confirmada na simulação.' :
        'Registro de exemplo aguarda autorização simulada.', revision: active ? 1 : 0}]
    };
  }

  function initial() {
    return [
      makeConnection('codex-a', 'Codex · pessoal A', 'Codex', 'Pessoal A · exemplo', 'demo-capacity-a', true),
      makeConnection('codex-b', 'Codex · pessoal B', 'Codex', 'Pessoal B · exemplo', 'demo-capacity-b', false),
      makeConnection('claude', 'Claude · pessoal', 'Claude Code', 'Pessoal · exemplo', 'demo-capacity-claude', true)
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
    `<button type="button" class="button conn-button${primary ? ' primary' : ''}" data-conn-action="${h.esc(action)}"${id ? ` data-conn-id="${h.esc(id)}"` : ''}>${h.esc(text)}</button>`;
  const fact = (label, value, h) => `<div><dt>${h.esc(label)}</dt><dd>${h.esc(value)}</dd></div>`;
  const status = (c, h) => `<span class="badge conn-status ${c.lifecycle === 'active' ? 'ready' : 'blocked'}">${h.esc(c.status)}</span>`;

  function change(c, lifecycle, text, detail, h) {
    c.lifecycle = lifecycle;
    c.status = labels[lifecycle];
    c.acceptsNewWork = lifecycle === 'active';
    c.history.unshift({text, revision: c.identity.revision});
    h.record(`${c.name} · ${text}`, `${detail} · simulação; tentativas anteriores preservadas.`, 'plug');
  }

  function view(state, h) {
    return h.pageHead('CONEXÕES · SIMULAÇÃO', 'Contas distintas, escolhas claras',
      'Escolha a conexão para novos trabalhos. Sessões existentes preservam sua conta, configuração e contexto.',
      button('Adicionar conexão simulada', 'add', null, h, true)) +
      `<div class="connection-list conn-list">${(state.connections || []).map(c => {
        const tasks = related(c, state);
        return `<article class="connection-row conn-row" aria-label="${h.esc(c.name)}">
          <span class="provider-logo" aria-hidden="true">${h.esc(c.letter)}</span>
          <div><strong>${h.esc(c.name)}</strong><small>${h.esc(c.provider)} · ${h.esc(c.workspace)}</small>
            <small>${h.esc(c.modality)}</small></div>
          <div>${status(c, h)}<small>${eligible(c) ? 'Elegível para nova tentativa simulada' : 'Indisponível para nova tentativa'}</small>
            <small>${tasks.length} ${tasks.length === 1 ? 'tentativa vinculada' : 'tentativas vinculadas'} · capacidade real desconhecida</small></div>
          ${button('Inspecionar conexão', 'inspect', c.id, h)}</article>`;
      }).join('')}</div>
      <p class="notice conn-note">Todos os registros e catálogos são fictícios, sem credenciais. Conta, workspace e grupo de capacidade têm identidades próprias; mais sessões não significam mais cota. Nenhum limite muda a origem de cobrança automaticamente.</p>`;
  }

  function catalogView(c, state, h) {
    if(c.catalogLoading)return '<div class="attention-banner" role="status" aria-live="polite"><div><h3>Descobrindo catálogo · simulação</h3><p>Modelos disponíveis e acesso ainda não foram confirmados. Novas tentativas aguardam; as existentes preservam seus snapshots.</p></div></div>';
    const p = state.preferences || {};
    const requestedModel = p.model === 'runtime' ? 'Preferência do runtime' :
      p.model === 'favorite' ? 'Favorito do perfil' : p.model || 'Não definido';
    const requestedThinking = {auto: 'Gerenciado pelo runtime', medium: 'Médio', high: 'Alto'}[p.thinking] || p.thinking || 'Não definido';
    return `<h3>Escolhas para a próxima tentativa</h3><dl class="routing conn-facts">
      ${fact('Modelo solicitado', requestedModel, h)}${fact('Raciocínio solicitado', requestedThinking, h)}
      ${fact('Catálogo', c.catalogStatus, h)}${fact('Modelo efetivo', 'Não informado; nenhuma inferência real', h)}</dl>
      <ul class="conn-catalog">${c.catalog.map(model => `<li><strong>${h.esc(model.name)}</strong>
        <small>Exemplo de raciocínio: ${h.esc(model.thinking.map(v => v === 'high' ? 'alto' : 'médio').join(', '))}; acesso real não verificado.</small></li>`).join('')}</ul>
      ${c.catalogCompatible ? '<p class="field-hint">Catálogo simulado compatível. Isso não garante acesso real ao modelo.</p>' :
        '<p class="error-note" role="status">Escolha incompatível com o catálogo simulado desta conexão. Ajuste a preferência ou selecione outra conexão para uma nova tentativa. Nenhum fallback para API foi realizado.</p>'}`;
  }

  function sessionsView(c, state, h) {
    const tasks = related(c, state);
    return `<h3>Tentativas vinculadas</h3>${tasks.length ? `<ul class="conn-sessions">${tasks.map(task =>
      `<li><strong>${h.esc(task.id)} · tentativa ${h.esc(task.attempt || 1)}</strong>
        <small>${h.esc(task.title)} · ${h.esc(task.status)} · simulação</small>
        <small>Sessão/contexto próprios; conta e configuração da tentativa preservadas.</small></li>`).join('')}</ul>` :
      '<p class="field-hint">Nenhuma tentativa vinculada nesta demonstração.</p>'}
      <p class="field-hint">Outra sessão nesta conexão pode realizar revisão, com contexto separado. Isso não cria outra conta nem capacidade independente.</p>`;
  }

  function inspect(id, state, h) {
    const c = find(id, state);
    if (!c) { h.notify('Conexão não encontrada na demonstração.'); return false; }
    const identity = c.identity.validation === 'validated-simulated' ? 'Validada somente na simulação' : 'Validação pendente';
    const next = c.lifecycle === 'pending' ? button('Continuar autorização simulada', 'authorize', c.id, h, true) :
      ['expired', 'denied', 'disconnected'].includes(c.lifecycle) ? button('Reconectar na simulação', 'reconnect', c.id, h, true) : '';
    h.showModal(`${c.name} · conexão simulada`, `<div class="conn-inspector">${status(c, h)}
      <dl class="routing conn-facts">${fact('Registro estável', c.id, h)}${fact('Runtime', c.provider, h)}
        ${fact('Identidade', identity, h)}${fact('Identidade ilustrativa', c.identity.label, h)}
        ${fact('E-mail fictício', c.identity.email, h)}${fact('Registro de autorização', c.identity.recordId || 'Ainda não validado', h)}
        ${fact('Modalidade', c.modality, h)}${fact('Workspace', c.workspace, h)}
        ${fact('Grupo de capacidade', c.capacityGroup, h)}${fact('Capacidade', c.capacity, h)}</dl>
      <p class="field-hint">O e-mail pode coincidir entre contas/workspaces; ele não substitui a identidade do registro. Valores acima são exemplos, sem tokens.</p>
      <div class="form-actions conn-actions">${next}${button('Gerenciar uso simulado', 'usage', c.id, h)}</div>
      ${catalogView(c, state, h)}${sessionsView(c, state, h)}
      <details class="conn-scenarios"><summary>Simular estados desta conexão</summary><p class="field-hint">Apenas a elegibilidade de novos trabalhos muda. Nenhuma tentativa existente é retomada, interrompida ou transferida.</p>
        <div class="form-actions conn-actions">${button(c.catalogLoading?'Concluir descoberta simulada':'Simular descoberta em andamento',c.catalogLoading?'finish-discovery':'discovery',c.id,h)}${button('Simular limite', 'limit', c.id, h)}${button('Simular expiração', 'expire', c.id, h)}
          ${button('Simular acesso negado', 'deny', c.id, h)}${button(c.catalogCompatible ? 'Simular catálogo incompatível' : 'Restaurar catálogo simulado', c.catalogCompatible ? 'incompatible' : 'catalog', c.id, h)}</div></details>
      <details class="conn-history"><summary>Histórico de registros e decisões</summary><ol>${c.history.map(entry =>
        `<li>${h.esc(entry.text)} <small>· registro r${h.esc(entry.revision)} · simulação</small></li>`).join('')}</ol></details>
      <h3>Encerramento e confirmação</h3><dl class="routing conn-facts">${fact('Novos trabalhos', c.acceptsNewWork ? 'Permitidos se elegíveis' : 'Bloqueados', h)}
        ${fact('Término de workers', c.workerTermination, h)}${fact('Saída da conta', c.logout, h)}${fact('Revogação remota', c.revocation, h)}</dl>
      <div class="form-actions conn-actions">${c.lifecycle === 'disconnected' ? button('Simular revogação sem resposta', 'revoke-no-signal', c.id, h) :
        button('Desconectar para novos trabalhos', 'disconnect', c.id, h)}</div></div>`);
    return true;
  }

  function addForm(state, h) {
    h.showModal('Adicionar conexão · simulação', `<p>Crie um registro pendente, separado das conexões em uso. Não informe tokens, senhas ou dados de contas reais.</p>
      <form id="conn-add-form" class="conn-form">
        <label class="form-label" for="conn-provider">Runtime de exemplo</label><select id="conn-provider" name="provider"><option>Codex</option><option>Claude Code</option></select>
        <label class="form-label" for="conn-name">Nome para reconhecer a conta</label><input id="conn-name" name="name" required maxlength="70" placeholder="Ex.: minha segunda conta de exemplo">
        <label class="form-label" for="conn-workspace">Workspace de exemplo</label><input id="conn-workspace" name="workspace" required maxlength="70" placeholder="Ex.: pessoal ou equipe fictícia">
        <label class="form-label" for="conn-mode">Origem de autorização e cobrança</label><select id="conn-mode" name="mode"><option value="own-plan">Plano próprio do provedor · simulação</option><option value="api">API com cobrança separada · simulação</option></select>
        <p class="field-hint">A modalidade será confirmada antes da ativação. API é uma escolha separada; limites de assinatura nunca a selecionam automaticamente.</p>
        <p class="error-note" id="conn-form-error" role="alert"></p><div class="form-actions">${button('Cancelar', 'close', null, h)}<button class="button primary" type="submit">Adicionar registro pendente</button></div></form>`);
    return true;
  }

  function authorization(c, state, h) {
    if (!c.pendingRegistration) c.pendingRegistration = {revision: c.identity.revision + 1, mode: c.mode};
    const candidate = c.pendingRegistration;
    const needsConsent = c.consentMode !== candidate.mode || (candidate.mode === 'own-plan' && !c.planConfirmed);
    const confirmation = candidate.mode === 'own-plan' ? 'Confirmo nesta simulação o uso do meu plano próprio pela conexão indicada.' :
      'Confirmo nesta simulação a escolha separada de API, com outra origem de cobrança.';
    h.showModal('Validar autorização · simulação', `<p>Nenhum provedor será acessado. O novo registro só fica disponível depois desta validação simulada. As tentativas antigas conservam o registro e contexto anteriores.</p>
      <div class="snapshot"><strong>${h.esc(c.name)}</strong><br>${h.esc(c.workspace)}<br>${h.esc(modeLabels[candidate.mode])}<br>Registro proposto r${h.esc(candidate.revision)} · fictício</div>
      <form id="conn-confirm-form" class="conn-form"><input type="hidden" name="connectionId" value="${h.esc(c.id)}">
        <label class="form-label" for="conn-identity-result">Resultado da validação de identidade simulada</label><select id="conn-identity-result" name="identityResult"><option value="same">Identidade corresponde à conta indicada</option><option value="different">Identidade diferente — impedir ativação</option></select>
        ${needsConsent ? `<label class="conn-consent"><input type="checkbox" name="confirmMode" value="yes" required> <span>${h.esc(confirmation)}</span></label>` :
          '<p class="field-hint">Esta modalidade já foi confirmada neste registro de demonstração; não é necessário consentir novamente.</p>'}
        <p class="field-hint">Uma validação de exemplo não comprova assinatura, acesso a modelos ou capacidade independente.</p>
        <p class="error-note" id="conn-form-error" role="alert"></p><div class="form-actions">${button('Voltar à conexão', 'inspect', c.id, h)}<button class="button primary" type="submit">Validar registro simulado</button></div></form>`);
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
    if (act === 'add') return addForm(state, h);
    if (act === 'close') { h.closeModal(); return true; }
    const c = find(el.dataset.connId, state);
    if (!c) { h.notify('Conexão não encontrada na demonstração.'); return true; }
    if (act === 'inspect') return inspect(c.id, state, h);
    if (act === 'authorize') return authorization(c, state, h);
    if (act === 'reconnect') {
      c.pendingRegistration = {revision: c.identity.revision + 1, mode: c.mode};
      c.authorizationValid = false;
      change(c, 'pending', 'Nova autorização pendente', 'Registro anterior preservado', h);
      h.render(); return authorization(c, state, h);
    }
    if (act === 'usage') {
      h.showModal('Gerenciar uso · simulação', `<p>${h.esc(c.name)} usa ${h.esc(c.modality)}. A versão real encaminhará ao gerenciamento de uso do provedor correspondente; esta demonstração não acessa esse serviço.</p>
        <div class="snapshot">Capacidade efetiva: desconhecida.<br>Grupo: ${h.esc(c.capacityGroup)}.<br>Mais sessões/hosts não comprovam uma franquia adicional.</div>
        <p>Um limite permite aguardar, ajustar a próxima execução ou escolher outra conexão elegível. O histórico e a origem de cobrança permanecem explícitos.</p>
        <div class="form-actions">${button('Voltar à conexão', 'inspect', c.id, h)}${c.lifecycle === 'limited' ? button('Simular limite liberado', 'clear-limit', c.id, h, true) : ''}</div>`);
      return true;
    }
    if (act === 'disconnect') {
      const active = related(c, state).filter(task => openStatuses.has(task.status));
      h.showModal('Bloquear novos trabalhos · simulação', `<p>A conexão ${h.esc(c.name)} deixará de aceitar novas tentativas. As ${active.length} tentativas pendentes continuam vinculadas à identidade original.</p>
        <p>Esta ação não confirma término dos workers, saída da conta ou revogação no provedor. Cada confirmação precisa de evidência própria.</p>
        <div class="form-actions">${button('Voltar à conexão', 'inspect', c.id, h)}${button('Bloquear novos trabalhos', 'confirm-disconnect', c.id, h, true)}</div>`);
      return true;
    }
    if(act==='discovery'||act==='finish-discovery'){c.catalogLoading=act==='discovery';c.status=c.catalogLoading?'Catálogo em descoberta · simulação':labels[c.lifecycle];h.record(`${c.name} · descoberta de catálogo ${c.catalogLoading?'pendente':'concluída'}`,'Dados fictícios; nenhuma consulta ao provedor.','plug');h.render();inspect(c.id,state,h);return true;}
    if (act === 'confirm-disconnect') {
      c.pendingRegistration = null;
      c.authorizationValid = false;
      change(c, 'disconnected', 'Novos trabalhos bloqueados', 'Sem confirmação de término, logout ou revogação', h);
      c.workerTermination = 'Não confirmado; nenhuma tentativa foi encerrada';
      c.logout = 'Não solicitado'; c.revocation = 'Não solicitada';
    } else if (act === 'revoke-no-signal') {
      if (c.lifecycle !== 'disconnected') return true;
      c.revocation = 'Não confirmada · sem resposta do provedor na simulação';
      c.history.unshift({text: 'Revogação sem confirmação; não tratada como sucesso.', revision: c.identity.revision});
      h.record(`${c.name} · revogação não confirmada`, 'Sem sinal remoto · simulação; workers e histórico preservados.', 'alert');
    } else if (act === 'limit') {
      change(c, 'limited', 'Limite observado', 'Capacidade real desconhecida; sem fallback para API', h);
    } else if (act === 'expire') {
      c.authorizationValid = false;
      change(c, 'expired', 'Autorização expirada', 'Nova autorização necessária antes de novos trabalhos', h);
    } else if (act === 'deny') {
      c.authorizationValid = false;
      change(c, 'denied', 'Acesso negado', 'Sem retry silencioso ou troca de modalidade', h);
    } else if (act === 'clear-limit') {
      if (c.lifecycle !== 'limited') return true;
      const validated = c.authorizationValid && c.identity.validation === 'validated-simulated' && (c.mode !== 'own-plan' || c.planConfirmed);
      change(c, validated ? 'active' : 'pending', 'Limite liberado na simulação', 'Elegibilidade reavaliada; nenhuma tentativa retomada automaticamente', h);
    } else if (act === 'incompatible' || act === 'catalog') {
      c.catalogCompatible = act === 'catalog';
      c.catalogStatus = c.catalogCompatible ? 'Catálogo ilustrativo; acesso real desconhecido' : 'Escolha incompatível · catálogo de exemplo';
      c.history.unshift({text: c.catalogCompatible ? 'Catálogo de exemplo restaurado.' : 'Incompatibilidade identificada; sem fallback pago.', revision: c.identity.revision});
      h.record(`${c.name} · catálogo ${c.catalogCompatible ? 'compatível' : 'incompatível'}`, 'Somente novas escolhas afetadas · simulação; tentativas preservadas.', 'plug');
    } else return false;
    h.render(); inspect(c.id, state, h);
    return true;
  }

  function submit(form, data, state, h) {
    if (form.id === 'conn-add-form') {
      const name = String(data.get('name') || '').trim();
      const workspace = String(data.get('workspace') || '').trim();
      const provider = String(data.get('provider') || '');
      const mode = String(data.get('mode') || '');
      if (!name || name.length > 70 || !workspace || workspace.length > 70 ||
          !['Codex', 'Claude Code'].includes(provider) || !Object.hasOwn(modeLabels, mode)) {
        error('Preencha nome, workspace, runtime e modalidade válidos para o exemplo.', h); return true;
      }
      if ((state.connections || []).some(c => c.name.toLocaleLowerCase('pt-BR') === name.toLocaleLowerCase('pt-BR'))) {
        error('Escolha outro rótulo para distinguir as conexões. Elas podem ter o mesmo e-mail fictício.', h); return true;
      }
      state.connections ||= [];
      let sequence = (state.nextConnectionSequence || 0) + 1;
      while (find(`demo-connection-${sequence}`, state)) sequence++;
      state.nextConnectionSequence = sequence;
      const c = makeConnection(`demo-connection-${sequence}`, name, provider, workspace, `demo-capacity-${sequence}`, false);
      c.mode = mode; c.modality = modeLabels[mode]; c.pendingRegistration.mode = mode;
      state.connections.push(c);
      h.record(`${c.name} · registro pendente adicionado`, 'Sem autenticação real; conexões e tentativas em uso preservadas.', 'plug');
      h.closeModal(); h.render(); h.notify('Registro pendente criado. Valide a autorização simulada antes de selecionar esta conexão.');
      return true;
    }
    if (form.id !== 'conn-confirm-form') return false;
    const c = find(String(data.get('connectionId') || ''), state);
    if (!c || c.lifecycle !== 'pending' || !c.pendingRegistration) {
      error('Este pedido não está mais pendente. Inspecione a conexão e inicie uma nova autorização simulada.', h); return true;
    }
    const candidate = c.pendingRegistration;
    const needsConsent = c.consentMode !== candidate.mode || (candidate.mode === 'own-plan' && !c.planConfirmed);
    if (needsConsent && data.get('confirmMode') !== 'yes') {
      error('Confirme a modalidade indicada para ativar somente este registro simulado.', h); return true;
    }
    if (data.get('identityResult') !== 'same') {
      c.pendingRegistration = null;
      c.authorizationValid = false;
      change(c, 'denied', 'Identidade diferente; ativação impedida', 'Registro anterior e tentativas conservados', h);
      h.closeModal(); h.render(); h.notify('A identidade não corresponde à conta indicada. Nenhuma tentativa foi transferida.');
      return true;
    }
    c.identity = {...c.identity, recordId: `demo-${c.id}-r${candidate.revision}`, revision: candidate.revision, validation: 'validated-simulated'};
    c.mode = candidate.mode; c.modality = modeLabels[c.mode];
    c.authorizationValid = true;
    c.consentMode = c.mode; c.planConfirmed = c.mode === 'own-plan'; c.pendingRegistration = null;
    c.catalogStatus = c.catalogCompatible ? 'Catálogo ilustrativo; acesso real desconhecido' : 'Escolha incompatível · catálogo de exemplo';
    change(c, 'active', `Registro r${c.identity.revision} validado na simulação`, 'Identidade e modalidade verificadas somente no exemplo; sem garantia de cota/acesso', h);
    h.closeModal(); h.render(); h.notify(c.catalogCompatible ? 'Conexão disponível para novas tentativas simuladas. Histórico anterior preservado.' :
      'Registro validado; escolha incompatível ainda impede uma nova tentativa. Nenhum fallback pago.');
    return true;
  }

  window.OrchestrixConnections = Object.freeze({initial, view, inspect, addForm, click, submit, eligible});
})();
