# Documentation fact check — 2026-09-17

Audited the documentation working tree on `docs/update-changelog`, based on
implementation commit `7eb7c93`. Checked 23 selected claims about reasoning,
queries, temporal support, aggregation, trust, integration, and attribution.
Fourteen were supported by the checked evidence; nine old claims were contradicted
and corrected. This is a targeted audit, not certification of every book sentence.

## Elephant method

Used Elephant 0.1.7 with an isolated local store:

```sh
export ELEPHANT_HOME=/private/tmp/spindle-docs-factcheck-elephant
elephant -t spindle-docs-factcheck status
elephant -t spindle-docs-factcheck explain '(not (verified defeater-direction-old))'
```

Theory ID: `848b38787eef85104e29905f63d65e97833c558aa9b6cf3851840617c399969e`.
The private signing key remains in the temporary store and is not part of these
artifacts. The store is ephemeral; the exported evidence and theory are retained here.

Evidence was collected from executable CLI counterexamples, repository source,
existing regression tests, and official CSIRO pages. Elephant did not retrieve or
independently validate these sources. It reasoned over explicit evidence assertions:
source-supported or reproduced evidence supports a defeasible `verified` label;
a contrary `contradicted` assertion defeats that label. Corrections derive
`resolved` without erasing the old claim's contradiction. No claim becomes verified
merely because no counterexample was found.

Artifacts:

- [Claim/evidence register](2026-09-17-factcheck-evidence.json).
- [Portable SPL evidence theory](2026-09-17-factcheck.spl).
- [Elephant conclusion snapshot](2026-09-17-elephant-status.json).

The exported theory can also be evaluated with:

```sh
cargo run -p spindle-cli -- reason docs/audits/2026-09-17-factcheck.spl --positive
```

## Corrections

| Old claim | Verified replacement | Evidence |
|---|---|---|
| A defeater headed `flies` blocks `flies`. | A defeater attacks its head's complement; use `(not flies)` to block `flies`. | CLI counterexamples with both head polarities; `reason/defeasible.rs`. |
| A `(given (not guilty))` fact is an overridable default. | Facts establish definite proof; use an empty-body defeasible rule and explicit priority for an overridable presumption. | CLI fact/default counterexamples. |
| An unknown literal necessarily has `-d` tags. | Cycles can be undecided without negative tags; absent literals are not automatically listed in conclusions. | Strict self-loop CLI example; `traditional_dl.rs`. |
| Every fired strict rule establishes `+D`. | Definite premises establish `+D`; merely defeasible premises can support only `+d`. | CLI strict rule following a defeasible rule. |
| Ordinary negation is stratified negation. | Ordinary SPL negation is explicit/strong; missing evidence does not satisfy a negative premise. | CLI bird/not-penguin example; literal polarity model. |
| An atemporal body requires a synthesized base conclusion to consume temporal evidence. | Family-aware body matching consumes temporal evidence directly. | CLI `(given (during p 1 10)) (always consume p q)` produces `+D q` without positive base `p`; `reason/mod.rs`. |
| Preparation inserts a `TemporalBridge` stage and reserves bridge labels. | The current pipeline has no such stage; family matching and projection tokens supply the behavior. | `pipeline/mod.rs`, `reason/mod.rs`, `projection/`. |
| SPL has no interval variables or interval propagation. | Whole-interval and endpoint variables are supported, including propagation into heads. | CLI interval-copy example; temporal propagation tests. |
| Allen constraints are not exposed in SPL. | All 13 relations are supported as body constraints; `within` names Allen's During relation. | CLI `before` example; all-relation parser and grounding tests. |

Updated the affected concept, grounding, temporal, SPL reference, troubleshooting,
and Rust integration pages. Also made the specificity explanation explicit and
removed the remaining arbitrary-precision decimal wording from the root README.

## Supported current claims

Executable checks support aggregate row multiplicity, constructive cycle behavior,
ambiguity blocking, contradictory definite facts without explosion, strong negation,
and the corrected defeater/default examples. Source inspection supports the current
WASM result structure, verified `requires` implementation, separate metadata stores,
trust diminishment pass, extension registry, and stated Lean proof boundary.

The CSIRO attribution is supported by its
[May 2026 newsletter](https://www.csiro.au/en/Newsletters/D61-NextGenConnect/2026-05),
which describes combining Data61 and Manufacturing as CSIRO Technology, and a
[current staff profile](https://people.csiro.au/T/R/Ronnie-Taib) identifying
CSIRO Technology as formerly Data61. The documentation now links the attribution
to the official announcement.

## Validation and limits

Ran six existing core regression suites: `traditional_dl`,
`allen_constraint_tests`, `temporal_propagation_tests`, `trust_integration_tests`,
`requires_verified_tests`, and `spl_aggregation`: **99 passed, 0 failed**.
CLI counterexamples and replacement examples also passed. The Elephant snapshot
contains 14 positive `verified` classifications and nine `resolved` corrections;
none of the nine contradicted claims is positively verified.

The mdBook build and generated internal-link check pass. The installed mdBook 0.5.2
needs the temporary `MDBOOK_OUTPUT__HTML__GIT_REPOSITORY_ICON=fas-code-fork`
override; the checked-in `fa-code-fork` remains for CI's pinned mdBook 0.4.51.

Lean's full verification gate and external-oracle suites were not rerun for this
documentation audit. Claims about their scope were checked against their source
and guides, not treated as fresh proof-run results. Browser WASM behavior was
checked against bindings and shared contracts, not executed in a browser here.
