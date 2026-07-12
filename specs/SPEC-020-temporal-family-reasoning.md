| Field | Value |
|---|---|
| Document ID | SPEC-020 |
| Title | First-Class Temporal Family Reasoning Without Synthetic Bridge Rules |
| Version | 1.1.0 |
| Status | Implemented |
| Created | 2026-03-10 |
| Last Updated | 2026-07-04 |
| Authors | GPT-5 Codex (AI agent) |
| Reviewers | Core Maintainers |
| Protocol | [USDD Agent Protocol v1.3.0](../../handbook/engineering/usdd-agent-protocol.md) |
| Traces | SPEC-018, BUG-001, branch `hence/plan-IMPL-018` |

---

# SPEC-020: First-Class Temporal Family Reasoning Without Synthetic Bridge Rules

## 1. Executive Summary

### 1.1 Problem

`SPEC-018` fixed a real correctness bug by making temporal bounds part of atom identity in `crates/spindle-core/src/index.rs`. That part is sound. The remaining instability comes from the compatibility mechanism layered on top: `crates/spindle-core/src/pipeline/bridge.rs` synthesizes extra rules so temporal literals can continue to interact with atemporal rules.

The bridge approach works, but it is structurally fragile because it translates semantic projection into generated rules. Every generated rule must preserve:

- rule strength
- polarity
- original applicability
- template label identity for superiority
- trust and provenance metadata
- deterministic deduplication

The regressions on this branch all came from one of those properties being lost while synthesizing compatibility rules.

### 1.2 Proposed Redesign

This redesign removes synthetic bridge rules entirely. Instead, the engine will represent two distinct but related identities:

1. **Exact temporal literal identity**: `p[1,10]` and `p[20,30]` are different atoms.
2. **Base family identity**: both belong to the same atemporal family `p`.

Reasoning then operates over **first-class support and attack tokens** rather than fake generated rules:

- a temporal fact or rule can emit exact support for `p[t]`
- when policy permits, that same proof can emit projected family support for base `p`
- a temporal defeater can emit a projected family attack using its **original body and original template label**

This preserves semantics without inventing intermediary rules that have to masquerade as user-authored rules.

### 1.3 Outcome

If adopted, this design:

- keeps the `AtomKey` identity fix from `SPEC-018`
- removes `TemporalBridge` as a correctness-critical compatibility layer
- preserves superiority and trust by using original rule labels directly
- makes temporal/base projection explicit in the reasoning core instead of implicit in pipeline rewriting

This is a deeper architectural change than the current branch, but it is conceptually simpler because it moves the semantics to the level where they actually belong.

---

## 2. Problem Statement and Context

### 2.1 What `SPEC-018` fixed correctly

`BUG-001` showed that `AtomKey` ignored temporal bounds, causing:

- cross-window false positives
- body-counter corruption
- query/result conflation across temporal windows

The fix in `crates/spindle-core/src/index.rs` is correct: temporal bounds are semantically significant and must be included in indexed atom identity.

### 2.2 Why the current compatibility layer feels flimsy

Once temporal identity becomes exact, old atemporal rules such as `p => q` no longer automatically match `p[1,10]`. The current branch resolves that by generating compatibility rules in `crates/spindle-core/src/pipeline/bridge.rs`.

This makes the system behave as though a semantic projection were an ordinary rule. That is the core design smell.

The failure modes observed on this branch are all instances of the same mismatch:

- a generated rule used the wrong body
- a generated rule had the wrong strength
- a generated rule used the wrong polarity source
- distinct temporal defeaters were collapsed even though superiority is label-based
- an existing atemporal rule suppressed a needed generated blocker

None of those are indexing bugs. They are artifacts of encoding semantic projection as rule synthesis.

### 2.3 Design diagnosis

The current system has three different notions of sameness:

1. **Exact identity** for interned literals, now including temporal
2. **`Literal::PartialEq` identity** in `crates/spindle-core/src/literal.rs`, which intentionally ignores temporal
3. **Bridge-mediated compatibility**, which tries to recover base-level semantics by adding rules

This split is manageable, but it is hard to reason about because the compatibility semantics are encoded indirectly.

### 2.4 Redesign objective

The system SHALL model temporal-to-base projection as a first-class reasoning concept, not as generated rules.

---

## 3. Scope

| In Scope | Out of Scope |
|---|---|
| Replace synthetic temporal bridge rules with first-class projection semantics | Parser syntax changes for temporal literals |
| Introduce base-family identity alongside exact literal identity | Changes to user-facing SPL syntax |
| Preserve superiority/trust semantics using original labels | New temporal operators beyond existing `(during ...)` |
| Redesign query matching around explicit exact vs family semantics | New modal semantics unrelated to temporal support |
| Differential migration against current `SPEC-018` behavior | CLI contract versioning beyond fields needed to expose semantics |

---

## 4. Architecture Overview

### 4.1 Core model

The redesign introduces a dual-identity model:

- `ExactLitId`
  - Current `LitId` semantics
  - Includes functor, args, mode, negation, and temporal bounds
- `FamilyId`
  - Same functor, args, mode, and polarity
  - Excludes temporal bounds
  - Represents the atemporal/base family

Every exact literal belongs to exactly one family.

Examples:

- `p[1,10]` and `p[20,30]` map to different `ExactLitId` values
- both map to the same positive `FamilyId(p)`
- `~p[1,10]` maps to the negative family `FamilyId(~p)`

### 4.2 Support and attack tokens

Instead of generating synthetic rules, rule application emits tokens:

- `ExactSupport { exact_lit, rule_label, rule_type }`
- `FamilySupport { family_id, source_exact_lit, rule_label, rule_type }`
- `FamilyAttack { family_id, source_exact_lit?, rule_label, rule_type }`

The crucial point is that these tokens keep the **original rule label** and **original rule applicability**.

### 4.3 Rule compilation

Rule bodies are compiled into explicit match keys:

- temporal body literal `p[1,10]`
  - requires exact evidence for that exact temporal literal
- atemporal body literal `p`
  - requires family-level support for family `p`

Rule heads are compiled into emission policies:

- temporal head `q[t]`
  - emits `ExactSupport(q[t])`
  - may emit `FamilySupport(q)` according to projection policy
- atemporal head `q`
  - emits `ExactSupport(q@empty)` and `FamilySupport(q)`
- temporal defeater head `~q[t]`
  - emits `FamilyAttack(~q)` using the original defeater body and original label

### 4.4 Why this is simpler

The redesign makes explicit what the bridge stage currently simulates:

```text
Current branch:
temporal rule/fact
  -> synthesize fake compatibility rule
  -> reason over fake rule
  -> preserve original label/provenance manually

Redesign:
temporal rule/fact
  -> emit exact support
  -> project to family support/attack directly
  -> retain original label/provenance by construction
```

### 4.5 Conceptual diagram

```text
                +----------------------+
                |  Original Rule       |
                |  label = d_temp      |
                |  body = b            |
                |  head = ~q[1,10]     |
                +----------+-----------+
                           |
                    applicability check
                           |
                           v
                +----------------------+
                |  FamilyAttack(~q)    |
                |  source = d_temp     |
                |  body source = b     |
                +----------+-----------+
                           |
                           v
                +----------------------+
                | SDL defeat / trust / |
                | superiority engine   |
                +----------------------+
```

No synthetic `b ~> ~q` bridge rule is needed.

---

## 5. Purity Boundary Map

### Pure Core

- `index` family/exact interning
- rule-body match-key compilation
- projection policy evaluation
- support/attack token emission
- superiority resolution over original rule labels
- query match semantics (`Exact`, `Family`, `Wildcard`)

### Effectful Shell

- SPL parsing and source loading
- pipeline orchestration
- CLI/WASM presentation of conclusions
- diagnostics and metrics emission
- trust metadata retrieval from theory metadata

### Boundary rule

The pure core SHALL never synthesize user-visible rules as an implementation detail. Projection is a semantic relation, not a source-to-source rewrite.

---

## 6. Functional Requirements

### REQ-001: Dual literal identity

The system SHALL maintain both exact temporal literal identity and base family identity for every logical literal.

Acceptance criteria:

- exact identity includes temporal bounds
- family identity excludes temporal bounds
- each exact literal maps deterministically to one family

Trace:
- CON-001
- TEST-001
- TEST-PBT-001

### REQ-002: Atemporal bodies depend on family support, not synthetic bridge rules

The system SHALL satisfy an atemporal body literal `p` using family-level support for `p`, without generating compatibility rules in the theory.

Acceptance criteria:

- `p[1,10]` can satisfy body `p` when projection policy allows
- no synthetic bridge rule labels are required in prepared theories

Trace:
- CON-001
- TEST-002
- TEST-006

### REQ-003: Temporal defeaters preserve original applicability

The system SHALL allow a temporal defeater head such as `~q[t]` to attack base family `q` using the defeater’s original body and original template label.

Acceptance criteria:

- a temporal defeater does not depend on proving `~q[t]`
- blocking behavior is determined by original defeater applicability

Trace:
- CON-002
- TEST-003
- TEST-004

### REQ-004: Superiority SHALL remain label-based over original rules

The system SHALL evaluate superiority using original rule/template labels only. Projection SHALL NOT invent substitute labels that participate in superiority.

Acceptance criteria:

- if `r_q > d_base` but not `d_temp`, then `d_temp` remains an undefeated attacker
- multiple temporal defeaters with identical structural shape but different labels remain distinct attackers

Trace:
- CON-002
- TEST-004
- TEST-005

### REQ-005: Trust and provenance SHALL remain source-rule derived

Projected support and attack SHALL inherit trust and provenance from the original rule metadata, not from generated compatibility artifacts.

Acceptance criteria:

- weighted conclusions for projected base support resolve source metadata through the original template label
- trust order is unaffected by internal projection bookkeeping

Trace:
- CON-003
- TEST-007

### REQ-006: Query semantics SHALL expose exact, family, and wildcard intent explicitly

The system SHALL provide explicit query matching semantics:

- `Exact`: exact temporal match required
- `Family`: any support in the same family
- `Wildcard`: user-facing empty-temporal query matches base and temporal family members

Acceptance criteria:

- bounded temporal queries do not cross-match wrong windows
- family queries do not require synthetic base conclusions

Trace:
- CON-004
- TEST-008
- TEST-009

### REQ-007: Non-temporal theories remain semantically unchanged

The redesign SHALL preserve all conclusions for non-temporal theories.

Acceptance criteria:

- differential tests against the current engine match on non-temporal inputs
- no new support or attack paths appear in non-temporal theories

Trace:
- TEST-010
- TEST-PBT-002

### REQ-008: Prepared theories remain close to authored theories

The system SHALL avoid inserting synthetic temporal compatibility rules into the prepared theory as a normal execution mechanism.

Acceptance criteria:

- prepared theories contain authored rules, grounded instances, and validation metadata only
- any projection state is internal runtime state, not user-facing generated rules

Trace:
- CON-001
- TEST-011

### REQ-009: Conflict and attack are window-identity based; overlap is not conflict

Temporal windows are opaque identity, not intervals with point semantics.
Two literals conflict (complement, attack, ambiguity-block) only when their
windows are *identical*. Overlapping-but-distinct windows are independent
assertions: `p[1,10]` and `~p[5,15]` do NOT attack each other and may both
be defeasibly concluded from the same theory, while `p[1,10]` and `~p[1,10]`
ambiguity-block as usual.

This is a deliberate semantic decision, aligned with the exact-identity
design of REQ-001 and mirrored by the verified Lean model (`AtomKey`
includes `temporal`; `LitId::complement()` flips negation only). Authors who
intend point-in-time conflict semantics must normalize windows upstream
(e.g. author identical windows, or split assertions at window boundaries
before submission). Interval-overlap conflict detection, if ever wanted, is
a separate future specification — it requires matching changes to the Lean
reference model and the difftest generators.

Acceptance criteria:

- identical-window complements ambiguity-block (neither `+d`)
- overlapping distinct-window complements both conclude `+d`
- the behavior is pinned by a regression test so any future change to it is
  a deliberate spec revision, not an accident

Trace:
- CON-001
- TEST-015

---

## 7. Non-Functional Requirements

### NFR-001: Semantic auditability

Reasoning state SHALL remain auditable by tracing support and attack back to authored rule labels without bridge indirection.

Trace:
- OBS-001
- TEST-012

### NFR-002: Complexity locality

Temporal/base compatibility logic SHALL be localized to indexing, projection, and query matching components. It SHALL NOT require widespread synthetic-rule special cases across the pipeline.

Trace:
- TEST-013

### NFR-003: Deterministic output

Family support selection, trust tie-breaking, and projected conclusion ordering SHALL be deterministic.

Trace:
- TEST-014
- OBS-002

---

## 8. Contracts

### CON-001: Exact/family indexing contract

Module/Interface:
- `IndexedTheory`

Required capabilities:

- `exact_lit_id(&Literal) -> Option<ExactLitId>`
- `family_id(&Literal) -> FamilyId`
- `family_members(FamilyId) -> &[ExactLitId]`
- `family_for_exact(ExactLitId) -> FamilyId`

Implements:
- REQ-001
- REQ-002
- REQ-008

Verified by:
- TEST-001
- TEST-002
- TEST-011

### CON-002: Projection token contract

Module/Interface:
- `ProjectionEngine`

Pre-conditions:

- rule applicability has been evaluated against body match keys
- original rule label and type are available

Post-conditions:

- emits support/attack tokens using original labels
- does not materialize synthetic rules in `Theory`

Implements:
- REQ-003
- REQ-004

Verified by:
- TEST-003
- TEST-004
- TEST-005

### CON-003: Provenance/trust contract

Module/Interface:
- trust resolution over projected support

Post-conditions:

- trust lookup resolves through original template label
- projected evidence does not introduce substitute provenance identities

Implements:
- REQ-005

Verified by:
- TEST-007

### CON-004: Query match contract

Module/Interface:
- query matcher API

Modes:

- `ExactTemporal`
- `Family`
- `WildcardTemporal`

Implements:
- REQ-006

Verified by:
- TEST-008
- TEST-009

---

## 9. Architecture Decisions

### ADR-001: First-class family identity vs. synthetic bridge rules

Decision:
- Introduce `FamilyId` and runtime projection tokens instead of generating compatibility rules in the pipeline.

Rationale:
- superiority and trust are rule-label-based, not structural-rule-based
- temporal defeaters must preserve original applicability
- generated rules are a lossy encoding of the required semantics

Rejected alternative:
- keep `TemporalBridge` and continue hardening deduplication/provenance logic

Rejection reason:
- correctness depends on reproducing authored-rule semantics in generated artifacts
- every new metadata-sensitive subsystem reopens the same class of bug

### ADR-002: Preserve `Literal::PartialEq` compatibility, but forbid it as semantic identity

Decision:
- keep `Literal::PartialEq` unchanged for backward compatibility
- require exact/family-aware APIs in indexing, reasoning, and query layers

Rationale:
- changing equality globally would have broad blast radius
- the real problem is misuse of `==` in semantic code, not the existence of ergonomic equality itself

### ADR-003: Projection is emitted from rule application, not inferred from conclusion scanning

Decision:
- when a rule becomes applicable, its projection effects are emitted immediately as runtime tokens

Rationale:
- projection depends on rule strength, label, body applicability, and polarity
- post-hoc inference from conclusions discards too much information

### ADR-004: Family attacks and supports use original labels directly

Decision:
- projected family evidence carries the originating rule label unchanged

Rationale:
- superiority in `crates/spindle-core/src/reason/defeasible.rs` is label-based
- trust resolution in `crates/spindle-core/src/pipeline/mod.rs` is label-based

### ADR-005: Migration uses differential shadow execution before cutover

Decision:
- implement redesign behind a gated path and compare outputs against the current branch across fixtures, integration tests, and property tests before removing `TemporalBridge`

Rationale:
- reduces semantic regression risk
- gives maintainers concrete parity evidence instead of architectural hope

---

## 10. Test Specification

### TEST-001: Exact and family identities are distinct but linked

Verify:
- `p[1,10]` and `p[20,30]` produce different exact ids
- both map to the same family id

Verifies:
- REQ-001

### TEST-002: Atemporal body is satisfied via family support

Verify:
- `>> p[1,10]` plus `p => q` derives `q` without any synthetic bridge rules in prepared theory

Verifies:
- REQ-002
- REQ-008

### TEST-003: Temporal defeater blocks base literal via original body

Verify:
- `a => q` and `b ~> ~q[1,10]` block `q` when `b` is true
- the attack does not depend on proving `~q[1,10]`

Verifies:
- REQ-003

### TEST-004: Distinct temporal defeaters remain distinct attackers

Verify:
- two temporal defeaters with same body/head shape but different labels remain separate attackers for superiority purposes

Verifies:
- REQ-004

### TEST-005: Existing atemporal defeater does not suppress temporal attacker

Verify:
- a temporal defeater projection remains active even when a structurally matching atemporal defeater exists with a different label

Verifies:
- REQ-004

### TEST-006: Prepared theory does not contain synthetic temporal bridge rules

Verify:
- preparation/grounding does not inject `__bridge::*` rules under redesign mode

Verifies:
- REQ-002
- REQ-008

### TEST-007: Projected support preserves trust provenance

Verify:
- trust degree and source attribution for base-family support resolve through the original source rule

Verifies:
- REQ-005

### TEST-008: Exact temporal query rejects wrong window

Verify:
- `query(p[1,10])` does not match `p[20,30]`

Verifies:
- REQ-006

### TEST-009: Wildcard/family query matches family members deterministically

Verify:
- base query `p` matches atemporal `p` and temporal family members
- output ordering is deterministic

Verifies:
- REQ-006
- NFR-003

### TEST-010: Non-temporal equivalence

Verify:
- differential testing shows non-temporal theories have identical conclusions before and after redesign

Verifies:
- REQ-007

### TEST-011: Authored-theory preservation

Verify:
- prepared theory remains close to authored/grounded theory and exposes no synthetic compatibility rules

Verifies:
- REQ-008

### TEST-012: Explanation/why-not audit trail uses authored labels

Verify:
- explanations cite original rule labels, never bridge labels

Verifies:
- NFR-001

### TEST-013: Complexity locality guard

Verify:
- projection logic is confined to new family/projection modules; pipeline stages do not carry temporal-bridge-specific semantics

Verifies:
- NFR-002

### TEST-014: Deterministic projected evidence ordering

Verify:
- repeated runs over same theory produce stable ordering for projected conclusions and trust tie-breaks

Verifies:
- NFR-003

### TEST-015: Window-identity conflict semantics

Verify:
- identical-window complements (`p[1,10]` vs `~p[1,10]`) ambiguity-block: neither is `+d`
- overlapping distinct-window complements (`p[1,10]` vs `~p[5,15]`) are independent: both are `+d`

Implemented as `test_overlapping_windows_are_independent_identical_windows_conflict`
in `crates/spindle-core/tests/regression_known_bugs.rs`.

Verifies:
- REQ-009

### TEST-PBT-001: Family/exact mapping invariants

Verify:
- exact literals differing only in temporal share family identity but not exact identity

### TEST-PBT-002: Non-temporal parity

Verify:
- redesigned engine and current engine are equivalent on randomly generated non-temporal theories

---

## 11. Observability

### OBS-001: Projection audit trace

Emit structured diagnostic counters for:

- exact supports emitted
- family supports emitted
- family attacks emitted
- rule labels contributing projected evidence

Purpose:
- validate that projection activity is explainable and bounded

### OBS-002: Determinism guardrail

Emit stable debug snapshots in test mode for:

- projected evidence ordering
- tie-break label selection

Purpose:
- catch non-deterministic regressions early

---

## 12. Migration Plan

### Phase A: Introduce family identity alongside existing bridge path

- add `FamilyId` to indexing
- add internal projection/token types
- keep current bridge path as control implementation

### Phase B: Shadow execution and differential verification

- run bridge-based and redesign-based reasoning in tests
- compare conclusions, explanation labels, and trust outputs

### Phase C: Cut query operators to family-aware interfaces

- switch `query`, `why_not`, and `abduce` to the new match modes
- retain wildcard semantics intentionally, not accidentally

### Phase D: Remove `TemporalBridge`

- once parity is established for intended semantics, remove bridge stage from default pipeline
- retain focused regression tests that motivated the redesign

---

## 13. Risks and Open Questions

### Risk 1: Larger reasoning-core change

This redesign moves complexity from the pipeline into the reasoning/indexing core. That is the right architectural location, but it is a broader change than another bridge patch.

Mitigation:
- shadow-mode differential testing before cutover

### Risk 2: Query API semantics become more explicit

Callers may currently rely on accidental wildcard behavior.

Mitigation:
- preserve current user-facing wildcard behavior, but model it explicitly as `WildcardTemporal`

### Open Question 1

Should family support be represented as a separate conclusion kind internally, or as side-channel evidence attached to existing conclusions?

Current recommendation:
- separate internal evidence kind, not a user-visible conclusion type

### Open Question 2

Should atemporal authored facts `>> p.` be represented as both an exact empty-temporal literal and a family support token, or should family support be derived lazily from exact empty-temporal support?

Current recommendation:
- authored atemporal facts emit both, for simpler body matching

---

## 14. Recommendation

Adopt this redesign if the team wants the temporal model to be maintainable long-term.

Do not adopt it merely to avoid the current branch. The current branch fixes a real correctness problem and may still be an acceptable short-term merge if the team wants the smallest semantic delta. But if maintainers are already uneasy because the bridge approach feels flimsy, that intuition is directionally correct: the remaining instability comes from representing semantic projection as generated rules.

This redesign gives the system a cleaner invariant:

- exact temporal identity stays exact
- base-family compatibility becomes explicit
- superiority and trust remain attached to authored rules
- no synthetic bridge rules are required to recover semantics

---

## 15. Traceability Matrix

| Requirement | Contracts | Tests | ADRs | Observability |
|---|---|---|---|---|
| REQ-001 | CON-001 | TEST-001, TEST-PBT-001 | ADR-001 | — |
| REQ-002 | CON-001 | TEST-002, TEST-006 | ADR-001, ADR-003 | OBS-001 |
| REQ-003 | CON-002 | TEST-003 | ADR-001, ADR-003 | OBS-001 |
| REQ-004 | CON-002 | TEST-004, TEST-005 | ADR-004 | OBS-001 |
| REQ-005 | CON-003 | TEST-007 | ADR-004 | OBS-001 |
| REQ-006 | CON-004 | TEST-008, TEST-009 | ADR-002 | OBS-002 |
| REQ-007 | — | TEST-010, TEST-PBT-002 | ADR-005 | — |
| REQ-008 | CON-001 | TEST-006, TEST-011 | ADR-001 | OBS-001 |
| NFR-001 | — | TEST-012 | ADR-004 | OBS-001 |
| NFR-002 | — | TEST-013 | ADR-001 | — |
| NFR-003 | — | TEST-014 | ADR-003 | OBS-002 |
