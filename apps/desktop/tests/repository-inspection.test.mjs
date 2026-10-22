import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {repositoryInspectionReady,repositoryResponseSupported,repositoryReplyCurrent,repositoryOutcome,repositoryBlockerLabel} from '../src/repository-inspection-state.ts';
import {repositoryTranslations} from '../src/repository-locales.ts';
const report={schemaVersion:1,profile:'regular-utf8-v1',baseOid:'a'.repeat(40),headOid:'a'.repeat(40),compatible:true,applicationEligible:false,complete:true,inventory:{files:3,bytes:2000,largestFileBytes:1200,regular:3,executables:0,links:0,submodules:0,oversized:0,binary:0,nonUtf8:0,secretPattern:0,excluded:0,contentInspected:3,contentSkipped:0},checkout:{clean:false,status:' M src/task.ts',truncated:false,ignored:0,untracked:0},blockers:[],limits:{files:4096,fileBytes:1048576,readBytes:67108864},modelCalled:false,authority:'inspection-only-revalidate-before-effects'};
test('worker preparation requires a complete compatible inspection for the exact native grant',()=>{
 const inspection={directoryGrant:'folder-one',report};
 assert.equal(repositoryInspectionReady(inspection,'folder-one'),true);
 assert.equal(repositoryInspectionReady(inspection,'folder-two'),false);
 assert.equal(repositoryInspectionReady(inspection,''),false);
 assert.equal(repositoryInspectionReady(null,'folder-one'),false);
 for(const patch of [{schemaVersion:2},{profile:'arbitrary-profile'},{compatible:false},{complete:false},{modelCalled:true},{authority:'execution-authorized'}])assert.equal(repositoryInspectionReady({...inspection,report:{...report,...patch}},'folder-one'),false);
 assert.equal(report.applicationEligible,false,'an application restriction does not invent a worker restriction');
 assert.equal(inspection.report,report,'the observation is not rewritten or granted execution authority');
});
test('incomplete or blocked inspections stay visible with an explicit distinct outcome',()=>{
 assert.equal(repositoryOutcome(report),'Compatible with the worker profile');
 assert.equal(repositoryOutcome({...report,complete:false}),'Inspection incomplete');
 assert.equal(repositoryOutcome({...report,compatible:false}),'Repository blocked for this profile');
 assert.equal(repositoryResponseSupported({...report,compatible:false}),true,'a supported negative is still a valid inspection response');
 assert.equal(repositoryResponseSupported({...report,modelCalled:true}),false);
 assert.equal(repositoryBlockerLabel('binary-content'),'Binary content');
 assert.equal(repositoryBlockerLabel('a-new-restriction-code'),'Repository profile restriction','unknown restriction codes are not given an invented cause');
});
test('late inspection and Git status replies cannot cross a native grant or newer request',()=>{
 assert.equal(repositoryReplyCurrent('first','first',2,2,true),true);
 assert.equal(repositoryReplyCurrent('first','second',2,2,true),false);
 assert.equal(repositoryReplyCurrent('first','first',2,3,true),false);
 assert.equal(repositoryReplyCurrent('first','first',2,2,false),false);
});
test('repository inspection messages have Portuguese and Spanish counterparts',()=>{
 const source=['repository-inspection.tsx','repository-inspection-state.ts'].map(name=>readFileSync(new URL(`../src/${name}`,import.meta.url),'utf8')).join('\n');
 const existing=['i18n.ts','connection-locales.ts','ox016-locales.ts','diagnostic-history-locales.ts'].map(name=>readFileSync(new URL(`../src/${name}`,import.meta.url),'utf8')).join('\n');
 const catalogs=['chrome','shell-revision','settings-revision','activity-shell','files-shell','experience','session-revision','chat-controls'].flatMap(name=>Object.values(JSON.parse(readFileSync(new URL(`../../../prototypes/desktop/locales/${name}.json`,import.meta.url),'utf8'))));
 for(const match of source.matchAll(/\bt\('([^']+)'\)/g))assert(repositoryTranslations[match[1]]?.every(Boolean)||existing.includes(`'${match[1]}'`)||catalogs.some(value=>value.en===match[1]&&value.es),match[1]);
 for(const text of ['Inspecting repository…','Reading Git status…','Inspection incomplete','Compatible with the worker profile','Repository blocked for this profile','Regular files','Executable files','Links','Submodules','Oversized files','Binary files','Non-UTF-8 files','Sensitive patterns','Excluded paths','Content not inspected'])assert(repositoryTranslations[text]?.every(Boolean),text);
});


test('opaque profile requires the same explicit editable scope and never grants read confinement',()=>{
 const v2={...report,schemaVersion:2,profile:'opaque-base-v2',preservation:{baseFiles:573,baseBytes:54332380,preservedFiles:572,preservedBytes:54320000,editablePaths:['docs/guide.md'],selectionRequired:false}};
 assert(repositoryResponseSupported(v2));
 assert(!repositoryInspectionReady({directoryGrant:'folder-one',report:v2},'folder-one'));
 assert(repositoryInspectionReady({directoryGrant:'folder-one',report:v2},'folder-one',['docs/guide.md']));
 assert(!repositoryInspectionReady({directoryGrant:'folder-one',report:v2},'folder-one',['another.md']));
 assert(!repositoryInspectionReady({directoryGrant:'folder-one',report},'folder-one',['docs/guide.md']));
 assert.equal(repositoryOutcome(v2),'Compatible with the opaque-base profile');
 assert.equal(repositoryOutcome({...v2,compatible:false,preservation:{...v2.preservation,selectionRequired:true}}),'Select editable files to complete compatibility');
});

test('opaque preservation and protected-content explanations retain Portuguese and Spanish labels',()=>{
 for(const text of ['Recorded base preservation','File inventory','Pattern matches in preserved files do not prove real credentials and do not authorize their content as context or edits. Selected text still requires privacy validation.','Protected or unlisted content changed. Application is blocked; preserve and inspect the worktree and target before a new authorized decision.'])assert(repositoryTranslations[text]?.every(Boolean),text);
});
