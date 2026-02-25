# Arithmetic for Spindle

| Field | Value |
|---|---|
| Document ID | SPEC-017 |
| Title | Arithmetic: Numeric Terms and Constraints in Polish Notation |
| Version | 1.0.0 |
| Status | Draft |
| Created | 2026-02-24 |
| Last Updated | 2026-02-24 |
| Authors | Claude (AI agent) |
| Reviewers | Core Maintainers |
| Protocol | [USDD Agent Protocol v1.0.0](../../handbook/engineering/usdd-agent-protocol.md) |

---

## 1. Executive Summary

Spindle currently treats all terms as opaque interned symbols. There is no numeric type, no arithmetic evaluation, and no way to express constraints such as "cost exceeds threshold" or "total equals sum of parts" within a theory. This limits applicability to purely qualitative reasoning.

This specification adds **built-in arithmetic** to SPL: numeric terms, arithmetic expressions in Polish (prefix) notation, and built-in arithmetic predicates (`bind`, comparisons). Arithmetic operators (`+`, `-`, `*`, `/`, `<`, `>`, `=`, etc.) and `bind` are **reserved keywords**. No opt-in directive is required. All arithmetic expressions use S-expression prefix notation consistent with SPL's existing syntax — no new notation forms are introduced.

### Scope

| In Scope | Out of Scope |
|---|---|
| Integer arithmetic (i64) | Symbolic/algebraic solving |
| Exact decimal arithmetic (128-bit, via `rust_decimal`) | Constraint logic programming (CLP(R)) |
| IEEE 754 floating-point arithmetic (f64) | String operations |
| Numeric comparison predicates | Arithmetic in rule heads (computed conclusions) |
| Arithmetic in rule bodies (constraints + bindings) | Arithmetic during tabling / abduction |
| Arithmetic during grounding | |

---

## 2. Motivation and Context

### 2.1 Current Limitations

Consider a payroll theory. In current Spindle, you cannot write:

```lisp
; IMPOSSIBLE today — no way to express this
(normally r1
  (and (salary ?emp ?amount) (> ?amount 100000))
  (high-earner ?emp))
```

Instead, users must encode numeric domains as propositional constants (`salary-high`, `salary-medium`, etc.), discarding quantitative information. This is both lossy and combinatorially expensive for large numeric domains.

### 2.2 Design Constraints

Spindle's semantics are grounded in defeasible logic (DL(d)). Several constraints follow:

1. **Arithmetic is not defeasible.** `3 + 4 = 7` cannot be defeated by a superiority relation. Arithmetic predicates are strict constraints that succeed or fail during grounding, not participants in the defeasible proof.

2. **Arithmetic does not produce rule labels.** Built-in predicates produce no `RuleLabel` and cannot appear in superiority declarations. They are anonymous.

3. **No inference from arithmetic alone.** Arithmetic predicates can appear only in rule *bodies*, not heads. This prevents circular arithmetic definitions.

4. **Polish notation is already SPL's syntax.** SPL uses S-expressions. `(+ ?x ?y)` is idiomatic and requires no new parsing infrastructure beyond recognising arithmetic operators as term-level forms.

---

## 3. Functional Requirements

### REQ-001: Numeric Literal Terms

The system SHALL accept numeric literals as predicate arguments in any position where a symbol term is currently accepted. Numeric literals include:

- **Integer literals**: decimal digit sequences optionally prefixed with `-` (e.g., `0`, `42`, `-7`, `1000000`). Parsed as `Term::Integer(i64)`.
- **Decimal literals**: digit sequences with a `.` separator (e.g., `3.14`, `-0.5`, `0.08`). Parsed as `Term::Decimal`. Decimal is the **default** for non-integer numeric literals — this ensures exact representation for financial and policy arithmetic.
- **Float literals**: scientific notation with an `e`/`E` exponent (e.g., `1.5e3`, `-2.0e-4`, `1e6`). Parsed as `Term::Float(f64)`. Float is the **explicit opt-in** for IEEE 754 approximate arithmetic.

```lisp
(given (cost item-a 100))        ; Integer(100)
(given (price widget 9.99))      ; Decimal(9.99) — exact
(given (tax-rate standard 0.08)) ; Decimal(0.08) — exact, not 0.07999...
(given (mass particle 1.5e-27))  ; Float(1.5e-27) — scientific notation → float
(given (balance account -250))   ; Integer(-250)
```

Trace:
- TEST-001
- CON-001

---

### REQ-002: Arithmetic Expression Terms

The system SHALL accept arithmetic expressions as terms in predicate argument positions. Arithmetic expressions are S-expressions using the prefix operators defined in CON-002. Variables within expressions follow the existing `?name` convention.

```lisp
; (+ ?x ?y) is an arithmetic expression term
(normally r1
  (and (price ?item ?p) (tax-rate ?r) (bind ?tax (* ?p ?r)))
  (tax ?item ?tax))
```

An arithmetic expression may:
- Nest arbitrarily: `(+ (* ?a ?b) (- ?c ?d))`
- Mix variables and numeric literals: `(+ ?x 10)`
- Appear as any argument to any user-defined predicate or arithmetic predicate **in a rule body position** (see REQ-009 for the prohibition on arithmetic expressions in rule heads)
- In user-defined body literals, expression arguments are evaluated during grounding using the current substitution before literal matching. If evaluation fails (unbound variable, arithmetic error), that substitution path is discarded.

Trace:
- TEST-002
- CON-002

---

### REQ-003: `bind` Binding Predicate

The system SHALL provide a built-in `bind` predicate of the form `(bind ?var <expr>)` that binds the variable `?var` to the result of evaluating `<expr>` under the current substitution. The keyword `bind` is chosen over Prolog's `is` because it unambiguously communicates single-assignment binding without implying equality testing, and follows the precedent of CLIPS and Jess rule engines which use `(bind ?var ...)` with identical syntax and semantics.

- `?var` MUST be an unbound variable in the current substitution context.
- `<expr>` may reference only variables already bound by preceding body literals/constraints; otherwise the `bind` literal fails (the rule body does not fire).
- `bind` MUST appear in rule bodies only; it is a parse error to use `bind` as a rule head.
- On arithmetic error (division by zero, overflow), the `bind` literal fails silently (the rule body does not fire for that grounding).

```lisp
(given (price widget 80))
(given (quantity order-1 5))

(normally r1
  (and (price ?item ?p) (quantity ?order ?q) (bind ?total (* ?p ?q)))
  (order-total ?order ?total))

; Derived: +d order-total(order-1, 400)
```

Trace:
- TEST-003
- CON-003

---

### REQ-004: Comparison Predicates

The system SHALL provide six built-in comparison predicates — `=`, `!=`, `<`, `>`, `<=`, `>=` — each of the form `(<op> <expr1> <expr2>)`.

- A comparison predicate evaluates both expressions under the current substitution and checks the relation.
- If either expression contains an unbound variable, the comparison fails silently.
- On arithmetic error, the comparison fails silently.
- Comparison predicates MUST appear in rule bodies only; they are a parse error in rule heads.
- These predicates produce no conclusions of their own — they act as guards.

```lisp
(given (salary alice 120000))
(given (salary bob 85000))

(normally r1
  (and (salary ?emp ?amount) (> ?amount 100000))
  (high-earner ?emp))

; Derived: +d high-earner(alice)
; No conclusion for bob (85000 is not > 100000)
```

Trace:
- TEST-004
- CON-004

---

### REQ-005: Numeric Type Promotion

The system SHALL support arithmetic between `Integer`, `Decimal`, and `Float` operands. Promotion follows a **precision-loss hierarchy**: operations stay at the most precise type possible, and only widen toward `Float` when explicitly introduced.

**Promotion rules** (for operators `+`, `-`, `*`, `/`, `min`, `max`, `**`; applied pairwise in left-fold for variadic operators):

- Integer OP Integer → Integer (except `/`, which produces Decimal)
- Integer OP Decimal → Decimal
- Decimal OP Integer → Decimal
- Decimal OP Decimal → Decimal
- Float OP any → Float (float is "contagious" — once approximate, stays approximate)
- any OP Float → Float

The `/` operator returns `Decimal` for Integer/Integer and Decimal operands (e.g. `(/ 10 3)` → `Decimal(3.3333...)`), and `Float` if either operand is `Float`.

**Decimal precision limits**: `Decimal` values have a maximum of 29 significant digits and a scale of 0–28 (i.e., up to 28 fractional digits). The representable range is `±79,228,162,514,264,337,593,543,950,335` (2^96 − 1) with the decimal point placed anywhere within that digit span. When an exact result exceeds available precision (e.g., repeating decimals from division), the result is rounded using **round-half-even** (banker's rounding). If a decimal operation overflows the representable range, it is treated as an arithmetic error and fails silently. Implementation uses `rust_decimal::Decimal` (128-bit).

Float values are restricted to finite IEEE 754 values. Any arithmetic operation that yields `NaN`, `+inf`, or `-inf` is treated as an arithmetic error and fails silently.

The `div` operator performs **floor division** (rounds toward negative infinity). Both operands must be integers; if either is a Decimal or Float, the `div` literal fails silently. Examples: `(div 10 3)` → `3`; `(div -7 2)` → `-4`.

The `rem` operator returns the **floor remainder** matching `div`, defined as `a - (a div b) * b`. Both operands must be integers; if either is a Decimal or Float, the literal fails silently. Examples: `(rem 10 3)` → `1`; `(rem -7 2)` → `1`; `(rem 7 -2)` → `-1`.

**Runtime type enforcement**: If an operator or predicate expects a specific numeric type (e.g. `div`/`rem` require integers), passing a value of the wrong type causes the literal to fail silently — consistent with the general arithmetic failure model. No static type system is required; type mismatches are caught at grounding time.

Trace:
- TEST-005
- CON-002

---

### REQ-006: Arithmetic in Temporal Rules

The system SHALL permit arithmetic predicates and expressions to appear in the bodies of rules that also contain temporal literals. Variables bound as temporal endpoints or interval variables are not numeric and MUST NOT be used as arithmetic operands. If such a variable is encountered during arithmetic evaluation, the literal fails silently.

```lisp
(given (during (salary alice 120000) 0 100))
(given (during (salary alice 95000) 100 200))

(normally r1
  (and (during (salary ?emp ?amount) ?start ?end) (> ?amount 100000))
  (during (high-earner ?emp) ?start ?end))
```

Trace:
- TEST-006

---

### REQ-007: Arithmetic Predicates are Body-only Guards

The system SHALL ensure that arithmetic predicates (`bind`, `=`, `!=`, `<`, `>`, `<=`, `>=`) are body-only guards: they generate no `RuleLabel`, do not participate in superiority declarations, and do not appear in the conclusions set.

Trace:
- TEST-007

---

### REQ-008: Reserved Arithmetic Keywords

The arithmetic operators (`+`, `-`, `*`, `/`, `div`, `rem`, `abs`, `min`, `max`, `**`), the `bind` predicate, and the comparison predicates (`=`, `!=`, `<`, `>`, `<=`, `>=`) are **reserved keywords**. They cannot be used as user-defined predicate names, rule labels, or labels in `(prefer ...)` declarations.

The following keywords are also **reserved for future use**: `sum`, `count`, `avg`, `round`, `floor`, `ceil`. These are anticipated for aggregate operations and numeric rounding functions in a future specification. Reserving them now prevents user-defined predicates from colliding with future built-ins, avoiding breaking changes. Using any of these as a predicate name, rule label, or superiority label SHALL produce a parse error.

Trace:
- TEST-008

---

### REQ-009: No Arithmetic in Rule Heads

The system SHALL emit a parse error if an arithmetic predicate or an arithmetic expression appears in the head position of any rule (fact, strict, defeasible, or defeater). Rule heads remain regular predicate literals (variables are still allowed for grounding), but arithmetic forms are prohibited there. Arithmetic belongs exclusively to the constraint layer (body evaluation).

```lisp
; ILLEGAL — arithmetic predicate in rule head position
(normally r1 some-fact (bind ?x 5))

; ILLEGAL — comparison predicate in rule head position
(normally r1 some-fact (> ?x 0))

; ILLEGAL — arithmetic expression as argument in rule head
(given (cost (+ 3 4)))

; LEGAL — bind in body position (correct per REQ-003)
(normally r1 (bind ?x (+ 1 2)) result)
```

Trace:
- TEST-009

---

### REQ-010: Cross-Type Numeric Matching During Grounding

The system SHALL use numeric promotion when matching bound variable values against fact arguments during grounding. If a bound variable holds a numeric `Term` and the candidate fact argument is a different numeric `Term` type, the system SHALL promote both to a common type using the same promotion rules as REQ-005 and compare for equality.

Specifically:
- `Integer(100)` SHALL match `Decimal(100.00)` (integer promoted to decimal)
- `Integer(100)` SHALL match `Float(100.0)` (integer promoted to float)
- `Decimal(100.00)` SHALL match `Float(100.0)` (decimal promoted to float)
- `Integer(3)` SHALL NOT match `Decimal(3.14)` (not numerically equal after promotion)
- Non-numeric terms (`Symbol`) are never promoted; they match only by exact identity

**Rationale**: Without promotion during matching, arithmetic that produces `Decimal` values (e.g., `(* 100 0.08)` → `Decimal(8.00)`) would silently fail to match `Integer` facts (e.g., `(given (threshold 8))`). This creates a type-awareness burden on theory authors that contradicts the goal of transparent numeric support. Promotion during matching ensures that numerically equal values match regardless of how they were produced.

```lisp
(given (threshold 8))       ; Integer(8)
(given (rate standard 0.08))

(normally r1
  (and (rate standard ?r) (bind ?val (* 100 ?r))
       (threshold ?val))    ; Decimal(8.00) matches Integer(8) via promotion
  (threshold-met))

; Derived: +d threshold-met
```

Trace:
- TEST-010
- CON-005

---

### REQ-011: No Negation of Arithmetic Predicates

The system SHALL emit a parse error if an arithmetic predicate (`bind`, `=`, `!=`, `<`, `>`, `<=`, `>=`) appears inside a `not`/`~` negation in a rule body.

**Rationale**: SPL's `not`/`~` is classical negation — it matches explicit negated ground facts (e.g., `(not flies)` matches `~flies`). Arithmetic predicates are grounding-phase guards that produce no ground facts, so negating them is semantically meaningless. Users should write the complementary comparison directly:

- Instead of `(not (> ?x 100))`, write `(<= ?x 100)`
- Instead of `(not (< ?x 0))`, write `(>= ?x 0)`
- Instead of `(not (= ?x ?y))`, write `(!= ?x ?y)`

```lisp
; ILLEGAL — negation of arithmetic predicate
(normally r1 (and (val ?x) (not (> ?x 100))) (low ?x))

; LEGAL — use complementary comparison
(normally r1 (and (val ?x) (<= ?x 100)) (low ?x))
```

Trace:
- TEST-011

---

### REQ-012: Typed Numeric JSON Serialization (v2 Contracts)

The system SHALL provide a v2 JSON contract schema family that preserves the type identity and scale of numeric predicate arguments. Each predicate argument in v2 JSON output SHALL be represented as a tagged object rather than a bare JSON `number`, to avoid lossy representation.

**Rationale**: JSON's `number` type is IEEE 754 double-precision, which cannot losslessly represent:
- The distinction between `Integer(8)`, `Decimal(8.00)`, and `Float(8.0)`
- Decimal scale/trailing zeros (`8.00` vs `8` — significant for financial reasoning)
- Integers larger than 2^53 (JSON number loses precision)
- Exact decimal values (JSON number is binary floating-point)

The v1 contract schema SHALL remain unchanged for backward compatibility.

Trace:
- TEST-012
- CON-006

---

## 4. Non-Functional Requirements

### NFR-001: Grounding Performance

Arithmetic evaluation SHALL add negligible overhead to theory grounding time. Specifically: for a theory with fewer than 10,000 ground rule instances, the grounding phase with arithmetic predicates SHALL complete within the same order of magnitude as an equivalent non-arithmetic theory with identical predicate structure.

**Verification method**: A benchmark test (TEST-NFR-001) compares grounding time for a theory with arithmetic constraints against a structurally equivalent theory without arithmetic. The test measures **relative overhead** (arithmetic_time / baseline_time) across multiple runs, using statistical methods (e.g., median of N runs, discarding outliers) to account for system variance. The overhead ratio SHOULD be below 1.10 (10%) on the CI environment. This is a performance regression guard, not an absolute performance guarantee.

Rationale: Arithmetic evaluation is O(expression depth) per substitution. Since expression trees are shallow in practice and numeric evaluation is constant-time, overhead is expected to be negligible. The benchmark provides a regression signal without being tied to specific hardware.

Trace:
- TEST-NFR-001

---

### NFR-002: Overflow Safety

Arithmetic operations on integer operands SHALL never panic. On overflow, the operation SHALL fail silently (the `bind` predicate fails, comparison predicates fail). Overflow detection SHALL use checked arithmetic (`i64::checked_add`, etc.).

Rationale: Spindle is used in policy and contract reasoning where panics are unacceptable. Arithmetic failures must degrade gracefully.

Trace:
- TEST-NFR-002

---

### NFR-003: No Unsafe Code

The arithmetic module implementation SHALL contain no `unsafe` blocks.

---

## 5. Architecture Decisions

### ADR-001: Arithmetic as a Grounding-Phase Constraint

**Context**: Arithmetic predicates could be evaluated at parse time, grounding time, or reasoning time.

**Decision**: Arithmetic predicates are evaluated during the **grounding phase**, immediately after term substitution for each candidate substitution path. `bind`/comparison literals and arithmetic expression arguments are resolved while processing the rule body in source order (ADR-001b). If arithmetic evaluation fails, that substitution path is discarded.

**Rationale**:
- The grounding phase already performs substitution lookups; arithmetic evaluation is a natural extension.
- Evaluating at reasoning time would require the reasoning loop to understand types, coupling it to the arithmetic module.
- Evaluating at parse time is not possible for rules with variables.
- This is analogous to Prolog's `is/2` evaluation model.

**Trade-offs**:
- Arithmetic is not available during abduction (which works backward from goals). Abduction with arithmetic constraints is deferred to a future spec.
- The grounding phase must be extended to distinguish arithmetic literals from regular literals.

**Rejected alternatives**:
- *Reasoning-time evaluation*: Would require modifying the defeasible reasoning loop and indexing machinery — high risk, high coupling.
- *CLP(FD) integration*: Constraint logic programming over finite domains provides stronger guarantees (full constraint propagation) but is significantly more complex and out of scope.

---

### ADR-001b: Rule Body Evaluation Order (Logic + Arithmetic)

**Context**: A rule body may interleave user-defined literals and arithmetic constraints, and user-defined body literals may themselves contain arithmetic expression arguments (e.g. `(line-total ?o (* ?p ?q))`). Evaluation order determines visibility of bindings and whether expression arguments are evaluable at match time.

**Decision**: Rule bodies are evaluated **strictly in source order** (left to right) over a stream of substitutions.

For each body element in order:
- `BodyLiteral::Logic`: evaluate any `BodyArg::Arith` arguments under the current substitution, then match the resulting literal pattern against candidate facts to extend/filter substitutions.
- `BodyLiteral::Arithmetic`: evaluate `bind`/comparison under the current substitution to extend/filter substitutions.

Each step sees bindings produced by all preceding steps in the same body.

**Rationale**: Source-order evaluation is predictable. Users naturally write binding-producing literals before dependent constraints, expecting left-to-right visibility. A model that batches all logic before arithmetic would break valid dependent forms and produce confusing failures.

**Consequence**: The grounding implementation MUST preserve source order of all body elements and thread substitutions through each step. This is tested by TEST-004 scenarios 9 and 10 (chained `bind` + comparison in both valid and invalid orders).

**Rejected alternative**: *Dependency-graph ordering* — infer order from variable dependencies. More powerful but significantly more complex to implement and reason about.

---

### ADR-002: Extend `Term` Rather than Overloading `SymbolId`

**Context**: Currently all predicate arguments are `SymbolId` (a 4-byte interned string identifier). Numeric values could be encoded as special strings (e.g., `"__num_42"`) or as a new `Term` enum.

**Decision**: Introduce a `Term` enum with four variants — `Symbol`, `Integer`, `Decimal`, and `Float`:

```rust
pub enum Term {
    Symbol(SymbolId),
    Integer(i64),
    Decimal(rust_decimal::Decimal),  // 128-bit exact decimal
    Float(f64),
}
```

`Decimal` is the default for non-integer numeric literals (e.g. `0.08`). `Float` is opt-in via scientific notation (e.g. `1.5e3`). This ensures exact arithmetic by default for policy and financial reasoning, with IEEE 754 available as an escape hatch.

Arithmetic expressions (`ArithExpr`) are evaluated to `Term::Integer`, `Term::Decimal`, or `Term::Float` before being stored in ground literals.
In parsed rule bodies, arithmetic expressions may appear in `BodyLiteral::Arithmetic` and in user-defined body literal argument slots (`BodyArg::Arith`), but never in rule heads.

**Rationale**:
- Encoding numerics as strings would corrupt the string interner with sentinel values and would make comparison O(string parse) rather than O(1).
- A `Term` enum keeps numeric values typed and avoids implicit conversions.
- `Decimal` as default prevents silent precision loss in the primary use case (policy/contract/financial reasoning). `0.1 + 0.2 == 0.3` holds for `Decimal` but not `Float`.
- Ground literals (after grounding) contain only `Term` values. No `ArithExpr` persists past grounding.

**Trade-offs**:
- All code paths that handle `SymbolId` arguments must be updated to handle `Term`. This is a breaking change to `Literal`'s internal representation.
- `Literal` currently stores `predicate_ids: Vec<SymbolId>` — this becomes `predicate_args: Vec<Term>`. The `SymbolId` variant keeps existing literal behaviour unchanged.
- `Term` is now 24 bytes (128-bit Decimal + tag) vs. 4 bytes for `SymbolId`. `SmallVec` optimization may need re-evaluation.
- Adds `rust_decimal` as a dependency to `spindle-core`. This crate is mature (>50M downloads), no-std compatible, and provides banker's rounding (round-half-even) out of the box.

**Rejected alternatives**:
- *String-encoded numerics*: Corrupts the interner; O(string) comparisons; breaks sorted/canonical forms.
- *Separate numeric slot on `Literal`*: Awkward API; does not compose with arbitrary predicate arities.

---

### ADR-003: Polish Notation Arithmetic Expressions in S-expression Parser

**Context**: SPL already uses S-expression syntax (parenthesised prefix lists). Arithmetic expressions must be distinguishable from predicate applications.

**Decision**: Arithmetic operators (`+`, `-`, `*`, `/`, `div`, `rem`, `abs`, `min`, `max`, `**`) and built-in predicates (`bind`, `=`, `!=`, `<`, `>`, `<=`, `>=`) are **always reserved keywords**. When the parser sees `(+ ...)` or `(* ...)` etc. in a term position (i.e., as a predicate argument), it parses the form as an `ArithExpr` rather than a literal application.

This means:
- `(+ ?x ?y)` in argument position → `ArithExpr::BinOp(Add, Var(?x), Var(?y))`
- `(+ ?x ?y)` in predicate position (as head or rule keyword position) → parse error

The lexer requires targeted additions. The current `parse_atom` in `lexer.rs` accepts `+` but not `*`, `/`, `<`, `>`, `=`, or `!`. These characters must be added to the atom character set so that operator tokens such as `*`, `/`, `<=`, `>=`, `!=`, and `**` are lexed. Adding them to `parse_atom` is sufficient; no dedicated token types are required. The expression dispatcher then gains awareness of which atom values are reserved arithmetic operator keywords and dispatches accordingly. The `+` operator already lexes correctly.

**Trade-offs**:
- Theories that previously used `+`, `-`, `*`, `/`, `<`, `>`, `=`, or `bind` as standalone predicate names or as rule labels will break. These names are now reserved, consistent with Prolog and other logic languages. This is considered acceptable — using operator characters as predicate names/labels is rare and confusing. Multi-character atoms containing these characters (e.g., `high-earner`, `c++test`) are unaffected, since reservation applies only to standalone reserved tokens.

---

---

## 6. Contracts

### CON-001: Numeric Literal Term Grammar

The literal parser accepts numeric terms in all argument positions.

```
numeric-term    ::= integer-literal | decimal-literal | float-literal
integer-literal ::= '-'? [0-9]+
decimal-literal ::= '-'? [0-9]+ '.' [0-9]+
float-literal   ::= '-'? [0-9]+ ('.' [0-9]+)? ('e' | 'E') '-'? [0-9]+
```

The presence of an `e`/`E` exponent distinguishes `Float` from `Decimal`:
- `3.14` → `Term::Decimal` (exact)
- `3.14e0` → `Term::Float` (IEEE 754)
- `1e6` → `Term::Float`
- `42` → `Term::Integer`

Parsing produces `Term::Integer(i64)`, `Term::Decimal(rust_decimal::Decimal)`, or `Term::Float(f64)`. Parse errors for out-of-range values use the existing `ParseError` type with source offset.

**Range limits**:
- `Integer`: `−9,223,372,036,854,775,808` to `9,223,372,036,854,775,807` (i64)
- `Decimal`: up to 29 significant digits; max value `±79,228,162,514,264,337,593,543,950,335`; up to 28 fractional digits. Literals exceeding these limits produce a parse error.
- `Float`: finite IEEE 754 f64 values (`±1.7976931348623157 × 10^308`). Non-finite parse results produce a parse error.

Implements: REQ-001
Verified by: TEST-001

---

### CON-002: Arithmetic Expression AST and Operators

```rust
pub enum ArithExpr {
    Lit(NumericValue),
    Var(SymbolId),
    NaryOp { op: NaryArithOp, args: Vec<ArithExpr> },
    BinOp { op: BinArithOp, lhs: Box<ArithExpr>, rhs: Box<ArithExpr> },
    UnaryOp { op: UnaryArithOp, expr: Box<ArithExpr> },
}

pub enum NumericValue {
    Integer(i64),
    Decimal(rust_decimal::Decimal),
    Float(f64),
}

pub enum NaryArithOp {
    Add,      // +  (0+ args; identity: 0)
    Sub,      // -  (1+ args; unary: negation; n-ary: left-fold subtraction)
    Mul,      // *  (0+ args; identity: 1)
    Div,      // /  (1+ args; unary: reciprocal; n-ary: left-fold division)
    Min,      // min (1+ args)
    Max,      // max (1+ args)
}

pub enum BinArithOp {
    IDiv,     // div (integer floor division, rounds toward −∞; non-integer operands → fail)
    Rem,      // rem (floor remainder: a − (a div b)×b; non-integer operands → fail)
    Pow,      // **
}

pub enum UnaryArithOp {
    Abs,  // abs
}
```

**Variadic semantics** follow Racket and Common Lisp conventions:

| Operator | 0 args | 1 arg | 2+ args |
|---|---|---|---|
| `+` | `0` (identity) | `x` (identity) | Left-fold addition |
| `-` | parse error | `(- x)` → negation | `(- a b c)` → `a - b - c` (left-fold) |
| `*` | `1` (identity) | `x` (identity) | Left-fold multiplication |
| `/` | parse error | `(/ x)` → reciprocal (`1/x`, Decimal) | `(/ a b c)` → `a / b / c` (left-fold) |
| `min` | parse error | `x` (identity) | Minimum of all args |
| `max` | parse error | `x` (identity) | Maximum of all args |
| `div` | — | — | Binary only (2 args required) |
| `rem` | — | — | Binary only (2 args required) |
| `**` | — | — | Binary only (2 args required) |
| `abs` | — | Absolute value | — |

Unary `(- x)` replaces the former `neg` keyword. `neg` is no longer a reserved keyword.

**S-expression grammar for arithmetic expressions**:

```
arith-expr     ::= numeric-literal
                 | variable
                 | '(' nary-op arith-expr* ')'
                 | '(' bin-op arith-expr arith-expr ')'
                 | '(' 'abs' arith-expr ')'

nary-op        ::= '+' | '-' | '*' | '/' | 'min' | 'max'
bin-op         ::= 'div' | 'rem' | '**'
```

Examples:
```lisp
(+ ?x ?y ?z)               ; variadic add: x + y + z
(+ ?x ?y)                  ; binary add (also valid)
(- ?x)                     ; unary negation (replaces neg)
(- ?a ?b ?c)               ; left-fold: a - b - c
(* 2 ?radius)              ; multiply by constant
(/ (- ?a ?b) 2)            ; (a - b) / 2, result is decimal
(/ ?x)                     ; reciprocal: 1/x
(div ?n 3)                 ; floor division (binary only)
(abs (- ?x ?y))            ; absolute difference
(** ?base 2)               ; exponentiation (binary only)
(min ?a ?b ?c)             ; minimum of three values
(max ?score 0)             ; clamp to non-negative
```

Float safety and identity:
- Arithmetic producing non-finite float values (`NaN`, `+inf`, `-inf`) is treated as arithmetic failure.
- Stored float terms MUST use a total-equality representation suitable for hashing/indexing (e.g., canonicalized bit representation with `-0.0` normalized to `0.0`, or an equivalent finite-float wrapper type).

**Arithmetic error type**:

All arithmetic failures are represented internally using a typed error enum. The external behavior is always silent failure (grounding instance discarded), but the error type is retained for diagnostic purposes (e.g., future `why_not` explanations per OQ-6).

```rust
pub enum ArithError {
    /// Division by zero: `(/ x 0)`, `(div x 0)`, `(rem x 0)`
    DivisionByZero,
    /// Integer overflow: `(+ i64::MAX 1)`, `(* i64::MAX 2)`
    IntegerOverflow,
    /// Decimal overflow: result exceeds 29 significant digits / 128-bit range
    DecimalOverflow,
    /// Variable not bound in current substitution
    UnboundVariable { name: SymbolId },
    /// Operator requires a specific numeric type (e.g., div/rem require integers)
    TypeMismatch { op: &'static str, expected: &'static str, got: &'static str },
    /// Operation produced NaN, +inf, or -inf
    NonFiniteFloat,
    /// Unary reciprocal `(/ 0)` or `(/ 0.0)` — reciprocal of zero
    ReciprocalOfZero,
}
```

**Contract**: `ArithExpr::eval()` and `ArithConstraint::eval()` return `Result<NumericValue, ArithError>`. Callers in the grounding phase discard the substitution path on `Err`, but MAY log or collect the error for diagnostic output. The `ArithError` variant SHALL carry enough context to produce a human-readable explanation (variable name, operator, operand types).

Implements: REQ-002, REQ-005
Verified by: TEST-002, TEST-005

---

### CON-003: `bind` Predicate Interface

```
Syntax:        (bind <variable> <arith-expr>)
Position:      Rule body only (parse error in head position)
Pre-conditions:
  - <variable> must be a ?-prefixed variable identifier
  - <arith-expr> must be a valid arithmetic expression (CON-002)
  - <variable> must not already be bound in the current substitution
Post-conditions:
  - If all variables referenced in <arith-expr> are bound in the current substitution:
      <variable> is bound to the evaluated numeric result
  - If any variable referenced in <arith-expr> is unbound:
      grounding instance is discarded (ArithError::UnboundVariable)
  - If arithmetic error occurs (division by zero, overflow, type mismatch for div/rem):
      grounding instance is discarded (ArithError variant per CON-002)
  - The `bind` literal itself does not appear in the conclusions set
Errors (parse time):
  - `(bind ...)` with non-variable first argument: parse error
  - `(bind ...)` with non-expression second argument: parse error
  - `(bind ...)` in a rule head position: parse error
```

Implements: REQ-003
Verified by: TEST-003

---

### CON-004: Comparison Predicate Interface

```
Operators: = != < > <= >=
Syntax:    (<op> <arith-expr> <arith-expr>)
Position:  Rule body only (parse error in head position)
Pre-conditions:
  - Both arguments must be valid arithmetic expressions (numeric literals,
    variables, or compound arith expressions)
Post-conditions:
  - If both expressions evaluate to numeric values under current substitution:
      grounding instance is retained iff the comparison holds
  - If either expression is unbound or contains an arithmetic error:
      grounding instance is discarded (ArithError variant per CON-002)
  - Comparison predicates appear nowhere in the conclusions set
Numeric comparison:
  - Integer compared to Decimal: integer is promoted to Decimal
  - Integer compared to Float: integer is promoted to Float
  - Decimal compared to Float: decimal is promoted to Float (precision loss)
  - Float compared to Float: IEEE 754 semantics for finite values
  - Non-finite float values are treated as arithmetic errors (comparison fails)
  - Decimal compared to Decimal: exact comparison
Errors (parse time):
  - Comparison operator in rule head position: parse error
  - Non-expression argument: parse error
```

Implements: REQ-004
Verified by: TEST-004

---

### CON-005: Cross-Type Numeric Matching

```
Context:       Grounding — matching a bound variable value against a fact argument
Pre-conditions:
  - A variable ?x is bound to a numeric Term (Integer, Decimal, or Float)
  - A candidate fact argument is also a numeric Term
Behavior:
  - Promote both values to a common type using REQ-005 promotion rules:
      Integer + Decimal → both Decimal
      Integer + Float  → both Float
      Decimal + Float  → both Float
      same type        → no promotion
  - Compare promoted values for exact equality
  - If equal: match succeeds (substitution extended or validated)
  - If not equal: match fails (substitution path discarded)
  - Symbol terms are never promoted; Symbol matches only by exact SymbolId identity
  - A numeric Term never matches a Symbol Term (and vice versa)
Non-finite float:
  - A Float value that is non-finite cannot be stored (per CON-002),
    so non-finite matching is not reachable at runtime
```

Implements: REQ-010
Verified by: TEST-010

---

### CON-006: v2 JSON Typed Argument Representation

Predicate arguments in v2 JSON contracts SHALL use a tagged object format:

```json
{
  "args": [
    { "type": "symbol", "value": "alice" },
    { "type": "integer", "value": 120000 },
    { "type": "decimal", "value": "8.00" },
    { "type": "float", "value": 1.5e3 }
  ]
}
```

**Type mapping**:
- `Term::Symbol` → `{ "type": "symbol", "value": "<string>" }`
- `Term::Integer` → `{ "type": "integer", "value": <json-number> }` (safe for values within ±2^53; values exceeding ±2^53 use string: `{ "type": "integer", "value": "9223372036854775807" }`)
- `Term::Decimal` → `{ "type": "decimal", "value": "<string>" }` (always string to preserve exact representation and scale, e.g., `"8.00"`, `"0.08"`, `"3.3333333333333333333333333333"`)
- `Term::Float` → `{ "type": "float", "value": <json-number> }` (IEEE 754 double; finite values only)

**Design rationale**:
- Decimal values MUST be serialized as strings because JSON numbers are IEEE 754 and cannot preserve exact decimal representation or trailing zeros.
- Integer values exceeding ±2^53 MUST be serialized as strings because JSON numbers lose precision beyond this range.
- The `type` tag enables lossless round-trip deserialization back to the correct `Term` variant.

**v1 compatibility**: v1 schemas continue to serialize all arguments as strings. v1 behavior is unchanged.

Implements: REQ-012
Verified by: TEST-012

---

## 7. Data Model Changes

### 7.1 Literal Predicate Arguments

**Current** (`literal.rs`):
```rust
pub struct Literal {
    // ...
    predicate_ids: Vec<SymbolId>,
}
```

**Proposed**:
```rust
pub struct Literal {
    // ...
    predicate_args: Vec<Term>,
}

pub enum Term {
    Symbol(SymbolId),
    Integer(i64),
    Decimal(rust_decimal::Decimal),
    Float(f64),
}
```

`Term::Symbol` is the default; existing code paths that destructure `predicate_ids` must be updated to pattern-match on `Term`. Theories without numeric literals will only produce `Term::Symbol`.

`Term` values participate in equality/hash paths (indexing, deduplication, substitution keys). Therefore `Term::Float` MUST have stable total equality/hash semantics (no NaN payload ambiguity; `-0.0` and `0.0` canonicalized) via an explicit wrapper or canonicalization strategy.

### 7.2 Grounding Substitution

**Current** (`grounding.rs`):
```rust
pub struct Substitution {
    pub terms: FxHashMap<SymbolId, SymbolId>,         // ?x → symbol
    pub temporal: FxHashMap<SymbolId, TimePoint>,
    pub intervals: FxHashMap<SymbolId, Temporal>,
}
```

**Proposed**:
```rust
pub struct Substitution {
    pub terms: FxHashMap<SymbolId, Term>,             // ?x → symbol or numeric
    pub temporal: FxHashMap<SymbolId, TimePoint>,
    pub intervals: FxHashMap<SymbolId, Temporal>,
}
```

### 7.3 ArithExpr — Scope of Storage

`ArithExpr` appears in two distinct phases with different storage lifetimes:

1. **Parsed rule representation**: `ArithExpr` IS stored in parsed rules as part of:
   - `BodyLiteral::Arithmetic(ArithConstraint)`, and
   - `BodyArg::Arith(ArithExpr)` within `BodyLiteral::Logic`.
   This is necessary because the same rule is grounded repeatedly for different substitutions, so expression trees must be retained.

2. **Ground literals**: `ArithExpr` is **never stored** in ground `Literal` instances or the conclusions set. After grounding evaluates a `bind` predicate or arithmetic guard, the resulting `Term::Integer`, `Term::Decimal`, or `Term::Float` is stored in the substitution map, and only ground `Term` values appear in ground literals.

In summary: `ArithExpr` is stored at the rule/theory level (in `BodyLiteral`/`BodyArg`), but never at the ground literal level. `ArithExpr` does not flow into ground `Literal::predicate_args` or the reasoning engine's conclusions.

### 7.4 Arithmetic Literal Representation in the Theory

Built-in arithmetic predicates (`bind`, `=`, `!=`, `<`, `>`, `<=`, `>=`) are represented in parsed rule bodies as a new variant of a `BodyLiteral` enum (or equivalent type). This separates them from user-defined literals at the type level and makes their special treatment in grounding explicit, without coupling them to the core `Literal` type.

```rust
pub enum BodyArg {
    Term(Term),            // Symbol or numeric literal
    Arith(ArithExpr),      // Arithmetic expression in body argument position
}

pub struct BodyLogicLiteral {
    pub name: SymbolId,
    pub negation: bool,
    pub mode: Mode,
    pub temporal: Temporal,
    pub temporal_expr: Option<TemporalExpr>,
    pub interval_var: Option<SymbolId>,
    pub predicate_args: Vec<BodyArg>,
}

pub enum BodyLiteral {
    Logic(BodyLogicLiteral),            // Match against theory facts
    Arithmetic(ArithConstraint),        // Evaluate numerically; no theory lookup
    // NOTE: Arithmetic has no negation flag. Negating an arithmetic
    // predicate (e.g., `(not (> ?x 100))`) is a parse error per REQ-011.
    // BodyLogicLiteral carries `negation: bool` for classical negation;
    // ArithConstraint intentionally does not.
}

pub enum ArithConstraint {
    Bind { var: SymbolId, expr: ArithExpr },
    Compare { op: CmpOp, lhs: ArithExpr, rhs: ArithExpr },
}

pub enum CmpOp { Eq, Ne, Lt, Gt, Le, Ge }
```

`BodyArg::Arith` is allowed only in rule-body logic literals. Rule heads and ground literals continue to store only concrete `Term` arguments.

---

## 8. SPL Syntax Summary

The following arithmetic forms are valid in rule bodies:

| Form | Meaning | Example |
|---|---|---|
| Numeric literal term | Integer, decimal, or float constant | `(given (cost item 42))` |
| `(bind ?v <expr>)` | Bind `?v` to evaluated `<expr>` | `(bind ?tax (* ?price 0.1))` |
| `(= <e1> <e2>)` | Numeric equality guard | `(= ?x 0)` |
| `(!= <e1> <e2>)` | Numeric inequality guard | `(!= ?a ?b)` |
| `(< <e1> <e2>)` | Less-than guard | `(< ?age 18)` |
| `(> <e1> <e2>)` | Greater-than guard | `(> ?salary 50000)` |
| `(<= <e1> <e2>)` | Less-than-or-equal guard | `(<= ?score 100)` |
| `(>= <e1> <e2>)` | Greater-than-or-equal guard | `(>= ?balance 0)` |
| `(+ e ...)` | Addition (variadic, 0+ args; identity: 0) | `(+ ?base ?bonus ?adj)` |
| `(- e ...)` | Unary negation or left-fold subtraction (1+ args) | `(- ?x)` / `(- ?total ?a ?b)` |
| `(* e ...)` | Multiplication (variadic, 0+ args; identity: 1) | `(* ?qty ?price)` |
| `(/ e ...)` | Unary reciprocal or left-fold division (1+ args) | `(/ ?x)` / `(/ ?revenue ?days)` |
| `(div e1 e2)` | Integer floor division (binary only) | `(div ?total 3)` |
| `(rem e1 e2)` | Integer floor remainder (binary only) | `(rem ?n 2)` |
| `(** e1 e2)` | Exponentiation (binary only) | `(** ?base 2)` |
| `(abs e)` | Absolute value | `(abs (- ?a ?b))` |
| `(min e ...)` | Minimum (variadic, 1+ args) | `(min ?a ?b ?c)` |
| `(max e ...)` | Maximum (variadic, 1+ args) | `(max ?score 0)` |

---

## 9. Worked Examples

### 9.1 Order Total with Tax

```lisp
; Facts
(given (unit-price widget 25))
(given (quantity order-001 4))
(given (tax-rate standard 0.08))

; Rules
(normally r1
  (and (unit-price ?item ?p) (quantity ?order ?q) (bind ?subtotal (* ?p ?q)))
  (subtotal ?order ?subtotal))

(normally r2
  (and (subtotal ?order ?s) (tax-rate standard ?r) (bind ?tax (* ?s ?r)))
  (tax ?order ?tax))

(normally r3
  (and (subtotal ?order ?s) (tax ?order ?t) (bind ?total (+ ?s ?t)))
  (total ?order ?total))
```

Expected conclusions (after grounding and reasoning):

```
+D unit-price(widget, 25)
+D quantity(order-001, 4)
+D tax-rate(standard, 0.08)       ; Decimal — exact
+d subtotal(order-001, 100)        ; Integer (25 * 4)
+d tax(order-001, 8.00)            ; Decimal (100 * 0.08 — exact, no float drift)
+d total(order-001, 108.00)        ; Decimal (100 + 8.00)
```

### 9.2 Defeasible Classification with Override

```lisp
(given (score alice 72))
(given (score bob 45))
(given (bonus alice 15))   ; alice gets a bonus

; Default classification
(normally r-pass  (and (score ?s ?n) (>= ?n 50)) (passes ?s))
(normally r-fail  (and (score ?s ?n) (< ?n 50))  (fails ?s))

; With bonus, adjust effective score
(normally r-bonus
  (and (score ?s ?n) (bonus ?s ?b) (bind ?adj (+ ?n ?b)))
  (effective-score ?s ?adj))

; Override: use effective score if available
(normally r-pass-eff
  (and (effective-score ?s ?n) (>= ?n 50))
  (passes ?s))

(prefer r-pass-eff r-fail)
```

Expected:
```
+d passes(alice)    ; 72 already >= 50, bonus gives effective 87
+d fails(bob)       ; 45 < 50, no bonus
+d effective-score(alice, 87)
```

### 9.3 Temporal Arithmetic (Salary Bands Over Time)

```lisp
(given (during (salary alice 95000) 0 100))
(given (during (salary alice 110000) 100 200))
(given (threshold senior 100000))

(normally r1
  (and (during (salary ?emp ?amt) ?s ?e)
       (threshold senior ?t)
       (> ?amt ?t))
  (during (senior-earner ?emp) ?s ?e))
```

Expected:
```
+d during(senior-earner(alice), 100, 200)  ; only the 110000 period qualifies
```

### 9.4 Decimal Precision and Rounding

```lisp
; Exact decimal arithmetic — no float drift
(given (amount x 0.1))
(given (amount y 0.2))
(normally r1
  (and (amount x ?a) (amount y ?b) (bind ?sum (+ ?a ?b)))
  (total ?sum))

; Derived: +d total(0.3)  — exact, not 0.30000000000000004

; Division with rounding
(given (shares total 10))
(given (parties num 3))
(normally r2
  (and (shares total ?n) (parties num ?d) (bind ?each (/ ?n ?d)))
  (share-per-party ?each))

; Derived: +d share-per-party(3.3333333333333333333333333333)
; 28 fractional digits, round-half-even applied
```

---

## 10. Test Specifications

### TEST-001: Numeric Literal Terms

**Scenarios**:
1. Integer literal: `(given (quantity x 42))` → `Term::Integer(42)`
2. Negative integer: `(given (balance x -100))` → `Term::Integer(-100)`
3. Decimal literal: `(given (rate x 0.15))` → `Term::Decimal(0.15)` — exact
4. Decimal with leading zero: `(given (tax x 0.08))` → `Term::Decimal(0.08)` — exact, not `0.07999...`
5. Float via scientific notation: `(given (mass x 1.5e3))` → `Term::Float(1500.0)`
6. Float without decimal: `(given (big x 1e6))` → `Term::Float(1000000.0)`
7. Integer and symbol as separate arguments: `(given (indexed item-a 1))` — parses correctly.
8. Integer that overflows i64 — parse error with source position.
9. Decimal that exceeds 29 significant digits (e.g., `123456789012345678901234567890.0`) — parse error with source position.
10. Decimal rounding: `(bind ?z (/ 10 3))` → `Decimal(3.3333333333333333333333333333)` — 28 fractional digits, round-half-even applied.

Verifies: REQ-001, CON-001

---

### TEST-002: Arithmetic Expression Parsing

**Scenarios**:
1. `(+ 3 4)` in argument position → `ArithExpr::NaryOp(Add, [Lit(3), Lit(4)])`
2. Nested: `(* (+ ?a ?b) 2)` → parses as nested `ArithExpr`
3. Variadic: `(+ 1 2 3)` → `NaryOp(Add, [Lit(1), Lit(2), Lit(3)])`, evaluates to `6`
4. Zero-arg identity: `(+)` → `Integer(0)`; `(*)` → `Integer(1)`
5. Unary `-`: `(- ?x)` → negation; unary `/`: `(/ ?x)` → reciprocal (Decimal)
6. `(- 10 3 2)` → left-fold: `10 - 3 - 2 = 5`; `(/ 12 3 2)` → `12 / 3 / 2 = 2`
7. Strictly binary operators parse: `div`, `rem`, `**`
8. `(abs (- ?a ?b))` → absolute difference
9. Variadic min/max: `(min ?a ?b ?c)`, `(max 0 ?x)` parse correctly
10. `(div 10 3 2)` → parse error (div requires exactly 2 arguments)
11. Arithmetic operator at predicate (non-argument) position → parse error
12. Arithmetic expression inside a user-defined body literal argument parses as `BodyArg::Arith`, e.g. `(normally r1 (and (price ?i ?p) (line-total ?i (* ?p 2))) (ok ?i))`

Verifies: REQ-002, CON-002

---

### TEST-003: `bind` Predicate Binding

**Scenarios**:
1. `(bind ?z (+ ?x ?y))` with `?x=3, ?y=4` bound → `?z` binds to `7`
2. `(bind ?z (+ ?x ?y))` with `?y` unbound → grounding discarded (no conclusion)
3. `(bind ?z (/ 10 0))` → division by zero; grounding discarded
4. `(bind ?z (** 2 62))` → `z = 4611686018427387904` (within i64)
5. `(bind ?z (** 2 63))` → overflow; grounding discarded
6. `bind` in rule head position → parse error
7. `bind` with non-variable first argument `(bind 5 (+ 2 3))` → parse error

Verifies: REQ-003, CON-003

---

### TEST-004: Comparison Predicates

**Preconditions**: Theory has `(given (val x 10))`, `(given (val y 20))`.

**Scenarios**:
1. `(> ?a ?b)` with `?a=20, ?b=10` → grounding retained
2. `(> ?a ?b)` with `?a=5, ?b=10` → grounding discarded
3. `(= ?a ?b)` with `?a=10, ?b=10` → retained; `?a=10, ?b=11` → discarded
4. `(!= ?a ?b)` with `?a=10, ?b=10` → discarded; `?a=10, ?b=11` → retained
5. `(<= ?a ?b)` with `?a=10, ?b=10` → retained
6. Mixed types: `(> 10 9.5)` → integer promoted to decimal; retained
7. Comparison in rule head position → parse error
8. Unbound variable in comparison → grounding discarded
9. Chained order-dependent constraints: `(and (bind ?x (+ 2 3)) (> ?x 4))` → retained
10. Reversed dependent order: `(and (> ?x 4) (bind ?x (+ 2 3)))` → discarded (`?x` unbound at comparison time)

Verifies: REQ-004, CON-004

---

### TEST-005: Numeric Type Promotion Rules

**Scenarios** (Integer/Decimal/Float promotion):
1. `(+ 3 4)` → `Integer(7)`
2. `(+ 3 4.0)` → `Decimal(7.0)` (decimal literal, not float)
3. `(+ 3 4.0e0)` → `Float(7.0)` (scientific notation → float, contagious)
4. `(+ 0.1 0.2)` → `Decimal(0.3)` — exact (would be `0.30000000000000004` as float)
5. `(/ 10 3)` → `Decimal(3.3333333333333333333333333333)` (integer/integer → decimal, up to 28 fractional digits)
6. `(/ 10 3.0e0)` → `Float(3.333...)` (float operand → float result)
7. `(* 100 0.08)` → `Decimal(8.00)` (integer × decimal → decimal, exact)

**Scenarios** (div/rem — integer-only):
8. `(div 10 3)` → `Integer(3)` (floor: ⌊10/3⌋ = 3)
9. `(div -7 2)` → `Integer(-4)` (floor: ⌊-7/2⌋ = -4, not -3)
10. `(rem 10 3)` → `Integer(1)` (floor remainder: 10 − (3×3) = 1)
11. `(rem -7 2)` → `Integer(1)` (floor remainder: -7 − (-4×2) = 1)
12. `(rem 7 -2)` → `Integer(-1)` (floor remainder: 7 − (-4×-2) = -1)
13. `(div 10 3.0)` → evaluation fails (div requires integer operands)
14. `(rem 10 3.0)` → evaluation fails

Verifies: REQ-005, CON-002

---

### TEST-006: Arithmetic in Temporal Rules

**Preconditions**: Temporal reasoning active.

**Scenarios**:
1. Rule with `during` literal and `>` comparison — fires for intervals where comparison holds, does not fire for others (see worked example §9.3).
2. Rule body referencing an interval variable `?T` in arithmetic expression `(+ ?T 1)` → arithmetic literal evaluation fails (temporal/interval variables are not numeric).

Verifies: REQ-006

---

### TEST-007: Arithmetic Predicates Cannot Appear in Superiority

**Scenarios**:
1. `(prefer bind r1)` → parse error ("'bind' is a reserved arithmetic keyword and cannot be used as a rule label").
2. Arithmetic guard literals do not produce conclusions (e.g., from `(normally r1 (and (p) (> 1 0)) (q))`, conclusions include `q` but never include a literal named `>` or `bind`).

Verifies: REQ-007

---

### TEST-008: Reserved Keyword Enforcement

**Scenarios**:
1. `(normally r1 (+ ?x ?y) result)` where `+` is used as a predicate name → parse error ("'+' is a reserved arithmetic operator")
2. `(normally r1 body (bind ?x 5))` where `bind` is in head position → parse error
3. `(given (cost (+ 3 4)))` where arithmetic expression appears in fact argument → parse error (arithmetic expression in head/fact position)
4. `(normally bind body result)` where `bind` is used as a rule label → parse error ("'bind' is reserved")
5. `(prefer r1 +)` where `+` is used as a rule label in superiority → parse error ("'+' is reserved")
6. `(given (sum report 100))` where `sum` is used as a predicate name → parse error ("'sum' is reserved for future use")
7. `(normally count body result)` where `count` is used as a rule label → parse error ("'count' is reserved for future use")
8. Future-reserved keywords `avg`, `round`, `floor`, `ceil` as predicate names → parse error

Verifies: REQ-008

---

### TEST-009: No Arithmetic in Rule Heads

**Scenarios**:
1. `(normally r1 body (bind ?x 5))` → parse error
2. `(normally r1 body (> ?x 0))` → parse error
3. `(normally r1 body (cost item (+ 1 2)))` → parse error (arithmetic expression in head argument)

Verifies: REQ-009

---

### TEST-010: Cross-Type Numeric Matching

**Scenarios**:
1. Fact `(given (threshold 8))` (Integer), variable bound to `Decimal(8.00)` via `(bind ?val (* 100 0.08))` — matching `(threshold ?val)` succeeds (Integer promoted to Decimal, `8 == 8.00`).
2. Fact `(given (limit 100))` (Integer), variable bound to `Float(100.0)` via `(bind ?val (* 50.0e0 2.0e0))` — matching `(limit ?val)` succeeds (Integer promoted to Float).
3. Fact `(given (rate 0.5))` (Decimal), variable bound to `Integer(1)` via `(bind ?val (+ 0 1))` — matching `(rate ?val)` fails (`Decimal(0.5) != Decimal(1)`).
4. Fact `(given (name alice))` (Symbol), variable bound to `Integer(100)` — matching `(name ?val)` fails (Symbol never matches numeric).
5. Fact `(given (score 95))` (Integer), variable bound to `Decimal(95.00)` — matching `(score ?val)` succeeds.
6. Fact `(given (score 95))` (Integer), variable bound to `Decimal(95.01)` — matching `(score ?val)` fails (`95 != 95.01`).

**End-to-end scenario**:
```lisp
(given (threshold 8))
(given (rate standard 0.08))

(normally r1
  (and (rate standard ?r) (bind ?val (* 100 ?r)) (threshold ?val))
  (threshold-met))

; Derived: +d threshold-met
```

Verifies: REQ-010, CON-005

---

### TEST-011: No Negation of Arithmetic Predicates

**Scenarios**:
1. `(normally r1 (and (val ?x) (not (> ?x 100))) (low ?x))` → parse error ("arithmetic predicate cannot be negated; use complementary comparison")
2. `(normally r1 (and (val ?x) (not (= ?x 0))) (nonzero ?x))` → parse error
3. `(normally r1 (and (val ?x) (not (bind ?y (+ ?x 1)))) (result ?x))` → parse error
4. `(normally r1 (and (val ?x) (~(> ?x 100))) (low ?x))` → parse error (tilde negation variant)
5. `(normally r1 (and (val ?x) (<= ?x 100)) (low ?x))` → legal (complementary comparison)

Verifies: REQ-011

---

### TEST-012: v2 JSON Typed Argument Serialization

**Scenarios**:
1. `Term::Symbol("alice")` → `{ "type": "symbol", "value": "alice" }`
2. `Term::Integer(120000)` → `{ "type": "integer", "value": 120000 }`
3. `Term::Integer(9223372036854775807)` (> 2^53) → `{ "type": "integer", "value": "9223372036854775807" }` (string to preserve precision)
4. `Term::Decimal(8.00)` → `{ "type": "decimal", "value": "8.00" }` (string; trailing zeros preserved)
5. `Term::Decimal(0.08)` → `{ "type": "decimal", "value": "0.08" }` (string; exact)
6. `Term::Float(1.5e3)` → `{ "type": "float", "value": 1500.0 }`
7. Mixed argument list `[Symbol("widget"), Integer(25), Decimal(0.08)]` round-trips losslessly through v2 JSON serialization and deserialization.
8. v1 JSON output for the same theory is unchanged (all arguments serialized as strings).

Verifies: REQ-012, CON-006

---

### TEST-NFR-001: Grounding Performance Regression

**Setup**: Generate two theories: (a) 500 facts, 200 defeasible rules with 3-literal bodies including arithmetic `bind` and comparison predicates; (b) structurally equivalent theory with the same predicate shapes but no arithmetic predicates (arithmetic literals replaced with additional fact-matching literals).

**Method**: Run grounding phase only (excluding defeasible reasoning) for both theories. Take the median of at least 10 runs each, discarding the fastest and slowest run. Compute overhead ratio = median(a) / median(b).

**Assertion**: The overhead ratio SHOULD be below 1.10 (10%). If the ratio exceeds 1.10, the test emits a warning (not a hard failure) to flag potential performance regressions for investigation.

Verifies: NFR-001

---

### TEST-NFR-002: Integer Overflow Safety

**Scenarios**:
1. `(bind ?z (+ 9223372036854775807 1))` → silent failure, no panic
2. `(bind ?z (* 9223372036854775807 2))` → silent failure, no panic
3. `(bind ?z (- -9223372036854775808 1))` → silent failure, no panic

Verifies: NFR-002

---

### TEST-PBT-001: Property-Based Arithmetic Semantics

Use `proptest` to generate arithmetic expressions, substitutions, and rule-body fragments under bounded depth/size.

**Generator constraints**:
1. Expression depth bounded (for example, max depth 4) to avoid pathological recursion.
2. Numeric domains include `Integer`, `Decimal`, and finite `Float` values.
3. Separate suites for division/remainder enforce non-zero divisors when testing algebraic identities.
4. Float generators exclude non-finite inputs (`NaN`, `+inf`, `-inf`) unless explicitly testing failure behavior.

**Properties**:
1. **Determinism**: `ArithExpr::eval(expr, subst)` returns the same result/failure on repeated evaluation.
2. **Panic safety**: evaluating generated expressions and arithmetic constraints never panics.
3. **Promotion correctness**: result type matches REQ-005 promotion rules for all generated binary operations.
4. **`div`/`rem` identity** (integer operands, divisor ≠ 0): `a = b * div(a,b) + rem(a,b)`.
5. **Floor remainder sign rule**: `rem(a,b)` has the same sign as `b` (or is zero), matching floor-division semantics.
6. **Comparison reflexivity/coherence** (finite values): `x = x`, `x <= x`, `x >= x`, and `x != x` is false.
7. **Float safety**: any operation that yields non-finite float fails rather than producing a stored value.
8. **Source-order dependency**: for generated dependent pairs, `(and (bind ?x E) (> ?x K))` may succeed, while the reversed order fails when `?x` is unbound at comparison time.
9. **Body-arg expression equivalence**: matching behavior of a body literal with `BodyArg::Arith` is equivalent to an explicit pre-binding form using `bind` and a symbol/numeric literal argument.
10. **Parser round-trip stability (normalized)**: arithmetic forms parse to AST and re-parse from canonical SPL to an equivalent AST.

**Implementation note**:
When cross-checking arithmetic results against host Rust behavior, use checked integer operations and `rust_decimal` APIs for decimal semantics; treat Rust float non-finite outcomes as expected arithmetic failures in Spindle.

Verifies: REQ-002, REQ-003, REQ-004, REQ-005, NFR-002

---

## 11. Implementation Guidance

### Phase 1 — Core Term Type (spindle-core)

1. Add `rust_decimal` dependency to `spindle-core/Cargo.toml`.
2. Add `Term` enum (with `Symbol`, `Integer`, `Decimal`, `Float` variants) to `crates/spindle-core/src/term.rs` (new file).
3. Add `ArithExpr`, `ArithConstraint`, `BinArithOp`, `UnaryArithOp` to `crates/spindle-core/src/arith.rs` (new file).
4. Add `CmpOp` enum and `ArithConstraint::eval(subst)` method with promotion rules from REQ-005.
5. Implement stable float term identity for hashing/equality (canonicalized finite-float wrapper or equivalent; reject non-finite runtime results).
6. Update `Literal::predicate_args` from `Vec<SymbolId>` to `Vec<Term>`.
7. Update `Substitution::terms` from `FxHashMap<SymbolId, SymbolId>` to `FxHashMap<SymbolId, Term>`.
8. Add `BodyLiteral` enum to `rule.rs` (or a new `body.rs`), replacing raw `Literal` in `RuleBody`.

> **Risk**: Step 6 and 7 are breaking changes. All match arms on `predicate_ids` and `Substitution::terms` must be audited. The grounding module (`grounding.rs`) is the most impacted.

### Phase 2 — Parser Extension (spindle-parser)

1. Extend `spl/lexer.rs` `parse_atom` to accept `*`, `/`, `<`, `>`, `=`, `!` as atom characters, so operator tokens lex correctly.
2. Extend `spl/literals.rs` to parse numeric literals: integers → `Term::Integer`, decimal-point literals → `Term::Decimal`, scientific notation → `Term::Float`.
3. Add `spl/arith.rs` to parse arithmetic expressions recursively and produce `ArithExpr`.
4. Parse arithmetic expressions in user-defined body literal argument positions; produce `BodyArg::Arith`.
5. Parse `bind` and comparison predicates in body literal position; produce `BodyLiteral::Arithmetic`.
6. Add parse-time guards: arithmetic in head position → parse error; negation of arithmetic predicates → parse error (REQ-011); reserved keywords as predicate names and rule labels (`normally`/`always`/`except` labels, `prefer` labels) → parse error.

### Phase 3 — Grounding Integration (spindle-core)

1. Extend `grounding.rs` `match_literal` to handle `Term::Integer`/`Term::Decimal`/`Term::Float` in substitution matching with cross-type numeric promotion (REQ-010, CON-005). When both sides are numeric, promote to common type and compare.
2. Evaluate rule bodies in source order, threading substitutions through each `BodyLiteral`:
   - For `BodyLiteral::Logic`, evaluate any `BodyArg::Arith` arguments first, then match facts.
   - For `BodyLiteral::Arithmetic`, evaluate `bind`/comparison directly.
   Discard substitution paths on failure at any step.
3. Propagate bound `Term::Integer`/`Term::Decimal`/`Term::Float` values through `Substitution::apply`.

### Phase 4 — Tests

Write tests in the order of TEST-001 through TEST-009, then NFR tests, then TEST-PBT-001. Implement TEST-PBT-001 with `proptest` using bounded generators and explicit finite-float handling.

### Phase 5 — Contract and CLI Output (REQ-012)

1. Keep `spindle.*.v1` schemas and DTOs unchanged for backward compatibility.
2. Add a new schema family (`spindle.reason.v2`, `spindle.query.v2`, `spindle.requires.v2`, `spindle.explain.v2`, `spindle.why_not.v2`) with typed literal args using the tagged representation from CON-006. Each predicate argument is a JSON object with `type` and `value` fields to preserve type identity and decimal scale.
3. Update CLI/WASM serialization to emit v2 payloads when requested (or when v2 is negotiated via capabilities), while preserving v1 behavior.
4. Add regression tests for mixed argument lists (`symbol + integer + decimal + float`) in v2 JSON output and compatibility tests proving v1 payloads are unchanged.

---

## 12. Open Questions

| # | Question | Impact | Proposed Resolution |
|---|---|---|---|
| OQ-1 | Should rule-body evaluation be strictly source-ordered to support dependent bindings across logic and arithmetic forms? | High | **Resolved**: Yes. Rule bodies are evaluated left-to-right in source order, and each element sees substitutions extended by preceding elements. See ADR-001b. |
| OQ-2 | Should JSON/contracts represent numeric predicate args as typed numbers or continue string-only serialization? | Medium | **Resolved**: Introduce typed numeric args in a new `spindle.*.v2` schema family; keep `spindle.*.v1` string args unchanged for compatibility. |
| OQ-3 | Should `=` be overloaded for both numeric equality and symbol identity? | High | **Resolved**: No. `=` is numeric-only (reserved keyword). Symbol identity is handled via pattern matching in substitution. |
| OQ-4 | Should arithmetic predicates produce `+D` conclusions (strict) or `+d` (defeasible)? | High | **Resolved**: Neither — they are grounding-phase guards; they produce no conclusions. See REQ-007. |
| OQ-5 | Should abduction support arithmetic? E.g. "what value of ?x makes `(> ?x 100)` hold?" | Medium | **Resolved**: Out of scope for this spec. Abduction works backward from goals; arithmetic evaluation requires forward (ground) substitutions. Deferred to a future abduction-with-constraints spec. |
| OQ-6 | Should `why_not` explain arithmetic failures? | Medium | **Resolved**: Enabled by design. The `ArithError` enum (CON-002) captures typed failure reasons (division by zero, overflow, unbound variable, type mismatch, etc.). The grounding phase MAY collect these errors for diagnostic output. Implementation of `why_not` arithmetic explanations is deferred but no further design work is needed — the error type provides the necessary context. |
| OQ-7 | Should overflow produce a warning or strict error rather than silent failure? | Low | Silent failure preferred (consistent with Prolog's arithmetic); observable via `why_not` if implemented |
| OQ-8 | Rational numbers (exact fractions)? | Low | Partially addressed by `Decimal`; full rationals (arbitrary numerator/denominator) deferred |
| OQ-9 | Cross-type fact matching: if a fact stores `Integer(100)` and `bind` produces `Decimal(100.00)`, does matching `(cost ?item ?x)` against the fact succeed? `Integer(100) ≠ Decimal(100.00)` at the `Term` level, which could silently discard valid groundings. | High | **Resolved**: Yes — numeric promotion during matching. When both values are numeric, promote to a common type per REQ-005 and compare for equality. See REQ-010, CON-005, TEST-010. |
| OQ-10 | Cross-type comparison vs unification: `(= 1 1.0)` is true under comparison promotion rules, but should `Integer(1)` and `Decimal(1.0)` unify during pattern matching? If not, users will encounter subtle mismatches where a comparison succeeds but a fact lookup fails for the same values. | High | **Resolved**: Yes — matching uses the same promotion rules as comparison predicates. `Integer(1)` matches `Decimal(1.0)` during fact lookup. See REQ-010. |
| OQ-11 | Arithmetic in negated body literals: is `(and (not (> ?x 100)) ...)` legal? If so, what are the semantics — does `not` negate the guard (i.e., `<= 100`), or does negation-as-failure apply (the guard is "not provable")? If the variable is unbound inside `not`, should it fail silently or produce a parse error? | High | **Resolved**: Parse error. SPL's `not`/`~` is classical negation (matches explicit negated ground facts). Arithmetic predicates produce no ground facts, so negating them is meaningless. Users write the complementary comparison directly. See REQ-011, TEST-011. |
| OQ-12 | Decimal display formatting: `100 * 0.08` yields `Decimal(8.00)` because `rust_decimal` preserves scale. Should output display `8.00`, `8.0`, or `8`? For financial/policy use cases trailing zeros carry meaning; for general use they may confuse. | Medium | **Resolved**: Preserve scale. v2 JSON serializes decimals as strings (e.g., `"8.00"`) to retain exact representation (CON-006). CLI text output uses `rust_decimal`'s default display, which preserves trailing zeros. |
| OQ-13 | Multi-arity arithmetic operators: should `(+ 1 2 3)` be valid (variadic, as in Lisp)? | Medium | **Resolved**: Yes. `+`, `-`, `*`, `/`, `min`, `max` follow Racket/Common Lisp variadic conventions. `div`, `rem`, `**` remain binary-only. See CON-002. |
| OQ-14 | Arithmetic in queries: can the query interface accept arithmetic guards, e.g., `(query (and (salary ?emp ?amt) (> ?amt 100000)))`? The spec covers rules but not the query system. | Medium | **Resolved**: Yes. Queries invoke `reason_with_options()` which runs the full pipeline (validate → wildcard rewrite → ground → reason). Arithmetic in rule bodies is evaluated during grounding, and queries search the resulting conclusions. No query-specific changes are needed — arithmetic support in grounding automatically benefits queries. |
| OQ-15 | Arithmetic in `except` (defeater) rules: defeater bodies should support arithmetic guards (e.g., `(except r1 (and (hardship ?emp) (< ?income 30000)) (high-earner ?emp))`). The spec says "body-only" but does not explicitly mention defeaters. | Medium | **Resolved**: Yes. Defeaters (`except`) are first-class rules with bodies parsed by the same `parse_body_with_line()` function as `normally`/`always` rules, and grounded by the same pipeline. No defeater-specific changes are needed. Arithmetic in defeater bodies works identically to arithmetic in defeasible rule bodies. |
| OQ-16 | Variable safety with `bind`: if `bind` introduces a variable that only appears in the head and not in any fact-matching literal (e.g., `(normally r1 (bind ?x (+ 1 2)) (result ?x))`), is this safe? The variable has no grounding source from facts. | Low | Valid per spec — `bind` produces a ground value — but unusual; may warrant a lint warning |
| OQ-17 | Aggregates and defeasibility: when `sum`/`count`/`avg` are eventually implemented, should aggregation occur before or after defeasible conflict resolution? Aggregating before resolution could include defeated conclusions; aggregating after could miss intermediate values. | Medium | **Resolved**: Deferred by design. This question cannot be answered without a complete aggregation semantics specification. The reserved keywords (`sum`, `count`, `avg` per REQ-008) ensure forward compatibility. The resolution will be provided in the future aggregation spec; no decision in this spec is affected by the eventual answer. |

---

## 13. Traceability Matrix

| REQ | NFR | ADR | CON | TEST |
|---|---|---|---|---|
| REQ-001 | — | ADR-002 | CON-001 | TEST-001 |
| REQ-002 | — | ADR-003 | CON-002 | TEST-002 |
| REQ-003 | — | ADR-001, ADR-001b | CON-003 | TEST-003 |
| REQ-004 | — | ADR-001, ADR-001b | CON-004 | TEST-004 |
| REQ-005 | — | ADR-002 | CON-002 | TEST-005 |
| REQ-006 | — | ADR-001 | — | TEST-006 |
| REQ-007 | — | ADR-001 | — | TEST-007 |
| REQ-008 | — | ADR-003 | — | TEST-008 |
| REQ-009 | — | ADR-003 | — | TEST-009 |
| REQ-010 | — | — | CON-005 | TEST-010 |
| REQ-011 | — | — | — | TEST-011 |
| REQ-012 | — | — | CON-006 | TEST-012 |
| — | NFR-001 | — | — | TEST-NFR-001 |
| — | NFR-002 | — | — | TEST-NFR-002 |
| — | NFR-003 | — | — | (static analysis) |
| REQ-002, REQ-003, REQ-004, REQ-005 | NFR-002 | ADR-001, ADR-001b, ADR-002 | CON-002, CON-003, CON-004 | TEST-PBT-001 |
