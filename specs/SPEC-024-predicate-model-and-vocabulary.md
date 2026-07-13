---
id: SPEC-024
title: Predicate Model, Theory Signatures, and Vocabulary
status: draft
version: 0.1.0
created: 2026-07-13
last-updated: 2026-07-13
authors: Codex (GPT-5, AI agent)
reviewers: Core Maintainers (pending)
protocol: USDD Agent Protocol v1.11.0
---

# SPEC-024: Predicate Model, Theory Signatures, and Vocabulary

## Orientation

**Intent:** Give Spindle a structural vocabulary model so tools can identify
`assign-to/2`, describe its ordered arguments, inspect observed argument kinds,
and validate applications without parsing conventions out of literal names.

**Metaphor:** A [[SPEC-024-predicate-model-and-vocabulary#Theory|theory]] is a program; its [[SPEC-024-predicate-model-and-vocabulary#Theory Signature|theory
signature]] is the exported symbol table, and its [[SPEC-024-predicate-model-and-vocabulary#Vocabulary|vocabulary]] is
the annotated catalogue built from that table. Neither is the execution engine.

**Key decisions:**

- [[SPEC-024-predicate-model-and-vocabulary#Predicate Symbol|`PredicateSymbol`]] is functor plus arity only.
- [[SPEC-024-predicate-model-and-vocabulary#Predicate Indicator|`assign-to/2`]] is human-facing notation, not a second
  semantic identity or a wire key.
- [[SPEC-024-predicate-model-and-vocabulary#Predicate Signature|`PredicateSignature`]] adds ordered argument names and
  primitive sorts; it does not change defeasible inference.
- [[SPEC-024-predicate-model-and-vocabulary#Literal Pattern|`LiteralPattern`]] and [[SPEC-024-predicate-model-and-vocabulary#Ground Literal|`GroundLiteral`]]
  are validated phase types around the compatibility [[SPEC-024-predicate-model-and-vocabulary#Literal|`Literal`]]
  model.
- [[SPEC-024-predicate-model-and-vocabulary#Argument Profile|`ArgumentProfile`]] reports observations; [[SPEC-024-predicate-model-and-vocabulary#Shape|`Shape`]]
  expresses constraints. Observation and prescription remain separate.
- [[SPEC-024-predicate-model-and-vocabulary#Theory|`Theory`]] remains the canonical Spindle term. "Ontology" is an
  interoperability description, not a duplicate Rust type.

**Load-bearing requirements:** [[SPEC-024-predicate-model-and-vocabulary#REQ-001]], [[SPEC-024-predicate-model-and-vocabulary#REQ-003]], [[SPEC-024-predicate-model-and-vocabulary#REQ-005]],
[[SPEC-024-predicate-model-and-vocabulary#REQ-009]], [[SPEC-024-predicate-model-and-vocabulary#REQ-012]], and [[SPEC-024-predicate-model-and-vocabulary#REQ-013]].

**Structure:**

```text
                    +-------------------+
                    | PredicateSymbol   |
                    | functor + arity   |
                    +---------+---------+
                              ^
              +---------------+---------------+
              |                               |
    +---------+----------+          +---------+----------+
    | Literal / Pattern  |          | PredicateSignature |
    | observed uses      |          | declared intent    |
    +---------+----------+          +---------+----------+
              |                               |
              +---------------+---------------+
                              |
                    +---------v---------+
                    | TheorySignature   |
                    +---------+---------+
                              |
                    +---------v---------+
                    | Vocabulary        |
                    | docs + provenance |
                    +-------------------+

        Shape validates at the boundary; reasoning identities are unchanged.
```

**Reading path:** Start with [[SPEC-024-predicate-model-and-vocabulary#Terminology and Identity Boundaries]], then
[[SPEC-024-predicate-model-and-vocabulary#Functional Requirements]], [[SPEC-024-predicate-model-and-vocabulary#Contracts]], and [[SPEC-024-predicate-model-and-vocabulary#Architecture Decisions]].
Implementation and review work is indexed by [[SPEC-024-predicate-model-and-vocabulary#Traceability Matrix]].

### Conformance

The key words MUST, MUST NOT, REQUIRED, SHALL, SHALL NOT, SHOULD, SHOULD NOT,
RECOMMENDED, MAY, and OPTIONAL in this document are to be interpreted as
described in [[usdd-agent-protocol#Requirement-Level Keywords (BCP 14)|BCP 14
(RFC 2119, RFC 8174)]] when, and only when, they appear in all capitals.

## 1. Context and Problem

Spindle already stores typed argument values in `Term`, but it has no public,
structural model for the predicate itself. Consumers that need a stable key,
an argument schema, or generated documentation are consequently tempted to:

1. encode type or role information in the functor name;
2. recover that information with suffix regular expressions;
3. use a literal-level ID whose identity is either too broad or too narrow; or
4. infer one schema from whichever literal occurrence happens to be visited first.

These approaches are fragile. A naming convention is not a type system, a
regular expression cannot establish semantic identity, and Spindle's existing
literal IDs deliberately answer different questions.

The immediate stakeholder intent is to make predicate structure explicit for
library consumers and downstream message vocabularies. The feature SHALL expose
structural discovery and validation while preserving the current reasoning
semantics.

### 1.1 User goals

| User | Goal | Success criterion |
|---|---|---|
| Theory author | See which predicates a theory uses | Every head and logical body occurrence contributes the correct functor and arity |
| Tooling author | Key documentation and mappings without name regexes | A typed `PredicateSymbol` is available from public APIs and structured serialization |
| Integrator | Describe argument roles and primitive sorts | A checked `PredicateSignature` can be attached to a symbol |
| Validator author | Detect malformed applications before an effectful boundary | A `Shape` returns deterministic, positional diagnostics |
| Reasoner maintainer | Preserve existing proof behavior | Adding declarations, profiles, or vocabulary annotations does not change conclusions |

### 1.2 Current identities that remain authoritative

This specification adds a predicate-level identity; it does not replace:

- `AtomKey` and `LitId`, which identify indexed atoms including arguments,
  modality, and temporal bounds under
  [[SPEC-018-temporal-atom-identity|temporal atom identity]];
- `ExactLitId`, which identifies exact projection-local literals; or
- `FamilyId`, which groups temporal variants while retaining functor,
  arguments, modality, and polarity under
  [[SPEC-020-temporal-family-reasoning|temporal family reasoning]].

The distinctions are normative in [[SPEC-024-predicate-model-and-vocabulary#CON-001]].

## 2. Scope

### 2.1 In scope

- A public `PredicateSymbol` value composed of functor and arity.
- A reversible Prolog-style predicate-indicator notation for humans.
- Checked primitive argument declarations in `PredicateSignature`.
- Deterministic `TheorySignature` extraction from rule heads and logical bodies.
- `LiteralPattern` and `GroundLiteral` phase types compatible with the current AST.
- Observed primitive argument kinds in `ArgumentProfile`.
- Non-semantic `Shape` validation.
- A derived `Vocabulary` containing descriptions and occurrence provenance.
- Additive Rust and serialization contracts.

### 2.2 Out of scope

- Nominal domain types such as `Task`, `Agent`, or `Message`.
- Subtyping, type inference, coercion, or dependent argument constraints.
- Changing unification, numeric equality, defeat, temporal projection, or modal semantics.
- Encoding a signature in a functor name.
- A new SPL declaration form in this revision.
- A normative RDF, RDFS, OWL, or SHACL mapping.
- Per-theory predicates in a CBCL dialect definition.

## 3. Terminology and Identity Boundaries

### 3.1 Predicate Symbol

A **PredicateSymbol** is the ordered pair `(functor, arity)`. For example,
`assign-to/2` denotes functor `assign-to` with two argument positions.

It excludes argument values, polarity, modal operator, temporal bounds, rule
label, and provenance. It is suitable as a map key for declarations and
documentation. It is not suitable as a proof-state or literal-identity key.

### 3.2 Predicate Indicator

A **PredicateIndicator** is the Prolog-style textual notation `functor/arity`
for a [[SPEC-024-predicate-model-and-vocabulary#Predicate Symbol|predicate symbol]]. It is a notation, not a second
core type. The canonical machine representation is the structured object in
[[SPEC-024-predicate-model-and-vocabulary#CON-002]].

### 3.3 Predicate Signature

A **PredicateSignature** associates a [[SPEC-024-predicate-model-and-vocabulary#Predicate Symbol|predicate symbol]]
with one ordered declaration per argument position. Each declaration has a
stable argument name and a [[SPEC-024-predicate-model-and-vocabulary#Primitive Sort|primitive sort]]. Its declaration
count always equals the symbol arity.

### 3.4 Theory Signature

A **TheorySignature** is the deterministic set of predicate symbols available
to a [[SPEC-024-predicate-model-and-vocabulary#Theory|theory]], plus any checked predicate-signature declarations
associated with those symbols. Availability is the union of observed logical
occurrences and explicit declarations; neither source uses naming conventions.

### 3.5 Vocabulary

A **Vocabulary** is a derived, inspectable catalogue of theory terms. Each
entry is keyed by `PredicateSymbol` and may carry a `PredicateSignature`, an
`ArgumentProfile`, a description, and occurrence provenance. It is a tooling
projection, not a second source of reasoning truth.

### 3.6 Literal

A **Literal** is a positive or negated application of a predicate to arguments,
with Spindle's optional modal and temporal structure. In the public Rust API,
the existing `Literal` remains the compatibility AST and may still contain
unresolved symbols or temporal variables. Code that requires a phase invariant
uses one of the checked types below.

### 3.7 Literal Pattern

A **LiteralPattern** is an application that still requires grounding. It
contains at least one variable, arithmetic argument expression, or unresolved
temporal endpoint. It cannot be submitted where a ground application is
required without an explicit grounding transition.

### 3.8 Ground Literal

A **GroundLiteral** is a checked literal with no variable symbols, arithmetic
argument expressions, interval variable, or unresolved temporal endpoints.
The wrapper proves this invariant at construction time.

### 3.9 Primitive Sort

A **PrimitiveSort** is one of `symbol`, `integer`, `decimal`, `float`, `number`,
or `any`. The first four correspond to current `Term` variants. `number`
accepts all three numeric variants according to existing numeric compatibility;
`any` accepts any ground `Term`. Primitive sorts do not assign domain meaning.

### 3.10 Argument Profile

An **ArgumentProfile** is observational evidence, grouped by predicate symbol
and argument position, about kinds found in a theory. Observable kinds are
`variable`, `symbol`, `integer`, `decimal`, `float`, and
`arithmetic-expression`. A profile does not claim that future occurrences are
constrained to the observed kinds.

### 3.11 Shape

A **Shape** is a set of validation constraints compiled from a predicate
signature. It checks arity and known argument values and reports deferred
positions for unresolved patterns. Shape validation is separate from inference.

### 3.12 Theory

A **Theory** is Spindle's canonical aggregate for rules, facts, superiority
relations, metadata, and other axiomatic inputs. Predicate declarations are
supplied alongside the current `Theory` API in this revision and may move into
the aggregate only after first-class SPL syntax is specified. An ontology may
be represented by a theory, but **Ontology** SHALL remain an explanatory
semantic-web term in this revision and SHALL NOT be introduced as an alias or
duplicate Rust type.

### 3.13 Identity lattice

| Identity | Functor | Arity | Values | Polarity | Mode | Temporal | Purpose |
|---|---:|---:|---:|---:|---:|---:|---|
| `PredicateSymbol` | yes | yes | no | no | no | no | Declarations, discovery, vocabulary |
| `FamilyId` | yes | implicit | yes | yes | yes | no | Temporal family reasoning |
| `AtomKey` / `LitId` | yes | implicit | yes | yes in `LitId` | yes | yes | Main reasoning index |
| `ExactLitId` | yes | implicit | yes | yes | yes | yes | Projection-local exact identity |

No conversion from a richer identity to `PredicateSymbol` may imply that the
richer identities are equal.

## 4. Architecture

### 4.1 Layered model

The design has four layers:

1. **Syntax and phase:** classify applications as literal patterns or ground literals.
2. **Symbol:** project any logical application to functor plus arity.
3. **Schema and validation:** attach argument declarations and compile shapes.
4. **Catalogue:** derive a theory signature, observations, descriptions, and provenance.

The reasoner continues to consume the existing theory and literal structures.
The new layers inspect or validate them and do not participate in rule firing.

### 4.2 Purity Boundary Map

#### Pure core

- construct and compare `PredicateSymbol`;
- classify literal phases;
- derive occurrence and aggregate argument profiles;
- check `PredicateSignature` invariants;
- compile and apply `Shape`;
- merge and sort theory-signature and vocabulary entries; and
- render structured diagnostics.

#### Effectful shell

- parse untrusted predicate indicators;
- traverse parsed theories and collect rule-label provenance;
- load caller-supplied vocabulary annotations;
- serialize results; and
- expose results through CLI, FFI, or service adapters.

#### Dependency rule

The pure core SHALL NOT perform file, environment, network, clock, or logging
I/O. The effectful shell MAY call the pure core; the pure core SHALL NOT import
effectful adapters.

### 4.3 Composition-first placement

The design reuses `InternedLiteralName`, `Term`, `Literal`, `BodyLogicLiteral`,
`RuleLabel`, and `Theory`. It adds small checked projections in
`spindle-core`, parser recognition in `spindle-parser`, and additive DTOs in
`spindle-contract`. No new dependency or parallel ontology subsystem is
required.

This settles at rung 4 of the
[[usdd-agent-protocol#The Simplicity Ladder (operationalises the Simplicity Gate)|Simplicity Ladder]]:
existing components provide the values, while new domain types are justified
only where they encode invariants the existing AST does not express.

## 5. Functional Requirements

### REQ-001: Structural predicate identity

`PredicateSymbol` SHALL compare and hash by exactly its interned functor and
unsigned arity. Polarity, argument values, mode, temporal bounds, labels, and
metadata SHALL NOT affect equality.

**Acceptance criteria:** `p(a)` and `~p(b)` both project to `p/1`; `p` projects
to `p/0`; `p(a,b)` projects to `p/2`; and all three arities remain distinct.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-001]], [[SPEC-024-predicate-model-and-vocabulary#ADR-001]], [[SPEC-024-predicate-model-and-vocabulary#TEST-001]].

### REQ-002: Lossless symbol extraction

Every head `Literal` and logical-body `BodyLogicLiteral` SHALL expose its
`PredicateSymbol` directly from its functor and stored argument count.

**Acceptance criteria:** A body application containing arithmetic arguments
retains their positions in its arity. Extraction SHALL NOT pass through
`BodyLogicLiteral::to_literal()`, because that conversion currently omits
arithmetic arguments.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-001]], [[SPEC-024-predicate-model-and-vocabulary#TEST-002]].

### REQ-003: Predicate-indicator recognition

The predicate-indicator parser SHALL fully recognize the grammar in
[[SPEC-024-predicate-model-and-vocabulary#CON-002]] and SHALL return a `PredicateSymbol` only when all input is
consumed.

**Acceptance criteria:** `assign-to/2` round-trips; malformed arities, trailing
input, and ambiguous slash-bearing bare functors are rejected; quoted functors
round-trip without loss.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-002]], [[SPEC-024-predicate-model-and-vocabulary#ADR-002]], [[SPEC-024-predicate-model-and-vocabulary#TEST-003]].

### REQ-004: Total literal-phase classification

The classifier SHALL place each existing `Literal` into exactly one of
`GroundLiteral` or `LiteralPattern`.

**Acceptance criteria:** No input is classified as both or neither. A symbol
whose resolved spelling begins with `?`, an interval variable, or an unresolved
temporal endpoint makes the application a pattern.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-003]], [[SPEC-024-predicate-model-and-vocabulary#ADR-003]], [[SPEC-024-predicate-model-and-vocabulary#TEST-004]].

### REQ-005: Ground-literal invariant

Safe construction of `GroundLiteral` SHALL reject every application that
contains unresolved variables or temporal endpoints.

**Acceptance criteria:** Once constructed through public safe APIs,
`GroundLiteral::as_literal()` cannot expose a variable symbol, interval
variable, or unresolved temporal expression.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-003]], [[SPEC-024-predicate-model-and-vocabulary#TEST-005]].

### REQ-006: Typed grounding transition

The grounding boundary SHALL return `Result<GroundLiteral, GroundingError>`
for a `LiteralPattern` rather than returning an unchecked `Literal`.

**Acceptance criteria:** Complete bindings produce a ground wrapper; missing or
invalid bindings produce positional errors; no success value requires a second
groundness scan by the caller.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-003]], [[SPEC-024-predicate-model-and-vocabulary#ADR-003]], [[SPEC-024-predicate-model-and-vocabulary#TEST-006]].

### REQ-007: Positional argument observation

`ArgumentProfile` derivation SHALL aggregate observed argument kinds separately
for every argument position of every predicate symbol.

**Acceptance criteria:** Observing `cost(item, 2)` and `cost(other, 2.5)` yields
`{symbol}` for position 0 and `{integer, decimal}` for position 1. Numeric kinds
remain distinct observations even though numeric matching is compatible.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-004]], [[SPEC-024-predicate-model-and-vocabulary#ADR-005]], [[SPEC-024-predicate-model-and-vocabulary#TEST-007]].

### REQ-008: Checked predicate signatures

Safe construction of `PredicateSignature` SHALL require an argument-declaration
count equal to the predicate arity and unique, non-empty argument names.

**Acceptance criteria:** Invalid declarations return structured errors naming
the predicate and offending position or duplicate name.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-004]], [[SPEC-024-predicate-model-and-vocabulary#TEST-008]].

### REQ-009: Complete theory-signature derivation

`TheorySignature::derive` SHALL include each distinct predicate symbol appearing
in any rule head or logical rule body, including zero-arity, negated, modal,
temporal, variable-bearing, and arithmetic-bearing applications.

**Acceptance criteria:** Polarity, mode, and temporal variants collapse only
when their functor and arity agree. Arithmetic constraints that are not logical
predicate applications contribute no predicate symbol. A valid declaration is
also present in the symbol set even when no rule currently uses it, and is
reported as unobserved.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-005]], [[SPEC-024-predicate-model-and-vocabulary#TEST-009]].

### REQ-010: Explicit declaration conflicts

Combining declarations SHALL report conflicting signatures for the same
predicate symbol and SHALL NOT select one according to traversal or insertion
order.

**Acceptance criteria:** Identical declarations deduplicate; differing argument
names or sorts produce a deterministic conflict containing every distinct
declaration.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-005]], [[SPEC-024-predicate-model-and-vocabulary#TEST-010]].

### REQ-011: Provenanced vocabulary derivation

`Vocabulary::derive` SHALL produce one entry per predicate symbol and SHALL
associate each observed use with its rule label, head-or-body role, and
zero-based occurrence position.

**Acceptance criteria:** Descriptions are joined through a structured map keyed
by `PredicateSymbol`; no description or provenance relationship is recovered
from a literal-name prefix, suffix, or regular expression.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-005]], [[SPEC-024-predicate-model-and-vocabulary#ADR-006]], [[SPEC-024-predicate-model-and-vocabulary#TEST-011]].

### REQ-012: Deterministic shape validation

A `Shape` SHALL validate arity and every statically known argument against its
declared primitive sort and SHALL return all violations in ascending argument
position.

**Acceptance criteria:** `number` accepts integer, decimal, and float terms;
exact numeric sorts accept only their matching term variant; `any` accepts all
ground terms; unresolved pattern positions are reported as deferred rather than
as type matches.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-006]], [[SPEC-024-predicate-model-and-vocabulary#ADR-004]], [[SPEC-024-predicate-model-and-vocabulary#TEST-012]].

### REQ-013: Validation is non-semantic

Declarations, profiles, vocabulary annotations, and shape diagnostics SHALL NOT
change the conclusion set or proof traces produced by the reasoner.

**Acceptance criteria:** Reasoning the same rules with no schema metadata, with
valid metadata, and with conflicting metadata produces identical logical
results. Callers MAY reject a theory at their own boundary after validation;
the core reasoner does not do so implicitly.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#ADR-004]], [[SPEC-024-predicate-model-and-vocabulary#TEST-013]], [[SPEC-024-predicate-model-and-vocabulary#NFR-003]].

### REQ-014: Structured serialization

Public serialization of predicate symbols, signatures, profiles, shapes, and
vocabulary entries SHALL use structured fields and SHALL NOT use a
predicate-indicator string as the sole machine key.

**Acceptance criteria:** The JSON contracts in [[SPEC-024-predicate-model-and-vocabulary#CON-007]] round-trip functors
containing `/`, preserve ordered positions, and sort map-like output by
`(functor, arity)`.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-007]], [[SPEC-024-predicate-model-and-vocabulary#ADR-002]], [[SPEC-024-predicate-model-and-vocabulary#TEST-014]].

### REQ-015: Additive compatibility

The first implementation SHALL preserve existing public `Literal`,
`BodyLogicLiteral`, `Term`, `Theory`, `FamilyId`, `LitId`, and `ExactLitId`
behavior while adding the new APIs.

**Acceptance criteria:** Existing source and test suites compile without
requiring consumers to adopt signatures, profiles, shapes, or phase wrappers.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#ADR-003]], [[SPEC-024-predicate-model-and-vocabulary#TEST-015]].

## 6. Non-Functional Requirements

### NFR-001: Determinism

For structurally equal theories and annotations, serialized theory signatures,
profiles, diagnostics, and vocabulary entries SHALL be byte-identical across
runs on the same Spindle version, independent of `HashMap` iteration order.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-005]], [[SPEC-024-predicate-model-and-vocabulary#CON-007]], [[SPEC-024-predicate-model-and-vocabulary#TEST-016]].

### NFR-002: Derivation complexity

Theory-signature and vocabulary derivation SHALL visit each logical literal
occurrence once and SHALL have `O(L log P)` worst-case time, where `L` is the
number of logical occurrences and `P` is the number of distinct predicate
symbols. Additional storage SHALL be `O(P + O)`, where `O` is retained
provenance records.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-005]], [[SPEC-024-predicate-model-and-vocabulary#TEST-017]].

### NFR-003: Reasoning parity

On the existing non-schema test corpus, enabling construction of the new
projections SHALL produce zero differences in conclusions and proof metadata.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#REQ-013]], [[SPEC-024-predicate-model-and-vocabulary#TEST-013]].

### NFR-004: Bounded untrusted parsing

Predicate-indicator recognition SHALL use the caller's configured input-size
limit, use checked arity conversion, perform no catastrophic-backtracking
regular expression, and either consume the complete input or return an error.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#CON-002]], [[SPEC-024-predicate-model-and-vocabulary#TEST-018]].

## 7. Contracts

### CON-001: PredicateSymbol and projections

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PredicateSymbol {
    functor: InternedLiteralName,
    arity: u32,
}

impl PredicateSymbol {
    pub fn try_new(
        functor: InternedLiteralName,
        arity: usize,
    ) -> Result<Self, PredicateSymbolError>;

    pub fn functor(self) -> InternedLiteralName;
    pub fn arity(self) -> u32;
    pub fn indicator(self) -> PredicateIndicatorDisplay;
}

pub trait HasPredicateSymbol {
    fn predicate_symbol(&self) -> Result<PredicateSymbol, PredicateSymbolError>;
}

impl HasPredicateSymbol for Literal { /* direct args.len() */ }
impl HasPredicateSymbol for BodyLogicLiteral { /* direct args.len() */ }
```

`PredicateSymbolError::ArityOverflow { actual: usize }` is returned when an
application cannot be represented by `u32`. Implementations MUST count the
stored `BodyArg` sequence directly. They MUST NOT derive body arity through a
lossy conversion to `Literal`.

`PredicateSymbol` SHALL implement `Ord` manually by comparing the resolved
functor's Unicode scalar sequence and then numeric arity. It SHALL NOT derive
ordering from `InternedLiteralName`, because its `SymbolId` order reflects
process-local interning history rather than lexical order.

### CON-002: PredicateIndicator grammar and wire form

`PredicateIndicator` is parsed with an explicit recognizer in `spindle-parser`.
It is not parsed with a suffix regular expression.

```ebnf
predicate-indicator = indicator-functor, "/", arity ;
indicator-functor   = bare-functor | quoted-functor ;
bare-functor        = bare-char, { bare-char } ;
bare-char           = letter | digit | "-" | "_" | "?" | "~" | ":" |
                      "." | "+" | "*" | "<" | ">" | "=" | "!" ;
quoted-functor      = '"', { quoted-char | escape }, '"' ;
quoted-char         = ? any Unicode scalar except '"', "\\", or control ? ;
escape              = "\\", ( '"' | "\\" | "/" ) ;
arity               = "0" | non-zero-digit, { digit } ;
non-zero-digit      = "1" | "2" | "3" | "4" | "5" |
                      "6" | "7" | "8" | "9" ;
digit               = "0" | non-zero-digit ;
letter              = ? Unicode alphabetic scalar accepted by the SPL lexer ? ;
```

Bare functors exclude `/`; a slash-bearing functor is quoted, so
`"rate/limit"/2` is unambiguous. Leading zeroes, signs, whitespace, trailing
characters, invalid escapes, and arities above `u32::MAX` are rejected. The
recognizer SHALL reuse the lexer/escape machinery already present where
possible and SHALL fully consume input in accordance with
[[usdd-agent-protocol#Language-Theoretic Security (LangSec)|LangSec]].

The canonical structured form is:

```json
{ "functor": "assign-to", "arity": 2 }
```

### CON-003: Literal phase API

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClassifiedLiteral {
    Pattern(LiteralPattern),
    Ground(GroundLiteral),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiteralPattern(Literal);

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct GroundLiteral(Literal);

impl Literal {
    pub fn classify(self) -> ClassifiedLiteral;
}

impl TryFrom<Literal> for GroundLiteral {
    type Error = GroundnessError;
}

impl LiteralPattern {
    pub fn ground(
        self,
        bindings: &Bindings,
    ) -> Result<GroundLiteral, GroundingError>;
}
```

`LiteralPattern` initially wraps the current head-compatible `Literal` AST.
`BodyLogicLiteral` remains the body pattern representation because arithmetic
arguments require `BodyArg`. A later implementation plan MAY generalize the
wrapper only if doing so removes duplication without weakening invariants.

Groundness is determined by structure, except for the existing variable
encoding: a `Term::Symbol` whose resolved value starts with `?` is a variable.
This compatibility rule SHALL live in one shared recognizer, not in repeated
call-site string tests.

### CON-004: Signatures, sorts, and profiles

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PrimitiveSort {
    Symbol,
    Integer,
    Decimal,
    Float,
    Number,
    Any,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentDecl {
    pub name: String,
    pub sort: PrimitiveSort,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateSignature {
    pub symbol: PredicateSymbol,
    pub arguments: Box<[ArgumentDecl]>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ObservedArgumentKind {
    Variable,
    Symbol,
    Integer,
    Decimal,
    Float,
    ArithmeticExpression,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentProfile {
    pub symbol: PredicateSymbol,
    pub positions: Box<[BTreeSet<ObservedArgumentKind>]>,
}
```

`PredicateSignature::try_new` enforces [[SPEC-024-predicate-model-and-vocabulary#REQ-008]]. Profile position count is
constructed from the symbol arity and cannot be caller-supplied independently.

### CON-005: TheorySignature and Vocabulary

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TheorySignature {
    pub symbols: BTreeSet<PredicateSymbol>,
    pub declarations: BTreeMap<PredicateSymbol, DeclarationState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeclarationState {
    Declared(PredicateSignature),
    Conflict(Box<[PredicateSignature]>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OccurrenceRole {
    Head { index: u32 },
    Body { index: u32 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateOrigin {
    pub rule: RuleLabel,
    pub role: OccurrenceRole,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct VocabularyAnnotations {
    pub descriptions: BTreeMap<PredicateSymbol, String>,
    pub declarations: Vec<PredicateSignature>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VocabularyEntry {
    pub symbol: PredicateSymbol,
    pub declaration: Option<DeclarationState>,
    pub profile: ArgumentProfile,
    pub description: Option<String>,
    pub origins: Box<[PredicateOrigin]>,
}

impl TheorySignature {
    pub fn derive(
        theory: &Theory,
        declarations: impl IntoIterator<Item = PredicateSignature>,
    ) -> Self;
}

impl Vocabulary {
    pub fn derive(
        theory: &Theory,
        annotations: &VocabularyAnnotations,
    ) -> VocabularyReport;
}
```

The symbol set is the union of observed applications and supplied declarations.
Rules are traversed in stable `RuleLabel` order. Head and body positions use
source order. Origins are sorted by rule label, role (`Head` before `Body`),
and index. An annotation for a symbol with no observed occurrence is retained
as a declared vocabulary entry and emits
`VocabularyDiagnostic::UnobservedDeclaration`; it is not silently discarded.

### CON-006: Shape validation

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Shape {
    pub predicate: PredicateSymbol,
    pub arguments: Box<[PrimitiveSort]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ShapeDiagnostic {
    PredicateMismatch { expected: PredicateSymbol, actual: PredicateSymbol },
    SortMismatch { position: u32, expected: PrimitiveSort, actual: ObservedArgumentKind },
    Deferred { position: u32, actual: ObservedArgumentKind },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ShapeReport {
    pub diagnostics: Vec<ShapeDiagnostic>,
}
```

`Shape::from(&PredicateSignature)` is infallible because signature construction
has already checked arity. Validation of a `GroundLiteral` has no `Deferred`
result. Validation of a pattern reports variables and arithmetic expressions as
deferred unless a future specification introduces sound static expression sorts.

### CON-007: Serialization schema

New DTOs SHALL be additive to the typed V2 contract. V1 string-argument DTOs
remain unchanged.

```json
{
  "schema": "spindle.vocabulary/1",
  "entries": [
    {
      "symbol": { "functor": "assign-to", "arity": 2 },
      "signature": {
        "arguments": [
          { "name": "task", "sort": "symbol" },
          { "name": "agent", "sort": "symbol" }
        ]
      },
      "profile": [
        ["symbol", "variable"],
        ["symbol", "variable"]
      ],
      "description": "Assign a task to an agent.",
      "origins": [
        { "rule": "r1", "role": "head", "index": 0 }
      ]
    }
  ],
  "diagnostics": []
}
```

Arrays preserve argument order. Entry order is `(functor code-point order,
arity numeric order)`. Enum strings are lowercase kebab case. Unknown enum
strings and inconsistent arities are rejected rather than preserved as partially
valid objects.

## 8. Architecture Decisions

### ADR-001: PredicateSymbol, not PredicateKey

**Status:** Proposed.

**Context:** `PredicateKey` describes a storage role, while the same value is
also used in signatures, diagnostics, documentation, and serialization.
Semantic-web terminology offers property and class vocabulary, but Spindle
supports arbitrary predicate arity and is fundamentally a rule language.

**Decision:** Name the domain value `PredicateSymbol`. Maps and sets MAY use it
as a key. `PredicateIndicator` names its textual notation.

**Consequences:** The API follows standard logic-programming terminology and
does not imply one storage implementation. Integrations may map unary symbols
to class-like concepts and binary symbols to property-like concepts, but those
mappings are not built into identity.

### ADR-002: Indicator strings are presentation, not identity

**Status:** Proposed.

**Context:** Functor names can contain `/`, and string keys invite suffix
parsing. A raw `name/arity` field is therefore ambiguous without another
grammar and fragile as a machine contract.

**Decision:** Store and serialize functor and arity separately. Provide the
indicator grammar only for display, CLI input, and interoperability with
Prolog-style tooling.

**Consequences:** Consumers never need a naming regex. The parser has a small,
fully recognized grammar, and slash-bearing names remain representable through
quoting.

### ADR-003: Add phase wrappers before replacing the AST

**Status:** Proposed.

**Context:** Existing APIs and internals use `Literal`; body literals additionally
support arithmetic arguments. Replacing these types wholesale would create a
large migration unrelated to predicate signatures.

**Decision:** Add checked `LiteralPattern` and `GroundLiteral` wrappers and
typed transitions while retaining the current AST during migration.

**Consequences:** New code can make illegal phase transitions unrepresentable,
while existing consumers remain source compatible. The variable marker remains
a lexical compatibility concern centralized in one recognizer.

### ADR-004: Shapes validate but do not infer

**Status:** Proposed.

**Context:** Semantic-web systems commonly separate ontology inference from
shape validation. Making declaration mismatch alter rule firing would silently
introduce a new typed proof theory.

**Decision:** Compile signatures into explicit `Shape` validators. Do not
consult shapes in the reasoner.

**Consequences:** Boundaries can choose strict admission, warning-only behavior,
or no validation. Logical results remain stable. Any future typed inference
requires its own specification and formal semantics.

### ADR-005: Primitive sorts only in version 1

**Status:** Proposed.

**Context:** Current `Term` values distinguish symbols and three numeric
representations. Domain sorts such as `Task` and `Agent` require nominal typing,
subsumption, and a source of declarations not currently present in SPL.

**Decision:** Model the current primitive value space plus `number` and `any`.
Keep observation kinds more precise than compatibility sorts.

**Consequences:** The first implementation is useful for content typing and
tooling without pretending that symbol values carry ontology classes.

### ADR-006: Vocabulary is derived, not authoritative

**Status:** Proposed.

**Context:** A second mutable catalogue can drift from the actual theory.
Description and provenance are useful, but predicate availability must reflect
the rules being reasoned over.

**Decision:** Derive vocabulary entries from theory traversal and join optional
annotations through `PredicateSymbol`.

**Consequences:** Documentation remains synchronized with observed use.
Unobserved declarations are explicit diagnostics rather than hidden drift.

### ADR-007: Theory is canonical; Ontology is contextual

**Status:** Proposed.

**Context:** In semantic-web terminology, an ontology may include classes,
properties, axioms, and constraints. Spindle's aggregate also contains
defeasible rules, superiority, modes, and trust metadata that do not map
directly to one standard ontology model.

**Decision:** Retain `Theory` as the canonical domain and Rust name. Use
"ontology" only when describing an external interpretation or mapping.

**Consequences:** The API remains precise about Spindle semantics while leaving
room for an explicit RDF/OWL/SHACL adapter later.

## 9. Validation and Error Model

Errors are divided by boundary:

| Boundary | Error type | Examples |
|---|---|---|
| Construction | `PredicateSymbolError` | Arity does not fit `u32` |
| Indicator parsing | `PredicateIndicatorError` | Ambiguous slash, invalid escape, trailing input, overflow |
| Signature construction | `PredicateSignatureError` | Arity mismatch, empty name, duplicate name |
| Phase construction | `GroundnessError` | Variable argument, interval variable, unresolved temporal endpoint |
| Grounding | `GroundingError` | Missing binding, incompatible binding, unevaluated expression |
| Validation | `ShapeDiagnostic` | Predicate or primitive-sort mismatch, deferred position |
| Vocabulary derivation | `VocabularyDiagnostic` | Conflicting or unobserved declaration |

Errors and diagnostics SHALL carry structural values and positions. Human
messages MAY include predicate indicators but SHALL NOT require consumers to
parse those messages.

## 10. Security and Robustness

1. Indicator parsing follows [[SPEC-024-predicate-model-and-vocabulary#CON-002]] and [[SPEC-024-predicate-model-and-vocabulary#NFR-004]]; a permissive regex
   or split-on-last-slash implementation is not conforming.
2. Structured JSON is the authority for machine interchange. Display strings
   are never reparsed implicitly.
3. Description and provenance text is untrusted data. Serializers SHALL escape
   it using the target format's standard encoder.
4. Vocabulary derivation does not resolve files or URLs found in metadata.
5. Arity and occurrence-index conversions are checked. No truncating cast is
   permitted.
6. Diagnostics avoid embedding an entire theory or binding map by default, to
   limit accidental disclosure and unbounded error output.

## 11. Observability

### OBS-001: Derivation summary

The vocabulary report SHALL expose counts for distinct predicate symbols,
observed occurrences, declarations, declaration conflicts, and shape
diagnostics. Library code returns these values; adapters MAY emit metrics or
logs from them.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#REQ-009]], [[SPEC-024-predicate-model-and-vocabulary#REQ-010]], [[SPEC-024-predicate-model-and-vocabulary#REQ-011]], [[SPEC-024-predicate-model-and-vocabulary#TEST-019]].

### OBS-002: Structural diagnostics

Every conflict or validation diagnostic SHALL include the structured
`PredicateSymbol` and, when applicable, argument position and rule-label
origin. It SHALL NOT expose only a formatted predicate-indicator string.

**Traces:** [[SPEC-024-predicate-model-and-vocabulary#REQ-010]], [[SPEC-024-predicate-model-and-vocabulary#REQ-012]], [[SPEC-024-predicate-model-and-vocabulary#TEST-020]].

## 12. Test Specifications

Each test below contains the requirement-targeted positive, negative-input,
and negative-output checks applicable to its requirement. An omitted category
is marked not applicable rather than silently skipped.

### TEST-001: PredicateSymbol identity law

- **Positive:** Project positive and negated `p(a)` and assert equal `p/1` symbols.
- **Negative input:** Attempt construction with arity above `u32::MAX` and assert `ArityOverflow`.
- **Negative output:** Mutate polarity, values, mode, and temporal bounds and assert none appear in equality or serialization; mutate arity and assert inequality.

### TEST-002: Direct extraction preserves body arity

- **Positive:** Extract from head and body applications with equal functor and argument count.
- **Negative input:** Use a body argument list containing an arithmetic expression.
- **Negative output:** Assert the resulting arity includes that expression and does not equal the arity obtained from the current lossy `to_literal()` path.

### TEST-003: Indicator grammar

- **Positive:** Round-trip `assign-to/2`, `p/0`, and `"rate/limit"/2`.
- **Negative input:** Reject empty names, leading-zero arities, signs, overflow, invalid escapes, whitespace, bare slash-bearing names, and trailing input.
- **Negative output:** Assert canonical rendering quotes and escapes only when required and never changes functor content.

### TEST-004: Total phase classification

- **Positive:** Classify representative ground and variable-bearing literals.
- **Negative input:** Include interval variables and unresolved temporal endpoints.
- **Negative output:** Property-test that exactly one enum variant is returned for every generated `Literal`.

### TEST-005: GroundLiteral invariant

- **Positive:** Construct from literals containing each ground `Term` variant.
- **Negative input:** Attempt construction from each unresolved form.
- **Negative output:** Traverse every successful wrapper and assert no unresolved form exists.

### TEST-006: Typed grounding

- **Positive:** Ground all variables and receive `GroundLiteral`.
- **Negative input:** Omit one binding and supply one incompatible arithmetic binding.
- **Negative output:** Assert errors identify the missing or incompatible position and no unchecked success value is returned.

### TEST-007: ArgumentProfile aggregation

- **Positive:** Aggregate multiple kinds independently by position.
- **Negative input:** Include variables and body arithmetic expressions.
- **Negative output:** Assert numeric kinds are not collapsed to `number` and positions are not merged.

### TEST-008: PredicateSignature invariants

- **Positive:** Construct `assign-to/2` with `task:symbol`, `agent:symbol`.
- **Negative input:** Reject wrong declaration count, empty names, and duplicate names.
- **Negative output:** Assert no safe constructor can return an arity-inconsistent signature.

### TEST-009: Complete TheorySignature

- **Positive:** Derive symbols from heads and logical bodies across polarity, mode, temporal, and zero-arity cases.
- **Negative input:** Include arithmetic constraints and arithmetic predicate arguments.
- **Negative output:** Assert constraints add no symbol, argument expressions still count toward body arity, and no logical occurrence is omitted.

### TEST-010: Declaration conflict handling

- **Positive:** Deduplicate identical declarations.
- **Negative input:** Supply conflicting names and conflicting sorts in different insertion orders.
- **Negative output:** Assert all distinct conflicts are retained in stable order and no winner is selected.

### TEST-011: Vocabulary provenance

- **Positive:** Derive one entry with sorted head/body origins and a structured description annotation.
- **Negative input:** Supply an annotation for an unobserved symbol.
- **Negative output:** Assert the entry and diagnostic are retained and no name-based relationship is inferred.

### TEST-012: Shape validation

- **Positive:** Validate every exact sort plus `number` and `any` compatibility.
- **Negative input:** Supply wrong arity, wrong primitive kinds, variables, and expressions.
- **Negative output:** Assert all mismatches and deferrals are returned once, sorted by position, with no variable reported as a successful type match.

### TEST-013: Reasoning parity

- **Positive:** Reason over a representative theory without annotations.
- **Negative input:** Add valid, invalid, and conflicting schema annotations.
- **Negative output:** Differentially assert identical conclusions and proof metadata in every case.

### TEST-014: Structured serialization

- **Positive:** Round-trip the example in [[SPEC-024-predicate-model-and-vocabulary#CON-007]], including a slash-bearing functor.
- **Negative input:** Reject unknown enum values, inconsistent array arity, and invalid schema versions.
- **Negative output:** Assert no serialized map uses an indicator as its sole key and order is canonical.

### TEST-015: Source compatibility

- **Positive:** Compile existing public API examples and run the pre-feature suite.
- **Negative input:** Enable and disable optional `serde` support where applicable.
- **Negative output:** Assert no existing DTO or method changes shape or behavior.

### TEST-016: Deterministic output

- **Positive:** Derive output repeatedly from equal theories built in different insertion orders.
- **Negative input:** Randomize rule, declaration, and annotation insertion order.
- **Negative output:** Assert byte-identical serialization and diagnostic ordering.

### TEST-017: Complexity envelope

- **Positive:** Benchmark synthetic theories at increasing occurrence counts.
- **Negative input:** Use many occurrences of one predicate and many distinct predicates.
- **Negative output:** Assert traversal counts each logical occurrence once and observed scaling remains within the [[SPEC-024-predicate-model-and-vocabulary#NFR-002]] envelope.

### TEST-018: Parser robustness

- **Positive:** Parse valid indicators at configured size boundaries.
- **Negative input:** Fuzz arbitrary UTF-8, long digit runs, escape runs, and slash-heavy strings.
- **Negative output:** Assert bounded completion, full consumption, checked overflow, and no panic.

### TEST-019: Derivation summary

- **Positive:** Verify every summary count against a hand-enumerated theory.
- **Negative input:** Include conflicts and unobserved declarations.
- **Negative output:** Assert counts do not depend on output filtering or logging configuration.

### TEST-020: Structural diagnostics

- **Positive:** Verify symbols, positions, and origins in diagnostics.
- **Negative input:** Use functors requiring indicator quoting.
- **Negative output:** Assert consumers can identify the predicate without parsing diagnostic text.

## 13. Traceability Matrix

| Requirement | Contract / decision | Verification |
|---|---|---|
| [[SPEC-024-predicate-model-and-vocabulary#REQ-001]] | [[SPEC-024-predicate-model-and-vocabulary#CON-001]], [[SPEC-024-predicate-model-and-vocabulary#ADR-001]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-001]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-002]] | [[SPEC-024-predicate-model-and-vocabulary#CON-001]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-002]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-003]] | [[SPEC-024-predicate-model-and-vocabulary#CON-002]], [[SPEC-024-predicate-model-and-vocabulary#ADR-002]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-003]], [[SPEC-024-predicate-model-and-vocabulary#TEST-018]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-004]] | [[SPEC-024-predicate-model-and-vocabulary#CON-003]], [[SPEC-024-predicate-model-and-vocabulary#ADR-003]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-004]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-005]] | [[SPEC-024-predicate-model-and-vocabulary#CON-003]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-005]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-006]] | [[SPEC-024-predicate-model-and-vocabulary#CON-003]], [[SPEC-024-predicate-model-and-vocabulary#ADR-003]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-006]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-007]] | [[SPEC-024-predicate-model-and-vocabulary#CON-004]], [[SPEC-024-predicate-model-and-vocabulary#ADR-005]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-007]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-008]] | [[SPEC-024-predicate-model-and-vocabulary#CON-004]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-008]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-009]] | [[SPEC-024-predicate-model-and-vocabulary#CON-005]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-009]], [[SPEC-024-predicate-model-and-vocabulary#TEST-017]], [[SPEC-024-predicate-model-and-vocabulary#TEST-019]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-010]] | [[SPEC-024-predicate-model-and-vocabulary#CON-005]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-010]], [[SPEC-024-predicate-model-and-vocabulary#TEST-019]], [[SPEC-024-predicate-model-and-vocabulary#TEST-020]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-011]] | [[SPEC-024-predicate-model-and-vocabulary#CON-005]], [[SPEC-024-predicate-model-and-vocabulary#ADR-006]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-011]], [[SPEC-024-predicate-model-and-vocabulary#TEST-019]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-012]] | [[SPEC-024-predicate-model-and-vocabulary#CON-006]], [[SPEC-024-predicate-model-and-vocabulary#ADR-004]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-012]], [[SPEC-024-predicate-model-and-vocabulary#TEST-020]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-013]] | [[SPEC-024-predicate-model-and-vocabulary#ADR-004]], [[SPEC-024-predicate-model-and-vocabulary#NFR-003]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-013]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-014]] | [[SPEC-024-predicate-model-and-vocabulary#CON-007]], [[SPEC-024-predicate-model-and-vocabulary#ADR-002]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-014]], [[SPEC-024-predicate-model-and-vocabulary#TEST-016]] |
| [[SPEC-024-predicate-model-and-vocabulary#REQ-015]] | [[SPEC-024-predicate-model-and-vocabulary#ADR-003]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-015]] |
| [[SPEC-024-predicate-model-and-vocabulary#NFR-001]] | [[SPEC-024-predicate-model-and-vocabulary#CON-005]], [[SPEC-024-predicate-model-and-vocabulary#CON-007]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-016]] |
| [[SPEC-024-predicate-model-and-vocabulary#NFR-002]] | [[SPEC-024-predicate-model-and-vocabulary#CON-005]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-017]] |
| [[SPEC-024-predicate-model-and-vocabulary#NFR-003]] | [[SPEC-024-predicate-model-and-vocabulary#REQ-013]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-013]] |
| [[SPEC-024-predicate-model-and-vocabulary#NFR-004]] | [[SPEC-024-predicate-model-and-vocabulary#CON-002]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-018]] |
| [[SPEC-024-predicate-model-and-vocabulary#OBS-001]] | [[SPEC-024-predicate-model-and-vocabulary#CON-005]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-019]] |
| [[SPEC-024-predicate-model-and-vocabulary#OBS-002]] | [[SPEC-024-predicate-model-and-vocabulary#CON-006]] | [[SPEC-024-predicate-model-and-vocabulary#TEST-020]] |

## 14. Migration and Implementation Sequence

1. Add `PredicateSymbol`, its direct projection trait, and identity-law tests to
   `spindle-core`.
2. Add the indicator recognizer to `spindle-parser`; keep structured fields as
   the contract representation.
3. Centralize variable recognition and add `GroundLiteral` plus
   `LiteralPattern` compatibility wrappers.
4. Change grounding boundaries incrementally to return `GroundLiteral`, with
   temporary internal `as_literal()` adapters where required.
5. Add primitive sorts, signatures, profiles, theory-signature derivation, and
   vocabulary derivation as pure projections.
6. Add shape validation and structural reports without wiring them into the
   reasoner.
7. Add typed V2 DTOs and deterministic serialization.
8. Run differential reasoning, parser fuzzing, property tests, and the full
   existing workspace suite before considering deprecation of unchecked paths.

No existing type is deprecated by this specification. A later implementation
plan SHALL identify any API proposed for deprecation and provide a separate
compatibility window.

## 15. Deferred Decisions

| Decision | Rationale for deferral | Owner / trigger |
|---|---|---|
| First-class SPL predicate declarations | Syntax should follow the proven API model and needs separate parser/source-location design | Core maintainers after [[SPEC-024-predicate-model-and-vocabulary#CON-004]] implementation |
| Nominal and ontology sorts | Requires class identity, subsumption, and interaction with unification | Semantics maintainers; new specification required |
| RDF/OWL/SHACL mapping | Arity, modality, defeasibility, and closed-world validation need an explicit mapping policy | Interoperability maintainer; adapter use case |
| CBCL message content profile | CBCL dialects describe transport vocabulary; a message content type/profile can point to a Spindle vocabulary without embedding per-theory predicates in the dialect | CBCL and Spindle maintainers; joint integration spec |
| Static arithmetic-expression sorts | Requires sound result-sort analysis across operators and bindings | Arithmetic maintainer; extension of [[SPEC-017-arithmetic-module]] |
| Repository-local USDD link resolution | The governing protocol lives in the sibling `handbook` repository, so a `zetl` scan rooted only at `spindle-rust` reports its four links as external debt | Repository maintainers; configure a shared parent vault or mirror the protocol page |

## 16. Review and Comprehension Gates

This document introduces public API and serialization schema and is therefore a
Tier 2 review artefact under
[[usdd-agent-protocol#Review Tiers|USDD review tiers]]. Because it was synthesized
by one AI model, it SHALL remain `draft` until:

1. a different model family reviews the identity boundaries, contracts, and
   requirement-targeted tests;
2. a human maintainer approves the terminology and non-semantic boundary;
3. a fresh-context reader, given only [[SPEC-024-predicate-model-and-vocabulary#Orientation]], can restate the intent,
   predict that `p(a)` and `~p(b)` share `p/1` but not literal identity, and
   locate the artefact that decides whether shapes affect reasoning; and
4. `zetl` validation reports no unexplained dead technical-concept links.

Current gate status: **pending**.

## Changelog

<details>
<summary>Revision history</summary>

- 0.1.0 (2026-07-13): Initial draft defining predicate symbols, indicators,
  signatures, theory signatures, vocabulary, literal phase types, argument
  profiles, and non-semantic shapes.

</details>
