# DFL Format Reference

DFL (Defeasible Logic Format) is a textual syntax for writing defeasible logic theories.

## File Extension

`.dfl`

## Comments

Lines starting with `#` are comments:

```dfl
# This is a comment
f1: >> bird   # Inline comment
```

## Grammar

```ebnf
theory      = (rule | superiority | comment | blank)*
rule        = label ":" body? arrow head
superiority = label ">" label
label       = identifier
body        = literal ("," literal)*
head        = literal
literal     = negation? identifier
negation    = "-" | "~" | "¬"
arrow       = ">>" | "->" | "=>" | "~>"
identifier  = [a-zA-Z_][a-zA-Z0-9_-]*
comment     = "#" .*
blank       = whitespace
```

## Rules

### Facts (`>>`)

No body, unconditional truth.

```dfl
f1: >> bird
f2: >> penguin
f3: >> -guilty
```

### Strict Rules (`->`)

Body implies head necessarily.

```dfl
r1: penguin -> bird
r2: human, mortal -> dies
r3: a, b, c -> result
```

### Defeasible Rules (`=>`)

Body typically implies head.

```dfl
r1: bird => flies
r2: penguin => -flies
r3: student, employed => busy
```

### Defeaters (`~>`)

Body blocks head (doesn't prove it).

```dfl
d1: broken_wing ~> flies
d2: exception ~> normal_behavior
```

## Literals

### Simple Literals

```dfl
bird
flies
has_feathers
```

### Negated Literals

All three forms are equivalent:

```dfl
-flies
~flies
¬flies
```

### Identifiers

Valid identifiers:
- Start with letter or underscore
- Contain letters, digits, underscores, hyphens

```dfl
bird
_private
has-feathers
rule_1
```

## Rule Bodies

### Single Literal

```dfl
r1: bird => flies
```

### Multiple Literals (Conjunction)

Comma-separated:

```dfl
r1: bird, healthy => flies
r2: a, b, c, d => result
```

### Mixed Negation

```dfl
r1: bird, -penguin => flies
r2: student, -rich, -employed => needs_loan
```

## Superiority Relations

Declare that one rule beats another:

```dfl
r2 > r1
more_specific > general
exception > default
```

Multiple declarations:

```dfl
r3 > r2
r2 > r1
```

## Complete Example

```dfl
# The Penguin Example
# Demonstrates defeasible reasoning with exceptions

# Facts
f1: >> bird
f2: >> penguin

# Strict rule: all penguins are birds
s1: penguin -> bird

# Defeasible rules
r1: bird => flies
r2: bird => has_feathers
r3: penguin => -flies
r4: penguin => swims

# Superiority: penguin rules beat bird rules
r3 > r1

# Defeater: broken wings block flying
d1: broken_wing ~> flies
```

## Whitespace

- Leading/trailing whitespace is ignored
- Whitespace around operators is optional
- Blank lines are ignored

These are equivalent:

```dfl
r1: bird => flies
r1:bird=>flies
r1 : bird => flies
```

## Error Handling

Common errors:

| Error | Example | Problem |
|-------|---------|---------|
| Missing label | `>> bird` | Rules need labels |
| Missing colon | `r1 >> bird` | Colon required after label |
| Invalid arrow | `r1: bird -> flies` | Wrong arrow for defeasible |
| Undefined rule | `r3 > r1` | r3 not defined |

## Best Practices

1. **Use meaningful labels**
   ```dfl
   # Good
   bird_flies: bird => flies

   # Avoid
   r1: bird => flies
   ```

2. **Group related rules**
   ```dfl
   # Bird rules
   r1: bird => flies
   r2: bird => has_feathers

   # Penguin rules
   r3: penguin => -flies
   ```

3. **Document superiority**
   ```dfl
   # Penguin is more specific than bird
   r3 > r1
   ```
