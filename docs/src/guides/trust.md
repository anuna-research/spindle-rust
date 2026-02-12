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
- **Trust decay**: Time-based trust adjustment with exponential, linear, and step-function models
- **Trust directives**: Declarative trust configuration in SPL format
- **Pipeline integration**: Automatic trust-weighted conclusions from the reasoning pipeline
- **Trust-filtered queries**: Filter reasoning results by source and minimum trust degree
- **CLI trust output**: Display trust weights alongside conclusions with `--trust`

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
| `:id`  | Claims block identifier              | `:id "claim-001"`               |
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

## Trust Directives in SPL

Trust policies can be declared directly in SPL source files using three directive forms.

### `(trusts source value)`

Assigns a trust value to a source identifier:

```lisp
(trusts agent:security 0.95)
(trusts agent:coder 0.9)
(trusts system:policy 1.0)
(trusts external:api 0.6)
```

Values must be in the range `[0.0, 1.0]`.

### `(decays source model param)`

Attaches a time-based decay model to a source. The decay model reduces trust as assertions age:

```lisp
; Trust halves every hour (3600 seconds)
(decays agent:sensor exponential 3600.0)

; Trust decreases at 0.001 per second
(decays agent:temp linear 0.001)

; Trust drops to zero after 24 hours
(decays agent:ephemeral step 86400.0)
```

See [Decay Models](#decay-models) for formula details.

### `(threshold name value)`

Defines a named threshold for decision-making:

```lisp
(threshold action 0.7)
(threshold warn 0.5)
(threshold log 0.3)
```

### Complete SPL Example

```lisp
; Trust configuration
(trusts agent:security 0.95)
(trusts agent:coder 0.9)
(decays agent:sensor exponential 3600.0)
(threshold action 0.7)
(threshold warn 0.5)

; Claims from sources
(claims agent:security
  :at "2026-01-20T09:00:00Z"
  (given vulnerability_detected)
  (normally sec1 vulnerability_detected security_risk))

(claims agent:coder
  :at "2026-01-20T09:30:00Z"
  (given tests_pass)
  (normally dev1 tests_pass code_compiles))

(prefer sec1 dev1)
```

## Decay Models

Decay models compute a time-dependent multiplier in `[0.0, 1.0]` that is applied to a source's base trust value. This models the intuition that older assertions become less trustworthy over time.

### Exponential Decay

```
multiplier = 0.5 ^ (age_secs / half_life_secs)
```

After one half-life, trust is halved. After two half-lives, trust is quartered. Trust approaches zero asymptotically.

```rust
use spindle_core::trust::{DecayModel, TrustPolicy};

let policy = TrustPolicy::new(0.5)
    .with_trust("agent:sensor", 0.9)
    .with_decay("agent:sensor", DecayModel::Exponential { half_life_secs: 3600.0 });

// At age 0: effective trust = 0.9
assert_eq!(policy.get_effective_trust("agent:sensor", 0.0), 0.9);

// At 1 hour: effective trust = 0.9 * 0.5 = 0.45
let at_1h = policy.get_effective_trust("agent:sensor", 3600.0);
assert!((at_1h - 0.45).abs() < 1e-10);
```

### Linear Decay

```
multiplier = max(1.0 - rate_per_sec * age_secs, 0.0)
```

Trust decreases at a constant rate per second, reaching zero at `1.0 / rate` seconds.

```rust
let policy = TrustPolicy::new(0.5)
    .with_trust("agent:temp", 1.0)
    .with_decay("agent:temp", DecayModel::Linear { rate_per_sec: 0.001 });

// At 500 seconds: 1.0 * (1.0 - 0.001 * 500) = 0.5
let at_500s = policy.get_effective_trust("agent:temp", 500.0);
assert!((at_500s - 0.5).abs() < 1e-10);

// At 1500 seconds: fully decayed to 0.0
assert_eq!(policy.get_effective_trust("agent:temp", 1500.0), 0.0);
```

### Step Function

```
multiplier = 1.0 if age_secs < cutoff_secs, else 0.0
```

Trust is full until the cutoff, then drops to zero instantly. Useful for time-limited credentials or ephemeral assertions.

```rust
let policy = TrustPolicy::new(0.5)
    .with_trust("agent:session", 0.9)
    .with_decay("agent:session", DecayModel::StepFunction { cutoff_secs: 86400.0 });

// Before cutoff: full trust
assert_eq!(policy.get_effective_trust("agent:session", 100.0), 0.9);

// At cutoff: zero trust
assert_eq!(policy.get_effective_trust("agent:session", 86400.0), 0.0);
```

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

## Pipeline Integration

The reasoning pipeline can automatically compute trust-weighted conclusions after reasoning. The `compute_weighted_conclusions` function examines each conclusion's derivation rule, looks up the source metadata, and applies the theory's trust policy.

### Rust API

```rust
use spindle_core::pipeline::{prepare, PrepareOptions, compute_weighted_conclusions};

// Parse a theory with trust directives
let theory = spindle_parser::parse_spl(r#"
    (trusts agent:security 0.95)
    (trusts agent:coder 0.9)
    (threshold action 0.7)

    (claims agent:security
      (given vulnerability_detected)
      (normally sec1 vulnerability_detected security_risk))
"#).unwrap();

// Run the pipeline
let opts = PrepareOptions::default();
let result = prepare(&theory, opts).unwrap();

// Reason
let conclusions = spindle_core::reason::reason_prepared(&result.theory).unwrap();

// Compute trust-weighted conclusions
let policy = result.theory.trust_policy();
let weighted = compute_weighted_conclusions(&conclusions, &result.theory, policy);

for wc in &weighted {
    println!("{}: trust={:.2}, sources={:?}",
        wc.literal, wc.degree,
        wc.sources.iter().map(|s| &s.id).collect::<Vec<_>>());
}
```

Each `WeightedConclusion` contains:
- `degree`: Trust value from the source's policy entry (or default)
- `sources`: Set of contributing source identifiers
- `above_threshold`: Pre-computed pass/fail for each named threshold

## CLI Usage

### `--trust` Flag

The `reason` command accepts a `--trust` flag that displays trust weights alongside conclusions:

```bash
spindle reason --trust theory.spl
```

Output format:

```
Conclusions:

  +D bird (trust: 0.95) [agent:security]
  +d flies (trust: 0.90) [agent:coder]
  -d -flies (trust: 0.90) [agent:coder]
```

Each conclusion shows:
- The provability symbol (`+D`, `-D`, `+d`, `-d`)
- The literal
- The trust degree in parentheses
- The contributing sources in brackets

Without `--trust`, conclusions display in the standard format without trust information.

## Trust-Filtered Queries

The `TrustFilter` struct allows filtering reasoning results by minimum trust degree and source pattern.

### Rust API

```rust
use spindle_core::query::TrustFilter;
use spindle_core::trust::TrustPolicy;

let policy = TrustPolicy::new(0.5)
    .with_trust("agent:trusted", 0.9)
    .with_trust("agent:untrusted", 0.3);

// Filter: only conclusions from agent: sources with trust >= 0.7
let filter = TrustFilter::new()
    .with_min_degree(0.7)
    .with_source("agent:")
    .with_policy(policy);

// Check if a specific rule's conclusion passes the filter
let passes = filter.passes(&theory, Some("rule_label"));
```

### Filter Fields

| Field | Description |
|---|---|
| `min_degree` | Minimum trust degree for a conclusion to pass |
| `source_pattern` | Substring match on the rule's source metadata |
| `policy` | Trust policy used to look up source trust values |

When no policy is set, all conclusions pass the filter (permissive by default).

## Mining Confidence Metrics

When rules are learned from process mining, each rule can be annotated with support and confidence metrics.

### `LearnedRule`

```rust
use spindle_core::mining::{LearnedRule, calculate_support, calculate_confidence};

// Calculate support: number of traces where "submit" directly precedes "review"
let support = calculate_support(&event_log, "submit", "review");

// Calculate confidence: support / total transitions from "submit"
let confidence = calculate_confidence(&event_log, "submit", "review");
```

### Filtering by Metrics

```rust
use spindle_core::mining::rules_with_metrics;

// Get only rules with support >= 5 and confidence >= 0.8
let learned = rules_with_metrics(&event_log, &mined_rules, 5, 0.8);

for lr in &learned {
    println!("{}", lr); // "r1 (support: 10, confidence: 0.95)"
}
```

### Petri Net Mining with Metrics

```rust
use spindle_core::mining::petri_net_to_rules;

// Mine rules with minimum support=3, confidence=0.7
let learned_rules = petri_net_to_rules(&event_log, 3, 0.7);

for lr in &learned_rules {
    println!("{}: support={}, confidence={:.2}, source={}",
        lr.rule.label, lr.support, lr.confidence, lr.source);
}
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

### End-to-End Example

This example demonstrates the complete trust workflow from theory definition through CLI output.

**1. Define a theory with trust directives** (`review.spl`):

```lisp
; Trust configuration
(trusts agent:security 0.95)
(trusts agent:coder 0.85)
(trusts agent:qa 0.80)
(decays agent:qa exponential 86400.0)
(threshold deploy 0.8)
(threshold warn 0.5)

; Security agent's findings
(claims agent:security
  :at "2026-02-01T10:00:00Z"
  (given no_vulnerabilities)
  (normally sec1 no_vulnerabilities security_clear))

; Coder agent's results
(claims agent:coder
  :at "2026-02-01T10:30:00Z"
  (given tests_pass)
  (given lint_clean)
  (normally dev1 (and tests_pass lint_clean) code_ready))

; QA agent's assessment
(claims agent:qa
  :at "2026-02-01T11:00:00Z"
  (given manual_review_ok)
  (normally qa1 manual_review_ok qa_approved))

; Deployment rule: all three must agree
(normally deploy1
  (and security_clear code_ready qa_approved)
  ready_to_deploy)
```

**2. Run with trust output**:

```bash
spindle reason --trust review.spl
```

**3. Output**:

```
Conclusions:

  +D no_vulnerabilities (trust: 0.95) [agent:security]
  +D tests_pass (trust: 0.85) [agent:coder]
  +D lint_clean (trust: 0.85) [agent:coder]
  +D manual_review_ok (trust: 0.80) [agent:qa]
  +d security_clear (trust: 0.95) [agent:security]
  +d code_ready (trust: 0.85) [agent:coder]
  +d qa_approved (trust: 0.80) [agent:qa]
  +d ready_to_deploy (trust: 0.80)
```

The deployment conclusion (`ready_to_deploy`) has trust 0.80 — it meets the `deploy` threshold (0.8) and can proceed.

## Limitations

1. **Static trust values**: Trust values are fixed per policy. Dynamic trust that updates based on track record is not built in (use decay models for time-based adjustment).
2. **Weakest-link only**: The model uses minimum trust propagation. Alternative models (e.g., weighted average, product) are not supported.
3. **No cryptographic verification**: The `:sig` metadata field is stored but not verified against any cryptographic infrastructure.
4. **Floating-point precision**: Trust values are `f64`, so standard floating-point precision considerations apply to boundary comparisons.
5. **Decay requires reference time**: Decay models need the age of assertions to be computed externally; the pipeline does not automatically track assertion timestamps for decay purposes.
