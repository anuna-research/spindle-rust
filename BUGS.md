# Bug Hunt Report

> **Resolution status (2026-07-04):** Both findings below are fixed with
> regression coverage. The file paths in the reports are stale (the code has
> since been restructured into `pipeline/` and `reason/` module directories).
>
> 1. **Fixed** — `filter_temporal` now consults rule-level bounds:
>    `crates/spindle-core/src/pipeline/temporal.rs:64` checks
>    `rule.temporal.is_empty() || rule.temporal.active_at(t)`. Regression tests:
>    `test_rule_level_temporal_filtered_when_inactive` /
>    `test_rule_level_temporal_kept_when_active` in
>    `crates/spindle-core/tests/temporal_asof_tests.rs`.
> 2. **Fixed** — `match_literal` now rejects mode mismatches
>    (`crates/spindle-core/src/grounding.rs:116`) and binds temporal
>    variables/intervals against concrete ground endpoints. Regression tests:
>    `test_match_literal_mode_mismatch`, `test_ground_theory_mode_discrimination`,
>    `test_ground_theory_same_mode_matches` in `grounding.rs`.

## 1) Rule-level temporal bounds are ignored during temporal filtering

**Location:** `crates/spindle-core/src/pipeline.rs:166-196`

**What happens:**
`filter_temporal` only checks `rule.head[*].temporal` and `rule.body[*].temporal`. The rule’s own temporal bounds (`rule.temporal`) are never consulted, so a rule with an inactive temporal window still fires if its literals are active. This makes `PrepareOptions.reference_time` incomplete for rules that set temporal bounds at the rule level.

**Why it’s a bug:**
Rules can carry temporal bounds independently of their literals, but those bounds are currently ignored by the filtering pipeline. This violates the expected “as-of” semantics when the rule itself is time-scoped.

**Minimal repro (conceptual):**
- Create a rule with `rule.temporal = [1000, 2000]` and a body/head with empty temporals.
- Call `reason_with_options(... reference_time = 3000 ...)`.
- The rule still survives filtering and can fire, even though the rule-level temporal window excludes 3000.

**Expected behavior:**
Rules should be filtered out when their own temporal bounds do not include the reference time.

---

## 2) Grounding matches facts across incompatible modes/temporals

**Location:** `crates/spindle-core/src/grounding.rs:56-100` (and associated indexing)

**What happens:**
`match_literal` checks only name, negation, and predicate arity/arguments. It ignores `mode` and `temporal` fields. Because `fact_index_key` also ignores mode/temporal, grounding can match a rule body literal against a fact with a different modal operator or temporal window.

**Why it’s a bug:**
Modes (e.g., obligation vs permission) and temporal bounds are part of literal semantics. Matching a rule body literal against a fact with a different mode/temporal produces incorrect substitutions and grounded rules that should not exist.

**Minimal repro (conceptual):**
- Fact: `[O]p(a)` (mode = obligation)
- Rule body: `[P]p(?x)` (mode = permission)
- Grounding will match and substitute `?x = a`, producing a rule instance that conflates different modalities.

**Expected behavior:**
`match_literal` (and the indexing key) should include mode and temporal compatibility in matching so that only semantically compatible facts bind variables.
