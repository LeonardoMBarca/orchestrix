import {test} from 'node:test';
import assert from 'node:assert/strict';
import {startupObservation,startupLabel} from '../src/startup-state.ts';
const pending={state:'recovering',phase:'worker-recovery',phaseSource:'own-startup-report',hostElapsedMs:1200,daemonElapsedMs:900,automaticFollowUp:true,followUpBudgetMs:15000};
test('startup observations never grant readiness; follow-up is finite and needs explicit observation',()=>{
 assert.equal(startupObservation({state:{generation:1}}),null);
 assert.equal(startupObservation({startup:pending}).automaticFollowUp,true);
 assert.equal(startupLabel(pending),'Recovering saved work…');
 for(const patch of [{state:'blocked'},{state:'failed'},{hostElapsedMs:15000},{hostElapsedMs:16000}])assert.equal(startupObservation({startup:{...pending,...patch}}).automaticFollowUp,false);
 for(const patch of [{state:'ready'},{phase:'invented-phase'},{hostElapsedMs:-1},{followUpBudgetMs:300001},{automaticFollowUp:'yes'}])assert.throws(()=>startupObservation({startup:{...pending,...patch}}),/invalid-startup/);
 assert.equal(startupObservation({startup:{...pending,state:'blocked',explicitConnectAvailable:true}}).explicitConnectAvailable,true);
 assert.throws(()=>startupObservation({startup:{...pending,explicitConnectAvailable:'yes'}}),/invalid-startup/);
});
