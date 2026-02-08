# SpindleLean

Formal verification of the Spindle defeasible logic reasoning engine in Lean 4.

This project provides a Lean 4 model of the DL(d||) three-phase closure algorithm
implemented in `spindle-core`, along with correctness proofs and a JSON oracle
for differential testing against the Rust implementation.

## Requirements

- [elan](https://github.com/leanprover/elan) (Lean toolchain manager)
- Lean 4.27.0 (installed automatically via `lean-toolchain`)
- Mathlib 4.27.0 (fetched by Lake)

## Building

```bash
lake build          # Build library + executable (~480 jobs, first build fetches Mathlib)
lake exe spindlelean           # Run Tweety Triangle test
echo '...' | lake exe spindlelean --oracle  # Run JSON oracle mode
```

## Module Structure

### Core Types

| Module | Description |
|--------|-------------|
| `Basic.lean` | `Literal`, `Mode`, complement, `BEq`/`DecidableEq`/`LawfulBEq` instances |
| `Rule.lean` | `RuleType` (fact/strict/defeasible/defeater), `Rule`, `isDefinite`, `isProductive`, `bodySatisfied` |
| `Theory.lean` | `Theory` (rules + superiority), `addRule`, `addSuperiority`, `isSuperior`, `allLiterals` |

### Closures

| Module | Description |
|--------|-------------|
| `Closure/Delta.lean` | Delta closure: definite provability via facts and strict rules (+D) |
| `Closure/Lambda.lean` | Lambda closure: over-approximation including defeasible rules |
| `Closure/Partial.lean` | Partial closure: defeasible provability with conflict resolution (+d) |

All closures use fuel-based iteration (default 1000) with fixpoint detection via length check.
Step functions use `List.dedup` (from Mathlib) to maintain set semantics.

### Reasoning

| Module | Description |
|--------|-------------|
| `Reason.lean` | Top-level `reason` function: computes delta, lambda, partial closures and derives `+D`, `-D`, `+d`, `-d` conclusions |

### Properties (Proofs)

| Module | Sorry Count | Description |
|--------|-------------|-------------|
| `Soundness.lean` | 0 | Delta soundness: every `+D` literal has a supporting definite rule. Ambiguity blocking: conflicting rules with no superiority block both conclusions. |
| `Subset.lean` | 0 | Closure containment chain: delta ⊆ partial ⊆ lambda |
| `Acyclicity.lean` | 1 | Superiority acyclicity preserved through `addSuperiority` |
| `Termination.lean` | 5 | Step functions stay within theory universe; convergence bounds |
| `Confluence.lean` | 6 | Step function monotonicity and extensiveness; fixpoint stability and uniqueness |
| `Equivalence.lean` | 1 | Three-phase decomposition faithfully computes DL(d) semantics; soundness of `+D` and `+d` |
| `Faithfulness.lean` | 4 | Correspondence to paper DL(d) inference conditions; ambiguity blocking faithfulness (proven) |

### Differential Testing

| Module | Description |
|--------|-------------|
| `DiffTest/Oracle.lean` | JSON oracle: parses theory from stdin, runs reasoning, outputs conclusions as JSON |
| `Main.lean` | Executable entry point: `--oracle` flag for oracle mode, default runs Tweety Triangle test |

## Oracle Protocol

The oracle reads a JSON theory from stdin and writes JSON conclusions to stdout.

**Input format:**

```json
{
  "rules": [
    {"label": "f1", "type": "fact", "body": [], "head": {"name": "p", "negated": false}},
    {"label": "r1", "type": "defeasible",
     "body": [{"name": "p", "negated": false}],
     "head": {"name": "q", "negated": false}}
  ],
  "superiority": [["r2", "r1"]]
}
```

**Output format:**

```json
{
  "delta": [{"name": "p", "negated": false, "mode": null}],
  "lambda": [...],
  "partial": [...],
  "conclusions": [
    {"literal": {"name": "p", "negated": false, "mode": null}, "type": "+D"},
    {"literal": {"name": "p", "negated": false, "mode": null}, "type": "+d"}
  ]
}
```

## Rust Integration

The Lean oracle is tested against the Rust `reason_scalable()` implementation:

```bash
# From the workspace root
cargo test --package spindle-core --test difftest              # proptest (500 random theories)
cargo test --package spindle-core --test lean_oracle_difftest -- --ignored  # Lean oracle comparison
```

CI is configured in `.github/workflows/diff-test.yml`.

## Proof Status

**Fully proven (0 sorry):** Soundness, Subset chain

**Partially proven:** Acyclicity (1), Termination (5), Confluence (6), Equivalence (1), Faithfulness (4)

The remaining `sorry` placeholders are primarily in:
- Convergence bounds (requiring finiteness arguments over `allLiterals`)
- Monotonicity of `bodySatisfied` (requiring `List.contains` / `Bool` manipulation)
- Completeness directions (requiring fixpoint characterization)
