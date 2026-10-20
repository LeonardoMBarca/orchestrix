import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {diagnosticObservationKey,diagnosticStorageBlocked,diagnosticByteSize,diagnosticScopeValue,diagnosticCapturedValidity} from '../src/diagnostic-history-state.ts';
import {diagnosticHistoryTranslations} from '../src/diagnostic-history-locales.ts';
test('transitions in the same diagnostic remain distinct exact observations',()=>{
 assert.notEqual(diagnosticObservationKey({sequence:1,ordinal:1}),diagnosticObservationKey({sequence:2,ordinal:2}));
 assert.notEqual(diagnosticObservationKey({sequence:1,ordinal:1}),diagnosticObservationKey({sequence:1,ordinal:2}));
});
test('historical storage authority includes reserved terminal evidence and explicit blocking',()=>{
 const storage={usedBytes:80,reservedBytes:19,maximumHistoryBytes:100};
 assert.equal(diagnosticStorageBlocked(storage),false);
 assert.equal(diagnosticStorageBlocked({...storage,reservedBytes:20}),true);
 assert.equal(diagnosticStorageBlocked({...storage,storageBlocked:true}),true);
 assert.equal(diagnosticStorageBlocked({...storage,globalUsedAndReservedBytes:1000,maximumGlobalHistoryBytes:1000}),true);
 assert.equal(diagnosticByteSize(1024),'1.0 KiB');assert.equal(diagnosticByteSize(1024**3),'1.0 GiB');assert.equal(diagnosticByteSize(-1),'—');
});
test('only scalar scope evidence reaches human-readable technical fields',()=>{
 assert.equal(diagnosticScopeValue({purpose:'registration'},'purpose'),'registration');
 assert.equal(diagnosticScopeValue({revision:3},'revision'),'3');
 assert.equal(diagnosticScopeValue({descriptor:{private:'value'}},'descriptor'),null);
});
test('captured validity presents original durations without dating or renewing them',()=>{
 const captured=Object.freeze({identity:1000,configuration:5000,availability:2000,models:3000});
 assert.deepEqual(diagnosticCapturedValidity(captured),[{key:'identity',milliseconds:1000},{key:'configuration',milliseconds:5000},{key:'availability',milliseconds:2000},{key:'models',milliseconds:3000}]);
 assert.deepEqual(diagnosticCapturedValidity(undefined),[]);assert.equal(captured.models,3000);
});
test('history presentation has complete Portuguese and Spanish messages',()=>{
 const source=readFileSync(new URL('../src/diagnostic-history.tsx',import.meta.url),'utf8');
 const existing=readFileSync(new URL('../src/connection-locales.ts',import.meta.url),'utf8')+readFileSync(new URL('../src/i18n.ts',import.meta.url),'utf8')+readFileSync(new URL('../src/ox016-locales.ts',import.meta.url),'utf8')+readFileSync(new URL('../../../prototypes/desktop/locales/experience.json',import.meta.url),'utf8');
 const catalogs=['chrome','shell-revision','settings-revision','activity-shell','files-shell','experience','session-revision','chat-controls'].flatMap(name=>Object.values(JSON.parse(readFileSync(new URL(`../../../prototypes/desktop/locales/${name}.json`,import.meta.url),'utf8'))));
 for(const match of source.matchAll(/\bt\('([^']+)'\)/g))assert(diagnosticHistoryTranslations[match[1]]?.every(Boolean)||existing.includes(`'${match[1]}'`)||catalogs.some(value=>value.en===match[1]&&value.es),match[1]);
});
