| Field | Value |
|---|---|
| Document ID | SPEC-021 |
| Title | Unified Evidence Resolution: Shared Primitives for Temporal and Base Reasoning |
| Version | 1.0.0 |
| Status | Draft |
| Created | 2026-03-12 |
| Last Updated | 2026-03-12 |
| Authors | Claude (AI agent) |
| Reviewers | David Factor, Hugo O'Connor, Core Maintainers |
| Protocol | [USDD Agent Protocol v1.3.0](../../handbook/engineering/usdd-agent-protocol.md) |
| Traces | SPEC-020 (supersedes), SPEC-018, BUG-001, branch `hence/plan-IMPL-020` |

---

# SPEC-021: Unified Evidence Resolution — Shared Primitives for Temporal and Base Reasoning

## 1. Executive Summary

### 1.1 Problem

SPEC-020 correctly identified that synthetic bridge rules are the wrong abstraction for temporal-to-base projection. The implementation on `hence/plan-IMPL-020` replaced bridge rules with a projection engine that emits first-class support/attack tokens. That projection engine works, but it introduced a different structural problem: **it added a parallel semantics path alongside the core reasoner rather than extending the core reasoner's own primitives**.

The result is that evidence matching and attacker discovery logic now exists in three distinct implementations:

1. **Core reasoner** (`reason/defeasible.rs`): counter-based body satisfaction, index-based attacker lookup via `rules_with_head_id()`, implicit family-aware matching through `LitId`.
2. **Query operators** (`query/why_not.rs`, `query/abduce.rs`): conclusion-scanning body satisfaction via `has_positive_match()`, brute-force attacker discovery via theory iteration, explicit `exact_literal_match()` for attacker heads.
3. **Projection engine** (`projection.rs`, `shadow.rs`, `compilation.rs`): post-hoc token emission with `BodyMatchKey` compilation, `FamilyId`/`ExactLitId` types, diagnostic divergence detection.

These three implementations have subtle semantic discrepancies:

- The reasoner finds attackers via atemporal `LitId` family lookup; `why_not` uses `exact_literal_match()` and may miss temporal defeaters.
- The reasoner tracks body satisfaction via counters with discard state; query operators scan conclusions without discard awareness.
- The reasoner distinguishes strict from defeasible attackers in superiority handling; `why_not` collapses them.
- The projection engine's `CompiledBody` and `BodyMatchKey` types are not consumed by the core reasoner at all.

As David Factor observed: "this feature crosses a semantic seam that is currently split across multiple layers instead of being owned in one place."

### 1.2 Proposed Redesign

This spec proposes a smaller, more fundamental change than SPEC-020: **introduce one shared internal abstraction for evidence matching and attacker discovery**, then make temporal reasoning an extension that plugs into that abstraction rather than a parallel engine.

The redesign has three parts:

1. **`EvidenceResolver` trait**: a shared interface that encapsulates "is this literal supported?", "what attacks this literal?", and "is this attacker applicable?" — consumed by the reasoner, `why_not`, `abduce`, and `requires` alike.

2. **Core/extension boundary**: the reasoner provides extensible hooks for literal identity and match semantics. Temporal family matching becomes one implementation of those hooks, not a fork of the reasoning algorithm.

3. **Projection as observability only**: the projection engine remains as a diagnostic/audit tool but does not introduce types (`CompiledBody`, `BodyMatchKey`) that compete with the core reasoner's own matching infrastructure.

### 1.3 Outcome

If adopted, this design:

- Eliminates the three-implementation semantic seam
- Keeps `FamilyId` and `ExactLitId` from SPEC-020 (they are correct)
- Removes `CompiledBody`/`BodyMatchKey`/`ShadowReasoner` as reasoning-adjacent infrastructure
- Makes `why_not`, `abduce`, and `requires` semantically consistent with the core reasoner by construction
- Positions temporal reasoning as an extension on shared primitives, enabling Claire's smaller-core design

### 1.4 Relationship to SPEC-020

This spec **supersedes** SPEC-020. It preserves:

- The `AtomKey` identity fix from SPEC-018
- The `FamilyId` and `ExactLitId` types from SPEC-020
- The removal of synthetic bridge rules from SPEC-020
- The test suites (regression, property-based, differential)

It replaces:

- The `ProjectionEngine` as a parallel semantics path → becomes observability-only
- The `CompiledBody`/`BodyMatchKey` compilation layer → replaced by `EvidenceResolver` trait
- The `ShadowReasoner` → replaced by differential test infrastructure (no runtime shadow)
- The three-tier query/reason/projection split → unified through shared trait

---

## 2. Problem Statement and Context

### 2.1 What SPEC-020 got right

SPEC-020 correctly diagnosed that synthetic bridge rules are a lossy encoding of temporal-to-base projection semantics. The solution — `FamilyId` as base-family identity and `ExactLitId` as exact temporal identity — is sound. These types should be retained.

### 2.2 What SPEC-020 got wrong

SPEC-020 framed the problem as "projection should be first-class" and solved it by building a parallel projection engine. But the real problem is that evidence matching and attacker discovery were already duplicated between the reasoner and query operators. SPEC-020 added a third implementation instead of unifying the existing two.

Specifically:

**The projection engine introduced types the reasoner doesn't use.**

`CompiledBody` and `BodyMatchKey` represent a body-matching strategy (exact vs. family), but the core reasoner in `reason/defeasible.rs` uses counter-based body satisfaction. The compiled bodies are only consumed by `shadow.rs` for diagnostic comparison. This means the system has two independent models of "how to check if a body is satisfied":

1. The reasoner's `body_remaining` counters (implicit, efficient, but opaque)
2. The compilation module's `BodyMatchKey` enum (explicit, unused by reasoning)

**The shadow reasoner is verification, not extension.**

`ShadowReasoner` runs the standard reasoner first, then runs projection post-hoc, then compares. It never changes conclusions. This is useful for testing but it is not an architectural extension — it is a test harness that happens to live in production code.

**Query operators still don't share matching logic with the reasoner.**

After SPEC-020, `why_not` still loops all theory rules with `exact_literal_match()` instead of using the index. `abduce` still wraps `has_positive_match()` in trivial wrappers. Neither uses `FamilyId` or `ExactLitId` directly — they go through `semantic_literal_matches()` which internally constructs `FamilyId` for comparison but doesn't use the indexed lookup.

### 2.3 David Factor's diagnosis

> "The awkwardness is not that the codebase needs a full rewrite first. It is that this feature crosses a semantic seam that is currently split across multiple layers instead of being owned in one place."

The useful prep change is:

1. Introduce one shared internal abstraction for evidence matching and attacker discovery.
2. Make `reason`, `why_not`, `abduce`, and `requires` call that same abstraction.
3. Keep projection/shadow as observability, not as a second source of semantics.
4. Consider whether temporal stuff can be implemented as an extension on preexisting primitives.

### 2.4 Redesign objective

The system SHALL provide a single shared abstraction for evidence matching and attacker discovery that all reasoning and query consumers use, and SHALL allow temporal family matching to be expressed as an extension of that abstraction rather than a parallel implementation.

---

## 3. Scope

| In Scope | Out of Scope |
|---|---|
| Unified `EvidenceResolver` trait consumed by reasoner and query operators | Parser syntax changes |
| Core/extension boundary for literal identity and match semantics | New temporal operators beyond existing `(during ...)` |
| Temporal family matching as an extension implementation | Changes to user-facing SPL syntax |
| Retention of `FamilyId`/`ExactLitId` from SPEC-020 | CLI contract versioning |
| Removal of `CompiledBody`/`BodyMatchKey`/`ShadowReasoner` as reasoning infrastructure | Full rewrite of the reasoner's forward-chaining algorithm |
| Preservation of all existing test suites | Lean formal verification changes |

---

## 4. Architecture Overview

### 4.1 Core model: EvidenceResolver

The central abstraction is a trait that answers three questions:

1. **Support**: "Is literal `q` supported by available evidence?"
2. **Attack**: "What rules attack the complement of `q`, and are they applicable?"
3. **Superiority**: "Given supporter `s` and attacker `a`, does `s` defeat `a`?"

```rust
/// Shared abstraction for evidence matching and attacker discovery.
/// Consumed by the reasoner, why_not, abduce, and requires.
pub trait EvidenceResolver {
    /// The type of evidence state (conclusions, bitset, counters, etc.)
    type Evidence;

    /// Check if literal has positive support in the evidence.
    fn is_supported(&self, literal: &Literal, evidence: &Self::Evidence) -> bool;

    /// Find all applicable attackers for a literal.
    /// Returns rules whose head matches the complement of `literal`
    /// and whose body is satisfied in the evidence.
    fn find_applicable_attackers(
        &self,
        literal: &Literal,
        evidence: &Self::Evidence,
    ) -> Vec<ApplicableAttacker>;

    /// Check if supporter defeats attacker via superiority.
    fn is_defeated(
        &self,
        supporter: &RuleLabel,
        attacker: &ApplicableAttacker,
        theory: &Theory,
    ) -> bool;
}
```

### 4.2 Two implementations, shared semantics

**`IndexedResolver`** — used during forward-chaining reasoning:
- `is_supported()` checks `LiteralBitSet` (O(1) via `proven.contains(lit_id)`)
- `find_applicable_attackers()` uses `indexed.rules_with_head_id(complement)` + counter-based applicability
- `is_defeated()` delegates to `theory.is_superior()` using template labels
- Family-aware: attacker lookup uses `LitId` which groups temporal variants

**`ConclusionResolver`** — used by query operators over computed conclusions:
- `is_supported()` scans conclusion list with match-mode-aware semantics
- `find_applicable_attackers()` uses `indexed.rules_with_head_id(complement)` (same index as reasoner) + conclusion-scanning body check
- `is_defeated()` delegates to `theory.is_superior()` using template labels
- Family-aware: uses same `FamilyId` matching as `IndexedResolver`

The critical point: **both implementations use the same index for attacker discovery** and the **same match semantics for family-awareness**. The difference is only in how they represent "what's been proven" — bitset during reasoning, conclusion list after reasoning.

### 4.3 Match semantics as extension point

Literal matching is parameterized by a `MatchPolicy`:

```rust
pub trait MatchPolicy {
    /// Determine match mode for a literal (exact, family, wildcard).
    fn match_mode(&self, literal: &Literal) -> MatchMode;

    /// Check if `candidate` satisfies `expected` under the given mode.
    fn matches(&self, expected: &Literal, candidate: &Literal, mode: MatchMode) -> bool;
}

pub enum MatchMode {
    /// Exact identity required (including temporal bounds)
    Exact,
    /// Family identity sufficient (temporal bounds ignored)
    Family,
    /// Wildcard: prefer exact, fall back to family
    Wildcard,
}
```

**Default implementation** (`StandardMatchPolicy`):
- Atemporal literals use `Family` mode
- Temporal literals use `Exact` mode
- Wildcard used for user-facing queries

**Temporal extension** (`TemporalMatchPolicy`):
- Extends `StandardMatchPolicy`
- Adds `FamilyId`-based matching for `Family` mode
- Adds `ExactLitId`-based matching for `Exact` mode
- This is where `FamilyId` and `ExactLitId` from SPEC-020 live

### 4.4 Core/extension boundary

```text
CORE (reason crate, no temporal knowledge):
├── EvidenceResolver trait
├── MatchPolicy trait
├── IndexedTheory (with pluggable identity)
├── Reasoner (forward-chaining, calls EvidenceResolver)
├── QueryOperators (why_not, abduce, requires — call EvidenceResolver)
└── Superiority (label-based, unchanged)

TEMPORAL EXTENSION:
├── FamilyId (from SPEC-020, retained)
├── ExactLitId (from SPEC-020, retained)
├── TemporalMatchPolicy (implements MatchPolicy)
├── family_body_index in IndexedTheory (extended index)
└── Temporal types (Temporal, TimePoint, Allen relations)

OBSERVABILITY (diagnostic, never changes conclusions):
├── ProjectionEngine (audit token emission)
├── ProjectionToken types (ExactSupport, FamilySupport, FamilyAttack)
└── Differential test infrastructure (replaces ShadowReasoner)
```

### 4.5 Why this is simpler than SPEC-020

```text
SPEC-020 approach:
  reasoner (counters) ─── own matching logic
  query ops (conclusions) ─── own matching logic
  projection engine (tokens) ─── own matching logic
  → 3 implementations, 5 discrepancies

SPEC-021 approach:
  EvidenceResolver trait ─── shared matching semantics
  ├── IndexedResolver (counters) ─── used by reasoner
  └── ConclusionResolver (conclusions) ─── used by query ops
  projection engine ─── observability only, no matching logic
  → 1 interface, 2 implementations, 0 semantic discrepancies
```

### 4.6 Conceptual diagram

```text
                  ┌─────────────────────────┐
                  │      MatchPolicy        │
                  │  (Standard / Temporal)   │
                  └────────────┬────────────┘
                               │
                  ┌────────────▼────────────┐
                  │    EvidenceResolver      │
                  │  is_supported()          │
                  │  find_applicable_attackers()│
                  │  is_defeated()           │
                  └────────────┬────────────┘
                               │
              ┌────────────────┼────────────────┐
              ▼                ▼                 ▼
    ┌─────────────────┐ ┌───────────────┐ ┌───────────────┐
    │ IndexedResolver │ │ Conclusion    │ │ (future ext)  │
    │ (reasoning)     │ │ Resolver      │ │               │
    │ bitset+counters │ │ (query ops)   │ │               │
    └─────────────────┘ └───────────────┘ └───────────────┘
```

---

## 5. Purity Boundary Map

### Pure Core

- `EvidenceResolver` trait definition and implementations
- `MatchPolicy` trait definition and implementations
- Literal identity types (`FamilyId`, `ExactLitId`)
- `IndexedTheory` indexing with family and exact lookups
- Superiority resolution (label-based, unchanged)
- Forward-chaining algorithm (calls `EvidenceResolver` instead of inlining matching)
- Query operators (`why_not`, `abduce`, `requires` — call `EvidenceResolver`)

### Effectful Shell

- SPL parsing and source loading
- Pipeline orchestration
- CLI/WASM presentation of conclusions
- Diagnostics emission
- Trust metadata retrieval
- Projection engine (observability, audit trail)

### Boundary rule

The pure core SHALL contain exactly one trait definition for evidence matching and attacker discovery. All consumers of evidence matching (reasoning, explanation, abduction, requirements) SHALL call that trait. No consumer SHALL inline its own matching logic.

---

## 6. Functional Requirements

### REQ-001: Shared evidence resolution interface

The system SHALL provide a single `EvidenceResolver` trait that encapsulates evidence matching and attacker discovery.

Acceptance criteria:

- The trait is defined in one module
- The reasoner's defeasible phase calls the trait for attacker discovery
- `why_not` calls the trait for attacker discovery
- `abduce` calls the trait for body satisfaction checks
- No consumer inlines its own attacker-matching loop

Trace:
- CON-001
- TEST-001
- TEST-002

### REQ-002: Consistent attacker discovery across consumers

All consumers of the `EvidenceResolver` SHALL discover the same set of attackers for the same literal and evidence state.

Acceptance criteria:

- The reasoner and `why_not` find the same attackers for the same literal (modulo evidence representation)
- Temporal defeaters are found by both consumers when applicable
- Strict vs. defeasible attacker handling follows the same rules in all consumers

Trace:
- CON-001
- TEST-003
- TEST-004

### REQ-003: Family-aware matching via MatchPolicy

The system SHALL parameterize literal matching via a `MatchPolicy` trait, allowing temporal family matching to be expressed as an implementation.

Acceptance criteria:

- Atemporal body literals match via family mode
- Temporal body literals match via exact mode
- The same `MatchPolicy` is used by both `IndexedResolver` and `ConclusionResolver`
- `FamilyId` and `ExactLitId` are used within the `TemporalMatchPolicy` implementation

Trace:
- CON-002
- TEST-005
- TEST-006

### REQ-004: Dual literal identity (retained from SPEC-020)

The system SHALL maintain both exact temporal literal identity and base family identity for every logical literal.

Acceptance criteria:

- Exact identity includes temporal bounds
- Family identity excludes temporal bounds
- Each exact literal maps deterministically to one family

Trace:
- CON-002
- TEST-007
- TEST-PBT-001

### REQ-005: Superiority remains label-based over original rules

The system SHALL evaluate superiority using original rule/template labels only. The `EvidenceResolver` SHALL delegate superiority to the theory's label-based resolution.

Acceptance criteria:

- Superiority is checked via `theory.is_superior(template_label, template_label)`
- No generated or projected labels participate in superiority
- Both `IndexedResolver` and `ConclusionResolver` use the same superiority call

Trace:
- CON-003
- TEST-008

### REQ-006: Trust and provenance remain source-rule derived

All evidence (support and attack) SHALL trace back to original authored rule labels, never to generated compatibility artifacts.

Acceptance criteria:

- Conclusion rule labels reference authored rules
- Trust resolution uses original template labels
- No synthetic bridge labels or projection-generated labels appear in user-facing output

Trace:
- CON-003
- TEST-009

### REQ-007: Projection engine is observability only

The projection engine SHALL NOT introduce types or logic that compete with the `EvidenceResolver` for body matching or attacker discovery. It SHALL exist purely for audit and diagnostic purposes.

Acceptance criteria:

- `ProjectionEngine` does not produce `CompiledBody` or `BodyMatchKey` types consumed by reasoning
- No reasoning path depends on projection token emission
- Projection tokens are available for structured logging, explanation augmentation, and differential testing

Trace:
- CON-004
- TEST-010

### REQ-008: Query semantics expose exact, family, and wildcard intent

The system SHALL provide explicit query matching semantics through the `MatchPolicy`:

- `Exact`: exact temporal match required
- `Family`: any support in the same family
- `Wildcard`: user-facing queries match family members deterministically

Acceptance criteria:

- Bounded temporal queries do not cross-match wrong windows
- Family queries match all temporal variants
- `why_not` uses the same match mode as the user's query

Trace:
- CON-002
- TEST-011
- TEST-012

### REQ-009: Non-temporal theories remain semantically unchanged

The redesign SHALL preserve all conclusions for non-temporal theories.

Acceptance criteria:

- Differential tests against the current engine match on non-temporal inputs
- No new support or attack paths appear in non-temporal theories

Trace:
- TEST-013
- TEST-PBT-002

### REQ-010: Temporal as extension, not fork

Temporal family matching SHALL be implementable as a `MatchPolicy` extension without modifying the core reasoner's forward-chaining algorithm.

Acceptance criteria:

- The core reasoner can run with `StandardMatchPolicy` (no temporal awareness)
- Temporal awareness is introduced by swapping to `TemporalMatchPolicy`
- No `if is_temporal()` branches exist in the core reasoning loop

Trace:
- CON-002
- TEST-014
- TEST-015

---

## 7. Non-Functional Requirements

### NFR-001: Semantic auditability

Reasoning state SHALL remain auditable by tracing support and attack back to authored rule labels through the `EvidenceResolver` interface.

Trace:
- OBS-001
- TEST-016

### NFR-002: Complexity locality

Evidence matching and attacker discovery logic SHALL be localized to the `EvidenceResolver` implementations. No other module SHALL inline its own matching or attacker-discovery loops.

Trace:
- TEST-017

### NFR-003: Deterministic output

Family support selection, trust tie-breaking, and conclusion ordering SHALL be deterministic.

Trace:
- TEST-018
- OBS-002

### NFR-004: Performance parity

The `IndexedResolver` SHALL maintain O(1) body satisfaction checking (via counters/bitset) and O(k) attacker discovery (via index lookup where k = number of rules with matching head).

Trace:
- TEST-019

---

## 8. Contracts

### CON-001: EvidenceResolver contract

Module/Interface:
- `EvidenceResolver` trait

Required capabilities:

- `is_supported(&Literal, &Evidence) -> bool`
- `find_applicable_attackers(&Literal, &Evidence) -> Vec<ApplicableAttacker>`
- `is_defeated(&RuleLabel, &ApplicableAttacker, &Theory) -> bool`

Invariants:

- For the same literal and equivalent evidence state, all implementations return the same set of applicable attackers
- Superiority delegation is identical across implementations
- Family-awareness is determined by the shared `MatchPolicy`, not per-implementation logic

Implements:
- REQ-001
- REQ-002

Verified by:
- TEST-001
- TEST-002
- TEST-003
- TEST-004

### CON-002: MatchPolicy contract

Module/Interface:
- `MatchPolicy` trait

Required capabilities:

- `match_mode(&Literal) -> MatchMode`
- `matches(&Literal, &Literal, MatchMode) -> bool`

Implementations:

- `StandardMatchPolicy`: all literals use exact mode (no temporal awareness)
- `TemporalMatchPolicy`: atemporal literals use family mode, temporal use exact mode

Invariants:

- `TemporalMatchPolicy` produces a superset of matches relative to `StandardMatchPolicy` (family mode is strictly more permissive than exact mode for atemporal queries)
- Match mode is deterministic for a given literal

Implements:
- REQ-003
- REQ-004
- REQ-008
- REQ-010

Verified by:
- TEST-005
- TEST-006
- TEST-007
- TEST-011
- TEST-012

### CON-003: Provenance/trust contract

Module/Interface:
- Trust resolution over evidence

Post-conditions:

- Trust lookup resolves through original template label
- `EvidenceResolver.is_defeated()` uses template labels from the original theory
- No generated labels participate in trust or superiority

Implements:
- REQ-005
- REQ-006

Verified by:
- TEST-008
- TEST-009

### CON-004: Observability boundary contract

Module/Interface:
- `ProjectionEngine` and diagnostic infrastructure

Post-conditions:

- Projection tokens do not influence reasoning conclusions
- No types from the projection module are required by `EvidenceResolver` implementations
- Projection engine consumes conclusions, does not produce them

Implements:
- REQ-007

Verified by:
- TEST-010

---

## 9. Architecture Decisions

### ADR-001: Shared EvidenceResolver trait vs. per-consumer matching logic

Decision:
- Introduce a shared `EvidenceResolver` trait that all evidence consumers call for matching and attacker discovery.

Rationale:
- Three implementations with five known discrepancies (attacker matching, body satisfaction, strict/defeasible handling, discard awareness, family vs. exact matching) demonstrate that per-consumer logic is unsustainable.
- A shared trait with two implementations (indexed for reasoning, conclusion-based for queries) preserves the performance characteristics of each while guaranteeing semantic consistency.

Rejected alternative:
- Keep per-consumer matching and add cross-implementation tests to catch discrepancies.

Rejection reason:
- Testing can catch known discrepancies but cannot prevent new ones. The discrepancies found in the audit were not caught by existing tests.

### ADR-002: MatchPolicy as extension point vs. hardcoded temporal logic

Decision:
- Parameterize literal matching via a `MatchPolicy` trait, allowing temporal family matching to be one implementation.

Rationale:
- David Factor's observation: "temporal stuff might then just be able to be implemented as an extension on preexisting primitives."
- Claire's smaller-core + extension design requires the core to be temporal-agnostic.
- A `MatchPolicy` trait achieves this: the core reasoner calls `policy.matches()` without knowing about temporal bounds.

Rejected alternative:
- Hardcode `FamilyId` matching directly in the core reasoner.

Rejection reason:
- Creates a dependency from the core to temporal types.
- Prevents the core from being reused for non-temporal extensions (modal, probabilistic, etc.).

### ADR-003: Remove CompiledBody/BodyMatchKey from reasoning infrastructure

Decision:
- Remove `CompiledBody` and `BodyMatchKey` as reasoning-adjacent types. The `MatchPolicy` handles match strategy selection.

Rationale:
- `CompiledBody` and `BodyMatchKey` were introduced to pre-compute whether a body literal requires exact or family evidence. But the core reasoner never consumed them — they only existed for `ShadowReasoner`.
- The `MatchPolicy.match_mode()` method provides the same information at the point of use, without requiring a compilation pass.

Retained:
- `FamilyId` and `ExactLitId` — these are identity types, not matching infrastructure. They belong in the temporal extension.

### ADR-004: Projection engine becomes observability-only

Decision:
- Retain `ProjectionEngine` for structured diagnostic output but remove it from the reasoning pipeline as a parallel semantics path.

Rationale:
- Audit confirmed: projection tokens never influence conclusions (ShadowReasoner runs standard reasoner first, projection second, compares post-hoc).
- The diagnostic value is real (tracing which rules contributed support/attack), but this should be positioned as observability infrastructure, not reasoning infrastructure.
- `ShadowReasoner` is replaced by differential test infrastructure that compares old-engine vs. new-engine outputs.

### ADR-005: Retain ConclusionResolver alongside IndexedResolver

Decision:
- Keep two `EvidenceResolver` implementations rather than forcing query operators to use the same bitset/counter state as the reasoner.

Rationale:
- Query operators run after reasoning is complete. They operate over `Vec<Conclusion>`, not over the reasoner's mutable `ReasoningState`.
- Forcing query operators to reconstruct `ReasoningState` from conclusions would add complexity without benefit.
- The `EvidenceResolver` trait guarantees semantic consistency even with different evidence representations.

### ADR-006: Attacker discovery uses index in all consumers

Decision:
- Both `IndexedResolver` and `ConclusionResolver` use `IndexedTheory.rules_with_head_id()` for attacker discovery instead of brute-force theory iteration.

Rationale:
- The audit found that `why_not` currently iterates all theory rules with `exact_literal_match()`, while the reasoner uses the index. This causes semantic discrepancy (why_not may miss temporal defeaters that the reasoner finds via family-aware index lookup).
- Using the index in both implementations eliminates this discrepancy and improves query operator performance.

---

## 10. Test Specification

### TEST-001: EvidenceResolver trait is the sole evidence-matching interface

Verify:
- The reasoner's defeasible phase calls `EvidenceResolver.find_applicable_attackers()`, not inline attacker loops.
- `why_not` calls `EvidenceResolver.find_applicable_attackers()`, not `theory.rules()` iteration.

Verifies:
- REQ-001

### TEST-002: Abduce and requires use EvidenceResolver for body satisfaction

Verify:
- `abduce` calls `EvidenceResolver.is_supported()` for body literal checks.
- No module-local `is_body_satisfied()` wrapper functions exist.

Verifies:
- REQ-001

### TEST-003: Reasoner and why_not find same attackers

Verify:
- For a given literal and conclusion set, the reasoner's `IndexedResolver` and `why_not`'s `ConclusionResolver` identify the same set of applicable attackers (same rule labels).

Verifies:
- REQ-002

### TEST-004: Temporal defeater found by both consumers

Verify:
- A temporal defeater `~p[1,10]` that blocks base `p` is found by both the reasoner and `why_not` when querying `p`.

Verifies:
- REQ-002

### TEST-005: MatchPolicy determines match mode

Verify:
- `TemporalMatchPolicy.match_mode()` returns `Family` for atemporal literals and `Exact` for temporal literals.
- `StandardMatchPolicy.match_mode()` returns `Exact` for all literals.

Verifies:
- REQ-003

### TEST-006: Family matching accepts temporal variants

Verify:
- Under `TemporalMatchPolicy`, body literal `p` (atemporal) is satisfied by conclusion `p[1,10]`.
- Under `StandardMatchPolicy`, body literal `p` is NOT satisfied by `p[1,10]`.

Verifies:
- REQ-003

### TEST-007: Exact and family identities are distinct but linked

Verify:
- `p[1,10]` and `p[20,30]` produce different exact ids.
- Both map to the same family id.

Verifies:
- REQ-004

### TEST-008: Superiority uses template labels in both resolvers

Verify:
- `IndexedResolver.is_defeated()` and `ConclusionResolver.is_defeated()` both delegate to `theory.is_superior()` with template labels.

Verifies:
- REQ-005

### TEST-009: No synthetic labels in conclusions or explanations

Verify:
- Conclusion rule labels reference authored rules only.
- `why_not` explanations cite authored labels, never bridge or projection labels.

Verifies:
- REQ-006

### TEST-010: Projection engine does not influence conclusions

Verify:
- Removing the projection engine from the pipeline produces identical conclusions.
- `ProjectionToken` types are not imported by any `EvidenceResolver` implementation.

Verifies:
- REQ-007

### TEST-011: Exact temporal query rejects wrong window

Verify:
- `query(p[1,10])` does not match `p[20,30]`.

Verifies:
- REQ-008

### TEST-012: Wildcard query matches family members

Verify:
- Base query `p` matches atemporal `p` and temporal family members `p[1,10]`, `p[20,30]`.
- Output ordering is deterministic.

Verifies:
- REQ-008

### TEST-013: Non-temporal equivalence

Verify:
- Differential testing shows non-temporal theories have identical conclusions before and after redesign.

Verifies:
- REQ-009

### TEST-014: Core reasoner works without temporal extension

Verify:
- The reasoner with `StandardMatchPolicy` produces correct conclusions for non-temporal theories.
- No temporal types are imported by the core reasoning loop.

Verifies:
- REQ-010

### TEST-015: Temporal extension plugs in without modifying core

Verify:
- Swapping `StandardMatchPolicy` for `TemporalMatchPolicy` enables temporal family matching.
- No source changes to the core reasoner are required.

Verifies:
- REQ-010

### TEST-016: Explanation audit trail uses authored labels

Verify:
- Explanations trace back to authored rule labels through the `EvidenceResolver` interface.

Verifies:
- NFR-001

### TEST-017: No inline matching outside EvidenceResolver

Verify:
- `grep` for `exact_literal_match`, `has_positive_match`, and `body_remaining` in modules other than `EvidenceResolver` implementations returns zero hits.

Verifies:
- NFR-002

### TEST-018: Deterministic projected evidence ordering

Verify:
- Repeated runs over same theory produce stable ordering for conclusions and family support.

Verifies:
- NFR-003

### TEST-019: IndexedResolver body satisfaction is O(1)

Verify:
- Body satisfaction in `IndexedResolver` is a counter/bitset check, not a conclusion scan.

Verifies:
- NFR-004

### TEST-PBT-001: Family/exact mapping invariants

Verify:
- Exact literals differing only in temporal share family identity but not exact identity.

### TEST-PBT-002: Non-temporal parity

Verify:
- Redesigned engine and current engine are equivalent on randomly generated non-temporal theories.

---

## 11. Observability

### OBS-001: Evidence resolution audit trace

Emit structured diagnostic counters for:

- Attacker discovery calls per literal
- Support checks per literal
- Superiority evaluations per literal pair
- Match mode distribution (exact vs. family vs. wildcard)

Purpose:
- Validate that evidence resolution is explainable and bounded.

### OBS-002: Determinism guardrail

Emit stable debug snapshots in test mode for:

- Conclusion ordering
- Attacker ordering per literal
- Tie-break label selection

Purpose:
- Catch non-deterministic regressions early.

---

## 12. Migration Plan

### Phase A: Introduce EvidenceResolver trait alongside existing code

- Define `EvidenceResolver` trait and `MatchPolicy` trait.
- Implement `IndexedResolver` that delegates to existing counter/index logic.
- Implement `ConclusionResolver` that delegates to existing conclusion-scanning logic.
- Both implementations wrap existing code — no behavior change.

### Phase B: Migrate consumers to EvidenceResolver

- Refactor `reason/defeasible.rs` to call `IndexedResolver` instead of inline attacker loops.
- Refactor `query/why_not.rs` to call `ConclusionResolver` instead of brute-force theory iteration.
- Refactor `query/abduce.rs` to call `ConclusionResolver` instead of local wrappers.
- Run differential tests to verify no behavior change.

### Phase C: Introduce MatchPolicy extension point

- Extract match mode logic from `query/mod.rs` into `TemporalMatchPolicy`.
- Wire `MatchPolicy` through `EvidenceResolver` implementations.
- Verify core reasoner works with `StandardMatchPolicy` (non-temporal).

### Phase D: Demote projection to observability

- Move `ProjectionEngine` from reasoning-adjacent to diagnostic module.
- Remove `CompiledBody` and `BodyMatchKey` (replaced by `MatchPolicy`).
- Remove `ShadowReasoner` (replaced by differential test infrastructure).
- Retain `ProjectionToken` types for structured logging and audit output.

### Phase E: Resolve known discrepancies

- Fix why_not attacker discovery to use index (via `ConclusionResolver` + `EvidenceResolver`).
- Fix strict vs. defeasible attacker handling in query operators (via shared `is_defeated()`).
- Verify no `exact_literal_match` calls remain outside `EvidenceResolver`.

---

## 13. Risks and Open Questions

### Risk 1: Abstracting the reasoner's hot loop

The reasoner's defeasible phase is performance-critical. Introducing a trait call for attacker discovery may add indirection.

Mitigation:
- `IndexedResolver` wraps existing counter/index logic without adding allocation.
- Trait methods can be monomorphized (generic parameter, not `dyn`).
- Benchmark before and after to verify no regression.

### Risk 2: Two EvidenceResolver implementations may still diverge

Even with a shared trait, `IndexedResolver` and `ConclusionResolver` could compute different results if the trait contract is insufficient.

Mitigation:
- TEST-003 explicitly verifies both implementations find the same attackers.
- Property-based tests generate random theories and compare attacker sets.
- The trait contract includes semantic invariants, not just type signatures.

### Risk 3: MatchPolicy abstraction may be premature

Temporal is currently the only extension. Adding a `MatchPolicy` trait for one implementation may be over-engineered.

Mitigation:
- `MatchPolicy` is a small trait (two methods). The cost of abstraction is low.
- Claire's core + extension design will benefit from this boundary.
- If only one implementation materializes, the trait can be inlined later.

### Open Question 1

Should `EvidenceResolver` be generic over evidence type (`type Evidence`) or use an enum?

Current recommendation:
- Generic with associated type, monomorphized at call sites. This avoids dynamic dispatch overhead in the hot loop.

### Open Question 2

Should `ConclusionResolver` maintain a pre-built index over conclusions (hash map by `LitId`) or continue linear scanning?

Current recommendation:
- Build a conclusion index on construction. Linear scanning is O(n*m) per query; a hash map makes it O(k) and brings query operator performance in line with the reasoner.

### Open Question 3

How does this interact with Claire's core + extension boundary?

Current recommendation:
- `EvidenceResolver` and `MatchPolicy` are the core traits. Temporal types (`FamilyId`, `ExactLitId`, `TemporalMatchPolicy`) are the extension. This aligns with the smaller-core pattern: the core provides the hooks, the extension provides the semantics.

---

## 14. SPEC-020 Comparison

### What stays from SPEC-020

| Artifact | Status | Rationale |
|---|---|---|
| `FamilyId` type | **Retained** | Correct representation of base-family identity |
| `ExactLitId` type | **Retained** | Correct representation of exact temporal identity |
| `family_body_index` in `IndexedTheory` | **Retained** | Needed for family-aware attacker/body lookup |
| `exact_to_family` / `family_to_exact` maps | **Retained** | Core identity mapping infrastructure |
| `ProjectionToken` types | **Retained as observability** | Useful for audit trail, not for reasoning |
| Test suites (regression, PBT, differential) | **Retained** | Valuable coverage |
| Removal of synthetic bridge rules | **Retained** | Correct diagnosis |

### What changes from SPEC-020

| Artifact | SPEC-020 | SPEC-021 | Rationale |
|---|---|---|---|
| `CompiledBody` / `BodyMatchKey` | Reasoning infrastructure | **Removed** | Replaced by `MatchPolicy` |
| `ShadowReasoner` | Runtime verification | **Removed** | Replaced by differential tests |
| `ProjectionEngine` | Reasoning-adjacent | **Observability-only** | Never influenced conclusions |
| Query operator matching | Per-operator logic | **Shared `ConclusionResolver`** | Eliminates discrepancies |
| Attacker discovery | 3 implementations | **2 via `EvidenceResolver`** | Shared trait, shared index |
| Temporal awareness | Hardcoded in core | **`MatchPolicy` extension** | Core becomes temporal-agnostic |

### IMPL-020 work that can be salvaged

- **All `FamilyId` and `ExactLitId` code** (~200 lines in `projection.rs`): extract to a `temporal_identity` module.
- **`family_body_index` additions to `IndexedTheory`** (~100 lines in `index.rs`): keep as-is.
- **All 2,687 lines of test code**: retain, adapt assertions to use `EvidenceResolver` API.
- **`ProjectionToken` types** (~100 lines): move to observability module.

### IMPL-020 work that needs rethinking

- **`CompiledBody` / `BodyMatchKey`** (426 lines in `compilation.rs`): remove entirely, replace with `MatchPolicy`.
- **`ShadowReasoner`** (821 lines in `shadow.rs`): remove, replace with test infrastructure.
- **`ProjectionEngine`** (1,092 lines in `projection.rs`): simplify to observability-only (~200 lines).
- **Query operator matching** (~300 lines across `why_not.rs`, `abduce.rs`): rewrite to use `ConclusionResolver`.

---

## 15. Traceability Matrix

| Requirement | Contracts | Tests | ADRs | Observability |
|---|---|---|---|---|
| REQ-001 | CON-001 | TEST-001, TEST-002 | ADR-001 | OBS-001 |
| REQ-002 | CON-001 | TEST-003, TEST-004 | ADR-001, ADR-006 | OBS-001 |
| REQ-003 | CON-002 | TEST-005, TEST-006 | ADR-002 | — |
| REQ-004 | CON-002 | TEST-007, TEST-PBT-001 | ADR-002 | — |
| REQ-005 | CON-003 | TEST-008 | ADR-001 | — |
| REQ-006 | CON-003 | TEST-009 | ADR-003 | OBS-001 |
| REQ-007 | CON-004 | TEST-010 | ADR-004 | — |
| REQ-008 | CON-002 | TEST-011, TEST-012 | ADR-002 | OBS-002 |
| REQ-009 | — | TEST-013, TEST-PBT-002 | — | — |
| REQ-010 | CON-002 | TEST-014, TEST-015 | ADR-002 | — |
| NFR-001 | — | TEST-016 | ADR-001 | OBS-001 |
| NFR-002 | — | TEST-017 | ADR-001 | — |
| NFR-003 | — | TEST-018 | — | OBS-002 |
| NFR-004 | — | TEST-019 | ADR-005 | — |
