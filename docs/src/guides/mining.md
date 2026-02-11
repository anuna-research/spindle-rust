# Process Mining

Spindle includes a process mining module that discovers defeasible logic rules from event logs. It implements the Alpha algorithm for Petri net discovery, footprint matrix construction, conflict detection, and SPL rule extraction with support/confidence metrics.

> **Note**: Process mining is currently available through the Rust API only. It is not exposed via the CLI or the WebAssembly bindings.

## Overview

Process mining bridges the gap between observed behavior (event logs) and formal models (defeasible logic rules). The pipeline works as follows:

```
EventLog -> Footprint -> PetriNet -> LearnedRules
               |                         |
               v                         v
         Relation analysis       SPL rules with
         (causal, parallel,      support/confidence
          unrelated)             metrics
```

Given a set of recorded process executions (cases), Spindle can:

1. Analyze activity relationships via a **footprint matrix**
2. Discover a **Petri net** process model using the Alpha algorithm
3. Detect **conflicts** (choice and mutex patterns)
4. Extract **defeasible logic rules** with statistical support

## Event Logs

### Structure

An event log consists of **cases** (process executions), each containing a sequence of **events**.

- `Event` -- a single activity execution with a timestamp, activity name, variable bindings, optional actor, and annotations
- `Case` -- a complete process trace identified by a unique ID, containing events sorted by timestamp
- `EventLog` -- the collection of cases with optional metadata

### Creating Events

```rust
use spindle_core::mining::{Event, Case, EventLog};
use std::collections::HashMap;

// Simple event with timestamp, activity, and bindings
let mut bindings = HashMap::new();
bindings.insert("entity".to_string(), "order-42".to_string());

let event = Event::new("2026-01-17T10:00:00Z", "submitted", bindings);

// Event with an actor
let event = Event::new("2026-01-17T10:05:00Z", "reviewed", HashMap::new())
    .with_actor("alice");

// Event with annotations
let mut annotations = HashMap::new();
annotations.insert("priority".to_string(), "high".to_string());

let event = Event::new("2026-01-17T10:10:00Z", "approved", HashMap::new())
    .with_actor("bob")
    .with_annotations(annotations);
```

### Creating Cases and Logs

Events within a case are automatically sorted by timestamp:

```rust
use spindle_core::mining::{Event, Case, EventLog};
use std::collections::HashMap;

// Build a case from events
let events = vec![
    Event::new("2026-01-17T12:00:00Z", "complete", HashMap::new()),
    Event::new("2026-01-17T10:00:00Z", "start", HashMap::new()),
    Event::new("2026-01-17T11:00:00Z", "process", HashMap::new()),
];
let case = Case::new("case-1", events);

// Events are sorted: start, process, complete
assert_eq!(case.activities(), vec!["start", "process", "complete"]);

// Build a log from cases
let log = EventLog::new(vec![case]);
```

### Log Inspection

```rust
// All unique activities across the log
let activities = log.activities();  // HashSet<&str>

// Total number of events
let count = log.total_events();
```

### Helper Functions

For testing and quick prototyping, helper functions simplify log construction:

```rust
use spindle_core::mining::{make_sequential_trace, make_log_from_traces, make_repeated_log};

// Single sequential trace
let case = make_sequential_trace("case-1", &["start", "process", "end"]);

// Log from multiple trace patterns
let log = make_log_from_traces(&[
    &["start", "a", "b", "end"],
    &["start", "b", "a", "end"],
    &["start", "a", "b", "end"],
]);

// Log with n identical traces
let log = make_repeated_log(10, &["submit", "review", "approve"]);
```

## Footprint Matrix

The footprint matrix captures **directly-follows** relationships between activities. It is the foundation for the Alpha algorithm.

### Relations

Given two activities `a` and `b`, the footprint matrix assigns one of four relations:

| Relation | Symbol | Meaning |
|----------|--------|---------|
| `Causality` | `->` | `a` directly precedes `b` (but not `b` before `a`) |
| `Reverse` | `<-` | `b` directly precedes `a` (but not `a` before `b`) |
| `Parallel` | `\|\|` | Both orderings observed in the log |
| `Unrelated` | `#` | Never directly adjacent in any trace |

### Building a Footprint

```rust
use spindle_core::mining::{Footprint, EventLog, make_repeated_log, make_log_from_traces};

// Sequential pattern: a -> b -> c
let log = make_repeated_log(5, &["a", "b", "c"]);
let fp = Footprint::from_log(&log);

// Check relations
assert!(fp.is_causal("a", "b"));   // a -> b
assert!(fp.is_causal("b", "c"));   // b -> c
assert!(fp.is_unrelated("a", "c")); // a # c (never directly adjacent)

// Parallel pattern
let log = make_log_from_traces(&[
    &["a", "b"],
    &["b", "a"],
]);
let fp = Footprint::from_log(&log);
assert!(fp.is_parallel("a", "b")); // a || b
```

### Querying the Matrix

```rust
use spindle_core::mining::Relation;

// Get a specific relation
let rel = fp.relation("a", "b");
match rel {
    Relation::Causality => println!("a causes b"),
    Relation::Reverse   => println!("b causes a"),
    Relation::Parallel  => println!("a and b are concurrent"),
    Relation::Unrelated => println!("a and b are unrelated"),
}

// Bulk queries
let causal = fp.causal_pairs();     // Vec<(String, String)>
let parallel = fp.parallel_pairs(); // Vec<(String, String)>
```

### What the Matrix Reveals

The footprint matrix answers key questions about a process:

- **Sequencing**: Which activities always follow others? (Causality)
- **Concurrency**: Which activities can happen in either order? (Parallel)
- **Independence**: Which activities are never adjacent? (Unrelated)
- **Reverse flow**: Which activities are preceded by others? (Reverse)

## Alpha Algorithm and Petri Net Discovery

The Alpha algorithm transforms a footprint matrix into a Petri net, a formal model of the process.

### Petri Net Structure

A Petri net consists of:

- `Place` -- a passive element with an ID and label (represents conditions/states)
- `Transition` -- an active element representing an activity
- `Arc` -- a directed connection between a place and a transition (or vice versa), using `ArcNode::Place` and `ArcNode::Transition` variants

### Running the Alpha Miner

```rust
use spindle_core::mining::{AlphaMiner, EventLog, make_repeated_log};

let log = make_repeated_log(10, &["a", "b", "c"]);

let mut miner = AlphaMiner::new();
let net = miner.mine(&log);

// Inspect the discovered net
println!("Places: {}", net.places.len());
println!("Transitions: {}", net.transitions.len());
println!("Arcs: {}", net.arcs.len());

// Find a transition by activity name
if let Some(trans) = net.find_transition("b") {
    println!("Found transition: {} ({})", trans.id, trans.activity);
}

// All activities in the net
let activities = net.activities(); // HashSet<&str>
```

### How It Works

The Alpha algorithm performs these steps:

1. **Compute footprint** from the event log's directly-follows pairs
2. **Identify start/end activities** (first/last in each trace)
3. **Find maximal pairs** `(A, B)` where all activities in `A` causally lead to all activities in `B`, and both sets are internally unrelated
4. **Build the net**: create transitions for each activity, places for start/end and each maximal pair, and arcs connecting them

### Building a Petri Net Manually

```rust
use spindle_core::mining::{PetriNet, Place, Transition, Arc, ArcNode};

let mut net = PetriNet::new();

net.add_place(Place::new("p1", "start"));
net.add_place(Place::new("p2", "end"));

net.add_transition(Transition::new("t1", "submit"));
net.add_transition(Transition::new("t2", "approve"));

net.add_arc(Arc::new(
    ArcNode::Place("p1".to_string()),
    ArcNode::Transition("t1".to_string()),
));
net.add_arc(Arc::new(
    ArcNode::Transition("t1".to_string()),
    ArcNode::Place("p2".to_string()),
));
```

## Conflict Detection

Conflicts arise when activities are mutually exclusive or represent choices in the process.

### Conflict Types

| Type | Source | Meaning |
|------|--------|---------|
| `Choice` | Petri net structure | XOR-split: a place has multiple outgoing transitions (only one fires) |
| `Mutex` | Trace analysis | Two activities never co-occur in the same trace |

### Detecting Conflicts

```rust
use spindle_core::mining::{detect_conflicts, AlphaMiner, make_log_from_traces};

let log = make_log_from_traces(&[
    &["start", "a", "end"],
    &["start", "a", "end"],
    &["start", "b", "end"],
    &["start", "b", "end"],
]);

let mut miner = AlphaMiner::new();
let net = miner.mine(&log);
let conflicts = detect_conflicts(&log, &net);

for conflict in &conflicts {
    println!("Conflict: {:?}", conflict.activities);
    match conflict.conflict_type {
        spindle_core::mining::ConflictType::Choice => {
            println!("  Type: XOR choice (from Petri net structure)");
        }
        spindle_core::mining::ConflictType::Mutex => {
            println!("  Type: Mutex (never co-occur in traces)");
        }
    }
    // Evidence explains the source of the conflict
    if let Some(source) = conflict.evidence.get("source") {
        println!("  Evidence: {}", source);
    }
}
```

### Choice vs Mutex

**Choice** conflicts are structural -- they come from XOR-split points in the Petri net where a place has multiple outgoing transitions. Only one transition can fire.

**Mutex** conflicts are behavioral -- two activities are never observed together in the same trace, but the relationship is not already captured by a choice conflict. This can indicate implicit exclusion rules.

## Rule Learning

The module extracts defeasible logic rules from causal relationships in the event log, annotated with statistical metrics.

### Support and Confidence

- **Support**: the number of traces where `a` is directly followed by `b`
- **Confidence**: the ratio of `a -> b` transitions to all transitions from `a`

For example, if `a` appears 10 times as a non-final activity and `a -> b` occurs 8 times, the confidence is 0.8.

### Extracting Rules

```rust
use spindle_core::mining::{petri_net_to_rules, make_repeated_log};

let log = make_repeated_log(10, &["submit", "review", "approve"]);

// Extract rules with minimum support of 5 and confidence of 0.7
let rules = petri_net_to_rules(&log, 5, 0.7);

for lr in &rules {
    println!("Rule: {}", lr.rule.label);
    println!("  Body: {:?}", lr.rule.body.iter().map(|l| l.name()).collect::<Vec<_>>());
    println!("  Head: {:?}", lr.rule.head.iter().map(|l| l.name()).collect::<Vec<_>>());
    println!("  Support: {}", lr.support);
    println!("  Confidence: {:.2}", lr.confidence);
    println!("  Source: {}", lr.source);  // "mined"
}
```

Each `LearnedRule` contains:

- `rule` -- a defeasible logic `Rule` (type `Defeasible`, labeled `r_mined_N`)
- `support` -- trace count supporting the causal pair
- `confidence` -- ratio of supporting transitions
- `source` -- origin of the rule (defaults to `"mined"`)

### Filtering by Thresholds

Rules below the minimum support or confidence thresholds are excluded:

```rust
// Strict thresholds: only high-confidence rules
let strict_rules = petri_net_to_rules(&log, 10, 0.9);

// Relaxed thresholds: discover more patterns
let relaxed_rules = petri_net_to_rules(&log, 1, 0.0);
```

## Complete Mining Pipeline

The `mine_rules` function runs the entire pipeline in a single call.

### Usage

```rust
use spindle_core::mining::{mine_rules, make_log_from_traces};

let log = make_log_from_traces(&[
    &["start", "a", "b", "end"],
    &["start", "a", "b", "end"],
    &["start", "a", "c", "end"],
    &["start", "a", "b", "end"],
]);

let result = mine_rules(&log, 2, 0.5);
```

### MiningResult Structure

The result bundles all outputs from the pipeline:

```rust
// Learned SPL rules
for lr in &result.rules {
    println!("{}: support={}, confidence={:.2}", lr.rule.label, lr.support, lr.confidence);
}

// Detected conflicts
for c in &result.conflicts {
    println!("Conflict {:?}: {:?}", c.conflict_type, c.activities);
}

// Discovered Petri net
println!("Net: {} places, {} transitions, {} arcs",
    result.petri_net.places.len(),
    result.petri_net.transitions.len(),
    result.petri_net.arcs.len(),
);

// Footprint matrix
for (a, b) in result.footprint.causal_pairs() {
    println!("{} -> {}", a, b);
}

// Mining metadata
println!("Traces: {}", result.metadata.get("trace_count").unwrap());
println!("Events: {}", result.metadata.get("event_count").unwrap());
println!("Min support: {}", result.metadata.get("min_support").unwrap());
println!("Min confidence: {}", result.metadata.get("min_confidence").unwrap());
```

### Pipeline Steps

`mine_rules` performs the following steps internally:

1. Build the footprint matrix from the event log
2. Run the Alpha miner to discover the Petri net
3. Detect conflicts from the net structure and trace analysis
4. Extract SPL rules from causal pairs, filtered by support and confidence
5. Package everything into a `MiningResult` with metadata

## Use Cases

### Workflow Analysis

Discover the actual execution patterns from system logs:

```rust
use spindle_core::mining::{Event, Case, EventLog, mine_rules};
use std::collections::HashMap;

// Build log from real workflow events
let case1 = Case::new("ticket-101", vec![
    Event::new("2026-01-17T09:00:00Z", "opened", HashMap::new())
        .with_actor("user"),
    Event::new("2026-01-17T09:30:00Z", "triaged", HashMap::new())
        .with_actor("support"),
    Event::new("2026-01-17T10:00:00Z", "assigned", HashMap::new())
        .with_actor("manager"),
    Event::new("2026-01-17T14:00:00Z", "resolved", HashMap::new())
        .with_actor("engineer"),
]);

let case2 = Case::new("ticket-102", vec![
    Event::new("2026-01-17T10:00:00Z", "opened", HashMap::new())
        .with_actor("user"),
    Event::new("2026-01-17T10:15:00Z", "triaged", HashMap::new())
        .with_actor("support"),
    Event::new("2026-01-17T10:30:00Z", "assigned", HashMap::new())
        .with_actor("manager"),
    Event::new("2026-01-17T16:00:00Z", "resolved", HashMap::new())
        .with_actor("engineer"),
]);

let log = EventLog::new(vec![case1, case2]);
let result = mine_rules(&log, 1, 0.5);

// Discovered rules describe the standard ticket workflow
for lr in &result.rules {
    println!("{}: {} => {}  (support={}, confidence={:.0}%)",
        lr.rule.label,
        lr.rule.body.iter().map(|l| l.name()).collect::<Vec<_>>().join(", "),
        lr.rule.head.iter().map(|l| l.name()).collect::<Vec<_>>().join(", "),
        lr.support,
        lr.confidence * 100.0,
    );
}
```

### Compliance Checking

Identify process deviations by comparing mined rules against expected patterns:

```rust
use spindle_core::mining::{Footprint, make_log_from_traces};

let log = make_log_from_traces(&[
    &["submit", "review", "approve"],
    &["submit", "review", "approve"],
    &["submit", "approve"],           // Skipped review
    &["submit", "review", "approve"],
]);

let fp = Footprint::from_log(&log);

// Check if review always precedes approval
if fp.is_causal("review", "approve") {
    println!("Compliant: review always directly precedes approve");
} else {
    println!("Violation: review does not always precede approve");
}

// Check for unauthorized shortcuts
if fp.is_causal("submit", "approve") {
    println!("Warning: direct submit-to-approve path detected");
}
```

### Process Discovery

Combine mining with Spindle's reasoning engine to build executable rule sets:

```rust
use spindle_core::mining::{mine_rules, make_log_from_traces};

let log = make_log_from_traces(&[
    &["init", "process", "validate", "complete"],
    &["init", "process", "reject"],
    &["init", "process", "validate", "complete"],
    &["init", "skip", "complete"],
]);

let result = mine_rules(&log, 1, 0.0);

// The learned rules can be added to a theory for further reasoning
println!("Discovered {} rules from {} traces",
    result.rules.len(),
    result.metadata.get("trace_count").unwrap(),
);

// Conflicts reveal decision points in the process
for c in &result.conflicts {
    println!("Decision point: {:?} ({:?})", c.activities, c.conflict_type);
}
```

## Limitations

1. **Alpha algorithm scope**: The Alpha miner handles sequential, parallel, and choice patterns. It does not support loops, invisible transitions, or duplicate activities.
2. **Directly-follows only**: The footprint matrix considers only directly adjacent activities, not long-range dependencies.
3. **Timestamp ordering**: Events within a case are sorted lexicographically by timestamp string. Use ISO-8601 format to ensure correct ordering.
4. **Rust API only**: Process mining is not yet available through the CLI or WebAssembly bindings.
5. **No incremental mining**: The entire log must be provided upfront; streaming or incremental updates are not supported.
