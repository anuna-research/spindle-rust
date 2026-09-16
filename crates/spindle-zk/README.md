# spindle-zk

A zero-knowledge proof compiler for bounded Spindle policies. Experimental, not production-audited.

## Quick Start

Run the semantic and actual-proof tests:

```sh
cargo test -p spindle-zk
```

## Usage

```rust
use spindle_zk::{Claim, Policy, Tag, parse_facts};

let policy = Policy::compile("(normally r bird flies)", "(given bird)")?;
let claim = Claim::new("flies", Tag::Defeasibly)?;
let proof = policy.prove(&parse_facts("(given bird)")?, &claim)?;
let verified = policy.verify(&proof, &claim)?;
# Ok::<(), spindle_zk::Error>(())
```

The second compilation argument declares public candidate inputs. Proving hides which candidates are present, not the candidate vocabulary itself.
The proof establishes reasoning over unattested facts. It does not establish issuer authenticity or factual truth.

## Architecture

The compiler unrolls constructive four-tag inference into a witness-independent Boolean program.
Halo2 gates constrain each operation and operand copy. Poseidon constrains a randomized commitment to the same private input cells.
The verifier derives the circuit and transparent parameters from its trusted policy and expected claim.

Ground nonmodal rules and finite-domain stratified aggregates are implemented
and tested; broader source lowering is outside the supported proof profile.
This module has no independent cryptographic deployment approval.
Read [SECURITY.md](SECURITY.md) for the statement, threat model, backend
configuration, disclosure boundaries, and verification limitations.
Read [PERFORMANCE.md](PERFORMANCE.md) for reproducible commitment and dense
inference measurements, including their resource-limit caveats.
Requirements: [[SPEC-025-zero-knowledge-compiler]]. Implementation evidence: [[IMPL-025-zero-knowledge-compiler]].

## API Reference

`Policy::compile`, `Policy::prove`, and `Policy::verify` provide the library workflow.
`Policy` and `Proof` provide bounded JSON readers and writers. `Claim` identifies a public literal and one constructive tag.
`Program` exposes the inference-only compiler for differential testing.

## Aggregate profile

```spl
(aggregate-domain 0 1 2 3)
(normally total (agg ?n sum ?v (row ?v)) (total ?n))
```

With public input schema `(given (row 1)) (given (row 2))`, this policy can prove
`(total 3)` when both private facts are selected. Sum, count, min-of, max-of, and
explicit seeded/nonempty folds use completed defeasible snapshots. Distinct rows
are counted once even if multiple rules prove them. Strict aggregate rules retain
their defeasible snapshot premise: a computed result is not automatically `+D`.

The current proof profile requires `aggregate-domain` to bound variable grounding
and possible results. This is a proof resource contract, unlike the ordinary SPL
pipeline where that legacy declaration is not needed. It is public and included
in the policy identity. An out-of-domain result or checked integer overflow
prevents proving any claim, even if another guard would discard the affected rule.
Private-dependent row selection, fold arithmetic, and rule activation are all
constrained. Arbitrary registered host functions are rejected.

## CLI

Build with `cargo build -p spindle-cli --features zk`, then use:

```sh
spindle zk check policy.spl --inputs inputs.spl
spindle zk compile policy.spl --inputs inputs.spl --out policy.zk.json
spindle zk prove policy.zk.json --facts private.spl --claim '(total 3)' --out proof.json
spindle zk verify proof.json --policy policy.zk.json --expect '(total 3)'
```

The default tag is defeasible (`+d`). Output files must not already exist.
Neither proof validity nor the commitment authenticates the source of facts.

## Development

Run `cargo test -p spindle-zk` for semantics, witness-corruption, and actual proof tests.
Run `cargo clippy -p spindle-zk --all-targets -- -D warnings` for lint checks.
The repository workspace gates remain required before integration.

The direct Lean comparison is a separate gate; default Cargo tests do not run
it. It checks all four query tags against the Lean aggregate oracle, including
undecided cycles, direct priorities, private row selection, and chained folds:

```sh
(cd lean && lake build AggregationOracle)
cargo test -p spindle-zk --test lean_oracle -- --ignored
```

`SPINDLE_AGGREGATION_ORACLE` can point to an independently built executable.
Missing executables, malformed responses, and oracle errors fail this test.
Passing the Rust-only suite must not be reported as passing this gate.
