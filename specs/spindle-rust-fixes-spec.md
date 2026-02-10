# RFC: Spindle‑Rust Correctness for Workflow Integration (Predicates, Grounding, Temporal Semantics, Output Contracts)

**Status:** Draft (proposed)  
**Primary drivers:** correctness, spec/doc alignment, integrator ergonomics  
**Scope:** `spindle-core`, `spindle-parser`, `spindle-cli`, `spindle-wasm`  
**Context:** Issues surfaced while planning external workflow integration + cross-checking related implementations.

---

## 0) Executive Summary

`spindle-rust` currently behaves like a *propositional* reasoner even when SPL uses predicates with arguments and first‑order variables. Temporal data is also effectively “display-only” in the reasoning hot path. This is due to:

1) **Literal identity collapsing arguments and modes** (core reasoner/index use IDs/keys derived only from name+negation).  
2) **Grounding not being part of the reasoning pipeline** (and even if wired, superiority/metadata handling across grounded instances is currently incomplete).  
3) **CLI output contract drift** (`--json` exists but does not affect `reason`; docs claim it does).  
4) **Parser/doc mismatches** around modal/temporal syntax and nested terms (docs show constructs parser can’t represent today).
5) **Temporal semantics not wired into reasoning** (no `reference_time` / “as-of” filtering; no overlap-gated conflicts; no temporal propagation).

This RFC proposes a correctness-first refactor that:
- makes predicate arguments and modal operators semantically meaningful (distinct literals),
- grounds variable theories deterministically before reasoning,
- preserves superiority and metadata semantics for grounded instances,
- provides stable JSON output for `reason`,
- defines a clear temporal semantics roadmap (timepoint “as-of” support first; interval inference as a follow-on),
- adds regression tests that would have caught the above.

---

## 0.1 Decision Summary (Proposed Defaults)

These are the recommended defaults for v1 of this refactor (chosen for workflow-engine ergonomics and implementability):

1) **Canonical literal string format:** SPL s-expressions, e.g. `(want_action classify_doc doc doc_1 h1)` (matches downstream parsing expectations and pack authoring style).
2) **Canonical machine format:** JSON always includes `literal_struct` (required) and a `schema_version` at the top-level.
3) **Grounding:** always happens inside a single `prepare()` pipeline (all public operations route through it).
4) **Wildcard `_`:** anonymous, matches anything, never binds, each occurrence independent; rejected in rule heads by validation.
5) **Temporal semantics (near-term):** implement **timepoint (“as-of”) reasoning** via `reference_time` filtering (Phase T1). Defer interval-set inference (Phase T2).
6) **SPL temporal surface syntax:** support only `(during <lit> <start> <end>)` (3-arg `during`). Treat `(during bird ?t)` (single interval variable) as a docs bug / future extension.
7) **`moment` support:** initially support `(moment "RFC3339")` only; keep multi-arity `(moment 2024 6 15 ...)` as a follow-on if needed.
8) **API breaking allowed:** prioritize correctness over backward compatibility (e.g. remove/rename misleading “name-only” identity helpers).

---

## 1) Motivation / Problem Statement (with observed evidence)

### 1.1 Predicate arguments are not respected by the reasoner
**Observed:** A rule requiring `needs_classify("doc_1","h1")` fires when only `needs_classify("doc_2","h2")` is present.  
**Root cause:** rule triggering and proven-sets are keyed by **name+negation only**, ignoring predicate args (and mode).

Contributing implementation details:
- `Literal::literal_id()` ignores predicate args + mode: `crates/spindle-core/src/literal.rs:197`
- `IndexedTheory` indexes by `literal_id()`: `crates/spindle-core/src/index.rs:39`
- Both `reason.rs` and `scalable.rs` use `literal_id()` in the hot path.

**History note:** even the initial port used `canonical_name()` for indexing/proven sets, which also ignored predicate args (see `git show 6f6f627:.../literal.rs` and `.../index.rs`), so this is a design gap, not a regression from the perf commits—though perf work entrenched the same assumption.

### 1.2 Variables are not grounded in outputs
**Observed:** `want_action(... ?doc ?hash)` is emitted rather than two grounded actions.  
**Root cause:** `ground_theory()` exists but is never called in `reason`, `query`, `why-not`, `requires`, etc. (`ground_theory` references are confined to `grounding.rs` tests).

### 1.3 Grounding, as implemented, does not preserve superiority semantics across instances
**Observed by inspection:** `ground_theory_with_limit()` appends `_N` to labels for instances, but copies superiority relations unchanged (`r2 > r1`), which no longer reference actual instance labels.  
- Copying superiorities without remap: `crates/spindle-core/src/grounding.rs:409`  
- Reasoner checks superiority by rule labels: `crates/spindle-core/src/reason.rs:249`

Docs explicitly state “superiority applies to the rule template, affecting all ground instances”, but current behavior cannot achieve that once grounding is wired.

### 1.4 CLI JSON contract drift
- CLI has a `--json` flag, docs claim `spindle reason --json penguin.dfl` outputs JSON, but `run_reason()` ignores json and always prints text.  
  - Flag: `crates/spindle-cli/src/main.rs:39`  
  - Reason path: `crates/spindle-cli/src/main.rs:156` and `crates/spindle-cli/src/main.rs:240`  
  - Docs claim JSON: `docs/src/getting-started.md:137`

### 1.5 Parser/doc mismatches (modal + temporal)
Docs show nested forms like `(must (report-hours ?x))` and `(during (employed alice acme) (moment 2020) (moment 2022))`.  
But SPL parser currently requires all predicate args be atoms (no nested lists): `crates/spindle-parser/src/spl.rs:348`.  
Separately, temporal default values produce noisy `[-inf,-inf]` suffixes due to using `Default::default()` instead of `Temporal::empty()` in SPL parsing.

---

## 2) Goals / Non‑Goals

### Goals (P0)
1) **Correct first‑order predicate instance semantics**
   - `p(a)` and `p(b)` must be distinct for indexing, proving, conflict detection, and outputs.
2) **Variables produce grounded conclusions**
   - Grounding must occur automatically (or be enforced) before reasoning and query operators.
3) **Superiority must apply correctly under grounding**
   - `prefer r2 r1` must influence all grounded instances of template rules.
4) **Stable, parseable `reason` output**
   - `reason` must support JSON output (CLI and WASM should converge on a stable schema).
5) **Regression tests**
   - Add tests that fail under current behavior and pass under the new design.

### Goals (P1)
6) **Align SPL parser and docs for modal/temporal surface syntax**
   - Either implement the documented syntax, or explicitly narrow docs to the implemented subset.
7) **Remove misleading temporal defaults**
   - `Temporal` “empty/unbounded” should not serialize as `[-inf,-inf]` for non-temporal literals.
8) **Timepoint (“as-of”) temporal semantics**
   - Support reasoning/querying at an explicit `reference_time` (filter active facts/rules) to align with workflow engines that reason “as of now”.

### Non‑Goals (initially)
- Fully implementing *interval-set* temporal inference semantics inside defeasible reasoning (e.g. union/splitting of derived temporals, full Allen-constraint solving). This RFC proposes a roadmap: start with timepoint (“as-of”) filtering + stable representation, then evaluate whether interval inference is required.

---

## 3) Proposed Design (Correctness‑first)

### 3.1 Replace “name-only” literal identity with a real **AtomKey / LitKey**
We need an internal identity used everywhere the engine currently uses `LiteralId` / `canonical_name()`:

**Define:**
- `AtomKey` (negation-free) = `{ mode, functor, args }`
  - `mode`: `(mode_name, mode_negation)` (should become interned IDs, not heap Strings)
  - `functor`: `SymbolId` (existing string interner)
  - `args`: `Vec<SymbolId>` (or `SmallVec`)
- `LitKey` = `AtomKey + negation` (negation bit)

**Key property:** two literals unify/compare equal for reasoning iff:
- same functor,
- same arity,
- same args (after grounding),
- same mode (if modal operators are supported as first-class),
- same negation.

**Temporal in identity:** for P0, keep temporal excluded from `AtomKey` / `LitKey` (so indexing stays fast and identity is stable). Temporal becomes semantically meaningful via:
- explicit `reference_time` filtering (see §3.6), and
- structured outputs (so callers can reason about/visualize temporal bounds),
with full interval inference deferred to a follow-on RFC.

**Implementation approach (recommended): per‑theory interning**
- Extend `IndexedTheory` to build a compact ID space for `AtomKey`:
  - map `AtomKey → AtomId (u32)`
  - represent `LitId` as `(AtomId, negation)` (bit-pack, like current `LiteralId` but *atom-based* rather than *symbol-based*)
- Update all hot paths to use `LitId`:
  - `IndexedTheory` indexes by `LitId`
  - `reason.rs` / `scalable.rs` proven sets use `LitId`
  - complement is `LitId ^ NEGATION_BIT`

This preserves performance characteristics while fixing semantics.

**API cleanup (breaking allowed)**
- Rename `Literal::literal_id()` → `Literal::name_literal_id()` and document it as **name+negation only** (not valid identity for predicates/modes; not to be used for reasoning/indexing).
- Remove `canonical_name()` (it is not canonical for predicates/modes). Use `Display` / `literal_spl` rendering instead.
- Introduce `AtomId`/`LitId` as the only supported identities for indexing/proven-sets.
  - Note: `LitId` is **prepared-theory scoped** (you obtain it from `PreparedTheory` / `IndexedTheory`, not from a bare `Literal`).

### 3.2 Wire grounding into all public reasoning operations
Public operations include:
- `reason` (standard/scalable)
- `query`
- `why_not`
- `explain`
- `requires` / `abduce`
- `what_if`

**Rule:** if a theory contains variables, it must be grounded before any of the above.

Add a single entrypoint:
- `fn prepare(theory: &Theory, opts: PrepareOptions) -> PreparedTheory`
  - performs validation (range restriction, wildcard rules)
  - applies timepoint (“as-of”) filtering when `opts.reference_time` is set (Phase T1)
  - grounds if needed (using only the active facts/rules if `reference_time` is set)
  - builds `IndexedTheory` over the prepared theory
  - returns `{ prepared_theory, index, evaluated_at, grounding_report }`

**PrepareOptions (v1)**
- `reference_time: Option<TimePoint>` (if `Some`, enable Phase T1 semantics)
- `grounding: { enabled: bool, max_iterations: u32, max_instances: u32 }`
- `validation: { enforce_range_restricted: bool, reject_wildcard_in_head: bool }`

**PreparedTheory (v1)**
- `theory: Theory` (filtered + grounded; no variables if grounding enabled)
- `index: IndexedTheory` (AtomId/LitId-based)
- `evaluated_at: Option<TimePoint>`
- `grounding_report: { performed: bool, had_variables: bool, instances: u32, limit_hit: bool }`

All downstream operations operate on `PreparedTheory`.

**Validation: range restriction**
Given docs state head vars must appear in body, enforce:
- For each rule: variables in head must be subset of variables in body.
- Wildcard `_` is treated as an “anonymous variable” (matches anything, never binds; each occurrence is independent). Reject `_` in rule heads (it cannot produce meaningful grounded outputs).

### 3.3 Fix grounding semantics for superiority + metadata (template-aware)
We need superiority comparisons to work for grounded instances without exploding superiority edges.

**Add to `Rule`:**
- `template_label: RuleLabel` (or `origin_label`)
  - default: same as `label`
  - grounded instance: `label = "<template>_<n>"`, `template_label = "<template>"`

**Superiority semantics:**
- superiority relations remain defined over **template labels**
- when checking superiority between two rules, compare their `template_label`s

Update:
- `Theory::is_superior(superior, inferior)` stays template-based.
- Reasoner’s superiority checks must pass template labels, not instance labels:
  - `theory.is_superior(attacker.template_label(), rule.template_label())`

**Metadata semantics:**
- SPL `(meta r1 ...)` attaches to template label `r1`.
- Explanations for an instance `r1_17` should still show meta from `r1`.
- Implement lookup:
  - `theory.get_meta(rule.label)` else `theory.get_meta(rule.template_label)`

**Grounder changes:**
- When applying substitution, preserve `template_label`.
- Ensure derived ground rules carry template label.
- Optionally store substitution for debugging/explainability (not required but helpful).

### 3.4 SPL surface syntax alignment (modal + temporal)
**Decision (v1):** implement SPL sugar that lowers into existing core `Mode` and `Temporal` fields.

- Modal sugar:
  - `(must <lit>)` → `<lit>` with `mode = obligation`
  - `(may <lit>)` → `mode = permission`
  - `(forbidden <lit>)` → `mode = forbidden`
- Temporal sugar:
  - `(during <lit> <start> <end>)` → `<lit>` with `temporal = [start,end]`

This requires the parser to accept `<lit>` as a literal expression (atom or list), i.e. nested parsing (currently disallowed). Implement by parsing the second position as a literal, not an atom.

**Time points (`moment`)**
**Decision (v1):** support only `(moment "RFC3339")` and reject other arities initially.
- Internally, represent time as `TimePoint::Moment(i64)` where the integer is **milliseconds since Unix epoch** (UTC).
- Also accept bare numeric timepoints as integer epoch-millis in SPL (explicitly documented as such).

Supported time expressions (v1):
- `-inf`, `inf`
- `NUMBER` (epoch millis)
- `(moment "2026-02-06T10:00:00Z")` (RFC3339)

**Doc alignment note (important):** current docs include forms like `(during bird ?t)` (single interval variable) in examples (e.g. `docs/src/guides/temporal.md:125`), but the supported/implemented model is `(during <lit> <start> <end>)`. This RFC requires updating docs to remove or correct the single-arg interval-variable form.

**Allen relations (`before/overlaps/...`)**
- **Decision (v1):** treat Allen relations as *deferred*. Without a concrete representation of interval variables inside the core, they cannot be evaluated soundly.
- If/when interval variables are introduced in a follow-on RFC, re-introduce Allen relations with a defined execution model (either builtin evaluation or lowering to explicit facts produced by projection).

**Important:** if temporal remains excluded from literal identity, temporal cannot discriminate `p(a)[t1]` from `p(a)[t2]` *as distinct atoms* without additional temporal reasoning machinery. Timepoint filtering (`reason_at`) provides practical semantics for workflow use-cases without requiring interval-set inference.

**Implementation detail:** prefer making `Temporal::default()` equal `Temporal::empty()` (and avoid using `Default::default()` for temporals in parsing). This prevents accidental `[-inf,-inf]` “non-empty” defaults leaking into outputs.

**Non-goal (v1):** “narrow docs to match an underpowered parser.” The goal is to implement the documented sugar (within the v1 constraints above) and then correct the docs where they are internally inconsistent.

### 3.5 Make CLI/WASM output stable and structured

**CLI**
- Implement JSON output for `reason`:
  - `spindle reason <file> --json` and `spindle --json reason <file>`
  - schema aligned with WASM and query outputs

**Recommended JSON schema (v1)**

Top-level requirements:
- Always include `schema_version`
- Always include `evaluated_at` when `reference_time` filtering is enabled (Phase T1), otherwise omit or set to `null`
- Always include `grounding` with limit/summary data
- Always include `conclusions` (stable order; recommended: sort by `literal_spl`)

```json
{
  "schema_version": "spindle.reason.v1",
  "evaluated_at": "2026-02-06T10:00:00Z",
  "grounding": {
    "performed": true,
    "had_variables": true,
    "instances": 42,
    "limit_hit": false
  },
  "conclusions": [
    {
      "conclusion_type": "+d",
      "literal_spl": "(want_action classify_doc doc doc_1 h1)",
      "literal_struct": {
        "mode": { "name": "", "negated": false },
        "negated": false,
        "functor": "want_action",
        "args": ["classify_doc", "doc", "doc_1", "h1"],
        "temporal": { "start": "-inf", "end": "+inf" }
      },
      "positive": true
    }
  ],
  "diagnostics": [
    {
      "severity": "warning",
      "code": "GROUNDING_LIMIT_HIT",
      "message": "Grounding stopped early due to max_instances.",
      "details": { "max_instances": 1000 }
    }
  ],
  "stats": { "rule_count": 123, "fact_count": 45 }
}
```

**Notes**
- `literal_spl` is the canonical string form (SPL s-expression).
- `literal_struct` is the canonical machine form (required).
- `literal` (if kept) must be treated as a deprecated alias of `literal_spl` to avoid dual sources of truth.

**WASM**
- Ensure `Spindle.reason()` returns the same per‑conclusion structure (it already returns JSON-ish via `JsValue`, but should match CLI).

**String rendering**
- Define canonical renderers for `literal` strings:
  - SPL s-expression: `(p a b)` (recommended default for CLI/JSON, matches integrator ergonomics)
  - Prolog-ish: `p(a, b)` (optional legacy/human format)
- no trailing temporal unless non-empty (after temporal default fix),
- stable comma spacing,
- explicit quoting/escaping rules if atoms can contain special chars.
- If we add `literal_struct`, the string is mostly for humans.

### 3.6 Temporal semantics roadmap (recommended)
This RFC intentionally separates **timepoint (“as-of”) semantics** from **full interval inference**.

**Phase T0 (P1): Make temporals representable without noise**
- Parser uses `Temporal::empty()` by default (and `Temporal::default()` should equal empty).
- JSON outputs include `literal_struct.temporal` when present, but omit temporal suffixes in string renderers when empty.

**Phase T1 (recommended next step): Timepoint (“as-of”) reasoning**
Add `reference_time` to `prepare()` / public APIs so callers can ask “what is provable at time *t*?”:
- **Reference time representation**
  - External/JSON/CLI form: RFC3339 timestamp string (UTC recommended), e.g. `"2026-02-06T10:00:00Z"`.
  - Internal form: `TimePoint::Moment(i64)` where `i64` is **milliseconds since Unix epoch** (UTC).
- **Active semantics**
  - `Temporal::empty()` is `[-inf, +inf]` (always active).
  - `active_at(t)` uses inclusive endpoints: `start <= t <= end`.
- Filter givens/facts and rule literals by `temporal.active_at(reference_time)` prior to reasoning.
  - Rule firing at time `t` requires all **body** literals be active at `t`.
  - A rule can only derive a **head** literal that is active at `t` (inactive head literals are ignored at that timepoint).
  - Conflicts/defeat only apply between complements that are both active at `t`.
- This yields correct workflow behavior for “overlap ⇒ conflict / disjoint ⇒ distinct evidence” *at a timepoint*, without needing interval unions/splitting.
- This matches workflow engines that already carry an explicit `reference_time`.

**Phase T2 (follow-on RFC): Interval-aware inference**
If/when needed for doc goals (“derive temporally bounded conclusions, propagate intersections, overlap-gate conflicts across intervals”), implement interval-aware propagation:
- Derived head temporal = intersection of head/body temporals.
- Conflicts/defeat only apply when temporals intersect.
- **Warning:** correctness quickly requires *interval-set* tracking (unions and possible splitting) once multiple independent supports exist; this is intentionally deferred.

---

## 4) Implementation Plan (milestones)

### Milestone 0: Regression tests first (must fail on current main)
Add failing tests demonstrating:
1) **Argument discrimination**
   - fact only for `p(a)` must not satisfy `p(b)`
2) **Grounding integration**
   - variable rules produce grounded conclusions (no `?x` in derived heads)
3) **Grounding + superiority**
   - `prefer r2 r1` applies to grounded instances
4) **CLI `--json` for reason**
   - `spindle --json reason file.dfl` outputs JSON parseable
   - `spindle reason file.dfl --json` outputs JSON parseable
5) **Timepoint (“as-of”) temporal filtering (when implemented)**
   - facts/rules outside `reference_time` are ignored
   - complements only conflict when both are active at `reference_time`

**Acceptance criteria**
- Each test has a minimal theory input and asserts an observable behavior (not just “no panic”).
- On the current main branch (pre-fix), at least one of these tests fails for the documented reason (arg collapse / missing grounding / superiority mismatch / JSON drift).
- After Milestones 1–4 (and 5 for temporal), the entire new regression suite passes.

### Milestone 1: Core identity refactor (AtomKey/LitId)
- Implement per-theory atom interner in `IndexedTheory`
- Update:
  - `index.rs`
  - `reason.rs`
  - `scalable.rs`
  - any other modules relying on `literal_id()` for reasoning state
- Stop using `canonical_name()` and name-only `literal_id()` in reasoning/indexing.

**Acceptance criteria**
- `p(a)` and `p(b)` are distinct in all indexes and proven-sets (standard + scalable).
- The earlier “doc_1/doc_2” reproduction cannot occur: rules only trigger when the *exact* atom (functor+args+mode+negation) is proven.
- Standard and scalable algorithms remain semantically equivalent on the existing test corpus.

### Milestone 2: Grounding pipeline wired + superiority correctness
- Add `prepare()` step and route all operations through it.
- Fix grounder to preserve template labels.
- Update superiority checks in reasoning to use template labels.

**Acceptance criteria**
- Every public operation (`reason/query/why_not/explain/abduce/what_if`) routes through `prepare()`.
- If the input theory contains variables, outputs contain no unbound variables (no `?x`, no `_`) when grounding is enabled.
- `prefer r2 r1` applies to all grounded instances (template-aware superiority works).
- Metadata lookups remain correct for grounded instances (`r1_17` sees meta from `r1`).

### Milestone 3: Parser alignment for modal/temporal sugar
- Extend SPL parser to accept nested literal expressions for `must/may/forbidden/during`
- Fix temporal defaults (`Temporal::empty()` not `Default::default()`)
- Add parser tests for:
  - `(must pay)` producing mode `[O]pay`
  - `(during (p a) (moment "2024-01-01T00:00:00Z") inf)` parsing without nested-arg rejection
- Implement `moment` v1: `(moment "RFC3339")` only (see §3.4) and document.

**Acceptance criteria**
- The parser accepts the documented sugar forms and lowers them into core `Mode`/`Temporal` fields.
- Non-temporal literals do not acquire noisy/default temporal bounds in outputs (no `[-inf,-inf]` leakage).
- Docs are updated to remove/repair the inconsistent `(during bird ?t)` example and any Allen-relation claims that aren’t implemented.

### Milestone 4: Output contracts
- Implement JSON output for `reason` (CLI + docs)
- Add CLI integration tests
- Align wasm output schema

**Acceptance criteria**
- `spindle --json reason <file>` and `spindle reason <file> --json` emit valid JSON with `schema_version: "spindle.reason.v1"`.
- JSON includes `grounding` and per-conclusion `literal_struct` + `literal_spl` for every conclusion.
- CLI docs match behavior (no “`--json` works” claims that are untrue).
- WASM output schema matches CLI (field names and meanings).

### Milestone 5 (P1): Timepoint (“as-of”) temporal filtering
- Extend `prepare()` / public APIs with `reference_time`
- Filter facts/rules by `temporal.active_at(reference_time)`
- Add/enable the Phase T1 regression tests from Milestone 0

**Acceptance criteria**
- `evaluated_at` is present in JSON output when `reference_time` is supplied and matches the provided RFC3339 time.
- At a fixed timepoint, disjoint temporals do not conflict; overlapping temporals only conflict when both are active at that timepoint.
- Boundary behavior is stable and tested (inclusive endpoints).

---

## 5) Test Coverage Plan (to prevent recurrence)

### 5.1 Unit tests (spindle-core)
Add focused tests in `crates/spindle-core/src/reason.rs` (or a new `reason_tests.rs`) that assert:

- **Predicate arg separation**
  - Theory:
    - facts: `p(a)`
    - rule: `p(b) => q`
  - Expect: `q` not provable.

- **Predicate arg propagation**
  - facts: `parent(alice,bob)`, `parent(bob,charlie)`
  - rule: `parent(?x,?y) => ancestor(?x,?y)`
  - Expect: `ancestor(alice,bob)` and `ancestor(bob,charlie)` provable; no `ancestor(?x,?y)`.

- **Superiority template semantics under grounding**
  - conflicting rules with variables and `prefer`
  - ensure defeasible proof respects `prefer` across instances.

- **Timepoint (“as-of”) temporal semantics**
  - given two opposing facts with disjoint temporals, `reason_at(t)` should only see the active one (no conflict at that timepoint)
  - boundary semantics are explicit and tested (inclusive endpoints match `Temporal::active_at`)

- **Wildcard `_`**
  - `_` matches any term but does not create a binding
  - multiple `_` occurrences do not need to unify with each other

### 5.2 Property tests (optional but valuable)
- Generate random small grounded theories with multiple predicate instances and assert:
  - engine results are invariant under permutation of unrelated facts
  - no rule can fire unless its *exact* body literals are provable (including args).

### 5.3 CLI integration tests (spindle-cli)
- Add tests for:
  - `spindle --json reason file.dfl` is valid JSON and contains expected keys
  - `spindle reason file.spl --json` same
  - `--positive` behavior consistent in JSON and text modes

### 5.4 WASM tests
- Ensure `Spindle.reason()` returns JSON array with conclusion_type/literal/positive.
- Add at least one test using predicates-with-args and one using variables (once grounding is wired in WASM path).

---

## 6) Compatibility / Breaking Changes

Expected breaking changes (acceptable under correctness-first):
- Any downstream that assumed `p(a)` and `p(b)` collapse will change (this is a bug fix).
- `literal_id()` / `canonical_name()` semantics likely change or are deprecated.
- Output may change:
  - fewer/no `[-inf,-inf]` suffixes after temporal default fix
  - new JSON shape for `reason`

---

## 7) Risks / Performance Considerations

- Correct identity will increase the number of distinct atoms; bitset sizes grow with atom count, not symbol count. That is intended.
- Grounding can blow up; must enforce:
  - range restriction
  - max grounding iterations / max generated instances
  - diagnostic reporting when limits are hit
- Superiority template semantics avoids exploding superiority edges, but requires consistent handling in explanation/trust modules.

---

## 8) Recent Git History Notes (for context)
- Perf commits around Feb 2–4, 2026 introduced bitset + `LiteralId` optimizations (`82ae834`, `a8b4e07`, `cedc0fb`), but the underlying “name-only key” assumption existed in the initial port (`6f6f627`) via `canonical_name()` ignoring predicate args.
- Grounding is present and has edge-case tests (`5d40f0b`), but never wired into the reasoning/query pipeline.

---

## 9) Deferred / Follow-on RFCs

These items are intentionally deferred to keep the initial refactor correct, testable, and shippable:

1) **Phase T2 interval-aware inference**
   - Propagate derived temporals by intersection, and overlap-gate conflict/defeat across intervals.
   - Likely requires interval-set tracking (union/splitting) once multiple independent derivations exist.
2) **Interval variables + Allen relations**
   - A concrete representation of interval variables inside the core, plus a defined execution model for `before/overlaps/...`.
3) **General `Term` AST / compound terms**
   - Only introduce when there’s a demonstrated need for nested terms in predicate arguments beyond the sugar forms.
4) **Expanded `(moment ...)` parsing**
   - Multi-arity calendar forms `(moment 2024 6 15 ...)`, timezones, and non-UTC parsing rules.
5) **Richer structured outputs**
   - Proof/explanation payloads, rule-instance substitution reporting, and stable “why-not/abduce” JSON schemas aligned with `reason`.

---

If you want, I can also turn this into a “tracking checklist” version (P0/P1 tasks with acceptance criteria per crate), or tailor the spec to your preferred upstream workflow (single RFC vs a series: identity/grounding first, then parser/output).
