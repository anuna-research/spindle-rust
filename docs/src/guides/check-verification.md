# How to run verification checks

Use the Lean toolchain specified under `lean/`.
Run these checks from the repository root:

```sh
make check
make test
scripts/check-lean-verification.sh
```

The Lean gate builds the oracle executables.
After the gate succeeds, run the external-oracle tests explicitly.
For example:

```sh
cargo test -p spindle-core --test lean_aggregation_oracle_difftest -- --ignored --nocapture
cargo test -p spindle-core --test lean_arith_oracle_difftest -- --ignored
```

Each test compares Rust results with the corresponding executable Lean model.
A passing run reports no mismatches for the tested inputs.

The repository guides record theorem statements, hypotheses, and the full suite list:

- [Lean guide](https://git.anuna.io/anuna-research/spindle-rust/src/branch/main/lean/README.md)
- [Proof catalogue](https://git.anuna.io/anuna-research/spindle-rust/src/branch/main/lean/PROOFS.md)
- [Aggregate proof guide](https://git.anuna.io/anuna-research/spindle-rust/src/branch/main/lean/AGGREGATION.md)

[Verification](../internals/verification.md) explains the scope of these checks.
