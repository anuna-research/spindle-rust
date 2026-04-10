| Field | Value |
|---|---|
| Document ID | SPEC-023 |
| Title | Diminishment Layer for Trust-Weighted Defeasible Reasoning |
| Version | 1.0.0 |
| Status | Draft |
| Created | 2026-04-10 |
| Last Updated | 2026-04-10 |
| Authors | Codex (AI agent) |
| Reviewers | Hugo O'Connor, Core Maintainers |
| Protocol | [USDD Agent Protocol v1.3.0](../../handbook/engineering/usdd-agent-protocol.md) |
| Traces | PAPER-001, `plans/trust-parity.spl`, `plans/paper-trust-module.spl`, `docs/paper/comma2026-rashomon-engine.tex` |

---

# SPEC-023: Diminishment Layer for Trust-Weighted Defeasible Reasoning

## 1. Executive Summary

### 1.1 Problem

Spindle-Rust currently implements a decoupled trust-weighting layer that computes a trust-weighted degree for each conclusion by traversing the winning derivation chain and taking the weakest source in that chain. This supports source attribution, temporal decay, thresholds, and multi-perspective evaluation. It does **not** currently account for the epistemic effect of losing objections.

That omission creates a semantic gap between the intended trust model and the current system behaviour:

1. Two conclusions with identical winning support receive the same score even if one faced a highly credible defeated objection and the other faced no objection at all.
2. `WeightedConclusion` and `DiminisherInfo` already reserve space for partial-defeat metadata, but the main trust pipeline never populates that metadata and never reduces the final degree.
3. The paper framing around a second epistemic layer is therefore only partially reflected in code: weakest-link support is implemented, but residual doubt from credible defeated defeaters is not.

The resulting system answers only one question:

> How trustworthy is the winning support chain?

It does not answer the richer question:

> How trustworthy is the winning support chain after accounting for serious losing objections?

### 1.2 Proposed Change

This spec introduces a **post-reasoning diminishment layer** for defeated defeaters. The layer remains decoupled from DL(d) conclusion derivation. It never changes whether a conclusion is logically derivable. Instead, it lowers the trust-weighted degree of a positive defeasible conclusion when there exist applicable defeater rules against that conclusion whose bodies are supported but which lose in Layer 1.

The core semantics are:

- Compute the current weakest-link support degree, `base_degree`, exactly as today.
- Identify applicable defeater rules that attack the conclusion and are defeated in the logical layer.
- Compute a trust degree for each such defeater from its own derivation chain.
- Aggregate their effect multiplicatively:

```text
final_degree = base_degree × ∏ (1 - defeater_degree_i)
```

This leaves the logical conclusion unchanged while making the trust layer sensitive to credible objections that were overruled procedurally.

### 1.3 Outcome

If adopted, this design:

- Preserves the existing DL(d) proof theory and conclusion set
- Retains the current weakest-link support computation as the base score
- Adds explicit, auditable metadata explaining how defeated defeaters weakened a conclusion
- Distinguishes "clean wins" from "contested wins"
- Aligns the implementation with the paper's intended two-layer story without coupling trust into the reasoner

### 1.4 Non-Goals

This spec does **not**:

- Change the logical reasoner's conclusion set
- Add trust thresholds to rule firing or proof search
- Generalize diminishment to all opposing strict/defeasible rules
- Learn trust automatically from outcomes
- Introduce cryptographic verification of claim signatures

---

## 2. Current State and Gap

### 2.1 What is implemented today

The current trust pipeline in `crates/spindle-core/src/pipeline/mod.rs`:

1. Builds a support tree for each positive conclusion
2. Computes a weakest-link trust degree over the winning derivation chain
3. Collects support-side sources
4. Applies temporal decay during source-trust lookup
5. Evaluates named thresholds against the computed degree

This yields a useful and coherent notion of **support quality**:

> A conclusion is only as trustworthy as the weakest source in the derivation that supports it.

### 2.2 What is missing

The pipeline does not currently inspect losing objections. In particular:

- `WeightedConclusion.degree` is always the support-side weakest-link degree
- `WeightedConclusion.diminished_by` is always empty in the production pipeline
- `DiminisherInfo` is tested in isolation but not integrated into the end-to-end weighting path

The system therefore does not distinguish between:

- a conclusion that won unopposed, and
- a conclusion that won despite a serious, credible objection

### 2.3 Why this matters

Many trust-sensitive domains care about the difference between those two cases:

- In law, a verdict may stand while still being weakened by credible counter-testimony.
- In compliance, a conclusion may be actionable for ordinary automation but require human review if it survived serious objection.
- In multi-agent coordination, a plan may be logically selected while still being epistemically fragile due to credible dissent.

The current system models only the strength of winning support. This spec adds a bounded notion of residual doubt from defeated defeaters.

---

## 3. Scope

| In Scope | Out of Scope |
|---|---|
| Post-reasoning diminishment for defeated defeater rules | Trust-aware rule firing in the reasoner |
| Computing defeater trust from the defeater's own support chain | Changing DL(d) proof conditions |
| Extending `WeightedConclusion` with base-vs-final trust transparency | Generalized gradual semantics for all attack types |
| Populating `diminished_by` metadata in the production pipeline | Learning trust from empirical history |
| Threshold evaluation against diminished degree | Signature verification and PKI |
| End-to-end tests for contested-but-provable conclusions | UI-specific presentation work beyond existing machine-readable outputs |

---

## 4. Architecture Overview

### 4.1 Design principle

The architecture remains strictly two-layered:

```text
Shared theory
    ↓
DL(d) reasoner
    ↓
Positive / negative conclusions
    ↓
Base trust weighting (current weakest-link support traversal)
    ↓
Diminishment pass over defeated defeaters
    ↓
Final weighted conclusions
```

Layer 1 determines whether a conclusion exists. Layer 2 annotates that conclusion with confidence information. Diminishment belongs entirely to Layer 2.

### 4.2 Support-side base degree

`base_degree` is the current trust score already implemented:

```text
base_degree(c) = min trust over all support-side nodes in the selected derivation for c
```

This value SHALL continue to be computed exactly as the current pipeline computes `WeightedConclusion.degree`, except that it SHALL be preserved as a separately accessible field.

### 4.3 Defeater-side objection degree

For each positive defeasible conclusion `c`, the diminishment layer SHALL inspect defeater rules `d` such that:

1. `d` is a defeater rule
2. the head of `d` attacks `c`
3. the body of `d` is supported in the current theory
4. `c` still survives logically as a positive defeasible conclusion

Each such `d` is a **defeated defeater** for purposes of the trust layer.

Its objection degree is computed by building a trust tree rooted at the defeater rule itself and traversing the defeater's supporting premises:

```text
defeater_degree(d) = min trust over defeater rule source and all supporting premises in d's derivation
```

Unlike ordinary positive conclusions, defeaters do not require a separate positive conclusion object. Their support tree is constructed directly from the defeater rule and the positive support available for each body literal.

### 4.4 Diminishment aggregation

The final degree of a positive defeasible conclusion is:

```text
final_degree(c) = base_degree(c) × ∏ (1 - defeater_degree(d_i))
```

where `d_i` ranges over all applicable defeated defeaters against `c`.

Properties:

- If there are no defeated defeaters, `final_degree = base_degree`
- A zero-trust defeater has no effect
- A unit-trust defeater reduces the final degree to zero
- Multiple defeated defeaters compose multiplicatively and monotonically

### 4.5 Why multiplicative aggregation

The multiplicative form preserves several desired properties for a post-reasoning trust layer:

- boundedness: `0 ≤ final_degree ≤ base_degree`
- monotonicity: more credible objections never increase confidence
- compositionality: each objection contributes a bounded proportional discount
- decoupling: the logical conclusion set remains invariant

This is a post-reasoning weakening operator, not a substitute for the reasoner's defeat semantics.

---

## 5. Purity Boundary Map

### Pure Core (no I/O, deterministic)

- `compute_base_trust_tree`: current weakest-link support traversal over selected derivation
- `compute_defeater_trust_tree`: defeater-side traversal over rule metadata and supported body literals
- `collect_applicable_defeated_defeaters`: identify body-supported defeater rules attacking a positive defeasible conclusion
- `aggregate_diminishment`: compute multiplicative reduction over defeated defeater degrees
- `build_diminisher_info`: produce stable audit records for each applied diminisher

### Effectful Shell (allocation, orchestration, serialization)

- `compute_weighted_conclusions`: orchestrates reasoner output, trust policy, and the pure diminishment helpers
- CLI / JSON / query surfaces that expose weighted conclusions
- Metrics/log emission for observability

### Boundary Rule

The mathematical semantics of diminishment SHALL live in pure helper functions. The pipeline SHALL orchestrate those functions but SHALL NOT embed the reduction formula ad hoc across multiple call sites.

---

## 6. Requirements

### REQ-001: Base Degree Preservation

The system SHALL compute a `base_degree` for every positive conclusion using the existing weakest-link support traversal over the selected derivation chain, before any diminishment is applied.

Trace:
- CON-001
- TEST-001
- TEST-002

### REQ-002: Diminishment Applies Only to Positive Defeasible Conclusions

The system SHALL apply diminishment only to conclusions of type `DefeasiblyProvable`.

`DefinitelyProvable`, `DefinitelyNotProvable`, and `DefeasiblyNotProvable` conclusions SHALL NOT be diminished.

Trace:
- CON-001
- TEST-003

### REQ-003: Applicable Defeated Defeater Discovery

For each `DefeasiblyProvable` conclusion, the system SHALL identify defeater rules whose head attacks the conclusion, whose body is positively supported, and which are defeated in the logical layer by the fact that the conclusion remains positively derivable.

Trace:
- CON-002
- TEST-004
- TEST-005

### REQ-004: Defeater Trust Computation

The system SHALL compute a `defeater_degree` for each applicable defeated defeater by taking the weakest-link trust across:

- the defeater rule's own source metadata, and
- the support chains of each positively supported body literal in the defeater body.

Trace:
- CON-002
- TEST-006

### REQ-005: Final Degree Formula

The system SHALL compute the final diminished degree of a positive defeasible conclusion as:

```text
final_degree = base_degree × ∏ (1 - defeater_degree_i)
```

where the product ranges over all applicable defeated defeaters.

Trace:
- CON-003
- TEST-007
- TEST-008

### REQ-006: Threshold Evaluation Uses Final Degree

Named thresholds SHALL be evaluated against the diminished `final_degree`, not the undiminished `base_degree`.

Trace:
- CON-001
- TEST-009

### REQ-007: Diminishment Audit Metadata

For each applied defeated defeater, the system SHALL record a `DiminisherInfo` entry containing enough information to explain:

- which defeater reduced the score
- the defeater's trust degree
- the target conclusion's pre-reduction degree at the time of application
- the resulting reduction amount
- whether the defeater produced a limit-case full defeat

Trace:
- CON-001
- TEST-010
- OBS-001

### REQ-008: Stable Ordering

The system SHALL emit `diminished_by` entries in deterministic order, sorted by defeater rule label after grounding/template-label normalization.

Trace:
- NFR-002
- TEST-011

### REQ-009: No Logical Side Effects

The diminishment layer SHALL NOT change:

- the set of conclusions returned by the reasoner,
- the conclusion type of any conclusion, or
- whether the system records a conclusion as logically positive or negative.

Trace:
- ADR-001
- TEST-012

### REQ-010: Backward-Compatible Zero-Diminisher Case

When no applicable defeated defeaters exist for a positive defeasible conclusion, the system SHALL produce:

- `degree == base_degree`
- `diminished_by == []`

Trace:
- CON-001
- TEST-013

---

## 7. Non-Functional Requirements

### NFR-001: Diminishment Pass Complexity

The additional diminishment pass SHALL run in time linear in:

- the size of the selected support tree for the target conclusion,
- plus the number of candidate defeater rules attacking that conclusion,
- plus the total size of the support trees needed to score applicable defeated defeaters.

The implementation SHALL avoid re-running the full reasoner for each trust policy.

Trace:
- TEST-014
- OBS-002

### NFR-002: Deterministic Output

Diminishment metadata and final degrees SHALL be deterministic for the same theory, trust policy, and reference time.

Trace:
- TEST-011
- TEST-015

### NFR-003: Existing Trust Semantics Remain Intact

The addition of diminishment SHALL NOT break existing end-to-end trust integration behaviour for:

- single-source facts,
- weakest-link support chains,
- temporal decay,
- thresholds in uncontested cases,
- grounded rule trust propagation.

Trace:
- TEST-016

---

## 8. Architecture Decisions

### ADR-001: Diminishment Remains Post-Reasoning

**Decision:** Diminishment is computed only after the DL(d) reasoner has produced its conclusion set.

**Rationale:** This preserves the current architectural separation and the invariance of logical conclusions under different trust policies. The reasoner remains trust-unaware.

**Consequences:**

- Strong preservation of existing proof theory
- Easier comparison across multiple trust policies
- Diminishment cannot block rule firing or alter proof search

### ADR-002: Only Defeater Rules Participate in Initial Diminishment

**Decision:** The first implementation scope is limited to `RuleType::Defeater`.

**Rationale:** This matches the current paper framing, avoids reinterpreting all contrary rules as epistemic diminishers, and keeps the implementation aligned with explicit "objection" rules rather than generic contradiction.

**Consequences:**

- Clear and narrow semantics
- Future work may generalize to contrary defeasible or strict attacks if needed

### ADR-003: Preserve Both Base and Final Degree

**Decision:** `WeightedConclusion` SHALL preserve both the support-side `base_degree` and the post-diminishment final `degree`.

**Rationale:** Without both values, the system cannot explain how much confidence came from support and how much was removed by objection. Auditability requires both.

**Consequences:**

- Slight schema expansion
- Better CLI/JSON/explanation support
- Clearer regression testing

### ADR-004: Multiplicative Composition

**Decision:** Multiple defeated defeaters are aggregated multiplicatively rather than additively or by minimum.

**Rationale:** Multiplicative composition keeps the output bounded, monotone, and sensitive to multiple moderate objections without requiring arbitrary clipping rules beyond `[0,1]`.

**Consequences:**

- Natural zero and unit edge cases
- Straightforward pure implementation
- Order-independent aggregation

---

## 9. Contracts

### CON-001: Weighted Conclusion Contract

`WeightedConclusion` SHALL be extended so that:

- `degree` is the final post-diminishment degree
- `base_degree` is the pre-diminishment support-side weakest-link degree
- `diminished_by` contains ordered `DiminisherInfo` entries for each applied defeated defeater
- `above_threshold` is computed from `degree`

Implements:
- REQ-001
- REQ-002
- REQ-006
- REQ-007
- REQ-010

Verified by:
- TEST-001
- TEST-003
- TEST-009
- TEST-010
- TEST-013

### CON-002: Defeated Defeater Collection Contract

The trust pipeline SHALL expose a pure helper that, given:

- a positive defeasible conclusion,
- the theory,
- the indexed positive support map,
- the trust policy, and
- the reference time,

returns all applicable defeated defeaters with their computed defeater degrees and audit metadata.

The helper SHALL:

- inspect only defeater rules attacking the target literal,
- require all defeater body literals to have positive support,
- compute defeater trust via weakest-link traversal rooted at the defeater rule,
- return deterministic ordering.

Implements:
- REQ-003
- REQ-004
- REQ-008

Verified by:
- TEST-004
- TEST-005
- TEST-006
- TEST-011

### CON-003: Diminishment Aggregation Contract

A pure aggregation helper SHALL accept:

- `base_degree: f64`
- an ordered list of defeated defeater degrees

and SHALL return:

```text
base_degree × ∏ (1 - defeater_degree_i)
```

The helper SHALL clamp the result to `[0.0, base_degree]`.

Implements:
- REQ-005
- REQ-006

Verified by:
- TEST-007
- TEST-008
- TEST-009

---

## 10. Test Specifications

### TEST-001: Base Degree Is Preserved Separately

Given a positive defeasible conclusion with no attacking defeaters, the system SHALL expose:

- `base_degree == degree`
- empty `diminished_by`

Verifies:
- REQ-001
- REQ-010

### TEST-002: Existing Weakest-Link Behaviour Remains the Base Degree

Given a multi-step support chain with trust values `0.9 -> 0.4 -> 0.8`, the system SHALL set `base_degree == 0.4` before any diminishment.

Verifies:
- REQ-001

### TEST-003: Definite Conclusions Are Not Diminished

Given a `DefinitelyProvable` conclusion with attacking defeater rules elsewhere in the theory, the system SHALL:

- leave `degree == base_degree`
- leave `diminished_by` empty

Verifies:
- REQ-002

### TEST-004: Supported Losing Defeater Is Collected

Given a positive defeasible conclusion and a defeater rule with attacking head and positively supported body, the system SHALL record that defeater in `diminished_by`.

Verifies:
- REQ-003

### TEST-005: Unsupported Defeater Is Ignored

Given an attacking defeater whose body is not positively supported, the system SHALL NOT include it in `diminished_by`.

Verifies:
- REQ-003

### TEST-006: Defeater Degree Uses Weakest-Link over Its Own Chain

Given a defeater sourced by `0.8` but supported by a premise sourced by `0.5`, the defeater degree SHALL be `0.5`.

Verifies:
- REQ-004

### TEST-007: Single Defeater Diminishment Formula

Given `base_degree = 0.9` and one defeated defeater with degree `0.3`, the final degree SHALL be `0.63`.

Verifies:
- REQ-005

### TEST-008: Multiple Defeaters Compose Multiplicatively

Given `base_degree = 0.9` and defeated defeaters with degrees `0.3` and `0.4`, the final degree SHALL be:

```text
0.9 × 0.7 × 0.6 = 0.378
```

Verifies:
- REQ-005

### TEST-009: Threshold Uses Final Diminished Degree

Given `base_degree = 0.9`, one defeated defeater with degree `0.3`, and threshold `0.7`, the system SHALL mark the conclusion below threshold because `0.63 < 0.7`.

Verifies:
- REQ-006

### TEST-010: Diminisher Audit Metadata Is Populated

For an applied defeater, the system SHALL populate `DiminisherInfo` with the defeater label, defeater degree, target degree, reduction amount, and full-defeat flag.

Verifies:
- REQ-007

### TEST-011: Deterministic Defeater Ordering

Given multiple defeated defeaters discovered in arbitrary rule iteration order, the emitted `diminished_by` list SHALL be sorted deterministically by normalized rule label.

Verifies:
- REQ-008
- NFR-002

### TEST-012: Logical Conclusions Are Unchanged

For a theory containing applicable defeated defeaters, the set of logical conclusions returned by the reasoner before and after diminishment integration SHALL be identical.

Verifies:
- REQ-009

### TEST-013: Zero-Diminisher Case Remains Backward Compatible

Given a positive defeasible conclusion with no applicable defeated defeaters, the final weighted output SHALL be identical to the current implementation.

Verifies:
- REQ-010

### TEST-014: Diminishment Pass Avoids Full Re-Reasoning

Benchmark or instrumentation SHALL verify that diminishment reuses existing reasoner output and does not invoke a fresh full reasoning pass per defeater.

Verifies:
- NFR-001

### TEST-015: Stable Output under Repeated Evaluation

Repeated evaluation of the same theory and trust policy SHALL produce identical final degrees and identical `diminished_by` ordering.

Verifies:
- NFR-002

### TEST-016: Existing Trust Integration Suite Still Passes

All current trust integration scenarios that do not involve defeated defeaters SHALL continue to pass unchanged.

Verifies:
- NFR-003

---

## 11. Observability

### OBS-001: Diminished Conclusions Counter

The system SHOULD expose a counter or debug diagnostic indicating how many positive defeasible conclusions had at least one applied diminisher in a run.

Supports:
- REQ-007

### OBS-002: Average Diminishment Ratio

The system SHOULD expose a diagnostic summary of the average ratio:

```text
degree / base_degree
```

across diminished conclusions, to help characterize how strongly objections are affecting outputs in practice.

Supports:
- NFR-001

### OBS-003: Per-Conclusion Diminishment Trace

When machine-readable output is requested, the system SHOULD emit both `base_degree` and `degree` so downstream tooling can detect contested-but-accepted conclusions.

Supports:
- ADR-003

---

## 12. Implementation Notes

### 12.1 Expected file touch points

Primary:

- `crates/spindle-core/src/pipeline/mod.rs`
- `crates/spindle-core/src/trust.rs`
- `crates/spindle-core/tests/trust_integration_tests.rs`

Possible secondary:

- `crates/spindle-core/src/query/mod.rs`
- `crates/spindle-cli/src/output.rs`
- `docs/src/guides/trust.md`

### 12.2 Suggested helper decomposition

The implementation SHOULD factor the work into small pure helpers:

- `build_support_trust_tree(...) -> TrustDerivationNode`
- `build_rule_trust_tree(rule_label, ...) -> TrustDerivationNode`
- `collect_applicable_defeated_defeaters(...) -> Vec<DiminisherInfo>`
- `apply_diminishment(base_degree, diminishers) -> f64`

This keeps the aggregation semantics testable without requiring full pipeline orchestration in every test.

### 12.3 Interaction with existing `best_conclusion`

The support-side derivation selection policy currently prefers `+D` over `+d` and prefers traceable rule-labelled conclusions. This spec does not change that policy. Diminishment SHALL operate on top of whatever derivation the current trust pipeline selects as the support-side explanation of the conclusion.

### 12.4 Interaction with defeater bodies

A defeater is only "credible" for diminishment purposes if its body is positively supported. The system SHALL NOT assign epistemic weight to an objection whose own premises are unproved.

### 12.5 Explanation semantics

The intended reading of a diminished conclusion is:

> The conclusion remains logically derivable, but its confidence is lowered by credible defeated objections.

This is not a new logical conclusion type. It is a richer annotation on the existing conclusion.

---

## 13. Risks and Open Questions

### 13.1 Risk: Double-counting shared support

If multiple defeaters share body support, naive traversal may repeat work. This is a performance concern, not a semantic blocker. Memoization may be introduced if needed after correctness is established.

### 13.2 Risk: Over-penalization in dense objection sets

Multiplicative aggregation can sharply reduce scores when many moderate defeaters apply. This is accepted in the initial implementation because it is monotone and bounded, but the system SHOULD preserve `base_degree` so future experimentation with alternative aggregators remains possible.

### 13.3 Open question: Support-side source set

This spec intentionally keeps `WeightedConclusion.sources` as support-side contributors only. Defeater-side contributors are carried in `diminished_by` metadata rather than merged into the support source set. This preserves the distinction between supporting witnesses and objecting witnesses.

### 13.4 Open question: Scope beyond defeaters

This spec does not yet treat opposing strict or defeasible rules as diminishers. If the project later wants "all credible losing attacks" rather than only defeaters, a follow-on spec will be required.

---

## 14. Acceptance Criteria

This specification is satisfied when all of the following are true:

1. The production trust pipeline computes and preserves `base_degree` and final diminished `degree`.
2. `WeightedConclusion.diminished_by` is populated for supported defeated defeaters.
3. Thresholds are evaluated against final diminished degree.
4. The logical conclusion set is unchanged.
5. The trust integration suite is extended with contested-but-provable scenarios.
6. Existing uncontested trust scenarios continue to pass.
7. Machine-readable outputs can distinguish `base_degree` from `degree`.

