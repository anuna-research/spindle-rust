# Spindle Contract Spec (CLI-First, Stateless, v1)

## 1. Purpose and Audience

This is the authoritative contract for data exchanged between gleg and spindle.

Audience:

1. Spindle maintainers implementing CLI/schema behavior.
2. gleg maintainers consuming spindle CLI outputs.

This document is normative unless marked otherwise.

## 2. Scope

In scope:

1. CLI options and request input model.
2. JSON output schemas and semantics.
3. Error/exit behavior.
4. Determinism requirements.
5. Capabilities/version negotiation.

Out of scope:

1. Spindle internal implementation design details.
2. gleg workflow policy (including verification gating).

## 3. Core Semantics (Normative)

1. Reasoning status vocabulary is fixed:
   - `provable`
   - `refuted`
   - `unknown`
2. Trust is overlay-only in v1:
   - Trust affects ranking/explainability only.
   - Trust must not change logical status or conclusion type.

## 4. CLI Surface (Normative)

Supported commands:

1. `reason`
2. `query`
3. `requires`
4. `explain`
5. `why-not`
6. `capabilities`

Global/command flags required for integration:

1. `--json` (structured output)
2. `--stdin` (theory input via stdin)
3. `--at <rfc3339>` (as-of evaluation time, when supported)
4. `--given "<spl-literal>"` (repeatable)
5. `--givens-file <path>` (repeatable)
6. `--source-weights-file <path>` (repeatable)
7. `--trust-policy-file <path>` (optional)
8. `--trust-mode overlay|off` (default: `overlay`)
9. `--timeout-ms <n>`
10. `--max-solutions <n>`
11. `--max-ground-instances <n>`
12. `--max-input-bytes <n>`
13. `--max-trust-contributors <n>`

## 5. Input Model and Precedence (Normative)

### 5.1 Theory Source

Exactly one theory source per invocation:

1. Positional file.
2. `--stdin`.

If both are provided, return a validation error.

### 5.2 Givens Merge Semantics

1. All givens are merged with set semantics using canonical literal identity.
2. Duplicate equivalent givens do not change results.
3. Input order and file order do not change reasoning outcomes.

### 5.3 Trust Merge Semantics

1. Source weights merge by `source_id`.
2. Trust policy entries merge by stable policy key (`policy_id` or equivalent).
3. Equal duplicate key/value entries dedupe.
4. Conflicting duplicate key/value entries in one request produce a validation error.

### 5.4 Append-Only Log Boundary

For event-sourced systems:

1. Event replay/projection conflict resolution is done before invoking spindle.
2. Spindle receives a projected snapshot request, not a raw event log.

## 6. Output Contract (Normative)

### 6.1 Common JSON Envelope

With `--json`, all command responses include:

1. `schema_version` (required).
2. `diagnostics` (required array; empty if none).

Diagnostic shape:

```json
{
  "severity": "error | warning | info",
  "code": "STRING_CODE",
  "message": "Human-readable message",
  "details": {}
}
```

`details` is optional; when present it is an object.

### 6.2 Canonical Literal Fields

Wherever literals are emitted, both are required:

1. `literal_spl`
2. `literal_struct`

Canonical `literal_struct` shape:

```json
{
  "functor": "string",
  "args": ["string"],
  "negated": false,
  "mode": { "name": "string|null", "negation": false },
  "temporal": { "start": null, "end": null }
}
```

Notes:

1. `mode.name` may be `null`.
2. `temporal.start` and `temporal.end` may be `null`.
3. The JSON schema files in `contracts/spindle/v1/schemas` are the normative machine-readable definitions.

### 6.3 Command Schemas

Schema IDs:

1. `spindle.reason.v1`
2. `spindle.query.v1`
3. `spindle.requires.v1`
4. `spindle.explain.v1`
5. `spindle.why_not.v1`
6. `spindle.capabilities.v1`

Draft schema artifacts in this repo:

1. `contracts/spindle/v1/schemas/spindle.common.v1.schema.json`
2. `contracts/spindle/v1/schemas/spindle.reason.v1.schema.json`
3. `contracts/spindle/v1/schemas/spindle.query.v1.schema.json`
4. `contracts/spindle/v1/schemas/spindle.requires.v1.schema.json`
5. `contracts/spindle/v1/schemas/spindle.explain.v1.schema.json`
6. `contracts/spindle/v1/schemas/spindle.why_not.v1.schema.json`
7. `contracts/spindle/v1/schemas/spindle.capabilities.v1.schema.json`

Precedence for schema-bearing commands:

1. For all schema-bearing commands, JSON schema files are authoritative for field-level shape/typing.
2. If prose and schema conflict, schema wins.

Required command-level semantics:

1. `reason` required fields:
   - `schema_version`, `evaluated_at` (nullable), `grounding`, `conclusions`, `diagnostics`
   - `stats` is optional informational metadata at top-level (same envelope level as `diagnostics`)
   - `grounding` shape:
     - `performed` (boolean)
     - `had_variables` (boolean)
     - `instances` (number)
     - `limit_hit` (boolean)
2. `query` required fields:
   - `schema_version`, `literal_spl`, `literal_struct`, `status`, `conclusion_type` (nullable), `evaluated_at` (nullable), `trust` (nullable), `diagnostics`
3. `requires` required fields:
   - `schema_version`, `goal_spl`, `goal_struct`, `satisfied`, `solutions`, `evaluated_at` (nullable), `trust` (nullable), `diagnostics`
4. `explain` required fields:
   - `schema_version`, `literal_spl`, `literal_struct`, `status`, `proof_tree` (nullable), `evaluated_at` (nullable), `trust` (nullable), `diagnostics`
5. `why-not` required fields:
   - `schema_version`, `literal_spl`, `literal_struct`, `status`, `blocked_by`, `evaluated_at` (nullable), `trust` (nullable), `diagnostics`

`requires` rules:

1. If `satisfied=true`, then `solutions=[]`.
2. If `satisfied=false`, `solutions` must be non-empty.
3. `solutions[*].facts` uses canonical literal values (`literal_struct` preferred, SPL string accepted where schema allows), and each solution must use one representation style (no mixed string/struct arrays).
4. `solutions[*].score` is a ranking/explainability signal only, not logical certainty.
5. `truncated` is optional and appears only when truncation occurs.

### 6.4 Trust Payload (v1)

`trust` is required and nullable in `query`, `requires`, `explain`, and `why-not`.

When present, recommended shape:

```json
{
  "score": 0.84,
  "contributors": [
    { "source_id": "doc:deed-1", "weight": 0.9, "impact": 0.6 }
  ],
  "explain": "Primary source evidence dominates score."
}
```

When no trust input is provided:

1. `trust` is `null`.
2. `contributors[*].impact` is normalized to `[-1, 1]`, where negative values indicate downward contribution to aggregate trust.
3. `trust.explain` is required and non-empty whenever `trust` is non-null.

## 7. Determinism Requirements (Normative)

1. `conclusions` sorted by `literal_spl`, then fixed `conclusion_type` order.
2. `solutions` sorted by set size then lexical order.
3. `facts` inside each solution sorted lexically.
4. Trust contributor ordering is stable and deterministic.
5. Equal-score ties are deterministic (`literal_spl`, then stable internal key).

## 8. Error and Exit Behavior (Normative)

### 8.1 Exit Codes

1. `0`: command executed successfully (including logical outcomes like `unknown`).
2. `2`: user input / parse / validation error.
3. `3`: execution/internal reasoning error.
4. `4`: resource/limit/timeout hit.

### 8.2 Domain Outcome Rule

Logical outcomes are not process failures:

1. `query` returning `unknown` is exit code `0`.
2. `requires` returning unsatisfied solutions is exit code `0`.
3. `explain` with no proof tree for unprovable literal is exit code `0` plus diagnostics.

### 8.3 Error JSON Shape

On failures with `--json`:

1. `diagnostics` is always present.
2. Top-level `error` is present only when exit code is non-zero.
3. When present, `error` has shape:

```json
{
  "code": "STRING_CODE",
  "message": "Human-readable summary",
  "details": {}
}
```

### 8.4 Limit/Truncation Semantics

If `--max-solutions` truncates output:

1. Response remains exit code `0`.
2. Include a warning diagnostic with code `SOLUTIONS_LIMIT_HIT`.
3. Include explicit truncation metadata:

```json
{
  "truncated": {
    "solutions": true
  }
}
```

When no truncation occurs, `truncated` may be omitted.

Related stable diagnostic codes:

1. `TIMEOUT`
2. `GROUNDING_LIMIT_HIT`
3. `INPUT_TOO_LARGE`
4. `SOLUTIONS_LIMIT_HIT`
5. `TRUST_CONFLICTING_SOURCE_WEIGHT`
6. `TRUST_INVALID_RANGE`
7. `TRUST_UNKNOWN_SOURCE`
8. `TRUST_POLICY_INVALID`

## 9. Capabilities and Version Negotiation (Normative)

`spindle capabilities --json` returns `spindle.capabilities.v1`.

Required fields:

1. `commands`
2. `features`
3. `schemas`

Example:

```json
{
  "schema_version": "spindle.capabilities.v1",
  "commands": ["reason", "query", "requires", "explain", "why-not"],
  "features": {
    "stdin": true,
    "given_flags": true,
    "trust_overlay_v1": true,
    "trust_explain_v1": true,
    "at": true,
    "reason_json": true
  },
  "schemas": {
    "reason": "spindle.reason.v1",
    "query": "spindle.query.v1",
    "requires": "spindle.requires.v1",
    "explain": "spindle.explain.v1",
    "why_not": "spindle.why_not.v1"
  }
}
```

Consumers should use `capabilities` for feature detection, not binary version strings.

## 10. Compatibility Policy

1. Schema version is the wire compatibility contract.
2. v1 schemas are closed-world (`additionalProperties: false` at top level), so adding fields is a schema change.
3. Additive/shape changes that introduce new fields require schema version bump.
4. Deprecated fields should remain for at least one minor release cycle.

## 11. End-to-End Example (Informative)

Query example input:

Theory passed via stdin:

```spl
#lang spindle
(rule r1 (needs_classify ?doc ?hash) (doc_uploaded ?doc ?hash))
```

Invocation:

```bash
cat <<'SPL' | spindle query --stdin "(needs_classify doc_1 h1)" \
  --json \
  --given "(doc_uploaded doc_1 h1)" \
  --given "(from_source doc_1 primary_registry)" \
  --source-weights-file ./weights.json
#lang spindle
(rule r1 (needs_classify ?doc ?hash) (doc_uploaded ?doc ?hash))
SPL
```

Response (abridged):

```json
{
  "schema_version": "spindle.query.v1",
  "literal_spl": "(needs_classify doc_1 h1)",
  "literal_struct": {
    "functor": "needs_classify",
    "args": ["doc_1", "h1"],
    "negated": false,
    "mode": { "name": null, "negation": false },
    "temporal": { "start": null, "end": null }
  },
  "status": "provable",
  "conclusion_type": "+d",
  "evaluated_at": null,
  "trust": {
    "score": 0.88,
    "contributors": [
      { "source_id": "primary_registry", "weight": 0.95, "impact": 0.88 }
    ],
    "explain": "Primary source weighted evidence supports this conclusion."
  },
  "diagnostics": []
}
```

`requires` example (unsatisfied):

```bash
cat <<'SPL' | spindle requires --stdin "(ready_for_review doc_1)" \
  --json \
  --given "(doc_uploaded doc_1 h1)"
#lang spindle
(rule r2 (ready_for_review ?doc) (human_verified ?doc))
SPL
```

Response (abridged):

```json
{
  "schema_version": "spindle.requires.v1",
  "goal_spl": "(ready_for_review doc_1)",
  "goal_struct": {
    "functor": "ready_for_review",
    "args": ["doc_1"],
    "negated": false,
    "mode": { "name": null, "negation": false },
    "temporal": { "start": null, "end": null }
  },
  "satisfied": false,
  "solutions": [
    {
      "facts": ["(human_verified doc_1)"],
      "score": 0.62
    }
  ],
  "evaluated_at": null,
  "trust": null,
  "diagnostics": []
}
```
