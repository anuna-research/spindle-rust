# ADR-007: Mining Module Decomposition

| Field       | Value                          |
|-------------|--------------------------------|
| Status      | Proposed                       |
| Date        | 2026-02-11                     |
| Authors     | spindle-rust contributors      |
| Supersedes  | --                             |

## Context

`crates/spindle-core/src/mining.rs` is currently a single 1837 LOC file that conflates
four distinct responsibilities behind one flat namespace:

1. **Event log data structures** -- `Event`, `Case`, `EventLog`, and the `Footprint`
   matrix (lines 1-253).
2. **Process discovery** -- the `AlphaMiner`, `PetriNet`, `Place`, `Transition`, and
   `Arc` types plus the mining algorithm (lines 255-597).
3. **Conflict detection** -- `ConflictType`, `Conflict`, and `detect_conflicts`
   (lines 599-709).
4. **Rule conversion and metrics** -- `LearnedRule`, `calculate_support`,
   `calculate_confidence`, `petri_net_to_rules`, `rules_with_metrics`, plus the
   top-level `mine_rules` orchestrator and `MiningResult` (lines 711-913).
5. **Test helpers and 900+ lines of inline tests** (lines 915-1837).

The problems with the current layout:

- **No clear boundaries.** Conflict detection, semantic validation of mined rules,
  and superiority analysis all live at the same level.  A contributor looking for
  "how do we decide two activities conflict?" must scan through Petri net
  construction, footprint algebra, and metric calculations to find the relevant
  40 lines.
- **Difficult to test in isolation.** Every function operates on concrete owned
  types (`EventLog`, `PetriNet`).  There is no way to feed a unit test a
  hand-crafted iterator of rule pairs without constructing the full event-log
  pipeline first.
- **Growing scope.** Upcoming work (theory validation diagnostics, automatic
  superiority suggestion, completeness checking) will further bloat this file
  unless we establish sub-module boundaries now.
- **Coupling with the rest of the crate.** Only two imports cross into
  `spindle-core` internals (`Literal` and `Rule`/`RuleType`).  The module is
  already loosely coupled, which makes a clean split straightforward.

## Decision

Split `mining.rs` into a `mining/` directory with the following sub-modules:

```
crates/spindle-core/src/mining/
    mod.rs              -- re-exports, MiningResult, mine_rules orchestrator
    event.rs            -- Event, Case, EventLog
    footprint.rs        -- Relation, Footprint
    discovery.rs        -- AlphaMiner, PetriNet, Place, Transition, Arc, ArcNode
    analysis/
        mod.rs          -- re-exports shared types
        conflicts.rs    -- find_conflicts, is_conflicting
        validation.rs   -- validate_theory -> Vec<Diagnostic>
        superiority.rs  -- suggest_superiorities, check_completeness
    conversion.rs       -- LearnedRule, calculate_support, calculate_confidence,
                           petri_net_to_rules, rules_with_metrics
    helpers.rs          -- make_sequential_trace, make_log_from_traces,
                           make_repeated_log (cfg(test) + cfg(feature = "test-helpers"))
```

### Shared types

Three new types anchor the `analysis` sub-module boundary.  They are defined in
`analysis/mod.rs` so every analysis sub-module can return them without circular
imports.

```rust
// mining/analysis/mod.rs

pub mod conflicts;
pub mod superiority;
pub mod validation;

use crate::rule::RuleLabel;
use std::fmt;

/// Two rules whose heads are complementary, with the evidence trail.
#[derive(Debug, Clone)]
pub struct ConflictReport {
    pub rule_a: RuleLabel,
    pub rule_b: RuleLabel,
    pub head_a: String,
    pub head_b: String,
    pub conflict_type: ConflictKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// Classical negation: `p` vs `~p`
    Negation,
    /// Mutual exclusion inferred from traces
    MutualExclusion,
    /// XOR-choice from Petri net structure
    Choice,
}

/// A diagnostic produced by theory validation.
#[derive(Debug, Clone)]
pub struct ValidationDiagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    /// The rule label(s) involved, if applicable.
    pub rules: Vec<RuleLabel>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
}

impl fmt::Display for ValidationDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}: {}", self.severity, self.code, self.message)
    }
}

/// A suggested superiority relation with justification.
#[derive(Debug, Clone)]
pub struct SuperioritySuggestion {
    pub superior: RuleLabel,
    pub inferior: RuleLabel,
    /// Human-readable justification for why this superiority makes sense.
    pub reason: String,
    /// Confidence score in [0.0, 1.0] based on trace evidence.
    pub confidence: f64,
}
```

### Iterator-based API for maximum testability

Each analysis sub-module accepts iterators over rules or rule pairs rather than
requiring a fully-constructed `EventLog` or `PetriNet`.  This lets unit tests
supply hand-crafted inputs without going through the entire mining pipeline.

```rust
// mining/analysis/conflicts.rs

use super::{ConflictKind, ConflictReport};
use crate::rule::Rule;

/// Check whether two rules have conflicting heads.
///
/// Two heads conflict when one is the classical negation of the other
/// (e.g. `flies` vs `~flies`).
pub fn is_conflicting(a: &Rule, b: &Rule) -> bool {
    a.head.iter().any(|ha| {
        b.head.iter().any(|hb| {
            ha.name() == hb.name() && ha.is_negated() != hb.is_negated()
        })
    })
}

/// Scan an iterator of rules and return every pair whose heads conflict.
///
/// Accepts `impl IntoIterator` so callers can pass `&[Rule]`, `Vec<Rule>`,
/// or a filtered iterator without allocation.
pub fn find_conflicts<'a, I>(rules: I) -> Vec<ConflictReport>
where
    I: IntoIterator<Item = &'a Rule>,
{
    let rules: Vec<&Rule> = rules.into_iter().collect();
    let mut reports = Vec::new();

    for i in 0..rules.len() {
        for j in (i + 1)..rules.len() {
            if is_conflicting(rules[i], rules[j]) {
                reports.push(ConflictReport {
                    rule_a: rules[i].label.clone(),
                    rule_b: rules[j].label.clone(),
                    head_a: format!("{}", rules[i].head[0]),
                    head_b: format!("{}", rules[j].head[0]),
                    conflict_type: ConflictKind::Negation,
                });
            }
        }
    }

    reports
}
```

```rust
// mining/analysis/validation.rs

use super::{Severity, ValidationDiagnostic};
use crate::rule::{Rule, RuleType};

/// Validate a set of rules and produce diagnostics.
///
/// Current checks:
/// - Unsupported head: a defeasible rule whose head never appears in any
///   strict rule or fact (orphan conclusion).
/// - Empty body non-fact: a non-fact rule with an empty body that will
///   never fire in the standard algorithm.
pub fn validate_theory<'a, I>(rules: I) -> Vec<ValidationDiagnostic>
where
    I: IntoIterator<Item = &'a Rule>,
{
    let rules: Vec<&Rule> = rules.into_iter().collect();
    let mut diagnostics = Vec::new();

    for rule in &rules {
        // Empty-body non-fact rules never fire in reason.rs
        if rule.body.is_empty() && rule.rule_type != RuleType::Fact {
            diagnostics.push(ValidationDiagnostic {
                severity: Severity::Warning,
                code: "W001",
                message: format!(
                    "Rule '{}' has an empty body but is not a fact; \
                     it will never fire in the standard reasoner.",
                    rule.label,
                ),
                rules: vec![rule.label.clone()],
            });
        }
    }

    // Check for orphan conclusions (heads that no strict rule/fact supports)
    let strict_heads: std::collections::HashSet<&str> = rules
        .iter()
        .filter(|r| matches!(r.rule_type, RuleType::Strict | RuleType::Fact))
        .flat_map(|r| r.head.iter().map(|h| h.name()))
        .collect();

    for rule in &rules {
        if rule.rule_type == RuleType::Defeasible {
            for h in &rule.head {
                if !strict_heads.contains(h.name())
                    && !rules.iter().any(|r| {
                        r.rule_type == RuleType::Defeasible
                            && r.label != rule.label
                            && r.head.iter().any(|rh| {
                                rh.name() == h.name()
                                    && rh.is_negated() != h.is_negated()
                            })
                    })
                {
                    // Only warn if no other rule contests this head
                    // (uncontested defeasible conclusions are fine)
                }
            }
        }
    }

    diagnostics
}
```

```rust
// mining/analysis/superiority.rs

use super::SuperioritySuggestion;
use crate::rule::Rule;

/// Given a set of conflicting rule pairs, suggest superiority relations
/// based on specificity (more body literals = more specific = superior).
pub fn suggest_superiorities<'a, I>(conflicts: I) -> Vec<SuperioritySuggestion>
where
    I: IntoIterator<Item = (&'a Rule, &'a Rule)>,
{
    let mut suggestions = Vec::new();

    for (a, b) in conflicts {
        // Heuristic: the more specific rule (more body literals) should
        // be superior.  Equal specificity produces no suggestion.
        let spec_a = a.body.len();
        let spec_b = b.body.len();

        if spec_a > spec_b {
            suggestions.push(SuperioritySuggestion {
                superior: a.label.clone(),
                inferior: b.label.clone(),
                reason: format!(
                    "'{}' has {} body literals vs {} for '{}' (more specific)",
                    a.label, spec_a, spec_b, b.label,
                ),
                confidence: specificity_confidence(spec_a, spec_b),
            });
        } else if spec_b > spec_a {
            suggestions.push(SuperioritySuggestion {
                superior: b.label.clone(),
                inferior: a.label.clone(),
                reason: format!(
                    "'{}' has {} body literals vs {} for '{}' (more specific)",
                    b.label, spec_b, spec_a, a.label,
                ),
                confidence: specificity_confidence(spec_b, spec_a),
            });
        }
    }

    suggestions
}

/// Confidence based on the ratio of specificity difference.
fn specificity_confidence(more: usize, fewer: usize) -> f64 {
    if more == 0 {
        return 0.0;
    }
    (more - fewer) as f64 / more as f64
}

/// Check whether every conflicting pair in the theory has a declared
/// superiority relation.  Returns labels of unresolved pairs.
pub fn check_completeness<'a, I, S>(
    conflicts: I,
    declared: S,
) -> Vec<(String, String)>
where
    I: IntoIterator<Item = (&'a str, &'a str)>,
    S: IntoIterator<Item = (&'a str, &'a str)>,
{
    let declared_set: std::collections::HashSet<(&str, &str)> =
        declared.into_iter().collect();

    conflicts
        .into_iter()
        .filter(|(a, b)| {
            !declared_set.contains(&(*a, *b))
                && !declared_set.contains(&(*b, *a))
        })
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}
```

### Re-export surface in `mining/mod.rs`

The top-level `mining/mod.rs` re-exports everything that was previously public
from the flat `mining.rs`, preserving backward compatibility:

```rust
// mining/mod.rs

mod event;
mod footprint;
mod discovery;
mod conversion;
pub mod analysis;
#[cfg(any(test, feature = "test-helpers"))]
mod helpers;

// Re-export all public types at the mining:: level for backward compat
pub use event::{Event, Case, EventLog};
pub use footprint::{Relation, Footprint};
pub use discovery::{AlphaMiner, PetriNet, Place, Transition, Arc, ArcNode};
pub use conversion::{
    LearnedRule, calculate_support, calculate_confidence,
    petri_net_to_rules, rules_with_metrics,
};
pub use analysis::{
    ConflictReport, ConflictKind, ValidationDiagnostic,
    Severity, SuperioritySuggestion,
};
#[cfg(any(test, feature = "test-helpers"))]
pub use helpers::{make_sequential_trace, make_log_from_traces, make_repeated_log};

// --- Existing types that stay here ---

pub use self::conflicts_compat::{Conflict, ConflictType, detect_conflicts};
mod conflicts_compat {
    //! Backward-compatible wrappers around the original conflict detection
    //! that operates on EventLog + PetriNet (trace-level, not rule-level).
    pub use super::*;
    // The original detect_conflicts, Conflict, ConflictType stay unchanged.
    // They are defined in discovery.rs or a compat shim.
}

/// Complete result of the process mining pipeline.
#[derive(Debug, Clone)]
pub struct MiningResult {
    pub rules: Vec<LearnedRule>,
    pub conflicts: Vec<Conflict>,
    pub petri_net: PetriNet,
    pub footprint: Footprint,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Mine rules from an event log.  Top-level orchestrator.
pub fn mine_rules(
    log: &EventLog,
    min_support: usize,
    min_confidence: f64,
) -> MiningResult {
    // ... same implementation as today ...
    # unimplemented!()
}
```

### Property-based testing with proptest

Conflict detection is the highest-risk logic (it underpins whether the
reasoner even sees a conflict).  We add proptest strategies:

```rust
// In crates/spindle-core/tests/mining_conflicts_proptest.rs

use proptest::prelude::*;
use spindle_core::literal::Literal;
use spindle_core::rule::{Rule, RuleType};
use spindle_core::mining::analysis::conflicts::{find_conflicts, is_conflicting};

/// Generate a random literal name from a small alphabet to force collisions.
fn arb_atom() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "p".to_string(),
        "q".to_string(),
        "r".to_string(),
        "s".to_string(),
    ])
}

/// Generate a random rule with 1-3 body literals and 1 head literal,
/// where the head may or may not be negated.
fn arb_rule() -> impl Strategy<Value = Rule> {
    (
        "[a-z]{1,4}",              // label
        prop::collection::vec(arb_atom(), 1..=3),  // body atoms
        arb_atom(),                // head atom
        any::<bool>(),             // head negated?
    )
        .prop_map(|(label, body_atoms, head_atom, negated)| {
            let body: Vec<Literal> = body_atoms
                .iter()
                .map(|a| Literal::simple(a))
                .collect();
            let head = if negated {
                Literal::negated(&head_atom)
            } else {
                Literal::simple(&head_atom)
            };
            Rule::new(label, RuleType::Defeasible, body, vec![head])
        })
}

proptest! {
    /// Every pair returned by find_conflicts must satisfy is_conflicting.
    #[test]
    fn conflicts_sound(rules in prop::collection::vec(arb_rule(), 2..=20)) {
        let reports = find_conflicts(&rules);
        for report in &reports {
            let a = rules.iter().find(|r| r.label == report.rule_a).unwrap();
            let b = rules.iter().find(|r| r.label == report.rule_b).unwrap();
            prop_assert!(
                is_conflicting(a, b),
                "find_conflicts reported {:?} vs {:?} but is_conflicting disagrees",
                report.rule_a,
                report.rule_b,
            );
        }
    }

    /// is_conflicting is symmetric: if (a,b) conflicts then (b,a) conflicts.
    #[test]
    fn conflict_symmetric(
        a in arb_rule(),
        b in arb_rule(),
    ) {
        prop_assert_eq!(
            is_conflicting(&a, &b),
            is_conflicting(&b, &a),
            "is_conflicting must be symmetric",
        );
    }

    /// A rule never conflicts with itself (same head, same polarity).
    #[test]
    fn no_self_conflict(r in arb_rule()) {
        prop_assert!(
            !is_conflicting(&r, &r),
            "A rule should not conflict with itself",
        );
    }
}
```

## Consequences

### Benefits

- **Clear boundaries.** Each file has a single responsibility: event structures,
  footprint algebra, process discovery, conflict detection, validation,
  superiority analysis, or metric-based rule conversion.  A new contributor
  can navigate to the right file by name alone.
- **Iterator-based APIs unlock isolated unit testing.** Tests for
  `is_conflicting` no longer need to construct an `EventLog` or run the alpha
  miner -- they feed a pair of `Rule` values directly.
- **Property-based testing catches edge cases.** The proptest suite exercises
  soundness, symmetry, and reflexivity invariants across hundreds of
  randomly-generated theories.
- **Incremental growth.** New analysis passes (e.g. loop detection, deadlock
  analysis) slot into `analysis/` without touching the discovery or event
  modules.
- **Backward compatibility.** The `mining/mod.rs` re-exports preserve the
  existing `use spindle_core::mining::{...}` import paths.  No downstream
  breakage in `spindle-parser`, `spindle-cli`, `spindle-wasm`, or the mdBook
  guide code snippets.

### Costs

- **More files to navigate.** Seven source files plus three analysis files
  replace one.  IDE "go to definition" mitigates this in practice.
- **Temporary churn.** Moving code across files touches `git blame` for the
  entire module.  Use `git log --follow` for post-split archaeology.
- **Re-export maintenance.** If a new public type is added in a sub-module,
  it must be re-exported in `mining/mod.rs` to maintain the flat namespace
  contract.

### Trade-offs

| Aspect             | Before (flat file)                          | After (sub-modules)                              |
|--------------------|---------------------------------------------|--------------------------------------------------|
| Discoverability    | Ctrl-F in one 1837-line file                | Navigate to purpose-named file                   |
| Test isolation     | Need full `EventLog` for any test           | Feed raw `&[Rule]` or iterator                   |
| Compile time       | Single compilation unit                     | Parallel compilation of sub-modules              |
| Import paths       | `use spindle_core::mining::Foo`             | Same (re-exports), or `mining::analysis::Foo`    |
| `git blame`        | Intact history                              | Reset on move; use `--follow`                    |
| New analysis pass  | Append to bottom of 1837-line file          | Create new file in `analysis/`                   |

### Migration plan

1. Create the `mining/` directory and `mod.rs` with re-exports.
2. Move types and functions file-by-file, one PR per sub-module, keeping
   `cargo test --all` green at every step.
3. Add the `analysis/` sub-modules with the new iterator-based API alongside
   the existing `detect_conflicts` function (which operates on `EventLog` +
   `PetriNet`).
4. Add the proptest suite in `crates/spindle-core/tests/mining_conflicts_proptest.rs`.
5. Deprecate nothing -- the re-exports keep every existing import path valid.

### Validation

- `cargo test -p spindle-core` passes before and after each migration PR.
- `cargo doc --no-deps -p spindle-core` builds without broken intra-doc links.
- The proptest suite (`cargo test --test mining_conflicts_proptest`) runs 500+
  cases with no failures.
- The mdBook guide code snippets (`docs/src/guides/mining.md`) compile
  unchanged because `use spindle_core::mining::{...}` paths are preserved.
