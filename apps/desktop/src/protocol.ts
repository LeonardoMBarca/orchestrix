import {invoke} from '@tauri-apps/api/core';
import type {Language} from './i18n';
export type Side='left'|'right';
export type Activity='chat'|'files'|'studio'|'connections';
export type Preferences={strategy?:string|null,preset?:string|null,model_id?:string|null,reasoning?:string|null,connection_id?:string|null,[key:string]:unknown};
export type Policy={schema_version:number,revision:number,mode:{id:string,version:number},profile:{id:string,version:number},mode_defaults:Preferences,mode_requirements:Record<string,unknown>,profile_defaults:Preferences,layers:unknown[],model_ranks:unknown[],observation_ttls:Record<string,number>};
export type Settings={language:Language|'pt-BR',theme:string,density:'compact'|'comfortable',textScale:number,closeBehavior:'continue'|'suspend',systemInstructions:string,policy:Policy};
export const defaultSettings:Settings={language:'en',theme:'studio',density:'compact',textScale:100,closeBehavior:'continue',systemInstructions:'',policy:{schema_version:1,revision:1,mode:{id:'development',version:1},profile:{id:'implementation-engineer',version:1},mode_defaults:{},mode_requirements:{subscription_only:true,forbid_network:false,read_only:false,required_capabilities:[],minimum_reasoning:null,minimum_strength:0,forced_model:null,forced_connection:null,max_concurrency:null,max_context_bytes:null,required_checks:[],require_review:false},profile_defaults:{},layers:[],model_ranks:[],observation_ttls:{identity_billing_ms:300000,availability_ms:60000,capability_configuration_ms:900000}}};
export type Message={id:string,role:string,text:string,ordinal:number};
export type Conversation={id:string,title:string,projectId?:string|null,messageCount:number};
export type ConversationWork={runId:string,taskId?:string,attemptId?:string,workerId?:string,revision?:string};
export type ConversationDetail={id:string,projectId?:string|null,messages:Message[],draft?:string|null,nextOrdinal:number,more:boolean,work:ConversationWork[]};
export type Project={id:string,name:string,directory?:string};
export type DiagnosticStage={stage:string,state:string,source?:string,reason?:string,observedAtMs?:number};
export type CapacityField<T>={state:'reported',value:T}|{state:'missing'|'null'|'invalid'};
export type CapacityWindow={slot:'primary'|'secondary',state:'reported'|'missing'|'null'|'invalid',unit:'percent-of-window',usedPercent:CapacityField<number>,remainingPercent?:{value:number,origin:'derived',basis:'100-minus-used-percent'}|null,durationMinutes:CapacityField<number>,resetsAtSeconds:CapacityField<number>};
export type CapacityBucket={reference:string,limitId:CapacityField<string>,name:CapacityField<string>,modelDisplayAlias:CapacityField<string>,reachedReason:CapacityField<string>,windows:CapacityWindow[]};
export type CapacityObservation={schemaVersion:1,source:'codex-account-rate-limits',connectionId:string,connectionRevision:number,runtimeVersion:string,observedAtMs:number,synthetic:boolean,ordinaryUsage:CapacityField<boolean>,bucketView:'multi'|'legacy'|'not-reported',buckets:CapacityBucket[],legacyBucket?:CapacityBucket|null,completeness:'complete'|'partial',coverage:'reported-subset',issues:string[],capacityGroupIds:string[]};
export type PreparedCapacity={diagnosticId:string,observation:CapacityObservation,current:boolean,effectiveValidUntilMs:number|null,identityChanged:boolean};
export type DiagnosticRun={id:string,state:string,connectionRevision:number,startedAtMs?:number,finishedAtMs?:number,observedAtMs?:number,validUntilMs?:number,effectiveValidUntilMs?:number,nextAllowedAtMs?:number,complete?:boolean,cached?:boolean,current?:boolean,errorCode?:string,identityChanged?:boolean,provenance?:string,stages?:DiagnosticStage[],descriptor?:Record<string,unknown>,capacity?:CapacityObservation,[key:string]:unknown};
export type ConnectionDiagnostic={latest?:DiagnosticRun|null,lastSuccessful?:DiagnosticRun|null,lastSuccessfulAtMs?:number,identityChanged?:boolean,history?:DiagnosticRun[],observationRenewedOnRead?:false,catalog?:Record<string,any>|null,catalogDiagnosticId?:string|null,catalogObservedAtMs?:number|null,catalogLabels?:{id:string,label:string,nativeEfforts:string[]}[]|null,controlObservations?:string[]|null,structuredResultSupport?:boolean|null};
export type DiagnosticStorage={operationalRows:number,archivedRows:number,totalRows:number,usedBytes:number,reservedBytes:number,operationalLimitRows:number,maximumHistoryBytes:number,globalUsedAndReservedBytes?:number,maximumGlobalHistoryBytes?:number,reservedTerminalRows?:number,pageLimit?:number,maximumPageBytes?:number,maximumDetailBytes?:number,storageBlocked?:boolean,blockedReason?:string|null,nextAction?:string|null,[key:string]:unknown};
export type DiagnosticHistoryItem={sequence:number,ordinal:number,archived:boolean,diagnostic:DiagnosticRun,digest:string,bodyBytes:number};
export type DiagnosticHistoryPage={connectionId:string,items:DiagnosticHistoryItem[],nextCursor:string|null,highWaterSequence:number,storage:DiagnosticStorage};
export type DiagnosticObservation=DiagnosticHistoryItem&{connectionId:string,historical:true,validity:{captured:Record<string,unknown>,digest:string,refreshRevision?:number,policyVersion?:number}|null};
export type Connection={id:string,label:string,name?:string,revision?:number,bindingRevision?:number,provider:string,mode:string,status:string,enabled?:boolean,linked?:boolean,selected?:boolean,adapterId?:string,runtimeVersion?:string,identityAssurance?:string,identityStatus?:string,diagnostics?:ConnectionDiagnostic,models?:{id:string,name:string,efforts?:string[],nativeEfforts?:string[]}[]};
export type NativeCandidate={id:string,kind:'executable'|'home',location:string,source:string};
export type RuntimeVersion={version:string,executableSha256:string,supported:boolean,inferenceUsed:false,authenticationRead:false};
export type WorkerSummary={workerId:string,status:string};
export type Cursor={database:string,sequence:number};
export type DaemonSnapshot={state:{suspended:boolean,generation:number,cursor:Cursor},workers:WorkerSummary[],workerCount:number,nextWorker?:string|null,workerFactsHistorical:true,activeWorkerId?:string|null,activeCancellable?:boolean,activeOperationKind?:string|null,features?:string[]};
export type Workspace={revision:number,projects:Project[],conversations:Conversation[],settings:Settings,connections:Connection[],eventCursor:Cursor,admissionsSuspended:boolean};
export type Bootstrap=Workspace&{nativeTestMode?:boolean,daemon?:DaemonSnapshot,runtime?:{available:boolean,version?:string,reason?:string}};
export type DesktopRequest=
 |{type:'configurations',request:import('./configuration-types').ConfigurationRequest}
 |{type:'prepare-policy-export'}
 |{type:'preview-policy-import',importRef:string,mappings:ImportMappings}
 |{type:'apply-policy-import',commandId:string,expectedRevision:number,importRef:string,mappings:ImportMappings,expectedPreviewDigest:string,confirm:boolean}
 |{type:'release-policy-import',importRef:string}
 |{type:'configure-connection-refresh',commandId:string,id:string,expectedRevision:number,expectedRefreshRevision:number,settings:RefreshSettings}
 |{type:'connection-refresh-status',id?:string|null}
 |{type:'bootstrap'}
 |{type:'test-fixture'}
 |{type:'conversation',conversationId:string,afterOrdinal?:number,limit?:number}
 |{type:'create-project',commandId:string,expectedRevision:number,id:string,name:string,directoryGrant:string}
 |{type:'send-message',commandId:string,expectedRevision:number,conversationId:string,projectId?:string|null,content:string}
 |{type:'save-draft',commandId:string,expectedRevision:number,conversationId:string,content:string}
 |{type:'attach-conversation',commandId:string,expectedRevision:number,conversationId:string,projectId:string}
 |{type:'save-settings',commandId:string,expectedRevision:number,settings:Settings}
 |{type:'inspect-worker',workerId:string}
 |{type:'prepare-integration',commandId:string,expectedRevision:number,workerId:string,artifactRevision:string,targetGrant:string,integrationGrant:string,authorizeChecks:boolean}
 |{type:'inspect-integration',integrationId:string}
 |{type:'revalidate-integration',commandId:string,integrationId:string}
 |{type:'approve-integration',commandId:string,expectedRevision:number,integrationId:string,candidateRevision:string,authorize:boolean}
 |{type:'apply-integration',commandId:string,expectedRevision:number,integrationId:string,candidateRevision:string,authorize:boolean}
 |{type:'artifact-text',artifactId:string,revision:string,path:null|string,offset:number,limit:number}
 |{type:'prepare-task',draft:TaskDraft}
 |{type:'submit-task',commandId:string,preparationId:string,authorizeRuntime:boolean,authorizeChecks:boolean}
 |{type:'save-connection',commandId:string,label:string,version:string,executableGrant:string,homeGrant:string}
 |{type:'detect-codex-candidates'}
 |{type:'inspect-runtime-version',executableRef:string}
 |{type:'link-connection',commandId:string,id?:string,expectedRevision?:number,name:string,version:string,executableRef:string,homeRef:string,executableSha256:string,allowMetadataRead:boolean}
 |{type:'update-connection',commandId:string,id:string,expectedRevision:number,change:'rename'|'select'|'disable'|'enable'|'unlink'|'reconfirm-identity',name?:string,diagnosticId?:string}
 |{type:'diagnose-connection',commandId:string,id:string,expectedRevision:number,allowMetadataRead:boolean}
 |{type:'cancel-connection-diagnostic',commandId:string,id:string,expectedRevision:number,diagnosticId:string}
 |{type:'connection-diagnostics',id:string}
 |{type:'connection-diagnostic-history',id:string,cursor?:string|null,limit?:number}
 |{type:'connection-diagnostic-observation',id:string,diagnosticId:string,ordinal?:number|null}
 |{type:'control',action:'suspend'|'resume'|'cancel'|'recheck-recovery'|'revalidate-artifact',commandId:string,workerId?:string}
 |{type:'intervene',commandId:string,workerId:string,expectedRevision:number,action:'record-inspection'|'set-aside',authorize:boolean}
 |{type:'set-aside-review',commandId:string,reviewId:string,expectedRevision:number,authorize:boolean}
 |{type:'recovery'}
 |{type:'request-review',commandId:string,workerId:string,authorizeRuntime:boolean}
 |{type:'authorize-correction',commandId:string,workerId:string,reviewId:string,authorizeRuntime:boolean};
export type CheckDraft={id:string,executable:string,args:string[],timeoutSeconds:number};
export type TaskDraft={mode?:{id:string,version:number},profile?:{id:string,version:number},conversationId:string,projectId?:string|null,repositoryGrant:string,worktreeGrant:string,connectionId:string,connectionRevision?:number,objective:string,acceptanceCriteria:string[],contextFiles:{grant:string,relative:string}[],checks:CheckDraft[],strategy:string,preset:string,model:string,effort:string,correctionBudget:number,fallback:string,allowMetadataRead?:boolean};
export type PreparedTask={id:string,task:unknown,configuration?:import('./configuration-types').ConfigurationPreview,decision?:unknown,context?:unknown,warning?:string,capacity?:PreparedCapacity};
export type RefreshSettings={schemaVersion:1,mode:'manual'|'automatic',intervalMs:number,validityMs:{identity:number,configuration:number,availability:number,models:number},recurringConsent:boolean};
export type RefreshStatus={connectionId:string,connectionRevision?:number,bindingRevision?:number,refreshRevision:number,requested:RefreshSettings,effective?:RefreshSettings,reason:string,nextOpportunityAtMs?:number|null,activeDiagnosticId?:string|null,activeSource?:'periodic'|'foreground'|null,recurringAuthorized:boolean,sourceTtlMs?:number,policyValidityMs?:RefreshSettings['validityMs'],consentScope?:string,consentPortable?:false,storage?:DiagnosticStorage,limits?:{globalConcurrentReads:number,perConnectionConcurrentReads:number,minimumIntervalMs:number,maximumIntervalMs:number,metadataDeadlineMs:number,maximumBackoffMs:number,retainedDiagnosticRows:number,remainingDiagnosticRows:number}};
export type ImportMappings={connections:{alias:string,connectionId:string,bindingRevision:number}[],models:{alias:string,connectionAlias:string,modelId:string}[],mode?:{id:string,version:number}|null,profile?:{id:string,version:number}|null};
export type ImportPreview={schemaVersion:number,expectedRevision:number,documentDigest:string,previewDigest:string,canApply:boolean,issues:{code:string,reference?:string|null}[],changes:string[],connectionReferences:{alias:string}[],modelReferences:{alias:string,sourceModelId:string,connectionAlias?:string|null}[],definitionReferences:Record<string,unknown>,mappings:ImportMappings};
export async function request<T>(request:DesktopRequest):Promise<T>{return invoke<T>('desktop_request',{request});}
export async function poll(cursor:Cursor):Promise<{daemon:DaemonSnapshot,cursor:Cursor,events:{sequence:number,kind:string,body:unknown}[],resyncRequired:boolean}>{return invoke('desktop_poll',{cursor});}
export const commandId=()=>`desktop-${crypto.randomUUID()}`;
