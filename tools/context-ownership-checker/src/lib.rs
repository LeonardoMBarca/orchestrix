//! Proof-specific read-only structural evaluation. Meaning remains mandatory review.
mod markdown;
mod read_only;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

pub const CONTRACT_BYTES: &[u8] = include_bytes!("../proof-contract.json");
pub const RELATIVE_PATH: &str = "docs/CONTEXT_OWNERSHIP.md";
pub const ORIGINAL_BYTES: usize = 7_170;
pub const ORIGINAL_SHA256: &str =
    "b7b0d293cea53c782d2fba2d88c4ac801ea8b8fb49c7fa93f7e638f3710bd20d";
pub const MINIMUM_ADDITION_BYTES: usize = 160;
pub const MAXIMUM_ADDITION_BYTES: usize = 2_048;
pub const MAXIMUM_FILE_BYTES: usize = ORIGINAL_BYTES + MAXIMUM_ADDITION_BYTES;

pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn contract() -> &'static Value {
    static CONTRACT: OnceLock<Value> = OnceLock::new();
    CONTRACT
        .get_or_init(|| serde_json::from_slice(CONTRACT_BYTES).expect("compiled proof contract"))
}
pub fn contract_hash() -> String {
    hash(CONTRACT_BYTES)
}
pub fn describe() -> Value {
    json!({"schemaVersion":2,"check":"context-ownership-structural-v2",
        "contractSha256":contract_hash(),"relativePath":RELATIVE_PATH,
        "originalBytes":ORIGINAL_BYTES,"originalSha256":ORIGINAL_SHA256,
        "minimumAdditionBytes":MINIMUM_ADDITION_BYTES,"maximumAdditionBytes":MAXIMUM_ADDITION_BYTES,
        "maximumFileBytes":MAXIMUM_FILE_BYTES,"semanticReviewRequired":true,
        "otherPathsEnforcedBy":"Core opaque-base-v2 capture; not this checker",
        "readProfile":"fixed regular file, read only, bounded buffer, no subprocess or network"})
}
pub fn error(code: &str) -> Value {
    json!({"schemaVersion":2,"check":"context-ownership-structural-v2",
        "contractSha256":contract_hash(),"outcome":"execution-error","passed":false,
        "codes":[code],"semanticReviewRequired":true})
}
pub(crate) fn size_rejection(observed_bytes: u64) -> Value {
    json!({"schemaVersion":2,"check":"context-ownership-structural-v2",
        "contractSha256":contract_hash(),"outcome":"structural-rejection","passed":false,
        "codes":["file-size-limit"],"fileBytesObserved":observed_bytes,
        "originalPrefixMatches":null,"semanticReviewRequired":true})
}
pub fn exit_code(value: &Value) -> i32 {
    if value["outcome"] == "execution-error" {
        2
    } else if value["passed"] == true {
        0
    } else {
        1
    }
}
pub fn verify_bytes(bytes: &[u8]) -> Value {
    let prefix_matches =
        bytes.len() >= ORIGINAL_BYTES && hash(&bytes[..ORIGINAL_BYTES]) == ORIGINAL_SHA256;
    let addition = &bytes[bytes.len().min(ORIGINAL_BYTES)..];
    let mut codes = Vec::new();
    if bytes.len() > MAXIMUM_FILE_BYTES {
        codes.push("file-size-limit");
    }
    if !prefix_matches {
        codes.push("original-prefix-changed");
    }
    if !(MINIMUM_ADDITION_BYTES..=MAXIMUM_ADDITION_BYTES).contains(&addition.len()) {
        codes.push("addition-size-limit");
    }
    if bytes.contains(&0) {
        codes.push("nul-byte");
    }
    if bytes.contains(&b'\r') {
        codes.push("lf-required");
    }
    if bytes.last() != Some(&b'\n') {
        codes.push("final-lf-required");
    }
    match (std::str::from_utf8(bytes), std::str::from_utf8(addition)) {
        (Ok(_), Ok(text)) if bytes.len() <= MAXIMUM_FILE_BYTES => {
            let section = markdown::inspect(text);
            if section.h2_count != 1 || section.other_top_level {
                codes.push("one-visible-h2-required");
            }
            if section.empty_title {
                codes.push("nonempty-h2-title-required");
            }
            if section.before_heading {
                codes.push("section-must-start-with-h2");
            }
            if !section.prose {
                codes.push("visible-prose-required");
            }
            if section.unclosed_block {
                codes.push("unclosed-markdown-block");
            }
            if section.unsupported_html {
                codes.push("raw-html-outside-profile");
            }
            if section.setext {
                codes.push("setext-heading-outside-profile");
            }
            if section.reference_definition {
                codes.push("reference-definition-outside-profile");
            }
        }
        (Err(_), _) | (_, Err(_)) => codes.push("invalid-utf8"),
        _ => {}
    }
    json!({"schemaVersion":2,"check":"context-ownership-structural-v2",
        "contractSha256":contract_hash(),"outcome":if codes.is_empty(){"mechanical-pass"}else{"structural-rejection"},
        "passed":codes.is_empty(),"codes":codes,"additionBytes":addition.len(),
        "originalPrefixMatches":prefix_matches,"semanticReviewRequired":true})
}
pub fn inspect_current_directory() -> Value {
    read_only::inspect()
}
