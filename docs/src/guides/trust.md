# Trust-Weighted Reasoning

Spindle supports trust-weighted defeasible reasoning, enabling source attribution, trust-weighted conclusions, partial defeat (diminishment), and multi-perspective evaluation.

## Overview

In multi-agent and multi-source environments, not all information is equally reliable. Trust-weighted reasoning extends Spindle's defeasible logic with:

- **Source attribution**: Track which agents or systems contributed facts and rules
- **Trust weighting**: Assign trust values to sources and propagate them through derivations
- **Weakest-link model**: A conclusion's trust degree equals the minimum trust in its derivation chain
- **Partial defeat (diminishment)**: Defeaters can reduce a conclusion's trust without fully defeating it
- **Threshold evaluation**: Named thresholds determine whether a conclusion is actionable
- **Multi-perspective evaluation**: Different trust policies can evaluate the same derivation differently

## Source Attribution and Claims

### Source Identifiers

Sources use a `category:name` format to identify agents, systems, or users:

```
agent:security    - A security scanning agent
agent:coder       - A code analysis agent
agent:qa          - A QA testing agent
system:policy     - System-level policy rules
user:admin        - An administrative user
```

### SPL Claims Syntax

The `claims` block attributes statements to a source identity:

```lisp
(claims agent:security
  :at "2026-01-20T09:00:00Z"
  :note "Automated security scan results"
  (given vulnerability_detected)
  (normally sec1 vulnerability_detected security_risk))
```

**Syntax**: `(claims source claims-meta? statements...)`

**Metadata fields** (all optional):

| Field  | Description                          | Example                          |
|--------|--------------------------------------|----------------------------------|
| `:at`  | ISO 8601 timestamp                   | `:at "2026-01-20T09:00:00Z"`    |
| `:sig` | Cryptographic signature              | `:sig "abc123signature"`         |
| `:note`| Human-readable annotation            | `:note "CI pipeline results"`    |

**Allowed inner statements**:

- Facts: `(given literal)`
- Rules: `(normally label body head)`, `(always label body head)`, `(except label body head)`
- Superiorities: `(prefer label1 label2)`

Claims blocks cannot be nested.

### Multi-Agent Example

Multiple agents contribute claims about a pull request:

```lisp
(claims agent:security
  :at "2026-01-20T09:00:00Z"
  :note "Automated security scan results"
  (given vulnerability_detected)
  (normally sec1 vulnerability_detected security_risk))

(claims agent:coder
  :at "2026-01-20T09:30:00Z"
  :note "CI pipeline results"
  (given tests_pass)
  (normally dev1 tests_pass code_compiles))

; Global superiority (outside claims blocks)
(prefer sec1 dev1)
```

### Rust API: Source and SourcedConclusion

```rust
use spindle_core::trust::{Source, SourcedConclusion};
use spindle_core::conclusion::ConclusionType;
use spindle_core::literal::Literal;

// Create sources
let alice = Source::new("agent:alice");
let bob = Source::with_label("agent:bob", "Bob the Reviewer");

// Display formatting
println!("{}", alice);  // "agent:alice"
println!("{}", bob);    // "Bob the Reviewer (agent:bob)"

// Track source attribution on conclusions
let conclusion = SourcedConclusion::new(
        Literal::simple("approved"),
        ConclusionType::DefeasiblyProvable,
    )
    .with_source(Source::new("agent:coder"))
    .with_source(Source::new("agent:reviewer"))
    .with_source(Source::new("agent:security"))
    .with_derivation("r1")
    .with_derivation("r2");

assert_eq!(conclusion.sources.len(), 3);
assert_eq!(conclusion.derivation, vec!["r1", "r2"]);
```

## Trust Policies and Configuration

A `TrustPolicy` defines how much to trust each source, what thresholds to apply, and the default trust for unknown sources.

### Creating a Trust Policy

```rust
use spindle_core::trust::TrustPolicy;

let policy = TrustPolicy::new(0.5)              // default trust for unknown sources
    .with_trust("agent:coder", 0.9)              // high trust
    .with_trust("agent:security", 0.95)          // very high trust
    .with_trust("system:policy", 1.0)            // full trust
    .with_trust("external:api", 0.6)             // moderate trust
    .with_threshold("action", 0.7)               // threshold for taking action
    .with_threshold("warn", 0.5)                 // threshold for warnings
    .with_threshold("log", 0.3);                 // threshold for logging
```

### Querying Trust

```rust
// Look up trust for a known source
assert_eq!(policy.get_trust("agent:coder"), 0.9);
assert_eq!(policy.get_trust("agent:security"), 0.95);

// Unknown sources fall back to default_trust
assert_eq!(policy.get_trust("unknown_agent"), 0.5);
```

### Trust Value Range

Trust values are `f64` in the range `[0.0, 1.0]`:

| Value | Meaning                                      |
|-------|----------------------------------------------|
| `0.0` | No trust (fully untrusted)                   |
| `0.5` | Neutral / unknown                            |
| `1.0` | Full trust (axiomatically reliable)           |

## Weakest-Link Trust Model

The trust degree of a derived conclusion equals the **minimum trust value** encountered along its entire derivation chain. This is the "weakest-link" model: a conclusion is only as trustworthy as the least trusted step that produced it.

### How It Works

Given a derivation tree, each node has a trust value from its contributing source. The `weakest_link_trust()` method recursively computes the minimum:

```
       root (0.8)
      /         \
branch1 (0.9)   branch2 (0.6)
    |                |
leaf1 (0.95)    leaf2 (0.7)
```

The weakest link is `0.6` (from `branch2`).

### Rust API: TrustDerivationNode

```rust
use spindle_core::trust::{TrustDerivationNode, Source};
use spindle_core::literal::Literal;

// Build a derivation tree
let leaf1 = TrustDerivationNode::new(Literal::simple("bird"), 0.9)
    .with_source(Source::new("agent:alice"));
let leaf2 = TrustDerivationNode::new(Literal::simple("healthy"), 0.7)
    .with_source(Source::new("agent:bob"));

let root = TrustDerivationNode::new(Literal::simple("flies"), 0.8)
    .with_children(vec![leaf1, leaf2]);

// Weakest link is 0.7 (from "healthy" via agent:bob)
assert_eq!(root.weakest_link_trust(), 0.7);
```

### Chain Propagation

In a linear derivation chain, the weakest link propagates upward:

```rust
// Chain: a (0.9) -> b (0.9) -> c (0.5)
let leaf = TrustDerivationNode::new(Literal::simple("a"), 0.9)
    .with_source(Source::new("agent:hightrust"));
let mid = TrustDerivationNode::new(Literal::simple("b"), 0.9)
    .with_children(vec![leaf]);
let root = TrustDerivationNode::new(Literal::simple("c"), 0.5)
    .with_source(Source::new("agent:lowtrust"))
    .with_children(vec![mid]);

// Weakest link is 0.5 (from node "c")
assert_eq!(root.weakest_link_trust(), 0.5);
```

### Single Source

When a conclusion comes from a single source with no derivation chain, the degree equals the source's trust value directly:

```rust
let node = TrustDerivationNode::new(Literal::simple("tests_pass"), 0.9)
    .with_source(Source::new("agent:coder"));

assert_eq!(node.weakest_link_trust(), 0.9);
```

## Partial Defeat (Diminishment)

Standard defeasible logic uses binary defeat: a conclusion is either proven or not. Trust-weighted reasoning introduces **diminishment**, where a defeater can reduce a conclusion's trust degree without fully defeating it.

### Diminishment Formula

```
diminishment = min(defeater_degree * target_degree, target_degree)
resulting_degree = (target_degree - diminishment).max(0.0)
```

If the defeater fully defeats the target, the resulting degree is `0.0`.

### Example Calculation

Given a target with degree `0.8` and a defeater with degree `0.4`:

```
diminishment = min(0.4 * 0.8, 0.8) = min(0.32, 0.8) = 0.32
resulting_degree = (0.8 - 0.32).max(0.0) = 0.48
```

The conclusion survives but with reduced trust.

### Rust API: DiminisherInfo

```rust
use spindle_core::trust::DiminisherInfo;

// Partial diminishment
let dim = DiminisherInfo::new("defeater_rule", 0.4, 0.8);
assert_eq!(dim.defeater_label, "defeater_rule");
assert_eq!(dim.defeater_degree, 0.4);
assert_eq!(dim.target_degree, 0.8);
assert!(!dim.full_defeat);
// resulting_degree = 0.8 - min(0.4 * 0.8, 0.8) = 0.48
assert!((dim.resulting_degree() - 0.48).abs() < 0.001);

// Full defeat
let full = DiminisherInfo::new("strong_defeater", 0.9, 0.7).as_full_defeat();
assert!(full.full_defeat);
assert_eq!(full.resulting_degree(), 0.0);
```

### Diminished Conclusions

A `WeightedConclusion` tracks all diminishers that affected it:

```rust
use spindle_core::trust::WeightedConclusion;
use spindle_core::conclusion::ConclusionType;
use spindle_core::literal::Literal;

let mut wc = WeightedConclusion::new(
    Literal::simple("approved"),
    ConclusionType::DefeasiblyProvable,
    0.9,
);

assert!(!wc.was_diminished());

// Apply diminishers
wc.diminished_by.push(DiminisherInfo::new("d1", 0.3, 0.9));
wc.diminished_by.push(DiminisherInfo::new("d2", 0.4, 0.9));

assert!(wc.was_diminished());
assert_eq!(wc.diminished_by.len(), 2);
```

### Resulting Degree is Never Negative

Even with strong diminishment, the resulting degree is clamped to `0.0`:

```rust
let dim = DiminisherInfo::new("strong", 1.0, 0.5);
assert!(dim.resulting_degree() >= 0.0);
```

## Threshold-Based Decisions

Named thresholds allow you to make decisions based on trust levels without hardcoding numeric comparisons throughout your application.

### Defining Thresholds

```rust
let policy = TrustPolicy::new(0.5)
    .with_threshold("action", 0.7)   // safe to act on
    .with_threshold("warn", 0.5)     // worth a warning
    .with_threshold("log", 0.3);     // worth logging
```

### Evaluating Against Thresholds

```rust
// A conclusion with degree 0.6
assert_eq!(policy.is_above_threshold(0.6, "action"), Some(false));  // below action
assert_eq!(policy.is_above_threshold(0.6, "warn"), Some(true));     // above warn
assert_eq!(policy.is_above_threshold(0.6, "log"), Some(true));      // above log

// Unknown thresholds return None
assert_eq!(policy.is_above_threshold(0.9, "unknown"), None);
```

### Boundary Behavior

Threshold evaluation uses `>=` (greater than or equal):

```rust
let policy = TrustPolicy::new(0.5)
    .with_threshold("exact", 0.7);

// Exactly at threshold is considered above
assert_eq!(policy.is_above_threshold(0.7, "exact"), Some(true));
assert_eq!(policy.is_above_threshold(0.69999, "exact"), Some(false));
```

### Per-Conclusion Threshold Results

`WeightedConclusion` stores pre-computed threshold results:

```rust
let mut wc = WeightedConclusion::new(
    Literal::simple("important_fact"),
    ConclusionType::DefeasiblyProvable,
    0.9,
);

wc.above_threshold.insert("action".to_string(), true);
wc.above_threshold.insert("warn".to_string(), true);
wc.above_threshold.insert("critical".to_string(), false);

assert_eq!(wc.is_above_threshold("action"), Some(true));
assert_eq!(wc.is_above_threshold("critical"), Some(false));
assert_eq!(wc.is_above_threshold("unknown"), None);
```

## Multi-Perspective Evaluation

The same derivation can be evaluated under different trust policies, yielding different conclusions. This models real-world scenarios where different stakeholders have different trust assessments.

### Different Perspectives on the Same Sources

```rust
// Security team perspective: trusts security agents highly
let security_perspective = TrustPolicy::new(0.5)
    .with_trust("agent:security", 0.95)
    .with_trust("agent:coder", 0.6);

// Developer perspective: trusts coders highly
let developer_perspective = TrustPolicy::new(0.5)
    .with_trust("agent:security", 0.5)
    .with_trust("agent:coder", 0.9);

// Same source, different trust values
assert!(
    security_perspective.get_trust("agent:security")
    > security_perspective.get_trust("agent:coder")
);
assert!(
    developer_perspective.get_trust("agent:coder")
    > developer_perspective.get_trust("agent:security")
);
```

### Conservative vs. Permissive Policies

```rust
// Conservative: high thresholds, low default trust
let conservative = TrustPolicy::new(0.3)
    .with_threshold("action", 0.9)
    .with_threshold("warn", 0.7);

// Permissive: low thresholds, high default trust
let permissive = TrustPolicy::new(0.8)
    .with_threshold("action", 0.5)
    .with_threshold("warn", 0.3);

let degree = 0.75;

// Conservative: above warn, below action
assert_eq!(conservative.is_above_threshold(degree, "action"), Some(false));
assert_eq!(conservative.is_above_threshold(degree, "warn"), Some(true));

// Permissive: above both
assert_eq!(permissive.is_above_threshold(degree, "action"), Some(true));
assert_eq!(permissive.is_above_threshold(degree, "warn"), Some(true));
```

This enables the same reasoning results to drive different behavior depending on which stakeholder's perspective is applied.

## Trust Explanations

`TrustExplanation` provides a complete explanation of how a conclusion's trust degree was derived, including the derivation tree and any diminishers that affected it.

### Generating Explanations

```rust
use spindle_core::trust::{TrustExplanation, TrustDerivationNode, DiminisherInfo, Source};
use spindle_core::literal::Literal;

// Build derivation tree
let leaf = TrustDerivationNode::new(Literal::simple("premise"), 0.9)
    .with_source(Source::with_label("src1", "Source One"));
let root = TrustDerivationNode::new(Literal::simple("conclusion"), 0.85)
    .with_children(vec![leaf]);

// Create explanation
let explanation = TrustExplanation::new(Literal::simple("conclusion"), 0.85)
    .with_tree(root);

println!("{}", explanation.to_natural_language());
```

### Natural Language Output

The `to_natural_language()` method produces human-readable output:

```
Trust Explanation for "conclusion"
Final trust degree: 0.85

Derivation tree:
  1. "conclusion" (trust: 0.85)
     1. "premise" (trust: 0.90) [source: Source One (src1)]
```

### Explanations with Diminishers

```rust
let dim1 = DiminisherInfo::new("d1", 0.4, 0.9);
let dim2 = DiminisherInfo::new("d2", 0.3, 0.9).as_full_defeat();

let explanation = TrustExplanation::new(Literal::simple("goal"), 0.0)
    .with_diminishers(vec![dim1, dim2]);

println!("{}", explanation.to_natural_language());
```

Output includes diminisher details:

```
Trust Explanation for "goal"
Final trust degree: 0.00

Diminishers:
  1. Diminished by 'd1' (degree 0.40): 0.90 -> 0.45
  2. Fully defeated by 'd2' (degree 0.30)
```

### Non-Provable Literals

When a literal is not provable, the explanation has a zero degree and no derivation tree:

```rust
let explanation = TrustExplanation::new(Literal::simple("not_provable"), 0.0);
assert_eq!(explanation.final_degree, 0.0);
assert!(explanation.derivation_tree.is_none());
```

## Use Cases

### Multi-Agent Systems

In a code review pipeline, multiple agents contribute assessments with varying trust levels:

```lisp
; Security scanner has high credibility for vulnerability findings
(claims agent:security
  :at "2026-01-20T09:00:00Z"
  :note "Automated security scan results"
  (given vulnerability_detected)
  (normally sec1 vulnerability_detected security_risk))

; CI pipeline reports test results
(claims agent:coder
  :at "2026-01-20T09:30:00Z"
  :note "CI pipeline results"
  (given tests_pass)
  (normally dev1 tests_pass code_compiles))

; Superiority: security findings override development claims
(prefer sec1 dev1)
```

A trust policy assigns credibility:

```rust
let policy = TrustPolicy::new(0.5)
    .with_trust("agent:security", 0.95)
    .with_trust("agent:coder", 0.9)
    .with_trust("agent:qa", 0.85)
    .with_threshold("action", 0.7)
    .with_threshold("warn", 0.5);
```

### Auditing

Trust explanations provide a full audit trail for every conclusion:

- Which sources contributed
- What derivation chain was followed
- What the trust degree is at each step
- Whether any diminishers reduced the conclusion
- Whether the conclusion meets each named threshold

This is useful for compliance requirements where decisions must be traceable and explainable.

### Regulatory Compliance

Different regulatory frameworks can be modeled as different trust policies applied to the same reasoning results:

```rust
// Strict regulatory perspective
let regulatory = TrustPolicy::new(0.3)
    .with_trust("system:policy", 1.0)
    .with_trust("agent:auditor", 0.95)
    .with_trust("external:vendor", 0.4)
    .with_threshold("compliant", 0.9)
    .with_threshold("review_needed", 0.7);

// Internal operations perspective
let operations = TrustPolicy::new(0.7)
    .with_trust("system:policy", 1.0)
    .with_trust("agent:auditor", 0.8)
    .with_trust("external:vendor", 0.7)
    .with_threshold("compliant", 0.6)
    .with_threshold("review_needed", 0.4);

// Same conclusion degree, different compliance outcomes
let degree = 0.75;
assert_eq!(regulatory.is_above_threshold(degree, "compliant"), Some(false));
assert_eq!(operations.is_above_threshold(degree, "compliant"), Some(true));
```

## Limitations

1. **Claims parsing**: The `(claims ...)` SPL syntax is documented but not yet implemented in the parser. Source attribution is available through the Rust API.
2. **Static trust values**: Trust values are fixed per policy. Dynamic trust that updates based on track record is not built in.
3. **Weakest-link only**: The model uses minimum trust propagation. Alternative models (e.g., weighted average, product) are not supported.
4. **No cryptographic verification**: The `:sig` metadata field is stored but not verified against any cryptographic infrastructure.
5. **Floating-point precision**: Trust values are `f64`, so standard floating-point precision considerations apply to boundary comparisons.
