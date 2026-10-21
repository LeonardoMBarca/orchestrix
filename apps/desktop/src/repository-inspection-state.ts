import type {RepositoryInspectionResult,RepositoryPreflight} from './protocol';

export function repositoryInspectionReady(value:RepositoryInspectionResult|null,directoryGrant:string){
 return !!directoryGrant&&value?.directoryGrant===directoryGrant&&value.report.schemaVersion===1&&value.report.profile==='regular-utf8-v1'&&value.report.modelCalled===false&&value.report.authority==='inspection-only-revalidate-before-effects'&&value.report.complete===true&&value.report.compatible===true;
}
export function repositoryResponseSupported(report:RepositoryPreflight){
 return report.schemaVersion===1&&report.profile==='regular-utf8-v1'&&report.modelCalled===false&&report.authority==='inspection-only-revalidate-before-effects';
}
export function repositoryReplyCurrent(requestGrant:string,currentGrant:string,version:number,currentVersion:number,mounted:boolean){return mounted&&requestGrant===currentGrant&&version===currentVersion;}
export function repositoryOutcome(report:RepositoryPreflight){return !report.complete?'Inspection incomplete':report.compatible?'Compatible with the worker profile':'Repository blocked for this profile';}
export function repositoryBlockerLabel(code:string){return ({'inventory-limit':'File inventory limit reached','file-size-limit':'File size limit reached','read-bytes-limit':'Read budget reached','executable-mode':'Executable file mode','symbolic-link':'Symbolic link','submodule':'Submodule','binary-content':'Binary content','non-utf8':'Non-UTF-8 content','secret-pattern':'Sensitive content pattern','excluded-content':'Excluded content','base-not-head':'The base differs from HEAD','unsupported-path':'Unsupported path','unsupported-mode':'Unsupported file mode','checkout-omissions':'Checkout content is omitted by this profile','head-changed':'HEAD changed during inspection','checkout-changed':'Checkout changed during inspection','unsupported-attributes':'Unsupported Git attributes','unsupported-base-attributes':'Pinned base has unsupported Git attributes','base-attribute-inspection-unavailable':'Git could not inspect attributes in the pinned base'} as Record<string,string>)[code]??'Repository profile restriction';}
