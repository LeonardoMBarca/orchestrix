import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {capacityFieldLabel,capacityIsCurrent,capacityPercent,capacityResetReached,capacityUsageLabel,diagnosticCapacity,preparedCapacity,selectedCapacity} from '../src/capacity-state.ts';
import {connectionResourceFacts,diagnosticNextAction,mergeDiagnosticProjection,acceptDiagnosticDetail} from '../src/connection-state.ts';
import {capacityTranslations} from '../src/capacity-locales.ts';
const reported=value=>({state:'reported',value}),missing={state:'missing'},unknown={state:'null'};
const window={slot:'primary',state:'reported',unit:'percent-of-window',usedPercent:reported(0),remainingPercent:{value:100,origin:'derived',basis:'100-minus-used-percent'},durationMinutes:reported(300),resetsAtSeconds:reported(2)};
const observation={schemaVersion:1,source:'codex-account-rate-limits',connectionId:'account-a',connectionRevision:3,runtimeVersion:'0.162.0-alpha.17.2',observedAtMs:1000,synthetic:true,ordinaryUsage:reported(true),bucketView:'multi',buckets:[],completeness:'complete',coverage:'reported-subset',issues:[],capacityGroupIds:[]};
const diagnostic={id:'diagnostic-a',state:'complete',connectionRevision:3,observedAtMs:1000,effectiveValidUntilMs:3000,current:true,capacity:observation};
const presentation={observation,diagnosticId:'diagnostic-a',current:true,effectiveValidUntilMs:3000,previous:false,identityChanged:false};

test('zero, missing, null, invalid and out-of-range percentages remain distinct',()=>{
 assert.equal(capacityPercent(window),0);
 assert.equal(capacityPercent({...window,usedPercent:reported(100)}),100);
 for(const usedPercent of [missing,unknown,{state:'invalid'},reported(-1),reported(101),reported(0.2),reported(NaN)])assert.equal(capacityPercent({...window,usedPercent}),null);
 assert.equal(capacityPercent({...window,state:'null'}),null);
 assert.equal(capacityFieldLabel(missing),'Not reported');assert.equal(capacityFieldLabel(unknown),'Not provided (null)');assert.equal(capacityFieldLabel({state:'invalid'}),'Uninterpretable value');
});
test('permission and quantitative coverage are independent, without inferring a model quota',()=>{
 assert.equal(capacityUsageLabel(presentation,1500),'Included usage allowed');
 const denied={...presentation,observation:{...observation,ordinaryUsage:reported(false)}};
 assert.equal(capacityUsageLabel(denied,1500),'Included usage unavailable');
 assert.equal(capacityUsageLabel({...presentation,observation:{...observation,ordinaryUsage:unknown}},1500),'Usage permission unknown');
 assert.equal(capacityUsageLabel(null,1500),'Capacity not reported');
 assert.equal(diagnosticCapacity({latest:diagnostic},'account-a',3).observation.buckets.length,0,'a permission-only read remains useful');
});
test('reset time and elapsed validity do not fabricate restoration or renew historical facts',()=>{
 assert.equal(capacityResetReached(window,2000),true);
 assert.equal(capacityResetReached(window,1999),false);
 assert.equal(capacityIsCurrent(presentation,3000),false);
 assert.equal(capacityIsCurrent(presentation,999),false,'clock before observation is incoherent');
 for(const change of [{current:false},{identityChanged:true},{effectiveValidUntilMs:null}])assert.equal(capacityUsageLabel({...presentation,...change},2000),'Historical capacity observation');
 assert.equal(capacityUsageLabel({...presentation,observation:{...observation,ordinaryUsage:reported(false)}},2500),'Included usage unavailable','passing reset time alone never releases a denial');
 assert.equal(observation.observedAtMs,1000);
});
test('failed refresh preserves the previous observation within its original validity without renewing it',()=>{
 const value=diagnosticCapacity({latest:{id:'failed',state:'failed',connectionRevision:3},lastSuccessful:diagnostic},'account-a',3);
 assert.equal(value.previous,true);assert.equal(value.observation,observation);assert.equal(value.observation.observedAtMs,1000);
 assert.equal(capacityUsageLabel(value,1500),'Included usage allowed','transport failure does not invalidate a still-current source observation');
 assert.equal(value.effectiveValidUntilMs,3000,'the failed refresh cannot renew the original deadline');
 assert.equal(capacityUsageLabel(value,3000),'Historical capacity observation','the observation expires at its original deadline');
 const negative={...diagnostic,id:'negative',capacity:{...observation,ordinaryUsage:reported(false)}};
 assert.equal(diagnosticCapacity({latest:negative,lastSuccessful:diagnostic}).diagnosticId,'negative');
 assert.match(diagnosticNextAction({latest:negative},1500),/Included usage is unavailable/);
 assert.match(diagnosticNextAction({latest:{...negative,current:false}},1500),/observation is stale/,'an expired denial does not claim fresh provider status');
});
test('exact binding and prepared scope evidence never fall back to another connection or revision',()=>{
 for(const [id,revision] of [['account-b',3],['account-a',4]]){
  assert.equal(diagnosticCapacity({latest:diagnostic},id,revision),null);
  assert.equal(preparedCapacity({...presentation,identityChanged:false},id,revision),null);
 }
 assert.equal(preparedCapacity({...presentation,identityChanged:false},'account-a',3).diagnosticId,'diagnostic-a');
 assert.equal(diagnosticCapacity({latest:{...diagnostic,capacity:undefined}}),null,'legacy evidence remains unknown');
 assert.equal(connectionResourceFacts({latest:diagnostic}).find(fact=>fact.id==='usage').state,'reported');
 assert.equal(connectionResourceFacts({latest:diagnostic},{id:'other-account',bindingRevision:3}).find(fact=>fact.id==='usage').state,'unknown','a selection change cannot flash another account capacity in resource facts');
});
test('compact snapshots preserve exact detailed capacity without renewing or crossing transitions',()=>{
 const detail={latest:diagnostic,lastSuccessful:diagnostic};
 const {capacity,...compact}=diagnostic;
 const merged=mergeDiagnosticProjection(detail,{latest:{...compact,current:false},lastSuccessful:{...compact,current:false}});
 assert.equal(merged.latest.capacity,capacity);assert.equal(merged.latest.current,false);assert.equal(merged.latest.observedAtMs,1000);
 const next=mergeDiagnosticProjection(detail,{latest:{id:'next-transition',state:'running',connectionRevision:3},lastSuccessful:{...compact,current:false}});
 assert.equal(next.latest.capacity,undefined,'a later transition cannot inherit a previous capacity body');
 assert.equal(next.lastSuccessful.capacity,capacity,'an exact historical success remains inspectable with its original date');
 assert.equal(mergeDiagnosticProjection(detail,{latest:{...compact,connectionRevision:4}}).latest.capacity,undefined,'a different binding cannot inherit capacity even with a matching textual diagnostic ID');
 assert.equal(diagnosticCapacity(next).previous,true);
 assert.equal(mergeDiagnosticProjection(detail,undefined),undefined,'an absent scope does not revive a detailed observation');
});
test('footer resolves the selected or prepared connection without falling back from an unresolved preference',()=>{
 const account={id:'account-a',bindingRevision:3,selected:true,label:'A',diagnostics:{latest:diagnostic}};
 const other={...account,id:'account-b',selected:false,diagnostics:{latest:{...diagnostic,capacity:{...observation,connectionId:'account-b',ordinaryUsage:reported(false)}}}};
 assert.equal(capacityUsageLabel(selectedCapacity([account,other]),1500),'Included usage allowed');
 assert.equal(capacityUsageLabel(selectedCapacity([account,other],{connectionId:'account-b'}),1500),'Included usage unavailable');
 for(const connectionId of ['', 'missing-account'])assert.equal(capacityUsageLabel(selectedCapacity([account,other],{connectionId}),1500),'Capacity not reported');
 assert.equal(selectedCapacity([{...account,enabled:false}],{connectionId:'account-a'}),null);
 assert.equal(selectedCapacity([{...account,linked:false}],{connectionId:'account-a'}),null);
 assert.equal(selectedCapacity([account],{connectionId:'account-a',bindingRevision:4}),null);
 const pinned={diagnosticId:'project-observation',observation:{...observation,observedAtMs:1200},current:true,effectiveValidUntilMs:2500,identityChanged:false};
 assert.equal(selectedCapacity([account],{connectionId:'account-a',bindingRevision:3,prepared:pinned}).diagnosticId,'project-observation');
 assert.equal(capacityUsageLabel(selectedCapacity([account],{connectionId:'account-a',prepared:pinned}),2500),'Historical capacity observation','a prepared observation keeps its own original deadline');
 const updated={...account,diagnostics:{latest:{...diagnostic,id:'new-denial',capacity:{...observation,observedAtMs:1400,ordinaryUsage:reported(false)}}}};
 assert.equal(capacityUsageLabel(selectedCapacity([updated],{connectionId:'account-a',prepared:pinned}),1500),'Included usage unavailable','a newer denial must supersede an older prepared display');
 assert.equal(selectedCapacity([account],{connectionId:'account-a',prepared:{...pinned,observation:{...pinned.observation,connectionId:'account-b'}}}).diagnosticId,'diagnostic-a','a foreign prepared snapshot contributes no facts');
 assert.equal(selectedCapacity([{...account,diagnostics:undefined}]),null);
});
test('delayed detail cannot replace a newer denial or identity fence in the current snapshot',async()=>{
 const detail={latest:diagnostic,lastSuccessful:diagnostic,identityChanged:false};
 const {capacity:omitted,...compact}=diagnostic;
 let resolve;const delayed=new Promise(done=>{resolve=done;});
 const negative={...diagnostic,id:'new-negative',capacity:undefined,current:true};
 const newer={latest:negative,lastSuccessful:negative,identityChanged:false};
 resolve(detail);const arrived=await delayed;
 assert.equal(acceptDiagnosticDetail(arrived,newer),null,'an older positive cannot overwrite the newer exact negative transition');
 assert.equal(acceptDiagnosticDetail(arrived,{latest:{...diagnostic,state:'failed'},lastSuccessful:diagnostic}),null,'matching textual ID alone cannot cross terminal states');
 assert.equal(acceptDiagnosticDetail(arrived,{latest:{...diagnostic,connectionRevision:4},lastSuccessful:diagnostic}),null,'a reused textual ID cannot cross bindings');
 assert.equal(acceptDiagnosticDetail(arrived,{latest:compact,lastSuccessful:{...compact,current:false},identityChanged:true}).identityChanged,true,'the current identity fence wins over delayed detail');
 const accepted=acceptDiagnosticDetail(arrived,{latest:{...compact,current:false},lastSuccessful:{...compact,current:false},identityChanged:true});
 assert.equal(accepted.latest.current,false);assert.equal(accepted.latest.capacity.observedAtMs,1000);assert.equal(accepted.latest.effectiveValidUntilMs,3000);
 assert.equal(capacityUsageLabel(diagnosticCapacity(accepted),1500),'Historical capacity observation');
 assert.equal(acceptDiagnosticDetail(arrived,undefined),null,'absence of a current projection cannot grant a detail response authority');
 const failedAfterNewSuccess={latest:{id:'later-failed',state:'failed',connectionRevision:3},lastSuccessful:negative};
 assert.equal(acceptDiagnosticDetail({...failedAfterNewSuccess,lastSuccessful:diagnostic},failedAfterNewSuccess),null,'an unchanged failure cannot replace the snapshot’s newer last successful evidence');
});
test('a current identity fence prevents a prepared positive from reviving through a compact snapshot',()=>{
 const pinned={diagnosticId:'prepared-positive',observation:{...observation,observedAtMs:1200},current:true,effectiveValidUntilMs:2500,identityChanged:false};
 const original=structuredClone(pinned),base={id:'account-a',bindingRevision:3,selected:true,label:'A'};
 const compact={latest:{id:'failed-after-identity-changed-read',state:'failed',connectionRevision:3,current:false}};
 for(const connection of [{...base,identityStatus:'changed',diagnostics:compact},{...base,diagnostics:{...compact,identityChanged:true}}]){
  const selected=selectedCapacity([connection],{connectionId:'account-a',bindingRevision:3,prepared:pinned});
  assert.equal(selected.diagnosticId,'prepared-positive');assert.equal(selected.identityChanged,true);
  assert.equal(selected.observation,pinned.observation,'the original prepared observation body is preserved');
  assert.equal(selected.effectiveValidUntilMs,2500,'the original deadline is preserved');
  assert.equal(capacityUsageLabel(selected,1500),'Historical capacity observation','a prepared positive cannot override current identity uncertainty');
 }
 assert.deepEqual(pinned,original,'presentation fences never rewrite the prepared snapshot');
 assert.equal(capacityUsageLabel(selectedCapacity([{...base,diagnostics:{latest:{...compact.latest,id:'failed-refresh',state:'failed'},lastSuccessful:undefined}}],{connectionId:'account-a',prepared:pinned}),1500),'Included usage allowed','transport failure alone does not invent an identity change or revoke an original lease');
});
test('an unhydrated completed transition cannot lend an older preparation permission or identity',()=>{
 const pinned={diagnosticId:'prepared-positive',observation:{...observation,observedAtMs:1200},current:true,effectiveValidUntilMs:2500,identityChanged:false};
 const original=structuredClone(pinned),base={id:'account-a',bindingRevision:3,selected:true,label:'A'};
 for(const state of ['complete','partial']){
  const latest={id:'different-completed-transition',state,connectionRevision:3,current:true,observedAtMs:1100};
  assert.equal(capacityUsageLabel(selectedCapacity([{...base,diagnostics:{latest}}],{connectionId:'account-a',prepared:pinned}),1500),'Capacity not reported','exact transition identity fences unknown content even if its cached date is older');
 }
 for(const state of ['failed','interrupted','running'])assert.equal(capacityUsageLabel(selectedCapacity([{...base,diagnostics:{latest:{id:'different-incomplete-transition',state,connectionRevision:3,current:false}}}],{connectionId:'account-a',prepared:pinned}),1500),'Included usage allowed','a failed/incomplete refresh preserves the original still-valid observation');
 const exact={id:pinned.diagnosticId,state:'complete',connectionRevision:3,current:false,effectiveValidUntilMs:2500};
 assert.equal(capacityUsageLabel(selectedCapacity([{...base,diagnostics:{latest:exact}}],{connectionId:'account-a',prepared:pinned}),1500),'Historical capacity observation','the exact compact snapshot current flag wins after restart');
 const shortened=selectedCapacity([{...base,diagnostics:{latest:{...exact,current:true,effectiveValidUntilMs:1400}}}],{connectionId:'account-a',prepared:pinned});
 assert.equal(shortened.effectiveValidUntilMs,1400);assert.equal(capacityUsageLabel(shortened,1500),'Historical capacity observation');
 assert.deepEqual(pinned,original,'current presentation fences do not mutate prepared evidence');
});
test('capacity presentation messages and dynamic states have Portuguese and Spanish labels',()=>{
 const sources=['capacity-observation.tsx','capacity-state.ts'].map(name=>readFileSync(new URL(`../src/${name}`,import.meta.url),'utf8')).join('\n');
 const existing=['i18n.ts','connection-locales.ts','ox016-locales.ts','diagnostic-history-locales.ts'].map(name=>readFileSync(new URL(`../src/${name}`,import.meta.url),'utf8')).join('\n');
 const catalogs=['chrome','shell-revision','settings-revision','activity-shell','files-shell','experience','session-revision','chat-controls'].flatMap(name=>Object.values(JSON.parse(readFileSync(new URL(`../../../prototypes/desktop/locales/${name}.json`,import.meta.url),'utf8'))));
 for(const match of sources.matchAll(/\bt\('([^']+)'\)/g))assert(capacityTranslations[match[1]]?.every(Boolean)||existing.includes(`'${match[1]}'`)||catalogs.some(value=>value.en===match[1]&&value.es),match[1]);
 for(const label of ['Not provided (null)','Uninterpretable value','Capacity not reported','Historical capacity observation','Included usage allowed','Included usage unavailable','Usage permission unknown','Primary window','Secondary window','Allowed at observation','Unavailable at observation'])assert(capacityTranslations[label]?.every(Boolean),label);
});
