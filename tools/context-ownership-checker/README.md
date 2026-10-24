# Context ownership checker v2

Small standalone, proof-specific Rust checker for the next OX-017 proposal. It reads
only `docs/CONTEXT_OWNERSHIP.md` in its authorized cwd, using a bounded buffer and
regular-file/handle checks. It writes one bounded JSON line to stdout, does not
write the workspace, and has no subprocess, network or model calls. Only Windows
is validated by the evaluator test. It is neither a general Markdown parser nor
a semantic reviewer or filesystem sandbox.

[`proof-contract.json`](proof-contract.json) is the canonical criteria source.
The original four criteria are retained exactly from the previous READY; the
fifth publishes the small structural profile before authorization. Tests bind
compiled limits/baseline to that source. `--describe` exposes the embedded contract
hash and limits; ordinary execution takes **no arguments**. Arbitrary input/output
paths are not supported. Source, tests and executable must be frozen together in
the new READY. This does not replace or rerun the old checker or old worker.

| Criterion | Mechanical evidence | Mandatory semantic evidence |
| --- | --- | --- |
| C1 | Exact original 7,170-byte SHA-256 prefix; 160–2,048 appended bytes; UTF-8, no NUL/CR, final LF; one visible H2 section | Original, clear, relevant English prose (S3) |
| C2 | No lexical assertion | Four distinct controls and absence of a read sandbox (S1) |
| C3 | No lexical assertion | Preservation versus content safety/consent; enforcement or blocking (S2) |
| C4 | Core opaque-base-v2 fixes the allowlist and checks all other paths/index before/after checks; this executable reads **one file** | Exact artifact, diff, preservation and check evidence; no unsupported conclusions (S4) |
| C5 | Published `visible-atx-section-v2` profile below | A structural pass never substitutes for S1–S4 |

The profile allows blank lines/comments before the single H2, a nonempty title
with free wording/length within the byte budget, prose, H3–H6, closed code fences
and comments. Headings in fenced/indented code or comments do not count. Inline
code alone does not supply prose. Lines starting with `<` after up to three
spaces (except comments), Setext headings and link-reference definitions are
rejected. Comments never manufacture block markers by joining separated text;
the closing line of a comment block does not supply a heading. Prose before the
H2, another H1/H2, missing visible prose and unclosed blocks are rejected. This
limited ATX profile is deliberate and visible in C5; it does not claim complete
CommonMark support. No title keywords or semantic lexical patterns exist.

Exit **0** means only mechanical pass; **1** is structural rejection; **2** is
read/execution/argument failure. Output contains static codes and counts, never
the document or private paths. A size rejection before reading has an unknown
prefix result, not a fabricated failed hash. The evaluator retains its existing
output limits, 64 MiB executable guard, 5 s timeout and final SHA-256 pin check.

Tests use invented unrelated prose, varied titles, comments/code, inclusive byte
boundaries, mutations and read failures. A deliberately contradictory example
passes structure: S1/S2 must reject its claims. That annotation is local test
analysis, **not an independent product review**. No fixture is the old generated
answer or an expected paragraph supplied to the implementer. Full-scope writes
are rejected by Core, not claimed to be detected by this one-file checker.
An additional test compiles the five canonical criteria and the sole authorized
baseline through the existing Core Context Pack compiler, checking exact criteria,
file/hash, no omissions and the unchanged budget without metadata or execution.

```powershell
$env:CARGO_TARGET_DIR = Join-Path (Get-Location) 'target/context-ownership-checker-v2'
cargo test --manifest-path tools/context-ownership-checker/Cargo.toml --release --locked --offline -- --test-threads=1
cargo clippy --manifest-path tools/context-ownership-checker/Cargo.toml --release --all-targets --locked --offline -- -D warnings
cargo fmt --manifest-path tools/context-ownership-checker/Cargo.toml --check
```

To test the **frozen future binary** through the same evaluator, set
`ORCHESTRIX_CHECKER_EXE` and `ORCHESTRIX_CHECKER_PIN`; an optional
`ORCHESTRIX_CHECKER_REPORT` writes a new test receipt outside the cwd. The test
uses the frozen check ID, explicit local authorization, argv empty and 5 s timeout. It
verifies pass/structural rejection/read error, exit/report hashes, tree/streams,
no file changes and rejection of an altered copy before launch. Test directories
are owned, explicitly synthetic and retained in Temp; no historical proof is
opened or rewritten. Use the frozen proposal's receipt for exact execution data.
`ORCHESTRIX_CHECKER_BASELINE_CWD` additionally evaluates the pristine future target
read-only (expected rejection of the unchanged original). Positive cases remain
separate synthetic directories. The test authorization reference is explicit and
local; native preparation will issue its own fresh `check-consent-*` identity.

Core currently permits one correction only after a valid **Rejected review** of
a Verified artifact. A check failure/NeedsWork stops this proof; it is not a
supported shortcut into correction. Technical/uncertain results stop without
retry. Approval and application remain separate explicit operations on the exact
candidate and isolated destination.
