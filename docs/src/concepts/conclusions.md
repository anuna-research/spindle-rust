# Conclusions

Spindle computes four types of conclusions, representing different levels of provability.

## The Four Conclusion Types

| Symbol | Name | Meaning |
|--------|------|---------|
| `+D` | Definitely Provable | Proven via facts and strict rules only |
| `-D` | Definitely Not Provable | Cannot be proven via strict rules |
| `+d` | Defeasibly Provable | Proven via defeasible rules (may be defeated) |
| `-d` | Defeasibly Not Provable | Cannot be proven even defeasibly |

## Definite Conclusions (+D / -D)

**Definite provability** uses only facts and strict rules. No defeasible reasoning is involved.

```lisp
(given bird)
(always r1 bird animal)        ; Strict rule
(normally r2 bird flies)       ; Defeasible rule
```

Conclusions:
- `+D bird` — fact
- `+D animal` — via strict rule r1
- `-D flies` — no strict path to prove flies

### When is +D Useful?

Use definite provability when you need certainty:
- Legal requirements that must be met
- Safety constraints
- Logical necessities

## Defeasible Conclusions (+d / -d)

**Defeasible provability** extends definite provability with defeasible rules.

```lisp
(given bird)
(normally r1 bird flies)
```

Conclusions:
- `+d bird` — fact (also +D)
- `+d flies` — via defeasible rule r1

### The Relationship

```
+D implies +d
   If definitely provable, then defeasibly provable

-d implies -D
   If not defeasibly provable, then definitely not provable
```

## Conflict and Ambiguity

When rules conflict without a superiority relation, neither conclusion is provable:

```lisp
(given trigger)
(normally r1 trigger outcome)
(normally r2 trigger (not outcome))
; No superiority declared
```

Conclusions:
- `+D trigger`
- `-d outcome` — blocked by r2
- `-d -outcome` — blocked by r1

Both outcomes are **ambiguous** — neither can be proven.

## Resolved Conflict

With superiority, the conflict is resolved:

```lisp
(given trigger)
(normally r1 trigger outcome)
(normally r2 trigger (not outcome))
(prefer r1 r2)
```

Conclusions:
- `+d outcome` — r1 wins
- `-d -outcome` — r2 is defeated

## Example: Multi-Level

```lisp
(given a)
(always r1 a b)                ; Strict: a implies b
(normally r2 b c)              ; Defeasible: b typically implies c
(normally r3 b (not c))        ; Defeasible: b typically implies (not c)
(prefer r2 r3)                 ; r2 wins
```

Conclusions:
- `+D a` — fact
- `+D b` — strict from a
- `+d a` — (implied by +D)
- `+d b` — (implied by +D)
- `+d c` — defeasible, r2 wins over r3
- `-D c` — no strict path
- `-D -c` — no strict path
- `-d -c` — r3 defeated

## Negative Conclusions

Negative conclusions (`-D`, `-d`) indicate unprovability:

### -D (Definitely Not Provable)
No chain of facts and strict rules leads to this literal.

### -d (Defeasibly Not Provable)
No chain of rules (including defeasible) leads to this literal, OR the literal is blocked by:
- A strict rule proving the complement
- A superior defeasible rule
- A defeater

## Reading Spindle Output

```bash
$ spindle reason penguin.spl
+D bird
+D penguin
+d bird
+d penguin
+d -flies
-D flies
-D -flies
-d flies
```

Interpretation:
- `bird` and `penguin` are facts (both +D and +d)
- `-flies` is defeasibly provable (penguin rule wins)
- `flies` is not provable at any level
- Neither `flies` nor `-flies` is definitely provable

## Filtering Output

Show only positive conclusions:
```bash
spindle reason --positive penguin.spl
```

Output:
```
+D bird
+D penguin
+d bird
+d penguin
+d -flies
```
