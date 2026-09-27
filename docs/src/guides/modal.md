---
mode: reference
---

# Modal Operators (Deontic Logic)

Use modal operators to describe what someone **must**, **may**, or **must not**
do. These are statements about duties and permissions, not evidence that an
action happened.

## The Three Standard Operators

| SPL | Meaning |
|---|---|
| `(must sign-in)` | There is a duty to sign in: signing in is obligatory. |
| `(may enter-archive)` | There is explicit permission to enter the archive. |
| `(forbidden take-originals)` | Taking originals is prohibited. This means `(must (not take-originals))`. |

`must` means obliged; `may` means permitted. Neither one proves that the action
actually occurred. An obligation does not automatically create a `may` conclusion.

## Running the examples

Each SPL block below is a complete, independent theory. Save one as `example.spl`
and run:

```sh
spindle reason example.spl
```

The accompanying result lists every proved conclusion, including input facts;
ordering may differ. The CLI writes zero-argument atoms in parentheses, so
`(may (enter-archive))` is the same SPL expression as `(may enter-archive)`.
Use `--detailed` to see proof tags and refutations as well.

## Permission overrides a default prohibition

An archive is closed to visitors by default, but a reader with an access pass
has explicit permission to enter.

```spl
(given access-pass)

; () means this default has no conditions.
(normally archive-closed () (forbidden enter-archive))
(normally pass-allows-entry access-pass (may enter-archive))
(prefer pass-allows-entry archive-closed)
```

Expected proved conclusions:

```text
(access-pass)
(may (enter-archive))
```

Both rules apply. `prefer` makes the permission rule stronger, so the prohibition
is defeated. Permission does not prove `(enter-archive)` itself.

Try changing just one line:

| Change | Result, apart from the input fact |
|---|---|
| Remove `(given access-pass)` | `(forbidden (enter-archive))` is proved. |
| Remove the `prefer` line | Neither permission nor prohibition is proved: the defaults block each other. |
| Reverse it to `(prefer archive-closed pass-allows-entry)` | The prohibition is proved instead of the permission. |

An unconditional body is `()`; no extra `true` fact is needed.
Preferences resolve competing defaults, but cannot override facts or strict conclusions.

## Obligations do not report completed actions

An archive visitor must sign in and must not take original documents away.

```spl
(given visitor)
(normally visitor-sign-in visitor (must sign-in))
(normally originals-stay visitor (forbidden take-originals))
```

Expected proved conclusions:

```text
(visitor)
(must (sign-in))
(forbidden (take-originals))
```

This establishes the two duties. It establishes neither `(sign-in)` nor
`(not (take-originals))` as a fact about what the visitor actually did.

## SPL Negation

**Not obliged to act** and **obliged not to act** mean different things:

| Expression | Meaning |
|---|---|
| `(not (must attend-briefing))` | The obligation to attend is explicitly negated. |
| `(must (not attend-briefing))` | There is an obligation not to attend. |
| `(forbidden attend-briefing)` | The same prohibition as the preceding row. |

For example, staff normally must attend a briefing, but staff on leave are exempt:

```spl
(given staff)
(given on-leave)
(normally staff-briefing staff (must attend-briefing))
(normally leave-exemption on-leave (not (must attend-briefing)))
(prefer leave-exemption staff-briefing)
```

Expected proved conclusions:

```text
(staff)
(on-leave)
(not (must (attend-briefing)))
```

The exemption removes the duty; it does not forbid attendance or grant an
explicit permission. Similarly, `(not (may p))` negates a permission assertion;
it is not another spelling of `(forbidden p)`.

## Using a modal conclusion as a premise

A duty can be a condition of another rule. Here a visitor has an outstanding
sign-in requirement because there is explicit evidence they have not signed in.

```spl
(given visitor)
(given (not signed-in))
(normally require-sign-in visitor (must sign-in))
(normally outstanding-sign-in
  (and (must sign-in) (not signed-in))
  sign-in-outstanding)
```

Expected proved conclusions:

```text
(visitor)
(not (signed-in))
(must (sign-in))
(sign-in-outstanding)
```

Removing `(given (not signed-in))` removes the outstanding conclusion. A missing
`signed-in` fact is not enough: ordinary negative premises need explicit negative
evidence.

## Combining with Predicates and Variables

Use arguments to keep different people's duties separate:

```spl
(given (visitor alice))
(given (visitor bob))
(normally individual-sign-in (visitor ?person) (must (sign-in ?person)))
```

Expected proved conclusions:

```text
(visitor alice)
(visitor bob)
(must (sign-in alice))
(must (sign-in bob))
```

The variable is bound by each visitor fact. Each resulting obligation concerns
that particular person.

## Weak permission is not explicit permission

A rule may check that a prohibition has been **refuted**. This is called weak
permission. It does not create the explicit permission expressed by `may`.

In this independent example, photography is prohibited during conservation work.
We explicitly know that no conservation work is taking place:

```spl
(given (not conservation-work))
(normally protect-documents conservation-work (forbidden photograph))
(normally no-prohibition-check (not (forbidden photograph)) photography-not-prohibited)
```

Expected proved conclusions:

```text
(not (conservation-work))
(photography-not-prohibited)
```

The prohibition rule cannot apply, so the engine can refute the prohibition.
The negative modal premise consumes that refutation. There is no rule granting
`(may photograph)`, so that explicit permission is not proved.

This special treatment of negative **modal** premises differs from ordinary
negative premises such as `(not signed-in)` above. An undecided prohibition is
not a refuted prohibition; merely failing to see it in the default output does
not establish weak permission. `--detailed` exposes the distinction with `-d`
for a defeasible refutation.

## Defeaters block a duty without asserting its opposite

A disputed copying fee can block a duty to pay. A defeater uses `except` and
never proves its own head:

```spl
(given copying-requested)
(given fee-disputed)
(normally copying-fee copying-requested (must pay-fee))
(except dispute-blocks-fee fee-disputed (forbidden pay-fee))
(prefer dispute-blocks-fee copying-fee)
```

Expected proved conclusions:

```text
(copying-requested)
(fee-disputed)
```

Neither `(must pay-fee)` nor `(forbidden pay-fee)` is proved. The defeater
blocks the obligation without imposing a prohibition. This example does not
grant permission either. Under the strong-permission profile, an
obligation-headed defeater cannot block an explicit permission.

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
In a rule body, `(not (forbidden p))` can consume that negative proof.

See the worked weak-permission example above.

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

## Modal Negation and Complements

The Rust display format uses the traditional modal notation below. CLI reasoning
text and JSON `literal_spl` use the SPL expressions shown in the examples.

| SPL expression | Rust display |
|---|---|
| `(must pay)` | `[O]pay` |
| `(not (must pay))` | `[-O]pay` |
| `(must (not pay))` | `[O]~pay` |
| `(may pay)` | `[P]pay` |
| `(forbidden pay)` | `[F]pay` |

`Mode::complement()` toggles outer modal negation. `Literal::complement()`
flips the inner literal negation instead; use `Literal::outer_negation()`
for expression scope.

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

## Limitations

1. **CLI syntax**: Modal operators use SPL wrappers in theory files. The CLI reasons over these literals without a separate modal flag.
2. **No automatic permission from obligation**: The built-in conflict relationships do not derive `[P]a` from `[O]a`.
3. **Custom modes are uninterpreted**: Custom modes created with `Mode::new(name)` have no built-in semantics. The theory’s rules entirely determine their meaning.
4. **No modal logic tableau**: Spindle performs defeasible reasoning, not modal logic model checking. The strong-permission profile defines modal opposition without Kripke-style accessibility relations.


SPL provides the three standard modal wrappers. `(K ?x)` is an ordinary
predicate named `K`, not a custom modal wrapper. Custom modes can be constructed
through the Rust API with `Mode::new("K")`.
