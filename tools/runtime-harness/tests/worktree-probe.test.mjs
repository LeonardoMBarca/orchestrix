import test from 'node:test';
import assert from 'node:assert/strict';
import {writeFile} from 'node:fs/promises';
import {join} from 'node:path';
import {EventEmitter} from 'node:events';
import {CodexSession} from '../src/codex-session.mjs';
import {createWorktreeFixture,inspectAttempt,runFixtureCheck} from '../worktree-probe.mjs';
import {reportConfiguration,supportsCodexVersion,fixtureGitEnvironment} from '../probe.mjs';

test('attempt diff and five checks bind to the modified worktree while main remains unchanged',async()=>{
  const fixture=await createWorktreeFixture();assert.equal(fixture.baselineCheck.passed,false);
  await writeFile(join(fixture.worktree,'src/add.mjs'),'export function add(a, b) { return a + b; }\n');
  const result=await inspectAttempt(fixture);
  assert.equal(result.passed,true);assert.equal(result.base,fixture.base);assert.equal(result.mainUnchanged,true);
  assert.equal(result.checksSameSnapshot,true);assert.equal(result.checks.cases,5);
  assert.deepEqual(result.paths,['src/add.mjs']);assert.match(result.diff,/^\+export function add\(a, b\)/m);
  assert.match(result.diffSha256,/^[a-f0-9]{64}$/);
});
test('tampering with the trusted checker or adding an untracked file blocks check execution',async()=>{
  const fixture=await createWorktreeFixture();
  await writeFile(join(fixture.worktree,'src/add.mjs'),'export function add(a, b) { return a + b; }\n');
  await writeFile(join(fixture.worktree,'checks/add-check.mjs'),'throw new Error("Checker must not run");');
  const tampered=await inspectAttempt(fixture);assert.equal(tampered.passed,false);assert.equal(tampered.checks.notRun,true);
  const other=await createWorktreeFixture();
  await writeFile(join(other.worktree,'src/add.mjs'),'export function add(a, b) { return a + b; }\n');
  await writeFile(join(other.worktree,'unexpected.txt'),'Untracked content');
  const result=await inspectAttempt(other);assert.equal(result.passed,false);assert.equal(result.checks.notRun,true);
});
test('main checkout changes and non-arithmetic generated code cannot pass the fixture check',async()=>{
  const fixture=await createWorktreeFixture();
  await writeFile(join(fixture.worktree,'src/add.mjs'),'export function add(a, b) { return a + b; }\n');
  await writeFile(join(fixture.main,'AGENTS.md'),'Changed outside the attempt');
  const result=await inspectAttempt(fixture);assert.equal(result.mainUnchanged,false);assert.equal(result.passed,false);assert.equal(result.checks.notRun,true);
  const other=await createWorktreeFixture();
  await writeFile(join(other.worktree,'src/add.mjs'),'process.exit(0); export function add(a, b) { return a + b; }');
  assert.equal((await runFixtureCheck(other.worktree)).passed,false);
});
test('schema-reviewed versions are explicit and reports exclude session IDs, paths and bindings',()=>{
  for(const version of ['0.162.0-alpha.2','0.162.0-alpha.17.2'])assert.equal(supportsCodexVersion(version),true);
  for(const version of ['0.162.0-alpha.17.3','latest',null])assert.equal(supportsCodexVersion(version),false);
  const report=reportConfiguration({threadId:'PRIVATE_THREAD',ephemeral:false,accountBinding:'PRIVATE_BINDING',requested:{threadId:'PRIVATE_THREAD',cwd:'PRIVATE_PATH',model:'sample',effort:'low',sandbox:'workspace-write',writableRoots:[]},resolvedConfiguration:{threadId:'PRIVATE_THREAD',cwd:'PRIVATE_PATH',model:'sample',sandboxPolicy:{writableRoots:['PRIVATE_PATH']},writableRoots:['PRIVATE_PATH'],cwdMatched:true}});
  assert.equal(JSON.stringify(report).includes('PRIVATE_'),false);assert.equal(report.resolvedConfiguration.writableRootsCount,1);
});
test('inherited Git overrides cannot redirect disposable fixture writes to another repository',async t=>{
  const outside=await createWorktreeFixture();
  const before=await inspectAttempt(outside);
  const previous=Object.fromEntries(['GIT_DIR','GIT_WORK_TREE','GIT_INDEX_FILE','GIT_CONFIG_COUNT','GIT_CONFIG_KEY_0','GIT_CONFIG_VALUE_0'].map(key=>[key,process.env[key]]));
  t.after(()=>{for(const [key,value] of Object.entries(previous)){if(value===undefined)delete process.env[key];else process.env[key]=value;}});
  Object.assign(process.env,{GIT_DIR:join(outside.main,'.git'),GIT_WORK_TREE:outside.main,GIT_INDEX_FILE:join(outside.main,'.git','index'),GIT_CONFIG_COUNT:'1',GIT_CONFIG_KEY_0:'core.worktree',GIT_CONFIG_VALUE_0:outside.main});
  const env=fixtureGitEnvironment();assert.equal(env.GIT_DIR,undefined);assert.equal(env.GIT_INDEX_FILE,undefined);assert.equal(env.GIT_CONFIG_COUNT,undefined);
  const fixture=await createWorktreeFixture();assert.notEqual(fixture.main,outside.main);
  const after=await inspectAttempt(outside);assert.equal(after.mainUnchanged,true);assert.equal(after.base,before.base);assert.equal(after.diffSha256,before.diffSha256);
});
test('native delegation is confirmed disabled only when the resolved feature is explicitly false',async()=>{
  const client=new EventEmitter();let config={features:{multi_agent:false}};
  client.request=async()=>({config});const session=new CodexSession(client);
  assert.equal((await session.inspectEndpoints('/fixture')).resources.nativeDelegationDisabled,true);
  config={features:{multi_agent:true}};
  assert.equal((await session.inspectEndpoints('/fixture')).resources.nativeDelegationDisabled,false);
  config={features:{}};
  assert.equal((await session.inspectEndpoints('/fixture')).resources.nativeDelegationDisabled,false);
});
