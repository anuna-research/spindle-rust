import Mathlib.Data.List.Perm.Basic
import Mathlib.Data.List.Dedup

/-!
# Aggregation over a finite, completed relation

The reducer laws are obligations of the extension, not assumptions about machine
arithmetic. In particular, floating-point addition and checked integer addition
do not implement this total algebra without a separate refinement argument.

Rows have set semantics; extracted values have multiset semantics. Two different
shifts paying the same amount contribute twice. Duplicate proofs of one shift do
not. Matching includes the outer environment's grouping constraints.
-/
namespace Spindle.Aggregation

structure Reducer (α : Type) where
  combine : α → α → α
  associative : ∀ a b c, combine (combine a b) c = combine a (combine b c)
  commutative : ∀ a b, combine a b = combine b a

variable {α Row : Type}

/-- `none` means no value has been collected yet, implementing `required`.
An explicit seed is included once, including on nonempty input. -/
def Reducer.step (r : Reducer α) (acc value : Option α) : Option α :=
  match acc, value with
  | none, x => x
  | x, none => x
  | some a, some b => some (r.combine a b)

theorem Reducer.step_right_comm (r : Reducer α) (a b c : Option α) :
    r.step (r.step a b) c = r.step (r.step a c) b := by
  cases a with
  | none => cases b <;> cases c <;> simp [step, r.commutative]
  | some a =>
    cases b with
    | none => cases c <;> rfl
    | some b =>
      cases c with
      | none => rfl
      | some c =>
        simp only [step, r.associative]
        rw [r.commutative b c]

/-- Fold successful extracted values. `none` on empty input is failure;
`some identity` on empty input returns the declared identity. -/
def Reducer.eval (r : Reducer α) (seed : Option α) (values : List α) : Option α :=
  (values.map some).foldl r.step seed

theorem Reducer.eval_permutation (r : Reducer α) (seed : Option α)
    {xs ys : List α} (h : xs.Perm ys) : r.eval seed xs = r.eval seed ys := by
  exact (h.map some).foldl_eq' (fun a _ b _ acc => r.step_right_comm acc a b) seed

@[simp] theorem Reducer.eval_empty (r : Reducer α) (seed : Option α) :
    r.eval seed [] = seed := rfl

@[simp] theorem Reducer.eval_required_singleton (r : Reducer α) (value : α) :
    r.eval none [value] = some value := rfl

theorem Reducer.eval_seeded_ne_none (r : Reducer α) (seed : α) (xs : List α) :
    r.eval (some seed) xs ≠ none := by
  induction xs generalizing seed with
  | nil => simp [eval]
  | cons x xs ih => exact ih (r.combine seed x)

theorem Reducer.required_fails_iff_empty (r : Reducer α) (xs : List α) :
    r.eval none xs = none ↔ xs = [] := by
  cases xs with
  | nil => simp [eval]
  | cons x xs =>
    have nonempty := r.eval_seeded_ne_none x xs
    simpa [eval, step] using nonempty

/-- Deduplicate whole rows before extracting their contributions. -/
def values [DecidableEq Row] (rows : List Row) (select : Row → Bool)
    (extract : Row → α) : List α :=
  (rows.dedup.filter select).map extract

theorem mem_values [DecidableEq Row] (rows : List Row) (select : Row → Bool)
    (extract : Row → α) (value : α) :
    value ∈ values rows select extract ↔
      ∃ row ∈ rows, select row = true ∧ extract row = value := by
  simp [values, List.mem_filter, and_assoc]

theorem aggregate_permutation [DecidableEq Row] (r : Reducer α) (seed : Option α)
    (select : Row → Bool) (extract : Row → α)
    {xs ys : List Row} (h : xs.Perm ys) :
    r.eval seed (values xs select extract) =
      r.eval seed (values ys select extract) := by
  exact r.eval_permutation seed ((h.dedup.filter select).map extract)

/-- Exact, unbounded integer sum is a concrete inhabitant of the contract. -/
def sum : Reducer Int := ⟨(· + ·), Int.add_assoc, Int.add_comm⟩

def minimum : Reducer Int := ⟨min, Int.min_assoc, Int.min_comm⟩
def maximum : Reducer Int := ⟨max, Int.max_assoc, Int.max_comm⟩

-- Equal amounts on different rows count twice; duplicate rows count once.
example : sum.eval (some 0) (values [(1, 25), (2, 25), (1, 25)]
    (fun _ => true) (fun row : Nat × Int => row.2)) = some 50 := by decide

example : sum.eval (some 0) (values [(1, 25), (2, 30)]
    (fun row : Nat × Int => row.1 == 1) Prod.snd) = some 25 := by decide

example : sum.eval none [] = none := rfl
example : sum.eval (some 0) [] = some 0 := rfl

example : minimum.eval none [30, 25, 40] = some 25 := by decide
example : maximum.eval none [30, 25, 40] = some 40 := by decide
example : sum.eval (some 0) (values [1, 2, 1] (fun _ : Nat => true)
    (fun _ => (1 : Int))) = some 2 := by decide

end Spindle.Aggregation
