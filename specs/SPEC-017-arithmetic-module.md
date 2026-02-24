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

This specification adds **built-in arithmetic** to SPL: numeric terms, arithmetic expressions in Polish (prefix) notation, and built-in arithmetic predicates (`is`, comparisons). Arithmetic operators (`+`, `-`, `*`, `/`, `<`, `>`, `=`, etc.) and `is` are **reserved keywords**, consistent with every major logic programming language (Prolog, Datalog, ASP). No opt-in directive is required. All arithmetic expressions use S-expression prefix notation consistent with SPL's existing syntax — no new notation forms are introduced.

### Scope

| In Scope | Out of Scope |
|---|---|
| Integer arithmetic (i64) | Symbolic/algebraic solving |
| Floating-point arithmetic (f64) | Constraint logic programming (CLP(R)) |
| Numeric comparison predicates | String operations |
| Arithmetic in rule bodies (constraints + bindings) | Arithmetic in rule heads (computed conclusions) |
| Arithmetic during grounding | Arithmetic during tabling / abduction |

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

- **Integer literals**: decimal digit sequences optionally prefixed with `-` (e.g., `0`, `42`, `-7`, `1000000`)
- **Float literals**: IEEE 754 double-precision, expressed as decimal with a `.` separator (e.g., `3.14`, `-0.5`, `1.0e6`)

```lisp
(given (cost item-a 100))        ; integer term 100
(given (price widget 9.99))      ; float term 9.99
(given (balance account -250))   ; negative integer
```

Trace:
- TEST-001
- CON-001

---

### REQ-002: Arithmetic Expression Terms

The system SHALL accept arithmetic expressions as terms in predicate argument positions. Arithmetic expressions are S-expressions using the prefix operators defined in CON-003. Variables within expressions follow the existing `?name` convention.

```lisp
; (+ ?x ?y) is an arithmetic expression term
(normally r1
  (and (price ?item ?p) (tax-rate ?r) (is ?tax (* ?p ?r)))
  (tax ?item ?tax))
```

An arithmetic expression may:
- Nest arbitrarily: `(+ (* ?a ?b) (- ?c ?d))`
- Mix variables and numeric literals: `(+ ?x 10)`
- Appear as any argument to any user-defined predicate or arithmetic predicate **in a rule body position** (see REQ-009 for the prohibition on arithmetic expressions in rule heads)

Trace:
- TEST-002
- CON-002

---

### REQ-003: `is` Binding Predicate

The system SHALL provide a built-in `is` predicate of the form `(is ?var <expr>)` that binds the variable `?var` to the result of evaluating `<expr>` under the current substitution.

- `?var` MUST be an unbound variable in the current substitution context.
- If `<expr>` contains an unbound variable other than `?var`, the `is` literal fails to unify (the rule body does not fire).
- `is` MUST appear in rule bodies only; it is a parse error to use `is` as a rule head.
- On arithmetic error (division by zero, overflow), the `is` literal fails silently (the rule body does not fire for that grounding).

```lisp
(given (price widget 80))
(given (quantity order-1 5))

(normally r1
  (and (price ?item ?p) (quantity ?order ?q) (is ?total (* ?p ?q)))
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

### REQ-005: Mixed Integer and Float Semantics

The system SHALL support arithmetic between integer and float operands with the following promotion rules:

- Integer OP Integer → Integer (except `/`, which always produces Float)
- Integer OP Float → Float
- Float OP Integer → Float
- Float OP Float → Float

The `/` operator always performs IEEE 754 double-precision division and always returns a `Float`, even for two integer operands (e.g. `(/ 10 3)` → `Float(3.333...)`).

The `div` operator performs **floor division** (rounds toward negative infinity). Both operands must be integers; if either is a float, the `div` literal fails silently. Examples: `(div 10 3)` → `3`; `(div -7 2)` → `-4`.

The `rem` operator returns the **floor remainder** matching `div`, defined as `a - (a div b) * b`. Both operands must be integers; if either is a float, the literal fails silently. Examples: `(rem 10 3)` → `1`; `(rem -7 2)` → `1`; `(rem 7 -2)` → `-1`.

Trace:
- TEST-005
- CON-002

---

### REQ-006: Arithmetic in Temporal Rules

The system SHALL permit arithmetic predicates and expressions to appear in the bodies of rules that also contain temporal literals. Arithmetic expressions MUST NOT reference interval variables (`?T`, `?S`); temporal variables are not numeric.

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

### REQ-007: Arithmetic Predicates are Bodyless — No Rule Labels

The system SHALL ensure that arithmetic predicates (`is`, `=`, `!=`, `<`, `>`, `<=`, `>=`) generate no `RuleLabel`, do not participate in superiority declarations, and do not appear in the conclusions set. It is a parse error to reference an arithmetic predicate in a `(prefer ...)` declaration.

Trace:
- TEST-007

---

### REQ-008: Reserved Arithmetic Keywords

The arithmetic operators (`+`, `-`, `*`, `/`, `div`, `rem`, `abs`, `neg`, `min`, `max`, `**`), the `is` binding predicate, and the comparison predicates (`=`, `!=`, `<`, `>`, `<=`, `>=`) are **reserved keywords**. They cannot be used as user-defined predicate names. This matches the convention of Prolog, Datalog, and ASP, where arithmetic is part of the core language.

Trace:
- TEST-008

---

### REQ-009: No Arithmetic in Rule Heads

The system SHALL emit a parse error if an arithmetic predicate or an arithmetic expression appears in the head position of any rule (fact, strict, defeasible, or defeater). Rule heads must remain ground-symbol predicates. Arithmetic belongs exclusively to the constraint layer (body evaluation).

```lisp
; ILLEGAL — arithmetic predicate in rule head position
(normally r1 some-fact (is ?x 5))

; ILLEGAL — comparison predicate in rule head position
(normally r1 some-fact (> ?x 0))

; ILLEGAL — arithmetic expression as argument in rule head
(given (cost (+ 3 4)))

; LEGAL — is in body position (correct per REQ-004)
(normally r1 (is ?x (+ 1 2)) result)
```

Trace:
- TEST-009

---

## 4. Non-Functional Requirements

### NFR-001: Grounding Performance

Arithmetic evaluation SHALL add no more than 5% overhead to theory grounding time for theories with fewer than 10,000 ground rule instances, measured on a reference machine (4-core, 3 GHz, 8 GB RAM) with no I/O.

Rationale: Arithmetic evaluation is O(expression depth) per substitution. Since expression trees are shallow in practice and numeric evaluation is constant-time, overhead is expected to be negligible. The 5% bound provides slack for interning and memory allocation of `NumericValue` variants.

Trace:
- TEST-NFR-001

---

### NFR-002: Overflow Safety

Arithmetic operations on integer operands SHALL never panic. On overflow, the operation SHALL fail silently (the `is` predicate fails, comparison predicates fail). Overflow detection SHALL use checked arithmetic (`i64::checked_add`, etc.).

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

**Decision**: Arithmetic predicates are evaluated during the **grounding phase**, immediately after term substitution. An `is` or comparison literal in a rule body is resolved to `true` or `false` after all other body literals have been matched to ground facts. If arithmetic fails, the grounding instance is discarded.

**Rationale**:
- The grounding phase already performs substitution lookups; arithmetic evaluation is a natural extension.
- Evaluating at reasoning time would require the reasoning loop to understand types, coupling it to the arithmetic module.
- Evaluating at parse time is not possible for rules with variables.
- This matches Prolog's `is/2` and SWI-Prolog constraint semantics.

**Trade-offs**:
- Arithmetic is not available during abduction (which works backward from goals). Abduction with arithmetic constraints is deferred to a future spec.
- The grounding phase must be extended to distinguish arithmetic literals from regular literals.

**Rejected alternatives**:
- *Reasoning-time evaluation*: Would require modifying the defeasible reasoning loop and indexing machinery — high risk, high coupling.
- *CLP(FD) integration*: Constraint logic programming over finite domains provides stronger guarantees (full constraint propagation) but is significantly more complex and out of scope.

---

### ADR-001b: Arithmetic Constraint Evaluation Order

**Context**: A rule body may contain multiple arithmetic literals (e.g. `(is ?x ...) (> ?x 5)`). The evaluation order determines whether a binding produced by one literal is visible to a later one.

**Decision**: Arithmetic constraints in a rule body are evaluated **strictly in source order** (left to right). After all `BodyLiteral::Logic` literals have been matched against ground facts, the `BodyLiteral::Arithmetic` literals are processed in the order they appear in the source. Each constraint sees the substitution as extended by all preceding constraints in the same body.

**Rationale**: Source-order evaluation is predictable and matches Prolog's `is/2` convention. Users naturally write `(is ?x ...)` before `(> ?x 5)`, expecting the binding to be visible. An unordered or fixpoint model would silently discard such groundings and produce confusing failures.

**Consequence**: The grounding implementation MUST process `BodyLiteral::Arithmetic` items in vector order, threading the substitution forward. This is tested by TEST-004 scenario 1 (chained `is` + comparison).

**Rejected alternative**: *Dependency-graph ordering* — infer order from variable dependencies. More powerful but significantly more complex to implement and reason about.

---

### ADR-002: Extend `Term` Rather than Overloading `SymbolId`

**Context**: Currently all predicate arguments are `SymbolId` (a 4-byte interned string identifier). Numeric values could be encoded as special strings (e.g., `"__num_42"`) or as a new `Term` enum.

**Decision**: Introduce a `Term` enum that wraps either a `SymbolId` or a `NumericValue`:

```rust
pub enum Term {
    Symbol(SymbolId),
    Integer(i64),
    Float(f64),
}
```

Arithmetic expressions (`ArithExpr`) appear only transiently during grounding — they are evaluated to a `Term::Integer` or `Term::Float` before being stored in ground literals.

**Rationale**:
- Encoding numerics as strings would corrupt the string interner with sentinel values and would make comparison O(string parse) rather than O(1).
- A `Term` enum keeps numeric values typed and avoids implicit conversions.
- Ground literals (after grounding) contain only `Term::Symbol` or `Term::Integer`/`Term::Float`. No `ArithExpr` persists past grounding.

**Trade-offs**:
- All code paths that handle `SymbolId` arguments must be updated to handle `Term`. This is a breaking change to `Literal`'s internal representation.
- `Literal` currently stores `predicate_ids: Vec<SymbolId>` — this becomes `predicate_args: Vec<Term>`. The `SymbolId` variant keeps existing literal behaviour unchanged.
- `SmallVec` optimization for predicate args may need re-evaluation; `Term` is 16 bytes vs. 4 bytes for `SymbolId`.

**Rejected alternatives**:
- *String-encoded numerics*: Corrupts the interner; O(string) comparisons; breaks sorted/canonical forms.
- *Separate numeric slot on `Literal`*: Awkward API; does not compose with arbitrary predicate arities.

---

### ADR-003: Polish Notation Arithmetic Expressions in S-expression Parser

**Context**: SPL already uses S-expression syntax (parenthesised prefix lists). Arithmetic expressions must be distinguishable from predicate applications.

**Decision**: Arithmetic operators (`+`, `-`, `*`, `/`, `div`, `rem`, `abs`, `neg`, `min`, `max`, `**`) and built-in predicates (`is`, `=`, `!=`, `<`, `>`, `<=`, `>=`) are **always reserved keywords**. When the parser sees `(+ ...)` or `(* ...)` etc. in a term position (i.e., as a predicate argument), it parses the form as an `ArithExpr` rather than a literal application.

This means:
- `(+ ?x ?y)` in argument position → `ArithExpr::BinOp(Add, Var(?x), Var(?y))`
- `(+ ?x ?y)` in predicate position (as head or rule keyword position) → parse error

The lexer requires targeted additions. The current `parse_atom` in `lexer.rs` accepts `+` but not `*`, `/`, `<`, `>`, `=`, or `!`. These characters must be added to the atom character set so that operator tokens such as `*`, `/`, `<=`, `>=`, `!=`, and `**` are lexed. Adding them to `parse_atom` is sufficient; no dedicated token types are required. The expression dispatcher then gains awareness of which atom values are reserved arithmetic operator keywords and dispatches accordingly. The `+` operator already lexes correctly.

**Trade-offs**:
- Theories that previously used `+`, `-`, `*`, `/`, `<`, `>`, `=`, or `is` as standalone predicate names will break. These names are now reserved, consistent with Prolog and other logic languages. This is considered acceptable — using operator characters as predicate names is rare and confusing. Multi-character atoms containing these characters (e.g., `high-earner`, `c++test`) are unaffected, since reservation applies only to standalone atoms at list head position.

---

---

## 6. Contracts

### CON-001: Numeric Literal Term Grammar

The literal parser accepts numeric terms in all argument positions.

```
numeric-term   ::= integer-literal | float-literal
integer-literal ::= '-'? [0-9]+
float-literal  ::= '-'? [0-9]+ '.' [0-9]+ (('e' | 'E') '-'? [0-9]+)?
```

Parsing produces `Term::Integer(i64)` or `Term::Float(f64)`. Parse errors for out-of-range values use the existing `ParseError` type with source offset.

Implements: REQ-001
Verified by: TEST-001

---

### CON-002: Arithmetic Expression AST and Operators

```rust
pub enum ArithExpr {
    Lit(NumericValue),
    Var(SymbolId),
    BinOp { op: BinArithOp, lhs: Box<ArithExpr>, rhs: Box<ArithExpr> },
    UnaryOp { op: UnaryArithOp, expr: Box<ArithExpr> },
}

pub enum NumericValue {
    Integer(i64),
    Float(f64),
}

pub enum BinArithOp {
    Add,      // +
    Sub,      // -
    Mul,      // *
    Div,      // /   (float division; integer/integer → float)
    IDiv,     // div (integer floor division, rounds toward −∞; non-integer operands → fail)
    Rem,      // rem (floor remainder: a − (a div b)×b; non-integer operands → fail)
    Min,      // min
    Max,      // max
    Pow,      // **
}

pub enum UnaryArithOp {
    Neg,  // neg or unary -
    Abs,  // abs
}
```

**S-expression grammar for arithmetic expressions**:

```
arith-expr     ::= numeric-literal
                 | variable
                 | '(' bin-op arith-expr arith-expr ')'
                 | '(' unary-op arith-expr ')'

bin-op         ::= '+' | '-' | '*' | '/' | 'div' | 'rem' | 'min' | 'max' | '**'
unary-op       ::= 'neg' | 'abs'
```

Examples:
```lisp
(+ ?x ?y)                  ; add
(* 2 ?radius)              ; multiply by constant
(/ (- ?a ?b) 2)            ; (a - b) / 2, result is float
(div ?n 3)                 ; floor division
(abs (- ?x ?y))            ; absolute difference
(** ?base ?exp)            ; exponentiation
```

Implements: REQ-002, REQ-005
Verified by: TEST-002, TEST-005

---

### CON-003: `is` Predicate Interface

```
Syntax:        (is <variable> <arith-expr>)
Position:      Rule body only (parse error in head position)
Pre-conditions:
  - <variable> must be a ?-prefixed variable identifier
  - <arith-expr> must be a valid arithmetic expression (CON-003)
  - <variable> must not already appear as a bound symbol term in the current literal
Post-conditions:
  - If all variables in <arith-expr> are bound in the current substitution:
      <variable> is bound to the evaluated numeric result
  - If any variable in <arith-expr> is unbound: grounding instance is discarded
  - If arithmetic error occurs (division by zero, overflow, type mismatch for div/rem):
      grounding instance is discarded (silent failure)
  - The `is` literal itself does not appear in the conclusions set
Errors (parse time):
  - `(is ...)` with non-variable first argument: parse error
  - `(is ...)` with non-expression second argument: parse error
  - `(is ...)` in a rule head position: parse error
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
      grounding instance is discarded (silent failure)
  - Comparison predicates appear nowhere in the conclusions set
Numeric comparison:
  - Integer compared to Float: integer is promoted to Float before comparison
  - Float compared to Float: IEEE 754 semantics (NaN comparisons always fail)
Errors (parse time):
  - Comparison operator in rule head position: parse error
  - Non-expression argument: parse error
```

Implements: REQ-004
Verified by: TEST-004

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
    Float(f64),
}
```

`Term::Symbol` is the default; existing code paths that destructure `predicate_ids` must be updated to pattern-match on `Term`. Theories without numeric literals will only produce `Term::Symbol`.

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

1. **Parsed rule representation**: `ArithExpr` IS stored in `BodyLiteral::Arithmetic(ArithConstraint)` as part of a parsed `Rule`, which lives in the `Theory`. This is necessary because the same rule is grounded repeatedly for different substitutions, so the expression tree must be retained.

2. **Ground literals**: `ArithExpr` is **never stored** in ground `Literal` instances or the conclusions set. After grounding evaluates an `is` predicate or arithmetic guard, the resulting `Term::Integer` or `Term::Float` is stored in the substitution map, and only ground `Term` values appear in ground literals.

In summary: `ArithExpr` is stored at the rule/theory level (in `BodyLiteral`), but never at the ground literal level. `ArithExpr` does not flow into `Literal::predicate_args` or the reasoning engine's conclusions.

### 7.4 Arithmetic Literal Representation in the Theory

Built-in arithmetic predicates (`is`, `=`, `!=`, `<`, `>`, `<=`, `>=`) are represented in parsed rule bodies as a new variant of a `BodyLiteral` enum (or equivalent type). This separates them from user-defined literals at the type level and makes their special treatment in grounding explicit, without coupling them to the core `Literal` type.

```rust
pub enum BodyLiteral {
    Logic(Literal),                     // Existing literal (look up in theory)
    Arithmetic(ArithConstraint),        // Evaluate numerically; no theory lookup
}

pub enum ArithConstraint {
    Is { var: SymbolId, expr: ArithExpr },
    Compare { op: CmpOp, lhs: ArithExpr, rhs: ArithExpr },
}

pub enum CmpOp { Eq, Ne, Lt, Gt, Le, Ge }
```

---

## 8. SPL Syntax Summary

The following arithmetic forms are valid in rule bodies:

| Form | Meaning | Example |
|---|---|---|
| Numeric literal term | Integer or float constant | `(given (cost item 42))` |
| `(is ?v <expr>)` | Bind `?v` to evaluated `<expr>` | `(is ?tax (* ?price 0.1))` |
| `(= <e1> <e2>)` | Numeric equality guard | `(= ?x 0)` |
| `(!= <e1> <e2>)` | Numeric inequality guard | `(!= ?a ?b)` |
| `(< <e1> <e2>)` | Less-than guard | `(< ?age 18)` |
| `(> <e1> <e2>)` | Greater-than guard | `(> ?salary 50000)` |
| `(<= <e1> <e2>)` | Less-than-or-equal guard | `(<= ?score 100)` |
| `(>= <e1> <e2>)` | Greater-than-or-equal guard | `(>= ?balance 0)` |
| `(+ e1 e2)` | Addition expression | `(+ ?base ?bonus)` |
| `(- e1 e2)` | Subtraction expression | `(- ?total ?discount)` |
| `(* e1 e2)` | Multiplication expression | `(* ?qty ?price)` |
| `(/ e1 e2)` | Float division expression | `(/ ?revenue ?days)` |
| `(div e1 e2)` | Integer floor division | `(div ?total 3)` |
| `(rem e1 e2)` | Integer floor remainder | `(rem ?n 2)` |
| `(** e1 e2)` | Exponentiation | `(** ?base 2)` |
| `(neg e)` | Unary negation | `(neg ?loss)` |
| `(abs e)` | Absolute value | `(abs (- ?a ?b))` |
| `(min e1 e2)` | Minimum | `(min ?x ?y)` |
| `(max e1 e2)` | Maximum | `(max ?x 0)` |

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
  (and (unit-price ?item ?p) (quantity ?order ?q) (is ?subtotal (* ?p ?q)))
  (subtotal ?order ?subtotal))

(normally r2
  (and (subtotal ?order ?s) (tax-rate standard ?r) (is ?tax (* ?s ?r)))
  (tax ?order ?tax))

(normally r3
  (and (subtotal ?order ?s) (tax ?order ?t) (is ?total (+ ?s ?t)))
  (total ?order ?total))
```

Expected conclusions (after grounding and reasoning):

```
+D unit-price(widget, 25)
+D quantity(order-001, 4)
+D tax-rate(standard, 0.08)
+d subtotal(order-001, 100)
+d tax(order-001, 8.0)
+d total(order-001, 108.0)
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
  (and (score ?s ?n) (bonus ?s ?b) (is ?adj (+ ?n ?b)))
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

---

## 10. Test Specifications

### TEST-001: Numeric Literal Terms

**Scenarios**:
1. Integer literal as predicate argument: `(given (count x 42))` → `+D count(x, 42)`
2. Negative integer: `(given (balance x -100))` → `+D balance(x, -100)`
3. Float literal: `(given (rate x 0.15))` → `+D rate(x, 0.15)`
4. Float in scientific notation: `(given (mass x 1.5e3))` → `+D mass(x, 1500.0)`
5. Integer and symbol as separate arguments: `(given (indexed item-a 1))` — parses correctly.
6. Integer that overflows i64 — parse error with source position.

Verifies: REQ-001, CON-001

---

### TEST-002: Arithmetic Expression Parsing

**Scenarios**:
1. `(+ 3 4)` in argument position → `ArithExpr::BinOp(Add, Lit(3), Lit(4))`
2. Nested: `(* (+ ?a ?b) 2)` → parses as nested `ArithExpr`
3. All binary operators parse: `+`, `-`, `*`, `/`, `div`, `rem`, `**`, `min`, `max`
4. Unary operators parse: `(neg ?x)`, `(abs (- ?a ?b))`
5. Arithmetic operator at predicate (non-argument) position → parse error

Verifies: REQ-002, CON-002

---

### TEST-003: `is` Predicate Binding

**Scenarios**:
1. `(is ?z (+ ?x ?y))` with `?x=3, ?y=4` bound → `?z` binds to `7`
2. `(is ?z (+ ?x ?y))` with `?y` unbound → grounding discarded (no conclusion)
3. `(is ?z (/ 10 0))` → division by zero; grounding discarded
4. `(is ?z (** 2 62))` → `z = 4611686018427387904` (within i64)
5. `(is ?z (** 2 63))` → overflow; grounding discarded
6. `is` in rule head position → parse error
7. `is` with non-variable first argument `(is 5 (+ 2 3))` → parse error

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
6. Mixed types: `(> 10 9.5)` → integer promoted to float; retained
7. Comparison in rule head position → parse error
8. Unbound variable in comparison → grounding discarded

Verifies: REQ-004, CON-004

---

### TEST-005: Integer/Float Promotion Rules

**Scenarios**:
1. `(+ 3 4)` → `Integer(7)`
2. `(+ 3 4.0)` → `Float(7.0)`
3. `(/ 10 3)` → `Float(3.3333...)` (integer/integer → float)
4. `(div 10 3)` → `Integer(3)` (floor: ⌊10/3⌋ = 3)
5. `(div -7 2)` → `Integer(-4)` (floor: ⌊-7/2⌋ = -4, not -3)
6. `(rem 10 3)` → `Integer(1)` (floor remainder: 10 − (3×3) = 1)
7. `(rem -7 2)` → `Integer(1)` (floor remainder: -7 − (-4×2) = 1)
8. `(rem 7 -2)` → `Integer(-1)` (floor remainder: 7 − (-4×-2) = -1)
9. `(div 10 3.0)` → evaluation fails (div requires integer operands)
10. `(rem 10 3.0)` → evaluation fails

Verifies: REQ-005, CON-002

---

### TEST-006: Arithmetic in Temporal Rules

**Preconditions**: Temporal reasoning active.

**Scenarios**:
1. Rule with `during` literal and `>` comparison — fires for intervals where comparison holds, does not fire for others (see worked example §9.3).
2. Rule body referencing an interval variable `?T` in arithmetic expression `(+ ?T 1)` → parse error (interval variables not numeric).

Verifies: REQ-006

---

### TEST-007: Arithmetic Predicates Cannot Appear in Superiority

**Scenario**: `(prefer is r1)` → parse error ("arithmetic predicates cannot appear in prefer declarations").

Verifies: REQ-007

---

### TEST-008: Reserved Keyword Enforcement

**Scenarios**:
1. `(normally r1 (+ ?x ?y) result)` where `+` is used as a predicate name → parse error ("'+' is a reserved arithmetic operator")
2. `(normally r1 body (is ?x 5))` where `is` is in head position → parse error
3. `(given (cost (+ 3 4)))` where arithmetic expression appears in fact argument → parse error (arithmetic expression in head/fact position)

Verifies: REQ-008

---

### TEST-009: No Arithmetic in Rule Heads

**Scenarios**:
1. `(normally r1 body (is ?x 5))` → parse error
2. `(normally r1 body (> ?x 0))` → parse error
3. `(normally r1 body (cost item (+ 1 2)))` → parse error (arithmetic expression in head argument)

Verifies: REQ-009

---

### TEST-NFR-001: Grounding Performance Baseline

**Setup**: Generate a theory with 500 facts, 200 defeasible rules with 3-literal bodies including arithmetic `is` and comparison predicates.

**Assertion**: The **grounding phase only** (fact indexing + substitution + arithmetic constraint evaluation, excluding the defeasible reasoning loop) completes in ≤ baseline_grounding_time × 1.05, where baseline_grounding_time is the grounding time for an equivalent non-arithmetic theory with identical predicate structure but without arithmetic predicates. Reasoning phase time is excluded from this measurement, consistent with NFR-001.

Verifies: NFR-001

---

### TEST-NFR-002: Integer Overflow Safety

**Scenarios**:
1. `(is ?z (+ 9223372036854775807 1))` → silent failure, no panic
2. `(is ?z (* 9223372036854775807 2))` → silent failure, no panic
3. `(is ?z (- -9223372036854775808 1))` → silent failure, no panic

Verifies: NFR-002

---

## 11. Implementation Guidance

### Phase 1 — Core Term Type (spindle-core)

1. Add `Term` enum to `crates/spindle-core/src/term.rs` (new file).
2. Add `ArithExpr`, `ArithConstraint`, `BinArithOp`, `UnaryArithOp` to `crates/spindle-core/src/arith.rs` (new file).
3. Add `CmpOp` enum and `ArithConstraint::eval(subst)` method.
4. Update `Literal::predicate_args` from `Vec<SymbolId>` to `Vec<Term>`.
5. Update `Substitution::terms` from `FxHashMap<SymbolId, SymbolId>` to `FxHashMap<SymbolId, Term>`.
6. Add `BodyLiteral` enum to `rule.rs` (or a new `body.rs`), replacing raw `Literal` in `RuleBody`.

> **Risk**: Step 4 and 5 are breaking changes. All match arms on `predicate_ids` and `Substitution::terms` must be audited. The grounding module (`grounding.rs`) is the most impacted.

### Phase 2 — Parser Extension (spindle-parser)

1. Extend `spl/lexer.rs` `parse_atom` to accept `*`, `/`, `<`, `>`, `=`, `!` as atom characters, so operator tokens lex correctly.
2. Extend `spl/literals.rs` to parse numeric literals as `Term::Integer`/`Term::Float`.
3. Add `spl/arith.rs` to parse arithmetic expressions recursively and produce `ArithExpr`.
4. Parse `is` and comparison predicates in body literal position; produce `BodyLiteral::Arithmetic`.
5. Add parse-time guards: arithmetic in head position → parse error; reserved keywords as predicate names → parse error.

### Phase 3 — Grounding Integration (spindle-core)

1. Extend `grounding.rs` `match_literal` to handle `Term::Integer`/`Term::Float` in substitution matching (exact numeric equality).
2. Add arithmetic evaluation in the grounding fixpoint: after matching all `BodyLiteral::Logic` literals, evaluate each `BodyLiteral::Arithmetic` under the current substitution. Discard the grounding instance on failure.
3. Propagate bound `Term::Integer`/`Term::Float` values through `Substitution::apply`.

### Phase 4 — Tests

Write tests in the order of TEST-001 through TEST-009, then NFR tests. Add property-based tests using `proptest` for arithmetic expression evaluation correctness (compare `ArithExpr::eval` against Rust's own arithmetic).

---

## 12. Open Questions

| # | Question | Impact | Proposed Resolution |
|---|---|---|---|
| OQ-1 | Should arithmetic constraints in a body be evaluated strictly in source order to support dependent bindings? | High | **Resolved**: Yes. Arithmetic constraints are evaluated left-to-right in source order. Each constraint sees the substitution as extended by all preceding constraints. See ADR-001b. |
| OQ-2 | Should JSON/contracts represent numeric predicate args as typed numbers or continue string-only serialization? | Medium | **Resolved**: Numeric predicate arguments (`Term::Integer`, `Term::Float`) are serialized as JSON numbers (not strings) in any structured output. |
| OQ-3 | Should `=` be overloaded for both numeric equality and symbol identity? | High | Keep symbol identity via pattern matching in substitution; `=` is numeric-only (reserved keyword) |
| OQ-4 | Should arithmetic predicates produce `+D` conclusions (strict) or `+d` (defeasible)? | High | Neither — they are grounding-phase guards; they produce no conclusions |
| OQ-5 | Should abduction support arithmetic? E.g. "what value of ?x makes `(> ?x 100)` hold?" | Medium | Out of scope for this spec; deferred |
| OQ-6 | Should `why_not` explain arithmetic failures? | Medium | Desirable; deferred — requires arithmetic constraint explanation |
| OQ-7 | Should overflow produce a warning or strict error rather than silent failure? | Low | Silent failure preferred (consistent with Prolog's arithmetic); observable via `why_not` if implemented |
| OQ-8 | Rational numbers (exact fractions)? | Low | Deferred; `f64` covers most use cases; rational would require a dependency |

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
| — | NFR-001 | — | — | TEST-NFR-001 |
| — | NFR-002 | — | — | TEST-NFR-002 |
| — | NFR-003 | — | — | (static analysis) |
