/-
  SpindleLean.Family
  Temporal-family reasoning: the SPEC-020 semantics for ground temporal
  theories, as implemented by the projection engine and family-aware
  indexing (`crates/spindle-core/src/projection.rs`, `index.rs`).

  Semantics established by direct probing of the engine
  (`crates/spindle-core/tests/family_probe.rs`) and SPEC-020's contracts:

  1. **Exact identity**: a literal is (name, negation, optional window).
     `p[1,10]`, `p[20,30]`, and `p` are three distinct atoms.
  2. **Exact conflict**: the complement flips negation and keeps the
     window. `~p[1,10]` attacks `p[1,10]` and nothing else — disjoint,
     overlapping, and atemporal complements do NOT conflict (probes
     P5–P8, P10; SPEC-020 "distinct temporal defeaters remain distinct
     attackers", "existing atemporal defeater does not suppress temporal
     attacker").
  3. **Family support**: an ATEMPORAL body literal is satisfied by any
     provable literal with the same name and negation, whatever its
     window (probe P1); a TEMPORAL body literal requires an exact match —
     neither an atemporal member nor a different window satisfies it
     (probes P2–P4). Family support applies uniformly: definite phase,
     defeasible phase, and attacker/defeater bodies (probes P11, P12).

  The closure algorithms below mirror `SpindleLean/Closure/*` exactly,
  with body satisfaction replaced by the family-aware `famSat`. The
  non-temporal parity theorem (`famDeltaClose_eq_exact` etc.) shows the
  family machinery collapses to exact satisfaction on window-free
  theories — SPEC-020's "non-temporal theories remain semantically
  unchanged", at the model level.
-/
import SpindleLean.Basic
import Mathlib.Data.List.Dedup

namespace Family

/-- A ground temporal window. -/
structure Window where
  start : Int
  stop : Int
  deriving DecidableEq, Repr, BEq

/-- A family literal: exact identity is (name, negation, mode, window).
    `window = none` is the atemporal base literal; `mode = none` is
    non-modal. Modes are identity-bearing (they are part of the engine's
    `FamilyId`). -/
structure FLit where
  name : String
  negated : Bool
  mode : Option String
  window : Option Window
  deriving DecidableEq, Repr, BEq

instance : LawfulBEq Window where
  eq_of_beq {a b} h := by
    cases a; cases b
    have h' : (_ == _ && _ == _) = true := h
    rw [Bool.and_eq_true] at h'
    obtain ⟨h1, h2⟩ := h'
    obtain rfl := LawfulBEq.eq_of_beq h1
    obtain rfl := LawfulBEq.eq_of_beq h2
    rfl
  rfl {a} := by
    cases a
    show (_ == _ && _ == _) = true
    rw [Bool.and_eq_true]
    exact ⟨beq_self_eq_true _, beq_self_eq_true _⟩

instance : LawfulBEq FLit where
  eq_of_beq {a b} h := by
    cases a; cases b
    have h' : (_ == _ && (_ == _ && (_ == _ && _ == _))) = true := h
    rw [Bool.and_eq_true, Bool.and_eq_true, Bool.and_eq_true] at h'
    obtain ⟨h1, h2, h3, h4⟩ := h'
    obtain rfl := LawfulBEq.eq_of_beq h1
    obtain rfl := LawfulBEq.eq_of_beq h2
    obtain rfl := LawfulBEq.eq_of_beq h3
    obtain rfl := LawfulBEq.eq_of_beq h4
    rfl
  rfl {a} := by
    cases a
    show (_ == _ && (_ == _ && (_ == _ && _ == _))) = true
    rw [Bool.and_eq_true, Bool.and_eq_true, Bool.and_eq_true]
    exact ⟨beq_self_eq_true _, beq_self_eq_true _, beq_self_eq_true _,
      beq_self_eq_true _⟩

namespace FLit

/-- Exact complement: flip negation, keep the window (probe P5–P8:
    conflict never crosses windows). -/
def complement (l : FLit) : FLit := { l with negated := !l.negated }

theorem complement_involutive (l : FLit) : l.complement.complement = l := by
  simp [complement, Bool.not_not]

/-- Same family: same name, negation, and mode — any window (mirrors the
    engine's `FamilyId`, which includes the mode). -/
def sameFamily (a b : FLit) : Bool :=
  a.name == b.name && a.negated == b.negated && a.mode == b.mode

/-- Family-aware satisfaction of one body literal by a proven set:
    exact membership always suffices; an atemporal body literal is also
    satisfied by any proven family member (probes P1–P4). -/
def famSat (proven : List FLit) (l : FLit) : Bool :=
  proven.contains l
    || (l.window == none && proven.any (fun m => sameFamily m l))

/-- Exact satisfaction: plain membership. -/
def exactSat (proven : List FLit) (l : FLit) : Bool :=
  proven.contains l

/-- Exact satisfaction implies family satisfaction. -/
theorem famSat_of_exactSat (proven : List FLit) (l : FLit)
    (h : exactSat proven l = true) : famSat proven l = true := by
  simp only [famSat, exactSat] at *
  rw [h]; rfl

/-- **Family-support characterization**: famSat holds iff the literal is
    exactly proven, or it is atemporal and some family member is proven. -/
theorem famSat_iff (proven : List FLit) (l : FLit) :
    famSat proven l = true ↔
      l ∈ proven
      ∨ (l.window = none ∧ ∃ m ∈ proven, sameFamily m l = true) := by
  simp only [famSat, Bool.or_eq_true, Bool.and_eq_true, List.any_eq_true,
    List.contains_iff_mem, beq_iff_eq]

/-- On an atemporal proven set, an atemporal literal's family clause
    collapses to exact membership: a family witness with the same name,
    negation, and (absent) window IS the literal. -/
theorem famSat_eq_exactSat_of_atemporal (proven : List FLit) (l : FLit)
    (hproven : ∀ m ∈ proven, m.window = none) :
    famSat proven l = exactSat proven l := by
  simp only [famSat, exactSat]
  cases hc : proven.contains l
  · simp only [Bool.false_or]
    cases hw : (l.window == none)
    · rfl
    · simp only [Bool.true_and]
      cases ha : proven.any (fun m => sameFamily m l)
      · rfl
      · exfalso
        simp only [List.any_eq_true] at ha
        obtain ⟨m, hm, hfam⟩ := ha
        simp only [sameFamily, Bool.and_eq_true, beq_iff_eq] at hfam
        have hwm := hproven m hm
        have hlw : l.window = none := beq_iff_eq.mp hw
        have hml : m = l := by
          cases m; cases l
          simp_all
        rw [hml] at hm
        rw [List.contains_iff_mem.mpr hm] at hc
        exact Bool.noConfusion hc
  · rfl

end FLit

/-! ## Family rules and theories -/

/-- Rule types mirror the SDL model. -/
inductive FRuleType where
  | fact | strict | defeasible | defeater
  deriving DecidableEq, Repr, BEq

structure FRule where
  label : String
  ruleType : FRuleType
  body : List FLit
  head : FLit
  deriving DecidableEq, Repr, BEq

namespace FRule

def isFact (r : FRule) : Bool := r.ruleType == .fact

def isProductive (r : FRule) : Bool :=
  r.ruleType == .fact || r.ruleType == .strict || r.ruleType == .defeasible

def isDefinite (r : FRule) : Bool :=
  r.ruleType == .fact || r.ruleType == .strict

/-- Family-aware body satisfaction (probes P1–P4, P11, P12). -/
def bodySat (r : FRule) (proven : List FLit) : Bool :=
  r.body.all (FLit.famSat proven)

/-- Exact body satisfaction, for the parity theorem. -/
def bodySatExact (r : FRule) (proven : List FLit) : Bool :=
  r.body.all (FLit.exactSat proven)

end FRule

structure FTheory where
  rules : List FRule
  superiority : List (String × String)

namespace FTheory

def isSuperior (t : FTheory) (winner loser : String) : Bool :=
  t.superiority.any (fun (w, l) => w == winner && l == loser)

/-- Rules with an EXACT head literal (conflict is exact — probe P5–P8). -/
def rulesWithHead (t : FTheory) (lit : FLit) : List FRule :=
  t.rules.filter (fun r => r.head == lit)

def allLiterals (t : FTheory) : List FLit :=
  ((t.rules.map (fun r => r.head)) ++ t.rules.flatMap (fun r => r.body)).dedup

/-- A theory is atemporal when no literal anywhere carries a window. -/
def Atemporal (t : FTheory) : Prop :=
  ∀ r ∈ t.rules, r.head.window = none ∧ ∀ l ∈ r.body, l.window = none

end FTheory

/-! ## Family closures — mirrors Closure/Delta, Lambda, Partial with
     famSat in place of exact body satisfaction. Parametrized by the
     satisfaction function so the non-temporal parity theorem can compare
     family and exact instantiations of the SAME algorithm. -/

section Closures

variable (sat : FRule → List FLit → Bool)

/-- One step of the definite closure. -/
def deltaStepWith (t : FTheory) (current : List FLit) : List FLit :=
  (current ++ t.rules.filterMap fun r =>
    if r.isDefinite && sat r current && !current.contains r.head then
      some r.head
    else none).dedup

/-- Definite closure to fixpoint. -/
def deltaCloseWith (t : FTheory) (fuel : Nat := 1000) : List FLit :=
  go ((t.rules.filter (fun r => r.ruleType == .fact)).map (fun r => r.head)).dedup fuel
where
  go (current : List FLit) : Nat → List FLit
    | 0 => current
    | n + 1 =>
      let next := deltaStepWith sat t current
      if next.length == current.length then current
      else go next n

/-- One step of the lambda over-approximation. -/
def lambdaStepWith (t : FTheory) (delta current : List FLit) : List FLit :=
  (current ++ t.rules.filterMap fun r =>
    if r.isProductive && sat r current
       && !current.contains r.head
       && !delta.contains r.head.complement then
      some r.head
    else none).dedup

def lambdaCloseWith (t : FTheory) (delta : List FLit) (fuel : Nat := 1000) :
    List FLit :=
  go delta fuel
where
  go (current : List FLit) : Nat → List FLit
    | 0 => current
    | n + 1 =>
      let next := lambdaStepWith sat t delta current
      if next.length == current.length then current
      else go next n

/-- Attack reachability: the attacker's body must be satisfiable in
    lambda (family-aware — probe P12). -/
def attackReachesWith (lambda : List FLit) (attacker : FRule) : Bool :=
  sat attacker lambda

/-- Team defeat: a superior productive defender with satisfied body. -/
def teamDefeatsWith (t : FTheory) (lit : FLit) (attacker : FRule)
    (partial_ : List FLit) : Bool :=
  (t.rulesWithHead lit).any fun defender =>
    defender.isProductive
    && sat defender partial_
    && t.isSuperior defender.label attacker.label

def allAttacksDefeatedWith (t : FTheory) (lit : FLit)
    (lambda partial_ : List FLit) : Bool :=
  (t.rulesWithHead lit.complement).all fun attacker =>
    attacker.isFact
    || !attackReachesWith sat lambda attacker
    || teamDefeatsWith sat t lit attacker partial_

/-- Family canProve, with the delta-consistency gate (condition (2))
    checked first, exactly as in the exact model. The gate is EXACT:
    only the same-window complement blocks (probe P10). -/
def canProveWith (t : FTheory) (lit : FLit)
    (delta lambda partial_ : List FLit) : Bool :=
  if delta.contains lit.complement then false
  else if delta.contains lit then true
  else
    let hasSupport := (t.rulesWithHead lit).any fun r =>
      r.isProductive && sat r partial_
    hasSupport && allAttacksDefeatedWith sat t lit lambda partial_

def gatedDeltaF (delta : List FLit) : List FLit :=
  delta.filter fun l => !delta.contains l.complement

def partialStepWith (t : FTheory) (delta lambda current : List FLit) :
    List FLit :=
  let candidates := lambda.filter fun lit =>
    !current.contains lit && canProveWith sat t lit delta lambda current
  (current ++ candidates).dedup

def partialCloseWith (t : FTheory) (delta lambda : List FLit)
    (fuel : Nat := 1000) : List FLit :=
  go (gatedDeltaF delta) fuel
where
  go (current : List FLit) : Nat → List FLit
    | 0 => current
    | n + 1 =>
      let next := partialStepWith sat t delta lambda current
      if next.length == current.length then current
      else go next n

end Closures

/-- The family reasoning pipeline (the verified model of the projection
    engine's ground-temporal behavior). -/
def famReason (t : FTheory) :
    List FLit × List FLit × List FLit :=
  let delta := deltaCloseWith FRule.bodySat t
  let lambda := lambdaCloseWith FRule.bodySat t delta
  let partial_ := partialCloseWith FRule.bodySat t delta lambda
  (delta, lambda, partial_)

/-! ## Non-temporal parity -/

/-- On an atemporal theory, family body satisfaction coincides with exact
    body satisfaction whenever the proven set is drawn from the theory's
    (atemporal) literals. -/
theorem bodySat_eq_exact_of_atemporal (r : FRule) (proven : List FLit)
    (hproven : ∀ m ∈ proven, m.window = none) :
    r.bodySat proven = r.bodySatExact proven := by
  unfold FRule.bodySat FRule.bodySatExact
  generalize r.body = bs
  induction bs with
  | nil => rfl
  | cons x xs ih =>
    simp only [List.all_cons, ih,
      FLit.famSat_eq_exactSat_of_atemporal proven x hproven]

/-- Every literal in a step of the family delta closure over an atemporal
    theory is atemporal, provided the current set is. -/
theorem deltaStepWith_atemporal (sat : FRule → List FLit → Bool)
    (t : FTheory) (ht : t.Atemporal) (current : List FLit)
    (hcur : ∀ m ∈ current, m.window = none) :
    ∀ m ∈ deltaStepWith sat t current, m.window = none := by
  intro m hm
  simp only [deltaStepWith, List.mem_dedup, List.mem_append] at hm
  rcases hm with h | h
  · exact hcur m h
  · simp only [List.mem_filterMap] at h
    obtain ⟨r, hr, hcond⟩ := h
    split at hcond
    · cases hcond
      exact (ht r hr).1
    · cases hcond

/-- **Non-temporal parity (delta step)**: on atemporal current sets over
    an atemporal theory, the family-aware delta step equals the exact
    delta step — the family machinery is invisible without windows.
    (SPEC-020: non-temporal theories remain semantically unchanged.) -/
theorem deltaStepWith_parity (t : FTheory) (current : List FLit)
    (hcur : ∀ m ∈ current, m.window = none) :
    deltaStepWith FRule.bodySat t current
      = deltaStepWith FRule.bodySatExact t current := by
  unfold deltaStepWith
  have hfun :
      (fun r : FRule =>
        if r.isDefinite && r.bodySat current && !current.contains r.head then
          some r.head
        else none)
      = (fun r : FRule =>
        if r.isDefinite && r.bodySatExact current && !current.contains r.head then
          some r.head
        else none) := by
    funext r
    rw [bodySat_eq_exact_of_atemporal r current hcur]
  rw [hfun]

end Family
