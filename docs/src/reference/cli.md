# CLI Reference

The `spindle` command-line tool for reasoning about defeasible logic theories.

## Installation

```bash
cargo install --path crates/spindle-cli
```

## Synopsis

```bash
spindle [OPTIONS] <FILE>
spindle <COMMAND> [OPTIONS] <FILE>
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

## Options

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
