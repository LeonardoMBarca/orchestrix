import test from 'node:test';
import assert from 'node:assert/strict';
import {writeFile,realpath,readFile} from 'node:fs/promises';
import {join,dirname} from 'node:path';
import {tmpdir} from 'node:os';
import {EventEmitter} from 'node:events';
import {CodexSession} from '../src/codex-session.mjs';
import {createWorktreeFixture,inspectAttempt,runFixtureCheck,requireFailingBaseline} from '../worktree-probe.mjs';
import {reportConfiguration,supportsCodexVersion,fixtureGitEnvironment,codexLaunchArguments} from '../probe.mjs';

test('MXC is an explicit subprocess override and cannot silently enable another sandbox implementation',()=>{
  const defaults=codexLaunchArguments();assert.equal(defaults.some(value=>value.startsWith('windows.sandbox=')),false);
  const selected=codexLaunchArguments({windowsSandbox:'mxc'});
  assert.equal(selected.filter(value=>value==='windows.sandbox="mxc"').length,1);
  assert.ok(selected.indexOf('windows.sandbox="mxc"')<selected.indexOf('app-server'));
  for(const boundary of ['features.hooks=false','features.plugins=false','features.apps=false','features.multi_agent=false','--no-daemon'])assert.ok(selected.includes(boundary));
  for(const value of ['elevated','unelevated','danger-full-access','mxc\nnotify=["bad"]',null])assert.throws(()=>codexLaunchArguments({windowsSandbox:value}),error=>error.kind==='arguments');
});

test('Windows sandbox discovery accepts only explicit known config values and keeps absence unknown',async()=>{
  for(const value of ['mxc','elevated','unelevated',undefined,'unexpected',false]) {
    const client=new EventEmitter();client.request=async()=>({config:{windows:{sandbox:value}}});
    const session=new CodexSession(client);
    assert.equal((await session.inspectEndpoints('/unused')).windowsSandbox,['mxc','elevated','unelevated'].includes(value)?value:null);
  }
});

test('baseline admission requires the arithmetic assertion, never an unavailable or broken checker',async()=>{
  const fixture=await createWorktreeFixture();assert.equal(fixture.baselineCheck.failureKind,'assertion-failed');
  assert.doesNotThrow(()=>requireFailingBaseline(fixture.baselineCheck));
  const unavailable=await runFixtureCheck(fixture.hooks);assert.equal(unavailable.passed,false);assert.equal(unavailable.failureKind,'checker-error');
  assert.throws(()=>requireFailingBaseline(unavailable),error=>error.kind==='fixture');
  for(const check of [{passed:false,exitCode:null,failureKind:'timeout'},{passed:true,exitCode:0,cases:5},{passed:false,exitCode:1,failureKind:'checker-error'}])assert.throws(()=>requireFailingBaseline(check),error=>error.kind==='fixture');
});

test('Windows temporary aliases cannot change the worktree identity persisted by Git or a runtime',async()=>{
  const fixture=await createWorktreeFixture();
  const canonicalTemp=await realpath(tmpdir());
  assert.equal(dirname(fixture.folder),canonicalTemp);
  for(const directory of [fixture.folder,fixture.main,fixture.worktree,fixture.hooks])assert.equal(directory,await realpath(directory));
  // Git embeds the same canonical fixture location in the linked worktree's
  // .git file; short TEMP names must not create a second cwd representation.
  const pointer=await readFile(join(fixture.worktree,'.git'),'utf8');
  const gitDirectory=pointer.trim().slice('gitdir: '.length);
  assert.equal(await realpath(gitDirectory),join(fixture.main,'.git','worktrees','attempt'));
  assert.equal(fixture.baselineCheck.passed,false);
});

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
