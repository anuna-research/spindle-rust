# Architecture

This document describes the internal architecture of Spindle-Rust.

## Crate Structure

```
spindle-rust/
├── crates/
│   ├── spindle-core/     # Core reasoning engine
│   ├── spindle-parser/   # SPL parser
│   ├── spindle-cli/      # Command-line interface
│   └── spindle-wasm/     # WebAssembly bindings
```

## spindle-core

The core reasoning engine with these modules:

```
spindle-core/src/
├── lib.rs              # Crate root, prelude
├── intern.rs           # String interning (SymbolId, LiteralId)
├── literal.rs          # Literal representation
├── mode.rs             # Modal operators
├── temporal.rs         # Temporal reasoning (Allen relations, TimePoint)
├── term.rs             # Terms (Symbol, Numeric: Integer/Decimal/Float)
├── rule.rs             # Rule types
├── body.rs             # Body literals (BodyLogicLiteral, BodyArg, ArithConstraint)
├── superiority.rs      # Superiority relations
├── theory.rs           # Theory container
├── conclusion.rs       # Conclusion types (+D, -D, +d, -d)
├── claims.rs           # Claims blocks (provenance metadata)
├── index.rs            # Theory indexing (AtomKey, LitId)
├── arith.rs            # Arithmetic expressions and constraints
├── grounding.rs        # Variable grounding (bottom-up Datalog)
├── worklist.rs         # Worklist data structures
├── trust.rs            # Trust-weighted reasoning
├── mining.rs           # Process mining (alpha algorithm)
├── error.rs            # Error types
├── analysis/           # Theory analysis
│   ├── mod.rs          #   Shared types (ValidationDiagnostic, ConflictReport)
│   ├── conflicts.rs    #   Conflict detection
│   ├── superiority.rs  #   Superiority suggestion
│   └── validation.rs   #   Semantic validation
├── explanation/        # Explanation system
│   ├── mod.rs          #   Proof/derivation tree structures
│   ├── types.rs        #   Core explanation types
│   └── format/         #   Output formats
│       ├── mod.rs
│       ├── natural_language.rs
│       ├── json.rs
│       ├── jsonld.rs
│       └── dot.rs
├── pipeline/           # Composable preparation pipeline
│   ├── mod.rs          #   PipelineStage trait, PipelineBuilder, prepare()
│   ├── validate.rs     #   Validate stage
│   ├── wildcard.rs     #   WildcardRewrite stage
│   ├── ground.rs       #   Ground stage
│   └── temporal.rs     #   TemporalFilter, TemporalVarValidation stages
├── query/              # Query operators
│   ├── mod.rs          #   QueryOperator trait
│   ├── what_if.rs      #   Hypothetical reasoning
│   ├── why_not.rs      #   Failure explanation
│   ├── abduce.rs       #   Abduction (hypothesis finding)
│   └── requires.rs     #   Dependency queries
└── reason/             # Reasoning engine
    ├── mod.rs          #   Reasoner trait, StandardReasoner
    ├── state.rs        #   ReasoningState (worklist, proven sets, counters)
    ├── facts.rs        #   Fact initialization phase
    ├── definite.rs     #   Definite (+D/-D) forward chaining
    └── defeasible.rs   #   Defeasible (+d/-d) forward chaining
```

## Data Flow

```
Input (SPL)
      │
      ▼
┌─────────────┐
│   Parser    │  spindle-parser
└─────────────┘
      │
      ▼
┌─────────────┐
│   Theory    │  Rules, facts, superiority
└─────────────┘
      │
      ▼
┌─────────────────────────────────────────┐
│  Pipeline  (composable stages)          │
│  ┌──────────────────┐                   │
│  │ TemporalFilter   │ (optional as-of)  │
│  ├──────────────────┤                   │
│  │ Validate         │ range restriction │
│  ├──────────────────┤                   │
│  │ WildcardRewrite  │ _ → ?_wN          │
│  ├──────────────────┤                   │
│  │ Ground           │ variable inst.    │
│  ├──────────────────┤                   │
│  │ TemporalVarValid.│ reject unresolved │
│  ├──────────────────┤                   │
│  │ TemporalFilter   │ (optional re-filt)│
│  └──────────────────┘                   │
└─────────────────────────────────────────┘
      │
      ▼
┌─────────────┐
│   Index     │  Build lookup tables (AtomKey, LitId)
└─────────────┘
      │
      ▼
┌─────────────┐
│  Reasoning  │  StandardReasoner — DL(d)
└─────────────┘
      │
      ▼
┌─────────────┐
│ Conclusions │  +D, -D, +d, -d
└─────────────┘
```

## Pipeline Architecture

The preparation pipeline transforms a raw `Theory` into a form ready for
reasoning. It is built from composable stages, each implementing the
`PipelineStage` trait.

### PipelineStage Trait

```rust
pub trait PipelineStage: std::fmt::Debug {
    /// Human-readable name used in diagnostics and tracing.
    fn name(&self) -> &'static str;

    /// Apply this stage, returning a (possibly transformed) theory.
    /// Returning Err aborts the pipeline.
    fn apply(&self, theory: Theory, ctx: &mut PipelineContext) -> Result<Theory>;
}
```

Stages are assembled with `PipelineBuilder` and run left-to-right:

```rust
use spindle_core::pipeline::{Pipeline, Validate, WildcardRewrite, Ground};

let pipeline = Pipeline::builder()
    .stage(Validate::default())
    .stage(WildcardRewrite)
    .stage(Ground::default())
    .build();

let (prepared_theory, ctx) = pipeline.run(theory)?;
```

### Built-in Stages

| Stage | Module | Purpose |
|-------|--------|---------|
| `Validate` | `pipeline/validate.rs` | Enforces range restriction (head vars must appear in body) and rejects wildcards in rule heads. Both checks are independently configurable. |
| `WildcardRewrite` | `pipeline/wildcard.rs` | Rewrites anonymous wildcards (`_`) to unique fresh variables (`?_wN`) so each position is distinct during grounding. |
| `Ground` | `pipeline/ground.rs` | Bottom-up Datalog grounding. Instantiates rules containing variables via fixpoint iteration over facts, with configurable `max_iterations` and `max_instances` limits. |
| `TemporalFilter` | `pipeline/temporal.rs` | Removes rules/facts not active at a given reference `TimePoint` ("as-of" semantics). Omitted by default since it requires a reference time. |
| `TemporalVarValidation` | `pipeline/temporal.rs` | Rejects theories with unresolved temporal variables after grounding. Runs in strict mode by default (returns `Err`); can be set to warning-only. |

### PipelineContext

A `PipelineContext` is threaded through all stages, carrying:

- **`diagnostics: Vec<Diagnostic>`** — Info, Warning, and Error messages
  emitted by stages. Each diagnostic records the stage name and a
  human-readable message.
- **`metadata: HashMap<String, MetadataVal>`** — Arbitrary key-value data
  for inter-stage communication (e.g., `grounding_instances`,
  `grounding_limit_hit`, `evaluated_at`).

### Default Pipeline

`Pipeline::default_pipeline()` returns `Validate → WildcardRewrite → Ground`.
The higher-level `prepare()` function builds a richer pipeline from
`PrepareOptions`, optionally inserting `TemporalFilter` (before and after
grounding) and `TemporalVarValidation`.

## Key Design Decisions

### String Interning

All literal names are interned to reduce memory and enable O(1) comparisons.

```rust
// intern.rs
pub struct SymbolId(u32);   // Interned string ID
pub struct LiteralId(u32);  // Interned literal (name + negation)

// Convert string to ID (only allocates once per unique string)
let id = intern("bird");

// Compare IDs (O(1), no string comparison)
if id1 == id2 { ... }

// Resolve back to string when needed
let name = resolve(id);
```

**Benefits:**
- HashSet`LiteralId` uses 4 bytes per entry vs ~24+ for String
- No heap allocation in hot reasoning loops
- O(1) equality checks

### Indexed Theory

Rules are indexed by head and body literals for O(1) lookup.

```rust
// index.rs
pub struct IndexedTheory {
    theory: Theory,
    by_head: HashMap<LiteralId, Vec<&Rule>>,
    by_body: HashMap<LiteralId, Vec<&Rule>>,
}

// O(1) lookup instead of O(n) scan
indexed.rules_with_head(&literal)
indexed.rules_with_body(&literal)
```

### LiteralId Design

A `LiteralId` packs symbol ID and negation into a single u32:

```rust
pub struct LiteralId(u32);

impl LiteralId {
    // High bit = negation flag
    // Lower 31 bits = symbol ID

    pub fn is_negated(&self) -> bool {
        (self.0 & 0x8000_0000) != 0
    }

    pub fn symbol(&self) -> SymbolId {
        SymbolId(self.0 & 0x7FFF_FFFF)
    }

    pub fn complement(&self) -> LiteralId {
        LiteralId(self.0 ^ 0x8000_0000)
    }
}
```

This allows:
- O(1) negation check
- O(1) complement computation
- Efficient hash set storage

### Superiority Index

Superiority relations are indexed for O(1) lookup:

```rust
pub struct SuperiorityIndex {
    // superior -> set of inferior rules
    superiors: HashMap<RuleLabel, HashSet<RuleLabel>>,
}

// O(1) check
theory.is_superior("r1", "r2")
```

## Algorithm Implementation

### Reasoner Trait

The `Reasoner` trait abstracts over reasoning backends. One concrete
implementation is provided:

- **`StandardReasoner`** — the standard DL(d) forward-chaining algorithm
  (the default and only built-in backend).

Use `select_reasoner("standard")` to obtain a boxed trait object for
runtime backend selection. Additional backends can be added by
implementing the `Reasoner` trait.

### Standard DL(d) (reason/)

The reasoning state (worklist, proven sets, body counters, conclusions) is
consolidated into `ReasoningState` in `reason/state.rs`. The algorithm
proceeds in three phases, each in its own submodule:

```
Phase 1: Fact initialization (reason/facts.rs)
  ├── Add to definite_proven
  ├── Add to defeasible_proven
  └── Add to worklist

Phase 2: Definite forward chaining (reason/definite.rs)
  while worklist not empty:
    ├── Pop literal
    ├── Find rules with literal in body
    ├── Decrement body counters
    └── If counter = 0:
        ├── Strict → add +D, +d
        ├── Defeasible → (handled in phase 3)
        └── Defeater → (no conclusion)

Phase 3: Defeasible forward chaining (reason/defeasible.rs)
  for defeasible rules with all body literals proven:
    ├── Check ambiguity blocking (complement, team defeat)
    ├── Check superiority relations
    └── If unblocked → add +d

Phase 4: Negative conclusions
  for each literal:
    ├── If not in definite_proven → -D
    └── If not in defeasible_proven → -d
```

Uses `LitId` (4-byte Copy type) and BitSet for O(1) proven literal checks,
eliminating heap allocations and hash computations in the hot reasoning loop.

## Memory Layout

### Theory

```rust
pub struct Theory {
    rules: Vec<Rule>,
    superiorities: Vec<Superiority>,
    meta: HashMap<String, Meta>,
}

pub struct Rule {
    label: String,
    rule_type: RuleType,     // 1 byte + padding
    body: SmallVec<[Literal; 4]>,  // inline for ≤4 elements
    head: SmallVec<[Literal; 4]>,
}

pub struct Literal {
    name: String,            // 24 bytes (String)
    negation: bool,          // 1 byte + padding
    mode: Option<Mode>,      // 1-2 bytes + padding
    temporal: Option<...>,   // Variable
    predicates: Vec<String>, // 24 bytes (Vec)
}
```

### Optimized Representation (reasoning)

```rust
// During reasoning, we use IDs instead of strings
proven: BitSet  // bitmap indexed by LiteralId
worklist: VecDeque`LiteralId`
```

## Extension Points

### Adding a Pipeline Stage

1. Implement the `PipelineStage` trait
2. Add the struct to `pipeline/` (new file or existing)
3. Re-export from `pipeline/mod.rs`
4. Insert into pipelines with `PipelineBuilder::stage()` or `stage_at()`

### Adding a New Rule Type

1. Add variant to `RuleType` enum
2. Update parser (spl.rs)
3. Update reasoning logic in `reason/`
4. Add tests

### Adding a New Query Operator

1. Implement the `QueryOperator` trait in a new file under `query/`
2. Re-export from `query/mod.rs`
3. Add to WASM bindings if needed
4. Document in guides/queries.md

### Adding a Reasoning Backend

1. Implement the `Reasoner` trait
2. Register in `select_reasoner()` for runtime dispatch
3. Add tests

### Process Mining Pipeline

The mining module provides:
- Event log → Footprint matrix → Petri net discovery (alpha algorithm)
- Conflict detection (choice/mutex patterns)
- Rule learning with support/confidence metrics

### Adding Temporal Features

1. Extend `temporal.rs`
2. Update literal matching in `grounding.rs`
3. Update parsers for new syntax

## Testing Strategy

```
tests/
├── Unit tests (per module, inline #[cfg(test)])
│   ├── reason/ - phase-isolated tests
│   ├── pipeline/ - per-stage tests
│   ├── query/ - operator tests
│   └── ...
├── Integration tests (crates/spindle-core/tests/)
│   ├── difftest.rs - proptest random theory gen (500 cases)
│   └── ...
├── Parser tests
│   └── spl.rs
└── WASM tests
    └── Basic functionality
```

### Key Test Categories

1. **Basic reasoning**: facts, rules, chains
2. **Conflict resolution**: superiority, defeaters
3. **Edge cases**: cycles, empty, self-reference
4. **Pipeline stages**: validation, grounding, temporal filtering
5. **Differential testing**: proptest-generated random theories

## Performance Characteristics

| Operation | Complexity |
|-----------|------------|
| Add fact | O(1) amortized |
| Add rule | O(1) amortized |
| Build index | O(n) where n = rules |
| Lookup by head | O(1) |
| Lookup by body | O(1) |
| Superiority check | O(1) |
| Standard reasoning | O(n * m) |

Where:
- n = number of rules
- m = average body size

## Dependencies

Core dependencies:
- `rustc-hash` - Fast hash function for internal HashMaps
- `smallvec` - Inline storage for small vectors (rule body/head)

Parser dependencies:
- `nom` - Parser combinator library

WASM dependencies:
- `wasm-bindgen` - Rust/JavaScript interop
- `serde` - JSON serialization
