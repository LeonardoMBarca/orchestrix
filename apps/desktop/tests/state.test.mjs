import {test} from 'node:test';
import assert from 'node:assert/strict';
import {initialLayout,movePanel,selectActivity,resizeDock,moveActivity,uniqueEvents,sessionForFirstMessage,confirmsRevalidation,reviewMatchesArtifact,mergeCompactWorker} from '../src/state.ts';

test('one Navigation panel retains its chosen side through activity switches and moves',()=>{
 let state=movePanel(initialLayout,'navigation','right');
 state=selectActivity(state,'files');assert.equal(state.navigation,'right');assert.equal(state.activeRight,'navigation');assert.equal(state.sessions,'right');
 state=selectActivity(state,'studio');state=selectActivity(state,'connections');assert.equal(state.navigation,'right');assert.equal(state.activeRight,'navigation');
 state=movePanel(state,'navigation','left');state=selectActivity(state,'files');assert.equal(state.navigation,'left');assert.equal(state.sessions,'right');
 assert.equal(selectActivity(state,'chat'),state);
});
test('collapse/restore and rail reordering do not duplicate or move independent panels',()=>{
 const small=resizeDock(initialLayout,'left',30);assert.equal(small.leftWidth,0);
 const visible=selectActivity(small,'files');assert.equal(visible.leftWidth,224);assert.equal(visible.sessions,'right');
 const reordered=moveActivity(visible,'files',-1);assert.deepEqual(reordered.activityOrder,['files','chat','studio','connections']);assert.equal(new Set(reordered.activityOrder).size,4);
 assert.equal(resizeDock(visible,'right',900).rightWidth,480);
});
test('reconnect pages deduplicate event identity and preserve monotonic order',()=>{
 assert.deepEqual(uniqueEvents([{sequence:1},{sequence:2}],[{sequence:2},{sequence:4},{sequence:3}]).map(e=>e.sequence),[1,2,3,4]);
 assert.equal(uniqueEvents([],Array.from({length:250},(_,i)=>({sequence:i}))).length,200);
});
test('HOME, whitespace and drafts create no conversation identity',()=>{
 let calls=0;const id=()=>{calls++;return 'conversation-one';};
 assert.equal(sessionForFirstMessage('  \n',null,id),null);assert.equal(calls,0);
 assert.equal(sessionForFirstMessage('Actual instruction',null,id),'conversation-one');assert.equal(calls,1);
 assert.equal(sessionForFirstMessage('Follow-up','existing',id),'existing');assert.equal(calls,1);
});

test('only the matching completed revalidation restores exact-current evidence',()=>{
 const pin={commandId:'check-one',workerId:'worker',revision:'hash-one',epoch:2},event={commandId:'check-one',workerId:'worker',state:'finished'},current={workerId:'worker',revision:'hash-one',status:'verified'};
 assert.equal(confirmsRevalidation(pin,event,current,2),true);
 for(const wrong of [{...event,commandId:'other'},{...event,state:'failed'},{...event,state:'accepted'},{...event,workerId:'other'}])assert.equal(confirmsRevalidation(pin,wrong,current,2),false);
 assert.equal(confirmsRevalidation(pin,event,{...current,revision:'new-hash'},2),false);
 assert.equal(confirmsRevalidation(pin,event,{...current,status:'invalidated'},2),false);
 assert.equal(confirmsRevalidation(pin,event,current,3),false);
});
test('review approval and correction remain scoped to the exact worker and artifact version',()=>{
 const old={worker_id:'worker',artifact_revision:'old'};assert.equal(reviewMatchesArtifact(old,'worker','new'),false);assert.equal(reviewMatchesArtifact(old,'other-worker','old'),false);assert.equal(reviewMatchesArtifact(old,'worker','old'),true);
});

test('compact daemon status preserves inspected checks, review findings and immutable artifact',()=>{
 const full={summary:{workerId:'worker-real-shape',status:'verified',details:{artifactRevision:'artifact-hash',checks:[{id:'fixture-check',result:'passed',exit_code:0}]}},artifact:{id:'artifact-id',revision:'artifact-hash',immutable:true},reviews:[{id:'review-other-id',worker_id:'worker-real-shape',status:'rejected',artifact_revision:'artifact-hash',result:{findings:[{id:'finding-one',path:'fixture.txt'}]}}]};
 const compact={workerId:'worker-real-shape',status:'verified'};
 const merged=mergeCompactWorker(full,compact);assert.deepEqual(merged.summary.details.checks,[{id:'fixture-check',result:'passed',exit_code:0}]);assert.equal(merged.reviews[0].result.findings[0].id,'finding-one');assert.equal(merged.artifact.revision,'artifact-hash');
 assert.equal(mergeCompactWorker(full,{workerId:'different-worker',status:'failed'}),full);
 assert.equal(mergeCompactWorker(full,{...compact,status:'artifact-invalidated'}).summary.status,'artifact-invalidated');assert.deepEqual(mergeCompactWorker(full,compact),mergeCompactWorker(merged,compact));
});
