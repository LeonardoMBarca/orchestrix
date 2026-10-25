import {test} from 'node:test';
import assert from 'node:assert/strict';
import {activeFileDestination,dockTab,preparationStillCurrent,taskFingerprint} from '../src/task-preparation.ts';
import {workerStage,workerInterruptionNotice} from '../src/worker-presentation.ts';

test('preparation belongs to the exact authorized draft, including commands and budget',()=>{
 const draft={conversationId:'conversation-one',projectId:'project-one',objective:'Fix the function',acceptanceCriteria:['Arithmetic passes'],repositoryGrant:'repo-grant',worktreeGrant:'work-grant',connectionId:'connection-one',contextFiles:[],checks:[{id:'check-one',executable:'trusted-executable',args:['--check'],timeoutSeconds:30}],strategy:'balanced',preset:'',model:'',effort:'',correctionBudget:0,fallback:'block'};
 const expected=taskFingerprint(draft);
 assert.equal(preparationStillCurrent(expected,structuredClone(draft)),true);
 for(const patch of [{projectId:'project-other'},{objective:'Changed objective'},{connectionId:'connection-two'},{connectionRevision:2},{allowMetadataRead:true},{correctionBudget:1},{fallback:'compatible'},{checks:[{...draft.checks[0],args:['--execute']}]}])assert.equal(preparationStillCurrent(expected,{...draft,...patch}),false);
});

test('New file follows the active authorized directory or requires explicit selection',()=>{
 const grants=[{id:'first-folder',directory:true},{id:'active-folder',directory:true},{id:'single-file',directory:false}];
 assert.equal(activeFileDestination(grants,'active-folder'),'active-folder');
 assert.equal(activeFileDestination(grants,'single-file'),'');
 assert.equal(activeFileDestination(grants,'unknown-grant'),'');
 assert.equal(activeFileDestination(grants),'');
 assert.equal(activeFileDestination([{id:'only-folder',directory:true}]),'');
});

test('dock keyboard selection supports Home, End and cyclic arrows without changing panel ownership',()=>{
 const items=['navigation','sessions'];
 assert.equal(dockTab(items,'sessions','Home'),'navigation');
 assert.equal(dockTab(items,'navigation','End'),'sessions');
 assert.equal(dockTab(items,'navigation','ArrowLeft'),'sessions');
 assert.equal(dockTab(items,'sessions','ArrowRight'),'navigation');
 assert.equal(dockTab(['sessions'],'sessions','Home'),'sessions');
 assert.equal(dockTab(items,'navigation','Enter'),null);
 assert.equal(dockTab([],'navigation','Home'),null);
});

test('saved and intermediate work never claim a runtime start, passing checks or completed review',()=>{
 assert.equal(workerStage('reserved').title,'Request saved');
 assert.match(workerStage('reserved').description,/not been confirmed/);
 assert.match(workerStage('supervisor-ready').description,/not been confirmed/);
 assert.equal(workerStage('checking').title,'Checking artifact');
 assert.equal(workerStage('verified').title,'Checks passed');
 assert.match(workerStage('verified').description,/review is a separate step/);
 assert.equal(workerStage('no-checks').title,'Captured without checks');
});

test('cancelled work explains only its recorded interruption reason without claiming checks or an actor',()=>{
 const deadline=Object.freeze({interruptionReason:'turn-deadline-elapsed'});
 const requested=Object.freeze({interruptionReason:'cancellation-requested'});
 assert.equal(workerInterruptionNotice('cancelled',deadline),'The turn reached its time limit and was interrupted. Inspect the preserved result; this does not imply passing checks or review.');
 assert.equal(workerInterruptionNotice('cancelled',requested),'Cancellation was requested. The record does not identify who requested it. Inspect any preserved artifacts.');
 assert.equal(deadline.interruptionReason,'turn-deadline-elapsed');
 assert.equal(requested.interruptionReason,'cancellation-requested');
});

test('legacy, null and unrecognized interruption details remain unknown without attribution',()=>{
 const unknown='No supported interruption reason is recorded for this attempt. The Cancelled status alone does not identify the cause.';
 for(const details of [undefined,null,{},[],42,'turn-deadline-elapsed',{interruptionReason:null},{interruptionReason:'unknown'},{interruptionReason:'future-reason'},{interruptionReason:42}]){
  assert.equal(workerInterruptionNotice('cancelled',details),unknown);
 }
});

test('interruption notices never replace a non-cancelled execution state',()=>{
 for(const status of ['reserved','preparing','supervisor-ready','dispatch-planned','runtime-observed','checking','verified','needs-work','no-checks','failed','uncertain','blocked','invalidated','inspection-required','set-aside','future-status']){
  for(const interruptionReason of ['turn-deadline-elapsed','cancellation-requested',null])assert.equal(workerInterruptionNotice(status,{interruptionReason}),undefined);
 }
});
