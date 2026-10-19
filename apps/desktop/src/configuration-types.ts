import type {Preferences} from './protocol';

export type DefinitionBinding={id:string,version:number};
export type ExecutorRole='implementation'|'review';
export type DefinitionKind='mode'|'profile';
export type DefinitionRequirements={subscription_only?:boolean,forbid_network?:boolean,read_only?:boolean,require_review?:boolean,required_capabilities?:string[],required_checks?:string[],minimum_reasoning?:string|null,minimum_strength?:number,forced_model?:string|null,forced_connection?:string|null,max_concurrency?:number|null,max_context_bytes?:number|null};
type DefinitionIdentity=DefinitionBinding&{editRevision:number,origin:'native'|'local',base?:DefinitionBinding|null,archived:boolean,name:string,instructions:string,preferences:Preferences,requirements:DefinitionRequirements};
export type ModeDefinition=DefinitionIdentity&{purpose:string,roles:{role:ExecutorRole,profile:DefinitionBinding}[],outputs:string[],criteria:string[],executionSupport:'manual-worker'|'artifact-flow-unproven'};
export type ProfileDefinition=DefinitionIdentity&{role:ExecutorRole,outputLanguage:string,conventions:string[],specialty:string};
export type ScopedPolicy={scope:'user'|'project',projectId?:string|null,preferences:Preferences,requirements:DefinitionRequirements,removed:boolean,version:number,editRevision:number};
export type ConfigurationCatalog={schemaVersion:1,revision:number,executionFingerprint?:string,modes:ModeDefinition[],profiles:ProfileDefinition[],scopes:ScopedPolicy[]};
export type ConfigurationPreview={schemaVersion:1,revision:number,policy:unknown,decision:unknown,definitions:{mode:DefinitionBinding|null,profile:DefinitionBinding|null,reviewProfile:DefinitionBinding|null}|null,fingerprint:string,warnings:string[]};
export type ConfigurationRequest=
 |{type:'catalog'}
 |{type:'get',kind:DefinitionKind,id:string,version:number}
 |{type:'save-mode',commandId:string,expectedRevision:number,definition:ModeDefinition}
 |{type:'save-profile',commandId:string,expectedRevision:number,definition:ProfileDefinition}
 |{type:'restore'|'archive',kind:DefinitionKind,id:string,commandId:string,expectedRevision:number}
 |{type:'save-scope',commandId:string,expectedRevision:number,scope:ScopedPolicy}
 |{type:'preview',mode:DefinitionBinding,profile:DefinitionBinding,projectId?:string,modeDraft?:ModeDefinition,profileDraft?:ProfileDefinition,scopeDraft?:ScopedPolicy};
export type ConfigurationSend=<T>(request:ConfigurationRequest)=>Promise<T>;
export type ConfigurationDraft=ModeDefinition|ProfileDefinition|ScopedPolicy;
export type DefinitionSelection={mode:DefinitionBinding,profile:DefinitionBinding};
export type DefinitionChange={field:string,before:string,after:string};

export function copyConfiguration<T>(value:T):T{return structuredClone(value);}
export function configurationEqual(left:unknown,right:unknown){return JSON.stringify(left)===JSON.stringify(right);}
export function configurationKey(kind:DefinitionKind|'scope',value:{id?:string,scope?:string,projectId?:string|null}){return kind==='scope'?`scope:${value.scope}:${value.projectId??''}`:`${kind}:${value.id}`;}
// Keep blank/trailing lines while typing. Removing them on each keystroke would
// erase Enter before the user can type the next item.
export function configurationLines(value:string){return value.split(/\r?\n/);}
export function cleanConfigurationDraft<T extends ConfigurationDraft>(value:T):T{
 const next=copyConfiguration(value);
 for(const field of ['outputs','criteria','conventions'] as const)if(field in next){const record=next as unknown as Record<string,unknown>;record[field]=(record[field] as string[]).map(line=>line.trim()).filter(Boolean);}
 return next;
}
export function configurationDiff(before:ConfigurationDraft|undefined,after:ConfigurationDraft):DefinitionChange[]{
 const keys=Object.keys(after).filter(key=>!['version','editRevision','origin','base','executionSupport'].includes(key));
 const readable=(value:unknown):string=>value===undefined||value===null?'—':typeof value==='string'?value:typeof value==='boolean'?String(value):Array.isArray(value)?value.map(item=>typeof item==='string'?item:JSON.stringify(item)).join('\n'):Object.entries(value as Record<string,unknown>).filter(([,v])=>v!==null&&v!==undefined).map(([key,v])=>`${key}: ${Array.isArray(v)?v.join(', '):String(v)}`).join('\n');
 return keys.filter(key=>!configurationEqual((before as unknown as Record<string,unknown>|undefined)?.[key],(after as unknown as Record<string,unknown>)[key])).map(key=>({field:key,before:readable((before as unknown as Record<string,unknown>|undefined)?.[key]),after:readable((after as unknown as Record<string,unknown>)[key])}));
}
export function configurationError(error:unknown){const value=String(error);if(/revision|conflict|stale/i.test(value))return 'Another window saved a newer version. Your draft is preserved. Compare or reload before saving.';return value;}
export function selectedDefinition<T extends DefinitionBinding>(definitions:T[],selection:DefinitionBinding):T|undefined{return definitions.find(value=>value.id===selection.id&&value.version===selection.version);}
export function defaultDefinitionSelection(catalog:ConfigurationCatalog):DefinitionSelection{
 const mode=catalog.modes.find(value=>value.id==='development'&&!value.archived)??catalog.modes[0],profile=catalog.profiles.find(value=>value.id==='implementation-engineer'&&!value.archived)??catalog.profiles[0];
 return {mode:{id:mode.id,version:mode.version},profile:{id:profile.id,version:profile.version}};
}
export function configurationDraftValid(kind:DefinitionKind|'scope',draft:ConfigurationDraft):boolean{
 if(kind==='scope'){const value=draft as ScopedPolicy;return value.scope==='user'||!!value.projectId;}
 const value=draft as ModeDefinition|ProfileDefinition;
 if(!/^[a-z0-9][a-z0-9-]{0,95}$/.test(value.id)||!value.name.trim()||value.name.length>120||value.instructions.length>8192)return false;
 if(kind==='mode'){const mode=value as ModeDefinition;return !!mode.purpose.trim()&&mode.outputs.some(line=>line.trim())&&mode.criteria.some(line=>line.trim())&&mode.roles.length===2&&new Set(mode.roles.map(role=>role.role)).size===2&&mode.roles.every(role=>!!role.profile.id&&role.profile.version>0);}
 return ['implementation','review'].includes((value as ProfileDefinition).role);
}
