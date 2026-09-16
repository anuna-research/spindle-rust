---
id: SPEC-025
title: Zero-knowledge proof compiler
status: implemented
version: 0.1.0
last-updated: 2026-09-16
---

# Zero-knowledge proof compiler

## Orientation

Intent: A policy author publishes a reusable policy. A holder proves a specified conclusion without disclosing private facts or intermediate reasoning.

Structure:

```text
SPL policy + public input schema -> compiler -> reusable policy
private facts + claim + policy   -> prover   -> proof
expected claim + trusted policy  -> verifier -> authenticated claim
```

The optional `spindle-zk` crate owns compilation, proving, and verification. The existing parser owns SPL recognition.
The ordinary proof relation follows [[DEFEASIBLE-LOGIC-SEMANTICS]]. Aggregates require proofs of snapshot selection and arithmetic as well as ordinary inference.

Decisions: [[SPEC-025-zero-knowledge-compiler#ADR-001]] isolates cryptographic dependencies; [[SPEC-025-zero-knowledge-compiler#ADR-002]] defines the compilation strategy.

Load-bearing: [[SPEC-025-zero-knowledge-compiler#REQ-001]], [[SPEC-025-zero-knowledge-compiler#REQ-002]], [[SPEC-025-zero-knowledge-compiler#REQ-003]], [[SPEC-025-zero-knowledge-compiler#REQ-007]].

Controls:
- Reject unsupported semantics and undeclared private facts: [[SPEC-025-zero-knowledge-compiler#REQ-004]].
- Never authenticate editable metadata as a proven claim: [[SPEC-025-zero-knowledge-compiler#REQ-002]].
- Never disclose private witnesses in proofs or normal diagnostics: [[SPEC-025-zero-knowledge-compiler#REQ-003]].
- Reject malformed packages and exhausted compilation budgets: [[SPEC-025-zero-knowledge-compiler#REQ-008]].
- Never describe unauthenticated facts as issuer-attested: [[SPEC-025-zero-knowledge-compiler#REQ-009]].
- Reject unsupported custom functions instead of evaluating them outside the proof: [[SPEC-025-zero-knowledge-compiler#REQ-007]].

Implementation evidence for aggregate circuits, benchmark-derived limits, and dependency evaluation is recorded in [[IMPL-025-zero-knowledge-compiler]].
Credential authentication and anonymous holder binding are separate features, consistent with the agreed CLI design.
Independent cryptographic review remains a release obligation owned by the maintainer; implementation tests do not establish production suitability.

Detail: Implementers follow contracts to tests; reviewers follow decisions and controls; policy users follow [[users/zk/user]] and [[users/zk/happy-paths]].

The key words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT, RECOMMENDED, MAY, and OPTIONAL follow BCP 14 only in capitals.

## Amendment Channels

The repository owner can amend this specification through direct task instructions or an accepted specification revision.
Implementation observations produce recorded proposals, not silent changes to proof guarantees.
Claim binding, witness privacy, and rejection of unsupported computation remain hard stops until a specification revision changes their contracts.

## Failure mechanisms

FM-001: Boolean witness checks accept arbitrary conclusions when inference constraints are absent.
FM-002: Editable query metadata misrepresents the statement that a cryptographic verifier actually checked.
FM-003: Independent reasoners disagree about negative evidence, cycles, defeat, and aggregate snapshots.
FM-004: Host-computed aggregates become unverified premises when only their lowered output enters a circuit.
FM-005: Duplicate input parsers or unbounded package fields cause divergent acceptance or resource exhaustion.

## Requirements

### REQ-001

The compiler SHALL enforce finite traditional four-tag inference, including strict rules, defeasible rules, defeaters, direct superiority, and undecided cycles.
Acceptance compares all tags against the existing reasoner and the Lean operational reference on their common fragment.
Source: FM-001 and FM-003; user request to build against merged branches.
Trace: [[SPEC-025-zero-knowledge-compiler#CON-001]], [[SPEC-025-zero-knowledge-compiler#TEST-001]], [[SPEC-025-zero-knowledge-compiler#TEST-002]].

### REQ-002

Verification SHALL bind the trusted policy, declared input universe, selected literal, proof tag, and fact commitment to the authenticated statement.
A verifier supplies the expected claim independently. Policy identity includes the semantics version and compilation bounds.
The circuit recomputes the commitment from the exact complete presence-bit vector used for inference and private blinding.
Commitment domain separation includes the public policy and schema identity.
The verifier derives keys and transparent parameters from the trusted policy; proof packages cannot supply replacement keys or parameters.
Source: FM-002.
Trace: [[SPEC-025-zero-knowledge-compiler#CON-002]], [[SPEC-025-zero-knowledge-compiler#TEST-003]], [[SPEC-025-zero-knowledge-compiler#TEST-004]].

### REQ-003

The proof package SHALL omit private facts, commitment blinding, and intermediate trace values.
The fact commitment uses fresh randomness to prevent dictionary enumeration of small private fact sets.
The public policy, public input universe, claim, and compilation bounds are explicitly disclosed.
The backend uses its zero-knowledge configuration and a hiding commitment targeting at least 128-bit computational security.
Public layout and proof length depend on public policy bounds, never private fact selection.
Backend configuration review supplies security evidence; package inspection alone does not establish zero knowledge.
Source: agreed UX and FM-002.
Trace: [[SPEC-025-zero-knowledge-compiler#CON-002]], [[SPEC-025-zero-knowledge-compiler#TEST-005]].

### REQ-004

The compiler SHALL reject features outside its declared proof profile before proof generation.
The prover rejects unknown private facts instead of silently dropping them. Presence and absence are distinct from explicit negation.
The input universe is public and fixed before proving; it does not vary with private witness contents.
Source: FM-003 and FM-004.
Trace: [[SPEC-025-zero-knowledge-compiler#CON-001]], [[SPEC-025-zero-knowledge-compiler#TEST-006]].

### REQ-005

The CLI SHALL expose `spindle zk check`, `compile`, `prove`, and `verify` through an optional build feature.
Text and JSON output carry equivalent results. Existing commands retain their behaviour when that feature is disabled.
Source: accepted UX sketch.
Trace: [[SPEC-025-zero-knowledge-compiler#CON-003]], [[SPEC-025-zero-knowledge-compiler#TEST-007]], [[SPEC-025-zero-knowledge-compiler#TEST-008]].

### REQ-006

The public Rust API SHALL expose the same supported compilation and proof operations as the CLI without invoking subprocesses.
Source: user request for a module.
Trace: [[SPEC-025-zero-knowledge-compiler#CON-001]], [[SPEC-025-zero-knowledge-compiler#CON-002]], [[SPEC-025-zero-knowledge-compiler#TEST-007]].

### REQ-007

Aggregate proofs SHALL constrain completed-stratum membership, row deduplication, selected contributions, seeds, and checked integer fold results.
The supported builtins include sum, count, minimum, and maximum over declared finite input universes.
Grounding and lowering that depend on private facts form part of the proven relation.
Custom host functions without proof implementations fail closed. Ordinary arithmetic cannot silently use finite-field wraparound for signed integer operations.
Source: FM-004 and the request to integrate with merged aggregate branches.
Trace: [[SPEC-025-zero-knowledge-compiler#CON-001]], [[SPEC-025-zero-knowledge-compiler#TEST-009]], [[SPEC-025-zero-knowledge-compiler#TEST-010]].

### REQ-008

Package readers SHALL reject unsupported versions, malformed encodings, inconsistent lengths, and configured resource-limit violations before expensive cryptographic work.
Compilation bounds are explicit and included in policy identity. Exhaustion is an error, never a negative conclusion.
Source: FM-005.
Trace: [[SPEC-025-zero-knowledge-compiler#CON-002]], [[SPEC-025-zero-knowledge-compiler#TEST-011]].

### REQ-009

Verification SHALL report fact authenticity as unattested unless a separately specified attestation proof is verified.
Proof validity alone implies neither factual truth nor issuer authorization.
Source: agreed UX boundary.
Trace: [[SPEC-025-zero-knowledge-compiler#CON-003]], [[SPEC-025-zero-knowledge-compiler#TEST-012]].

## Decisions

### ADR-001

Place the module in `crates/spindle-zk`, with an optional CLI dependency.
Existing components supply parsing, theory types, and differential reference results. They do not supply cryptographic constraints.
This placement keeps normal reasoning and WASM consumers free of mandatory proving dependencies.
The composition-first check rejects copying the old parser, placeholder signature verifier, and incomplete Halo2 circuit.

### ADR-002

Compile the bounded proof relation into a deterministic constraint program independent of witness values.
Start with the four-tag operational rules, then compose bounded aggregate computation into the same relation.
The proving backend uses maintained Halo2 dependencies with genuine hash gadgets and no prover-selected trusted setup.
An algebraic trace evaluator assists witness generation; it is not an authority the verifier trusts.
Both key generation and proving synthesize the identical layout, fixed values, selectors, and copy constraints.
Neither truth-table enumeration of all private inputs nor unconstrained host lowering substitutes for the aggregate compiler.

## Contracts

### CON-001

Interface: `check`, `compile`, and compiled-policy preparation through typed Rust APIs.
Input grammar: existing SPL grammar, recognized by `spindle_parser::parse_spl`, followed by profile validation.
Private-input declarations use a separate SPL fact theory; their literals specify allowable presence bits, not mandatory truths.
Preconditions: bounded, recognized inputs and supported semantics.
Postconditions: deterministic policy identity and witness-independent relation; unsupported features yield structured errors.
Implements: [[SPEC-025-zero-knowledge-compiler#REQ-001]], [[SPEC-025-zero-knowledge-compiler#REQ-004]], [[SPEC-025-zero-knowledge-compiler#REQ-006]], [[SPEC-025-zero-knowledge-compiler#REQ-007]].
Verified by: [[SPEC-025-zero-knowledge-compiler#TEST-001]], [[SPEC-025-zero-knowledge-compiler#TEST-006]], [[SPEC-025-zero-knowledge-compiler#TEST-009]].

### CON-002

Interface: policy proof generation and verification against an explicit expected claim.
Input grammar: versioned JSON objects recognized by serde with unknown-field rejection; byte arrays have bounded, canonical lengths.
Preconditions: validated policy and fact schema; canonical claim in the policy vocabulary.
Postconditions: a verified result contains authenticated public values only, or a structured error.
Errors distinguish unsupported features, malformed input, unestablished claims, invalid proofs, policy mismatch, and resource exhaustion.
Implements: [[SPEC-025-zero-knowledge-compiler#REQ-002]], [[SPEC-025-zero-knowledge-compiler#REQ-003]], [[SPEC-025-zero-knowledge-compiler#REQ-008]].
Verified by: [[SPEC-025-zero-knowledge-compiler#TEST-003]], [[SPEC-025-zero-knowledge-compiler#TEST-004]], [[SPEC-025-zero-knowledge-compiler#TEST-005]], [[SPEC-025-zero-knowledge-compiler#TEST-011]].

### CON-003

Interface: `spindle zk <check|compile|prove|verify>`.
Clap recognizes command arguments. The existing parser recognizes policy and fact files.
`compile` accepts a policy and public input schema. `prove` accepts private facts and a claim. `verify` requires the expected policy and claim.
No default behaviour reveals private facts. Claim disclosure includes identifiers appearing in the claim.
Output paths are explicit; failed operations leave no successful-looking proof package.
Implements: [[SPEC-025-zero-knowledge-compiler#REQ-005]], [[SPEC-025-zero-knowledge-compiler#REQ-009]].
Verified by: [[SPEC-025-zero-knowledge-compiler#TEST-007]], [[SPEC-025-zero-knowledge-compiler#TEST-008]], [[SPEC-025-zero-knowledge-compiler#TEST-012]].

## Verification cases

| Artefact | Type and required evidence | Validates |
|---|---|---|
| TEST-001 | Core: generated ground theories agree on all four tags, including cycles and direct priorities | [[SPEC-025-zero-knowledge-compiler#REQ-001]] |
| TEST-002 | Core: corrupted inference witnesses fail circuit verification | [[SPEC-025-zero-knowledge-compiler#REQ-001]] |
| TEST-003 | Core: actual proof generation and verification succeeds across independent serialization roundtrips | [[SPEC-025-zero-knowledge-compiler#REQ-002]] |
| TEST-004 | Core: changed claims, policies, commitments, and proof bytes fail verification | [[SPEC-025-zero-knowledge-compiler#REQ-002]] |
| TEST-005 | Core: package schema excludes secrets; repeated proofs randomize commitments | [[SPEC-025-zero-knowledge-compiler#REQ-003]] |
| TEST-006 | Core: unsupported features and undeclared facts produce errors | [[SPEC-025-zero-knowledge-compiler#REQ-004]] |
| TEST-007 | Core: library and four-command CLI roundtrip agree in text and JSON modes | [[SPEC-025-zero-knowledge-compiler#REQ-005]], [[SPEC-025-zero-knowledge-compiler#REQ-006]] |
| TEST-008 | Core: default workspace tests and feature-off builds retain existing behaviour | [[SPEC-025-zero-knowledge-compiler#REQ-005]] |
| TEST-009 | Depth: private aggregate rows, defeat, grouping, duplicates, seeds, and chained strata match the reference | [[SPEC-025-zero-knowledge-compiler#REQ-007]] |
| TEST-010 | Depth: corrupted aggregate membership and arithmetic fail; overflow and unsupported functions fail closed | [[SPEC-025-zero-knowledge-compiler#REQ-007]] |
| TEST-011 | Core: malformed, oversized, and mutated packages fail without panic or success files | [[SPEC-025-zero-knowledge-compiler#REQ-008]] |
| TEST-012 | Core: verification reports unattested facts and never claims issuer authentication | [[SPEC-025-zero-knowledge-compiler#REQ-009]] |

Depth cases remain required for completion. The distinction sequences work; it does not remove requirements.
Each test group requires concrete Rust tests and a durable command/result record before acceptance.
Adversarial mutation targets missing inference constraints, public bindings, and aggregate membership constraints.
Independent review follows implementation; self-authored passing tests alone do not approve cryptographic deployment.

## Purity Boundary Map

Parsing, policy normalization, constraint generation, trace evaluation, and verification contain no filesystem operations.
Proving obtains randomness through an explicit cryptographic RNG boundary.
The CLI owns file access and output rendering. Library modules do not import CLI modules.

## Evidence

The implementation branch starts from `7eb7c93`, which merges PRs #38 and #39.
The Lean operational rules reside in `lean/Spindle/Aggregation/Operational.lean`.
Bounded implementation and verification are complete; evidence and deployment limitations are recorded in [[IMPL-025-zero-knowledge-compiler]]. Fresh-context comprehension passed; the specification incorporates the independent review's commitment, key-trust, and disclosure findings.
