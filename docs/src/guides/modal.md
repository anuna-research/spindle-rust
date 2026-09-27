---
mode: reference
---

# Modal Operators (Deontic Logic)

Spindle supports modal operators for deontic reasoning, allowing you to express obligations, permissions, and prohibitions within defeasible logic theories.

## Introduction

Deontic logic is a branch of formal logic concerned with normative concepts such as obligation, permission, and prohibition. In defeasible reasoning, deontic modalities are particularly useful because normative rules are often subject to exceptions. For example, a general obligation to pay taxes can be defeated by a specific exemption for non-profit organizations.

Spindle integrates deontic modalities directly into its literal representation. Each literal can carry a modal operator that qualifies the proposition with a normative meaning. Modal literals undergo ordinary defeasible reasoning, including conflict resolution, superiority, and defeat.

> **CLI support:** SPL theories accept `(must ...)`, `(may ...)`, and `(forbidden ...)` wrappers through the CLI.
> The Rust API and WASM bindings also provide modal reasoning.

## The Three Standard Operators

Spindle provides three built-in deontic operators:

| Operator | Display | SPL Syntax | Meaning |
|----------|---------|------------|---------|
| Obligation | `[O]` | `(must ...)` | An obligation applies to the proposition |
| Permission | `[P]` | `(may ...)` | Permission applies to the proposition |
| Forbidden | `[F]` | `(forbidden ...)` | A prohibition applies to the proposition |

### Obligation (`must`)

An obligation states a normative duty. `(must pay)` means **obliged to pay**.
Obligation is already represented by `must`; `may` expresses permission.

### Permission (`may`)

A permission is explicit, strong permission. Refuting a prohibition does not produce a `(may ...)` conclusion.

### Forbidden (`forbidden`)

A prohibition is an obligation not to act: `(forbidden enter)` and `(must (not enter))` have the same reasoning identity.

## Strong-permission profile

Spindle uses the **single-head SDL strong-permission profile** by default.
The conflict and team-defense rules follow Definitions 8–11 of
[Computing Strong and Weak Permissions in Defeasible Logic](https://arxiv.org/pdf/1212.0079).
This is a named profile, not a claim about every SPINdle configuration.

For any plain literal `p`, including an inner-negated predicate:

| Goal | Supporting heads | Opposing heads |
|---|---|---|
| `(must p)` | `(must p)` | `(must (not p))`, `(may (not p))`, `(not (must p))` |
| `(may p)` | `(may p)` | `(must (not p))`, `(not (may p))` |

Opposite permissions can coexist. Modal conclusions do not establish the plain action.
An obligation also does not automatically establish strong permission.

An applicable superior rule can defeat an attacker.
An obligation attacker can be countered by same-direction obligation, permission, or defeater rules.
A permission or defeater attacker against an obligation requires a superior obligation rule, not another permission or defeater.
A defender need not itself win its own conflict; its body must be applicable.
Without a preference, competing defaults block each other.
The negative proof conditions mirror these checks; an unresolved cycle stays undecided.

```spl
(normally no-play () (forbidden play))
(normally hat-play hat (may play))
(prefer hat-play no-play)
(given hat)
```

The default text view lists `(hat)` and `(may (play))` under `Proved:`.
With `--detailed`, the permission appears as `+d (may (play))` and the
refuted prohibition as `-d (must (not (play)))`. The tables below use the
traditional `[O]`/`[P]` notation to explain the internal representation;
CLI reasoning text and JSON `literal_spl` use SPL syntax.
Reversing the preference derives the prohibition instead.
Removing the preference blocks both defaults; removing `hat` leaves the prohibition applicable.
`()` is an unconditional body and needs no `true` fact.

### Scope and extensions

The implementation uses Spindle's existing strict/defeasible rules, typed terms, and exact temporal opposition.
Only identical argument values and identical temporal windows compete.
An atemporal premise can still match a temporal family member.
Facts and strict conclusions retain `+D` and imply `+d`; preferences cannot override them.
Inconsistent modal facts remain inconsistent rather than being silently repaired.

The paper's ordered obligation/reparation and permission chains are outside this profile.
Spindle additionally permits explicit outer-negated modal facts and rule heads.
Typed defeaters never directly establish their head. Under Definition 10(2.3),
an obligation-headed defeater can block a contrary obligation but cannot block
an explicit permission. Only an obligation rule can attack that permission in
the paper's O/P fragment. Explicit outer-negated heads are a separate extension:
they oppose the same unnegated modal assertion using ordinary superiority.
Nested modal operators are rejected instead of discarding the inner operator.
Custom mode names retain their existing behavior.

### Weak permission and negated premises

Weak permission for `p` means a constructive `-d (forbidden p)` proof.
It is not a `+d (may p)` proof and is not inferred from missing output.
In a rule body, `(not (forbidden p))` can consume that negative proof:

```spl
(normally check (not (forbidden play)) weakly-allowed)
```

An explicit proof of the outer-negated expression can also satisfy that premise.
For an atemporal negative premise, every indexed positive family member must be refuted.
A live or undecided temporal member prevents that inference.
Variable arguments in a refutation premise need bindings from preceding positive premises.
Strict rules using refutation become defeasibly applicable; refutation does not create a definite premise.

`(not (must p))` negates the obligation; `(must (not p))` obliges the opposite action.
Likewise, refuting `(may p)` does not establish `(may (not p))`.
A query for an outer-negated assertion searches for that assertion, not a negative proof tag.
Inspect negative conclusions to distinguish refutation from unknown status.
Explanations retain modal opponents and equivalent prohibition premises.
Negative modal premises appear as `defeasible_refutation` evidence leaves in explanation trees, preserving their `-d` meaning.

### Compatibility

This profile takes effect automatically; there is no per-theory legacy switch.
Existing theories change if they relied on independent `F` and `O` modes,
noncompeting permission/prohibition rules, conflicting opposite permissions, or flattened modal negation.
Negative conclusions can use canonical `[O]~p` instead of `[F]p` and include implicit modal opponents.
Explicit positive conclusions retain their supporting rule's spelling.
Consumers should compare semantic identity rather than rendered strings.
Structured JSON uses `must`, `may`, and `forbidden` for standard `mode.name`
values. Update consumers that previously matched `O`, `P`, or `F`.
`mode.negation: true` means outer negation, not a prohibition; the inner
`negated` flag remains separate.
Explanation consumers must accept the new `DefeasibleRefutation` enum variant (`defeasible_refutation` in JSON).
Conflict diagnostics use `ConflictKind::ModalOpposition` and no longer flag opposite permissions as contradictory.
The Rust `Literal` structural equality API retains spelling distinctions;
`FamilyId`, indexing, grounding and queries normalize the prohibition alias.
`Literal::complement()` still flips inner negation; use `outer_negation()` for expression scope and `opponents()` for conflicts.

Review these theories before upgrading. Pin the earlier binary to restore earlier interpretation.
The Lean modal model proves modal laws and finite closure properties.
Its oracle compares all four tags against Rust; this is not a Rust refinement proof.
See the [modal proof reference](https://git.anuna.io/anuna-research/spindle-rust/src/branch/main/lean/MODAL.md)
for the exact guarantees and limitations, and
[verification commands](check-verification.md) to run the proofs and oracle comparisons.

## SPL Syntax

Modal operators use keyword wrappers:

```spl
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

```spl
; Negated obligation: not obligated to pay
(normally r4 exemption (not (must pay)))

; Negated permission: not permitted to access
(normally r5 revoked (not (may access)))

; Negated prohibition: explicit absence of prohibition, not strong permission
(normally r6 authorized (not (forbidden enter)))
```

## Using Modal Operators in Rules

Modal operators can appear in both the body and head of rules, in all rule types.

### Examples

```spl
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

; A prohibition rule: pending review overrides the permission
(normally r6 pending-review (forbidden access))
(prefer r6 r3)

; A defeater can block an obligation without establishing a prohibition
(except d1 payment-disputed (forbidden pay))
(prefer d1 r1)
```

### Combining with Predicates and Variables

In SPL, modal operators can be combined with predicates and variables:

```spl
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

Toggling the negation flag produces the complement of a modal operator. The complement of `[O]` (obligation) is `[-O]` (explicit negation of obligation). This is distinct from the negation of the underlying proposition.

| Expression | Meaning |
|------------|---------|
| `[O]pay` | There is an obligation to pay |
| `[-O]pay` | An explicit assertion negating the obligation to pay |
| `[O]~pay` | There is an obligation not to pay |
| `[-O]~pay` | An explicit assertion negating the obligation not to pay |

The distinction between modal negation and literal negation is important:

- **Modal negation** (`[-O]pay`): An explicit assertion negates the obligation to pay. It neither grants strong permission nor excludes an obligation not to pay.
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

Modal operators participate in literal equality and hashing. Spindle treats literals with the same name but different modes as distinct:

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

Regulatory compliance rules express obligations subject to exceptions:

```spl
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

Access control rules express defeasible permissions:

```spl
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

Chains of reasoning track obligations:

```spl
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

A single theory combines obligations, permissions, and prohibitions:

```spl
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

1. **CLI syntax**: Modal operators use SPL wrappers in theory files. The CLI reasons over these literals without a separate modal flag.
2. **No automatic permission from obligation**: The built-in conflict relationships do not derive `[P]a` from `[O]a`.
3. **Custom modes are uninterpreted**: Custom modes created with `Mode::new(name)` have no built-in semantics. The theory’s rules entirely determine their meaning.
4. **No modal logic tableau**: Spindle performs defeasible reasoning, not modal logic model checking. The strong-permission profile defines modal opposition without Kripke-style accessibility relations.

## Modal Rule Patterns

1. **Explicit modal relationships**
   ```spl
   ; If something is obligatory, it is also permitted
   (always obligation-implies-permission (must ?x) (may ?x))
   ```

2. **Superiority resolves conflicts between norms**
   ```spl
   (normally r1 employee (must attend-meeting))
   (normally r2 (and employee on-leave) (not (must attend-meeting)))
   (prefer r2 r1)
   ```

3. **Separate normative and factual rules**
   ```spl
   ; Factual rules
   (normally r1 penguin bird)
   (normally r2 bird flies)

   ; Normative rules
   (normally r3 (and endangered-species (may hunt)) (not (may hunt)))
   ```

4. **Metadata records the intended interpretation of custom modes**
   ```spl
   ; [K] = epistemic "known to be true"
   ; Use metadata to document the mode's meaning
   (meta r1 (description "Agents know their own obligations"))
   (normally r1 (must ?x) (K ?x))
   ```
