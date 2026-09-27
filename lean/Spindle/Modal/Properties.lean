import Spindle.Modal.Reason

namespace Spindle.Modal

theorem normalize_idempotent (l : Lit) : l.normalize.normalize = l.normalize := by
  rcases l with ⟨atom, mode, inner, outer, window⟩
  cases mode <;> rfl

theorem forbidden_is_obligation_not (atom : String) (inner outer : Bool)
    (window : Option (Int × Int)) :
    (Lit.mk atom .forbidden inner outer window).normalize =
      Lit.mk atom .obligation (!inner) outer window := rfl

theorem negate_involutive (l : Lit) : l.negate.negate = l := by
  rcases l with ⟨atom, mode, inner, outer, window⟩
  cases mode <;> simp [Lit.negate]

theorem opposition_symmetric (a b : Lit) : opposes a b = opposes b a := by
  rcases a with ⟨aa, am, ai, ao, aw⟩
  rcases b with ⟨ba, bm, bi, bo, bw⟩
  cases am <;> cases bm <;> cases ai <;> cases bi <;> cases ao <;> cases bo <;>
    simp [opposes, conflict, Lit.normalize, eq_comm, and_comm]

theorem opposite_permissions_compatible (atom : String) (inner : Bool)
    (window : Option (Int × Int)) :
    opposes ⟨atom, .permission, inner, false, window⟩
      ⟨atom, .permission, !inner, false, window⟩ = false := by
  cases inner <;> simp [opposes, conflict, Lit.normalize]

theorem permission_opposes_prohibition (atom : String) (inner : Bool)
    (window : Option (Int × Int)) :
    opposes ⟨atom, .permission, inner, false, window⟩
      ⟨atom, .forbidden, inner, false, window⟩ = true := by
  cases inner <;> simp [opposes, conflict, Lit.normalize]

theorem obligations_conflict (atom : String) (inner : Bool)
    (window : Option (Int × Int)) :
    opposes ⟨atom, .obligation, inner, false, window⟩
      ⟨atom, .obligation, !inner, false, window⟩ = true := by
  cases inner <;> simp [opposes, conflict, Lit.normalize]

theorem outer_negation_distinct (atom : String) (inner : Bool)
    (window : Option (Int × Int)) :
    Lit.mk atom .obligation inner true window ≠
      Lit.mk atom .obligation (!inner) false window := by
  intro eq
  have := congrArg Lit.outer eq
  cases this

/-- A permission defender cannot defeat an opposite permission attacker. -/
theorem permission_cannot_defend_against_permission
    (atom : String) (inner : Bool) (window : Option (Int × Int))
    (a d : String) (bodyA bodyD : List Lit) :
    defends ⟨atom, .obligation, inner, false, window⟩
      ⟨a, .defeasible, bodyA, ⟨atom, .permission, !inner, false, window⟩⟩
      ⟨d, .defeasible, bodyD, ⟨atom, .permission, inner, false, window⟩⟩ = false := by
  cases inner <;> simp [defends, conflict, MRule.productive]

/-- Refuting a prohibition alone cannot invent explicit strong permission. -/
theorem no_support_no_strong_permission (t : MTheory) (s : State) (l : Lit)
    (noDefinite : has s .definitelyProvable l = false)
    (noSupport : (heads t l).all (fun r => !r.productive) = true) :
    infer t s .defeasiblyProvable l = false := by
  have absent : ((heads t l).any fun r => r.productive && applicable t s .defeasiblyProvable r) = false := by
    apply List.any_eq_false.mpr
    intro r member
    have h := List.all_eq_true.mp noSupport r member
    cases hp : r.productive <;> simp_all
  simp [infer, noDefinite, absent]

/-- Inductive derivability from exactly the modal inference conditions.
Only finite-domain conclusions may enter a derivation. -/
inductive Derivable (t : MTheory) : ConclusionType × Lit → Prop where
  | rule (s : State) (tag : ConclusionType) (l : Lit)
      (premises : ∀ pair ∈ s, Derivable t pair)
      (domain : (tag, l) ∈ univ t)
      (enabled : infer t s tag l = true) : Derivable t (tag, l)

theorem step_sound (t : MTheory) (s : State)
    (sound : ∀ pair ∈ s, Derivable t pair) :
    ∀ pair ∈ step t s, Derivable t pair := by
  intro pair member
  rcases List.mem_append.mp member with old | fresh
  · exact sound pair old
  · obtain ⟨domain, enabled⟩ := List.mem_filter.mp fresh
    simp only [Bool.and_eq_true] at enabled
    exact Derivable.rule s pair.1 pair.2 sound domain enabled.2

theorem close_sound (t : MTheory) : ∀ pair ∈ close t, Derivable t pair := by
  have loop : ∀ (fuel : Nat) (s : State), (∀ pair ∈ s, Derivable t pair) →
      ∀ pair ∈ Spindle.Aggregation.FiniteIteration.close (step t) s fuel, Derivable t pair := by
    intro fuel
    induction fuel with
    | zero => intro s sound; exact sound
    | succ n ih =>
      intro s sound
      simp only [Spindle.Aggregation.FiniteIteration.close]
      split
      · exact sound
      · exact ih _ (step_sound t s sound)
  exact loop _ [] (by simp)

/-- In the completed run, absent support or defeaters alone never produce a
positive conclusion. In particular, weak permission cannot create strong permission. -/
theorem defeater_only_no_positive (t : MTheory) (l : Lit)
    (onlyDefeaters : ∀ r ∈ heads t l, r.kind = .defeater) :
    (.definitelyProvable, l) ∉ close t ∧ (.defeasiblyProvable, l) ∉ close t := by
  have impossible : ∀ pair, Derivable t pair → pair.2 = l →
      (pair.1 = .definitelyProvable ∨ pair.1 = .defeasiblyProvable) → False := by
    intro pair proof
    induction proof with
    | rule s tag q premises domain enabled ih =>
      intro same positive
      simp only at same positive
      subst q
      rcases positive with definite | defeasible
      · subst tag
        obtain ⟨r, member, h⟩ := List.any_eq_true.mp enabled
        simp [onlyDefeaters r member] at h
      · subst tag
        simp only [infer, Bool.or_eq_true] at enabled
        rcases enabled with definite | supported
        · exact ih _ (by simpa [has] using definite) rfl (Or.inl rfl)
        · simp only [Bool.and_eq_true] at supported
          have support := supported.1.2
          obtain ⟨r, member, h⟩ := List.any_eq_true.mp support
          simp [MRule.productive, onlyDefeaters r member] at h
  exact ⟨fun h => impossible _ (close_sound t _ h) rfl (Or.inl rfl),
    fun h => impossible _ (close_sound t _ h) rfl (Or.inr rfl)⟩

/-- Public-entry soundness includes prohibition normalization. -/
theorem reason_sound (t : MTheory) : ∀ pair ∈ reason t, Derivable t.normalize pair :=
  close_sound t.normalize

theorem reason_fixedpoint (t : MTheory) : step t.normalize (reason t) = reason t :=
  close_fixedpoint t.normalize

theorem reason_no_productive_support (t : MTheory) (l : Lit)
    (onlyDefeaters : ∀ r ∈ heads t.normalize l.normalize, r.kind = .defeater) :
    (.definitelyProvable, l.normalize) ∉ reason t ∧
      (.defeasiblyProvable, l.normalize) ∉ reason t :=
  defeater_only_no_positive t.normalize l.normalize onlyDefeaters

end Spindle.Modal
