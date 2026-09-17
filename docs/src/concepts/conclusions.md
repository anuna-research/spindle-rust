# Conclusions

Spindle computes four types of conclusions, representing different levels of provability.

## The Four Conclusion Types

| Symbol | Name | Meaning |
|--------|------|---------|
| `+D` | Definitely Provable | Proven via facts and strict rules only |
| `-D` | Definitely Not Provable | Constructively disproved at the definite level |
| `+d` | Defeasibly Provable | Proven via defeasible rules (subject to defeat) |
| `-d` | Defeasibly Not Provable | Constructively disproved at the defeasible level |

## Definite Conclusions (+D / -D)

**Definite provability** uses only facts and strict rules. No defeasible reasoning is involved.

```spl
(given bird)
(always r1 bird animal)        ; Strict rule
(normally r2 bird flies)       ; Defeasible rule
```

Conclusions:
- `+D bird` — fact
- `+D animal` — via strict rule r1
- `-D flies` — no strict path to prove flies

### When is +D Useful?

Definite provability represents certainty in domains such as:
- Binding legal requirements
- Safety constraints
- Logical necessities

## Defeasible Conclusions (+d / -d)

**Defeasible provability** extends definite provability with defeasible rules.

```spl
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

```spl
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

```spl
(given trigger)
(normally r1 trigger outcome)
(normally r2 trigger (not outcome))
(prefer r1 r2)
```

Conclusions:
- `+d outcome` — r1 wins
- `-d -outcome` — r2 is defeated

## Example: Multi-Level

```spl
(given a)
(always r1 a b)                ; Strict: a implies b
(normally r2 b c)              ; Defeasible: b typically implies c
(normally r3 b (not c))        ; Defeasible: b typically implies (not c)
(prefer r2 r3)                 ; r2 wins
```

Conclusions:
| Conclusion | Reason |
|---|---|
| `+D a` | Fact |
| `+D b` | Strict from a |
| `+d a` | Implied by +D |
| `+d b` | Implied by +D |
| `+d c` | Defeasible, r2 wins over r3 |
| `-D c` | No strict path |
| `-D -c` | No strict path |
| `-d -c` | r3 defeated |

## Negative Conclusions

Negative conclusions (`-D`, `-d`) require constructive evidence under the
proof conditions. Failure to find a positive proof is not itself a negative proof.
For example, `(always loop p p)` leaves `p` undecided: neither `+D p` nor `-D p`,
and neither `+d p` nor `-d p`. `(normally loop p p)` yields `-D p`, but leaves
defeasible provability undecided.

An undecided premise does not justify discarding an attacker. This distinction
matters when rules contain cycles. See [Algorithms](../guides/algorithms.md).

Negative tags also differ from strong negation: `-d p` does not establish `+d ~p`.
If both `p` and `~p` are definite facts, both retain `+D` and `+d`; this does not
prove unrelated literals.

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

The `--positive` flag restricts output to positive conclusions:
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
