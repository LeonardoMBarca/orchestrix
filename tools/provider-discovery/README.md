# Read-only provider catalog discovery

This package retrieves provider catalogs and normalizes their observations for Orchestrix. It does not run prompts, invoke tools, create cloud agreements, authenticate users, obtain credentials, or claim model entitlement. Tests use synthetic credentials and deterministic transports; the default transport is exercised only against an in-process loopback server.

```js
import {discoverProviderCatalog, ProviderDiscoveryError} from './src/index.mjs';

const catalog = await discoverProviderCatalog({
  provider: 'anthropic',
  apiKey: transientCredential,
  connectionId: stableConnectionId
}, {signal, timeoutMs: 10000});
```

Credentials are transient arguments. No result, error, cache, log, file, environment variable or persistence mechanism contains them. The caller owns the credential lifecycle and must prevent browser storage and unrelated metadata from retaining a key. Unexpected credential echoes in normalized fields are rejected.

## Public interface

`discoverProviderCatalog(config, options)` accepts:

- `provider`: `openai`, `anthropic`, `gemini`, `azure`, `bedrock`, `compatible`.
- `apiKey`, with optional separately authorized Azure `managementToken`.
- `endpoint`, `connectionId`, `region`, `deployment`, `authMethod`, `apiVersion` as applicable.
- Azure management enumeration only when `managementToken`, `subscriptionId`, `resourceGroup` and `accountName` are supplied together.
- `includeInferenceProfiles: false` to restrict Bedrock to the foundation-model catalog.
- `allowLoopbackHTTP: true` for an explicitly configured custom loopback service; other providers require HTTPS.

Options are `signal`, injected `fetchImpl`, `timeoutMs` (default 10 seconds), `maxPages` (20), `maxModels` (2,000), `maxResponseBytes` (2 MiB/page), and `maxTotalBytes` (8 MiB). Maximum supported bounds are enforced. An injected transport is trusted test/host infrastructure; it must honor the endpoint and redirect policy and never log credentials.

The default Node transport checks resolved addresses, blocks private/link-local destinations, pins one permitted address for the connection and preserves TLS verification. Explicit loopback is restricted to canonical localhost/loopback hosts. Redirects are refused. Provider hosts/paths are fixed; custom endpoints are an explicit provider choice. Pagination stays on the exact allowed catalog path and origin. There are no paid probes or automatic retries.

`ProviderDiscoveryError` exposes only a fixed `code`, sanitized `message`, provider, HTTP status and retryability. Provider error bodies and raw transport exceptions are discarded.

## Normalized observations

Catalog results contain `schemaVersion`, sanitized `scope`, `source`, `observedAt`, `status` (`complete`, `partial`, `unavailable`), `models`, `limitations`, and request count `pages`. Authentication, entitlement, usage and credits remain `unknown` regardless of a successful catalog response.

If a later page fails because of availability, permissions, rate limiting or timeout, earlier observations are retained with `partial` status and a sanitized limitation. Cancellation, redirects, credential echoes, malformed responses and exceeded byte limits fail the operation rather than exposing an unsafe partial result. Resource limits include manually declared deployments; resources are deduplicated by kind and ID.

Each resource has an ID, display name, kind (`model`, `deployment`, `inference-profile`), resource scope, provenance and Orchestrix text compatibility. Capabilities use `supported`, `unsupported` or `unknown`, an `observed` flag and field-level evidence. Missing/null metadata remains unknown. Explicit provider booleans may establish unsupported; a failed network request cannot.

The common capabilities are input/output modalities, context token limits, reasoning, streaming, functions, native web and host tools. Provider-specific observations remain separate. Input and output token limits are preserved independently; they are not added into an invented total window. Names, prices and model-family labels do not establish capabilities. Compatibility is based on explicit text modalities or an explicitly incompatible generation-method list; it never establishes execution access.

Documentation provenance records a primary URL, consulted date and API/runtime version when known. Catalog fields are `provider-metadata`; manually configured Azure deployments are `user-declaration`. The module never transports a metadata claim between backends.

## Provider contracts

| Provider | Catalog operation | Metadata boundary |
| --- | --- | --- |
| OpenAI | `GET /v1/models`, bearer key | Basic IDs/owner/lifecycle metadata; capabilities remain unknown. [Official reference](https://developers.openai.com/api/reference/resources/models/methods/list). |
| Anthropic | `GET /v1/models`, key/version headers; `after_id` pages | Consume explicitly returned thinking, effort, token limits and server-tool fields. Server tools do not imply function calling. [Official reference](https://platform.claude.com/docs/en/api/models/list). |
| Gemini Developer | `GET /v1beta/models`, key header; page token | Consume methods, token limits, thinking and sampling metadata. Thinking alone does not establish effort levels, web or tools. [Official reference](https://ai.google.dev/api/models). |
| Azure | Resource-key `GET /openai/v1/models`; optional separate ARM deployments | Base-model catalog is distinct from deployment enumeration. Configured deployment remains unverified until listed through its deployment surface. [API reference](https://learn.microsoft.com/en-us/rest/api/microsoft-foundry/azureopenai/models), [ARM deployment reference](https://learn.microsoft.com/en-us/rest/api/microsoftfoundry/accountmanagement/deployments/list?view=rest-microsoftfoundry-accountmanagement-2025-06-01). |
| Bedrock | Bearer key, control-plane foundation catalog and inference profiles | Region/permissions apply. Native modalities and streaming are observed; profile members do not inherit capabilities automatically. AWS credential profiles require an external signer. [API-key reference](https://docs.aws.amazon.com/bedrock/latest/userguide/api-keys-reference.html), [foundation catalog](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_ListFoundationModels.html), [profiles](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_ListInferenceProfiles.html). |
| OpenAI-compatible | `GET {configured base}/models` | Basic catalog only. Unknown custom extensions do not inherit OpenAI capabilities. An unavailable list operation remains an explicit limitation. |

Bedrock's catalog scope uses the control host, distinct from the configured execution runtime host. Azure's resource catalog and management deployment paths also carry distinct operation provenance. No output proves every catalog resource is available for inference.

## Caller-owned runtime catalog

`discoverRuntimeCatalog({request, connectionId, runtimeVersion, modelProvider}, options)` uses an already initialized caller-owned Codex request transport. It neither launches nor logs into a runtime. It reads every bounded `model/list` page and then `modelProvider/capabilities/read` once. The request function receives `(method, params, {signal})`; the caller must connect cancellation to its transport.

Hidden catalog entries are requested by default; set `includeHidden: false` to opt out. Preserved observations include input modalities, supported/default reasoning efforts, hidden/default status and optional service tiers, default service tier, model specialty and multi-agent metadata. Runtime provider web/image/namespace capability booleans stay in `providerCapabilities` at provider scope, never in individual model capabilities. Missing RPC support becomes an explicit limitation. Optional `redactValues` supplies already-known secret material for echo rejection; it is never returned. The trusted runtime transport owns frame-size limits; this helper bounds pagination, resources and the preserved metadata.

`normalizeBedrockCatalog` separately normalizes native foundation-model responses for a future caller-owned AWS signing adapter. It cannot obtain or sign with an AWS profile.

## Validation

Run `npm test` and `npm run check` in this directory. Coverage includes provider-specific wire formats, rich vs missing metadata, pagination, endpoint/redirect controls, separate Azure authorization, Bedrock scopes, bounded responses, cancellation/timeouts, credential redaction, runtime pagination and the absence of execution calls.

Real provider credentials were not available or used. Successful mock tests validate the adapters' contracts; they do not validate a live account, provider entitlement, latency or billing behavior.
