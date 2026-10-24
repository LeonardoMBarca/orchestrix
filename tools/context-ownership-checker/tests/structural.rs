use orchestrix_context_ownership_checker_v2::{
    MAXIMUM_ADDITION_BYTES, MAXIMUM_FILE_BYTES, MINIMUM_ADDITION_BYTES, ORIGINAL_BYTES,
    ORIGINAL_SHA256, RELATIVE_PATH, contract, contract_hash, error, exit_code, hash, verify_bytes,
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

const ORIGINAL: &[u8] = include_bytes!("../test-data/original.md");
// Deliberately about a different subject; this is structural data, not a proposed answer.
const PROSE: &str = "A synthetic orchard register describes ripe pears and the sequence in which volunteers sort them. This passage only exercises visible prose and bounded Markdown, and makes no claim to satisfy the project's semantic criteria.\n";
fn candidate(addition: &str) -> Vec<u8> {
    [ORIGINAL, addition.as_bytes()].concat()
}
fn sample() -> Vec<u8> {
    candidate(&format!("\n## Orchard notes\n\n{PROSE}"))
}
fn reject(bytes: &[u8], code: &str) {
    let value = verify_bytes(bytes);
    assert_eq!(exit_code(&value), 1, "{value}");
    assert!(
        value["codes"].as_array().unwrap().iter().any(|v| v == code),
        "{value}"
    );
    assert_eq!(value["semanticReviewRequired"], true);
}

#[test]
fn compiled_contract_and_baseline_are_the_canonical_mechanical_specification() {
    let c = contract();
    let m = &c["mechanical"];
    assert_eq!(ORIGINAL.len(), ORIGINAL_BYTES);
    assert_eq!(hash(ORIGINAL), ORIGINAL_SHA256);
    assert_eq!(m["relativePath"], RELATIVE_PATH);
    assert_eq!(m["originalBytes"], ORIGINAL_BYTES);
    assert_eq!(m["originalSha256"], ORIGINAL_SHA256);
    assert_eq!(m["minimumAdditionBytes"], MINIMUM_ADDITION_BYTES);
    assert_eq!(m["maximumAdditionBytes"], MAXIMUM_ADDITION_BYTES);
    assert_eq!(m["maximumFileBytes"], MAXIMUM_FILE_BYTES);
    assert_eq!(m["titleKeywords"], serde_json::json!([]));
    assert_eq!(m["semanticLexicalPatterns"], serde_json::json!([]));
    assert_eq!(
        &c["task"]["acceptanceCriteria"].as_array().unwrap()[..4],
        c["source"]["originalCriteria"].as_array().unwrap()
    );
    assert_eq!(
        c["task"]["editablePaths"],
        c["task"]["explicitContextFiles"]
    );
    assert_eq!(c["execution"]["timeoutMs"], 5000);
    assert_eq!(c["execution"]["arguments"], serde_json::json!([]));
    assert_eq!(c["reviewRubric"].as_array().unwrap().len(), 4);
    assert_eq!(verify_bytes(&sample())["contractSha256"], contract_hash());
}
#[test]
fn unchanged_original_and_empty_input_fail() {
    reject(ORIGINAL, "addition-size-limit");
    reject(b"", "original-prefix-changed");
}
#[test]
fn original_mutation_fails_even_with_valid_addition() {
    let mut b = sample();
    b[40] ^= 1;
    reject(&b, "original-prefix-changed");
}
#[test]
fn different_titles_and_prose_need_no_keywords_or_short_title_rule() {
    for title in [
        "Field notes".to_owned(),
        "A".repeat(140),
        "Observations and boundaries".into(),
        "!".into(),
    ] {
        let value = verify_bytes(&candidate(&format!("\n## {title}\n\n{PROSE}")));
        assert_eq!(value["passed"], true, "{value}");
    }
    let varied = format!(
        "\n## Another subject\nA wholly different synthetic account uses lists and subheadings.\n\n### Harvest\n\n- Record the number of crates.\n- Label the shelf before storage.\n\n{PROSE}"
    );
    assert_eq!(verify_bytes(&candidate(&varied))["passed"], true);
}
#[test]
fn valid_optional_code_comments_inline_code_and_subheadings_do_not_supply_structure() {
    let addition = format!(
        "\n<!-- not a heading: ## Hidden -->\n## Notes\n\n{PROSE}\n### Details\n```text\n## Workspace context\n<!-- this is literal code\n```\n`<!--` is a literal marker in prose.\n    ## Indented code\n"
    );
    assert_eq!(verify_bytes(&candidate(&addition))["passed"], true);
}
#[test]
fn no_heading_malformed_heading_and_multiple_sections_fail() {
    for addition in [
        format!("\n{PROSE}"),
        format!("\n##No separator\n{PROSE}"),
        format!("\n##\n{PROSE}"),
        format!("\n## One\n{PROSE}\n## Two\n{PROSE}"),
        format!("\n# Top\n## One\n{PROSE}"),
    ] {
        let v = verify_bytes(&candidate(&addition));
        assert_eq!(v["passed"], false, "{v}");
    }
    reject(
        &candidate(&format!("\n##\n{PROSE}")),
        "nonempty-h2-title-required",
    );
}
#[test]
fn headings_and_relevant_words_hidden_in_code_or_comments_do_not_pass() {
    for addition in [
        format!("\n```md\n## Context workspace\n{PROSE}```\n"),
        format!("\n<!--\n## Context workspace\n{PROSE}-->\n"),
        format!("\n    ## Context workspace\n    {PROSE}"),
    ] {
        reject(&candidate(&addition), "one-visible-h2-required");
    }
    for addition in [
        format!("\n## Notes\n<!-- {PROSE} -->\n"),
        format!("\n## Notes\n```text\n{PROSE}```\n"),
        format!("\n## Notes\n`{}`\n", PROSE.trim()),
    ] {
        reject(&candidate(&addition), "visible-prose-required");
    }
}
#[test]
fn unsupported_or_unclosed_markdown_is_explicitly_rejected() {
    for tail in ["<!-- never closed", "```text\nnot closed"] {
        reject(
            &candidate(&format!("\n## Notes\n{PROSE}{tail}\n")),
            "unclosed-markdown-block",
        );
    }
    reject(
        &candidate(&format!("\n<h2>Hidden HTML heading</h2>\n{PROSE}")),
        "raw-html-outside-profile",
    );
    reject(
        &candidate(&format!("\n## Notes\n{PROSE}Another heading\n---\n")),
        "setext-heading-outside-profile",
    );
    reject(
        &candidate(&format!("\n{PROSE}\n## Notes\n{PROSE}")),
        "section-must-start-with-h2",
    );
}
#[test]
fn removing_comments_cannot_manufacture_atx_markers_or_visible_headings() {
    for marker in [
        "<!-- hidden -->## Notes",
        "<!--\nhidden\n-->## Notes",
        "##<!-- hidden --> Notes",
        "#<!-- hidden --># Notes",
    ] {
        reject(
            &candidate(&format!("\n{marker}\n\n{PROSE}")),
            "one-visible-h2-required",
        );
    }
}
#[test]
fn invisible_reference_definitions_and_empty_closing_hash_titles_do_not_pass() {
    reject(
        &candidate(&format!(
            "\n## Notes\n\n[orchard]: https://example.invalid/{}\n",
            "x".repeat(180)
        )),
        "reference-definition-outside-profile",
    );
    reject(
        &candidate(&format!("\n## ###\n\n{PROSE}")),
        "nonempty-h2-title-required",
    );
    reject(
        &candidate(&format!(
            "\n## Notes\n\n[orchard\nlabel]: https://example.invalid/{}\n",
            "x".repeat(180)
        )),
        "reference-definition-outside-profile",
    );
    assert_eq!(
        verify_bytes(&candidate(&format!(
            "\n## Notes\n\n[A label](local.md) discusses the token ]: in this example.\n{PROSE}"
        )))["passed"],
        true
    );
}
#[test]
fn invalid_encoding_line_endings_and_nul_have_specific_codes() {
    let mut b = sample();
    b.push(255);
    b.push(b'\n');
    reject(&b, "invalid-utf8");
    reject(
        &candidate(&format!("\r\n## Notes\r\n{PROSE}")),
        "lf-required",
    );
    reject(&candidate(&format!("\n## Notes\n{PROSE}\0\n")), "nul-byte");
    reject(
        &candidate(&format!("\n## Notes\n{}", PROSE.trim_end())),
        "final-lf-required",
    );
    let mut split = vec![b' '; ORIGINAL_BYTES - 1];
    split.extend_from_slice("é".as_bytes());
    split.extend_from_slice(PROSE.as_bytes());
    reject(&split, "invalid-utf8");
}
#[test]
fn exact_byte_bounds_are_inclusive() {
    for n in [MINIMUM_ADDITION_BYTES, MAXIMUM_ADDITION_BYTES] {
        let mut a = "\n## N\n\n".to_owned();
        a.push_str(&"x".repeat(n - a.len() - 1));
        a.push('\n');
        assert_eq!(verify_bytes(&candidate(&a))["passed"], true);
    }
    reject(&candidate("\n## N\nShort.\n"), "addition-size-limit");
    reject(
        &candidate(&format!("\n## N\n{}\n", "x".repeat(MAXIMUM_ADDITION_BYTES))),
        "file-size-limit",
    );
}
#[test]
fn semantic_contradiction_passes_structure_but_requires_rejection_under_s1_s2() {
    let contradiction = "\n## A structural example\n\nKeeping a file out of the Context Pack guarantees that runtime tools cannot read it. Preserving a byte sequence certifies its content as safe and supplies permission to disclose and edit it.\n";
    let value = verify_bytes(&candidate(contradiction));
    assert_eq!(value["passed"], true);
    assert_eq!(value["semanticReviewRequired"], true);
    // Local, explicit semantic test annotation, not a product review or fake Accepted/Rejected record.
    // S1 rejects the alleged read sandbox; S2 rejects safety/consent from preservation.
    assert_eq!(contract()["reviewRubric"][0]["id"], "S1");
    assert_eq!(contract()["reviewRubric"][1]["id"], "S2");
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "orchestrix-checker-v2-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        fs::create_dir(path.join("docs")).unwrap();
        Self(path)
    }
}
// Keep owned test evidence in Temp; no broad cleanup or mutation of old proofs.
fn inventory(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, u64, std::time::SystemTime)> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        let p = entry.path();
        if p.is_dir() {
            result.extend(inventory(&p));
        } else {
            let m = fs::metadata(&p).unwrap();
            result.insert(
                p,
                (
                    fs::read(entry.path()).unwrap(),
                    m.len(),
                    m.modified().unwrap(),
                ),
            );
        }
    }
    result
}
fn binary() -> PathBuf {
    std::env::var_os("ORCHESTRIX_CHECKER_EXE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_context-ownership-checker-v2")))
}
fn cli(directory: &Directory, args: &[&str]) -> (i32, Value) {
    let mut cmd = Command::new(binary());
    cmd.current_dir(&directory.0).args(args).env_clear();
    if let Some(root) = std::env::var_os("SystemRoot") {
        cmd.env("SystemRoot", root);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    let out = cmd.output().unwrap();
    assert!(out.stderr.is_empty());
    assert!(out.stdout.len() <= 2048);
    (
        out.status.code().unwrap(),
        serde_json::from_slice(&out.stdout).unwrap(),
    )
}
#[test]
fn cli_reads_one_fixed_file_and_does_not_mutate_tree() {
    let d = Directory::new();
    fs::write(d.0.join(RELATIVE_PATH), sample()).unwrap();
    fs::write(d.0.join("other.txt"), "untouched\n").unwrap();
    let before = inventory(&d.0);
    let (code, v) = cli(&d, &[]);
    assert_eq!(code, 0);
    assert_eq!(v["passed"], true);
    assert_eq!(before, inventory(&d.0));
}
#[test]
fn cli_read_error_and_structural_rejection_are_distinct_and_sanitized() {
    let d = Directory::new();
    let (code, v) = cli(&d, &[]);
    assert_eq!(code, 2);
    assert_eq!(v, error("file-not-found"));
    assert!(!v.to_string().contains(d.0.to_str().unwrap()));
    fs::create_dir(d.0.join(RELATIVE_PATH)).unwrap();
    let (code, v) = cli(&d, &[]);
    assert_eq!(code, 2);
    assert_eq!(v, error("path-not-regular"));
    let d = Directory::new();
    fs::write(d.0.join(RELATIVE_PATH), vec![b'x'; MAXIMUM_FILE_BYTES + 1]).unwrap();
    let (code, v) = cli(&d, &[]);
    assert_eq!(code, 1);
    assert_eq!(v["originalPrefixMatches"], Value::Null);
    assert_eq!(v["codes"], serde_json::json!(["file-size-limit"]));
}
#[test]
fn arbitrary_paths_and_output_arguments_are_not_exposed() {
    let d = Directory::new();
    let (code, v) = cli(&d, &["--file", "other.txt"]);
    assert_eq!(code, 2);
    assert_eq!(v, error("arguments-not-supported"));
}

#[test]
#[cfg(windows)]
fn real_core_evaluator_uses_exact_executable_pin_arguments_cwd_and_authorization() {
    use orchestrix_core::{
        evaluator::{CheckSpec, EvaluationResult, evaluate},
        runtime::CancellationToken,
    };
    let d = Directory::new();
    fs::write(d.0.join("other.txt"), "untouched\n").unwrap();
    let mut spec = CheckSpec {
        id: "context-ownership-structural-v2".into(),
        executable: binary(),
        arguments: vec![],
        authorization_ref: "explicit-context-ownership-structural-v2".into(),
        authorized: true,
        timeout_ms: 5000,
        executable_sha256: None,
    };
    let pin = spec.preflight_executable().unwrap();
    assert_eq!(pin, hash(&fs::read(&spec.executable).unwrap()));
    if let Ok(expected) = std::env::var("ORCHESTRIX_CHECKER_PIN") {
        assert_eq!(pin, expected);
    }
    spec.executable_sha256 = Some(pin.clone());
    let mut reports = Vec::new();
    for bytes in [Some(sample()), Some(ORIGINAL.to_vec()), None] {
        if let Some(b) = &bytes {
            fs::write(d.0.join(RELATIVE_PATH), b).unwrap();
        } else {
            fs::remove_file(d.0.join(RELATIVE_PATH)).unwrap();
        }
        let before = inventory(&d.0);
        let revision = hash(bytes.as_deref().unwrap_or(b"missing-file-fixture"));
        let expected = bytes
            .as_deref()
            .map(verify_bytes)
            .unwrap_or_else(|| error("file-not-found"));
        let stdout = format!("{expected}\n");
        let r = evaluate(&spec, &d.0, &revision, &CancellationToken::new()).unwrap();
        assert_eq!(
            r.result,
            if exit_code(&expected) == 0 {
                EvaluationResult::Passed
            } else {
                EvaluationResult::Failed
            }
        );
        assert_eq!(r.exit_code, Some(exit_code(&expected) as u32));
        assert_eq!(r.artifact_revision, revision);
        assert_eq!(r.executable_hash, pin);
        assert_eq!(
            r.command_hash,
            hash(serde_json::to_value(&spec).unwrap().to_string().as_bytes())
        );
        assert_eq!(r.output_sha256, hash(stdout.as_bytes()));
        assert_eq!(r.stdout, stdout.chars().take(512).collect::<String>());
        assert_eq!(r.output_truncated, stdout.len() > 512);
        assert!(r.stderr.is_empty());
        assert!(r.tree_confirmed && r.streams_closed);
        assert_eq!(r.timeout_ms, 5000);
        assert_eq!(r.authorization_ref, spec.authorization_ref);
        assert_eq!(r.cwd, d.0.to_str().unwrap());
        assert_eq!(before, inventory(&d.0));
        reports.push(serde_json::to_value(r).unwrap());
    }
    // Optional pristine future target: same frozen executable/argv/pin/5s, actual cwd.
    // Never write there or treat this synthetic evaluator receipt as a worker artifact.
    if let Some(cwd) = std::env::var_os("ORCHESTRIX_CHECKER_BASELINE_CWD") {
        let cwd = PathBuf::from(cwd);
        assert_eq!(fs::read(cwd.join(RELATIVE_PATH)).unwrap(), ORIGINAL);
        let r = evaluate(&spec, &cwd, &hash(ORIGINAL), &CancellationToken::new()).unwrap();
        assert_eq!(r.result, EvaluationResult::Failed);
        assert_eq!(r.exit_code, Some(1));
        assert_eq!(r.executable_hash, pin);
        assert_eq!(r.cwd, cwd.to_str().unwrap());
        assert!(r.tree_confirmed && r.streams_closed);
        assert_eq!(fs::read(cwd.join(RELATIVE_PATH)).unwrap(), ORIGINAL);
        reports.push(serde_json::to_value(r).unwrap());
    }
    // Pin is checked again immediately before process launch, not just at preparation.
    let altered = d.0.join("changed-checker.exe");
    fs::copy(&spec.executable, &altered).unwrap();
    let mut b = fs::read(&altered).unwrap();
    let end = b.len() - 1;
    b[end] ^= 1;
    fs::write(&altered, b).unwrap();
    let mut changed = spec.clone();
    changed.executable = altered;
    let failure = evaluate(&changed, &d.0, "revision", &CancellationToken::new()).unwrap_err();
    assert_eq!(failure, "check-executable-changed");
    if let Some(p) = std::env::var_os("ORCHESTRIX_CHECKER_REPORT") {
        fs::OpenOptions::new().write(true).create_new(true).open(p).map(|mut f|{use std::io::Write;f.write_all(serde_json::to_string_pretty(&serde_json::json!({"spec":spec,"reports":reports,"latePinChangeRejected":true,"treeUnchanged":true})).unwrap().as_bytes()).unwrap();}).unwrap();
    }
}
