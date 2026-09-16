---
id: IMPL-025
title: Zero-knowledge compiler implementation and evidence
status: implemented
---

# Implementation ledger

Governing requirements: [[SPEC-025-zero-knowledge-compiler]]. Executable plan: `plans/IMPL-025-zero-knowledge-compiler.spl`.

| Task | Owner | Prerequisite | Acceptance |
|---|---|---|---|
| semantics | implementer | specification review | [[SPEC-025-zero-knowledge-compiler#TEST-001]] |
| cryptography | implementer | semantics | [[SPEC-025-zero-knowledge-compiler#TEST-002]], [[SPEC-025-zero-knowledge-compiler#TEST-003]], [[SPEC-025-zero-knowledge-compiler#TEST-004]], [[SPEC-025-zero-knowledge-compiler#TEST-005]] |
| aggregates | implementer | cryptography | [[SPEC-025-zero-knowledge-compiler#TEST-009]], [[SPEC-025-zero-knowledge-compiler#TEST-010]] |
| interfaces | implementer | cryptography | [[SPEC-025-zero-knowledge-compiler#TEST-006]], [[SPEC-025-zero-knowledge-compiler#TEST-007]], [[SPEC-025-zero-knowledge-compiler#TEST-008]], [[SPEC-025-zero-knowledge-compiler#TEST-011]], [[SPEC-025-zero-knowledge-compiler#TEST-012]] |
| acceptance | independent reviewer and implementer | aggregates, interfaces | complete test attribution, mutation evidence, workspace checks, documentation |

## Evidence log

- 2026-09-16: Branch `feat/zero-knowledge-compiler` created at merged commit `7eb7c93`. User untracked files remain untouched.
- Source inspection: the Lean operational reference uses direct superiority and constructive negative proofs. The old ZK witness algorithm is unsuitable.
- Dependency inspection: `halo2_proofs` 0.3.5 and `halo2_gadgets` 0.5.0 are available from the Zcash Halo2 repository through crates.io.
- Fresh-context specification review identified commitment-to-witness linkage, verification-key trust, zero-knowledge configuration, and schema disclosure gaps. The specification now includes those obligations.
- Red gate: `cargo test -p spindle-zk --test semantics` executed the stub and failed semantic cases with `inference compiler not implemented`.
- After implementing the bounded four-tag relation, the same command passed curated cases and generated theory comparisons.
- These tests establish progress on [[SPEC-025-zero-knowledge-compiler#TEST-001]], not completion of the compiler or aggregate obligations.

## Gates

### Initial circuit and recognizer evidence, 2026-09-16

- At this initial checkpoint, `cargo test -p spindle-zk` passed the semantic differential, actual proof roundtrip/tampering, private-error redaction, qualifier rejection, and individual-wire corruption tests. Aggregates were not implemented at that checkpoint; the later evidence below supersedes that status.
- Added a consistent forged-trace test for [[SPEC-025-zero-knowledge-compiler#TEST-005]]: private facts contain only one conjunction premise, while the forged trace evaluates every AND as OR and recomputes all consumers. The public commitment still binds the honest inputs. The circuit rejects this trace.
- Mutation gate: temporarily removed `config.and.enable` and ran `cargo test -p spindle-zk --lib consistently_forged_conjunction_is_rejected`. The test executed and failed with `accepted an OR trace for an AND policy`. Restored the selector; the same test passed. No mutation remains in the implementation.
- `cargo test -p spindle-parser --quiet` passed after correcting an old regression expectation that accepted `(always and body head)` by silently discarding `head`. Full recognition now rejects this malformed input rather than constructing a different rule.
- `cargo clippy -p spindle-zk -p spindle-cli --features spindle-cli/zk --all-targets -- -D warnings` passed. This is scoped lint evidence, not a workspace or cryptographic release approval.

### Current acceptance evidence

Commands below use `CARGO_INCREMENTAL=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_DEV_DEBUG=0`
to avoid exhausting local disk with build artifacts. Tests run against the working
branch, not a committed release. The current implementation includes finite-domain
aggregates and public-claim dependency pruning. Pruning preserves all declared
commitment inputs and global aggregate validity; it never depends on private facts.

| Verification case | Result | Authoritative evidence and scope |
|---|---|---|
| [[SPEC-025-zero-knowledge-compiler#TEST-001]] | pass | `tests/semantics.rs`: generated and curated Rust comparisons; explicitly executed `cargo test -p spindle-zk --test lean_oracle -- --ignored`: direct ground and aggregate Lean comparisons, including all constructive tags. Lean executable was independently built from the repository. |
| [[SPEC-025-zero-knowledge-compiler#TEST-002]] | pass | `src/circuit.rs` tests reject corrupted projected-program wires, non-Boolean inputs, consistent forged conjunction traces, and recomputed traces with mismatched commitments. |
| [[SPEC-025-zero-knowledge-compiler#TEST-003]] | pass | `tests/proofs.rs` and `tests/aggregates.rs`: actual Halo2 proofs survive policy/proof serialization and verify against independently supplied claims. |
| [[SPEC-025-zero-knowledge-compiler#TEST-004]] | pass | `tests/proofs.rs`: changed policy, claim, commitment, proof bytes, aligned truncation, and aligned extension are rejected. |
| [[SPEC-025-zero-knowledge-compiler#TEST-005]] | pass | Package-field inspection, fresh commitment randomness, and equal transcript lengths across private selections; configuration source review recorded in `crates/spindle-zk/SECURITY.md`. No claim of constant-time host execution or production cryptographic certification. |
| [[SPEC-025-zero-knowledge-compiler#TEST-006]] | pass | `tests/semantics.rs`, `tests/proofs.rs`, and `tests/aggregates.rs`: unsupported qualifiers/functions, undeclared or temporal facts, malformed typed aggregate facts, and invalid claims fail closed. |
| [[SPEC-025-zero-knowledge-compiler#TEST-007]] | pass | Feature-enabled `crates/spindle-cli/tests/zk.rs`: all CLI operations, text/JSON results, aggregate workflow, and library interoperability. |
| [[SPEC-025-zero-knowledge-compiler#TEST-008]] | pass | Fresh default-feature `cargo test --workspace --no-fail-fast` exited successfully after claim projection and the additional adversarial tests, including workspace doctests. Default-ignored tests remain separate obligations. |
| [[SPEC-025-zero-knowledge-compiler#TEST-009]] | pass | `tests/aggregates.rs`: private selection, grouping, strict snapshot strength, duplicate rows, seeds, undecided cycles, defeat, and chained strata agree with the finite Rust reference; direct Lean oracle also exercises aggregates. |
| [[SPEC-025-zero-knowledge-compiler#TEST-010]] | pass | Checked integer boundary/property tests and forged-validity circuit test; aggregate overflow/domain rejection. Replacing snapshot membership with unconditional membership made `aggregate_membership_requires_snapshot_evidence` fail with an absent row counted. Mutation restored. |
| [[SPEC-025-zero-knowledge-compiler#TEST-011]] | pass | `tests/packages.rs`, `tests/resources.rs`, bounded parser tests, and CLI tests exercise malformed/oversized/escaped packages, configured limits, no overwrite, redaction, and no successful-looking output on failure. No worst-case latency guarantee follows. |
| [[SPEC-025-zero-knowledge-compiler#TEST-012]] | pass | Library `VerifiedClaim.fact_authenticity` is `Unattested`; actual-proof regression asserts it and CLI workflows assert matching text/JSON disclosure. |

Latest combined scoped run: `cargo test -p spindle-zk -p spindle-cli --features spindle-cli/zk --no-fail-fast`
exited successfully after claim projection. Subsequent projection-specific regression,
projected-circuit corruption tests, direct Lean comparison, and
`cargo clippy -p spindle-zk --all-targets -- -D warnings` also passed.
The default-ignored dense compilation test was explicitly executed earlier;
the direct Lean test is never inferred from default Cargo test success.

Fresh-context pruning review found no blocking defect in reverse dependency
closure, ascending wire remapping, original literal indices, complete commitment
inputs, or aggregate validity preservation. Its suggested adversarial regressions
are now implemented and passed: a forged global-validity wire cannot prove an
independent public claim when an unrelated fold overflows; changing an irrelevant
private input while holding nonce and public commitment fixed is rejected.
These are targeted review results, not final acceptance approval.

Before the latest circuit packing, `make check` exited successfully. A workspace test refresh subsequently
failed during compilation with `No space left on device`, not a test assertion.
After `cargo clean` removed only regenerable Cargo output, the full workspace
test command was restarted and exited successfully. The updated module also
passed `cargo clippy -p spindle-zk --all-targets -- -D warnings`.
The user's build-location constraint is explicit: all builds remain on the
system disk; no temporary cache or output was placed on an external volume.

Vault validation: `zetl check --dead-links` reports only existing links from
[[SPEC-024-predicate-model-and-vocabulary]] to `usdd-agent-protocol`.
`zetl check --fail-on error` exits unsuccessfully on SPL examples in that document
and [[SPINDLE-CONTRACT]] that the installed zetl parser does not recognize
(`predicate`, structured `meta` label, and `rule`). These unrelated specifications
were not changed. No clean whole-vault gate is claimed.

### Resource evidence and remaining work

Reproducers and measured outputs live in `crates/spindle-zk/PERFORMANCE.md` and
the crate's `profile` and `dense_profile` examples. Before pruning, an optimized
73,346-operation dense policy completed proving in 35,425 ms and verification in
28,336 ms. The 294,146-operation boundary policy was stopped during proving under
severe whole-host memory pressure. The host was not isolated, so those observations
are not a precise prover peak-memory measurement.

Claim pruning reduces that dense boundary to 292,134 operations. An initial rerun
was stopped deliberately, but a later two-worker run completed: compilation
28 ms, proving 373,250 ms, verification 262,960 ms, transcript 3,424 bytes,
exit status 0. This includes fresh parameter/key generation in each API operation.
macOS reported maximum resident set size 3,972,579,328 bytes and peak memory
footprint 11,767,227,296 bytes; these distinct metrics are not interchangeable.
The executable used the pre-column-sharing circuit. Host activity was not
isolated, so these are reproducible workload observations, not a throughput SLA.

Subsequent circuit changes share Poseidon state with inference columns and pack
operations into independently selected lanes. The one-million-operation layout
fixture with two lanes synthesizes at `k=19`, predicted transcript 3,456 bytes, measurement
1,934 ms, without reducing the operation ceiling. Actual ground/aggregate proofs,
corruption tests, odd/even final-row and same-row cross-lane tests pass. Focused
independent review found no blocker in selectors, operand copies, public-cell
placement, witness independence, or proof-length correction. These are component
checks; final acceptance remains separate.

The ignored `maximum_budget_proof_roundtrip` was executed from an optimized
test executable with `RAYON_NUM_THREADS=2` and OS peak-memory measurement. It
tests a synthetic backend envelope, not a source-program semantic theorem.
Key generation completed in 272,932 ms, but the process terminated abnormally
without a proof result or assertion diagnostic. The wrapper exited 1 after
316.90 seconds, reporting maximum RSS 3,488,645,120 bytes and peak memory
footprint 11,776,746,688 bytes. Although the wrapper's signal diagnostic was
unusable, the unified system log confirms a memory-pressure kill: the kernel's
2026-09-16 16:35:54.928 `memorystatus` entry identifies this executable and PID
16287 as the largest compressed process, at 9741 MB.

The subsequent four-lane layout preserves the operation budget and measures
`k=18`, predicted transcript 4,288 bytes, and 1,919 ms layout measurement.
All active module tests passed, including boundary lengths before/at/after a
packed row, actual proofs, and typed transcript rejection. The direct Lean
oracle was explicitly rerun and passed. The workspace test refresh passed
before this four-lane change; at that checkpoint the final workspace/CLI refresh
was pending. The final passing results below supersede that checkpoint.
Feature-enabled CLI workflows and `make check` also passed for four lanes.
Independent source review found no blocker in the four-lane constraints or
transcript grammar: advice/permutation counts, degree, opening sets, row
placement, and proof-length correction were checked against the pinned backend.
The subsequent maximum-budget real proof passed: key generation 133,747 ms,
proof creation 35,039 ms, independent verification-key reconstruction plus
verification 10,587 ms, transcript 4,288 bytes, total wall time 180.00 seconds.
Maximum RSS was 7,129,333,760 bytes and peak footprint 8,926,386,872 bytes.
Verification reused transparent parameters; the public API regenerates them.
This is a synthetic backend envelope with observed costs, not a universal
latency or memory guarantee. A small regression build overlapped the tail;
host activity was not isolated. Full details remain in the performance report.

The broader acceptance review found additional contract/evidence gaps. The
implementation now distinguishes `PolicyMismatch` from `InvalidProof` and maps
it to `ZK_POLICY_MISMATCH`; library and CLI assertions passed. Tag-only tampering
is rejected independently of literal tampering. Custom reducer, extraction, and
binding rejection now have explicit tests for empty and declared row schemas;
declared aggregate inputs now obey the predicate-arity ceiling. Those aggregate
tests passed.

For [[SPEC-025-zero-knowledge-compiler#REQ-008]], an exact-length proof now also
undergoes typed point/scalar recognition before parameter/key generation.
The recognizer follows the pinned single-proof/no-lookup transcript grammar and
uses the backend's canonical readers. Proving self-checks every generated
transcript against this grammar. Actual proofs, CLI workflows, and a test
replacing each transcript word in turn with a noncanonical encoding passed.
Focused source review confirmed the pinned grammar and rejection ordering,
including commitment counts, opening sets, IPA tail, and the exact-length
precondition. It found no blocker; backend/layout changes still require a new
review of this configuration-specific recognizer.

Final independent review found a canonical-claim defect for bare zero-arity
special-form names. The added regression failed before the fix; canonicalization
now retains their bare spelling. Positive and explicitly negated forms pass
reparsing, claim projection, and actual proof/verification. The reviewer accepted
the fix and found no further functional/security blockers.

Final `cargo test --workspace --features spindle-cli/zk --no-fail-fast` exited
successfully, including CLI workflows and doctests. `make check` and the
explicit direct Lean oracle rerun also passed after the canonicalization fix.
Implementation acceptance is complete for this bounded experimental profile.
Open: The maintainer owns independent cryptographic review before deployment;
hostile-policy services must provide application-level resource isolation.
The dependency/backend source review is recorded in `crates/spindle-zk/SECURITY.md`;
independent deployment approval remains the maintainer's separate release task.

| Gate | Result | Evidence or owner |
|---|---|---|
| Specification review | reviewed with amendments | fresh-context spec_review agent; commitment/key/privacy/disclosure findings incorporated |
| Semantic differential tests | pass | Rust property/curated tests and explicitly executed direct Lean oracle |
| Actual proof roundtrip and tampering | pass | real Halo2 ground and aggregate tests; malformed transcript rejection |
| Aggregate circuits | pass | finite-domain selection, deduplication, checked folds, conditional rules, mutation evidence |
| CLI and package tests | pass | actual feature-enabled CLI/library workflows and bounded package tests |
| Workspace checks | pass | final feature-enabled workspace tests/doctests, `make check`, and explicit Lean oracle rerun passed |
| Vault-wide validation | existing errors | unrelated SPL examples in [[SPEC-024-predicate-model-and-vocabulary]] and [[SPINDLE-CONTRACT]]; no new dead links reported for this module |
| Benchmark-derived operational bounds | pass with measured limits | dense source-policy boundary and four-lane million-operation backend proof passed; observed costs and prior memory-pressure failure retained in PERFORMANCE.md; no universal resource SLA |
| Final independent acceptance review | accepted | reviewer accepted the bounded experimental implementation after final evidence refresh; no remaining functional/security implementation blockers; nonblocking stale status wording corrected |
| Independent cryptographic release review | unverified | maintainer; no production readiness claim |

The bounded implementation and its verification obligations are fulfilled;
independent cryptographic deployment approval is not claimed.
