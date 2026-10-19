import test from 'node:test';
import assert from 'node:assert/strict';
import {cleanConfigurationDraft,configurationDiff,configurationDraftValid,configurationEqual,configurationError,configurationKey,configurationLines,copyConfiguration,defaultDefinitionSelection,selectedDefinition} from '../src/configuration-types.ts';
import {configurationText,configurationTranslations} from '../src/configuration-locales.ts';

const profile={id:'implementation-engineer',version:2,editRevision:1,origin:'native',archived:false,name:'Implementation Engineer',role:'implementation',instructions:'Use small changes',outputLanguage:'inherit',conventions:[],specialty:'Code',preferences:{},requirements:{subscription_only:true}};
const mode={id:'development',version:2,editRevision:1,origin:'native',archived:false,name:'Development',purpose:'Implement software',instructions:'Keep evidence',roles:[{role:'implementation',profile:{id:profile.id,version:2}},{role:'review',profile:{id:'independent-reviewer',version:2}}],outputs:['Patch'],criteria:['Checks'],preferences:{},requirements:{subscription_only:true,require_review:true},executionSupport:'manual-worker'};
test('typing multiline criteria preserves Enter while wire normalization leaves the draft untouched',()=>{
 assert.deepEqual(configurationLines('first\n'),['first','']);
 const draft={...mode,outputs:configurationLines(' First output\n\nSecond output\n')};
 assert.deepEqual(cleanConfigurationDraft(draft).outputs,['First output','Second output']);
 assert.deepEqual(draft.outputs,[' First output','','Second output','']);
 assert.equal(configurationDraftValid('mode',draft),true);
 assert.equal(configurationDraftValid('mode',{...draft,criteria:['',' ']}),false);
});
test('local draft keys separate modes/profiles and project overrides without using names as identities',()=>{
 assert.notEqual(configurationKey('mode',{id:'shared'}),configurationKey('profile',{id:'shared'}));
 assert.notEqual(configurationKey('scope',{scope:'project',projectId:'a'}),configurationKey('scope',{scope:'project',projectId:'b'}));
 assert.notEqual(configurationKey('scope',{scope:'user'}),configurationKey('scope',{scope:'project',projectId:'a'}));
});
test('before/after is readable, preserves absent/false/zero and does not mutate prior snapshots',()=>{
 const saved=copyConfiguration(mode),next={...copyConfiguration(mode),instructions:'Prefer focused changes',preferences:{network:false,context_bytes:0}};
 const rows=configurationDiff(saved,next);
 assert.deepEqual(rows.map(row=>row.field),['instructions','preferences']);
 assert.equal(rows.find(row=>row.field==='preferences').after,'network: false\ncontext_bytes: 0');
 assert.equal(configurationEqual(saved,mode),true);
 assert.equal(configurationEqual({}, {network:false}),false);
 assert.equal(configurationEqual({}, {context_bytes:0}),false);
});
test('a definition reference requires exact identity and version; a homonym never matches',()=>{
 assert.equal(selectedDefinition([profile],{id:'other',version:2}),undefined);
 assert.equal(selectedDefinition([profile],{id:profile.id,version:1}),undefined);
 assert.equal(selectedDefinition([profile],{id:profile.id,version:2}),profile);
 assert.deepEqual(defaultDefinitionSelection({modes:[mode],profiles:[profile],scopes:[],revision:1,schemaVersion:1}),{mode:{id:mode.id,version:mode.version},profile:{id:profile.id,version:profile.version}});
});
test('validation checks a usable role contract without granting execution or validating capabilities',()=>{
 assert.equal(configurationDraftValid('mode',mode),true);
 assert.equal(configurationDraftValid('mode',{...mode,roles:[mode.roles[0],mode.roles[0]]}),false);
 assert.equal(configurationDraftValid('profile',{...profile,instructions:'Please write as a reviewer',role:'review'}),true,'text is not a permission grant; Core validates restrictions');
 assert.equal(configurationDraftValid('scope',{scope:'project',projectId:'',preferences:{},requirements:{}}),false);
 assert.equal(configurationDraftValid('scope',{scope:'user',preferences:{},requirements:{}}),true);
});
test('CAS errors preserve an actionable explanation and all editor translations exist in EN/PT/ES',()=>{
 const key=configurationError('stale-revision-resync');
 assert.match(key,/draft is preserved/);
 assert.equal(configurationText('en',key),key);
 assert.match(configurationText('pt',key),/rascunho/);
 assert.match(configurationText('es',key),/borrador/);
 assert.notEqual(configurationText('pt','Result language'),configurationText('es','Result language'));
 for(const [source,translations] of Object.entries(configurationTranslations))assert.ok(source&&translations.length===2&&translations.every(value=>value.trim()),source);
});
