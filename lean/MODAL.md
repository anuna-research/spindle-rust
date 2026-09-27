---
mode: reference
---

# Modal verification

The `Spindle.Modal` library formalizes the single-head strong-permission profile
used by issue #44. `must` means obligation; `may` means explicit permission;
`forbidden p` normalizes to `must (not p)`.

## Model boundary

`Basic.lean` separates inner negation from outer modal negation. Its atom names
are opaque ground identities. Windows are absent or finite integer pairs.
Opposition requires identical atoms and windows. Atemporal premises use family
matching, including universal refutation of a family for negative premises.

`Reason.lean` defines the four constructive inference conditions independently
of Rust's event queue. It includes facts, strict rules, defeasible rules,
defeaters, directly declared superiority, and typed team defense.
Negative modal premises accept explicit negative assertions or constructive
refutation of the positive modality. Missing positive evidence is insufficient.

`reason` normalizes every head and premise before executing the reference
closure. The lower-level `conflict`, `defends`, `infer`, and `close` functions
operate on canonical theories. Their algebraic theorems quantify over that
defined inference system; use `t.normalize` when applying them to source theories.

The finite tag domain is derived from mentioned atoms and their modal
opponents. Each iteration only adds previously absent tags. The closure budget
comes from the domain size, with a proof that it reaches a fixed point.

## Checked results

| Result | Guarantee |
|---|---|
| `normalize_idempotent`, `forbidden_is_obligation_not` | Prohibition is a stable obligation alias |
| `negate_involutive`, `outer_negation_distinct` | Expression negation preserves modal scope |
| `opposition_symmetric` | Either side recognizes the same conflict |
| `opposite_permissions_compatible` | Opposite permissions do not attack each other |
| `permission_opposes_prohibition`, `obligations_conflict` | Permission/prohibition and opposite-obligation conflicts exist |
| `permission_cannot_defend_against_permission` | A permission cannot defend an obligation against an opposite permission |
| `close_fixedpoint` | The finite reference closure terminates at a fixed point |
| `close_complete` | Every enabled inference over the finite domain is included at completion |
| `close_sound` | Every returned tag has a finite derivation under the modal inference rules |
| `defeater_only_no_positive` | Absent productive support, neither definite nor defeasible positive proof can appear in the completed closure |

The `reason_sound`, `reason_fixedpoint`, and `reason_no_productive_support`
theorems expose these guarantees at the normalized source-theory entry point.

The soundness target is the inductive `Derivable` relation defined from the
independent modal inference conditions. Saturation is a fixed-point property;
it is not a theorem of equivalence with the Rust implementation.

`Examples.lean` additionally checks concrete completed runs in the kernel:
preferred permission defeats prohibition, opposite permissions both hold,
and weak permission does not manufacture strong permission. These witnesses
use `decide`, not `native_decide`. The axiom audit includes these results.

## Rust comparison

`ModalOracle` is a JSONL adapter around `reason`. Each request supplies rules,
direct priorities, and queries; each response reports `+D`, `-D`, `+d`, and `-d`
as Boolean flags. An undecided tag remains absent. The adapter requires unique
rule labels, avoiding the different duplicate-label behavior of Rust maps and
Lean lists.

The Rust suite compares modal rule pairs across rule kinds and preference
directions. It also covers team defense, negative premises, cycles, direct
versus transitive priorities, temporal families, prohibition aliases, and
malformed oracle requests. It compares canonical exact identities, retaining
temporal windows and both negation scopes.

```sh
(cd lean && lake build ModalOracle)
cargo test -p spindle-core --test lean_modal_oracle_difftest -- --ignored --nocapture
scripts/check-lean-verification.sh
```

Both CI configurations build the oracle and run this differential target.
The proof gate rejects admitted proofs, local axioms, unapproved axiom
dependencies, and vacuous declarations.

## Limits

These are proofs about the Lean model, with executable comparison evidence
against Rust. They are not a refinement proof of the Rust worklist, parser,
grounder, explanation renderer, or trust layer. The oracle adapter is test
infrastructure and is not a verified recognizer.

The model does not formalize nested modalities, custom modes, ordered
reparation/permission chains, arithmetic, aggregation, or variable grounding.
It does not prove global coherence or normative consistency for arbitrary
theories. In particular, opposing definite facts remain possible.
The older nonmodal proof modules retain their own stated semantics and
hypotheses; the modal extension does not silently reinterpret them.
