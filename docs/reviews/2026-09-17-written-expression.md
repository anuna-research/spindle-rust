# mdBook written-expression review

Review target: `409a9e2` on `docs/update-changelog`.
Review basis: the sibling `anuna-dev-skill` repository at `16a46f9`, particularly
PROTO-001's Controlled Language and Documentation Modes sections.
Scope: Tier 4 documentation review. No specification, execution theory, or code
change is needed for this review. The documentation maintainer is the next actor.

The book needs an editorial pass before it meets the requested writing standard.
The main failure is mixed reader purposes within pages. Sentence edits alone do
not resolve that structure. Keep the existing examples, restrictions, rationale,
and evidence when separating or rewriting passages.

## Findings

### P2 — Separate the aggregation reference from benchmarking

Location: `docs/aggregation-extensions.md:26`, included by
`docs/src/guides/aggregation.md`.

The page introduces `agg`, then starts benchmark commands and historical results.
Syntax and variable scope appear only after that detour. Readers looking up an
aggregate premise encounter a contributor performance task before its language rules.

Move the benchmark procedure to a focused how-to. Keep baseline measurements and
methodology in linked reference material. Keep syntax, scoping, empty-input
behavior, and limits together. Preserve the measured values and their caveats.

Protocol basis: Documentation Modes, one mode per page and link rather than digress.

### P2 — Give the first-run tutorial a bounded destination

Location: `docs/src/getting-started.md:107` and `:123`.

After the worked penguin result, the page becomes a CLI option sampler and then
a Rust integration exercise. The second route has no stated destination, setup
sequence, or expected observable result. The learner loses the sequential path.

Keep the first CLI run as the tutorial's destination. Move the option examples
and library integration into their existing reference/integration pages. Link
those pages as next steps. Preserve examples that are not already present there.

Protocol basis: Documentation Modes, tutorial destination, observable results,
and one action at a time without branches.

### P2 — Define aggregate terms before explaining the architecture

Location: `docs/src/internals/architecture.md:21` and `:73`.

The diagram introduces strata, completed prefixes, and snapshot lowering before
defining them. Those terms then explain why aggregate evidence has defeasible
strength. Readers need the implementation's vocabulary before they can understand
its design rationale.

Define a stratum as an evaluation stage ordered by dependencies. Explain the
completed earlier rule prefix and the evidence a snapshot records. Link the
full definitions at first use. Then explain why the inserted premise does not
establish a definite proof by itself.

Protocol basis: Removing Accidental Density, jargon before definition;
Documentation Modes, explanation answers why.

### P2 — Replace the circular description of confidence

Location: `docs/src/guides/queries.md:76`.

“A confidence field on the candidate” repeats the field name without explaining
its meaning. Readers cannot distinguish this value from candidate verification.

Document the actual behavior: the constructor initializes `confidence` to `1.0`,
and the current abduction implementation does not compute a calibrated probability.
Explain that `requires_with_options` verifies provability separately. The source
for this wording is `crates/spindle-core/src/query/abduce.rs:54` and the verified
requirements implementation.

Protocol basis: Documentation Modes, reference explains the machinery's fields;
Controlled Language, precise terminology.

### P2 — Separate lifecycle actions and put conditions first

Location: `docs/src/integration/wasm.md:108` and `:114`.

The lifecycle paragraph combines initialization, reuse, clearing, disposal,
rendering, and error presentation. Several independent actions share a sentence.
“When needed” leaves worker placement without a usable condition.

An appropriate revision starts:

> Initialize the module once. Reuse a `Spindle` object for repeated runs.
> Before rebuilding a theory programmatically, call `clear()`.
> When the owner is disposed, call `free()` to release the instance.

Keep rendering, visible loading/error states, worker placement, and measurement
as separate instructions. State an observable worker condition, such as reasoning
blocking UI responsiveness. Do not delete those obligations to shorten the passage.

Protocol basis: Controlled Language, condition before command and one instruction
per sentence. Procedural passages have a 20-word sentence limit.

### P2 — Declare page modes before applying sentence-level rules

Location: book-wide; representative sources are `docs/src/README.md:1`,
`docs/src/reference/cli.md:1`, and `docs/src/guides/performance.md:1`.

The checker reports missing `mode:` metadata throughout the inspected pages.
Pages also mix mode-specific voices. The CLI reference includes procedures, and
several explanation pages give instructions. Adding metadata alone does not fix
those passages.

Choose one reader purpose for each page. Move passages that serve another purpose
to linked pages. Declare the mode and apply its title convention. For example,
a task-focused performance page needs a “How to …” title. Check that metadata
stays out of the rendered book when implementing this change.

Protocol basis: Documentation Modes, declared mode, title form, and one mode per page.

### P3 — Split dense sentences and replace ambiguous modal wording

Locations: `docs/src/README.md:12`, `docs/aggregation-extensions.md:42`,
`docs/aggregation-extensions.md:99`, and `docs/src/reference/cli.md:212`.

The landing-page definition combines an introductory explanation with advanced
complexity qualifications. The benchmark coverage sentence combines several
independent test dimensions. These passages exceed the checker's descriptive
sentence limit. Split them without losing qualifications; a coverage table also
fits the reference material.

The corpus also uses lowercase obligation words where the protocol demands a
clear distinction. Do not capitalize every occurrence mechanically. For example:

> The result and contribution must be variables.

State the language rule directly: “The result and contribution are variables.”
Keep the separate requirement that the contribution occurs in the row pattern.

The CLI's `confirm` wording also conflicts with the controlled vocabulary.
Describe the operation directly: “The engine reruns reasoning with each candidate
fact-set. It retains candidates that establish the goal.”

Protocol basis: Controlled Language, sentence limits, modal force, and vocabulary;
Removing Accidental Density, introductory framing before advanced terminology.

## Evidence and review limits

Ran `usdd-lint.sh --doc --json --show` and the controlled-language checker over
the book sources. The aggregation and changelog includes were expanded by selecting
their source files. `SUMMARY.md` was excluded because it is navigation configuration.

The raw per-page results are in `2026-09-17-written-expression-lint.json`.
Getting Started, Performance Tuning, and Troubleshooting used procedural mode.
Other pages used descriptive mode as an initial classification. Mixed pages need
passage-level review; these baseline results do not establish full conformance.

The checker's condition warnings include false positives. “Check if the rule
exists” asks whether a condition holds; it is not a command with a trailing
condition. The report does not treat that warning as a writing defect. Quoted
domain terms also need context-sensitive review before changing modal words.

A separate, fresh-context reviewer checked selected entry, aggregation, query,
WASM, and architecture pages against the protocol. The findings above combine
that review with the checker output and local source inspection. No book content
was changed during this review. No code tests were needed for review artefacts.

## Resolution — 2026-09-17

The editorial revision addresses the findings above. The user explicitly waived
page-mode metadata; the sources and rendered book contain no `mode:` headers.
Page purpose still determines structure, voice, and sentence limits.
The original findings and baseline remain above for comparison.

- Aggregation syntax now lives in `docs/src/guides/aggregation.md`.
  `docs/aggregation-extensions.md` links to the relocated reference and related pages.
  Separate benchmark procedure and measurement pages retain all baseline values,
  coverage dimensions, timing boundaries, and caveats.
- Getting Started ends with the penguin result. It links the CLI inspection
  workflow and existing Rust reference. Local dependency declarations remain in
  the Rust reference; duplicate theory-construction examples are linked there.
- Architecture defines strata, completed prefixes, and snapshot lowering before
  its pipeline diagram. It explains the definite/defeasible evidence boundary.
- The query reference describes the constructor's `1.0` confidence value and
  distinguishes it from provability verification.
- The browser how-to separates initialization, reuse, clearing, disposal, rendering,
  errors, and worker placement. The API reference retains methods and return fields.
- The debugging workflow, verification commands, and disjunction rationale have
  separate pages. Dense prose and ambiguous modal wording were revised throughout.

A fresh review identified a stale benchmark anchor and an inaccurate rewrite of
WASM statistics availability. Both were corrected. The root README retains the
CI-pinned mdBook prerequisite in its documentation build guidance.

Local source inspection and CLI runs also resolved contradictory statements:
modal SPL works through the CLI; arithmetic head expressions fail with REQ-009;
a body binding produces the intended result. Unresolved defeasible conflicts
block both sides, while contradictory facts establish both definite proofs.
Mining causality records observed adjacency, not compliance across every trace.
The first-run tutorial now uses the CLI's `~flies` spelling.

### Recorded checks

The sibling skill's controlled-language checker reports zero errors and warnings
across 34 effective book pages. Tutorials and how-to pages use the procedural
limit; reference and explanation pages use the descriptive limit.
The changelog check uses its included root source. Navigation is excluded.
Results are in `2026-09-17-written-expression-revised-lint.json`.
Metadata checks are intentionally excluded under the user's instruction.
These checks support the editorial review; they do not prove prose quality.

The local mdBook 0.5.2 build passes with the existing Font Awesome icon override.
An HTML link scan checks 37 generated pages with zero broken internal links.
All four aggregate examples produce SPL keyword and variable highlighting.
CLI smoke checks pass for aggregate syntax, explicit folds, cycle behavior,
conflict priorities, the documented aggregate query, and first-run examples.
`git diff --check` passes. No engine code changed, so the Rust suite was not rerun.

The local preview remains available at `http://127.0.0.1:4312`.
Before committing, `make fmt-fix`, `make check`, and `make test` passed.
The test run passed 2,215 tests and 21 doctests; it skipped 33 tests and ignored 3 doctests.
The deployment follow-up also builds successfully with CI-pinned mdBook 0.4.51.
