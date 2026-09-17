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
spindle vocabulary [FILE] [OPTIONS]
spindle what-if <LITERAL> [FILE] --given <LITERAL> [OPTIONS]
spindle abduce <LITERAL> [FILE] [OPTIONS]
spindle capabilities [OPTIONS]
spindle explain-code <CODE>
```

## Commands

### reason

`reason` performs defeasible reasoning on a theory.

```bash
spindle reason examples/penguin.spl
```

#### `--v2`

`--v2` selects JSON schema version v2 (`spindle.reason.v2`) for the JSON output envelope.
The v2 schema includes typed term arguments in the `literal_struct` fields, giving
downstream consumers richer structure than the v1 flat-string representation.

```bash
spindle reason examples/penguin.spl --json --v2
```

Without `--v2`, the default schema is `spindle.reason.v1`.

#### `--trust`

`--trust` includes trust-weight annotations on each conclusion. In JSON, each conclusion
carries a `trust_degree` (0.0--1.0), a `trust_sources` list when present, and
`trust_details` containing ordered `diminished_by` challenges and named
`above_threshold` results. Both JSON versions expose these fields.

```bash
spindle reason examples/penguin.spl --json --trust
```

### validate

`validate` checks syntax without reasoning.

```bash
spindle validate examples/penguin.spl
spindle --json validate --stdin < examples/penguin.spl
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

`stats` shows theory statistics.

```bash
spindle stats examples/penguin.spl
spindle --json stats --stdin < examples/penguin.spl
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

`query` reports whether a literal holds in the theory.

```bash
spindle query flies examples/penguin.spl
spindle query "~flies" examples/penguin.spl
spindle query "(not flies)" examples/penguin.spl
spindle query flies examples/penguin.spl --json
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

`explain` shows the derivation proof tree for a literal.

```bash
spindle explain "-flies" examples/penguin.spl
spindle explain "-flies" examples/penguin.spl --json
```

Shows the proof tree detailing how the reasoning engine derived the conclusion.

Output:
```
Explanation for -flies:
  -flies ← [defeasible] r3: penguin -> -flies
    penguin ← [fact]
  Blocked alternatives:
    r1: bird -> flies (defeated by r3 via superiority)
  Conflict resolutions:
    r3 > r1 (superiority)
```

With `--json`, the output is a JSON or JSON-LD structure containing an `Explanation` with proof nodes, blocked alternatives, and conflict resolutions.

### why-not

`why-not` explains why a literal is not provable.

```bash
spindle why-not flies examples/penguin.spl
spindle why-not flies examples/penguin.spl --json
```

Lists the blocking rules and the reasons they prevent the literal from being derived. Useful for debugging unexpected results. When the literal is provable, the JSON output includes `is_provable: true` and `blocked_by` will be empty.

Output:
```
Why not flies?
  Rule r1: bird -> flies
    Status: Defeated
    Defeated by: r3 (penguin -> -flies) via superiority r3 > r1
```

Possible blocking reasons:

| Reason | Meaning |
|--------|---------|
| MissingPremise | A premise of the rule is not provable |
| Defeated | The rule is defeated by a stronger or competing rule |
| Contradicted | The conclusion conflicts with a strictly proved literal |

### requires

`requires` finds minimal sets of facts needed to derive a literal through abduction.

```bash
spindle requires flies examples/penguin.spl
spindle requires flies examples/penguin.spl --max 5
spindle requires flies examples/penguin.spl --json
```

The `--max` option limits the number of solutions returned (defaults to 10).

As of the v2 contract (IMPL-011), `requires` **verifies all candidate solutions
by default**. The reasoning engine checks each candidate set of facts to verify that it makes the goal provable. The JSON output includes
`verification_mode: "verified"` and a `verification` object with
`raw_examined`, `accepted`, and `rejected` counts. Only accepted solutions
appear in the `solutions` array. The JSON envelope uses schema
`spindle.requires.v2`.

Output:
```
Verified requirements for flies:
  1. Add facts: {bird, -penguin}
  2. Add facts: {flies}
```

Each result is a minimal, verified set of assumptions. Adding those assumptions to the theory makes the literal provable.

### capabilities

`capabilities` lists the commands, features, and JSON schema versions this Spindle build supports. Useful for tooling that needs to discover available functionality
at runtime.

```bash
spindle capabilities
spindle capabilities --json
```

Output:
```
Spindle Capabilities:

Commands: reason, query, requires, explain, why-not, vocabulary, what-if, abduce, validate, stats, capabilities, explain-code

Features:
  --stdin: yes
  --at: yes
  --json: yes
  --v2: yes
  --trust (including diminishment and thresholds): yes
  --extensions: yes
  Trust overlay: no
  Given flags: no

Schema versions:
  reason: spindle.reason.v1
  reason_v2: spindle.reason.v2
  vocabulary: spindle.vocabulary/1
  query: spindle.query.v1
  requires: spindle.requires.v2
  explain: spindle.explain.v1
  why-not: spindle.why_not.v1
```

With `--json`, the output is a JSON object with schema `spindle.capabilities.v1`
containing `commands`, `features`, and `schemas` fields.

### explain-code

`explain-code` reports the meaning and common causes of a stable error code.

```bash
spindle explain-code RULE_NOT_FOUND
spindle explain-code SPL_PARSE_ERROR
```

This command does not accept `--json`; output is always human-readable text.
It provides guidance for unfamiliar error codes in JSON error envelopes.

## JSON Envelope Schema Versions

Reasoning and diagnostic envelopes use `schema_version`. Vocabulary uses `schema`;
what-if and raw abduction use the corresponding WASM result shapes. The following
schema versions are defined:

| Schema | Command | Description |
|--------|---------|-------------|
| `spindle.reason.v1` | `reason` | Default reason output with flat literal strings |
| `spindle.reason.v2` | `reason --v2` | Reason output with typed term arguments |
| `spindle.query.v1` | `query` | Query result with status |
| `spindle.explain.v1` | `explain` | Proof tree with blocked alternatives |
| `spindle.why_not.v1` | `why-not` | Blocking-reason analysis |
| `spindle.requires.v2` | `requires` | Verified abduction solutions (v2 contract) |
| `spindle.capabilities.v1` | `capabilities` | Feature and schema discovery |

## Options

### `--json`

`--json` outputs results in JSON format, including for `validate` and `stats`. The `explain-code` command does not accept this option.
When `--json` is present, success and failure paths are machine-readable JSON.

```bash
spindle reason examples/penguin.spl --json
spindle query flies examples/penguin.spl --json
spindle explain "-flies" examples/penguin.spl --json
spindle --json validate --stdin < examples/penguin.spl
```

Parse/usage failures also emit JSON envelopes when `--json` is present:

```bash
spindle --json
```

### `--at <TIME>`

`--at` sets the reference time for temporal ("as-of") reasoning. The value MUST be an ISO 8601 / RFC 3339 timestamp.

```bash
spindle reason --at 2024-06-15T12:00:00Z examples/temporal.spl
spindle query p --at 2024-06-15T12:00:00Z examples/temporal.spl
```

### `--stdin`

`--stdin` reads the theory from standard input instead of a file. Mutually exclusive with
providing a file path.

```bash
cat examples/penguin.spl | spindle reason --stdin
spindle --json validate --stdin < examples/penguin.spl
```

### `--positive`

`--positive` shows only positive conclusions (+D, +d). Applies to the `reason` command.

```bash
spindle reason --positive examples/penguin.spl
```

Output:
```
+D bird
+D penguin
+d bird
+d penguin
+d -flies
```

### `--debug-errors`

`--debug-errors` shows full error details including source chains and unredacted file paths.
Useful for diagnosing unexpected failures.

```bash
spindle reason examples/penguin.spl --debug-errors
```

## File Format Detection

The CLI auto-detects format by extension:

| Extension | Format |
|-----------|--------|
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
spindle reason penguin.spl
```

### Querying and Explaining

```bash
# Check if a literal holds
spindle query flies penguin.spl

# Get a proof tree for a derived conclusion
spindle explain "-flies" penguin.spl

# Debug why something is not provable
spindle why-not flies penguin.spl

# Find what facts would make a literal provable
spindle requires flies penguin.spl --max 5

# Get JSON output for scripting
spindle query flies penguin.spl --json
```

### Validate Before Reasoning

```bash
spindle validate theory.spl && spindle reason theory.spl
```

### Compare Runs

```bash
# Compare two revisions or theories
spindle reason theory-a.spl > run-a.txt
spindle reason theory-b.spl > run-b.txt
diff run-a.txt run-b.txt
```

### Scripting

```bash
#!/bin/bash
for file in theories/*.spl; do
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
| `SPINDLE_LOG` | Log level (error, warn, info, debug, trace) |

```bash
SPINDLE_LOG=debug spindle reason theory.spl
```

## Aggregate programs and query matching

```sh
spindle reason examples/aggregation.spl --json --v2
spindle query '(total-payment alice 20)' examples/aggregation.spl
```

Aggregation uses the normal `reason` and `query` commands without an extra flag. The CLI supplies the builtin function prelude. Registering custom
functions is a Rust embedding API, not a CLI plugin-loading facility.

Bounded temporal goals match identical windows, including in `requires`.
A containing window is not an exact match. Atemporal goals match any family
member. See [Query Operators](../guides/queries.md).

## Vocabulary and hypothetical queries

`spindle vocabulary theory.spl --json` returns `spindle.vocabulary/1`: structural
predicate symbols (functor/arity), declarations, profiles, provenance and shape
or declaration diagnostics. It inspects the original theory without grounding.

`spindle what-if '(q 2)' theory.spl --given '(p 2)' --json` evaluates hypothetical
facts without modifying the file. Repeat `--given` for multiple facts. Its output
contains `provable`, `new_conclusions`, `new_conclusions_struct` (typed literals),
and `changed_conclusions`. Facts are added before grounding.

`spindle abduce q theory.spl --max 10 --json` returns raw candidates with `facts` (display strings),
`facts_struct` (typed literals), `rules_used` and `confidence`. Use `requires` when candidates must be verified.
Both commands accept `--at` and `--extensions`.

Query literals use SPL, including typed arguments, modal forms and temporal
windows. Legacy comma notation such as `p(2)` is also accepted. Invalid input
now reports `INVALID_LITERAL` instead of silently treating it as an atom.

## Portable extensions

Both CLI and WASM accept a `spindle.extensions.v1` JSON document. The CLI's global
`--extensions FILE` option loads it; WASM uses `registerExtensions(jsonString)`.
Registrations supply finite pure lookup functions and named integer aggregators.

```json
{
  "schema_version": "spindle.extensions.v1",
  "functions": [{
    "name": "classification",
    "arguments": ["integer"],
    "returns": "symbol",
    "rows": [{
      "args": [{"type": "integer", "value": 2}],
      "result": {"type": "symbol", "value": "small"}
    }]
  }],
  "aggregators": [{"name": "total", "reducer": "+", "identity": 0}]
}
```

Supported declared types are `symbol`, `integer`, `decimal`, and `float`, with
values encoded using the typed term format. Decimal values are strings; integers
outside JavaScript's exact range should be strings. Matching is type-sensitive.
Names cannot replace builtins; duplicate names/keys, invalid terms, and signature
mismatches are rejected. Missing lookup rows return evaluation failure; ordinary
grounding discards that candidate binding.

An aggregator names a binary reducer, an optional integer identity (null means
empty input fails the premise), and optional `count: true`. Custom reducers must
be associative and commutative and return integers for every intermediate value.
The ordinary aggregate restrictions remain in force.

```bash
spindle reason examples/lookup-functions.spl \
  --extensions examples/lookup-functions.json --json --v2
```

The same registry is used by `reason`, `query`, `explain`, `why-not`, `requires`,
`what-if`, and `abduce`. This portable format does not execute arbitrary host code;
Rust embedding retains the unrestricted pure-function registration API.
