# Performance Tuning

This guide covers performance optimization for Spindle.

## Choosing an Algorithm

### Standard DL(d)

Best for:
- Small to medium theories (<1000 rules)
- Simple conflict patterns
- Low memory environments

```bash
spindle theory.dfl
```

### Scalable DL(d||)

Best for:
- Large theories (>1000 rules)
- Complex conflict resolution
- Long inference chains

```bash
spindle --scalable theory.dfl
```

### Benchmark Your Theory

```bash
# Time both algorithms
time spindle theory.dfl > /dev/null
time spindle --scalable theory.dfl > /dev/null
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
spindle stats theory.dfl
```

Look for:
- High rule count → consider `--scalable`
- Many defeaters → potential blocking overhead
- Complex bodies → grounding overhead

### Rust Profiling

```bash
# Build with profiling
cargo build --release

# Profile with flamegraph
cargo flamegraph -- spindle theory.dfl
```

### Memory Profiling

```bash
# Use heaptrack
heaptrack spindle large-theory.dfl
heaptrack_print heaptrack.spindle.*.gz
```

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
- `scalable_vs_standard` - algorithm comparison

## Performance Checklist

1. **Choose the right algorithm**
   - [ ] Small theory → standard
   - [ ] Large theory → `--scalable`

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
   - [ ] Benchmark algorithm choice
