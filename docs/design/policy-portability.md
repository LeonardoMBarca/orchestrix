# Portable policies — version 1

Implemented by `policy_portability.rs`, using the existing Desktop Settings and Store. This is a configuration document, not an execution manifest or database export.

## Format and limits

The UTF-8 JSON envelope has `schemaVersion: 1`, `format: "orchestrix-policy"`, `appearance`, `policy`, `connectionReferences` and `modelReferences`. Documents are limited to 262,144 bytes. Parsing rejects future versions, unknown or duplicate fields, invalid identifiers/references, excessive collections and out-of-range policy values. A rejected document does not produce a partial import.

The allowlist carries language, theme, density, text scale, close preference, the four resource strategies, supported presets, policy layers/overrides/requirements, model preference/rank rules, observation TTLs and mode/profile definition references. `sourceModelId` is an inspectable hint, never an automatic binding. Custom definitions require explicit remapping to an existing destination definition; importing a reference does not install an execution mode.

Exports omit credentials, authentication stores, runtime/home/directory bindings, connection source IDs, transcripts, personal system instructions, context/file contents, artifacts, diagnostics, session identities, approvals, receipts, execution authorization and recurring-read consent. Logical connection/model aliases replace source bindings. No unsupported mode engine or provider is added to make a document importable.

## Preview and explicit remapping

The host reads only a bounded file selected in the native chooser and retains an immutable document under an opaque reference. A renderer path does not grant access. Export uses a native destination chooser and a create-new publication: an existing file is preserved, and an unavailable filesystem publication fails explicitly.

Preview identifies changes, definition/connection/model references and incompatibilities. Each connection alias must be mapped explicitly to an eligible local connection and its binding revision. Each model reference must be selected explicitly from that connection's current observed catalog. Matching labels, emails, local IDs or model spellings do not prove identity or authorize remapping. Missing, stale, partial-negative or incompatible evidence blocks the affected reference without an authenticated query as a side effect. The user may explicitly Recheck in Connections and refresh the preview.

The preview digest binds the document, mappings, expected global Store revision and destination Settings resolution. Apply requires confirmation and that exact digest/revision; even an unrelated Store mutation may require a fresh preview. The Store repeats local mapping checks in the same short transaction as the Settings write. Unlink, identity change, expiry, generation change or another Settings edit can therefore invalidate a preview. Cancellation or validation failure leaves Settings unchanged. The native host journals the original apply command so a lost reply can be recovered using its identity without another application or a new execution.

## Local authority and future decisions

Local mandatory restrictions from every scope survive import. Subscription Only remains mandatory; imported preferences cannot authorize paid API fallback. Effective observation TTLs cannot exceed local policy limits. Model selection/requirements remain tied to the explicitly remapped connection. Where model ranking is a soft preference, its mapped owners become ordered connection preferences when the source supplies no explicit owner ordering; compatible fallback remains a preference rather than a fabricated hard requirement.

Import does not initiate metadata, login, inference, checks or worker submission, and never imports recurring consent. Personal instructions and work history remain local. Policy revision advances for future decisions; snapshots of previous attempts remain unchanged. The host checks the preparation's policy/instructions fingerprint again before dispatch, including edits made in another window. A stale preparation requires explicit preparation again.

See [diagnostic authority](connection-diagnostics.md#implemented-cadence-and-validity--ox-016-19-october-2026), [daemon contract](../contracts/local-daemon.md#diagnostic-cadence-and-portable-policy-extension) and [persistence](../contracts/persistence.md#diagnostic-cadence--policy-transport--schema-9). Full editable modes, telemetry, independent accounts and other OX-016 criteria remain separate.
