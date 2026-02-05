# Optimization Notes

This document records optimization attempts and findings for future reference.

## Lattice-Based Memoization (February 2026)

### Motivation

The `is_blocked_by_superior` function in `reason.rs` checks if a defeasible rule is blocked by attacking rules. The hypothesis was that:

1. Multiple rules deriving the same head would check the same attackers
2. Caching attacker status could avoid repeated O(body_size) checks
3. A lattice-based approach (inspired by Ascent) could formalize memoization

### Approaches Tried

#### 1. ProofStatus Lattice with Memoization

Created a `ProofStatus` enum representing lattice positions:

```rust
enum ProofStatus {
    Unknown,           // ⊥
    BlockedByDelta,    // Terminal false
    NotInLambda,       // Terminal false
    NoSupport,         // Pending
    HasSupport,        // Pending
    AllAttacksDefeated,// Ready to prove
    ProvedInPartial,   // ⊤
}
```

**Result**: Correct semantically, but added 5-15% overhead due to HashMap operations.

#### 2. BlockedByChecker with Conservative Invalidation

Cached active attackers per complement literal, clearing cache when `proven` set grew.

**Result**: Cache was cleared too often, providing no benefit.

#### 3. Incremental Invalidation

Tracked `pending_body_literals` for each cache entry, only invalidating when relevant literals were proven.

**Result**: Reduced unnecessary invalidation, but overhead still exceeded savings. Cloning the `proven` HashSet for snapshot tracking was expensive.

#### 4. Lazy Attacker Tracking

Tracked attacker activation incrementally (like rule body counters), avoiding body satisfaction checks entirely.

```rust
struct LazyAttackerTracker {
    attacker_remaining: FxHashMap<String, usize>,
    active_attackers: FxHashMap<LiteralId, Vec<String>>,
    attacker_heads: FxHashMap<String, LiteralId>,
}
```

**Result**: 10-80% slower due to HashMap operations and string allocations.

### Benchmark Results

```
multi_target_blocked (M targets × N defenders):
  5x10:   standard ~44µs,  lazy ~53µs (+20%),  memoized ~51µs (+16%)
  10x10:  standard ~88µs,  lazy ~100µs (+14%), memoized ~106µs (+20%)
  20x10:  standard ~164µs, lazy ~299µs (+82%), memoized ~228µs (+39%)
  10x20:  standard ~180µs, lazy ~215µs (+19%), memoized ~230µs (+28%)
```

### Why All Approaches Failed

The original `is_blocked_by_superior` is already well-optimized:

```rust
for attacker in attacking_rules {           // O(1) lookup via IndexedTheory
    let satisfied = attacker.body.iter()    // Small vector (~1-3 elements)
        .all(|b| proven.contains(&b));      // O(1) HashSet lookup
    // Superiority check is O(1) via SuperiorityIndex
}
```

Key factors:
1. **Operation is already cheap** - Small vectors, O(1) lookups
2. **Single-pass algorithm** - Each literal checked once, no reuse opportunity
3. **Cache overhead exceeds savings** - HashMap ops, allocations, tracking state

### When Memoization Would Help

| Scenario | Single-Pass | Would Benefit |
|----------|-------------|---------------|
| Batch reasoning | ❌ No reuse | N/A |
| Interactive queries | N/A | ✅ Repeated queries |
| Incremental reasoning | ❌ Rebuild | ✅ Cache across runs |
| Explanation generation | N/A | ✅ Re-queries same literals |
| What-if analysis | ❌ Fresh run | ✅ Partial cache hits |

### Existing Optimizations (Already Effective)

1. **LiteralId** - 4-byte interned identifier, O(1) comparison
2. **SuperiorityIndex** - O(1) superiority lookup
3. **IndexedTheory** - O(1) rule lookup by head/body
4. **HashSet\<LiteralId\>** - 4 bytes per entry vs ~24 for String

### Alternative Optimizations (Not Tried)

If future profiling shows `is_blocked_by_superior` as a bottleneck:

1. **Bit vectors** - Replace HashSet with bit vector for proven literals
2. **Arena allocation** - Pre-allocate rules in contiguous memory
3. **Rule ordering** - Process rules in topological order
4. **SIMD body checks** - Vectorize body satisfaction for large bodies

### Lessons Learned

1. **Profile before optimizing** - The target operation was already efficient
2. **Single-pass algorithms resist caching** - No repeated queries means no cache hits
3. **Cache overhead matters** - HashMap operations can exceed saved computation
4. **Simple code is often fastest** - Direct iteration beats fancy data structures

### Code Location

The experimental code is on branch `feature/lattice-proof-memoization`:

```
8af967a feat(lattice): add lattice-based proof status memoization
9dd60e0 feat(lattice): add BlockedByChecker for is_blocked_by_superior
33e9923 perf(lattice): implement incremental cache invalidation
241d5e9 bench: add memoization comparison benchmarks
270aa98 experiment: add reason_lazy with lazy attacker tracking
```

The lattice module (`lattice.rs`) and memoized/lazy functions remain available for reference but are not used in production.
