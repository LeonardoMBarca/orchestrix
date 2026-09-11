# ADR-0002 — Model and Reasoning Controls Are First-Class Policy

- **Status:** Accepted
- **Date:** 2026-09-10

## Context

Coding-agent runtimes expose different model-selection and reasoning/thinking controls. These capabilities differ by provider, model, runtime version, and authentication mode, and may evolve over time.

Orchestrix needs to let users control not only which runtime performs a task, but also the desired model strength and reasoning effort when the selected runtime supports those controls.

Hard-coding provider-specific concepts directly into the orchestration core would create brittle coupling.

## Decision

Orchestrix will treat model selection and reasoning/thinking effort as first-class orchestration policy.

The core will express provider-independent intent such as:

```text
modelPolicy: strongest-available
reasoning: maximum
```

Runtime adapters are responsible for translating that intent into the closest officially supported provider/runtime configuration.

The system will persist both requested and effective configuration.

Reasoning/model settings may be defined at multiple scopes, including user, project, Agent Profile, risk/complexity class, execution, and task.

Unsupported settings must degrade explicitly or fail according to policy; Orchestrix must never silently pretend that a requested reasoning level was honored.

## Consequences

### Positive

- Users can explicitly choose stronger or cheaper/faster inference strategies.
- Single-runtime and multi-runtime execution use the same policy abstraction.
- Routing can eventually account for reasoning capability.
- Provider-specific CLI flags remain isolated inside adapters.
- Historical metrics can compare model/reasoning configurations empirically.
- New providers can expose different thinking controls without changing the domain model.

### Negative

- Runtime adapters require capability discovery and translation logic.
- A normalized reasoning scale is necessarily approximate across providers.
- The UI must display requested versus effective configuration to avoid misleading users.
- Provider changes may require adapter updates.

## Rejected alternative

### Expose only raw provider settings

Rejected because it would make orchestration policy provider-specific and difficult to route across heterogeneous runtimes.

### Hide reasoning/model selection entirely

Rejected because inference strength is an important execution dimension for architecture, debugging, security, review, and other judgment-heavy tasks.

## Related documents

- `docs/REASONING_AND_MODEL_POLICY.md`
- `docs/ORCHESTRATION_CONTROL_CENTER.md`
- `docs/examples/orchestration-policy.example.yaml`
