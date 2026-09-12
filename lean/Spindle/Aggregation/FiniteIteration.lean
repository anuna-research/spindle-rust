import SpindleLean.Properties.Termination

namespace Spindle.Aggregation.FiniteIteration

variable {α : Type}

def close (step : List α → List α) (xs : List α) : Nat → List α
  | 0 => xs
  | n + 1 => if (step xs).length == xs.length then xs else close step (step xs) n

private theorem append_fixed (step : List α → List α)
    (grows : ∀ xs, ∃ added, step xs = xs ++ added)
    (xs : List α) (same : (step xs).length = xs.length) : step xs = xs := by
  obtain ⟨added, h⟩ := grows xs
  rw [h, List.length_append] at same
  have : added = [] := List.eq_nil_of_length_eq_zero (by omega)
  simp [h, this]

theorem fixed [DecidableEq α] (step : List α → List α) (univ : List α)
    (grows : ∀ xs, ∃ added, step xs = xs ++ added)
    (preserves : ∀ xs, xs.Nodup → xs ⊆ univ →
      (step xs).Nodup ∧ step xs ⊆ univ)
    (xs : List α) (nodup : xs.Nodup) (subset : xs ⊆ univ)
    (fuel : Nat) (enough : univ.length < xs.length + fuel) :
    step (close step xs fuel) = close step xs fuel := by
  induction fuel generalizing xs with
  | zero =>
    have hsub : xs.toFinset ⊆ univ.toFinset := by
      intro x hx
      exact List.mem_toFinset.mpr (subset (List.mem_toFinset.mp hx))
    have le := Finset.card_le_card hsub
    rw [List.toFinset_card_of_nodup nodup] at le
    have bound := List.toFinset_card_le univ
    omega
  | succ n ih =>
    simp only [close]
    split
    next h => exact append_fixed step grows xs (by simpa using h)
    next h =>
      have facts := preserves xs nodup subset
      apply ih _ facts.1 facts.2
      obtain ⟨added, addedEq⟩ := grows xs
      have ne : (step xs).length ≠ xs.length := by simpa using h
      have size : xs.length < (step xs).length := by rw [addedEq, List.length_append] at *; omega
      omega

end Spindle.Aggregation.FiniteIteration

namespace Spindle.Aggregation.FiniteIteration
variable {α : Type}

def run (step : List α → List α) (xs : List α) : Nat → List α
  | 0 => xs
  | n + 1 => run step (step xs) n

theorem run_fixed (step : List α → List α) (xs : List α)
    (fixed : step xs = xs) (n : Nat) : run step xs n = xs := by
  induction n with
  | zero => rfl
  | succ n ih => simpa only [run, fixed] using ih

theorem close_eq_run (step : List α → List α)
    (grows : ∀ xs, ∃ added, step xs = xs ++ added) (xs : List α) (n : Nat) :
    close step xs n = run step xs n := by
  induction n generalizing xs with
  | zero => rfl
  | succ n ih =>
    simp only [close, run]
    split
    next h =>
      have hf := append_fixed step grows xs (by simpa using h)
      rw [hf, run_fixed step xs hf n]
    next h => exact ih (step xs)

theorem run_add (step : List α → List α) (xs : List α) (m n : Nat) :
    run step xs (m + n) = run step (run step xs m) n := by
  induction m generalizing xs with
  | zero => simp only [Nat.zero_add, run]
  | succ m ih => simpa only [Nat.succ_add, run] using ih (step xs)

theorem close_eq_run_more (step : List α → List α)
    (grows : ∀ xs, ∃ added, step xs = xs ++ added) (xs : List α) (n extra : Nat)
    (hf : step (close step xs n) = close step xs n) :
    run step xs (n + extra) = close step xs n := by
  rw [run_add, ← close_eq_run step grows xs n, run_fixed _ _ hf]

theorem run_rel (step other : List α → List α) (rel : List α → List α → Prop)
    (preserves : ∀ xs ys, rel xs ys → rel (step xs) (other ys))
    (xs ys : List α) (initial : rel xs ys) (n : Nat) :
    rel (run step xs n) (run other ys n) := by
  induction n generalizing xs ys with
  | zero => exact initial
  | succ n ih => exact ih _ _ (preserves xs ys initial)

end Spindle.Aggregation.FiniteIteration
