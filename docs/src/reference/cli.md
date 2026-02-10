# CLI Reference

The `spindle` command-line tool for reasoning about defeasible logic theories.

## Installation

```bash
cargo install --path crates/spindle-cli
```

## Synopsis

```bash
spindle <COMMAND> [OPTIONS]
spindle reason [OPTIONS] [FILE]
spindle query <LITERAL> [FILE] [OPTIONS]
spindle explain <LITERAL> [FILE] [OPTIONS]
spindle why-not <LITERAL> [FILE] [OPTIONS]
spindle requires <LITERAL> [FILE] [OPTIONS]
```

## Commands

### reason

Perform defeasible reasoning on a theory.

```bash
spindle reason examples/penguin.dfl
```

### validate

Check syntax without reasoning.

```bash
spindle validate examples/penguin.dfl
spindle --json validate --stdin < examples/penguin.dfl
```

Output on success:
```
Valid theory file
```

With `--json` success output:
```json
{
  "valid": true,
  "diagnostics": []
}
```

Output on error:
```
Error at line 5: could not parse: invalid => syntax
```

### stats

Show theory statistics.

```bash
spindle stats examples/penguin.dfl
spindle --json stats --stdin < examples/penguin.dfl
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

With `--json` success output:
```json
{
  "stats": {
    "total_rules": 8,
    "facts": 2,
    "strict": 1,
    "defeasible": 4,
    "defeaters": 1,
    "superiorities": 1
  },
  "diagnostics": []
}
```

### query

Query if a literal holds in the theory.

```bash
spindle query flies examples/penguin.dfl
spindle query "~flies" examples/penguin.dfl
spindle query "(not flies)" examples/penguin.dfl
spindle query flies examples/penguin.dfl --json
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
spindle explain "-flies" examples/penguin.dfl
spindle explain "-flies" examples/penguin.dfl --json
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
spindle why-not flies examples/penguin.dfl
spindle why-not flies examples/penguin.dfl --json
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
spindle requires flies examples/penguin.dfl
spindle requires flies examples/penguin.dfl --max 5
spindle requires flies examples/penguin.dfl --json
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

Output results in JSON format. Available for all commands, including `validate` and `stats`.
When `--json` is present, success and failure paths are machine-readable JSON.

```bash
spindle reason examples/penguin.dfl --json
spindle query flies examples/penguin.dfl --json
spindle explain "-flies" examples/penguin.dfl --json
spindle --json validate --stdin < examples/penguin.dfl
```

Parse/usage failures also emit JSON envelopes when `--json` is present:

```bash
spindle --json
```

### `--scalable`

Use the scalable DL(d||) algorithm instead of standard DL(d).

```bash
spindle reason --scalable large-theory.dfl
```

Recommended for theories with:
- More than 1000 rules
- Complex conflict resolution
- Long inference chains

### `--positive`

Show only positive conclusions (+D, +d).

```bash
spindle reason --positive examples/penguin.dfl
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
| 2 | User input / parse / validation error |
| 3 | Execution/internal reasoning error |
| 4 | Resource/limit/timeout hit |

## Examples

### Basic Reasoning

```bash
# Reason about a theory
spindle reason penguin.dfl
```

### Querying and Explaining

```bash
# Check if a literal holds
spindle query flies penguin.dfl

# Get a proof tree for a derived conclusion
spindle explain "-flies" penguin.dfl

# Debug why something is not provable
spindle why-not flies penguin.dfl

# Find what facts would make a literal provable
spindle requires flies penguin.dfl --max 5

# Get JSON output for scripting
spindle query flies penguin.dfl --json
```

### Validate Before Reasoning

```bash
spindle validate theory.dfl && spindle reason theory.dfl
```

### Compare Algorithms

```bash
# Standard algorithm
spindle reason theory.dfl > standard.txt

# Scalable algorithm
spindle reason --scalable theory.dfl > scalable.txt

# Compare (should be identical for correct theories)
diff standard.txt scalable.txt
```

### Scripting

```bash
#!/bin/bash
for file in theories/*.dfl; do
    echo "Processing $file..."
    if spindle validate "$file"; then
        spindle reason --positive "$file"
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
SPINDLE_LOG=debug spindle reason theory.dfl
```
