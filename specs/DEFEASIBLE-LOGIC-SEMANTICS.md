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

```
f1: >> p
f2: >> -p
```

Both are facts (empty-body defeasible rules). No superiority relation defined.

**Evaluate `+d p`:**

- Condition (1): `f1` is applicable (empty body) — **pass**
- Condition (2): `-D -p` — `-p` has no strict rules, so yes — **pass**
- Condition (3): Check `f2 ∈ R[-p]`: is `f2` discarded? No (empty body). Is there `t ∈ Rsd[p]` with `t > f2`? No superiority defined — **fail**

**Result:** `+d p` fails. By symmetry `+d -p` also fails.

```
-d p
-d -p
```

Neither literal is defeasibly provable. **Consistency enforced.**

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
| Fact | `>>` | Yes (defeasible) | Yes | Defeasible |
| Strict | `->` | Yes (definite + defeasible) | Yes | Definite |
| Defeasible | `=>` | Yes (defeasible) | Yes | Defeasible |
| Defeater | `~>` | **No** | Yes | — |
| Superiority | `>` | — | — | — |
