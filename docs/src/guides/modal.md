# Modal Operators (Deontic Logic)

Spindle supports modal operators for deontic reasoning, allowing you to express obligations, permissions, and prohibitions within defeasible logic theories.

## Introduction

Deontic logic is a branch of formal logic concerned with normative concepts such as obligation, permission, and prohibition. In defeasible reasoning, deontic modalities are particularly useful because normative rules are often subject to exceptions. For example, a general obligation to pay taxes may be defeated by a specific exemption for non-profit organizations.

Spindle integrates deontic modalities directly into its literal representation. Each literal can carry a modal operator that qualifies the proposition with a normative meaning, and these modal literals participate in the same defeasible reasoning process as ordinary literals -- including conflict resolution, superiority, and defeat.

> **Note:** Modal operators are currently only accessible via the Rust API and WASM bindings. They are not directly supported through the CLI. If you need modal reasoning, use the library or WASM integration.

## The Three Standard Operators

Spindle provides three built-in deontic operators:

| Operator | Display | SPL Syntax | Meaning |
|----------|---------|------------|---------|
| Obligation | `[O]` | `(must ...)` | The proposition **must** hold |
| Permission | `[P]` | `(may ...)` | The proposition **may** hold |
| Forbidden | `[F]` | `(forbidden ...)` | The proposition **must not** hold |

### Obligation (`must`)

An obligation states that something is required. If `(must pay)` is concluded, then there is an obligation to pay.

### Permission (`may`)

A permission states that something is allowed. If `(may access)` is concluded, then access is permitted.

### Forbidden (`forbidden`)

A prohibition states that something is not allowed. If `(forbidden enter)` is concluded, then entering is forbidden.

## SPL Syntax

Modal operators use keyword wrappers:

```lisp
; Obligation: (must <literal>)
(given (must pay))
(normally r1 signed-contract (must pay))

; Permission: (may <literal>)
(given (may access))
(normally r2 member (may access))

; Forbidden: (forbidden <literal>)
(given (forbidden enter))
(normally r3 unauthorized (forbidden enter))
```

### SPL Negation

Negation of modal literals in SPL uses the `not` wrapper:

```lisp
; Negated obligation: not obligated to pay
(normally r4 exemption (not (must pay)))

; Negated permission: not permitted to access
(normally r5 revoked (not (may access)))

; Negated prohibition: not forbidden to enter (i.e., allowed)
(normally r6 authorized (not (forbidden enter)))
```

## Using Modal Operators in Rules

Modal operators can appear in both the body and head of rules, in all rule types.

### Examples

```lisp
; If you signed a contract, you are obligated to pay
(normally r1 signed-contract (must pay))

; If obligated to pay and haven't paid, violation
(normally r2 (and (must pay) (not paid)) violation)

; Members may access resources
(normally r3 member (may access))

; Unauthorized users are forbidden from entering
(normally r4 unauthorized (forbidden enter))

; An exemption defeats the obligation to pay
(normally r5 exemption (not (must pay)))
(prefer r5 r1)

; A defeater: pending review blocks the permission
(except d1 pending-review (may access))
```

### Combining with Predicates and Variables

In SPL, modal operators can be combined with predicates and variables:

```lisp
; Employees are obligated to report hours
(given (employee alice))
(given (employee bob))
(normally r1 (employee ?x) (must (report-hours ?x)))

; Managers may approve expenses
(given (manager alice))
(normally r2 (manager ?x) (may (approve-expenses ?x)))

; Contractors are forbidden from accessing internal systems
(given (contractor charlie))
(normally r3 (contractor ?x) (forbidden (access-internal ?x)))
```

## Modal Negation and Complements

Every modal operator has a complement, obtained by toggling the negation flag. The complement of `[O]` (obligation) is `[-O]` (no obligation). This is distinct from the negation of the underlying proposition.

| Expression | Meaning |
|------------|---------|
| `[O]pay` | There is an obligation to pay |
| `[-O]pay` | There is no obligation to pay |
| `[O]~pay` | There is an obligation not to pay |
| `[-O]~pay` | There is no obligation not to pay |

The distinction between modal negation and literal negation is important:

- **Modal negation** (`[-O]pay`): The obligation itself does not hold. You are not required to pay, but you may still choose to.
- **Literal negation** (`[O]~pay`): The obligation holds, but over the negated proposition. You are obligated not to pay.

### Display Format

| Mode | Display | Negated Display |
|------|---------|-----------------|
| Obligation | `[O]` | `[-O]` |
| Permission | `[P]` | `[-P]` |
| Forbidden | `[F]` | `[-F]` |
| Custom `X` | `[X]` | `[-X]` |
| Empty | *(nothing)* | *(nothing)* |

## Rust API Usage

The `Mode` struct is in `spindle_core::mode` and is re-exported through the prelude.

### Creating Modes

```rust
use spindle_core::prelude::*;

// Standard deontic operators
let obligation = Mode::obligation();  // [O]
let permission = Mode::permission();  // [P]
let forbidden = Mode::forbidden();    // [F]

// Empty mode (no modal operator)
let empty = Mode::empty();

// Custom mode
let custom = Mode::new("K");  // [K] (e.g., epistemic "known")
```

### Complement (Negation Toggle)

```rust
use spindle_core::prelude::*;

let obligation = Mode::obligation();
assert_eq!(format!("{}", obligation), "[O]");

let neg_obligation = obligation.complement();
assert_eq!(format!("{}", neg_obligation), "[-O]");

// Double complement returns to original
let double = neg_obligation.complement();
assert_eq!(format!("{}", double), "[O]");
```

### Checking Mode State

```rust
use spindle_core::prelude::*;

let mode = Mode::obligation();
assert!(!mode.is_empty());
assert!(!mode.negation);
assert_eq!(mode.name, Some("O".to_string()));

let empty = Mode::empty();
assert!(empty.is_empty());
```

### Creating Modal Literals

```rust
use spindle_core::prelude::*;

// A literal with an obligation mode: [O]pay
let must_pay = Literal::new(
    "pay",
    false,
    Mode::obligation(),
    Temporal::empty(),
    vec![],
);
assert!(must_pay.is_modal());
assert_eq!(format!("{}", must_pay), "[O]pay");

// A negated literal with permission mode: [P]~access
let not_access = Literal::new(
    "access",
    true,
    Mode::permission(),
    Temporal::empty(),
    vec![],
);
assert_eq!(format!("{}", not_access), "[P]~access");

// Modal literal with predicates: [F]enter(restricted_area)
let forbidden_enter = Literal::new(
    "enter",
    false,
    Mode::forbidden(),
    Temporal::empty(),
    vec!["restricted_area".to_string()],
);
assert_eq!(format!("{}", forbidden_enter), "[F]enter(restricted_area)");
```

### Modal Literals in Rules

```rust
use spindle_core::prelude::*;
use smallvec::smallvec;

// (normally r1 signed-contract (must pay))
let body = smallvec![Literal::simple("signed_contract")];
let head = smallvec![Literal::new(
    "pay",
    false,
    Mode::obligation(),
    Temporal::empty(),
    vec![],
)];
let rule = Rule::new("r1", RuleType::Defeasible, body, head);

// (normally r2 (and (must pay) (not paid)) violation)
let body = smallvec![
    Literal::new("pay", false, Mode::obligation(), Temporal::empty(), vec![]),
    Literal::negated("paid"),
];
let head = smallvec![Literal::simple("violation")];
let rule = Rule::new("r2", RuleType::Defeasible, body, head);
```

### Equality and Hashing

Modal operators participate in literal equality and hashing. Two literals with the same name but different modes are considered distinct:

```rust
use spindle_core::prelude::*;
use std::collections::HashSet;

let pay = Literal::simple("pay");
let must_pay = Literal::new(
    "pay",
    false,
    Mode::obligation(),
    Temporal::empty(),
    vec![],
);

// These are different literals
assert_ne!(pay, must_pay);

let mut set = HashSet::new();
set.insert(pay.literal_id());
// must_pay has a different hash due to the mode
```

## Use Cases

### Compliance Rules

Model regulatory compliance where obligations can be defeated by exceptions:

```lisp
; All companies must file annual reports
(normally r1 company (must file-annual-report))

; Small companies are exempt from detailed reporting
(normally r2 (and company small-company) (not (must file-annual-report)))
(prefer r2 r1)

; Public companies must disclose finances
(normally r3 public-company (must disclose-finances))

; Companies in bankruptcy are exempt from disclosure
(normally r4 (and public-company in-bankruptcy) (not (must disclose-finances)))
(prefer r4 r3)
```

### Permission Systems

Model access control with defeasible permissions:

```lisp
; Employees may access the office
(normally r1 employee (may access-office))

; Suspended employees lose access
(normally r2 (and employee suspended) (not (may access-office)))
(prefer r2 r1)

; Managers may access restricted areas
(normally r3 manager (may access-restricted))

; Even managers are forbidden from the server room without clearance
(normally r4 (and manager (not has-clearance)) (forbidden access-server-room))
```

### Obligation Tracking

Track obligations through chains of reasoning:

```lisp
; Signing a contract creates an obligation to pay
(normally r1 signed-contract (must pay))

; Obligation to pay and failure to pay results in violation
(normally r2 (and (must pay) (not paid)) in-violation)

; Being in violation creates an obligation to remedy
(normally r3 in-violation (must remedy))

; Payment within grace period removes the violation
(normally r4 (and in-violation paid-within-grace) (not in-violation))
(prefer r4 r2)
```

### Mixed Normative Reasoning

Combine obligations, permissions, and prohibitions in a single theory:

```lisp
; Citizens must pay taxes
(normally r1 citizen (must pay-taxes))

; Citizens may vote
(normally r2 citizen (may vote))

; Convicted felons are forbidden from voting (in some jurisdictions)
(normally r3 (and citizen convicted-felon) (forbidden vote))
(prefer r3 r2)

; Minors are exempt from taxation
(normally r4 (and citizen minor) (not (must pay-taxes)))
(prefer r4 r1)

; Non-citizens are forbidden from voting
(normally r5 (not citizen) (forbidden vote))
```

## Limitations

1. **No CLI support**: Modal operators cannot be specified through the command-line interface. Use the Rust API or WASM bindings to construct modal literals.
2. **No inter-modal axioms**: Spindle does not enforce relationships between operators (e.g., it does not automatically derive `[P]a` from `[O]a`). If you need such relationships, encode them as explicit rules.
3. **Custom modes are uninterpreted**: Custom modes created with `Mode::new(name)` have no built-in semantics. Their meaning is determined entirely by the rules you write.
4. **No modal logic tableau**: Spindle performs defeasible reasoning, not modal logic model checking. Modal operators are labels on literals, not Kripke-style accessibility relations.

## Best Practices

1. **Be explicit about modal relationships**
   ```lisp
   ; If something is obligatory, it is also permitted
   (always obligation-implies-permission (must ?x) (may ?x))
   ```

2. **Use superiority to handle conflicts between norms**
   ```lisp
   (normally r1 employee (must attend-meeting))
   (normally r2 (and employee on-leave) (not (must attend-meeting)))
   (prefer r2 r1)
   ```

3. **Separate normative and factual reasoning**
   ```lisp
   ; Factual rules
   (normally r1 penguin bird)
   (normally r2 bird flies)

   ; Normative rules
   (normally r3 (and endangered-species (may hunt)) (not (may hunt)))
   ```

4. **Document the intended interpretation of custom modes**
   ```lisp
   ; [K] = epistemic "known to be true"
   ; Use metadata to document the mode's meaning
   (meta r1 (description "Agents know their own obligations"))
   (normally r1 (must ?x) (K ?x))
   ```
