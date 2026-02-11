# Feature Comparison: spindle-rust vs spindle-racket

Both are v1.7.0 implementations of defeasible logic reasoning with the same core semantics.

## Core Reasoning

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| Standard DL(d) forward chaining | ✅ | ✅ |
| Scalable DL(d\|\|) three-phase closure | ✅ | ✅ |
| Auto mode selection | ❌ | ✅ (`'auto`) |
| Rule types (fact/strict/defeasible/defeater) | ✅ | ✅ |
| Provability levels (+D/-D/+d/-d) | ✅ | ✅ |
| Superiority relations | ✅ | ✅ |
| Cycle detection | ❌ | ✅ |
| Ambiguity blocking | Scalable only (reason.rs is credulous) | ✅ |

## Parsing

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| DFL format | ✅ | ✅ |
| SPL format | ✅ | ✅ |
| `#lang spindle` (native lang module) | N/A | ✅ (Racket language) |
| DFL `@import` with namespaces | ❌ | ✅ |
| DFL `@prefix` declarations | ❌ | ✅ |
| DFL front-matter annotations (`---`) | ❌ | ✅ |
| Predicate arguments (`p(?x, ?y)`) | SPL only | Both DFL and SPL |
| Modal operators in parser | SPL only | Both DFL and SPL |
| Temporal syntax in parser | SPL only | Both DFL and SPL |
| Block comments | ❌ | ✅ |

## Temporal Reasoning

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| Allen Interval Algebra (13 relations) | ✅ | ✅ |
| Temporal literals with bounds | ✅ | ✅ |
| "As-of" / `reason-at` queries | ✅ (`--at` flag) | ✅ (`reason-at`) |
| ISO-8601 timestamp parsing | ✅ | ✅ |
| Temporal variable propagation | ❌ | ✅ |

## Modal / Deontic Logic

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| Obligation `[O]` | ✅ | ✅ |
| Permission `[P]` | ✅ | ✅ |
| Prohibition `[F]` | ✅ | ✅ |
| Custom modalities | ✅ (`Mode::new`) | ❌ |
| Negated modalities `[-O]` | ✅ | ✅ |

## Grounding & Variables

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| `?`-prefixed variables | ✅ | ✅ |
| Bottom-up Datalog grounding | ✅ | ✅ |
| Fixpoint iteration | ✅ | ✅ |
| Wildcard `_` rewriting | ✅ | ❌ |
| Range restriction validation | ✅ | ❌ |
| Configurable iteration/instance limits | ✅ (100 iter / 10k instances) | ❌ |

## Query & Explanation

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| Basic query (provable/refuted/unknown) | ✅ | ✅ |
| `what-if` hypothetical reasoning | ✅ | ✅ |
| `why-not` failure explanation | ⚠️ (misses defeater blocking) | ✅ |
| Abduction (`abduce` / `requires`) | ⚠️ (no solution verification) | ✅ |
| Proof tree generation | ✅ | ✅ |
| Natural language explanations | ✅ | ✅ |
| JSON output | ✅ (versioned schemas) | ✅ (JSON-LD) |
| GraphViz DOT output | ❌ | ✅ |
| `diff` (compare conclusion sets) | ❌ | ✅ |
| `watch` (file monitoring + re-reason) | ❌ | ✅ |
| `export` command | ❌ | ✅ |

## Trust-Weighted Reasoning

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| Source attribution | ✅ | ✅ |
| Trust policies (`trusts`, `decays`, `threshold`) | ❌ | ✅ |
| Diminisher support | ✅ | ✅ |
| Weakest-link degree computation | ❌ | ✅ |
| Trust-aware explanations | ⚠️ Partial | ✅ |
| Claims blocks with `:sig` | ✅ (canonical signing workflow) | ✅ |

## Process Mining

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| Event log structures | ✅ | ✅ |
| Alpha algorithm (Petri net discovery) | ✅ | ✅ |
| Footprint matrix | ✅ | ✅ |
| DFL rule conversion from mined models | ✅ | ✅ |
| Confidence/support calculation | ❌ | ✅ |

## Modular Theory Composition

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| `@import` with path resolution | ❌ | ✅ |
| Namespace prefixing | ❌ | ✅ |
| Circular import detection | N/A | ✅ |
| Nested multi-level imports | ❌ | ✅ |
| Conclusion injection from imports | ❌ | ✅ |

## Output & Integration

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| JSON output | ✅ (spindle.*.v1 schemas) | ✅ (JSON-LD) |
| Linked-data / RDF vocabulary | ❌ | ✅ (Dublin Core, PROV, RDFS) |
| GraphViz visualization | ❌ | ✅ |
| Structured exit codes | ✅ (2/3/4) | ✅ |

## Platform & Deployment

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| Native CLI binary | ✅ (clap 4) | ✅ (Racket script) |
| WebAssembly (browser/Node.js) | ✅ (wasm-bindgen) | ❌ |
| IDE integration | ❌ | ✅ (DrRacket via `#lang spindle`) |
| REPL | ❌ | ✅ (Racket REPL) |

## Performance & Internals

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| String interning (4-byte SymbolId) | ✅ | ❌ (Racket symbols) |
| BitSet for O(1) conclusion lookup | ✅ | ❌ |
| Indexed theory for O(1) rule lookup | ✅ | ✅ |
| Memoization caching | ❌ | ✅ (explicit cache control) |

## Testing & Verification

| Feature | spindle-rust | spindle-racket |
|---|---|---|
| Test cases | 830+ | 800+ |
| Proptest (random theory generation) | ✅ (500 cases) | ❌ |
| Differential testing (standard vs scalable) | ✅ | ✅ (semantic equivalence) |
| Lean 4 formal verification | ✅ (in progress) | ❌ |
| Lean oracle differential testing | ✅ | ❌ |
| Performance/stress tests | ❌ | ✅ |

## Summary

### spindle-rust advantages

- WebAssembly target for browser/Node.js deployment
- String interning and bitset performance optimizations
- Formal verification via Lean 4 (in progress)
- Proptest fuzzing with 500 random cases
- Canonical signing workflow for claims blocks
- Configurable grounding limits (iteration + instance caps)
- Wildcard `_` rewriting and range restriction validation
- Custom modal operators

### spindle-racket advantages

- Modular theory composition (`@import` with namespaces)
- Trust policies (decay models, thresholds, weakest-link)
- GraphViz visualization of proof trees
- JSON-LD linked-data output with RDF vocabularies
- `#lang spindle` IDE integration with DrRacket
- Watch mode for live re-reasoning on file changes
- Auto reasoning mode selection
- Cycle detection in rule dependencies
- More complete DFL parser (modals, temporal, imports, block comments)
- Conclusion diffing and export commands
- Temporal variable propagation
- Confidence/support calculation in process mining
