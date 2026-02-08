# CLI Reference

The `spindle` command-line tool for reasoning about defeasible logic theories.

## Installation

```bash
cargo install --path crates/spindle-cli
```

## Synopsis

```bash
spindle [OPTIONS] <FILE>
spindle <COMMAND> [OPTIONS] <FILE> [LITERAL]
```

## Commands

### reason (default)

Perform defeasible reasoning on a theory.

```bash
spindle examples/penguin.dfl
spindle reason examples/penguin.dfl
```

### validate

Check syntax without reasoning.

```bash
spindle validate examples/penguin.dfl
```

Output on success:
```
Valid DFL theory.
```

Output on error:
```
Error at line 5: could not parse: invalid => syntax
```

### stats

Show theory statistics.

```bash
spindle stats examples/penguin.dfl
```

Output:
```
Theory Statistics:
  Facts:       2
  Strict:      1
  Defeasible:  4
  Defeaters:   1
  Superiority: 1
  Total rules: 8
```

### query

Query if a literal holds in the theory.

```bash
spindle query examples/penguin.dfl flies
spindle query examples/penguin.dfl "~flies"
spindle query examples/penguin.dfl "(not flies)"
spindle query examples/penguin.dfl --json flies
```

The literal argument supports multiple formats: `p`, `~p`, `(not p)`, or complex SPL expressions.

Returns a `QueryStatus`:

| Status | Meaning |
|--------|---------|
| Provable | The literal is defeasibly provable |
| Refuted | The negation of the literal is provable |
| Unknown | Neither the literal nor its negation is provable |

Output:
```
QueryStatus: Provable
```

With `--json`:
```json
{"literal":"flies","status":"Refuted"}
```

### explain

Show the derivation proof tree for why a literal holds.

```bash
spindle explain examples/penguin.dfl "-flies"
spindle explain examples/penguin.dfl --json "-flies"
```

Shows the proof tree detailing how the reasoning engine derived the conclusion.

Output:
```
Explanation for -flies:
  -flies ← [defeasible] r3: penguin => -flies
    penguin ← [fact]
  Blocked alternatives:
    r1: bird => flies (defeated by r3 via superiority)
  Conflict resolutions:
    r3 > r1 (superiority)
```

With `--json`, the output is a JSON or JSON-LD structure containing an `Explanation` with proof nodes, blocked alternatives, and conflict resolutions.

### why-not

Explain why a literal is NOT provable.

```bash
spindle why-not examples/penguin.dfl flies
spindle why-not examples/penguin.dfl --json flies
```

Lists the blocking rules and the reasons they prevent the literal from being derived. Useful for debugging unexpected results. When the literal is provable, the JSON output includes `is_provable: true` and `blocked_by` will be empty.

Output:
```
Why not flies?
  Rule r1: bird => flies
    Status: Defeated
    Defeated by: r3 (penguin => -flies) via superiority r3 > r1
```

Possible blocking reasons:

| Reason | Meaning |
|--------|---------|
| MissingPremise | A required premise of the rule is not provable |
| Defeated | The rule is defeated by a stronger or competing rule |
| Contradicted | The conclusion conflicts with a strictly proved literal |

### requires

Abduction: find the minimal sets of facts needed to derive a literal.

```bash
spindle requires examples/penguin.dfl flies
spindle requires examples/penguin.dfl --max 5 flies
spindle requires examples/penguin.dfl --json flies
```

The `--max` option limits the number of solutions returned (defaults to 10).

Output:
```
To derive flies, you could assume:
  1. { bird, -penguin }
  2. { flies }
```

Each result is a minimal set of assumptions that, if added to the theory, would make the literal provable.

## Options

### `--json`

Output results in JSON format. Available for `reason`, `query`, `explain`, `why-not`, and `requires` commands.

```bash
spindle reason examples/penguin.dfl --json
spindle query examples/penguin.dfl --json flies
spindle explain examples/penguin.dfl --json "-flies"
```

### `--scalable`

Use the scalable DL(d||) algorithm instead of standard DL(d).

```bash
spindle --scalable large-theory.dfl
```

Recommended for theories with:
- More than 1000 rules
- Complex conflict resolution
- Long inference chains

### `--positive`

Show only positive conclusions (+D, +d).

```bash
spindle --positive examples/penguin.dfl
```

Output:
```
+D bird
+D penguin
+d bird
+d penguin
+d -flies
```

## File Format Detection

The CLI auto-detects format by extension:

| Extension | Format |
|-----------|--------|
| `.dfl` | DFL (Defeasible Logic Format) |
| `.spl` | SPL (Spindle Lisp) |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error (parse error, file not found, etc.) |

## Examples

### Basic Reasoning

```bash
# Reason about a theory
spindle penguin.dfl

# Same with explicit command
spindle reason penguin.dfl
```

### Querying and Explaining

```bash
# Check if a literal holds
spindle query penguin.dfl flies

# Get a proof tree for a derived conclusion
spindle explain penguin.dfl "-flies"

# Debug why something is not provable
spindle why-not penguin.dfl flies

# Find what facts would make a literal provable
spindle requires penguin.dfl flies --max 5

# Get JSON output for scripting
spindle query penguin.dfl --json flies
```

### Validate Before Reasoning

```bash
spindle validate theory.dfl && spindle theory.dfl
```

### Compare Algorithms

```bash
# Standard algorithm
spindle theory.dfl > standard.txt

# Scalable algorithm
spindle --scalable theory.dfl > scalable.txt

# Compare (should be identical for correct theories)
diff standard.txt scalable.txt
```

### Scripting

```bash
#!/bin/bash
for file in theories/*.dfl; do
    echo "Processing $file..."
    if spindle validate "$file"; then
        spindle --positive "$file"
    else
        echo "Invalid: $file"
    fi
done
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `SPINDLE_LOG` | Set log level (error, warn, info, debug, trace) |

```bash
SPINDLE_LOG=debug spindle theory.dfl
```
