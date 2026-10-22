import {test} from 'node:test';
import assert from 'node:assert/strict';
import {conversationAttempt,workerActionIssue} from '../src/worker-presentation.ts';

test('unsupported staged review explains preservation without inventing a result or action for other errors',()=>{
 assert.match(workerActionIssue('review-staged-source-outside-profile'),/preserved.*unchanged index/);
 assert.equal(workerActionIssue('review-unavailable'),undefined);
 assert.equal(workerActionIssue({reason:'review-staged-source-outside-profile'}),undefined);
});

test('conversation attempts sharing a Task retain distinct chronological labels and exact observed statuses',()=>{
 const first={runId:'same-run',taskId:'same-task',attemptId:'attempt-one',workerId:'implementation-worker'},second={...first,attemptId:'attempt-two',workerId:'correction-worker'};
 const workers=[{workerId:'implementation-worker',status:'verified'},{workerId:'correction-worker',status:'needs-work'}];
 const implementation=conversationAttempt(first,0,workers),correction=conversationAttempt(second,1,workers);
 assert.equal(implementation.number,1);assert.equal(correction.number,2);
 assert.equal(implementation.workerId,first.workerId);assert.equal(correction.workerId,second.workerId);
 assert.equal(implementation.statusLabel,'Checks passed');assert.equal(correction.statusLabel,'Checks need attention');
 assert.equal(first.taskId,second.taskId);
});

test('missing or paginated worker observations stay unknown instead of matching a Task substring or inferring a correction role',()=>{
 const work={runId:'run',taskId:'shared-task',attemptId:'second-attempt',workerId:'unobserved-worker'};
 const observed=conversationAttempt(work,1,[{workerId:'worker-for-shared-task',status:'verified'}]);
 assert.equal(observed.status,undefined);assert.equal(observed.statusLabel,'Not yet observed');assert.equal(observed.workerId,'unobserved-worker');
 const unlinked=conversationAttempt({runId:'run',taskId:'shared-task'},0,[{workerId:'worker-for-shared-task',status:'verified'}]);
 assert.equal(unlinked.workerId,undefined);assert.equal(unlinked.status,undefined);
 assert.equal('role' in observed,false,'chronological position does not establish implementation or correction provenance');
});
