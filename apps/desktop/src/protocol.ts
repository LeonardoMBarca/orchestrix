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
export type ConversationDetail={id:string,projectId?:string|null,messages:Message[],draft?:string|null,nextOrdinal:number,more:boolean,work:{runId:string,taskId?:string,attemptId?:string,workerId?:string}[]};
export type Project={id:string,name:string,directory?:string};
export type Connection={id:string,label:string,provider:string,mode:string,status:string,models?:{id:string,name:string,efforts?:string[]}[]};
export type WorkerSummary={workerId:string,status:string};
export type Cursor={database:string,sequence:number};
export type DaemonSnapshot={state:{suspended:boolean,generation:number,cursor:Cursor},workers:WorkerSummary[],workerCount:number,nextWorker?:string|null,workerFactsHistorical:true,activeWorkerId?:string|null,activeCancellable?:boolean,activeOperationKind?:string|null,features?:string[]};
export type Workspace={revision:number,projects:Project[],conversations:Conversation[],settings:Settings,connections:Connection[],eventCursor:Cursor,admissionsSuspended:boolean};
export type Bootstrap=Workspace&{nativeTestMode?:boolean,daemon?:DaemonSnapshot,runtime?:{available:boolean,version?:string,reason?:string}};
export type DesktopRequest=
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
 |{type:'control',action:'suspend'|'resume'|'cancel'|'recheck-recovery'|'revalidate-artifact',commandId:string,workerId?:string}
 |{type:'intervene',commandId:string,workerId:string,expectedRevision:number,action:'record-inspection'|'set-aside',authorize:boolean}
 |{type:'set-aside-review',commandId:string,reviewId:string,expectedRevision:number,authorize:boolean}
 |{type:'recovery'}
 |{type:'request-review',commandId:string,workerId:string,authorizeRuntime:boolean}
 |{type:'authorize-correction',commandId:string,workerId:string,reviewId:string,authorizeRuntime:boolean};
export type CheckDraft={id:string,executable:string,args:string[],timeoutSeconds:number};
export type TaskDraft={conversationId:string,projectId?:string|null,repositoryGrant:string,worktreeGrant:string,connectionId:string,objective:string,acceptanceCriteria:string[],contextFiles:{grant:string,relative:string}[],checks:CheckDraft[],strategy:string,preset:string,model:string,effort:string,correctionBudget:number,fallback:string};
export type PreparedTask={id:string,task:unknown,decision?:unknown,context?:unknown,warning?:string};
export async function request<T>(request:DesktopRequest):Promise<T>{return invoke<T>('desktop_request',{request});}
export async function poll(cursor:Cursor):Promise<{daemon:DaemonSnapshot,cursor:Cursor,events:{sequence:number,kind:string,body:unknown}[],resyncRequired:boolean}>{return invoke('desktop_poll',{cursor});}
export const commandId=()=>`desktop-${crypto.randomUUID()}`;
