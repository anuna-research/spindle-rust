---
id: SPEC-025
title: Linked-Data and Semantic-Web Ingestion (A-Box Bridge)
status: draft
version: 0.4.1
created: 2026-07-14
last-updated: 2026-07-14
authors: Claude (Opus 4.8, AI agent)
reviewers: Core Maintainers (pending); adversarial model review round 1 (addressed)
protocol: USDD Agent Protocol v1.11.0
---

# SPEC-025: Linked-Data and Semantic-Web Ingestion (A-Box Bridge)

## Orientation

**Intent:** Let Spindle ingest instance data (RDF triples / linked data) from
files, SPARQL endpoints, and dereferenceable IRIs, turning it into a
defeasible-reasoning-ready [[SPEC-025-linked-data-ingestion#Theory|theory]] of
facts — building on the
[[SPEC-024-predicate-model-and-vocabulary|SPEC-024 predicate model]] rather
than inventing a parallel ontology subsystem.

**Metaphor:** A triple is a ground binary (or, for `rdf:type`, unary) fact. A
linked-data *source* is a claimant: its triples enter a
[[SPEC-025-linked-data-ingestion#Claims Provenance|claims block]] so every fact
is **attributable and trust-weightable**. This revision provides provenance
*annotation* — automatic conflict *reconciliation* between competing sources
requires core reasoner changes and is deferred
([[SPEC-025-linked-data-ingestion#Deferred Decisions]]). The ingestion policy
is **explicit triples only**: every asserted triple becomes a fact, but no
schema entailment runs and no schema translates to rules.

**Key decisions:**

- Ingest **explicit triples without schema entailment**. Every asserted
  triple — including RDFS/OWL schema triples — maps to a plain Spindle fact;
  no RDFS/OWL inference and no T-Box → rules
  ([[SPEC-025-linked-data-ingestion#ADR-003]]).
- **Class-aware mapping**: `(s rdf:type C)` → unary `C(s)`; every other
  `(s p o)` → binary `p(s o)`, honoring
  [[SPEC-024-predicate-model-and-vocabulary#ADR-001]] (unary ≈ class, binary ≈
  property) ([[SPEC-025-linked-data-ingestion#ADR-001]]).
- IRIs are kept **verbatim as internal identity**; CURIE compaction is a
  **presentation-only** transform through one global, deterministic registry
  ([[SPEC-025-linked-data-ingestion#ADR-002]]).
- The RDF-term → `Term` mapping is an **intentionally lossy projection** with
  documented collision cases
  ([[SPEC-025-linked-data-ingestion#Term Projection]]); tagged term encoding
  is deferred.
- Per-source **provenance uses the existing `claims` mechanism**; the claimant
  is the **retrieval origin**, never the self-asserted graph name
  ([[SPEC-025-linked-data-ingestion#ADR-004]],
  [[SPEC-025-linked-data-ingestion#ADR-008]]).
- Sources sit behind one [[SPEC-025-linked-data-ingestion#Triple Source|`TripleSource`]]
  seam with a top-level `ingest` operation returning an
  [[SPEC-025-linked-data-ingestion#CON-008|`IngestReport`]]; a static-document
  adapter ships first, SPARQL `CONSTRUCT` and dereferencing follow
  ([[SPEC-025-linked-data-ingestion#ADR-007]]).
- RDF parsing reuses the **oxrdf/oxrdfio** ecosystem, not a hand-rolled parser
  ([[SPEC-025-linked-data-ingestion#ADR-005]]).

**Load-bearing requirements:** [[SPEC-025-linked-data-ingestion#REQ-001]],
[[SPEC-025-linked-data-ingestion#REQ-003]], [[SPEC-025-linked-data-ingestion#REQ-006]],
[[SPEC-025-linked-data-ingestion#REQ-008]], [[SPEC-025-linked-data-ingestion#REQ-011]].

**Structure:**

```text
   sources (effectful shell)              pure core
  ┌───────────────────────┐        ┌───────────────────────┐
  │ FileSource   (Phase 1)│        │ Mapping               │
  │ SparqlSource (Phase 2)│──quads─▶│ triple → Spindle fact │
  │ DerefSource  (Phase 3)│        │ term projection •     │
  └───────────────────────┘        │ skolem • class-aware  │
                                    └──────────┬────────────┘
                                               │ GroundLiteral + origin + graph
                                    ┌──────────▼────────────┐
                                    │ ingest → IngestReport │
                                    │ Theory (+ claims)     │
                                    │ optional SPL emitter  │
                                    └───────────────────────┘
```

**Reading path:** [[SPEC-025-linked-data-ingestion#Terminology]],
[[SPEC-025-linked-data-ingestion#SPEC-024 Foundation and Affordances]],
[[SPEC-025-linked-data-ingestion#Functional Requirements]],
[[SPEC-025-linked-data-ingestion#Contracts]], then
[[SPEC-025-linked-data-ingestion#Architecture Decisions]].

### Conformance

The key words MUST, MUST NOT, SHALL, SHALL NOT, SHOULD, MAY, and OPTIONAL are to
be interpreted as in
[[usdd-agent-protocol#Requirement-Level Keywords (BCP 14)|BCP 14]] when, and
only when, they appear in all capitals.

## 1. Context and Problem

Spindle reasons over facts and defeasible rules, but authors who already hold
knowledge as RDF/linked data must hand-translate it into SPL. The semantic-web
world publishes vast instance data (Wikidata, DBpedia, schema.org catalogues,
enterprise knowledge graphs) in Turtle, N-Triples, JSON-LD, and behind SPARQL
endpoints and dereferenceable IRIs. There is no path to bring that data into a
Spindle theory.

The naïve temptation — map an OWL ontology's *axioms* to strict rules — is a
trap: RDF/OWL is **open-world and monotonic** (an absent triple is unknown, not
false; you may only add knowledge), whereas Spindle is **closed-world and
defeasible** (negation-as-failure, defeat, superiority). Translating axioms to
rules silently changes meaning. This specification therefore scopes to the part
of the problem with a *sound* mapping: asserting each explicit triple as a
ground fact is faithful at the level of what was said. The mapping into
Spindle's `Term` space is nonetheless an **intentionally lossy projection**
whose collision cases are enumerated in
[[SPEC-025-linked-data-ingestion#Term Projection]] — per
[[W3C RDF 1.2 Concepts]] (https://www.w3.org/TR/rdf12-concepts/), RDF keeps
IRIs, literals, and blank nodes distinct and includes datatype and language tag
in literal identity, distinctions `Term` cannot yet carry. Schema entailment,
SHACL validation, and `owl:sameAs` merging are deferred to their own
specifications ([[SPEC-025-linked-data-ingestion#Deferred Decisions]]).

### 1.1 User goals

| User | Goal | Success criterion |
|---|---|---|
| Data engineer | Load a Turtle/N-Triples dataset as facts | A `Theory` of ground facts, reasoned over unchanged |
| Integrator | Combine several linked-data sources | Each fact is attributed to its retrieval origin and trust-weightable; conflicts are *visible* per source (automatic reconciliation deferred) |
| Analyst | Query a SPARQL endpoint via `CONSTRUCT` and reason over the resulting graph | Constructed triples become facts through the same mapping |
| Tooling author | Inspect the ingested vocabulary | `Vocabulary::derive` yields per-predicate signatures and argument profiles |

## 2. Scope

### 2.1 In scope

- A pure `Mapping` from an RDF triple/quad to a Spindle ground fact.
- Class-aware `rdf:type` handling and binary property handling.
- Verbatim-IRI internal identity with presentation-only CURIE compaction.
- Typed-literal → `Term` conversion with a **total** (never-failing on valid
  RDF) degrade path and per-degrade diagnostics.
- Dataset-identity-scoped blank-node skolemization — deterministic and stable
  across re-ingestions of the same dataset, with optional per-run freshness via
  an ingestion-instance token ([[SPEC-025-linked-data-ingestion#REQ-004]]).
- Per-source provenance via the existing `claims`/`source` metadata mechanism,
  keyed by retrieval origin, with graph name as separate context metadata.
- A `TripleSource` trait, a top-level `ingest` operation with an
  `IngestReport`, and a Phase-1 static-document adapter (Turtle, N-Triples,
  N-Quads, TriG, RDF-XML) via `oxrdfio`.
- Bounded, streaming ingestion with byte/triple/time limits and, for network
  sources, request, redirect, scheme, and address-class bounds.
- `Theory` output plus an optional SPL emitter, and a CLI `import` subcommand.

### 2.2 Out of scope (this revision)

- RDFS/OWL entailment or any T-Box → rules translation.
- **Trust-aware conflict reconciliation** between sources (requires core
  alternative-proof aggregation; see
  [[SPEC-025-linked-data-ingestion#Deferred Decisions]]).
- A tagged term encoding distinguishing IRIs, strings, and language-tagged
  literals (deferred `Term` extension).
- SHACL validation (a later mapping onto [[SPEC-024-predicate-model-and-vocabulary#Shape|`Shape`]]).
- `owl:sameAs` identity merging (emitted as a plain fact only).
- JSON-LD ingestion (deferred; needs context processing).
- SPARQL `SELECT` row mapping (Phase 2 is `CONSTRUCT` only; a row-mapping
  contract is deferred).
- [[RDF Dataset Canonicalization|RDF dataset canonicalization (RDFC-1.0)]] for
  graph-isomorphism-invariant output.
- Exporting Spindle theories back out as RDF/OWL.
- Named-graph reasoning semantics (the graph name is context metadata only).

## 3. Terminology

### 3.1 Triple / Quad

A **Triple** is `(subject, predicate, object)`; a **Quad** additionally carries a
graph name. Subjects are IRIs or blank nodes; predicates are IRIs; objects are
IRIs, blank nodes, or literals. Graph names are IRIs or blank nodes.

### 3.2 Triple Source

A **TripleSource** is an effectful producer of quads with a **retrieval
origin**. Adapters: `FileSource` (static documents), `SparqlSource` (a
`CONSTRUCT` endpoint), and `DereferenceSource` (follow-your-nose IRI
dereferencing). All obey [[SPEC-025-linked-data-ingestion#REQ-009]] (bounded
ingestion).

### 3.3 Mapping

A **Mapping** is the pure function from a quad to a Spindle
[[SPEC-024-predicate-model-and-vocabulary#Ground Literal|`GroundLiteral`]] plus
provenance keys (origin and graph). It performs class-aware structuring, term
projection, datatype conversion, and skolemization. It performs no I/O.

### 3.4 CURIE

A **CURIE** is a compact IRI `prefix:local` (e.g. `foaf:knows`) resolved through
a prefix registry. In this specification CURIEs are **presentation-only**:
internal identity is always the full IRI
([[SPEC-025-linked-data-ingestion#ADR-002]]); compaction happens solely when
rendering for humans and is clearly marked as non-identity-preserving.

### 3.5 Class-Aware Mapping

Under **class-aware mapping**, `(s rdf:type C)` becomes the unary fact `C(s)`
and every other triple becomes the binary fact `p(s o)`. This aligns instance
data with [[SPEC-024-predicate-model-and-vocabulary#ADR-001]] so classes surface
as unary predicates in the derived
[[SPEC-024-predicate-model-and-vocabulary#Vocabulary|vocabulary]].

### 3.6 Claims Provenance

Ingested facts are grouped by **retrieval origin** — the canonical file path,
URL, or endpoint IRI the ingestion actually read — and attributed through
Spindle's existing `(claims source ...)` mechanism, which stamps each fact with
a `source` metadatum. The quad's graph name, when present, is recorded as a
separate `graph` context metadatum; it is **not** the claimant, because a graph
name is self-asserted by the document and is not required by
[[W3C RDF 1.2 Concepts]] to denote its graph
([[SPEC-025-linked-data-ingestion#ADR-008]]). Callers add
`(trusts source weight)` to weight origins. Provenance here is *annotation*:
imported triples are strict facts, so two conflicting facts from different
origins both remain definitely provable; reconciliation policy is deferred
([[SPEC-025-linked-data-ingestion#Deferred Decisions]]).

### 3.7 Theory

A **Theory** is Spindle's canonical aggregate (rules, facts, superiority,
metadata, trust). Ingestion produces facts and provenance metadata only; it adds
no rules. The `Theory` — not its SPL rendering — is the canonical output of
ingestion.

### 3.8 Term Projection

The **term projection** is the documented, intentionally lossy function from
RDF terms to Spindle `Term`s. Its collision cases:

- An IRI, a plain string literal of the same spelling, and a skolemized blank
  node symbol of the same spelling are **indistinguishable** once projected to
  `Symbol` (e.g. `<http://example/x>` vs `"http://example/x"`).
- Language tags are dropped: `"chat"@en` and `"chat"@fr` collide.
- Non-numeric datatype IRIs are dropped: `"true"^^xsd:boolean` and the plain
  string `"true"` collide.
- Distinct lexical forms of one value are **not** normalized (`"01"^^xsd:integer`
  and `"1"^^xsd:integer` both become `Integer(1)`; but as *strings* `"01"` and
  `"1"` stay distinct).
- Strings whose spelling matches SPL's numeric or variable grammar are
  escaped or coarsened as specified in
  [[SPEC-025-linked-data-ingestion#REQ-005]] and
  [[SPEC-025-linked-data-ingestion#REQ-008]].

Callers for whom these collisions are unacceptable must wait for the deferred
tagged term encoding ([[SPEC-025-linked-data-ingestion#Deferred Decisions]]).

## 4. SPEC-024 Foundation and Affordances

This bridge is deliberately a thin composition over the predicate branch. The
following affordances already exist and are load-bearing here; the gaps are
small and explicit.

| Bridge need | SPEC-024 / core affordance | Status |
|---|---|---|
| Class as unary, property as binary | `PredicateSymbol` + [[SPEC-024-predicate-model-and-vocabulary#ADR-001]] | Afforded |
| Ground facts at the ingest boundary | [[SPEC-024-predicate-model-and-vocabulary#Ground Literal|`GroundLiteral`]] / `Literal::classify` | Afforded |
| Typed literal values | `Term` (Symbol/Integer/Decimal/Float) + `PrimitiveSort` | Afforded |
| Per-predicate/position observations of ingested data | [[SPEC-024-predicate-model-and-vocabulary#Argument Profile|`ArgumentProfile`]] / `Vocabulary::derive` | Afforded (near-tailored) |
| Predicate schema (if declared later from RDFS domain/range) | `PredicateDeclaration` / `PredicateSignature` | Afforded (future T-Box) |
| `rdfs:comment`/`rdfs:label` on a property → predicate description | `MetaTarget::Predicate` + inline metadata (SPEC-024 v0.3.0) | Afforded |
| Full-IRI functor containing `/` | Indicator recognizer quotes slash-bearing functors ([[SPEC-024-predicate-model-and-vocabulary#CON-002]]) | Afforded |
| Later SHACL-style validation of ingested data | [[SPEC-024-predicate-model-and-vocabulary#Shape|`Shape`]] | Afforded (deferred use) |
| Per-triple source provenance + trust | Existing `claims`/`Meta` `source` + trust module (pre-SPEC-024) | Afforded (annotation only; see G-4) |

**Identified gaps (documented, not blocking v1):**

- **G-1 — value space:** `Term` has four variants, all untagged as to RDF term
  kind. IRIs, blank-node skolems, plain strings, language-tagged strings, and
  non-numeric typed literals all project to `Symbol`, with the collision cases
  enumerated in [[SPEC-025-linked-data-ingestion#Term Projection]]. This is a
  `Term` limitation, not a SPEC-024 gap; a tagged encoding and temporal
  routing of dates are deferred
  ([[SPEC-025-linked-data-ingestion#Deferred Decisions]]).
- **G-2 — instance vs schema provenance:** SPEC-024 predicate metadata is
  *schema-level* (per `PredicateSymbol`), while linked-data provenance is
  *instance-level* (per triple). The bridge correctly uses the schema channel
  for `rdfs:comment` and the `claims` channel for triple provenance; both exist.
- **G-3 — no core namespace concept:** prefix registries live entirely in this
  bridge crate; the core neither needs nor gains a namespace type. This is a
  boundary, not a gap.
- **G-4 — no alternative-proof aggregation:** imported triples are strict
  facts; the reasoner keeps conflicting strict facts definitely provable, and
  the core deduplicates identical fact literals, retaining a single rule
  label. Trust weighting therefore *annotates* rather than *reconciles*.
  Ingestion compensates at its own layer (deterministic label assignment,
  full assertion-record retention — [[SPEC-025-linked-data-ingestion#CON-008]]);
  true reconciliation is a deferred core change.
- **G-5 — no per-fact metadata syntax in SPL:** the `graph` context is carried
  on the mapped fact in the canonical `Theory` (first deduplicated context) and,
  in full, in the `IngestReport` assertion log — lossless programmatically. It is
  the **SPL text form** that cannot hold it: `(given ...)` facts have no
  per-fact metadata grammar and the `claims` block exposes only fixed keywords,
  so graph context does not survive a Theory → SPL → Theory round-trip. A
  parseable graph syntax (e.g. a `:graph` `claims` field plus a parser contract)
  is deferred ([[SPEC-025-linked-data-ingestion#Deferred Decisions]]).

**Verdict:** the predicate branch affords everything explicit-triple ingestion
needs, and several pieces (`ArgumentProfile`, class-as-unary, the slash-quoting
indicator recognizer, predicate metadata for `rdfs:comment`, `GroundLiteral`)
are close to purpose-built for it. The real limits are `Term`'s untagged value
space (G-1), the absence of alternative-proof aggregation (G-4), and SPL's lack
of per-fact metadata for graph context (G-5) — all accepted and documented for
v1, with programmatic retention where SPL cannot round-trip.

## 5. Architecture

### 5.1 Purity boundary

Mirrors [[SPEC-024-predicate-model-and-vocabulary#Purity Boundary Map]].

- **Pure core (`spindle-rdf::map`)**: quad → `GroundLiteral` + provenance keys;
  term projection; datatype conversion; skolemization; class-aware structuring;
  Theory assembly; SPL rendering; presentation-only CURIE compaction. No file,
  network, clock, or environment I/O.
- **Effectful shell (`spindle-rdf::source`)**: `TripleSource` adapters
  (`FileSource`, `SparqlSource`, `DereferenceSource`), the top-level `ingest`
  driver, and the CLI `import` command. Only these perform I/O.

The pure core SHALL NOT import the source adapters.

### 5.2 Placement (Simplicity Ladder)

New capability sits at rung 4–5: a new `spindle-rdf` crate that *composes*
existing components — `Theory`, `Term`, `intern`, the SPEC-024 vocabulary types,
the `claims`/trust mechanism — plus one external RDF-parsing dependency. No new
reasoning, identity, or ontology subsystem is introduced.

### 5.3 Phasing

1. **Phase 1 — static documents:** `FileSource` over `oxrdfio` (Turtle,
   N-Triples, N-Quads, TriG, RDF-XML). Pure `Mapping`. `ingest` +
   `IngestReport`. `Theory` + SPL output. CLI `import`.
2. **Phase 2 — SPARQL:** `SparqlSource` issuing **`CONSTRUCT` queries only**
   and parsing the resulting graph over a small HTTP client. `SELECT` produces
   solution mappings, not a graph ([[SPARQL 1.1 Query Language]],
   https://www.w3.org/TR/sparql11-query/), and is out of scope until a
   row-mapping contract exists.
3. **Phase 3 — dereferencing:** `DereferenceSource` following IRIs under the
   full network bounds of [[SPEC-025-linked-data-ingestion#CON-006]] (depth,
   request count, bytes, deadline, redirects, schemes, address classes) with a
   visited-set for cycles.

Each phase is independently shippable; the `Mapping` core is unchanged across
them.

## 6. Functional Requirements

### REQ-001: Class-aware triple mapping

The mapping SHALL translate `(s rdf:type C)` (where `C` is an IRI) to the unary
ground fact `C(s)`, and every other triple `(s p o)` to the binary ground fact
`p(s o)`.

**Acceptance criteria:** `(ex:alice rdf:type foaf:Person)` yields a unary fact
whose functor is the full IRI `http://xmlns.com/foaf/0.1/Person` —
arity 1; `(ex:alice foaf:knows ex:bob)` yields the corresponding binary fact —
arity 2. A triple whose object is a literal never becomes a class fact.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-001]], [[SPEC-025-linked-data-ingestion#ADR-001]], [[SPEC-025-linked-data-ingestion#TEST-001]].

### REQ-002: Verbatim IRI identity; presentation-only CURIE compaction

Internal fact identity SHALL use the full IRI, verbatim, as one interned
symbol. CURIE compaction SHALL be applied only in presentation layers (human-
facing SPL emission with compaction explicitly enabled, CLI display) through a
single global prefix registry, SHALL be deterministic given the same registry,
and SHALL never affect identity or unification. The round-trip guarantee
([[SPEC-025-linked-data-ingestion#REQ-008]]) is scoped to **default full-IRI
emission**; compacted display is opt-in (`--compact-iris`) and is explicitly
**non-round-trippable**, because the SPL parser has no prefix registry and would
intern a CURIE such as `foaf:knows` literally rather than recovering the full
IRI. Round-trippable emission therefore keeps compaction off.

**Acceptance criteria:** two documents binding `ex:` to different namespaces
produce facts whose functors are the two distinct full IRIs — no collision; the
same IRI reached under different prefixes in different documents produces one
symbol. With compaction enabled and `foaf: <http://xmlns.com/foaf/0.1/>` in
the registry, `<http://xmlns.com/foaf/0.1/knows>` *displays* as `foaf:knows`.
Full IRIs in functor position are quoted by the SPEC-024 indicator renderer.
Registry conflicts (one prefix, two namespaces) are rejected with a
diagnostic; two prefixes naming one namespace compact via the
lexicographically first prefix.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-002]], [[SPEC-025-linked-data-ingestion#ADR-002]], [[SPEC-025-linked-data-ingestion#TEST-002]].

### REQ-003: Total typed-literal conversion

Object literals SHALL convert to `Term` by datatype via a **total** function:
every well-formed RDF literal produces a `Term`; no literal aborts ingestion.
`xsd:integer` (and derived integer types) → `Integer` when the value fits
`i64`, else `Decimal` when it fits `rust_decimal::Decimal`, else `Symbol` of
the lexical form with a `degraded` diagnostic. `xsd:decimal` → `Decimal`, else
`Symbol` + diagnostic beyond `Decimal` range. `xsd:float` SHALL be parsed to
binary32 and widened to `Float`; `xsd:double` parsed to binary64 → `Float`;
non-finite values → `Symbol` of the lexical form. A literal whose lexical form
is **ill-typed** for its numeric datatype (e.g. `"abc"^^xsd:integer`) →
`Symbol` of the lexical form with a diagnostic (RDF permits ill-typed
literals; they are preserved, not errors). All other datatypes, plain
literals, and language-tagged strings → `Symbol` of the lexical form.

**Acceptance criteria:** `"2"^^xsd:integer` → `Integer(2)`; `"2.5"^^xsd:decimal`
→ `Decimal(2.5)`; `"foo"@en` → `Symbol("foo")`; an integer exceeding `i64` →
`Decimal`; an integer or decimal exceeding `Decimal`'s 96-bit range →
`Symbol` + diagnostic; `"abc"^^xsd:integer` → `Symbol("abc")` + diagnostic;
`"1.0E0"^^xsd:float` round-trips through binary32 before widening.
`Vocabulary::derive` over the result reports the observed argument kinds per
predicate position.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-003]], [[SPEC-025-linked-data-ingestion#TEST-003]].

### REQ-004: Blank-node skolemization

Blank nodes SHALL be skolemized to symbols scoped to a **dataset identity** — a
stable key derived from the retrieval origin (and, for a multi-document source,
the specific document that supplied the quad;
[[SPEC-025-linked-data-ingestion#REQ-007]]). Within one dataset identity, equal
blank-node labels — including across named graphs of one dataset, where blank
nodes may be shared — map to one symbol via a deterministic per-dataset
encounter ordinal; across distinct dataset identities they never unify, because
the dataset key is part of the skolem spelling. Blank-node labels are not stable
identifiers across datasets ([[W3C RDF 1.2 Concepts]]); the dataset key, not the
label alone, establishes identity.

Skolem naming is therefore a pure function of `(effective dataset key, encounter
ordinal)`, where the **effective dataset key** is the source-supplied
`DatasetId` optionally prefixed by a caller-supplied **ingestion-instance
token** (`IngestOptions::instance_token`, [[SPEC-025-linked-data-ingestion#CON-008]]).
Re-ingesting a byte-identical dataset with identical options — including the
same instance token, or none — reproduces the same symbols, as required for
byte-identical determinism ([[SPEC-025-linked-data-ingestion#REQ-011]]): a fixed
token is fully deterministic. Cross-run *freshness* (two ingestions of the same
bytes yielding non-unifying blank nodes) is available **only** by supplying two
**different** token values; it is otherwise not a default, because unconditional
freshness is mutually exclusive with determinism under spelling-interned symbols
and a no-clock/no-random rule ([[SPEC-025-linked-data-ingestion#NFR-001]]). Only
*varying* the token changes output; identical options (token included) are
byte-identical, so the token narrows rather than voids the REQ-011 guarantee.

**Acceptance criteria:** `_:b1` from two distinct dataset identities (two
different origins, or two documents of a dereferencing source) produces two
distinct symbols; `_:b1` twice within one dataset produces the same symbol;
`_:b1` shared across two named graphs of one TriG dataset produces the same
symbol; re-ingesting the same dataset with identical options (same instance
token, or none) reproduces identical symbols; supplying two **different**
`instance_token` values makes the two ingestions' blank nodes non-unifying.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-001]], [[SPEC-025-linked-data-ingestion#TEST-004]].

### REQ-005: Ground-fact output

Every mapped fact SHALL be a checked
[[SPEC-024-predicate-model-and-vocabulary#Ground Literal|`GroundLiteral`]]; the
mapping SHALL NOT emit variables, and ingestion adds no rule. Because the core
treats any symbol beginning with `?` as a variable (string-prefix detection in
grounding), the mapping SHALL escape a leading `?` in any projected symbol by
rewriting it to the reserved sequence `%3F` (percent-encoding of `?`), with a
diagnostic per escape. The resulting collision with genuine `%3F…` strings is
a documented case of the
[[SPEC-025-linked-data-ingestion#Term Projection|term projection]].

**Acceptance criteria:** the produced `Theory` has zero non-fact rules; each
fact passes `GroundLiteral::try_from`; ingesting the RDF string `"?name"`
yields `Symbol("%3Fname")` plus a diagnostic, and grounding treats no ingested
term as a variable.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-001]], [[SPEC-025-linked-data-ingestion#TEST-005]].

### REQ-006: Per-source provenance keyed by retrieval origin

Ingestion SHALL attribute each fact to the **retrieval origin of the quad that
produced it** — the canonical file path, URL, or authenticated endpoint IRI the
bytes were actually read from, carried per quad on
[[SPEC-025-linked-data-ingestion#CON-005|`AttributedQuad`]] so a multi-document
source (redirects, dereferencing) never lets one document's content inherit
another's origin or trust — through the existing `claims`/`source` metadata
mechanism, and SHALL NOT introduce a second provenance model. The quad's graph name, when present,
SHALL be recorded as a separate `graph` context metadatum and SHALL NOT be used
as the claimant: a graph name is chosen by the (possibly untrusted) document
and is not required to denote its graph ([[W3C RDF 1.2 Concepts]]); it may
even be a blank node. Provenance is annotation: it SHALL NOT change
conclusions ([[SPEC-025-linked-data-ingestion#NFR-003]]), and this revision
makes no automatic-reconciliation claim.

**Acceptance criteria:** a fact read from file `data.ttl` inside graph `ex:g`
carries `source = <canonical path of data.ttl>` identical to what
`(claims <origin> ...)` would produce, plus `graph = ex:g`; a hostile document
naming its graph `<https://trusted.example/>` cannot acquire that origin's
trust weight; the SPL emitter renders one `claims` block per origin;
conclusions are unchanged whether provenance is attached or not.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-004]], [[SPEC-025-linked-data-ingestion#ADR-004]], [[SPEC-025-linked-data-ingestion#ADR-008]], [[SPEC-025-linked-data-ingestion#TEST-006]].

### REQ-007: TripleSource abstraction, ingest driver, and static adapter

Ingestion SHALL consume quads through a `TripleSource` trait driven by a
top-level `ingest` operation ([[SPEC-025-linked-data-ingestion#CON-008]]) that
classifies errors as fatal or per-item, supports **atomic** (default: any
fatal error yields no theory) and **streaming** (explicitly requested: partial
theory plus diagnostics) modes, and returns an `IngestReport`. A `FileSource`
adapter over `oxrdfio` SHALL recognize Turtle, N-Triples, N-Quads, TriG, and
RDF-XML.

**Acceptance criteria:** the same `Mapping` produces identical facts regardless
of which adapter supplied the quads; a well-formed Turtle document ingests
successfully; a malformed document in atomic mode yields `Err(IngestError)`
whose `report.theory` is `None` (with counters and a fatal diagnostic retained)
and whose `cause` is the structured `SourceError`; the same document in
streaming mode yields `Ok` with the facts mapped before the failure plus a
diagnostic identifying it.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-005]], [[SPEC-025-linked-data-ingestion#CON-008]], [[SPEC-025-linked-data-ingestion#ADR-005]], [[SPEC-025-linked-data-ingestion#ADR-007]], [[SPEC-025-linked-data-ingestion#TEST-007]].

### REQ-008: Theory output and SPL emission

Ingestion SHALL return a `Theory` — the canonical output — and SHALL provide an
emitter rendering that theory (including `claims` provenance) to SPL text with
full IRIs (compaction off). SPL re-infers term variants from spelling, so the
emitter SHALL render terms to avoid ambiguity where it can and flag the
residue:

- **`Decimal` disambiguation:** every `Decimal` SHALL be rendered with a
  fractional marker (minimum `.0`), so a zero-scale decimal such as
  `"2"^^xsd:decimal`, and an integer promoted beyond `i64` into `Decimal`, never
  matches SPL's integer grammar. Since SPL reparses a `.`-bearing, exponent-less
  numeric token as `Decimal` and `rust_decimal` compares by value (`2 == 2.0`),
  such facts round-trip to an **equal** `Term` with no diagnostic.
- **`lossy_spl` residue:** the emitter SHALL detect and report in the
  `IngestReport` (as `lossy_spl`) every fact containing a `Symbol` whose spelling
  matches SPL's integer, decimal, or float grammar; a `Float` whose rendering
  lacks an exponent marker (it would reparse as `Decimal`); or any `Decimal`
  whose rendering still matches the integer grammar (a defensive check on the
  marker rule).

SPL round-trip fidelity covers **facts and `source` (origin) provenance**. The
`graph` context lives on the mapped fact in the `Theory` and, in full, in the
`IngestReport` assertion log (CON-004, §3.7), but it is **not representable in
current SPL** — `(given ...)` facts have no per-fact metadata syntax and the
`claims` block carries only fixed keywords — so `graph` alone does not survive a
Theory → SPL → Theory round-trip
([[SPEC-025-linked-data-ingestion#CON-008]]); a parseable `graph` SPL syntax is
deferred ([[SPEC-025-linked-data-ingestion#Deferred Decisions]]). Re-parsing
emitted SPL SHALL reproduce the original facts and `source` metadata exactly
when no `lossy_spl` diagnostic was reported, and re-emission after re-parsing
SHALL be byte-identical (idempotence from the first re-parse onward). A typed
SPL term syntax that removes the `lossy_spl` caveat is likewise deferred.

**Acceptance criteria:** for a corpus with no `lossy_spl` diagnostics,
emit → parse → compare yields equal facts and `source` metadata; `"2"^^xsd:decimal`
and an integer beyond `i64` round-trip as equal `Decimal`s with **no** `lossy_spl`
diagnostic; `"2"^^xsd:string` produces a `lossy_spl` diagnostic naming the fact;
for any corpus, parse(emit(T)) followed by emit is byte-stable.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-004]], [[SPEC-025-linked-data-ingestion#CON-008]], [[SPEC-025-linked-data-ingestion#TEST-008]].

### REQ-009: Bounded ingestion

Ingestion SHALL honor caller-configured limits. For all sources: maximum input
bytes (decompressed), maximum triples. For network sources additionally:
maximum total requests, per-request timeout, total elapsed deadline, maximum
aggregate response bytes (decompressed), maximum redirects per request,
allowed URL schemes (default `https` and `http` only), a private/loopback/
link-local address policy (default **deny**, to prevent server-side request
forgery), an acceptable content-type set, and (for dereferencing) maximum
depth plus a visited-IRI set for cycle avoidance. Limits reached SHALL produce
a diagnostic, not a panic or unbounded output.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-006]], [[SPEC-025-linked-data-ingestion#NFR-002]], [[SPEC-025-linked-data-ingestion#TEST-009]].

### REQ-010: Non-entailment guarantee

Ingestion SHALL assert explicit triples as facts only. It SHALL NOT perform
RDFS/OWL entailment, SHALL NOT translate schema to rules, and SHALL emit
`owl:sameAs` as a plain `owl:sameAs(a, b)` fact without merging identities.
Schema triples (e.g. `rdfs:subClassOf` assertions) are ingested as plain
facts like any other triple — the guarantee is that they gain **no inferential
force**.

**Acceptance criteria:** ingesting a document with `owl:sameAs` and RDFS axioms
adds only the literal triples as facts; no derived class memberships or merged
individuals appear until the reasoner runs user-supplied rules.

**Traces:** [[SPEC-025-linked-data-ingestion#ADR-003]], [[SPEC-025-linked-data-ingestion#ADR-006]], [[SPEC-025-linked-data-ingestion#TEST-010]].

### REQ-011: Determinism

For **byte-identical** source input, identical options — including the same
`instance_token` value, or none — and an identical prefix registry, the produced
theory, its SPL rendering, its `IngestReport`, and its diagnostics SHALL be
byte-identical across runs (independent of hash-map iteration order; skolem
symbols are a function of `(effective dataset key, encounter ordinal)` per
[[SPEC-025-linked-data-ingestion#REQ-004]], where the effective dataset key
includes the instance token when set; duplicate-fact labels are assigned by
first encounter). Two runs differ in output **only** when their `instance_token`
values differ. For inputs that
are equal only up to triple reordering, the produced **fact set** (ignoring
skolem symbol spellings and label assignment) SHALL be isomorphic, but output
bytes MAY differ: encounter-order skolem naming cannot be reorder-invariant,
and full invariance requires
[[RDF Dataset Canonicalization|RDF dataset canonicalization]], which is
deferred.

**Traces:** [[SPEC-025-linked-data-ingestion#NFR-001]], [[SPEC-025-linked-data-ingestion#TEST-011]].

## 7. Non-Functional Requirements

### NFR-001: Determinism

See [[SPEC-025-linked-data-ingestion#REQ-011]]. Skolem naming and duplicate
labeling SHALL derive from a deterministic function of `(effective dataset key,
encounter ordinal)` — the effective dataset key being the source-supplied
`DatasetId` prefixed by the optional `instance_token`
([[SPEC-025-linked-data-ingestion#REQ-004]]) — never from a random or clock
source or from hash-map iteration. The instance token is part of that key, so a
fixed token (or none) is fully deterministic; only runs whose token values
**differ** are excluded from the byte-identical guarantee.

### NFR-002: Bounded, streaming parsing

Static parsing SHALL stream (constant memory in graph size where the format
allows) and honor [[SPEC-025-linked-data-ingestion#REQ-009]] limits. No
catastrophic-backtracking recognizer is permitted; recognition delegates to
`oxrdfio` ([[usdd-agent-protocol#Language-Theoretic Security (LangSec)|LangSec]]).

### NFR-003: Reasoning parity

Enabling ingestion (facts + provenance) SHALL NOT change conclusions relative to
the same facts asserted directly. Provenance metadata is inert to inference; see
[[SPEC-024-predicate-model-and-vocabulary#REQ-013]].

**Traces:** [[SPEC-025-linked-data-ingestion#TEST-006]].

## 8. Contracts

### CON-001: Mapping core

```rust
/// A mapped, source-attributed ground fact.
pub struct MappedFact {
    pub fact: GroundLiteral,      // C(s) or p(s, o)
    pub origin: Option<Symbol>,   // retrieval origin (claimant) — REQ-006
    pub graph: Option<Symbol>,    // graph name, context metadata only
}

pub struct MappingOptions {
    pub class_aware: bool,        // default true (REQ-001)
    pub provenance: bool,         // default true (REQ-006)
    pub limits: IngestLimits,
}

/// Pure: no I/O. `ctx` holds skolem counters keyed by the *effective dataset
/// key* — the quad's `DatasetId` prefixed by `IngestOptions::instance_token`
/// when set — so blank-node scope follows the quad's own dataset (REQ-004);
/// `input.origin` becomes the fact's claimant (REQ-006).
pub fn map_quad(input: &AttributedQuad, ctx: &mut MapContext, opts: &MappingOptions)
    -> Result<MappedFact, MapError>;
```

The subject/object/predicate use `oxrdf` terms as input; outputs are Spindle
`Term`/`Symbol` under the
[[SPEC-025-linked-data-ingestion#Term Projection|term projection]] (verbatim
IRIs, `?`-escaping per [[SPEC-025-linked-data-ingestion#REQ-005]]). A `Quad`
with a variable is impossible (asserted data is ground), so `map_quad` never
yields a pattern. Theory assembly is part of `ingest`
([[SPEC-025-linked-data-ingestion#CON-008]]), which also owns duplicate
handling and label assignment.

### CON-002: Prefix registry (presentation only)

```ebnf
display-form = curie | quoted-full-iri ;
curie        = prefix ":" local ;      (* only when a registry entry matches *)
prefix       = ncname-ish ;
local        = { iri-path-char } ;
```

A `PrefixRegistry` maps prefix → namespace IRI and is **global to one
presentation context**, caller-supplied, and deterministic. The built-in table
seeds `rdf`, `rdfs`, `owl`, `xsd`, `foaf`, `schema`, `dc`, `dcterms`, `skos`.
Sources MAY report their `@prefix`/`PREFIX` declarations
(`TripleSource::prefixes`, [[SPEC-025-linked-data-ingestion#CON-005]]) to
*suggest* registry entries; a suggestion conflicting with an existing binding
is dropped with a diagnostic, never silently rebound. Compaction picks the
longest matching namespace; when two prefixes name one namespace, the
lexicographically first prefix wins. Compaction is display-only: it never
feeds back into identity ([[SPEC-025-linked-data-ingestion#REQ-002]]) and is
off in round-trippable SPL emission
([[SPEC-025-linked-data-ingestion#REQ-008]]). Full IRIs in functor position
are quoted by the
[[SPEC-024-predicate-model-and-vocabulary#CON-002|SPEC-024 indicator renderer]]
because of `/`.

### CON-003: Literal datatype mapping

| RDF literal | `Term` |
|---|---|
| `xsd:integer` and derived integer types, in `i64` | `Integer` |
| integer beyond `i64`, within `rust_decimal` range | `Decimal` |
| integer or decimal beyond `rust_decimal`'s 96-bit range | `Symbol` (lexical form) + diagnostic |
| `xsd:decimal` within range | `Decimal` |
| `xsd:double` (binary64), finite | `Float` |
| `xsd:float` (binary32, then widened), finite | `Float` |
| non-finite double/float (`NaN`, `INF`, `-INF`) | `Symbol` (lexical form) |
| ill-typed numeric lexical form (e.g. `"abc"^^xsd:integer`) | `Symbol` (lexical form) + diagnostic |
| `xsd:string`, plain, `rdf:langString` | `Symbol` (lexical form; language tag dropped in v1) |
| any other datatype IRI | `Symbol` (lexical form; datatype dropped in v1) |

The function is total over well-formed RDF literals; every degrade emits an
`IngestReport` diagnostic ([[SPEC-025-linked-data-ingestion#REQ-003]]).

### CON-004: Provenance and SPL output

Facts are grouped by `origin`; each group renders as:

```text
(claims <origin-symbol>
  (given (<Class> <individual>))
  (given (<predicate> <subject> <object>))
  ...)
```

(Illustrative shape with `<placeholders>`, not parseable SPL — hence the plain
fence.)

which is exactly the existing `claims` block. The `graph` metadatum **is**
carried on the mapped fact in the canonical `Theory` (REQ-006, §3.7): a
deduplicated fact literal keeps the `graph` of its **first** asserted context,
and the complete set of `(origin, graph)` contexts lives in the `IngestReport`
assertion log ([[SPEC-025-linked-data-ingestion#CON-008]]). Only **SPL
emission** drops `graph`: `(given ...)` facts have no per-fact metadata syntax
and the `claims` block carries only fixed keywords, so graph context is not
round-tripped through SPL; a parseable graph syntax is deferred
([[SPEC-025-linked-data-ingestion#Deferred Decisions]]). Every `Decimal` is
rendered with a fractional marker so it does not reparse as `Integer`
([[SPEC-025-linked-data-ingestion#REQ-008]]). Callers supply
`(trusts <origin> <weight>)` separately. With `provenance = false`, facts render
as bare `(given ...)` statements. Round-trippable emission uses full IRIs; CURIE
compaction is opt-in display only ([[SPEC-025-linked-data-ingestion#CON-002]]).

### CON-005: TripleSource

```rust
/// A quad annotated with the identity of the document that actually supplied
/// it, so a multi-document source attributes each quad correctly and scopes
/// blank nodes per document.
pub struct AttributedQuad {
    pub quad: Quad,
    /// Retrieval origin (claimant) of *this* quad — the URL/path the bytes were
    /// read from, which for a dereferencing source varies within one stream
    /// (REQ-006).
    pub origin: Symbol,
    /// Dataset identity for blank-node scoping (REQ-004): a stable key for the
    /// specific document this quad came from. Two quads from different fetched
    /// documents carry different `dataset` keys even under one `TripleSource`.
    pub dataset: DatasetId,
}

pub trait TripleSource {
    /// Stream attributed quads. Item errors carry a fatality classification.
    /// Each item already knows its own origin and dataset scope, so the driver
    /// never inherits one document's provenance for another's quads.
    fn quads(&mut self)
        -> Result<Box<dyn Iterator<Item = Result<AttributedQuad, SourceError>> + '_>, SourceError>;
    /// Namespace declarations observed so far (advisory, presentation only —
    /// CON-002). Empty when the format has none.
    fn prefixes(&self) -> Vec<(String, String)>;
}

pub struct SourceError { pub kind: SourceErrorKind, pub fatal: bool, /* … */ }

pub struct FileSource { /* format, reader, base IRI, limits */ }   // Phase 1
pub struct SparqlSource { /* endpoint, CONSTRUCT query, http, limits */ } // Phase 2
pub struct DereferenceSource { /* seed IRIs, http, limits, visited */ }   // Phase 3
```

A single-document `FileSource` emits one `dataset`/`origin` for all its quads; a
`DereferenceSource` emits a distinct `dataset`/`origin` per fetched document, so
per-quad provenance (REQ-006) and per-document blank-node scope (REQ-004) both
hold without the driver guessing.

### CON-006: Limits

```rust
pub struct IngestLimits {
    // all sources
    pub max_input_bytes: usize,        // decompressed
    pub max_triples: usize,
    // network sources (Phases 2–3)
    pub max_requests: u32,
    pub request_timeout: Duration,
    pub total_deadline: Duration,      // wall-clock for the whole ingestion
    pub max_response_bytes: usize,     // decompressed, aggregate
    pub max_redirects: u32,            // per request
    pub allowed_schemes: SchemeSet,    // default {https, http}
    pub address_policy: AddressPolicy, // default DenyPrivate (loopback, RFC 1918,
                                       // link-local, unique-local) — SSRF guard
    pub accepted_content_types: ContentTypeSet,
    // dereferencing (Phase 3)
    pub max_deref_depth: u32,
}
```

### CON-007: CLI

```text
spindle import [--format turtle|ntriples|nquads|trig|rdfxml] [--base IRI]
               [--trust WEIGHT] [--no-provenance]
               [--emit-spl [--compact-iris]] [--streaming] <file|->
```

`--emit-spl` prints SPL instead of reasoning; otherwise the imported theory is
reasoned and conclusions are printed, matching `spindle reason` output.
`--compact-iris` enables presentation-only CURIE display and marks the output
as non-round-trippable. `--streaming` selects streaming error mode
([[SPEC-025-linked-data-ingestion#REQ-007]]). URL and endpoint arguments
arrive with Phases 2–3 and are not advertised before those phases ship.

### CON-008: Ingest driver and report

```rust
pub enum ErrorMode { Atomic, Streaming }   // REQ-007

pub struct IngestOptions {
    pub mapping: MappingOptions,
    pub error_mode: ErrorMode,             // default Atomic
    /// Optional cross-run freshness control (REQ-004). `None` (default) makes
    /// blank-node identity depend only on the source-supplied `DatasetId`, so
    /// re-ingesting the same bytes reuses skolem symbols (byte-identical
    /// determinism). `Some(t)` forms the effective dataset key as `(t,
    /// DatasetId)` for every quad, so two ingestions with *different* tokens do
    /// not unify while each fixed-token run stays deterministic (REQ-011).
    pub instance_token: Option<String>,    // default None
}

/// One (origin, graph) assertion of a fact. The reasoning `Theory` holds each
/// fact literal once (the core deduplicates by literal), but *every* assertion
/// context is retained here so no (origin, graph) pair is lost — including the
/// same triple asserted in two named graphs of one origin, which is invisible
/// to literal dedup because `graph` is not part of fact identity.
pub struct AssertionRecord {
    pub fact: GroundLiteral,
    pub origin: Symbol,
    pub graph: Option<Symbol>,
}

pub struct IngestReport {
    /// `Some` on success; `None` only inside `IngestError::report` on atomic
    /// failure.
    pub theory: Option<Theory>,
    pub triples_read: u64,
    pub facts_added: u64,           // distinct fact literals in `theory`
    /// Every (origin, graph) assertion, in deterministic encounter order —
    /// a superset of `facts_added` whenever a literal is asserted more than
    /// once (multiple origins, multiple graphs, or both). The canonical
    /// `source` metadatum for a deduplicated literal is assigned to its first
    /// record; the rest remain recoverable here (G-4, G-5).
    pub assertions: Vec<AssertionRecord>,
    /// Term-projection degrades (CON-003), `?`-escapes (REQ-005),
    /// SPL-lossy facts (REQ-008), limit hits (REQ-009), prefix conflicts
    /// (CON-002), per-item source errors (streaming mode).
    pub diagnostics: Vec<Diagnostic>,
}

/// Carries the partial report *and* the fatal cause on atomic failure, so
/// counters, diagnostics, and `theory = None` are never lost to the error path.
pub struct IngestError {
    pub report: IngestReport,   // theory = None
    pub cause: SourceError,
}

pub fn ingest(source: &mut dyn TripleSource, opts: &IngestOptions)
    -> Result<IngestReport, IngestError>;   // Err (with report) only on a fatal error in Atomic mode
```

`ingest` owns deduplication, assertion-record retention, and stable label
assignment (encounter order, [[SPEC-025-linked-data-ingestion#REQ-011]]): the
core reasoner deduplicates identical fact literals and would otherwise keep an
arbitrary label, so the bridge resolves labels deterministically *before* the
core sees the theory while retaining every suppressed `(origin, graph)`
assertion in `assertions`. Deduplication for the reasoner and provenance
retention are thus separate concerns: the `Theory` is minimal, the report is
complete. On an atomic fatal error, `ingest` returns `Err(IngestError)` whose
`report` carries `theory = None`, the counters and diagnostics gathered so far,
and whose `cause` is the fatal `SourceError`.

## 9. Architecture Decisions

### ADR-001: Reuse the SPEC-024 predicate model (class = unary, property = binary)

**Status:** Proposed.
**Context:** RDF classes and properties have distinct arities; SPEC-024 already
blesses unary ≈ class, binary ≈ property.
**Decision:** Map `rdf:type` to a unary class predicate and all else to binary,
producing symbols that populate the SPEC-024 vocabulary directly.
**Consequences:** Ingested data is immediately inspectable via
`Vocabulary::derive`; no new identity model. Uniform-binary remains available as
a non-default option for callers who want lossless `rdf:type(s, C)`.

### ADR-002: Verbatim IRI identity; CURIEs for presentation only

**Status:** Proposed (revised in 0.2.0; superseded the 0.1.0
CURIE-as-identity design).
**Context:** Compacting IRIs to CURIEs *at identity level* breaks global
identity: two documents can bind one prefix to different namespaces (collision)
or different prefixes to one namespace (split), and the prefix table is not
retained in `Theory`, so identity would depend on unrecoverable context.
**Decision:** Intern the full IRI verbatim as the symbol. Perform CURIE
compaction only when rendering for humans, through one global deterministic
registry ([[SPEC-025-linked-data-ingestion#CON-002]]).
**Consequences:** Cross-source identity is exactly IRI equality, as RDF
defines it; displays stay readable; round-trippable SPL emission uses full
IRIs and relies on SPEC-024 indicator quoting for `/`.

### ADR-003: Explicit triples only; no entailment, no T-Box → rules

**Status:** Proposed (wording corrected in 0.2.0: schema *triples* are
ingested as inert facts; what is excluded is *entailment*).
**Context:** OWL/RDFS is open-world/monotonic; Spindle is closed-world/
defeasible. Axiom-to-rule translation is semantically lossy and needs its own
formal policy ([[SPEC-024-predicate-model-and-vocabulary#ADR-007]], §15).
**Decision:** Ingest every explicitly asserted triple as a plain fact —
including RDFS/OWL schema assertions, which arrive as inert facts like any
other — and run no entailment. Defer schema-to-rules translation.
**Consequences:** A sound first bridge; the hard semantic mapping is isolated
to a future specification. "A-Box bridge" remains the colloquial name because
schema gains no semantics, even though schema triples are not filtered out.

### ADR-004: Provenance via the existing claims mechanism — annotation, not reconciliation

**Status:** Proposed (claim narrowed in 0.2.0).
**Context:** Spindle already attributes facts to sources and lets callers
weight them (`claims`/`source`/`trusts`). However, imported triples are strict
facts: the reasoner keeps conflicting strict facts definitely provable, and
identical facts are deduplicated by literal in the core. Trust weighting today
does not arbitrate between strict facts.
**Decision:** Reuse `claims`/`source`/`trusts` for *attribution and
weighting inputs*; do not add a provenance model; do not claim automatic
conflict reconciliation. Deterministic duplicate labeling and full claimant
reporting live in the bridge ([[SPEC-025-linked-data-ingestion#CON-008]]).
Trust-aware reconciliation (e.g. a defeasible ingestion mode or core
alternative-proof aggregation) is deferred.
**Consequences:** Provenance is honest and inert
([[SPEC-025-linked-data-ingestion#NFR-003]]); the SPL emitter is a `claims`
renderer; the reconciliation problem is named, scoped, and deferred rather
than implied.

### ADR-005: oxrdf/oxrdfio dependency

**Status:** Proposed.
**Context:** Hand-rolling Turtle/RDF-XML/SPARQL parsers fails composition-first.
**Decision:** Depend on `oxrdf` (data model) and `oxrdfio` (format-unified
parser). Phase 2 parses `CONSTRUCT` responses as RDF graphs via the same
stack. Licenses (MIT/Apache) are compatible with LGPL-3.0-or-later.
**Consequences:** Robust, streaming, maintained parsing; concrete terms map
directly onto `Term`. `sophia` was considered and rejected for heavier generics.

### ADR-006: owl:sameAs is not merged

**Status:** Proposed.
**Context:** Identity merging changes the individual set and interacts with
closed-world negation.
**Decision:** Emit `owl:sameAs(a, b)` as a plain fact; never coalesce.
**Consequences:** Identity handling stays explicit and user-controlled (a rule
or a future spec), not a silent ingest-time transform.

### ADR-007: Phased sources behind one seam

**Status:** Proposed.
**Context:** Static parsing, SPARQL, and dereferencing differ vastly in cost and
risk.
**Decision:** One `TripleSource` trait and one `ingest` driver; ship
`FileSource` first; add SPARQL `CONSTRUCT` and dereferencing later without
touching the pure `Mapping`.
**Consequences:** Incremental delivery; the pure core is stable and fully tested
before any network code exists.

### ADR-008: Claimant is the retrieval origin, not the graph name

**Status:** Proposed (new in 0.2.0).
**Context:** A named graph's name is part of the untrusted document: RDF does
not require the name to denote the graph, the name may be a blank node, and a
hostile source can name its graph after a trusted IRI to inherit its
`(trusts ...)` weight ([[W3C RDF 1.2 Concepts]]).
**Decision:** The `claims` source is the retrieval origin — canonical file
path, fetched URL, or authenticated endpoint IRI — supplied by the effectful
adapter, which is the only component that knows where bytes actually came
from. The graph name is preserved as separate, inert `graph` context metadata.
**Consequences:** Trust weights attach to something the ingesting party
verified; graph context stays queryable; blank-node graph names are
skolemized like any blank node.

## 10. Validation and Error Model

| Boundary | Error type | Examples |
|---|---|---|
| Source I/O | `SourceError` (`fatal` classified) | file/network failure, parse error, timeout, limit exceeded, disallowed scheme/address |
| Mapping | `MapError` | non-IRI predicate, skolem overflow |
| Ingest driver | `IngestReport.diagnostics` | datatype degrade, `?`-escape, `lossy_spl`, duplicate claimant, prefix conflict |
| CLI | `CliError` | bad format flag, unreadable input |

Errors and diagnostics carry structural values (the offending IRI/quad, the
limit); human messages MAY include IRIs but consumers SHALL NOT parse them.
Atomic vs streaming behavior is defined in
[[SPEC-025-linked-data-ingestion#REQ-007]].

## 11. Test Specifications

### TEST-001: Class-aware mapping
- **Positive:** `rdf:type` → unary class fact; other triple → binary fact.
- **Negative input:** literal object with `rdf:type` predicate treated as binary (not a class).
- **Negative output:** assert arities (class/1, property/2) and that no rule is added.

### TEST-002: IRI identity and presentation compaction
- **Positive:** one IRI reached via different prefixes in different documents interns to one symbol; display compaction with a registry entry renders the CURIE.
- **Negative input:** two documents binding `ex:` to different namespaces; two prefixes naming one namespace; a source prefix suggestion conflicting with the registry.
- **Negative output:** distinct namespaces never collide; the lexicographically first prefix wins display; conflicting suggestions are dropped with a diagnostic; identity output is unaffected by any registry change.

### TEST-003: Datatype conversion totality
- **Positive:** integer/decimal/double/float/string map to the expected `Term`; `xsd:float` routes through binary32.
- **Negative input:** integer beyond `i64`; integer/decimal beyond `rust_decimal` range; non-finite double; ill-typed numeric lexical form; language-tagged string.
- **Negative output:** beyond-`i64` → `Decimal`; beyond-`Decimal` → `Symbol` + diagnostic; ill-typed → `Symbol` + diagnostic; no literal aborts ingestion; numeric kinds stay distinct in `Vocabulary::derive`.

### TEST-004: Skolemization
- **Positive:** repeated `_:b1` in one dataset unifies; `_:b1` shared across two named graphs of one TriG dataset unifies; re-ingesting the same dataset with the same options reproduces identical skolem symbols (determinism).
- **Negative input:** same label across two distinct dataset identities (two origins, or two documents of a dereferencing source); the same dataset re-ingested under distinct explicit instance tokens.
- **Negative output:** blank nodes from distinct dataset identities never unify; with distinct instance tokens the two ingestions do not unify (and REQ-011 byte-identity is not claimed across differing tokens).

### TEST-005: Ground output and variable escaping
- **Positive:** every fact is a `GroundLiteral`.
- **Negative input:** RDF string literal `"?name"`; IRI containing `?` in a non-leading position (query string) left untouched.
- **Negative output:** the theory has zero non-fact rules; leading `?` escapes to `%3F` with a diagnostic; grounding finds no variables in any ingested fact.

### TEST-006: Provenance parity and claimant integrity
- **Positive:** per-origin `source` metadata equals the `claims`-produced value; graph name lands in `graph` metadata.
- **Negative input:** provenance on vs off; a document whose graph name is a trusted party's IRI; a blank-node graph name.
- **Negative output:** conclusions identical either way (NFR-003); the hostile graph name never becomes the claimant; blank-node graph names are skolemized.

### TEST-007: TripleSource / ingest driver / static adapter
- **Positive:** Turtle/N-Triples/N-Quads/TriG/RDF-XML ingest to identical facts.
- **Negative input:** malformed document in atomic mode; the same in streaming mode.
- **Negative output:** atomic → `Err(IngestError)` whose `cause` is the fatal `SourceError` and whose `report.theory` is `None` (counters/diagnostics present); streaming → `Ok` with a partial theory plus a diagnostic; never a silent partial theory.

### TEST-008: SPL round-trip (scoped)
- **Positive:** corpus without `lossy_spl` diagnostics: emit (full IRIs), re-parse, compare facts + `source` metadata; re-emission byte-stable. `"2"^^xsd:decimal` and an integer beyond `i64` (promoted to `Decimal`) emit with a `.0` marker and re-parse as **equal** `Decimal`s with no `lossy_spl` diagnostic.
- **Negative input:** `"2"^^xsd:string`; a `Float` rendering without exponent; a fact carrying a `graph` context.
- **Negative output:** the numeric-string and exponent-less-float facts each produce a `lossy_spl` diagnostic naming them; the `graph` context is absent from the emitted SPL but present in the `IngestReport` assertion log; parse∘emit is idempotent from the first re-parse onward.

### TEST-009: Bounded ingestion
- **Positive:** ingest within limits.
- **Negative input:** exceed max-bytes / max-triples; (network) exceed request count, deadline, redirects; disallowed scheme; private-range address.
- **Negative output:** diagnostic, no panic, bounded output; denied network targets are never contacted.

### TEST-010: Non-entailment
- **Positive:** `owl:sameAs` + RDFS axioms ingest as literal facts only.
- **Negative output:** no derived memberships or merged individuals appear pre-reasoning.

### TEST-011: Determinism (byte-identical scope)
- **Positive:** re-ingest a byte-identical source; byte-identical theory/SPL/`IngestReport`; duplicate labels assigned by encounter order.
- **Negative input:** shuffled triple order (where the format permits).
- **Negative output:** shuffled input yields an isomorphic fact set (modulo skolem spellings and label assignment) — byte-identity is **not** asserted for reordered input, per [[SPEC-025-linked-data-ingestion#REQ-011]].

## 12. Traceability Matrix

| Requirement | Contract / decision | Verification |
|---|---|---|
| [[SPEC-025-linked-data-ingestion#REQ-001]] | [[SPEC-025-linked-data-ingestion#CON-001]], [[SPEC-025-linked-data-ingestion#ADR-001]] | [[SPEC-025-linked-data-ingestion#TEST-001]] |
| [[SPEC-025-linked-data-ingestion#REQ-002]] | [[SPEC-025-linked-data-ingestion#CON-002]], [[SPEC-025-linked-data-ingestion#ADR-002]] | [[SPEC-025-linked-data-ingestion#TEST-002]] |
| [[SPEC-025-linked-data-ingestion#REQ-003]] | [[SPEC-025-linked-data-ingestion#CON-003]] | [[SPEC-025-linked-data-ingestion#TEST-003]] |
| [[SPEC-025-linked-data-ingestion#REQ-004]] | [[SPEC-025-linked-data-ingestion#CON-001]] | [[SPEC-025-linked-data-ingestion#TEST-004]] |
| [[SPEC-025-linked-data-ingestion#REQ-005]] | [[SPEC-025-linked-data-ingestion#CON-001]] | [[SPEC-025-linked-data-ingestion#TEST-005]] |
| [[SPEC-025-linked-data-ingestion#REQ-006]] | [[SPEC-025-linked-data-ingestion#CON-004]], [[SPEC-025-linked-data-ingestion#ADR-004]], [[SPEC-025-linked-data-ingestion#ADR-008]] | [[SPEC-025-linked-data-ingestion#TEST-006]] |
| [[SPEC-025-linked-data-ingestion#REQ-007]] | [[SPEC-025-linked-data-ingestion#CON-005]], [[SPEC-025-linked-data-ingestion#CON-008]], [[SPEC-025-linked-data-ingestion#ADR-005]], [[SPEC-025-linked-data-ingestion#ADR-007]] | [[SPEC-025-linked-data-ingestion#TEST-007]] |
| [[SPEC-025-linked-data-ingestion#REQ-008]] | [[SPEC-025-linked-data-ingestion#CON-004]], [[SPEC-025-linked-data-ingestion#CON-008]] | [[SPEC-025-linked-data-ingestion#TEST-008]] |
| [[SPEC-025-linked-data-ingestion#REQ-009]] | [[SPEC-025-linked-data-ingestion#CON-006]] | [[SPEC-025-linked-data-ingestion#TEST-009]] |
| [[SPEC-025-linked-data-ingestion#REQ-010]] | [[SPEC-025-linked-data-ingestion#ADR-003]], [[SPEC-025-linked-data-ingestion#ADR-006]] | [[SPEC-025-linked-data-ingestion#TEST-010]] |
| [[SPEC-025-linked-data-ingestion#REQ-011]] | [[SPEC-025-linked-data-ingestion#NFR-001]], [[SPEC-025-linked-data-ingestion#CON-008]] | [[SPEC-025-linked-data-ingestion#TEST-011]] |
| [[SPEC-025-linked-data-ingestion#NFR-003]] | [[SPEC-024-predicate-model-and-vocabulary#REQ-013]] | [[SPEC-025-linked-data-ingestion#TEST-006]] |

## 13. Deferred Decisions

| Decision | Rationale for deferral | Trigger |
|---|---|---|
| Trust-aware conflict reconciliation (defeasible ingestion mode; core alternative-proof aggregation across claimants) | Imported triples are strict facts; the core deduplicates identical literals and keeps conflicting strict facts provable (G-4) | Multi-source arbitration use case; core reasoner spec |
| Tagged term encoding (IRI vs string vs lang-tagged vs skolem) | `Term` has four untagged variants; collisions documented in [[SPEC-025-linked-data-ingestion#Term Projection]] | Fidelity requirement from a consumer; `Term` extension spec |
| Typed SPL term syntax (or quote-aware string terms) | SPL re-infers term variants from spelling, so `Symbol("2")`-style facts cannot round-trip byte-exactly ([[SPEC-025-linked-data-ingestion#REQ-008]]) | SPL grammar revision |
| Parseable graph-context SPL syntax (e.g. a `:graph` `claims` field + parser contract) | `(given ...)` facts have no per-fact metadata grammar, so named-graph context — though present on the `Theory` fact and in the `IngestReport` (G-5) — is not round-tripped through SPL text | Consumer needing graph context in SPL text |
| T-Box → rules (RDFS/OWL entailment) | Open-world/monotonic vs closed-world/defeasible needs a formal mapping policy | Successor spec; interoperability use case |
| SHACL validation onto `Shape` | Needs a SHACL-core → `PredicateSignature`/`Shape` mapping | Validation use case; builds on SPEC-024 `Shape` |
| SPARQL `SELECT` row mapping | `SELECT` yields solution mappings, not a graph; needs its own row → fact contract | Analyst demand beyond `CONSTRUCT` |
| [[RDF Dataset Canonicalization|RDF dataset canonicalization (RDFC-1.0)]] for reorder-invariant skolem naming | Encounter-order naming is deterministic only for byte-identical input ([[SPEC-025-linked-data-ingestion#REQ-011]]) | Graph-isomorphism-invariant output requirement |
| JSON-LD ingestion | Context processing and possibly remote context fetching | After Phase 1 stabilizes |
| `owl:sameAs` merging | Identity coalescence interacts with negation-as-failure | Successor spec |
| Non-numeric datatype fidelity (bool/date/lang) | Subsumed by tagged term encoding; dates may route through the temporal system | `Term` extension or temporal mapping spec |
| RDF export (Spindle → RDF/OWL) | The reverse bridge; different concerns | Interop use case |

## 14. Review and Comprehension Gates

Tier 2 (new public API and file/network I/O). This document SHALL remain `draft`
until: a different model family reviews the mapping soundness and the
open/closed-world boundary; a human maintainer approves the explicit-triples
scope, the term projection, the identity and provenance/claimant design, and
the ingest error model; and `zetl` validation reports no unexplained dead
technical-concept links. It depends on the
[[SPEC-024-predicate-model-and-vocabulary]] branch being merged.

Review round 1 (adversarial, different model family) returned 11 findings
(7 high, 4 medium); all are addressed in 0.2.0. Review round 2 returned 7
findings (4 P1, 3 P2) on identity/error contracts, provenance-preserving source
and dedup APIs, and SPL round-trip of common terms and graph metadata; all are
addressed in 0.3.0. Review round 3 returned 4 findings (1 P1, 3 P2) on the
unexposed instance-token option, graph provenance in the canonical `Theory`, and
fixed-token determinism; all are addressed in 0.4.0. Round 4 (1 P2) aligned the
REQ-011 and NFR-001 determinism clauses with the fixed-token behavior; addressed
in 0.4.1 — see the changelog.

Dead-link note: the concept links [[W3C RDF 1.2 Concepts]],
[[SPARQL 1.1 Query Language]], and [[RDF Dataset Canonicalization]] are
intentionally dead pending concept pages for these external standards
(deferral owner: core maintainers; the URLs are inlined at first mention).
Links to [[SPEC-024-predicate-model-and-vocabulary]] resolve once that branch
merges.

Current gate status: **pending** (rounds 2–4 addressed in 0.3.0–0.4.1; awaiting
human maintainer approval and dependent SPEC-024 merge).

## Changelog

<details>
<summary>Revision history</summary>

- 0.4.1 (2026-07-14): Address adversarial review round 4 (1 finding). [P2]
  Aligned the normative determinism clauses with the 0.4.0 fixed-token behavior:
  REQ-011 and NFR-001 now define skolem naming over `(effective dataset key,
  encounter ordinal)` — the key including `instance_token` when set — and exclude
  only runs whose token values *differ*, so a fixed token stays deterministic
  ([[SPEC-025-linked-data-ingestion#REQ-011]],
  [[SPEC-025-linked-data-ingestion#NFR-001]]).
- 0.4.0 (2026-07-14): Address adversarial review round 3 (4 findings). [P1]
  Exposed the ingestion-instance token as `IngestOptions::instance_token` and
  defined the **effective dataset key** (`(token, DatasetId)`) it forms for
  skolem scoping, so the REQ-004/TEST-004 freshness mechanism is actually
  callable ([[SPEC-025-linked-data-ingestion#CON-008]],
  [[SPEC-025-linked-data-ingestion#REQ-004]]). [P2] Narrowed the token
  determinism exception: a fixed token is fully deterministic (REQ-011 holds);
  only *different* token values change output
  ([[SPEC-025-linked-data-ingestion#REQ-004]]). [P2] Restored `graph` provenance
  to the canonical `Theory` (first deduplicated context on the fact; full set in
  the assertion log), reconciling REQ-006/ADR-008/TEST-006/§3.7 — only *SPL
  emission* drops it ([[SPEC-025-linked-data-ingestion#CON-004]],
  [[SPEC-025-linked-data-ingestion#REQ-008]], G-5). [P2] Updated the §2.1 scope
  summary from per-ingestion-run to dataset-identity skolem semantics.
- 0.3.0 (2026-07-14): Address adversarial review round 2 (7 findings).
  [P1] `TripleSource` now streams `AttributedQuad` carrying per-quad origin and
  a `DatasetId` scope, so multi-document sources attribute each quad and isolate
  blank nodes per document ([[SPEC-025-linked-data-ingestion#CON-005]],
  [[SPEC-025-linked-data-ingestion#REQ-006]]). [P1] Reconciled blank-node scope
  with determinism: skolem identity is a function of `(dataset identity,
  encounter ordinal)` (deterministic, re-ingestion-stable), cross-run freshness
  is opt-in via an instance token excluded from the byte-identical guarantee
  ([[SPEC-025-linked-data-ingestion#REQ-004]],
  [[SPEC-025-linked-data-ingestion#REQ-011]],
  [[SPEC-025-linked-data-ingestion#NFR-001]]). [P1] Replaced the
  multi-origin-only `duplicates` list with an `AssertionRecord` log retaining
  every `(origin, graph)` assertion, separating reasoning dedup from provenance
  retention ([[SPEC-025-linked-data-ingestion#CON-008]], new G-5). [P1] `ingest`
  returns `Err(IngestError)` carrying the partial report (`theory = None`) and
  fatal cause, so atomic-failure counters/diagnostics survive
  ([[SPEC-025-linked-data-ingestion#CON-008]],
  [[SPEC-025-linked-data-ingestion#REQ-007]]). [P2] Every `Decimal` renders with
  a fractional marker so zero-scale and beyond-`i64` decimals round-trip as equal
  `Decimal`s ([[SPEC-025-linked-data-ingestion#REQ-008]]). [P2] Scoped `graph`
  context out of SPL round-trip (retained in the `IngestReport`; parseable syntax
  deferred — G-5). [P2] Scoped the round-trip guarantee to full-IRI emission;
  compacted display is explicitly non-round-trippable
  ([[SPEC-025-linked-data-ingestion#REQ-002]]).
- 0.2.0 (2026-07-14): Address adversarial review round 1 (11 findings).
  Narrowed the trust claim to provenance *annotation* and named the
  strict-fact/deduplication limits (new G-4, revised
  [[SPEC-025-linked-data-ingestion#ADR-004]]); documented the term mapping as
  an intentionally lossy projection with enumerated collisions (new
  [[SPEC-025-linked-data-ingestion#Term Projection]]); re-scoped SPL
  round-trip to injective spellings with `lossy_spl` diagnostics and added
  mandatory `?`-escaping ([[SPEC-025-linked-data-ingestion#REQ-005]],
  [[SPEC-025-linked-data-ingestion#REQ-008]]); corrected "A-Box only" to
  "explicit triples without schema entailment"
  ([[SPEC-025-linked-data-ingestion#ADR-003]],
  [[SPEC-025-linked-data-ingestion#REQ-010]]); switched internal identity to
  verbatim full IRIs with presentation-only CURIE compaction (revised
  [[SPEC-025-linked-data-ingestion#ADR-002]],
  [[SPEC-025-linked-data-ingestion#REQ-002]],
  [[SPEC-025-linked-data-ingestion#CON-002]]); re-scoped blank-node
  skolemization to the ingestion instance across graphs and limited
  byte-determinism to byte-identical input
  ([[SPEC-025-linked-data-ingestion#REQ-004]],
  [[SPEC-025-linked-data-ingestion#REQ-011]]); made the retrieval origin the
  claimant with graph name as context metadata (new
  [[SPEC-025-linked-data-ingestion#ADR-008]]); added the `ingest` driver,
  `IngestReport`, atomic/streaming modes, and fatal error classification (new
  [[SPEC-025-linked-data-ingestion#CON-008]]); restricted Phase 2 to SPARQL
  `CONSTRUCT`; made datatype conversion total with `rust_decimal`-range and
  ill-typed-lexical handling and binary32 routing for `xsd:float`
  ([[SPEC-025-linked-data-ingestion#REQ-003]],
  [[SPEC-025-linked-data-ingestion#CON-003]]); extended network bounds
  (requests, bytes, deadline, redirects, schemes, private-address denial,
  content types — [[SPEC-025-linked-data-ingestion#REQ-009]],
  [[SPEC-025-linked-data-ingestion#CON-006]]); fixed the CLI (`nquads`
  format, URL deferred to Phases 2–3).
- 0.1.0 (2026-07-14): Initial draft. A-Box linked-data ingestion built on the
  SPEC-024 predicate model: class-aware mapping, CURIE compaction, typed-literal
  conversion, skolemization, claims-based provenance/trust, a phased
  `TripleSource` (static → SPARQL → dereference), and `Theory`/SPL output.
  Includes an explicit SPEC-024 affordance analysis (§4).

</details>
