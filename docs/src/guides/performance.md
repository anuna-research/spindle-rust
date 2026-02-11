# Performance Tuning

This guide covers performance optimization for Spindle.

## Reasoning Engine

### Standard DL(d)

Spindle uses the standard DL(d) forward-chaining engine.

```bash
spindle reason theory.spl
```

### Benchmark Your Theory

```bash
# Time a representative run
time spindle reason theory.spl > /dev/null
```

## Theory Design

### Minimize Rule Bodies

Larger bodies = more matching work:

```lisp
; Slower: 5-way join
(normally r1 (and a b c d e) result)

; Faster: chain of 2-way joins
(normally r1 (and a b) ab)
(normally r2 (and ab c) abc)
(normally r3 (and abc d) abcd)
(normally r4 (and abcd e) result)
```

### Use Specific Predicates

```lisp
; Slower: matches all 'thing' facts
(normally r1 (thing ?x) (processed ?x))

; Faster: matches only relevant facts
(normally r1 (unprocessed-item ?x) (processed ?x))
```

### Reduce Grounding Explosion

Variables can cause combinatorial explosion:

```lisp
; If there are 100 'node' facts, this creates 10,000 ground rules
(normally r1 (and (node ?x) (node ?y)) (pair ?x ?y))

; Add constraints to reduce matching
(normally r1 (and (node ?x) (node ?y) (edge ?x ?y)) (connected ?x ?y))
```

## Memory Optimization

> **Note:** During reasoning, the proven literal set now uses a `BitSet` instead
> of `HashSet<LiteralId>`, providing O(1) membership tests with significantly
> better cache locality. See [Recent Optimizations](#recent-optimizations) for
> details.

### String Interning

Spindle uses string interning internally. You can benefit by:

```rust
// Reuse literal names
let bird = "bird";
theory.add_fact(bird);
theory.add_defeasible_rule(&[bird], "flies");
```

### Stream Large Results

For large result sets:

```rust
// Instead of collecting all conclusions
let conclusions: Vec<Conclusion> = theory.reason();

// Process incrementally (future API)
for conclusion in theory.reason_iter() {
    process(conclusion);
}
```

### Theory Cloning

Avoid unnecessary cloning:

```rust
// Expensive: clones the whole theory
let copy = theory.clone();

// Better: reuse the indexed theory
let indexed = IndexedTheory::build(theory);
// Use indexed for multiple queries
```

## Conflict Optimization

### Minimize Conflicts

Fewer conflicts = faster resolution:

```lisp
; Many potential conflicts
(normally r1 a result)
(normally r2 b ~result)
(normally r3 c result)
(normally r4 d ~result)

; Better: restructure to reduce conflicts
(normally supports a supports-result)
(normally supports c supports-result)
(normally opposes b opposes-result)
(normally opposes d opposes-result)
(normally conclusion supports-result result)
(normally conclusion opposes-result ~result)
(prefer conclusion ...)
```

### Explicit Superiority

Unresolved conflicts cause ambiguity propagation:

```lisp
; Bad: no superiority, causes ambiguity checks
(normally r1 a q)
(normally r2 b ~q)

; Good: explicit resolution
(normally r1 a q)
(normally r2 b ~q)
(prefer r1 r2)  ; or (prefer r2 r1)
```

## Indexing Benefits

Spindle indexes rules by:
- Head literal → O(1) lookup
- Body literal → O(1) lookup
- Superiority → O(1) lookup

The indexed theory is built once and reused:

```rust
let indexed = IndexedTheory::build(theory);

// Fast: O(1) lookups
for rule in indexed.rules_with_head(&literal) { ... }
for rule in indexed.rules_with_body(&literal) { ... }
```

## Profiling

### Theory Statistics

```bash
spindle stats theory.spl
```

Look for:
- High rule count → check grounding and rule-body fanout
- Many defeaters → potential blocking overhead
- Complex bodies → grounding overhead

### Rust Profiling

```bash
# Build with profiling
cargo build --release

# Profile with flamegraph
cargo flamegraph -- spindle reason theory.spl
```

### Memory Profiling

```bash
# Use heaptrack
heaptrack spindle reason large-theory.spl
heaptrack_print heaptrack.spindle.*.gz
```

## Recent Optimizations

Several targeted optimizations have been applied to Spindle's internals. These
are transparent to end users but can significantly reduce runtime and memory
overhead, especially for larger theories.

### SmallVec for Rule Bodies/Heads

*Commit [9bee7a3]*

Rule body and head storage was changed from `Vec<Literal>` to
`SmallVec<[Literal; 4]>`. This means that rules with four or fewer body/head
literals avoid heap allocation entirely -- the data is stored inline on the
stack.

Most real-world defeasible rules have 1-3 body literals, so the common case is
covered without any heap allocation. This reduces allocation pressure during
both grounding and reasoning.

```rust
// Before
struct Rule {
    head: Vec<Literal>,
    body: Vec<Literal>,
}

// After
use smallvec::SmallVec;
struct Rule {
    head: SmallVec<[Literal; 4]>,
    body: SmallVec<[Literal; 4]>,
}
```

### BitSet for Proven Literals

*Commit [82ae834]*

The set of proven literals during reasoning was changed from
`HashSet<LiteralId>` to a compact `BitSet`. Since `LiteralId` is a `u32`,
bitmap indexing is a natural fit:

- **O(1)** contains and insert operations (bit test / bit set).
- **Better cache locality** -- the entire proven set fits in a contiguous
  allocation rather than being scattered across hash buckets.
- **Significant improvement** for theories with many literals, where hash
  collisions and bucket traversal previously added up.

### FxHash for Internal HashMaps

*Commit [4fbe1fd]*

All internal `HashMap` / `HashSet` instances were switched from the default
`SipHash` hasher to `FxHash` (provided by the `rustc-hash` crate).

- `FxHash` is considerably faster for small keys (`u32`, `u64`) that are common
  throughout Spindle's indexing and reasoning structures.
- It is **not** cryptographically secure, but Spindle's internal maps are never
  exposed to untrusted input, so this is not a concern.
- This is the same hasher used inside the Rust compiler itself.

### Pre-allocation Strategies

*Commit [2e92b84]*

Worklists, intermediate buffers, and result vectors are now pre-allocated based
on the size of the theory before reasoning begins:

- The conclusion vector is allocated with an estimate of
  `rules.len() * avg_head_size`.
- Worklists are sized to the number of rules to avoid repeated reallocation
  during the fixed-point loop.

This avoids the cost of incremental `Vec` growth (repeated
allocate-copy-deallocate cycles) and reduces peak memory usage by avoiding
over-allocation.

## Benchmarks

Run the built-in benchmarks:

```bash
cd crates/spindle-core
cargo bench
```

Key benchmarks:
- `reason_penguin` - basic conflict resolution
- `reason_long_chain` - forward chaining depth
- `reason_wide` - parallel independent rules

## Performance Checklist

1. **Use explicit superiority in conflicts**
   - [ ] Conflicting defeasible rules are intentionally unresolved unless ordered
   - [ ] Add superiority relations where one side must win

2. **Optimize theory design**
   - [ ] Keep rule bodies small
   - [ ] Use specific predicates
   - [ ] Control grounding size

3. **Resolve conflicts**
   - [ ] Add superiority relations
   - [ ] Avoid unnecessary ambiguity

4. **Monitor resources**
   - [ ] Check `spindle stats` output
   - [ ] Profile if needed
   - [ ] Benchmark representative theories
