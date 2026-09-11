# Orchestrix — Model and Reasoning Policy

## Purpose

Orchestrix must treat **model selection** and **reasoning/thinking effort** as first-class orchestration controls.

The system should not merely choose a runtime such as Codex, Claude Code, or Antigravity. It should also be able to choose, when the runtime exposes the capability:

- model;
- reasoning/thinking mode;
- reasoning effort/intensity;
- task-specific inference configuration;
- role-specific defaults;
- risk-specific defaults;
- per-run and per-task overrides.

The exact controls differ by provider and may change over time, so Orchestrix must avoid hard-coding one provider's vocabulary into the core domain model.

---

## Architectural principle

Orchestrix should separate **normalized orchestration intent** from **provider-specific execution settings**.

Example normalized intent:

```text
reasoning profile: maximum
model policy: strongest-available
```

A Runtime Adapter then translates that intent into the closest supported configuration for that runtime/model.

The adapter must never invent support for a setting that the runtime does not expose.

---

## Capability discovery

Each runtime adapter should expose a capability description similar to:

```json
{
  "runtime": "codex",
  "models": ["..."],
  "reasoningControl": {
    "supported": true,
    "levels": ["low", "medium", "high", "xhigh"]
  },
  "modelSelection": {
    "supported": true
  }
}
```

The exact representation is not yet a stable contract.

Capabilities should be classified as:

```text
SUPPORTED
UNSUPPORTED
UNKNOWN
RUNTIME_MANAGED
```

The UI and scheduler must distinguish these states rather than assuming every provider has equivalent controls.

---

## Normalized reasoning profiles

The Orchestrix policy layer may expose provider-independent profiles such as:

```text
auto
minimal
low
medium
high
maximum
```

These values represent intent, not guaranteed provider parameters.

For example:

```text
maximum
```

means:

> Use the strongest reasoning/thinking configuration that this selected model/runtime officially supports.

If a runtime cannot honor the requested profile, the effective behavior must be visible in execution metadata.

Example:

```text
Requested reasoning: maximum
Effective reasoning: high
Reason: selected runtime does not expose a stronger setting
```

---

## Configuration scopes

Reasoning and model policy should support the same layered configuration model as the rest of Orchestrix:

```text
System default
    ↓
User default
    ↓
Project policy
    ↓
Agent Profile
    ↓
Task category / risk policy
    ↓
Execution override
    ↓
Task override
```

The effective configuration should be resolved deterministically and persisted with the execution record.

---

## Agent Profile configuration

Agent Profiles should be able to specify defaults such as:

```yaml
profiles:
  architect:
    modelPolicy: strongest-available
    reasoning: maximum

  implementation-engineer:
    modelPolicy: strongest-available
    reasoning: high

  security-reviewer:
    modelPolicy: strongest-available
    reasoning: maximum

  documentation-agent:
    modelPolicy: default
    reasoning: medium
```

The profile expresses desired behavior. Runtime adapters determine whether and how it can be applied.

---

## Risk-aware reasoning

Orchestrix should allow reasoning depth to scale with task risk or complexity.

Example:

```yaml
reasoningByRisk:
  low: low
  medium: high
  high: maximum
  critical: maximum
```

This enables policies such as:

```text
rename file
→ deterministic tool / minimal reasoning

ordinary feature
→ high

concurrency bug
→ maximum

security-sensitive auth change
→ maximum + independent review
```

Reasoning effort is therefore a schedulable/configurable dimension rather than a global toggle.

---

## Model selection policy

Orchestrix should support strategies such as:

```text
default
strongest-available
fastest-available
explicit-model
runtime-managed
auto-by-task
```

A user may also pin a specific model where the runtime supports model selection.

Example:

```yaml
model:
  strategy: strongest-available
```

or:

```yaml
model:
  strategy: explicit
  id: provider-specific-model-id
```

Provider-specific model IDs belong at the adapter/configuration boundary and should not leak into orchestration algorithms unnecessarily.

---

## Reasoning-aware routing

The Router may eventually consider whether a runtime can satisfy the requested reasoning profile.

Example:

```text
TASK-42 requires:
  capability: architecture
  reasoning: maximum

Candidate A:
  capability match: yes
  maximum reasoning supported: yes

Candidate B:
  capability match: yes
  maximum reasoning supported: no
```

This can influence routing score or eligibility depending on policy.

A user should be able to choose whether reasoning requirements are:

```text
PREFERENCE
or
HARD REQUIREMENT
```

A single-runtime user should usually prefer graceful degradation rather than task failure.

---

## Control Center UX

The Orchestration Control Center should expose a dedicated **Models & Reasoning** section.

Example:

```text
Models & Reasoning

Global model strategy
● Strongest available
○ Runtime default
○ Fastest available
○ Custom

Default reasoning
○ Auto
○ Low
○ Medium
○ High
● Maximum

Per-role overrides
Architect              Maximum
Implementation         High
Reviewer               Maximum
Debugger               Maximum
Documentation          Medium

Per-risk overrides
Low                    Low
Medium                 High
High                   Maximum
Critical               Maximum
```

Advanced users should also be able to inspect provider-specific effective settings.

---

## Nuclear preset

Orchestrix may include a high-quality preset informally referred to as **Nuclear**.

Its intent is:

```text
Token/resource optimization: low priority
Strongest supported models: preferred
Reasoning/thinking: maximum where useful
Independent review: enabled
Cross-model review: preferred when available
Deterministic verification: required
Retry/review budget: high
```

Nuclear must still avoid wasting model calls on operations that deterministic tools can perform more accurately.

For example, it should not use a maximum-reasoning model merely to determine whether tests passed, enumerate changed files, or run a formatter.

---

## Mechanical vs cognitive work

A central optimization principle is:

```text
Mechanical fact or operation
→ deterministic software

Judgment-heavy cognitive task
→ model reasoning
```

The purpose of configurable reasoning is not to maximize thinking everywhere. It is to allocate stronger reasoning where judgment matters.

---

## Observability

Every model-backed execution should persist, when known:

```text
requested runtime
selected runtime
requested model policy
selected/effective model
requested reasoning profile
effective reasoning configuration
whether fallback occurred
why fallback occurred
```

This enables benchmarking questions such as:

- Does maximum reasoning improve first-pass success for architecture tasks?
- Does it reduce iterations in debugging?
- Is it unnecessary for routine implementation?
- Which runtime/model/effort combinations perform best for each capability?

These metrics can later feed Orchestrix's adaptive routing system.

---

## Provider evolution

Provider capabilities will change.

Therefore:

- the core must not encode a permanent list of one provider's reasoning levels;
- runtime adapters own provider-specific translation;
- capability discovery should be refreshable;
- unsupported options must degrade explicitly;
- execution records should store both requested and effective configuration;
- documentation should distinguish stable Orchestrix concepts from provider-specific facts.

This keeps model/thinking configuration extensible without coupling the orchestration engine to today's CLI flags.
