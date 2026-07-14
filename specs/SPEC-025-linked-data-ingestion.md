---
id: SPEC-025
title: Linked-Data and Semantic-Web Ingestion (A-Box Bridge)
status: draft
version: 0.1.0
created: 2026-07-14
last-updated: 2026-07-14
authors: Claude (Opus 4.8, AI agent)
reviewers: Core Maintainers (pending)
protocol: USDD Agent Protocol v1.11.0
---

# SPEC-025: Linked-Data and Semantic-Web Ingestion (A-Box Bridge)

## Orientation

**Intent:** Let Spindle ingest instance data (RDF A-Box triples / linked data)
from files, SPARQL endpoints, and dereferenceable IRIs, turning it into a
defeasible-reasoning-ready [[SPEC-025-linked-data-ingestion#Theory|theory]] of
facts — building on the [[SPEC-024-predicate-model-and-vocabulary|SPEC-024
predicate model]] rather than inventing a parallel ontology subsystem.

**Metaphor:** A triple is a ground binary (or, for `rdf:type`, unary) fact. A
linked-data *source* is a claimant: its triples enter a
[[SPEC-025-linked-data-ingestion#Claims Provenance|claims block]] so Spindle's
trust module can weigh and reconcile competing sources. The ontology's *schema*
(T-Box) is deliberately **not** ingested in this revision; only its assertions
are.

**Key decisions:**

- Ingest **A-Box only**. A triple maps to a Spindle fact; no RDFS/OWL inference
  and no T-Box → rules ([[SPEC-025-linked-data-ingestion#ADR-003]]).
- **Class-aware mapping**: `(s rdf:type C)` → unary `C(s)`; every other
  `(s p o)` → binary `p(s o)`, honoring
  [[SPEC-024-predicate-model-and-vocabulary#ADR-001]] (unary ≈ class, binary ≈
  property) ([[SPEC-025-linked-data-ingestion#ADR-001]]).
- IRIs become **CURIEs with full-IRI fallback**; functors and arguments are
  interned Spindle symbols ([[SPEC-025-linked-data-ingestion#ADR-002]]).
- Per-source **provenance uses the existing `claims` mechanism**; no second
  provenance model is introduced ([[SPEC-025-linked-data-ingestion#ADR-004]]).
- Sources sit behind one [[SPEC-025-linked-data-ingestion#Triple Source|`TripleSource`]]
  seam; a static-document adapter ships first, SPARQL and dereferencing follow
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
  │ DerefSource  (Phase 3)│        │ CURIE • datatype •    │
  └───────────────────────┘        │ skolem • class-aware  │
                                    └──────────┬────────────┘
                                               │ GroundLiteral + source
                                    ┌──────────▼────────────┐
                                    │ Theory  (+ claims)    │
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
be interpreted as in [[usdd-agent-protocol#Requirement-Level Keywords (BCP 14)|BCP
14]] when, and only when, they appear in all capitals.

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
of the problem with a *sound, lossless* mapping: **assertions** (the A-Box). A
triple is simply a ground fact; asserting it in Spindle is faithful. Schema
translation, SHACL validation, and `owl:sameAs` merging are deferred to their
own specifications ([[SPEC-025-linked-data-ingestion#Deferred Decisions]]).

### 1.1 User goals

| User | Goal | Success criterion |
|---|---|---|
| Data engineer | Load a Turtle/N-Triples dataset as facts | A `Theory` of ground facts, reasoned over unchanged |
| Integrator | Combine several linked-data sources | Each source is trust-weightable; conflicts resolve via superiority/trust |
| Analyst | Query a SPARQL endpoint and reason over the results | Result rows become facts through the same mapping |
| Tooling author | Inspect the ingested vocabulary | `Vocabulary::derive` yields per-predicate signatures and argument profiles |

## 2. Scope

### 2.1 In scope

- A pure `Mapping` from an RDF triple/quad to a Spindle ground fact.
- Class-aware `rdf:type` handling and binary property handling.
- CURIE compaction with full-IRI fallback and a seedable prefix table.
- Typed-literal → `Term` conversion for the value space Spindle represents.
- Per-source blank-node skolemization.
- Per-source provenance via the existing `claims`/`source` metadata mechanism.
- A `TripleSource` trait and a Phase-1 static-document adapter (Turtle,
  N-Triples, TriG, RDF-XML) via `oxrdfio`.
- Bounded, streaming ingestion with size/time/depth limits.
- `Theory` output plus an optional SPL emitter, and a CLI `import` subcommand.

### 2.2 Out of scope (this revision)

- RDFS/OWL entailment or any T-Box → rules translation.
- SHACL validation (a later mapping onto [[SPEC-024-predicate-model-and-vocabulary#Shape|`Shape`]]).
- `owl:sameAs` identity merging (emitted as a plain fact only).
- JSON-LD ingestion (deferred; needs context processing).
- Exporting Spindle theories back out as RDF/OWL.
- Named-graph reasoning semantics beyond using the graph IRI as a provenance key.

## 3. Terminology

### 3.1 Triple / Quad

A **Triple** is `(subject, predicate, object)`; a **Quad** additionally carries a
graph name. Subjects are IRIs or blank nodes; predicates are IRIs; objects are
IRIs, blank nodes, or literals.

### 3.2 Triple Source

A **TripleSource** is an effectful producer of quads. Adapters: `FileSource`
(static documents), `SparqlSource` (a `SELECT`/`CONSTRUCT` endpoint), and
`DereferenceSource` (follow-your-nose IRI dereferencing). All obey
[[SPEC-025-linked-data-ingestion#REQ-009]] (bounded ingestion).

### 3.3 Mapping

A **Mapping** is the pure function from a quad to a Spindle
[[SPEC-024-predicate-model-and-vocabulary#Ground Literal|`GroundLiteral`]] plus a
provenance key. It performs class-aware structuring, CURIE compaction, datatype
conversion, and skolemization. It performs no I/O.

### 3.4 CURIE

A **CURIE** is a compact IRI `prefix:local` (e.g. `foaf:knows`) resolved through
a prefix table. When no prefix matches, the mapping falls back to the full IRI
string. Either way the result is one interned Spindle symbol.

### 3.5 Class-Aware Mapping

Under **class-aware mapping**, `(s rdf:type C)` becomes the unary fact `C(s)`
and every other triple becomes the binary fact `p(s o)`. This aligns instance
data with [[SPEC-024-predicate-model-and-vocabulary#ADR-001]] so classes surface
as unary predicates in the derived
[[SPEC-024-predicate-model-and-vocabulary#Vocabulary|vocabulary]].

### 3.6 Claims Provenance

Ingested facts are grouped by source (graph or document IRI) and attributed
through Spindle's existing `(claims source ...)` mechanism, which stamps each
fact with a `source` metadatum. Callers add `(trusts source weight)` to enable
trust-weighted, conflict-resolving reasoning across sources.

### 3.7 Theory

A **Theory** is Spindle's canonical aggregate (rules, facts, superiority,
metadata, trust). Ingestion produces facts and provenance metadata only; it adds
no rules.

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
| Full-IRI functor (CURIE fallback) containing `/` | Indicator recognizer quotes slash-bearing functors ([[SPEC-024-predicate-model-and-vocabulary#CON-002]]) | Afforded |
| Later SHACL-style validation of ingested data | [[SPEC-024-predicate-model-and-vocabulary#Shape|`Shape`]] | Afforded (deferred use) |
| Per-triple source provenance + trust | Existing `claims`/`Meta` `source` + trust module (pre-SPEC-024) | Afforded (platform) |

**Identified gaps (documented, not blocking A-Box v1):**

- **G-1 — value space:** `Term` has four variants. Non-numeric typed literals
  (`xsd:boolean`, `xsd:date`, language-tagged strings) degrade to a `Symbol`
  of their lexical form. This is a `Term` limitation, not a SPEC-024 gap; date
  literals MAY later route through the temporal system
  ([[SPEC-025-linked-data-ingestion#Deferred Decisions]]).
- **G-2 — instance vs schema provenance:** SPEC-024 predicate metadata is
  *schema-level* (per `PredicateSymbol`), while linked-data provenance is
  *instance-level* (per triple). The bridge correctly uses the schema channel
  for `rdfs:comment` and the `claims` channel for triple provenance; both exist.
- **G-3 — no core namespace concept:** prefix tables live entirely in this
  bridge crate; the core neither needs nor gains a namespace type. This is a
  boundary, not a gap.

**Verdict:** the predicate branch affords everything A-Box ingestion needs, and
several pieces (`ArgumentProfile`, class-as-unary, the slash-quoting indicator
recognizer, predicate metadata for `rdfs:comment`, `GroundLiteral`) are close to
purpose-built for it. The only real limit is `Term`'s value space (G-1), which
this spec accepts for v1.

## 5. Architecture

### 5.1 Purity boundary

Mirrors [[SPEC-024-predicate-model-and-vocabulary#Purity Boundary Map]].

- **Pure core (`spindle-rdf::map`)**: quad → `GroundLiteral` + provenance key;
  CURIE compaction; datatype conversion; skolemization; class-aware structuring;
  Theory assembly; SPL rendering. No file, network, clock, or environment I/O.
- **Effectful shell (`spindle-rdf::source`)**: `TripleSource` adapters
  (`FileSource`, `SparqlSource`, `DereferenceSource`) and the CLI `import`
  command. Only these perform I/O.

The pure core SHALL NOT import the source adapters.

### 5.2 Placement (Simplicity Ladder)

New capability sits at rung 4–5: a new `spindle-rdf` crate that *composes*
existing components — `Theory`, `Term`, `intern`, the SPEC-024 vocabulary types,
the `claims`/trust mechanism — plus one external RDF-parsing dependency. No new
reasoning, identity, or ontology subsystem is introduced.

### 5.3 Phasing

1. **Phase 1 — static documents:** `FileSource` over `oxrdfio` (Turtle,
   N-Triples, N-Quads, TriG, RDF-XML). Pure `Mapping`. `Theory` + SPL output.
   CLI `import`.
2. **Phase 2 — SPARQL:** `SparqlSource` issuing `SELECT`/`CONSTRUCT` and parsing
   results (`sparesults`) over a small HTTP client.
3. **Phase 3 — dereferencing:** `DereferenceSource` following IRIs with depth,
   count, and time bounds and a visited-set for cycles.

Each phase is independently shippable; the `Mapping` core is unchanged across
them.

## 6. Functional Requirements

### REQ-001: Class-aware triple mapping

The mapping SHALL translate `(s rdf:type C)` (where `C` is an IRI) to the unary
ground fact `C(s)`, and every other triple `(s p o)` to the binary ground fact
`p(s o)`.

**Acceptance criteria:** `(ex:alice rdf:type foaf:Person)` yields fact
`foaf:Person(ex:alice)` — `foaf:Person/1`; `(ex:alice foaf:knows ex:bob)` yields
`foaf:knows(ex:alice, ex:bob)` — `foaf:knows/2`. A triple whose object is a
literal never becomes a class fact.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-001]], [[SPEC-025-linked-data-ingestion#ADR-001]], [[SPEC-025-linked-data-ingestion#TEST-001]].

### REQ-002: CURIE compaction with fallback

IRIs SHALL be compacted to `prefix:local` when a prefix in the active table
matches the longest namespace, and otherwise retained as the full IRI string.
Either result is one interned symbol. Compaction SHALL be deterministic given
the same prefix table.

**Acceptance criteria:** with `foaf: <http://xmlns.com/foaf/0.1/>`,
`<http://xmlns.com/foaf/0.1/knows>` compacts to `foaf:knows`; an unprefixed IRI
is kept verbatim and, as a functor, is quoted by the SPEC-024 indicator
renderer. Two prefixes matching the same IRI resolve to the longest namespace.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-002]], [[SPEC-025-linked-data-ingestion#ADR-002]], [[SPEC-025-linked-data-ingestion#TEST-002]].

### REQ-003: Typed-literal conversion

Object literals SHALL convert to `Term` by datatype: `xsd:integer` (and its
sub/derived integer types) → `Integer` with checked range; `xsd:decimal` →
`Decimal`; `xsd:double`/`xsd:float` → `Float`; all other datatypes, plain
literals, and language-tagged strings → `Symbol` of the lexical form.

**Acceptance criteria:** `"2"^^xsd:integer` → `Integer(2)`; `"2.5"^^xsd:decimal`
→ `Decimal(2.5)`; `"foo"@en` → `Symbol("foo")`; an integer exceeding `i64` →
`Decimal`. `Vocabulary::derive` over the result reports the observed argument
kinds per predicate position.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-003]], [[SPEC-025-linked-data-ingestion#TEST-003]].

### REQ-004: Blank-node skolemization

Blank nodes SHALL be skolemized to fresh, unique symbols scoped to their source
document/graph, so identical labels in different sources do not unify.

**Acceptance criteria:** `_:b1` in two separately-ingested documents produces two
distinct symbols; `_:b1` twice within one document produces the same symbol.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-001]], [[SPEC-025-linked-data-ingestion#TEST-004]].

### REQ-005: Ground-fact output

Every mapped fact SHALL be a checked
[[SPEC-024-predicate-model-and-vocabulary#Ground Literal|`GroundLiteral`]]; the
mapping SHALL NOT emit variables, and ingestion adds no rule.

**Acceptance criteria:** the produced `Theory` has zero non-fact rules; each fact
passes `GroundLiteral::try_from`.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-001]], [[SPEC-025-linked-data-ingestion#TEST-005]].

### REQ-006: Per-source provenance

Ingestion SHALL attribute each fact to its source (graph IRI, else document IRI)
through the existing `claims`/`source` metadata mechanism, and SHALL NOT
introduce a second provenance model.

**Acceptance criteria:** a fact ingested from graph `ex:g` carries `source =
ex:g` metadata identical to what `(claims ex:g ...)` would produce; the SPL
emitter renders one `claims` block per source; conclusions are unchanged whether
provenance is attached or not.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-004]], [[SPEC-025-linked-data-ingestion#ADR-004]], [[SPEC-025-linked-data-ingestion#TEST-006]].

### REQ-007: TripleSource abstraction and static adapter

Ingestion SHALL consume quads through a `TripleSource` trait, and SHALL provide a
`FileSource` adapter over `oxrdfio` recognizing Turtle, N-Triples, N-Quads,
TriG, and RDF-XML.

**Acceptance criteria:** the same `Mapping` produces identical facts regardless
of which adapter supplied the quads; a well-formed Turtle document ingests
successfully; a malformed document yields a structured `SourceError` and no
partial theory unless streaming was explicitly requested.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-005]], [[SPEC-025-linked-data-ingestion#ADR-005]], [[SPEC-025-linked-data-ingestion#ADR-007]], [[SPEC-025-linked-data-ingestion#TEST-007]].

### REQ-008: Theory and SPL output

Ingestion SHALL return a `Theory`, and SHALL provide an emitter rendering that
theory (including `claims` provenance) to canonical SPL text.

**Acceptance criteria:** the emitted SPL re-parses to a theory whose facts and
source metadata equal the original; round-tripping is idempotent.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-004]], [[SPEC-025-linked-data-ingestion#TEST-008]].

### REQ-009: Bounded ingestion

Ingestion SHALL honor caller-configured limits: maximum input bytes, maximum
triples, and (for network sources) request timeout, maximum dereference depth,
and a visited-IRI set for cycle avoidance. Limits reached SHALL produce a
diagnostic, not a panic or unbounded output.

**Traces:** [[SPEC-025-linked-data-ingestion#CON-006]], [[SPEC-025-linked-data-ingestion#NFR-002]], [[SPEC-025-linked-data-ingestion#TEST-009]].

### REQ-010: Non-inference guarantee

Ingestion SHALL assert triples as facts only. It SHALL NOT perform RDFS/OWL
entailment, SHALL NOT translate schema to rules, and SHALL emit `owl:sameAs` as
a plain `owl:sameAs(a, b)` fact without merging identities.

**Acceptance criteria:** ingesting a document with `owl:sameAs` and RDFS axioms
adds only the literal triples as facts; no derived class memberships or merged
individuals appear until the reasoner runs user-supplied rules.

**Traces:** [[SPEC-025-linked-data-ingestion#ADR-003]], [[SPEC-025-linked-data-ingestion#ADR-006]], [[SPEC-025-linked-data-ingestion#TEST-010]].

### REQ-011: Determinism

For a structurally equal source and prefix table, the produced theory, its SPL
rendering, and its diagnostics SHALL be byte-identical across runs (independent
of hash-map iteration and, for skolem symbols, using a stable per-source
counter).

**Traces:** [[SPEC-025-linked-data-ingestion#NFR-001]], [[SPEC-025-linked-data-ingestion#TEST-011]].

## 7. Non-Functional Requirements

### NFR-001: Determinism

See [[SPEC-025-linked-data-ingestion#REQ-011]]. Skolem naming SHALL derive from a
deterministic per-source ordinal, never from a random or clock source.

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
    pub source: Option<Symbol>,   // provenance key (graph/document IRI, curied)
}

pub struct MappingOptions {
    pub prefixes: PrefixMap,
    pub class_aware: bool,        // default true (REQ-001)
    pub provenance: bool,         // default true (REQ-006)
    pub limits: IngestLimits,
}

/// Pure: no I/O. Skolem state is a per-source counter carried in `ctx`.
pub fn map_quad(quad: &Quad, ctx: &mut MapContext, opts: &MappingOptions)
    -> Result<MappedFact, MapError>;

pub fn assemble(facts: impl IntoIterator<Item = MappedFact>) -> Theory;
```

The subject/object/predicate use `oxrdf` terms as input; outputs are Spindle
`Term`/`Symbol`. A `Quad` with a variable is impossible (A-Box is ground), so
`map_quad` never yields a pattern.

### CON-002: CURIE grammar and prefix table

```ebnf
curie        = prefix ":" local | full-iri ;
prefix       = ncname-ish ;            (* seeded from source @prefix/PREFIX *)
local        = { iri-path-char } ;
full-iri     = absolute IRI as one interned symbol (verbatim) ;
```

A `PrefixMap` maps prefix → namespace IRI and carries an optional base. The
built-in table seeds `rdf`, `rdfs`, `owl`, `xsd`, `foaf`, `schema`, `dc`,
`dcterms`, `skos`. Longest-namespace match wins; ties are impossible because
namespaces with a common prefix are ordered by length. Full-IRI fallback keeps a
verbatim string; when it lands in functor position, the
[[SPEC-024-predicate-model-and-vocabulary#CON-002|SPEC-024 indicator renderer]]
quotes it because of `/`.

### CON-003: Literal datatype mapping

| RDF literal datatype | `Term` |
|---|---|
| `xsd:integer` and derived integer types | `Integer` (checked; overflow → `Decimal`) |
| `xsd:decimal` | `Decimal` |
| `xsd:double`, `xsd:float` | `Float` (non-finite → `Symbol` of lexical form) |
| `xsd:string`, plain, `rdf:langString` | `Symbol` (lexical form; language tag dropped in v1) |
| any other datatype IRI | `Symbol` (lexical form) |

### CON-004: Provenance and SPL output

Facts are grouped by `source`; each group renders as:

```spl
(claims <source-curie>
  (given (<Class> <individual>))
  (given (<predicate> <subject> <object>))
  ...)
```

which is exactly the existing `claims` block. Callers supply
`(trusts <source> <weight>)` separately. With `provenance = false`, facts render
as bare `(given ...)` statements.

### CON-005: TripleSource

```rust
pub trait TripleSource {
    /// Stream quads; a per-item error does not abort unless fatal.
    fn quads(&mut self)
        -> Result<Box<dyn Iterator<Item = Result<Quad, SourceError>> + '_>, SourceError>;
    /// The default provenance key for quads lacking an explicit graph.
    fn default_source(&self) -> Option<Symbol>;
}

pub struct FileSource { /* format, reader, base IRI, limits */ }   // Phase 1
pub struct SparqlSource { /* endpoint, query, http, limits */ }    // Phase 2
pub struct DereferenceSource { /* seed IRIs, depth, http, visited */ } // Phase 3
```

### CON-006: Limits

```rust
pub struct IngestLimits {
    pub max_input_bytes: usize,
    pub max_triples: usize,
    pub request_timeout: Duration,   // network sources
    pub max_deref_depth: u32,        // dereferencing
}
```

### CON-007: CLI

```text
spindle import [--format turtle|ntriples|trig|rdfxml] [--base IRI]
               [--trust WEIGHT] [--no-provenance] [--emit-spl] <file|url|->
```

`--emit-spl` prints SPL instead of reasoning; otherwise the imported theory is
reasoned and conclusions are printed, matching `spindle reason` output.

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

### ADR-002: CURIE with full-IRI fallback

**Status:** Proposed.
**Context:** Full IRIs as symbols are unambiguous but unreadable; CURIEs are
readable but need a prefix table and can be ambiguous.
**Decision:** Compact to CURIE when a prefix matches (longest wins), else keep
the full IRI. Never guess a prefix.
**Consequences:** Readable functors; nothing is lost; slash-bearing full IRIs
remain representable through SPEC-024 indicator quoting.

### ADR-003: A-Box only; no T-Box → rules

**Status:** Proposed.
**Context:** OWL/RDFS is open-world/monotonic; Spindle is closed-world/
defeasible. Axiom-to-rule translation is semantically lossy and needs its own
formal policy ([[SPEC-024-predicate-model-and-vocabulary#ADR-007]], §15).
**Decision:** Ingest assertions only. Defer schema translation.
**Consequences:** A sound, lossless first bridge; the hard semantic mapping is
isolated to a future specification.

### ADR-004: Provenance via the existing claims mechanism

**Status:** Proposed.
**Context:** Spindle already attributes facts to sources and trust-weights them.
**Decision:** Reuse `claims`/`source`/`trusts`; do not add a provenance model.
**Consequences:** Multi-source linked data becomes a trust-weighted defeasible
problem Spindle already solves; the SPL emitter is a `claims` renderer.

### ADR-005: oxrdf/oxrdfio dependency

**Status:** Proposed.
**Context:** Hand-rolling Turtle/RDF-XML/SPARQL parsers fails composition-first.
**Decision:** Depend on `oxrdf` (data model) and `oxrdfio` (format-unified
parser), later `sparesults` for SPARQL results. Licenses (MIT/Apache) are
compatible with LGPL-3.0-or-later.
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
**Decision:** One `TripleSource` trait; ship `FileSource` first; add SPARQL and
dereferencing later without touching the pure `Mapping`.
**Consequences:** Incremental delivery; the pure core is stable and fully tested
before any network code exists.

## 10. Validation and Error Model

| Boundary | Error type | Examples |
|---|---|---|
| Source I/O | `SourceError` | file/network failure, parse error, timeout, limit exceeded |
| Mapping | `MapError` | non-IRI predicate, unrepresentable value (documented degrade path), skolem overflow |
| CLI | `CliError` | bad format flag, unreadable input |

Errors carry structural values (the offending IRI/quad, the limit); human
messages MAY include IRIs but consumers SHALL NOT parse them.

## 11. Test Specifications

### TEST-001: Class-aware mapping
- **Positive:** `rdf:type` → unary class fact; other triple → binary fact.
- **Negative input:** literal object with `rdf:type` predicate treated as binary (not a class).
- **Negative output:** assert arities (`Class/1`, `property/2`) and that no rule is added.

### TEST-002: CURIE compaction
- **Positive:** known prefix compacts; longest namespace wins.
- **Negative input:** unknown-namespace IRI kept verbatim; slash-bearing full IRI quoted as a functor.
- **Negative output:** compaction deterministic; never invents a prefix.

### TEST-003: Datatype conversion
- **Positive:** integer/decimal/double/string map to the expected `Term`.
- **Negative input:** out-of-range integer; non-finite double; language-tagged string.
- **Negative output:** overflow → `Decimal`; non-finite → `Symbol`; numeric kinds stay distinct in `Vocabulary::derive`.

### TEST-004: Skolemization
- **Positive:** repeated `_:b1` in one document unifies.
- **Negative input:** same label across two documents.
- **Negative output:** cross-document blank nodes never unify.

### TEST-005: Ground output
- **Positive:** every fact is a `GroundLiteral`.
- **Negative output:** the theory has zero non-fact rules.

### TEST-006: Provenance parity
- **Positive:** per-source `source` metadata equals the `claims`-produced value.
- **Negative input:** provenance on vs off.
- **Negative output:** conclusions identical either way (NFR-003).

### TEST-007: TripleSource / static adapter
- **Positive:** Turtle/N-Triples/TriG/RDF-XML ingest to identical facts.
- **Negative input:** malformed document.
- **Negative output:** structured `SourceError`, no silent partial theory.

### TEST-008: SPL round-trip
- **Positive:** emit SPL, re-parse, compare facts + source metadata.
- **Negative output:** round-trip idempotent.

### TEST-009: Bounded ingestion
- **Positive:** ingest within limits.
- **Negative input:** exceed max-bytes / max-triples.
- **Negative output:** diagnostic, no panic, bounded output.

### TEST-010: Non-inference
- **Positive:** `owl:sameAs` + RDFS axioms ingest as literal facts only.
- **Negative output:** no derived memberships or merged individuals appear pre-reasoning.

### TEST-011: Determinism
- **Positive:** re-ingest an equal source; byte-identical theory/SPL/diagnostics.
- **Negative input:** shuffled triple order (where the format permits).
- **Negative output:** identical skolem naming and output ordering.

## 12. Traceability Matrix

| Requirement | Contract / decision | Verification |
|---|---|---|
| [[SPEC-025-linked-data-ingestion#REQ-001]] | [[SPEC-025-linked-data-ingestion#CON-001]], [[SPEC-025-linked-data-ingestion#ADR-001]] | [[SPEC-025-linked-data-ingestion#TEST-001]] |
| [[SPEC-025-linked-data-ingestion#REQ-002]] | [[SPEC-025-linked-data-ingestion#CON-002]], [[SPEC-025-linked-data-ingestion#ADR-002]] | [[SPEC-025-linked-data-ingestion#TEST-002]] |
| [[SPEC-025-linked-data-ingestion#REQ-003]] | [[SPEC-025-linked-data-ingestion#CON-003]] | [[SPEC-025-linked-data-ingestion#TEST-003]] |
| [[SPEC-025-linked-data-ingestion#REQ-004]] | [[SPEC-025-linked-data-ingestion#CON-001]] | [[SPEC-025-linked-data-ingestion#TEST-004]] |
| [[SPEC-025-linked-data-ingestion#REQ-005]] | [[SPEC-025-linked-data-ingestion#CON-001]] | [[SPEC-025-linked-data-ingestion#TEST-005]] |
| [[SPEC-025-linked-data-ingestion#REQ-006]] | [[SPEC-025-linked-data-ingestion#CON-004]], [[SPEC-025-linked-data-ingestion#ADR-004]] | [[SPEC-025-linked-data-ingestion#TEST-006]] |
| [[SPEC-025-linked-data-ingestion#REQ-007]] | [[SPEC-025-linked-data-ingestion#CON-005]], [[SPEC-025-linked-data-ingestion#ADR-005]], [[SPEC-025-linked-data-ingestion#ADR-007]] | [[SPEC-025-linked-data-ingestion#TEST-007]] |
| [[SPEC-025-linked-data-ingestion#REQ-008]] | [[SPEC-025-linked-data-ingestion#CON-004]] | [[SPEC-025-linked-data-ingestion#TEST-008]] |
| [[SPEC-025-linked-data-ingestion#REQ-009]] | [[SPEC-025-linked-data-ingestion#CON-006]] | [[SPEC-025-linked-data-ingestion#TEST-009]] |
| [[SPEC-025-linked-data-ingestion#REQ-010]] | [[SPEC-025-linked-data-ingestion#ADR-003]], [[SPEC-025-linked-data-ingestion#ADR-006]] | [[SPEC-025-linked-data-ingestion#TEST-010]] |
| [[SPEC-025-linked-data-ingestion#REQ-011]] | [[SPEC-025-linked-data-ingestion#NFR-001]] | [[SPEC-025-linked-data-ingestion#TEST-011]] |
| [[SPEC-025-linked-data-ingestion#NFR-003]] | [[SPEC-024-predicate-model-and-vocabulary#REQ-013]] | [[SPEC-025-linked-data-ingestion#TEST-006]] |

## 13. Deferred Decisions

| Decision | Rationale for deferral | Trigger |
|---|---|---|
| T-Box → rules (RDFS/OWL entailment) | Open-world/monotonic vs closed-world/defeasible needs a formal mapping policy | Successor spec; interoperability use case |
| SHACL validation onto `Shape` | Needs a SHACL-core → `PredicateSignature`/`Shape` mapping | Validation use case; builds on SPEC-024 `Shape` |
| JSON-LD ingestion | Context processing and possibly remote context fetching | After Phase 1 stabilizes |
| `owl:sameAs` merging | Identity coalescence interacts with negation-as-failure | Successor spec |
| Non-numeric datatype fidelity (bool/date/lang) | `Term` has four variants (G-1); dates may route through the temporal system | `Term` extension or temporal mapping spec |
| RDF export (Spindle → RDF/OWL) | The reverse bridge; different concerns | Interop use case |

## 14. Review and Comprehension Gates

Tier 2 (new public API and file/network I/O). This document SHALL remain `draft`
until: a different model family reviews the mapping soundness and the
open/closed-world boundary; a human maintainer approves the A-Box scope, the
CURIE and datatype policies, and the provenance/trust design; and `zetl`
validation reports no unexplained dead technical-concept links. It depends on the
[[SPEC-024-predicate-model-and-vocabulary]] branch being merged.

Current gate status: **pending** (early draft for refinement).

## Changelog

<details>
<summary>Revision history</summary>

- 0.1.0 (2026-07-14): Initial draft. A-Box linked-data ingestion built on the
  SPEC-024 predicate model: class-aware mapping, CURIE compaction, typed-literal
  conversion, skolemization, claims-based provenance/trust, a phased
  `TripleSource` (static → SPARQL → dereference), and `Theory`/SPL output.
  Includes an explicit SPEC-024 affordance analysis (§4).

</details>
