# Getting Started

This guide walks you through installing Spindle-Rust and running your first defeasible logic program.

Spindle-Rust is a Rust port of [SPINdle](https://research.csiro.au/bpli/tools/spindle/), a defeasible logic reasoning system originally developed by NICTA (now Data61/CSIRO). This implementation is based on [spindle-racket](https://codeberg.org/anuna/spindle-racket) v1.7.0.

## Installation

### Building from Source

```bash
git clone https://codeberg.org/anuna/spindle-rust
cd spindle-rust
cargo build --release
```

### Installing the CLI

```bash
cargo install --path crates/spindle-cli
```

This installs the `spindle` command to your Cargo bin directory.

## Your First Theory

Create a file called `hello.spl`:

```spl
; Facts
(given bird)

; Rules
(normally r1 bird flies)
(normally r2 bird has_feathers)
```

Run it:

```bash
spindle reason hello.spl
```

Output:

```
+D bird
+d bird
+d flies
+d has_feathers
-D flies
-D has_feathers
```

### Understanding the Output

| Conclusion | Meaning |
|------------|---------|
| `+D bird` | `bird` is **definitely** provable (it's a fact) |
| `+d bird` | `bird` is **defeasibly** provable |
| `+d flies` | `flies` is defeasibly provable via r1 |
| `-D flies` | `flies` is **not** definitely provable (no strict rule) |

## The Penguin Example

Create `penguin.spl`:

```spl
; Tweety is a bird and a penguin
(given bird)
(given penguin)

; Birds typically fly
(normally r1 bird flies)

; Penguins typically don't fly
(normally r2 penguin (not flies))

; Penguin rule is more specific
(prefer r2 r1)
```

Run it:

```bash
spindle reason penguin.spl
```

Output:

```
+D bird
+D penguin
+d bird
+d penguin
+d -flies
-D flies
-D -flies
-d flies
```

Key result: `+d -flies` - Tweety defeasibly doesn't fly because the penguin rule (`r2`) beats the bird rule (`r1`).

## CLI Options

```bash
# Show only positive conclusions
spindle reason --positive penguin.spl

# Output as JSON
spindle reason --json penguin.spl

# Validate syntax without reasoning
spindle validate penguin.spl

# Show theory statistics
spindle stats penguin.spl
```

## Using as a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
spindle-core = { path = "crates/spindle-core" }
```

Basic usage:

```rust
use spindle_core::prelude::*;

fn main() {
    let mut theory = Theory::new();

    // Add facts
    theory.add_fact("bird");
    theory.add_fact("penguin");

    // Add defeasible rules
    let r1 = theory.add_defeasible_rule(&["bird"], "flies");
    let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");

    // Set superiority
    theory.add_superiority(&r2, &r1);

    // Reason
    let conclusions = theory.reason();

    for c in conclusions {
        println!("{}", c);
    }
}
```

## Next Steps

- [Concepts](concepts.md) - Understand defeasible logic fundamentals
- [SPL Reference](reference/spl.md) - Complete SPL syntax
- [Examples](guides/grounding.md) - Advanced examples with variables
