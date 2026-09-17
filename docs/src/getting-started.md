# Getting Started

This tutorial installs Spindle-Rust and runs a theory where a penguin exception defeats the usual bird rule.

## Installation

Use Rust 1.87 or newer (edition 2024).

### Building from Source

```bash
git clone https://git.anuna.io/anuna-research/spindle-rust
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
+d ~flies
-D flies
-D ~flies
-d flies
```

Key result: `+d ~flies` - Tweety defeasibly doesn't fly because the penguin rule (`r2`) beats the bird rule (`r1`).

## Next steps

The result `+d ~flies` completes this tutorial: the explicit priority resolves the conflict.

- [Inspect the theory](guides/inspect-theory.md) with CLI filters, JSON output, syntax checks, and statistics.
- [Rust library](integration/rust.md) covers programmatic theory construction.
- [Concepts](concepts.md) explains the proof tags and rule types.
- [SPL reference](reference/spl.md) describes the complete syntax.
- [Variables and grounding](guides/grounding.md) explains rules with variables.
