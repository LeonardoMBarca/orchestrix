#!/usr/bin/env node
// Offline protocol fixture. No providers, credentials, filesystem writes or command execution.
import { createInterface } from 'node:readline';
import { resolve } from 'node:path';

const scenarios = new Set([
  'success', 'permission', 'auth-failure', 'rate-limit', 'malformed', 'crash',
  'hang', 'split-utf8', 'noisy-stderr', 'unknown-notification', 'slow',
]);
let scenario = 'success';
const args = process.argv.slice(2);
for (let index = 0; index < args.length; index += 1) {
  const argument = args[index];
  if (argument === '--help') {
    process.stderr.write(`Synthetic app-server fixture. Usage: node fake-app-server.mjs --scenario <${[...scenarios].join('|')}>\n`);
    process.exit(0);
  }
  if (argument === '--scenario') scenario = args[++index];
  else if (argument.startsWith('--scenario=')) scenario = argument.slice(11);
  else {
    process.stderr.write(`Unknown fixture argument: ${argument}\n`);
    process.exit(2);
  }
}
if (!scenarios.has(scenario)) {
  process.stderr.write(`Unknown fixture scenario: ${String(scenario)}\n`);
  process.exit(2);
}

const threads = new Map();
const threadConfigurations = new Map();
const turns = new Map();
const approvals = new Map();
let threadSequence = 0;
let turnSequence = 0;
let initialized = false;
let outputQueue = Promise.resolve();
let inputQueue = Promise.resolve();
const idFor = (kind, sequence) => `${kind}-fake-${String(sequence).padStart(3, '0')}`;
const delay = milliseconds => new Promise(resolveDelay => setTimeout(resolveDelay, milliseconds));

function fatal(error) {
  process.stderr.write(`Synthetic fixture failed: ${error.message}\n`);
  process.exitCode = 1;
  stopTimers();
  process.stdin.destroy();
}
process.stdout.on('error', error => {
  if (error.code === 'EPIPE') process.exit(0);
  fatal(error);
});
process.stderr.on('error', () => process.exit(1));

function write(stream, bytes) {
  return new Promise((resolveWrite, rejectWrite) => {
    stream.write(bytes, error => error ? rejectWrite(error) : resolveWrite());
  });
}

function enqueue(bytes) {
  outputQueue = outputQueue.then(async () => {
    if (scenario !== 'split-utf8') {
      await write(process.stdout, bytes);
      return;
    }
    const emojiOffset = bytes.indexOf(Buffer.from('🧩'));
    const boundaries = [...new Set([
      0, Math.min(7, bytes.length),
      ...(emojiOffset >= 0 ? [emojiOffset + 1, emojiOffset + 2] : []),
      Math.max(0, bytes.length - 1), bytes.length,
    ])].sort((left, right) => left - right);
    for (let index = 1; index < boundaries.length; index += 1) {
      await write(process.stdout, bytes.subarray(boundaries[index - 1], boundaries[index]));
      if (index < boundaries.length - 1) await delay(3);
    }
  });
  outputQueue.catch(fatal);
  return outputQueue;
}

// Codex app-server envelopes do not require a "jsonrpc" field. Accept it on input.
function send(message) {
  return enqueue(Buffer.from(`${JSON.stringify(message)}${scenario === 'split-utf8' ? '\r\n' : '\n'}`, 'utf8'));
}
const result = (id, value) => send({ id, result: value });
const rpcError = (id, code, message, data) => send({ id, error: { code, message, ...(data ? { data } : {}) } });
const notification = (method, params) => send({ method, params });

function descriptor(thread) {
  const configuration = threadConfigurations.get(thread.id);
  return {
    thread, model: configuration.model, modelProvider: 'openai', cwd: thread.cwd,
    reasoningEffort: configuration.effort,
    approvalPolicy: configuration.approvalPolicy, approvalsReviewer: configuration.approvalsReviewer,
    sandbox: { type: 'readOnly', networkAccess: false },
  };
}

function stopTurnTimers(turn) {
  if (turn.timer) clearTimeout(turn.timer);
  if (turn.keepAlive) clearInterval(turn.keepAlive);
  turn.timer = null;
  turn.keepAlive = null;
  if (turn.approvalId) approvals.delete(turn.approvalId);
}
function stopTimers() {
  for (const turn of turns.values()) stopTurnTimers(turn);
}

async function finishTurn(turn, status, text, error = null) {
  if (turn.value.status !== 'inProgress') return;
  stopTurnTimers(turn);
  turn.value.status = status;
  turn.value.error = error;
  threads.get(turn.threadId).status = { type: 'idle' };
  if (text) {
    await notification('item/agentMessage/delta', {
      threadId: turn.threadId, turnId: turn.value.id, itemId: turn.itemId, delta: text,
    });
  }
  await notification('turn/completed', { threadId: turn.threadId, turn: turn.value });
}

async function beginScenario(turn) {
  if (turn.value.status !== 'inProgress') return;
  if (scenario === 'permission') {
    turn.approvalId = idFor('approval', turn.sequence);
    approvals.set(turn.approvalId, turn);
    turn.keepAlive = setInterval(() => {}, 60_000);
    await send({
      id: turn.approvalId, method: 'item/commandExecution/requestApproval',
      params: {
        threadId: turn.threadId, turnId: turn.value.id, itemId: turn.itemId,
        startedAtMs: turn.sequence * 1_000,
        command: '[synthetic fixture command — never executable or executed]',
        cwd: threads.get(turn.threadId).cwd,
        reason: 'Synthetic approval request. Reply with decision: decline; no command runs.',
      },
    });
    return;
  }
  if (scenario === 'crash') {
    await write(process.stderr, 'Synthetic crash after turn/started. No provider or command ran.\n');
    await outputQueue;
    process.exit(23);
  }
  if (scenario === 'malformed') {
    await enqueue(Buffer.from('{ malformed synthetic fixture line }\n', 'utf8'));
  }
  if (scenario === 'unknown-notification') {
    await notification('fixture/unknownNotification', { synthetic: true, threadId: turn.threadId });
  }
  if (scenario === 'noisy-stderr') {
    for (let index = 0; index < 128; index += 1) {
      await write(process.stderr, `SYNTHETIC STDERR ${index}: ${'x'.repeat(8_192)}\n`);
    }
  }
  if (scenario === 'hang' || scenario === 'slow') {
    if (scenario === 'hang') turn.keepAlive = setInterval(() => {}, 60_000);
    await notification('item/agentMessage/delta', {
      threadId: turn.threadId, turnId: turn.value.id, itemId: turn.itemId,
      delta: `Synthetic ${scenario} turn; awaiting interrupt or fixture timer. No work executes.`,
    });
    if (turn.value.status !== 'inProgress' || scenario === 'hang') return;
    turn.timer = setTimeout(() => {
      finishTurn(turn, 'completed', 'Synthetic slow result; no files or providers were accessed.').catch(fatal);
    }, 10_000);
    return;
  }
  if (turn.value.status !== 'inProgress') return;
  turn.timer = setTimeout(() => {
    finishTurn(turn, 'completed', scenario === 'split-utf8'
      ? 'Olá 🧩 — café: resultado sintético, sem execução real.\nSegunda linha no mesmo delta.'
      : 'Synthetic success: no command, provider, credential or repository was accessed.').catch(fatal);
  }, 25);
}

async function handleApproval(message) {
  const turn = approvals.get(String(message.id));
  if (!turn) return;
  approvals.delete(turn.approvalId);
  if (message.result?.decision === 'decline') {
    await finishTurn(turn, 'completed', 'Synthetic approval declined; the agent continues without executing a command.');
  } else if (message.result?.decision === 'cancel') {
    await finishTurn(turn, 'interrupted', 'Synthetic approval cancelled; no command was executed.');
  } else {
    await finishTurn(turn, 'failed', null, {
      message: 'Synthetic fixture supports only decline/cancel for this approval; no command executes.',
      codexErrorInfo: null, additionalDetails: null,
    });
  }
}

async function handleMessage(message) {
  if (!message || typeof message !== 'object' || Array.isArray(message)) {
    await rpcError(null, -32600, 'Invalid request received by synthetic fixture');
    return;
  }
  if (typeof message.method !== 'string') {
    if (Object.hasOwn(message, 'id')) await handleApproval(message);
    return;
  }
  const { id, method } = message;
  const params = message.params ?? {};
  if (!Object.hasOwn(message, 'id')) {
    // initialized is a client notification. Unknown client notifications are ignored.
    return;
  }
  if (method === 'initialize') {
    initialized = true;
    await result(id, {
      userAgent: 'orchestrix-synthetic-app-server/1', platformFamily: 'windows', platformOs: 'windows',
      codexHome: resolve(process.cwd(), '.synthetic-codex-home-not-created'),
    });
    return;
  }
  if (!initialized) {
    await rpcError(id, -32000, 'Synthetic fixture requires initialize before requests');
    return;
  }
  if (method === 'account/read') {
    await result(id, {
      account: scenario === 'auth-failure' ? null : { type: 'chatgpt', email: null, planType: 'pro' },
      requiresOpenaiAuth: true,
    });
    return;
  }
  if (method === 'account/rateLimits/read') {
    await result(id, { rateLimits: null, rateLimitsByLimitId: null, ordinaryUsageAllowed: true });
    return;
  }
  if (method === 'config/read') {
    await result(id, {
      config: { model_provider: 'openai', openai_base_url: '', chatgpt_base_url: 'https://chatgpt.com/backend-api/' },
      origins: {}, layers: null,
    });
    return;
  }
  if (method === 'model/list') {
    await result(id, {
      data: [{
        id: 'fake-model', model: 'fake-model', displayName: 'Synthetic fixture model',
        description: 'Offline fixture, not a provider model.', hidden: false, isDefault: true,
        defaultReasoningEffort: 'low',
        supportedReasoningEfforts: [
          { reasoningEffort: 'low', description: 'Synthetic low setting' },
          { reasoningEffort: 'medium', description: 'Synthetic medium setting' },
          { reasoningEffort: 'high', description: 'Synthetic high setting' },
        ],
      }], nextCursor: null,
    });
    return;
  }
  if (method === 'thread/start') {
    const threadId = idFor('thread', ++threadSequence);
    const thread = {
      id: threadId, sessionId: threadId, cliVersion: 'synthetic-fixture', createdAt: 0, updatedAt: 0,
      cwd: resolve(params.cwd ?? process.cwd()), ephemeral: params.ephemeral ?? false, modelProvider: 'openai',
      preview: 'Synthetic thread; no provider conversation.', projectId: null,
      source: 'appServer', status: { type: 'idle' }, turns: [],
    };
    threads.set(threadId, thread);
    threadConfigurations.set(threadId, {
      model: params.model ?? 'fake-model',
      effort: params.config?.model_reasoning_effort ?? params.reasoningEffort ?? 'low',
      approvalPolicy: params.approvalPolicy ?? 'untrusted',
      approvalsReviewer: params.approvalsReviewer ?? 'user',
    });
    await result(id, descriptor(thread));
    return;
  }
  if (method === 'thread/resume') {
    const thread = threads.get(params.threadId);
    if (!thread) await rpcError(id, -32602, 'Synthetic thread does not exist in this process');
    else await result(id, descriptor(thread));
    return;
  }
  if (method === 'turn/start') {
    if (scenario === 'auth-failure' || scenario === 'rate-limit') {
      const auth = scenario === 'auth-failure';
      await rpcError(id, auth ? -32001 : -32029,
        auth ? 'Synthetic authentication failure; no provider called' : 'Synthetic rate limit; no provider called',
        { synthetic: true, kind: auth ? 'authentication' : 'rate_limit', ...(auth ? {} : { retryAfterSeconds: 1 }) });
      return;
    }
    const thread = threads.get(params.threadId);
    if (!thread) {
      await rpcError(id, -32602, 'Synthetic turn requires an existing threadId');
      return;
    }
    if (thread.status.type === 'active') {
      await rpcError(id, -32000, 'Synthetic thread already has an active turn');
      return;
    }
    const sequence = ++turnSequence;
    const turn = {
      sequence, threadId: thread.id, itemId: idFor('item', sequence), timer: null, keepAlive: null,
      value: { id: idFor('turn', sequence), items: [], status: 'inProgress', error: null },
    };
    turns.set(turn.value.id, turn);
    thread.turns.push(turn.value);
    thread.status = { type: 'active', activeFlags: [] };
    await result(id, { turn: turn.value });
    await notification('turn/started', { threadId: thread.id, turn: turn.value });
    // Do not await scenario completion in the input queue: interruption must remain responsive.
    beginScenario(turn).catch(fatal);
    return;
  }
  if (method === 'turn/interrupt') {
    const turn = turns.get(params.turnId);
    if (!turn || turn.threadId !== params.threadId) {
      await rpcError(id, -32602, 'Synthetic turn/thread identity does not match');
      return;
    }
    await result(id, {});
    await finishTurn(turn, 'interrupted', 'Synthetic turn interrupted; no command was executed.');
    return;
  }
  await rpcError(id, -32601, `Method not supported by synthetic fixture: ${method}`);
}

const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
lines.on('line', line => {
  if (!line.trim()) return;
  inputQueue = inputQueue.then(async () => {
    let message;
    try { message = JSON.parse(line); }
    catch {
      await rpcError(null, -32700, 'Parse error: synthetic fixture received invalid JSON');
      return;
    }
    await handleMessage(message);
  });
  inputQueue.catch(fatal);
});

process.once('SIGINT', () => { stopTimers(); process.exit(130); });
process.once('SIGTERM', () => { stopTimers(); process.exit(143); });
