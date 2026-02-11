# Algorithms

Spindle uses a single reasoning engine: **standard DL(d) forward chaining**.

## Overview

The engine computes conclusions in three phases:

1. Seed facts as both `+D` and `+d`.
2. Forward-chain strict and defeasible rules until saturation.
3. Emit negative conclusions (`-D`, `-d`) for literals not proven.

## Forward Chaining

At a high level:

```text
function reason(theory):
  proven_definite = {}
  proven_defeasible = {}
  worklist = facts + empty-body rule heads

  while worklist not empty:
    lit = pop(worklist)
    for rule triggered by lit:
      if body_satisfied(rule):
        fire(rule)

  emit -D and -d for literals not in proven sets
```

Strict rules add both `+D` and `+d`.  
Defeasible rules add `+d` only if not blocked.

## Conflict Handling

For a defeasible rule `r: body => q`, attackers are rules for `~q`.

`r` is blocked when:

1. An applicable **defeater** for `~q` exists, and `r` is not superior to it.
2. An applicable **defeasible** attacker exists that is explicitly superior to `r`.

If no superiority relation exists between two applicable defeasible opponents, Spindle does **not**
auto-block ties. Both sides can remain defeasibly derivable unless explicit superiority or defeaters
resolve the conflict.

This means superiority declarations are the primary mechanism for deterministic conflict resolution.

## Practical Semantics

- Add `prefer r1 r2` / `r1 > r2` when you want one side to win.
- Without superiority, opposing defeasible rules can yield both `+d p` and `+d ~p`.
- `-d p` means “not defeasibly provable”, not “false”.

## Complexity

- Time: roughly `O(n * m)` where `n` is rules and `m` is average body size.
- Space: linear in literals/rules for indexes, worklists, and proven sets.

The implementation relies on indexing and bitsets for fast membership checks during propagation.
