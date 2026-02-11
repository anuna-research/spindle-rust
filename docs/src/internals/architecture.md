# Architecture

This document describes the internal architecture of Spindle-Rust.

## Crate Structure

```
spindle-rust/
├── crates/
│   ├── spindle-core/     # Core reasoning engine
│   ├── spindle-parser/   # DFL and SPL parsers
│   ├── spindle-cli/      # Command-line interface
│   └── spindle-wasm/     # WebAssembly bindings
```

## spindle-core

The core reasoning engine with these modules:

```
spindle-core/src/
├── lib.rs              # Crate root, prelude
├── intern.rs           # String interning
├── literal.rs          # Literal representation
├── mode.rs             # Modal operators
├── temporal.rs         # Temporal reasoning
├── rule.rs             # Rule types
├── superiority.rs      # Superiority relations
├── theory.rs           # Theory container
├── conclusion.rs       # Conclusion types
├── index.rs            # Theory indexing
├── reason.rs           # Standard DL(d) algorithm
├── grounding.rs        # Variable grounding
├── worklist.rs         # Worklist data structures
├── explanation.rs      # Proof trees
├── trust.rs            # Trust-weighted reasoning
├── query.rs            # Query operators
├── mining.rs           # Process mining (alpha algorithm)
└── error.rs            # Error types
```

## Data Flow

```
Input (DFL/SPL)
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
┌─────────────┐
│  Grounding  │  Instantiate variables
└─────────────┘
      │
      ▼
┌─────────────┐
│   Index     │  Build lookup tables
└─────────────┘
      │
      ▼
┌─────────────┐
│  Reasoning  │  DL(d)
└─────────────┘
      │
      ▼
┌─────────────┐
│ Conclusions │  +D, -D, +d, -d
└─────────────┘
```

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

### Standard DL(d) (reason.rs)

```
Phase 1: Initialize with facts
  ├── Add to definite_proven
  ├── Add to defeasible_proven
  └── Add to worklist

Phase 2: Forward chaining
  while worklist not empty:
    ├── Pop literal
    ├── Find rules with literal in body
    ├── Decrement body counters
    └── If counter = 0:
        ├── Strict → add +D, +d
        ├── Defeasible → check blocking, add +d
        └── Defeater → (no conclusion)

Phase 3: Negative conclusions
  for each literal:
    ├── If not in definite_proven → -D
    └── If not in defeasible_proven → -d
```

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

### Adding a New Rule Type

1. Add variant to `RuleType` enum
2. Update parsers (dfl.rs, spl.rs)
3. Update reasoning logic (reason.rs)
4. Add tests

### Adding a New Query Operator

1. Add function to `query.rs`
2. Implement the algorithm
3. Add to WASM bindings if needed
4. Document in guides/queries.md

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
├── Unit tests (per module)
│   ├── reason.rs - 40+ tests
│   └── ...
├── Integration tests
│   └── End-to-end CLI/API behavior
├── Parser tests
│   ├── dfl.rs
│   └── spl.rs
└── WASM tests
    └── Basic functionality
```

### Key Test Categories

1. **Basic reasoning**: facts, rules, chains
2. **Conflict resolution**: superiority, defeaters
3. **Edge cases**: cycles, empty, self-reference
4. **Stress tests**: long chains, wide theories

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
