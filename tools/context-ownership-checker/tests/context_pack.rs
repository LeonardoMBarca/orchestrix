//! The exact canonical task must fit the existing Core preparation contract.
use orchestrix_context_ownership_checker_v2::{ORIGINAL_SHA256, RELATIVE_PATH, contract};
use orchestrix_core::policy::{
    AuthorizedContextFile, ContextBudget, ContextInput, ContextSourceIdentity, compile_context,
};
use serde_json::Value;

#[test]
fn canonical_five_criteria_and_only_authorized_baseline_survive_core_compilation() {
    let c = contract();
    let input = ContextInput {
        schema_version: 1,
        id: "offline-canonical-context-v2".into(),
        version: 1,
        objective: c["task"]["objective"].as_str().unwrap().into(),
        acceptance_criteria: c["task"]["acceptanceCriteria"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        constraints: vec![],
        sources: vec![],
        files: vec![AuthorizedContextFile {
            path: RELATIVE_PATH.into(),
            source: ContextSourceIdentity {
                reference: "baseline-document-v2".into(),
                revision: ORIGINAL_SHA256.into(),
            },
            authorization_ref: Some("explicit-offline-context-consent-v2".into()),
            content: include_str!("../test-data/original.md").into(),
            required: true,
            priority: 1,
        }],
    };
    let pack = compile_context(&input, ContextBudget::default()).unwrap();
    let body: Value = serde_json::from_str(pack.rendered()).unwrap();
    assert_eq!(body["acceptanceCriteria"], c["task"]["acceptanceCriteria"]);
    assert_eq!(body["objective"], c["task"]["objective"]);
    assert_eq!(body["files"].as_array().unwrap().len(), 1);
    assert_eq!(body["files"][0]["path"], RELATIVE_PATH);
    assert_eq!(body["files"][0]["contentHash"], ORIGINAL_SHA256);
    assert!(body["omissions"].as_array().unwrap().is_empty());
    assert!(pack.rendered().len() <= 65_536);
    // This offline compilation grants no runtime metadata access or execution.
}
