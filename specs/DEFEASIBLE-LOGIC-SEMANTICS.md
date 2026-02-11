# Defeasible Logic — Formal Semantics

Reference specification for SPINdle's Standard Defeasible Logic (SDL) inference engine.

## Primitives

| Concept | Notation | DFL Syntax |
|---|---|---|
| Fact | — | `>> p` |
| Strict rule | `r: A₁, ..., Aₙ → q` | `r: A1, ..., An -> q` |
| Defeasible rule | `r: A₁, ..., Aₙ ⇒ q` | `r: A1, ..., An => q` |
| Defeater | `r: A₁, ..., Aₙ ~> q` | `r: A1, ..., An ~> q` |
| Superiority | `r₁ > r₂` | `r1 > r2` |
| Complement | `~q` | `-q` (if `q = p` then `~q = -p`; if `q = -p` then `~q = p`) |

## Rule Sets

For a literal `q`, define:

- **R\[q\]** — all rules with head `q`
- **Rs\[q\]** — all *strict* rules with head `q`
- **Rsd\[q\]** — all *strict or defeasible* rules with head `q`
- **R\[~q\]** — all rules with head `~q` (the complement)

A rule `r` is **applicable** at proof level X when every literal in its body has been proved `+X`.

A rule `r` is **discarded** at proof level X when at least one literal in its body has been proved `-X`.

## Proof Tags

A **proof** is a sequence `P = (P(1), P(2), ..., P(n))` where each `P(i)` is a tagged literal of one of the forms: `+D q`, `-D q`, `+d q`, or `-d q`.

---

## Definite Provability

### +D q (Definitely Provable)

`q` is a fact, **OR** there exists a strict rule `r ∈ Rs[q]` such that every literal in `body(r)` is `+D` proved.

```
+D q  iff
    q ∈ Facts
    OR  ∃r ∈ Rs[q] : ∀a ∈ body(r), +D a ∈ P
```

### -D q (Definitely NOT Provable)

`q` is not a fact, **AND** every strict rule for `q` has at least one body literal that is `-D`.

```
-D q  iff
    q ∉ Facts
    AND  ∀r ∈ Rs[q] : ∃a ∈ body(r), -D a ∈ P
```

---

## Defeasible Provability

### +d q (Defeasibly Provable)

Either `q` is already definitely provable, **OR** all three conditions hold:

1. There exists an applicable strict-or-defeasible rule for `q`
2. `~q` is not definitely provable (no monotonic override)
3. Every rule for `~q` is either inapplicable, or defeated by a superior applicable rule for `q`

```
+d q  iff
    +D q ∈ P
    OR  (
        (1)  ∃r ∈ Rsd[q] : ∀a ∈ body(r), +d a ∈ P          -- r is applicable
        AND
        (2)  -D ~q ∈ P                                        -- ~q not definitely proved
        AND
        (3)  ∀s ∈ R[~q]:                                      -- for EVERY contrary rule
                ∃a ∈ body(s), -d a ∈ P                         --   s is discarded
                OR                                              --   OR
                ∃t ∈ Rsd[q]:                                   --   there exists a rule t for q
                    (∀a ∈ body(t), +d a ∈ P)  AND  t > s       --     that is applicable AND superior
        )
```

**Key points:**
- Condition (3) quantifies over **all** rules for `~q` — including strict, defeasible, *and* defeaters
- Condition (1) only considers strict/defeasible rules for `q` — defeaters cannot *establish* a conclusion
- The defeating rule `t` in condition (3) must also be strict or defeasible (from `Rsd[q]`)

### -d q (Defeasibly NOT Provable)

The mirror/dual of `+d`:

```
-d q  iff
    -D q ∈ P
    AND  (
        (1)  ∀r ∈ Rsd[q] : ∃a ∈ body(r), -d a ∈ P           -- every rule for q is discarded
        OR
        (2)  +D ~q ∈ P                                        -- ~q is definitely proved
        OR
        (3)  ∃s ∈ R[~q]:                                      -- there exists a contrary rule s
                (∀a ∈ body(s), +d a ∈ P)                       --   that is applicable
                AND                                             --   AND
                ∀t ∈ Rsd[q]:                                   --   for every rule t for q
                    (∃a ∈ body(t), -d a ∈ P)  OR  ¬(t > s)    --     t is discarded OR t doesn't beat s
        )
```

---

## Worked Example

### Example 1: Conflicting Facts

```
f1: >> p
f2: >> -p
```

Both are facts. Facts are `+D` (definite axioms). No superiority relation defined.

**Phase 1 (Definite):** `+D p`, `+D -p` (both are facts).

**Evaluate `+d p`:**

- First disjunct: `+D p ∈ P`? Yes — but the second disjunct still requires `-D -p`.
- Condition (2): `-D -p` — `-p` IS `+D`, so this is **false** — **fail**

**Result:** `+d p` fails. By symmetry `+d -p` also fails.

```
+D p, +D -p
-d p, -d -p
```

Neither literal is defeasibly provable despite both being definite. **Consistency enforced by condition (2).**

### Example 2: Conflicting Defeasible Rules

```
>> a
r1: a => q
r2: a => -q
```

No superiority relation defined.

**Phase 1:** `+D a` (fact). No strict rules for `q` or `-q`, so `-D q`, `-D -q`.

**Evaluate `+d q`:**

- Condition (1): `r1` is applicable (body `a` is `+d`) — **pass**
- Condition (2): `-D -q` — yes — **pass**
- Condition (3): Check `r2 ∈ R[-q]`: is `r2` discarded? No (body `a` is `+d`). Is there `t ∈ Rsd[q]` with `t > r2`? `r1` is applicable but no `r1 > r2` — **fail**

**Result:** `+d q` fails. By symmetry `+d -q` also fails.

```
-d q
-d -q
```

**Ambiguity blocking:** neither conclusion is defeasibly provable.

---

## Ambiguity Blocking vs Propagation

The conditions above describe **Ambiguity Blocking** (the default). The difference matters when ambiguous literals appear in rule bodies.

### Ambiguity Blocking (Default)

When `p` and `~p` are both blocked (neither is `+d`), the ambiguity is **localized**:

- Downstream rules that don't depend on `p` proceed normally
- `p` is treated as `-d p`, so any rule with `p` in its body is discarded
- But `q` might still be provable via other rules that don't mention `p`

### Ambiguity Propagation

When `p` and `~p` are both blocked, the ambiguity **cascades**:

- `p` is treated as `-d p`, so any rule with `p` in its body is discarded
- Additionally, `q` inherits the ambiguity and is also blocked, even if other uncontested rules support it

| Aspect | Ambiguity Blocking | Ambiguity Propagation |
|---|---|---|
| `p` and `~p` both blocked | Localized | Cascading |
| Rule `r: p => q` where `p` is ambiguous | `r` discarded; `q` may still be provable via other rules | `r` discarded; `q` inherits ambiguity even with other support |
| Total conclusions | More (localized blocking) | Fewer (cascading blocking) |

---

## Conflict Literals

In SDL, conflict is purely **classical negation**:

- `~p = -p`
- `~(-p) = p`

No user-defined conflict sets. (User-defined conflict sets exist only in Modal Defeasible Logic via mode conflict relations.)

---

## Defeaters

Defeaters (`~>`) are asymmetric "veto-only" rules:

- They **count as attackers** in condition (3) of `+d` — they can block a conclusion
- They **cannot establish** conclusions — they never appear in `Rsd[q]` (condition 1)
- They **can participate** in superiority relations

---

## Conclusion Types

| Tag | Enum | Meaning |
|---|---|---|
| `+D` | `DEFINITE_PROVABLE` | Proved using only facts and strict rules |
| `-D` | `DEFINITE_NOT_PROVABLE` | Cannot be proved using facts and strict rules |
| `+d` | `DEFEASIBLY_PROVABLE` | Proved using defeasible reasoning |
| `-d` | `DEFEASIBLY_NOT_PROVABLE` | Cannot be proved even defeasibly |

## Rule Types

| Type | Symbol | Can establish conclusions? | Can attack conclusions? | Proof level |
|---|---|---|---|---|
| Fact | `>>` | Yes (definite + defeasible) | Yes | Definite |
| Strict | `->` | Yes (definite + defeasible) | Yes | Definite |
| Defeasible | `=>` | Yes (defeasible) | Yes | Defeasible |
| Defeater | `~>` | **No** | Yes | — |
| Superiority | `>` | — | — | — |

> **Design note:** Facts (`>>`) are treated as definite axioms (`+D`), matching the
> pseudocode below and SPINdle's Java implementation. This means conflicting facts
> `>> p` and `>> -p` both produce `+D`, but neither produces `+d` because condition (2)
> of `+d` requires `-D ~q`, which fails when both sides are `+D`.

---

## Fixed-Point Computation — Pseudocode

### Data Structures

```
// Per-literal tracking
struct LiteralState {
    definite:  Option<bool>,   // None = unknown, Some(true) = +D, Some(false) = -D
    defeasible: Option<bool>,  // None = unknown, Some(true) = +d, Some(false) = -d
}

// Per-rule tracking
struct RuleState {
    pending_body_count: usize, // body literals not yet +proved at this level
    discarded: bool,           // at least one body literal is -proved at this level
}

// Index structures (built once from the theory)
rules_for:       Map<Literal, Vec<RuleId>>          // R[q]  — all rules with head q
strict_rules_for: Map<Literal, Vec<RuleId>>         // Rs[q] — strict rules with head q
sd_rules_for:    Map<Literal, Vec<RuleId>>          // Rsd[q] — strict + defeasible rules
body_occurs_in:  Map<Literal, Vec<RuleId>>          // rules where literal appears in body
superiors_of:    Map<RuleId, Set<RuleId>>           // r2 ∈ superiors_of[r1] means r2 > r1
complement:      Fn(Literal) -> Literal             // ~q
```

### Phase 1: Definite Provability (+D / -D)

```
fn compute_definite(theory) -> Map<Literal, bool>:
    let D: Map<Literal, Option<bool>>   // +D = true, -D = false
    let pending: Queue<(Literal, bool)> // (literal, is_proved)

    // --- per-rule counters at the DEFINITE level ---
    // only strict rules participate
    let rule_pending:  Map<RuleId, usize>  // body literals not yet +D
    let rule_discarded: Map<RuleId, bool>  // has a -D body literal

    for each strict rule r:
        rule_pending[r] = |body(r)|
        rule_discarded[r] = false

    // --- seed facts ---
    for each fact q:
        D[q] = Some(true)
        pending.push((q, true))

    // --- seed literals with no strict rules ---
    for each literal q that appears in the theory:
        if Rs[q] is empty and q is not a fact:
            D[q] = Some(false)
            pending.push((q, false))

    // --- fixed-point loop ---
    while let Some((q, proved)) = pending.pop():
        for each strict rule r where q ∈ body(r):
            if proved:                            // q is +D
                rule_pending[r] -= 1
                if rule_pending[r] == 0 and not rule_discarded[r]:
                    let h = head(r)
                    if D[h] is None:
                        D[h] = Some(true)
                        pending.push((h, true))
            else:                                 // q is -D
                rule_discarded[r] = true
                let h = head(r)
                // check if ALL strict rules for h are now discarded
                if D[h] is None:
                    if all r' in Rs[h]: rule_discarded[r']:
                        D[h] = Some(false)
                        pending.push((h, false))

    return D
```

### Phase 2: Defeasible Provability (+d / -d)

```
fn compute_defeasible(theory, D) -> Map<Literal, bool>:
    let d: Map<Literal, Option<bool>>   // +d = true, -d = false
    let pending: Queue<(Literal, bool)>

    // --- per-rule counters at the DEFEASIBLE level ---
    // strict + defeasible rules participate (NOT defeaters for applicability,
    // but defeaters are tracked for discarding since they act as attackers)
    let rule_pending:  Map<RuleId, usize>
    let rule_discarded: Map<RuleId, bool>

    for each rule r (strict, defeasible, AND defeater):
        rule_pending[r] = |body(r)|
        rule_discarded[r] = false

    // --- seed from definite conclusions ---
    for each literal q where D[q] == Some(true):       // +D q implies +d q
        d[q] = Some(true)
        pending.push((q, true))

    for each literal q:
        // if q has no strict-or-defeasible rules AND is -D, then -d q
        if D[q] == Some(false) and Rsd[q] is empty:
            d[q] = Some(false)
            pending.push((q, false))

    // --- fixed-point loop ---
    while let Some((q, proved)) = pending.pop():
        // update rule counters for all rules containing q in body
        for each rule r where q ∈ body(r):
            if proved:
                rule_pending[r] -= 1
            else:
                rule_discarded[r] = true

        if proved:                                      // q just proved +d
            // try to prove +d for head(r) of newly-applicable rules
            for each rule r in Rsd[q] where q ∈ body(r):
                if rule_pending[r] == 0 and not rule_discarded[r]:
                    try_prove_defeasible(head(r))

            // q being +d may cause ~q to become -d
            try_disprove_defeasible(complement(q))

        else:                                           // q just proved -d
            // rules for q with discarded body -> maybe -d for head
            for each rule r in Rsd[q_head] where q ∈ body(r):
                // r is now discarded; check if all Rsd rules for head(r) are discarded
                try_disprove_defeasible(head(r))

            // q being -d means rules for ~q that had q as an attacker
            // lose that attacker -> maybe +d for ~q
            try_prove_defeasible(complement(q))

    return d
```

### Core: try_prove_defeasible

```
fn try_prove_defeasible(q):
    if d[q] is not None: return           // already decided

    let nq = complement(q)

    // condition (1): ∃r ∈ Rsd[q] that is applicable
    let has_applicable = any r in Rsd[q]:
        rule_pending[r] == 0 and not rule_discarded[r]
    if not has_applicable: return

    // condition (2): -D ~q
    if D[nq] != Some(false): return       // ~q is definitely proved, can't win

    // condition (3): every rule for ~q is countered
    for each s in R[nq]:                  // includes defeaters
        if rule_discarded[s]: continue    // s is inapplicable, OK

        if rule_pending[s] > 0: continue  // s not yet fully applicable, can't decide yet
                                          // (but s is not discarded either — must wait)
        // CORRECTION: if s is applicable (pending=0, not discarded), we need a superior t
        // if s is still undecided (pending>0, not discarded), we can't conclude yet
        if rule_pending[s] > 0 and not rule_discarded[s]:
            return                        // undecided attacker — must wait

        // s is applicable. need ∃t ∈ Rsd[q]: t applicable AND t > s
        let defeated = any t in Rsd[q]:
            rule_pending[t] == 0
            and not rule_discarded[t]
            and t ∈ superiors_of[s]       // t > s
        if not defeated: return           // undefeated attacker — can't prove +d q

    // all conditions met
    d[q] = Some(true)
    pending.push((q, true))
```

### Core: try_disprove_defeasible

```
fn try_disprove_defeasible(q):
    if d[q] is not None: return           // already decided

    // precondition: -D q
    if D[q] != Some(false): return

    let nq = complement(q)

    // check disjunct (2): +D ~q
    if D[nq] == Some(true):
        d[q] = Some(false)
        pending.push((q, false))
        return

    // check disjunct (1): all Rsd rules for q are discarded
    let all_discarded = all r in Rsd[q]: rule_discarded[r]

    if all_discarded:
        d[q] = Some(false)
        pending.push((q, false))
        return

    // check disjunct (3): ∃ applicable attacker s that no t can beat
    for each s in R[nq]:
        if rule_pending[s] > 0 or rule_discarded[s]:
            continue                      // s not applicable, skip

        // s is applicable. check: ∀t ∈ Rsd[q]: t discarded OR ¬(t > s)
        let all_t_fail = all t in Rsd[q]:
            rule_discarded[t]
            or t ∉ superiors_of[s]        // t does NOT beat s
        // NOTE: we also need undecided t's to have resolved.
        // if any t is still undecided (not discarded, not applicable), we can't conclude yet
        let any_t_undecided = any t in Rsd[q]:
            not rule_discarded[t]
            and rule_pending[t] > 0
            and t ∈ superiors_of[s]       // t COULD beat s if it becomes applicable
        if any_t_undecided: continue      // can't conclude yet for this s

        if all_t_fail:
            d[q] = Some(false)
            pending.push((q, false))
            return
```

### Top-Level Driver

```
fn reason(theory) -> Vec<Conclusion>:
    // build indexes
    build_indexes(theory)

    // phase 1: monotonic layer
    let D = compute_definite(theory)

    // phase 2: defeasible layer
    let d = compute_defeasible(theory, D)

    // collect results
    let conclusions = []
    for each literal q in theory:
        if D[q] == Some(true):  conclusions.push(+D q)
        if D[q] == Some(false): conclusions.push(-D q)
        if d[q] == Some(true):  conclusions.push(+d q)
        if d[q] == Some(false): conclusions.push(-d q)

    return conclusions
```

### Termination Guarantee

The algorithm terminates because:

1. Each literal transitions from `None` to `Some(true)` or `Some(false)` **exactly once**
2. Only transitions enqueue work onto `pending`
3. The number of literals is finite
4. Therefore the total number of transitions (and pending items) is bounded by `2 * |literals|` per phase

---

## Ambiguity Propagation Mode — Pseudocode

In the default **ambiguity blocking** mode, when two rules conflict and neither wins
(e.g., `r1: => p` and `r2: => -p` with no superiority), both `p` and `-p` are marked
`-d` — but downstream literals that don't depend on `p` are unaffected.

In **ambiguity propagation** mode, the ambiguity is *infectious*: if a literal is
ambiguous (has support but is blocked by an equally strong contrary), then anything
that depends on it also becomes ambiguous — even if those downstream literals have
other, uncontested support.

### Key Concept: Supported vs Ambiguous

A literal can be in one of four states during AP reasoning:

```
enum APState {
    Unknown,     // not yet evaluated
    Supported,   // has an applicable rule, but not yet checked against attackers
    Ambiguous,   // has support AND has an unbeatable applicable contrary rule
    Proved,      // +d: all attackers defeated
    Disproved,   // -d: no applicable rule, or definitely overridden
}
```

The critical distinction from ambiguity blocking: in AB mode, an ambiguous literal
is simply `-d` and rules depending on it are "discarded" the same as if the literal
had no support at all. In AP mode, we must distinguish *unsupported* (`-d` because
no rule fires) from *ambiguous* (`-d` because of unresolved conflict). Ambiguity
is contagious; lack of support is not.

### Additional Data Structures

```
// extends the base algorithm's data structures
let supported: Set<Literal>          // literals that have at least one applicable Rsd rule
let ambiguous: Set<Literal>          // literals blocked by unresolvable conflict
let ambiguity_depends: Map<Literal, Set<Literal>>
    // tracks which literals' ambiguity contributed to this literal's state
```

### Phase 2-AP: Defeasible Provability with Ambiguity Propagation

Phase 1 (definite provability) is unchanged. Only phase 2 differs.

```
fn compute_defeasible_ap(theory, D) -> Map<Literal, bool>:
    let d: Map<Literal, Option<bool>>
    let supported: Set<Literal>
    let ambiguous: Set<Literal>
    let pending: Queue<(Literal, bool)>

    // rule counters — same as blocking mode
    let rule_pending:  Map<RuleId, usize>
    let rule_discarded: Map<RuleId, bool>

    for each rule r (strict, defeasible, AND defeater):
        rule_pending[r] = |body(r)|
        rule_discarded[r] = false

    // --- seed from definite conclusions (same as blocking) ---
    for each literal q where D[q] == Some(true):
        d[q] = Some(true)
        supported.insert(q)
        pending.push((q, true))

    for each literal q:
        if D[q] == Some(false) and Rsd[q] is empty:
            d[q] = Some(false)
            pending.push((q, false))

    // === MAIN LOOP: process until worklist is empty ===
    while let Some((q, proved)) = pending.pop():

        // --- update rule counters (same as blocking) ---
        for each rule r where q ∈ body(r):
            if proved:
                rule_pending[r] -= 1
            else:
                rule_discarded[r] = true

        if proved:
            for each rule r in Rsd where q ∈ body(r):
                if rule_pending[r] == 0 and not rule_discarded[r]:
                    supported.insert(head(r))
                    try_prove_ap(head(r))
            try_disprove_ap(complement(q))
        else:
            for each rule r in Rsd where q ∈ body(r):
                try_disprove_ap(head(r))
            try_prove_ap(complement(q))

        // --- AP-specific: propagate ambiguity ---
        if q ∈ ambiguous:
            propagate_ambiguity(q)

    // === CLEANUP: anything still Unknown with support is ambiguous ===
    for each literal q where d[q] is None:
        if q ∈ supported:
            ambiguous.insert(q)
            d[q] = Some(false)     // ambiguous => not defeasibly provable
        else:
            d[q] = Some(false)     // no support => simply -d

    return d
```

### Core: try_prove_ap

Identical to the blocking version — the difference is in how ambiguity propagates,
not in how individual proofs are attempted.

```
fn try_prove_ap(q):
    if d[q] is not None: return

    let nq = complement(q)

    // condition (1): ∃r ∈ Rsd[q] that is applicable
    let has_applicable = any r in Rsd[q]:
        rule_pending[r] == 0 and not rule_discarded[r]
    if not has_applicable: return

    // condition (2): -D ~q
    if D[nq] != Some(false): return

    // condition (3): every rule for ~q is countered
    for each s in R[nq]:
        if rule_discarded[s]: continue

        if rule_pending[s] > 0 and not rule_discarded[s]:
            return                        // undecided attacker — wait

        // s is applicable — need ∃t ∈ Rsd[q]: t applicable AND t > s
        let defeated = any t in Rsd[q]:
            rule_pending[t] == 0
            and not rule_discarded[t]
            and t ∈ superiors_of[s]
        if not defeated: return

    d[q] = Some(true)
    pending.push((q, true))
```

### Core: try_disprove_ap

Same structure as blocking, but must distinguish "unsupported" from "ambiguous"
when recording the `-d` result.

```
fn try_disprove_ap(q):
    if d[q] is not None: return
    if D[q] != Some(false): return

    let nq = complement(q)

    // disjunct (2): +D ~q — strict override
    if D[nq] == Some(true):
        d[q] = Some(false)
        pending.push((q, false))
        return

    // disjunct (1): all Rsd rules for q are discarded — no support
    let all_discarded = all r in Rsd[q]: rule_discarded[r]
    if all_discarded:
        // NOT ambiguous — genuinely unsupported
        d[q] = Some(false)
        pending.push((q, false))
        return

    // disjunct (3): ∃ unbeatable applicable attacker
    for each s in R[nq]:
        if rule_pending[s] > 0 or rule_discarded[s]: continue

        let any_t_undecided = any t in Rsd[q]:
            not rule_discarded[t]
            and rule_pending[t] > 0
            and t ∈ superiors_of[s]
        if any_t_undecided: continue

        let all_t_fail = all t in Rsd[q]:
            rule_discarded[t] or t ∉ superiors_of[s]

        if all_t_fail:
            // q IS supported (has applicable rules) but can't beat the attacker
            if q ∈ supported:
                ambiguous.insert(q)       // mark as ambiguous, not merely unsupported
            d[q] = Some(false)
            pending.push((q, false))
            return
```

### Core: propagate_ambiguity

This is the key difference from ambiguity blocking. When a literal `q` is ambiguous,
every literal that depends on `q` through an applicable rule chain inherits the ambiguity.

```
fn propagate_ambiguity(q):
    // q is ambiguous. Find all literals downstream of q that are still undecided
    // and infect them with ambiguity.

    let worklist: Queue<Literal> = [q]

    while let Some(a) = worklist.pop():
        // for each rule r where a appears in body AND r is strict/defeasible
        for each rule r in Rsd where a ∈ body(r):
            let h = head(r)
            if d[h] is not None: continue       // already decided, skip
            if h ∈ ambiguous: continue          // already infected, skip

            // the rule r has an ambiguous body literal.
            // in AP mode, h inherits the ambiguity EVEN IF h has other
            // uncontested rules supporting it.
            //
            // however, we only infect h if h actually has support
            // (otherwise it's just unsupported, not ambiguous)
            if h ∈ supported or (any r' in Rsd[h]: rule_pending[r'] == 0 and not rule_discarded[r']):
                ambiguous.insert(h)
                d[h] = Some(false)
                pending.push((h, false))
                worklist.push(h)            // cascade further downstream
```

### Comparison: Blocking vs Propagation on the Same Theory

```
Theory:
    r1: => p
    r2: => -p
    r3: => q          // independent support for q
    r4: p => q         // also supports q, but depends on ambiguous p

Ambiguity Blocking:
    p  → ambiguous, marked -d
    -p → ambiguous, marked -d
    q  → +d  (r3 is uncontested, r4 is discarded because -d p, but r3 suffices)

Ambiguity Propagation:
    p  → ambiguous, marked -d
    -p → ambiguous, marked -d
    q  → -d  (even though r3 supports q independently, the ambiguity from
              r4's dependency on p infects q — q is marked ambiguous)
```

### AP Driver

```
fn reason_ap(theory) -> Vec<Conclusion>:
    build_indexes(theory)
    let D = compute_definite(theory)         // phase 1 unchanged
    let d = compute_defeasible_ap(theory, D) // phase 2 with AP semantics

    let conclusions = []
    for each literal q in theory:
        if D[q] == Some(true):  conclusions.push(+D q)
        if D[q] == Some(false): conclusions.push(-D q)
        if d[q] == Some(true):  conclusions.push(+d q)
        if d[q] == Some(false): conclusions.push(-d q)

    return conclusions
```

### Termination Guarantee (AP)

Same argument as blocking mode, plus:

5. `propagate_ambiguity` visits each literal at most once (guarded by `h ∈ ambiguous` check)
6. Each literal can be added to `ambiguous` at most once
7. Total work in propagation is bounded by `|literals| + |rules|`
