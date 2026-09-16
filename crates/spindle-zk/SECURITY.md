# Security boundary

This module is experimental. Passing tests and source review are not an
independent cryptographic deployment audit. Do not describe the implementation
as production-certified or as a formally verified compiler.

## Statement and trust

The verifier independently chooses a trusted policy and expected claim. A proof
establishes that some private subset of the policy's declared input facts derives
the requested constructive tag under the supported bounded semantics. It does
not establish that those facts are true, issuer-signed, current, complete relative
to an external database, or supplied by a particular person.

The policy source, input candidates, resource bounds, claim (including its
identifiers), and randomized fact commitment are public. Candidate fact values
are not hidden; their selection is hidden. Applications that need to hide values
must not put those values into a public candidate schema and assume they remain
private. There is no credential, holder-binding, or selective-disclosure layer.

## Cryptographic configuration

The workspace pins `halo2_proofs` 0.3.5 and `halo2_gadgets` 0.5.0. The implementation
uses the Halo2 IPA backend over Pasta curves, its Blake2b transcript, and OS
randomness. It does not use a mock prover for exported proofs.
The installed manifests for these backend crates and `halo2_poseidon` 0.1.0
declare `MIT OR Apache-2.0` licensing and the Zcash Halo2 repository as their
source. This records dependency metadata, not a legal compliance certification
or a current vulnerability-advisory audit. Normal dependency trees for the
feature-disabled CLI and for `spindle-wasm` contain neither `spindle-zk` nor
Halo2; the proof dependencies are opt-in for those consumers.

Source inspection of `halo2_proofs/src/plonk/prover.rs` confirms that
`create_proof` fills unusable advice rows with random field elements and assigns
random advice commitment blinds. This module passes `OsRng` to that API. The
backend's `poly/commitment.rs` derives generators with hash-to-curve in
`Params::new`; proof packages cannot choose replacement parameters or keys.
The verifier regenerates the key from the trusted public policy and claim.

The fact commitment is a chain of the gadget's `P128Pow5T3` Poseidon hashes over
a fresh random field nonce, both 128-bit limbs of the policy's SHA-256 identity,
and every declared input-presence bit in canonical order. Circuit equality
constraints link those bits to inference inputs. The nonce is not serialized.
The security target is at least 128-bit classical computational security under
the backend and hash assumptions, not post-quantum security or an independently
proved bound for this composition.

The public instance binds the policy identity, selected literal index, tag, and
fact commitment. The selected output is constrained to one. Arithmetic is built
from constrained Boolean operations rather than unchecked field wraparound.
Inference packs four operations per row into independently selected lanes;
cross-lane operands remain equality-constrained. Public cells start after the
last complete or partial inference row. Poseidon state reuses the first lane's
advice columns in separate regions, with independent selectors.
Witness-free synthesis copies only public program data,
not private witness values. Circuit mutation and real-proof tests exercise this
shared-column configuration; performance measurements made before the change
are identified as historical baselines.

Before generating parameters or keys, verification checks the transcript's exact
byte count using public-layout measurement. The pinned Halo2 cost estimator
includes one unused lookup-opening scalar for this no-lookup configuration;
the implementation subtracts that 32-byte overcount. Every exported proof checks
the prediction against its actual transcript. This configuration-specific rule
must be revisited when the circuit or backend changes. Layout measurement itself
allocates memory and performs synthesis; this preflight is not constant-cost.
After measuring length, a typed encoding pass uses Halo2's own point/scalar
readers over the pinned single-proof, no-lookup transcript grammar. It rejects
noncanonical internal encodings before generating parameters or keys. The
grammar derives commitment counts from this circuit's advice/equality columns
and degree, and uses its actual opening sets and IPA tail. Every generated proof
must pass the same encoding check before export. This is encoding recognition,
not an alternative to cryptographic verification; backend or layout changes
require reviewing both the length model and this grammar.

## Semantic boundary

Constructive `-D` and `-d` proofs are not inferred merely from missing positive
proofs. Inference starts from the empty tag state and is unrolled to a finite
completion bound; arbitrary self-supporting fixed points are not accepted.
Before circuit synthesis, the compiler retains only the selected public claim's
transitive dependency cone, every declared input bit, and global aggregate
validity checks. This pruning depends only on the public policy and claim;
it never uses private fact selection. The complete input vector remains bound
by the commitment, including facts irrelevant to the selected conclusion.

For aggregates, candidate enumeration is public. Completed-stratum `+d` wires
select distinct rows. Checked signed integer folds determine activation of
candidate rule instances inside the circuit. Disabled instances cannot attack,
support, or obstruct negative proofs. An internal defeasible snapshot premise
prevents aggregate-dependent strict rules from silently acquiring definite
support. Arithmetic failure is constrained into every user output.

The aggregate profile requires an explicit public finite domain. This differs
from the ordinary unbounded/source-driven SPL pipeline. Out-of-domain results
and exceeded bounds fail closed; they are not negative conclusions. Unsupported
custom functions, modal/temporal reasoning, and trust-weighted inference are
not evaluated outside the proof and silently trusted.

## Verification evidence and limits

Tests cover real serialized proofs, changed statements/commitments/proof bytes,
private input rejection, malformed packages, integer boundaries, consistent
forged traces, and aggregate comparisons with the Rust reference. Deliberately
removing an AND constraint is detected by the forged-conjunction test.
Replacing aggregate snapshot membership with unconditional membership is
detected by the single-row membership regression. Both mutations were restored.

The direct Lean oracle test is separate and ignored by default; see the README
for its explicit command. Neither default Cargo test success nor a successful
Rust comparison implies that this separate test was executed. Lean's theorems
do not prove this Rust parser, compiler, Halo2 circuit, or proof serialization.

The cryptographic transcript has a witness-independent byte count for a fixed
policy/claim. Randomized bytes may have different lengths when rendered as
decimal JSON arrays. The code does not claim constant-time parsing, witness
construction, allocation, or proving; local timing, memory inspection, crashes,
and host compromise are outside the remote verifier privacy claim. Secret
buffers are not explicitly zeroized. Bounds limit work but are not a measured
service-level resource guarantee; hostile-policy verification should run with
application-level resource isolation.
