# Runtime input profile — portable JSON revision 1

Status: implemented in both experimental JavaScript and Rust consumers, **13 October 2026**. Identifier: `{id: "orchestrix-runtime-json", revision: 1}`. This is the input codec of the [runtime contract](runtime-contract-v1.md), not a competing runtime/domain contract or a provider protocol.

## Versioning and compatibility

Semantic wire version **1.1** continues to describe fields, ownership and lifecycle. Profile revision **1** independently specifies their portable decoding. Both consumers use this profile; a lower or future semantic minor cannot select a permissive decoder. JavaScript exports `RUNTIME_INPUT_PROFILE` and rejects an explicitly unsupported constructor profile with `version`. Rust exports matching constants; its offline host-bundle example requires the profile before constructing an observer. Missing/unsupported bundle profiles fail with `version`, without dispatch or fallback. Direct Rust APIs implement only this profile.

This is an **intentional replacement of experimental preproduction ingress**, not an additive 1.2 wire change or a guarantee that every raw input accepted by the previous JavaScript decoder remains valid. That decoder, preserved in Git at `58cdf6b`, used host-dependent `Date.parse` and accepted nonportable JSON strings/numbers. There is no automatic legacy importer or downgrade. Ambiguous historical timestamps require an explicit source-backed offset before a future import can normalize them; guessing UTC or the current machine's timezone would manufacture evidence.

Normalized v1.0/v1.1 data remains compatible: the preceding **104 corpus cases, descriptors, requests and three source hashes are unchanged**. Existing sanitized reports are not rewritten. Unknown events/compatible future-minor fields, strict legacy transitions and factual 1.1 observations retain their semantics. A future change that alters the codec's accepted domain requires a new profile revision and explicit host selection/migration; semantic changes retain the wire's major/minor rules.

## Timestamp grammar

`evidence.observedAt` may be omitted/null. A present value must match exactly:

```text
YYYY-MM-DDTHH:mm:ss[.fraction](Z|+HH:mm|-HH:mm)
```

The Gregorian date must exist. Years are four ASCII digits; hours are 00–23, minutes/seconds 00–59, offsets 00–23 hours and 00–59 minutes. Fractional seconds contain 1–9 digits and are truncated to milliseconds. The UTC result must also remain in years 0000–9999. Output is always `YYYY-MM-DDTHH:mm:ss.sssZ`, and normalization is idempotent. There is no clock or local timezone lookup.

Reject with `protocol`: timezone-free/minute-only dates, calendar/hour rollover, leap seconds, lowercase separators/zones, whitespace, expanded years, excess fractions and UTC year overflow. The profile also excludes `-00:00`, the [RFC 3339 unknown-local-offset convention](https://www.rfc-editor.org/rfc/rfc3339#section-4.3), while accepting `Z` and `+00:00`. This is a deliberately restricted [RFC 3339 profile](https://www.rfc-editor.org/rfc/rfc3339#section-5.6), not support for every RFC option.

## Frame decoding

Frames are bounded UTF-8 JSON objects. Every string/key must contain Unicode scalar values; adjacent escaped UTF-16 high/low pairs are accepted, lone surrogates are rejected. JSON numbers must decode to finite IEEE754 binary64 values. Numbers in known counters additionally obey the existing integer/safe-range constraints. Finite duplicate keys retain last-key-wins behavior. Rust enables `serde_json/float_roundtrip` to match JavaScript rounding before these checks, including fractions next to `MAX_SAFE_INTEGER`.

An iterative lexical guard checks the **whole original frame** before normal JSON decoding, including unknown fields and overwritten duplicate occurrences. This prevents discarded nonfinite numbers or malformed Unicode from producing different results between decoders. The actual JSON parser still checks all syntax, including ignored payloads; the guard is not another event parser. Frame byte budgets count the original UTF-8 representation. No new depth limit is applied to compatible unknown data; existing 500-level regressions remain valid.

The restrictions avoid interoperability hazards identified in [RFC 8259 numeric representation](https://www.rfc-editor.org/rfc/rfc8259#section-6) and [Unicode strings](https://www.rfc-editor.org/rfc/rfc8259#section-8.2). Arbitrary JavaScript objects/invalid UTF-8 are not an interchangeable wire representation. Descriptor/request normalizers receive trusted typed host values and whitelist known fields; the offline example's outer bounded host-bundle decoder is separate from event framing and is not a production daemon endpoint.

Rejected frames preserve typed errors/quarantine, zero accepted frame bytes/events, unknown effective settings and unknown terminal. Separately trusted supervisor cleanup can still be recorded within remaining budgets; it cannot create a remote result. Provider JSON cannot negotiate a codec or grant supervisor provenance.

## Evidence

[Hand-written vectors](../../tools/runtime-harness/fixtures/portable-input-v1.json) provide explicit expected values/errors independent of corpus regeneration. [JavaScript tests](../../tools/runtime-harness/tests/runtime-input.test.mjs), [Rust tests](../../crates/orchestrix-core/tests/portable_input.rs), the shared corpus and the adapter → Rust bridge exercise the same profile. Full results and scoped OX-003 acceptance are in the [increment record](../research/portable-runtime-input-increment.md).
