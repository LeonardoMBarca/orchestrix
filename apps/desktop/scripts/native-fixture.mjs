// Explicit native QA only. Normal Desktop startup never reads or runs this fixture.
import {mkdirSync,writeFileSync,realpathSync,existsSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';
import path from 'node:path';
if(process.platform!=='win32')throw new Error('Windows QA fixture only');
const repoRoot=fileURLToPath(new URL('../../../',import.meta.url));
const executable=path.join(repoRoot,'target','debug','orchestrix-runtime-fixture.exe');
if(!existsSync(executable))throw new Error('Build the Orchestrix-owned runtime fixture first');
const parent=path.join(repoRoot,'artifacts','native-desktop');mkdirSync(parent,{recursive:true});
const root=path.join(parent,`qa-${Date.now()}-${process.pid}`);mkdirSync(root);
const repository=path.join(root,'repository'),worktrees=path.join(root,'worktrees'),home=path.join(root,'runtime-home');
for(const directory of [repository,worktrees,home,path.join(root,'empty-hooks')])mkdirSync(directory);
writeFileSync(path.join(repository,'fixture.txt'),'before\n');
writeFileSync(path.join(repository,'README.md'),'# Native Desktop QA\n\nSynthetic runtime, real Git and SQLite.\n');
// Supporting-editor fixture stays outside the conservative Git artifact profile.
// Correction intentionally rejects snapshots containing excluded secret files.
writeFileSync(path.join(root,'.env'),'COLOR=blue\n');
const env=Object.fromEntries(Object.entries(process.env).filter(([key])=>!key.toUpperCase().startsWith('GIT_')));
Object.assign(env,{GIT_CONFIG_NOSYSTEM:'1',GIT_CONFIG_GLOBAL:'NUL',GIT_TERMINAL_PROMPT:'0'});
for(const args of [['init','--quiet','--initial-branch=main'],['add','--all'],['commit','--quiet','-m','Owned native Desktop fixture']]){
  const result=spawnSync('git',['-c',`core.hooksPath=${path.join(root,'empty-hooks')}`,'-c','core.autocrlf=false','-c','user.name=Orchestrix fixture','-c','user.email=fixture@invalid.local',...args],{cwd:repository,env,encoding:'utf8',windowsHide:true});
  if(result.status!==0)throw new Error(`Fixture Git ${args[0]} failed`);
}
const config={root:realpathSync(root),executable:realpathSync(executable),home:realpathSync(home),repository:realpathSync(repository),worktrees:realpathSync(worktrees),reviewScenario:'review-reject'};
const configPath=path.join(root,'fixture.json');writeFileSync(configPath,JSON.stringify(config,null,2));
console.log(JSON.stringify({fixtureConfig:configPath,...config},null,2));
