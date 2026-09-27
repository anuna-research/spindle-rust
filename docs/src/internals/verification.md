# Verification

The repository contains Lean models, proofs, and executable oracles alongside
Rust tests. The current standard, family, aggregate, and modal oracles use traditional
ambiguity-blocking DL(∂) with four constructive tags.

## What is checked

Lean modules cover arithmetic and grounding, temporal intervals and Allen
relations, query models, trust operations, and aggregation. Aggregate proofs
cover dependency checks, inferred strata, unordered row semantics, lowering,
fixed-point completion, and equivalence of completed prefixes for all four tags.

The single-head modal model covers obligations (`must`), explicit permissions
(`may`), and prohibitions (`forbidden`). Checked results include prohibition
normalization, symmetric opposition, distinct negation scopes, typed defense,
finite closure, saturation, and derivability. Without productive support, the
completed closure cannot produce a positive proof. Kernel-checked examples
cover preferred permission, compatible opposite permissions, and weak permission
without strong permission.

Rust differential tests compare supported inputs with executable Lean models.
The aggregate suite includes typed schemas, parsed SPL, grouping, empty input,
duplicate contributions, conflicts, priorities, and cycles. The modal suite
compares all four tags across rule kinds, preferences, defeaters, negated premises,
cycles, and temporal families, including reversed rule order.

These are proofs of the stated models and differential checks of the Rust
implementation, not a proof of whole-language Rust conformance. In particular:

- The modal model excludes nested/custom modalities, ordered reparation or
  permission chains, variable grounding, and aggregation. It does not prove
  normative consistency for arbitrary theories; opposing definite facts can coexist.
- Rust aggregate arithmetic uses checked `i64`; Lean uses exact integers.
  Intermediate overflow is outside an unqualified refinement claim.
- Custom extension implementations and their reducer laws are host contracts.
- SPL parsing and extension registration are tested integrations.
- Historical lambda-only and strengthened two-sided models retain their own
  proofs; those results do not establish properties of the current standard backend.

## Verification gate

The Lean gate builds libraries and oracle executables with warnings as errors.
It rejects admitted proofs and local axioms, checks vacuity, and audits theorem axioms.
The gate uses the Lean toolchain specified under `lean/`.
Ordinary Cargo tests do not perform these checks.

[Running verification checks](../guides/check-verification.md) gives the commands and external-oracle test examples.

See the repository's
[Lean guide](https://git.anuna.io/anuna-research/spindle-rust/src/branch/main/lean/README.md),
[proof catalogue](https://git.anuna.io/anuna-research/spindle-rust/src/branch/main/lean/PROOFS.md),
[modal proof guide](https://git.anuna.io/anuna-research/spindle-rust/src/branch/main/lean/MODAL.md),
and [aggregate proof guide](https://git.anuna.io/anuna-research/spindle-rust/src/branch/main/lean/AGGREGATION.md)
for theorem statements, hypotheses, and the full suite list.
