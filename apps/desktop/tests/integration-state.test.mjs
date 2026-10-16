import {test} from 'node:test';
import assert from 'node:assert/strict';
import {canPrepareIntegration,confirmsIntegrationRevalidation,integrationAction,integrationFooter,integrationPreparationBarriers,integrationStage,integrationRecoveryEvidence} from '../src/integration-state.ts';

const source={workerId:'worker-one',revision:'artifact-one',status:'verified',stale:false,reviews:[{status:'accepted',worker_id:'worker-one',artifact_revision:'artifact-one'}]};

test('integration recovery retains exact provenance and uncertainty without exposing ownership bindings',()=>{
 const summary={id:'integration-uncertain',worker_id:'worker-one',run_id:'run-one',task_id:'task-one',source_artifact_revision:'artifact-one',status:'uncertain',input_base:'base-commit',result_revision:null,candidate_revision:null,checks:null,reason:'git-inspection-required',delivered:false,profile:'single-code-task-clean-target-v1',target:{directory:'C:/fixture/target',head:'base-commit',symbolic_ref:'refs/heads/main',authority:'never-render-ownership-authority'},guardian:{authority:'never-render-ownership-authority'},authority:'never-render-ownership-authority'};
 const evidence=integrationRecoveryEvidence(summary);
 assert.equal(evidence.id,summary.id);assert.equal(evidence.worker_id,summary.worker_id);assert.equal(evidence.run_id,summary.run_id);assert.equal(evidence.task_id,summary.task_id);
 assert.equal(evidence.source_artifact_revision,summary.source_artifact_revision);assert.equal(evidence.target.directory,summary.target.directory);assert.equal(evidence.input_base,summary.input_base);
 assert.equal(evidence.status,'uncertain');assert.equal(evidence.reason,'git-inspection-required');assert.equal(evidence.checks,null);assert.equal(evidence.result_revision,null);assert.equal(evidence.candidate_revision,null);assert.equal(evidence.delivered,false);
 assert.equal(integrationStage(evidence.status),'Outcome uncertain');
 assert.doesNotMatch(JSON.stringify(evidence),/guardian|authority|never-render/);
 assert.equal(integrationAction(evidence,source,false),null,'a recovery summary cannot authorize apply or approval');
 const checked=integrationRecoveryEvidence({...summary,checks:[{id:'check-one',result:'passed',artifact_revision:'artifact-one',exit_code:0,tree_confirmed:true,streams_closed:true,output:'never-render-ownership-authority',guardian:{authority:'never-render-ownership-authority'}}]});
 assert.equal(checked.checks[0].result,'passed');assert.equal(checked.checks[0].exit_code,0);assert.doesNotMatch(JSON.stringify(checked),/guardian|authority|never-render/);
});
test('integration preparation requires exact worker/artifact accepted review, current evidence and idle admission',()=>{
 assert.equal(canPrepareIntegration(source,false),true);
 for(const patch of [{status:'reserved'},{status:'invalidated'},{stale:true},{revision:'artifact-two'},{workerId:'worker-two'},{reviews:[{...source.reviews[0],status:'rejected'}]},{reviews:[]}])assert.equal(canPrepareIntegration({...source,...patch},false),false);
 assert.equal(canPrepareIntegration(source,true),false);
});
test('candidate approval and application are separate exact-version actions; historical delivery never offers apply',()=>{
 const candidate={worker_id:'worker-one',source_artifact_revision:'artifact-one',status:'prepared',candidate_revision:'candidate-one',delivered:false};
 assert.equal(integrationAction(candidate,source,false),'approve');
 assert.equal(integrationAction({...candidate,status:'approved'},source,false),'apply');
 for(const patch of [{worker_id:'another-worker'},{source_artifact_revision:'old-artifact'},{candidate_revision:''},{delivered:true},{status:'applying'},{status:'uncertain'},{status:'invalidated'},{status:'delivered'}])assert.equal(integrationAction({...candidate,...patch},source,false),null);
 assert.equal(integrationAction(candidate,source,true),null);
 assert.equal(integrationStage('reserved'),'Integration request saved');
 assert.equal(integrationStage('approved'),'Candidate approved');
 assert.equal(integrationStage('delivered'),'Delivered');
});

test('recorded result commits and pending or uncertain integrations never release preparation after reopening',()=>{
 const integration={worker_id:'worker-one',source_artifact_revision:'artifact-one',status:'blocked',result_revision:'recorded-result'};
 for(const status of ['blocked','invalidated'])assert.equal(integrationPreparationBarriers([{...integration,status}],source).recordedResult,true);
 assert.equal(integrationPreparationBarriers([{...integration,source_artifact_revision:'historical-artifact'}],source).recordedResult,true,'a recorded result remains owned by this task even when source bytes differ');
 for(const status of ['reserved','preparing','materialized','checking','checked','commit-planned','integrated','applying','applied','uncertain'])assert.equal(integrationPreparationBarriers([{...integration,result_revision:null,status}],source).recoveryRequired,true);
 assert.deepEqual(integrationPreparationBarriers([{...integration,worker_id:'another-worker'}],source),{recordedResult:false,recoveryRequired:false,candidateExists:false});
 assert.deepEqual(integrationPreparationBarriers([{...integration,result_revision:null,status:'blocked'}],source),{recordedResult:false,recoveryRequired:false,candidateExists:false});
});

test('only the original deterministic revalidation and unchanged save epochs withdraw candidate staleness',()=>{
 const pin={commandId:'revalidation-one',integrationId:'candidate-one',candidateRevision:'exact-candidate',epoch:2,unknownEpoch:1};
 const event={commandId:'revalidation-one',workerId:'candidate-one',state:'finished'};
 const current={id:'candidate-one',candidate_revision:'exact-candidate',status:'approved'};
 assert.equal(confirmsIntegrationRevalidation(pin,event,current,2,1),true);
 assert.equal(confirmsIntegrationRevalidation(pin,event,{...current,status:'invalidated'},2,1),true);
 for(const patch of [{commandId:'another-command'},{workerId:'another-candidate'},{state:'revalidation-unavailable'}])assert.equal(confirmsIntegrationRevalidation(pin,{...event,...patch},current,2,1),false);
 assert.equal(confirmsIntegrationRevalidation(pin,event,{...current,candidate_revision:'different-candidate'},2,1),false);
 assert.equal(confirmsIntegrationRevalidation(pin,event,current,3,1),false);
 assert.equal(confirmsIntegrationRevalidation(pin,event,current,2,2),false,'a newer unclassified save cannot inherit an old validation');
});

test('worker footer follows exact candidate/application evidence and preserves historical delivery after source edits',()=>{
 const candidate={id:'integration-one',worker_id:'worker-one',source_artifact_revision:'artifact-one',status:'prepared',delivered:false};
 assert.equal(integrationFooter([candidate],source),'Candidate prepared · Awaiting explicit approval');
 assert.equal(integrationFooter([{...candidate,status:'approved'}],source),'Candidate approved · Awaiting explicit application');
 assert.equal(integrationFooter([{...candidate,status:'approved'}],source,true),'Candidate verification required before approval or application.');
 const delivered={...candidate,status:'delivered',delivered:true};
 assert.equal(integrationFooter([delivered],source,true),'Historical delivery recorded for this exact artifact.');
 assert.equal(integrationFooter([{...delivered,source_artifact_revision:'another-artifact'}],source),null);
 assert.equal(integrationFooter([{...delivered,delivered:false}],source),null);
 assert.equal(integrationFooter([{...candidate,status:'approved'},{...candidate,status:'invalidated'}],source),null,'the selected newer observation replaces the same historical integration identity');
});
