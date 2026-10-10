/* Observed resource metadata. Catalog visibility and execution access are separate. */
(() => {
  'use strict';
  const {html,text,t}=window.OrchestrixI18n;
  const phases={loading:'Descobrindo recursos...',observed:'Catálogo atualizado',partial:'Catálogo parcial',unavailable:'Catálogo não disponível',failed:'Falha na descoberta',timeout:'Consulta expirada',cancelled:'Consulta cancelada','host-unavailable':'Serviço de descoberta indisponível','authentication-required':'Autenticação necessária','access-denied':'Permissão de catálogo necessária','rate-limited':'Aguarde para consultar novamente'};
  const labels={supported:'Compatível',unsupported:'Incompatível',unknown:'A verificar'};
  const limits={'aws-signer-required':'A descoberta com perfil AWS requer o adaptador de autenticação AWS do aplicativo.','azure-deployment-unverified':'O deployment informado ainda precisa ser verificado com as permissões do recurso Azure.','bedrock-profile-catalog-unavailable':'A listagem de perfis de inferência não está disponível para esta credencial.','model-limit-reached':'O catálogo atingiu o limite desta consulta; a listagem está incompleta.','page-limit-reached':'Há mais páginas no provedor do que o limite desta consulta.','undocumented-pagination':'O provedor retornou paginação sem um contrato compatível; a listagem pode estar incompleta.'};
  function summary(c) {
    const phase=c.discovery?.status||'unavailable',count=(c.catalog||[]).length;
    return ['observed','partial'].includes(phase)?`${t(phases[phase])} · ${count} ${t(count===1?'recurso':'recursos')}`:t(phases[phase]||phases.unavailable);
  }
  function capability(label,cap,h,key) {
    const status=['supported','unsupported','unknown'].includes(cap?.status)?cap.status:'unknown';
    let detail=t({supported:'Disponível',unsupported:'Não disponível',unknown:'Não informado'}[status]);
    if(cap?.modalities?.length)detail=cap.modalities.join(', ');
    if(cap?.levels?.length)detail=cap.levels.join(', ');
    else if(cap?.modes?.length)detail=cap.modes.join(', ');
    const tokenFields=[['windowTokens','janela'],['inputTokens','entrada'],['outputTokens','saída']].filter(([key])=>cap?.[key]);
    if(tokenFields.length)detail=tokenFields.map(([key,name])=>text`${new Intl.NumberFormat(window.OrchestrixI18n.language).format(cap[key])} tokens (${t(name)})`).join(' · ');
    return html`<div class="resource-capability" data-capability="${key}"><dt>${t(label)}</dt><dd><span class="resource-value resource-${status}">${h.esc(detail)}</span><small>${cap?.observed?t('Informado pelo provedor'):cap?.evidence?.length?t('Regra documentada'):t('Sem metadados')}</small></dd></div>`;
  }
  function modelView(model,h) {
    const status=model.compatibility?.status||'unknown',caps=model.capabilities||{};
    return html`<details class="conn-resource" data-resource-model="${h.esc(model.id)}"><summary><span><strong>${h.esc(model.name)}</strong><small>${h.esc(model.id)}</small></span><span class="resource-badge resource-${status}">${t(labels[status]||labels.unknown)}</span></summary><div class="resource-details"><dl class="resource-capabilities">${[['Entrada','input'],['Saída','output'],['Contexto','context'],['Raciocínio','reasoning'],['Streaming','streaming'],['Chamadas de ferramentas','functions'],['Busca web nativa','nativeWeb'],['Ferramentas do Orchestrix','hostTools'],['Saída estruturada','structuredOutput']].map(([label,key])=>capability(label,caps[key],h,key)).join('')}</dl>${caps.reasoning?.levels?.length||model.thinking?.length?html`<p class="field-hint">${h.esc(text`Raciocínio informado pelo provedor: ${(caps.reasoning?.levels||model.thinking).join(', ')}`)}</p>`:html`<p class="field-hint">Capacidades de raciocínio não informadas.</p>`}${model.methods?.length?html`<p class="field-hint">${t('Operações do modelo')}: ${h.esc(model.methods.join(', '))}</p>`:''}<div class="resource-provenance"><strong>Origem dos recursos</strong><span>${h.esc(model.source?.operation||t('Adapter da conexão'))}</span>${model.baseModel?html`<span>${h.esc(model.baseModel.name)} ${h.esc(model.baseModel.version||'')}</span>`:''}<span>${h.esc(model.compatibility?.reason||t('Compatibilidade ainda não verificada'))}</span></div></div></details>`;
  }
  function view(c,h,button) {
    const phase=c.discovery?.status||'unavailable';
    const messages={loading:'Consultando os modelos disponíveis nesta conexão.',unavailable:'O runtime ainda não forneceu um catálogo para esta conexão.',failed:'Não foi possível consultar os modelos. Revise a conexão e tente novamente.',timeout:'A consulta demorou mais que o esperado. Tente novamente.',cancelled:'Consulta cancelada. Nenhum resultado tardio será usado.','host-unavailable':'Abra o aplicativo pelo servidor para consultar os recursos do provedor.','authentication-required':'Revise a credencial para consultar o catálogo do provedor.','access-denied':'Esta credencial não tem permissão para listar recursos.','rate-limited':'O provedor limitou a consulta de catálogo. Aguarde e tente novamente.'};
    const observed=['observed','partial'].includes(phase);
    const date=c.discovery?.observedAt&&new Date(c.discovery.observedAt);
    const time=date&&Number.isFinite(date.getTime())?new Intl.DateTimeFormat(window.OrchestrixI18n.language,{dateStyle:'medium',timeStyle:'short'}).format(date):null;
    const counts={supported:0,unsupported:0,unknown:0};for(const model of c.catalog||[])counts[model.compatibility?.status||'unknown']++;
    return html`<section id="connection-resources" class="conn-observed-catalog"><header class="resource-heading"><div><h3>Modelos e recursos</h3><p class="field-hint" role="status" aria-live="polite">${h.esc(summary(c))}</p></div>${phase==='loading'?html`<span class="resource-spinner" aria-hidden="true"></span>`:''}</header>${messages[phase]?html`<p class="field-hint">${t(messages[phase])}</p>`:''}${observed?html`<div class="resource-counts">${Object.entries(counts).map(([status,count])=>html`<span class="resource-badge resource-${status}">${count} ${t(labels[status])}</span>`).join('')}</div>${time?html`<p class="field-hint resource-updated">${t('Última consulta')}: ${h.esc(time)} · ${h.esc(c.discovery.source?.operation||t('Adapter da conexão'))}</p>`:''}${c.catalog.length?html`<div class="conn-resource-list conn-catalog">${c.catalog.map(model=>modelView(model,h)).join('')}</div>`:html`<p class="field-hint">O provedor não informou modelos nesta consulta.</p>`}`:''}${(c.discovery?.limitations||[]).map(item=>html`<p class="resource-limitation">${t(limits[item.code]||'O provedor não expõe todos os recursos nesta consulta.')} <code>${h.esc(item.code)}</code></p>`).join('')}${c.mode==='api'?html`<div class="form-actions">${phase==='loading'?button(t('Cancelar consulta'),'cancel-catalog',c.id,h):button(t('Consultar modelos'),'retry-catalog',c.id,h)}</div>`:''}</section>`;
  }
  window.OrchestrixResources=Object.freeze({summary,view});
})();
