/* Pure, read-only diagnostics. Observations do not authorize execution or apply runtime settings. */
(function (root, factory) {
  'use strict';
  const api = factory();
  if (typeof module === 'object' && module.exports) module.exports = api;
  else root.OrchestrixConnectionDiagnostics = api;
})(typeof globalThis !== 'undefined' ? globalThis : this, function () {
  'use strict';

  const API_PROVIDERS = new Set(['openai', 'anthropic', 'gemini', 'azure', 'bedrock', 'compatible']);
  const RUNTIMES = {'Codex': 'codex-runtime', 'Claude Code': 'claude-code-runtime', 'Antigravity': 'antigravity-runtime'};
  const SUPPORT = ['supported', 'unsupported', 'unknown', 'not-applicable'];
  const CHECK_STATUS = ['passed', 'failed', 'pending', 'blocked', 'skipped', 'unknown'];
  const FEATURE_IDS = ['reasoning', 'context', 'functionCalling', 'streaming', 'nativeWeb', 'structuredOutput', 'fastMode', 'nativeMultiAgent', 'sandbox', 'approval', 'fullAccess', 'usage', 'credits', 'deployment', 'regionProfile', 'generationMethods', 'nativeDelegation', 'orchestrixMultiAgent'];
  const CHECKS = [
    ['config', 'connection', 'configuration', true], ['identity', 'connection', 'metadata', true],
    ['catalog', 'resources', 'catalog', true], ['access', 'resources', 'metadata', true],
    ['reasoning', 'models', 'metadata'], ['context', 'models', 'metadata'], ['functionCalling', 'models', 'metadata'],
    ['streaming', 'models', 'metadata'], ['nativeWeb', 'models', 'metadata'], ['structuredOutput', 'models', 'metadata'],
    ['fastMode', 'runtime', 'metadata'], ['nativeMultiAgent', 'runtime', 'metadata'],
    ['sandbox', 'permissions', 'metadata'], ['approval', 'permissions', 'metadata'], ['fullAccess', 'permissions', 'metadata'],
    ['usage', 'account', 'metadata'], ['credits', 'account', 'metadata'], ['providerSpecific', 'provider', 'metadata'],
    ['nativeDelegationPolicy', 'policy', 'policy'], ['orchestrixMultiAgent', 'policy', 'policy']
  ];
  const CAP_MAP = {reasoning: 'reasoning', context: 'context', functionCalling: 'functions', streaming: 'streaming', nativeWeb: 'nativeWeb', structuredOutput: 'structuredOutput'};
  const ERROR_CODES = new Set(['authentication-required', 'access-denied', 'catalog-unavailable', 'rate-limited', 'provider-unavailable', 'network-error', 'timeout', 'cancelled', 'disconnected', 'adapter-unavailable', 'credentials-missing', 'host-unavailable', 'unavailable', 'failed', 'busy', 'invalid-catalog', 'invalid-response', 'credential-echo', 'endpoint-blocked', 'redirect-blocked', 'response-too-large', 'scope-mismatch', 'revision-mismatch']);
  const SOURCE_KINDS = new Set(['provider-metadata', 'runtime-observation', 'user-declaration', 'configuration', 'policy']);

  function secretsFor(connection, options) {
    return [connection?.apiKey, connection?.credential, connection?.accessToken, connection?.managementToken, ...(Array.isArray(options?.redactValues) ? options.redactValues.slice(0,100) : [])].filter(value => typeof value === 'string' && value.length >= 8 && value.length <= 8192);
  }
  function text(value, secrets, max = 240) {
    if (typeof value !== 'string' || !value.trim() || value.length > max || /[\u0000-\u001f\u007f]/.test(value) || secrets.some(secret => value.includes(secret))) return null;
    return value;
  }
  function code(value, fallback = 'unknown') { return typeof value === 'string' && /^[a-zA-Z0-9_.-]{1,100}$/.test(value) ? value : fallback; }
  function stamp(value, fallback) { const time = typeof value === 'string' && value.length <= 40 ? Date.parse(value) : NaN; return Number.isFinite(time) ? new Date(time).toISOString() : fallback; }
  function revision(connection) { const value = connection?.identity?.revision ?? connection?.revision; return Number.isSafeInteger(value) && value >= 0 ? value : 0; }
  function providerFor(connection) { return connection?.mode === 'api' ? API_PROVIDERS.has(connection.apiProvider) ? connection.apiProvider : 'unknown' : RUNTIMES[connection?.provider] || (Object.values(RUNTIMES).includes(connection?.provider) ? connection.provider : 'unknown'); }
  function url(value, secrets) {
    const raw = text(value, secrets, 500); if (!raw || /[?#]/.test(raw)) return null;
    try { const parsed = new URL(raw); if (parsed.username || parsed.password || !['https:', 'http:'].includes(parsed.protocol)) return null; return parsed.href.replace(/\/$/, ''); } catch { return null; }
  }
  function backendEndpoint(provider, value, region, secrets) {
    const endpoint = url(value, secrets); if (!endpoint) return null;
    const parsed = new URL(endpoint);
    if (provider === 'azure') {
      if (parsed.protocol !== 'https:' || !/^[a-z0-9-]+\.(?:openai\.azure\.com|cognitiveservices\.azure\.com|services\.ai\.azure\.com)$/.test(parsed.hostname) || !['', '/', '/openai/v1'].includes(parsed.pathname)) return null;
      return parsed.origin;
    }
    if (provider === 'bedrock') {
      if (!/^[a-z]{2}(?:-[a-z]+){1,3}-\d$/.test(region || '')) return null;
      const domain = region.startsWith('cn-') ? 'amazonaws.com.cn' : 'amazonaws.com';
      if (parsed.protocol !== 'https:' || ![`bedrock.${region}.${domain}`, `bedrock-runtime.${region}.${domain}`].includes(parsed.hostname) || !['', '/'].includes(parsed.pathname)) return null;
      return `https://bedrock.${region}.${domain}`;
    }
    if (provider === 'openai' && endpoint !== 'https://api.openai.com/v1') return null;
    if (provider === 'anthropic' && endpoint !== 'https://api.anthropic.com/v1') return null;
    if (provider === 'gemini' && !['https://generativelanguage.googleapis.com/v1beta','https://generativelanguage.googleapis.com/v1'].includes(endpoint)) return null;
    if (provider === 'compatible' && parsed.protocol !== 'https:' && !['localhost','127.0.0.1','[::1]'].includes(parsed.hostname)) return null;
    return endpoint;
  }
  function scopeFor(connection, secrets) {
    const provider = providerFor(connection);
    return {
      connectionId: text(connection?.id, secrets, 200), revision: revision(connection), provider,
      mode: connection?.mode === 'api' ? 'api' : 'own-plan',
      endpoint: connection?.mode === 'api' ? url(connection.endpoint, secrets) : null,
      region: text(connection?.region, secrets, 80), deployment: text(connection?.deployment, secrets, 200),
      authMethod: text(connection?.authMethod, secrets, 40) || (connection?.mode === 'api' ? 'api-key' : 'runtime'),
      runtimeVersion: text(connection?.runtimeVersion, secrets, 80)
    };
  }
  function scopeKey(scope) { return JSON.stringify(scope); }
  function matchesScope(candidate, scope, secrets) {
    if (!candidate || candidate.connectionId !== scope.connectionId || candidate.provider !== scope.provider) return false;
    const reportedRevision = candidate.revision ?? candidate.identityRevision;
    if (reportedRevision !== undefined && reportedRevision !== scope.revision) return false;
    if (scope.mode === 'api') {
      const expected = backendEndpoint(scope.provider, scope.endpoint, scope.region, secrets);
      const observed = backendEndpoint(scope.provider, candidate.endpoint, candidate.region ?? scope.region, secrets);
      if (!expected || observed !== expected) return false;
      if ((candidate.region || null) !== scope.region || (candidate.deployment || null) !== scope.deployment) return false;
      if (candidate.authMethod !== undefined && candidate.authMethod !== scope.authMethod) return false;
    } else if (candidate.runtimeVersion && scope.runtimeVersion && candidate.runtimeVersion !== scope.runtimeVersion) return false;
    return true;
  }
  function provenance(source, scope, at, secrets, field) {
    const kind = typeof source === 'string' ? source : source?.kind;
    if (!SOURCE_KINDS.has(kind)) return [];
    const operation = text(source?.operation, secrets, 100);
    const documentation = url(typeof source?.documentation === 'string' ? source.documentation : source?.documentation?.url, secrets);
    return [{kind, scope: {...scope}, observedAt: stamp(at, null), ...(operation ? {operation} : {}), ...(documentation ? {documentation} : {}), ...(field ? {field: code(field)} : {})}];
  }
  function feature(id, support = 'unknown', props = {}) {
    const safeSupport = SUPPORT.includes(support) ? support : 'unknown';
    return {id, support: safeSupport, available: safeSupport === 'unsupported' ? 'unavailable' : safeSupport === 'not-applicable' ? 'not-applicable' : 'unknown', effectivePolicy: null, observed: false, provenance: [], details: {}, ...props};
  }
  function evidenceFor(cap, context, field, allowUnknown = false) {
    if (context.metadataTrusted === false || !cap || cap.observed !== true || !['supported', 'unsupported', ...(allowUnknown ? ['unknown'] : [])].includes(cap.status)) return null;
    if (cap.scope && !matchesScope(cap.scope,context.scope,context.secrets)) return null;
    if (cap.source?.kind && cap.source.kind !== (typeof context.source === 'string' ? context.source : context.source?.kind)) return null;
    if (Array.isArray(cap.evidence) && cap.evidence.some(item => item?.scope && !matchesScope(item.scope, context.scope, context.secrets))) return null;
    return provenance(cap.source || context.source, context.scope, cap.observedAt || context.observedAt, context.secrets, field);
  }
  function telemetryDetails(cap, secrets, depth = 0) {
    const values = cap?.details && typeof cap.details === 'object' && !Array.isArray(cap.details) ? cap.details : cap;
    const details = {};
    for (const field of ['usedPercent','remainingPercent','availablePercent']) if (typeof values?.[field] === 'number' && Number.isFinite(values[field]) && values[field] >= 0 && values[field] <= 100) details[field] = values[field];
    for (const field of ['balance','amount','remaining','spent']) if (typeof values?.[field] === 'number' && Number.isFinite(values[field]) && values[field] >= 0 && values[field] <= Number.MAX_SAFE_INTEGER) details[field] = values[field];
    const currency = text(values?.currency,secrets,3); if (currency && /^[A-Z]{3}$/.test(currency)) details.currency = currency;
    const reset = text(values?.resetAt,secrets,40);
    if (reset && /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,3})?(?:Z|[+-]\d{2}:\d{2})$/.test(reset)) {
      const parsed = stamp(reset,null); const [,year,month,day] = /^(\d{4})-(\d{2})-(\d{2})/.exec(reset);
      const maxDay = new Date(Date.UTC(Number(year),Number(month),0)).getUTCDate();
      if (parsed && Number(month)>=1 && Number(month)<=12 && Number(day)>=1 && Number(day)<=maxDay) details.resetAt = parsed;
    }
    if (depth === 0 && values?.credit && typeof values.credit === 'object' && !Array.isArray(values.credit)) {
      const credit = telemetryDetails({details:values.credit},secrets,depth+1);
      const financial = Object.fromEntries(Object.entries(credit).filter(([key])=>['balance','amount','remaining','spent','currency'].includes(key)));
      if (Object.keys(financial).length) details.credit = financial;
    }
    return details;
  }
  function capability(id, cap, context, field) {
    const telemetry = id === 'usage' || id === 'credits';
    const evidence = evidenceFor(cap, context, field, telemetry); if (!evidence) return feature(id);
    const details = {};
    for (const name of ['inputTokens', 'outputTokens', 'windowTokens']) if (Number.isSafeInteger(cap[name]) && cap[name] > 0) details[name] = cap[name];
    for (const name of ['levels', 'modes', 'modalities']) if (Array.isArray(cap[name])) details[name] = cap[name].slice(0,30).map(value => text(value, context.secrets, 80)).filter(Boolean);
    if (cap.effort) { const effortEvidence = evidenceFor(cap.effort, context, `${field}.effort`); if (effortEvidence) details.effort = cap.effort.status; }
    if (telemetry) {
      Object.assign(details,telemetryDetails(cap,context.secrets));
      if (cap.status === 'unknown' && !Object.keys(details).length) return feature(id);
    }
    return feature(id, cap.status, {observed: true, provenance: evidence, details});
  }
  function observation(id, item, context) { return capability(id, item, context, id); }
  function metadataValue(item, context) {
    if (context && Array.isArray(item?.evidence) && item.evidence.some(entry=>entry?.scope&&!matchesScope(entry.scope,context.scope,context.secrets))) return null;
    return item && typeof item === 'object' && Object.hasOwn(item, 'value') ? item.value : item;
  }
  function normalizeModel(item, context) {
    const id = text(item?.id, context.secrets, 200), name = text(item?.name || item?.id, context.secrets, 240);
    if (!id || !name || item.scope && !matchesScope(item.scope, context.scope, context.secrets)) return null;
    const itemSourceKind = typeof item.source === 'string' ? item.source : item.source?.kind;
    context = {...context, metadataTrusted: !itemSourceKind || itemSourceKind === (typeof context.source === 'string' ? context.source : context.source?.kind)};
    const features = Object.fromEntries(Object.entries(CAP_MAP).map(([featureId, key]) => [featureId, capability(featureId, item.capabilities?.[key] || (key === 'functions' ? item.capabilities?.tools : key === 'nativeWeb' ? item.capabilities?.web : null), context, `capabilities.${key}`)]));
    const tiers = metadataValue(item.serviceTiers, context);
    // The pinned ModelServiceTier contract exposes IDs; display names are not capability evidence.
    const safeTiers = Array.isArray(tiers) ? tiers.slice(0,30).map(value => text(value && typeof value === 'object' ? value.id : null, context.secrets, 80)).filter(Boolean) : null;
    const defaultTier = text(metadataValue(item.defaultServiceTier, context), context.secrets, 80);
    features.fastMode = feature('fastMode');
    if (context.metadataTrusted && context.scope.mode !== 'api' && context.runtimeBound && safeTiers) {
      features.fastMode = feature('fastMode', safeTiers.some(value => /^(fast|ultrafast)$/i.test(value)) ? 'supported' : 'unknown', {observed: true, provenance: provenance(context.source, context.scope, context.observedAt, context.secrets, 'serviceTiers'), details: {serviceTiers: safeTiers, defaultServiceTier: defaultTier}});
    }
    const multi = metadataValue(item.multiAgentVersion, context);
    features.nativeMultiAgent = feature('nativeMultiAgent');
    if (context.metadataTrusted && context.scope.mode !== 'api' && context.runtimeBound && ['disabled', 'v1', 'v2'].includes(multi)) features.nativeMultiAgent = feature('nativeMultiAgent', multi === 'disabled' ? 'unsupported' : 'supported', {observed: true, provenance: provenance(context.source, context.scope, context.observedAt, context.secrets, 'multiAgentVersion'), details: {version: multi}});
    const access = observation('access', item.access, context);
    const kind = ['model', 'deployment', 'inference-profile'].includes(item.kind) ? item.kind : 'model';
    return {id, name, kind, scope: {...context.scope, resourceId: id}, available: access.support === 'supported' ? 'available' : access.support === 'unsupported' ? 'unavailable' : 'unknown', access, features, methods: Array.isArray(item.methods) ? item.methods.slice(0,30).map(value => text(value, context.secrets, 80)).filter(Boolean) : []};
  }
  function aggregate(id, models) {
    const values = models.map(model => model.features[id]).filter(Boolean);
    if (!values.length) return feature(id);
    const counts = {supported: 0, unsupported: 0, unknown: 0, 'not-applicable': 0}; values.forEach(value => counts[value.support]++);
    const support = counts.supported ? 'supported' : counts.unsupported === values.length ? 'unsupported' : 'unknown';
    const levels = [...new Set(values.flatMap(value => value.details.levels || []))];
    const serviceTiers = [...new Set(values.flatMap(value => value.details.serviceTiers || []))];
    const versions = [...new Set(values.map(value => value.details.version).filter(Boolean))];
    return feature(id, support, {observed: values.some(value => value.observed), counts, provenance: values.flatMap(value => value.provenance).slice(0,8), details: {...(levels.length ? {levels} : {}),...(serviceTiers.length ? {serviceTiers} : {}),...(versions.length ? {versions} : {}), coverage: counts.supported === values.length ? 'all-observed-resources' : counts.supported ? 'some-resources' : 'unknown'}});
  }
  function policy() {
    return {nativeDelegation: {effective: 'disabled', recommended: true, observedRuntimeState: 'unknown', applyStatus: 'not-applied', observationStatus:'unknown', conflict:false}, orchestrixMultiAgent: {status: 'planned', ownership: 'orchestrator', nativeDelegationRequired: false}, hostPermissions: {effective: 'requires-user-grant', authority: 'user', observedGrant: 'unknown'}};
  }
  function providerHints(provider) {
    return provider === 'azure' ? ['deployment', 'resource-model-catalog', 'management-deployments-separate'] : provider === 'bedrock' ? ['region', 'foundation-models', 'inference-profiles', 'aws-auth-method'] : provider === 'gemini' ? ['developer-api-generation-methods', 'not-antigravity-runtime'] : provider === 'antigravity-runtime' ? ['official-runtime-adapter-required', 'not-gemini-developer-api'] : provider === 'compatible' ? ['explicit-endpoint', 'protocol-compatibility-unverified'] : provider === 'codex-runtime' ? ['model-list', 'provider-capabilities', 'read-config-only', 'reasoning-tiers-native-multi-independent'] : provider === 'claude-code-runtime' ? ['official-runtime-identity', 'subscription-not-api'] : ['official-catalog-only'];
  }
  function plan(connection) {
    const scope = scopeFor(connection, secretsFor(connection));
    return {schemaVersion: 1, scope, scopeKey: scopeKey(scope), revision: scope.revision, steps: CHECKS.map(([id, group, operation, required]) => ({id, group, operation, required: Boolean(required), providerSpecific: id === 'providerSpecific' ? providerHints(scope.provider) : [], network: false})), policy: policy()};
  }
  function validConfiguration(connection, scope, secrets) {
    if (!scope.connectionId || scope.provider === 'unknown' || !['api', 'own-plan'].includes(connection?.mode)) return false;
    if (scope.mode !== 'api') return true;
    if (!backendEndpoint(scope.provider, scope.endpoint, scope.region, secrets)) return false;
    if (scope.provider === 'azure' && !/^[A-Za-z0-9][A-Za-z0-9_.-]{0,119}$/.test(scope.deployment || '')) return false;
    if (scope.provider === 'bedrock' && (!/^[a-z]{2}(?:-[a-z]+){1,3}-\d$/.test(scope.region || '') || !['api-key', 'aws-profile'].includes(scope.authMethod))) return false;
    return true;
  }
  function checkForFeature(id, item, runtimeBlocked) {
    return {id, group: CHECKS.find(value => value[0] === id)?.[1] || 'models', status: item.support === 'not-applicable' ? 'skipped' : item.observed ? 'passed' : runtimeBlocked ? 'blocked' : 'unknown', code: item.support === 'not-applicable' ? 'not-applicable' : item.observed ? item.support === 'unsupported' ? 'observed-unsupported' : 'metadata-observed' : runtimeBlocked ? 'runtime-identity-required' : 'not-reported', feature: id, provenance: item.provenance, details: item.details};
  }
  function summarize(report) {
    const counts = Object.fromEntries(CHECK_STATUS.map(status => [status,0])); (report?.checks || []).forEach(check => {if (Object.hasOwn(counts, check.status)) counts[check.status]++;});
    const features = Object.fromEntries(SUPPORT.map(status => [status,0])); Object.values(report?.features || {}).forEach(item => {if (Object.hasOwn(features, item.support)) features[item.support]++;});
    const resources = {available: 0, unavailable: 0, unknown: 0}; (report?.models || []).forEach(model => resources[Object.hasOwn(resources, model.available) ? model.available : 'unknown']++);
    return {counts, features, resources, totalChecks: (report?.checks || []).length, modelCount: (report?.models || []).length};
  }
  function buildReport(connection, catalogResult, options = {}) {
    const secrets = secretsFor(connection, options), scope = scopeFor(connection, secrets), now = new Date().toISOString();
    const phase = ['queued','running','cancelled','failed'].includes(options.phase) ? options.phase : 'completed';
    const startedAt = stamp(options.startedAt, now), completedAt = ['queued','running'].includes(phase) ? null : stamp(options.completedAt, now);
    const features = Object.fromEntries(FEATURE_IDS.map(id => [id, feature(id)]));
    const report = {schemaVersion: 1, scope, scopeKey: scopeKey(scope), revision: scope.revision, runId: text(options.runId, secrets, 200), phase, status: 'partial', startedAt, completedAt, checks: [], features, models: [], policy: policy()};
    const add = (id, status, codeValue, details = {}, evidence = []) => report.checks.push({id, group: CHECKS.find(value => value[0] === id)?.[1] || 'connection', status, code: codeValue, provenance: evidence, details});
    const configValid = validConfiguration(connection, scope, secrets);
    add('config', configValid ? 'passed' : 'failed', configValid ? 'configuration-valid' : 'invalid-configuration', {}, provenance('configuration', scope, startedAt, secrets));
    const runtime = options.runtimeObservation;
    const runtimeBound = scope.mode !== 'api' && runtime?.identityBound === true && runtime.source?.kind === 'runtime-observation' && matchesScope(runtime.scope, scope, secrets) && (runtime.scope.revision ?? runtime.scope.identityRevision) === scope.revision;
    add('identity', runtimeBound ? 'passed' : scope.mode === 'api' ? 'unknown' : 'blocked', runtimeBound ? 'runtime-identity-bound' : scope.mode === 'api' ? 'inference-identity-not-verified' : 'runtime-identity-required', {}, runtimeBound ? provenance(runtime.source, scope, runtime.observedAt, secrets) : []);
    const sourceKind = typeof catalogResult?.source === 'string' ? catalogResult.source : catalogResult?.source?.kind;
    const catalogSourceValid = scope.mode === 'api' ? sourceKind === 'provider-metadata' : sourceKind === 'runtime-observation' && runtimeBound;
    const catalogScoped = matchesScope(catalogResult?.scope, scope, secrets);
    const catalogValid = configValid && catalogResult?.schemaVersion === 1 && catalogSourceValid && catalogScoped && Array.isArray(catalogResult.models) && catalogResult.models.length <= 500;
    const context = {scope, secrets, source: catalogResult?.source, observedAt: catalogResult?.observedAt, runtimeBound};
    let errorCode = ERROR_CODES.has(options.errorCode || options.error?.code) ? options.errorCode || options.error.code : phase === 'failed' ? 'failed' : null;
    if (catalogValid) {
      const seen = new Set();
      for (const item of catalogResult.models) {
        const model = normalizeModel(item, context);
        if (!model) {errorCode = 'invalid-catalog'; report.models = []; break;}
        const key = `${model.kind}:${model.id}`; if (!seen.has(key)) {seen.add(key);report.models.push(model);}
      }
    }
    const pending = ['queued','running'].includes(phase);
    const catalogObserved = catalogValid && !errorCode && !pending && phase !== 'cancelled' && !['unavailable','failed'].includes(catalogResult.status);
    const blockedError = ['host-unavailable','unavailable','disconnected','adapter-unavailable','credentials-missing'].includes(errorCode) || errorCode === 'authentication-required' && scope.mode !== 'api' && !runtimeBound && options.attempted !== true;
    add('catalog', pending ? 'pending' : phase === 'cancelled' ? 'skipped' : errorCode ? blockedError ? 'blocked' : 'failed' : catalogObserved ? 'passed' : 'blocked', pending ? 'catalog-pending' : phase === 'cancelled' ? 'cancelled' : errorCode || (catalogObserved ? catalogResult.status === 'partial' ? 'partial-catalog-observed' : 'catalog-observed' : !catalogScoped && catalogResult ? 'catalog-scope-unbound' : 'catalog-adapter-required'), {resourceCount: catalogObserved ? report.models.length : 0}, catalogObserved ? provenance(catalogResult.source, scope, catalogResult.observedAt, secrets) : []);
    if (!catalogObserved) report.models = [];
    for (const id of Object.keys(CAP_MAP).concat(['fastMode','nativeMultiAgent'])) features[id] = aggregate(id, report.models);
    const runtimeContext = {scope, secrets, source: runtime?.source, observedAt: runtime?.observedAt, runtimeBound};
    const access = catalogObserved ? observation('access', catalogResult.access, context) : feature('access');
    add('access', access.observed ? 'passed' : runtimeBound || scope.mode === 'api' ? 'unknown' : 'blocked', access.observed ? access.support === 'supported' ? 'access-observed' : 'access-unavailable' : 'execution-access-not-verified', {}, access.provenance);
    if (runtimeBound && !pending && phase !== 'cancelled' && !errorCode) {
      for (const id of ['sandbox','approval','usage','credits']) features[id] = observation(id, runtime.features?.[id], runtimeContext);
      const configuration = runtime.config;
      const sandbox = ['read-only','workspace-write','danger-full-access'].includes(configuration?.sandbox_mode) ? configuration.sandbox_mode : null;
      const approval = ['untrusted','on-failure','on-request','never'].includes(configuration?.approval_policy) ? configuration.approval_policy : null;
      if (sandbox) features.sandbox = feature('sandbox','supported',{observed:true,provenance:provenance(runtime.source,scope,runtime.observedAt,secrets,'sandbox_mode'),details:{currentMode:sandbox,executionGrantVerified:false}});
      if (approval) features.approval = feature('approval','supported',{observed:true,provenance:provenance(runtime.source,scope,runtime.observedAt,secrets,'approval_policy'),details:{currentPolicy:approval,executionGrantVerified:false}});
      if (sandbox && approval) features.fullAccess = feature('fullAccess',sandbox==='danger-full-access'&&approval==='never'?'supported':'unknown',{observed:true,provenance:provenance(runtime.source,scope,runtime.observedAt,secrets,'execution-policy'),details:{currentConfigurationAllowsFullAccess:sandbox==='danger-full-access'&&approval==='never',executionGrantVerified:false}});
      const nativeState = typeof runtime.nativeDelegationEnabled === 'boolean' ? runtime.nativeDelegationEnabled : configuration?.agents?.enabled;
      if (typeof nativeState === 'boolean') {report.policy.nativeDelegation.observedRuntimeState = nativeState ? 'enabled' : 'disabled';report.policy.nativeDelegation.observationStatus = 'observed';report.policy.nativeDelegation.conflict = nativeState;}
      const providerCaps = catalogResult?.providerCapabilities;
      if (providerCaps?.scope && matchesScope(providerCaps.scope, scope, secrets) && providerCaps.webSearch?.observed === true && features.nativeWeb.support === 'unknown') {
        features.nativeWeb = capability('nativeWeb', providerCaps.webSearch, runtimeContext, 'providerCapabilities.webSearch');
        features.nativeWeb.details = {...features.nativeWeb.details,scope:'runtime-provider',perModelSupport:'unknown'};
      }
    } else if (scope.mode === 'api') {
      for (const id of ['sandbox','approval','fullAccess','nativeMultiAgent']) features[id] = feature(id, 'not-applicable', {effectivePolicy: id === 'nativeMultiAgent' ? 'disabled' : 'requires-user-grant'});
    }
    for (const id of ['sandbox','approval','fullAccess']) features[id].effectivePolicy = 'requires-user-grant';
    if (scope.mode === 'api' && catalogObserved) {for (const id of ['usage','credits']) features[id] = observation(id, catalogResult.features?.[id], context);}
    if (scope.provider === 'azure') {
      const deployment = report.models.find(model => model.kind === 'deployment' && model.id === scope.deployment);
      const listed = Boolean(deployment && sourceKind === 'provider-metadata' && catalogResult.models.some(model => model.kind === 'deployment' && model.id === scope.deployment && model.source?.kind === 'provider-metadata'));
      features.deployment = feature('deployment', listed ? 'supported' : 'unknown', {observed: listed, provenance: listed ? provenance(catalogResult.source,scope,catalogResult.observedAt,secrets,'deployment') : [], details: {configured: Boolean(scope.deployment), managementCatalogSeparate: true,inferenceAccessVerified:false}});
    } else features.deployment = feature('deployment','not-applicable');
    if (scope.provider === 'bedrock') features.regionProfile = feature('regionProfile','unknown',{details:{regionConfigured:Boolean(scope.region),authMethod:scope.authMethod,profileCatalogObserved:catalogObserved&&report.models.some(model=>model.kind==='inference-profile'),executionAccessVerified:false}});
    else features.regionProfile = feature('regionProfile','not-applicable');
    if (scope.provider === 'gemini') {const methods = [...new Set(report.models.flatMap(model => model.methods))];features.generationMethods = feature('generationMethods', methods.length ? 'supported' : 'unknown', {observed: methods.length > 0, provenance: methods.length ? provenance(catalogResult.source, scope, catalogResult.observedAt, secrets, 'supportedGenerationMethods') : [], details: {methods, antigravityRuntime: false}});}
    else features.generationMethods = feature('generationMethods','not-applicable');
    features.nativeDelegation = feature('nativeDelegation', features.nativeMultiAgent.support, {effectivePolicy:'disabled',observed:features.nativeMultiAgent.observed,provenance:features.nativeMultiAgent.provenance,details:{runtimeState:report.policy.nativeDelegation.observedRuntimeState,policyAppliedToRuntime:false}});
    features.orchestrixMultiAgent = feature('orchestrixMultiAgent','unknown',{effectivePolicy:'orchestrator-managed',details:{implementation:'planned',nativeDelegationRequired:false}});
    for (const id of Object.keys(CAP_MAP).concat(['fastMode','nativeMultiAgent','sandbox','approval','fullAccess','usage','credits'])) report.checks.push(checkForFeature(id, features[id], scope.mode !== 'api' && !runtimeBound));
    const special = scope.provider === 'azure' ? features.deployment : scope.provider === 'bedrock' ? features.regionProfile : scope.provider === 'gemini' ? features.generationMethods : feature('providerSpecific');
    add('providerSpecific', special.observed ? 'passed' : 'unknown', special.observed ? 'provider-metadata-observed' : 'provider-specific-access-not-verified', {hints:providerHints(scope.provider)}, special.provenance);
    add('nativeDelegationPolicy','passed','native-delegation-disabled-by-orchestrix',{recommended:true,runtimeState:report.policy.nativeDelegation.observedRuntimeState,runtimeSettingChanged:false},provenance('policy',scope,startedAt,secrets));
    add('orchestrixMultiAgent','skipped','orchestrator-planned',{nativeDelegationRequired:false},provenance('policy',scope,startedAt,secrets));
    report.summary = summarize(report);
    report.status = pending ? phase === 'running' ? 'running' : 'pending' : phase === 'cancelled' ? 'cancelled' : report.summary.counts.failed ? 'failed' : report.checks.find(check=>check.id==='catalog')?.status==='blocked' ? 'blocked' : report.summary.counts.unknown || report.summary.counts.blocked || catalogResult?.status==='partial' ? 'partial' : 'complete';
    return report;
  }
  function isCurrent(report, connection, catalogResult) {
    if (!report || report.schemaVersion !== 1 || connection?.lifecycle === 'disconnected') return false;
    const secrets = secretsFor(connection), scope = scopeFor(connection,secrets);
    return report.scopeKey === scopeKey(scope) && report.revision === scope.revision && (!catalogResult || matchesScope(catalogResult.scope,scope,secrets));
  }
  return Object.freeze({plan,buildReport,isCurrent,summarize});
});
