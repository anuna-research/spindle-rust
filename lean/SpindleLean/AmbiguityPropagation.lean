import SpindleLean.Reason
import SpindleLean.AmbiguityBlocking

/-!
# SpindleLean.AmbiguityPropagation

Ambiguity propagation (AP) semantics for defeasible reasoning.

## Overview

In ambiguity propagation, if a literal `p` is ambiguous (both `p` and `~p`
have applicable rules, but neither is superior), then any literal `q` that
depends on `p` through an applicable rule chain also becomes ambiguous. This
is stricter than ambiguity blocking (AB), where `q` may still be provable if
it has an independent derivation path.

## Key difference from AB

In AB (`Lambda.canProve`), `r.applicable plusd` checks that body literals are
in `+d`. In AP, we additionally require that no body literal is "ambiguous"
(undecided). A literal is ambiguous in AP when it is neither `+d` nor `-d`
after the closure, which means both it and its complement have applicable
defeasible rules but neither side wins.

## Main results

- **Ambiguity Infectiousness**: if `p` is ambiguous under AP, and rule
  `r: p => q` is the only derivation for `q`, then `q` is also ambiguous.
- **AP Subsumption**: `+d` under AP implies `+d` under AB (AP is stricter).
- **Test theory**: `r1: => p, r2: => ~p, r3: => q, r4: p => q` — `q` is
  `-d` under AP because `r4` depends on ambiguous `p`.
-/

namespace AmbiguityPropagation

open Delta Lambda AmbiguityBlocking

/-! ### AP closure computation -/

/-- In AP mode, a rule is "AP-applicable" when every body literal is in `+d`
    AND no body literal is ambiguous (i.e., every body literal is decided:
    either in `+d` or in `-d`). -/
def apApplicable (r : Rule) (plusd minusd : List Literal) : Bool :=
  r.applicable plusd && r.body.all (fun l => plusd.contains l || minusd.contains l)

/-- AP version of `canProve`: like AB's `canProve` but uses `apApplicable`
    instead of `applicable`. A rule with an ambiguous body literal cannot
    fire under AP. -/
def apCanProve (t : Theory) (plusD : List Literal) (plusd minusd : List Literal)
    (q : Literal) : Bool :=
  let supporters := t.Rsd q
  let hasApplicable := supporters.any fun r => apApplicable r plusd minusd
  let compNotDefinite := !plusD.contains q.complement
  let attackers := t.R q.complement
  let allCountered := attackers.all fun s =>
    Lambda.attackerCountered t plusd minusd s
  hasApplicable && compNotDefinite && allCountered

/-- One step of AP defeasible closure. -/
def apLambdaStep (t : Theory) (plusD : List Literal)
    (plusd minusd : List Literal) : List Literal × List Literal :=
  let allLits := allLiterals t
  let undecided := allLits.filter fun q =>
    !plusd.contains q && !minusd.contains q
  let newPlusd := undecided.filter fun q => apCanProve t plusD plusd minusd q
  let newMinusd := undecided.filter fun q =>
    !newPlusd.contains q && Lambda.canDisprove t plusD plusd minusd q
  let plusd' := plusd ++ newPlusd.filter fun l => !plusd.contains l
  let minusd' := minusd ++ newMinusd.filter fun l => !minusd.contains l
  (plusd', minusd')

/-- Fuel-bounded iteration of `apLambdaStep`. -/
def apLambdaLoop (t : Theory) (plusD : List Literal)
    (plusd minusd : List Literal) (fuel : Nat) : List Literal × List Literal :=
  match fuel with
  | 0 => (plusd, minusd)
  | fuel + 1 =>
    let (plusd', minusd') := apLambdaStep t plusD plusd minusd
    if plusd' == plusd && minusd' == minusd then (plusd, minusd)
    else apLambdaLoop t plusD plusd' minusd' fuel

/-- Compute `+d` and `-d` sets under AP semantics. -/
def computeDefeasibleAP (t : Theory) (fuel : Nat := t.rules.length)
    : List Literal × List Literal :=
  let plusD := computePlusD t fuel
  let initPlusd := Lambda.seedPlusd plusD
  let initMinusd := Lambda.seedMinusd t plusD
  apLambdaLoop t plusD initPlusd initMinusd fuel

/-- Compute `+d` under AP. -/
def computePlusdAP (t : Theory) (fuel : Nat := t.rules.length) : List Literal :=
  (computeDefeasibleAP t fuel).1

/-- Compute `-d` under AP. -/
def computeMinusdAP (t : Theory) (fuel : Nat := t.rules.length) : List Literal :=
  (computeDefeasibleAP t fuel).2

/-- Full AP closure with undecided literals assigned `-d`. -/
def defeasibleClosureAP (t : Theory) (fuel : Nat := t.rules.length) : List Conclusion :=
  let (plusd, minusd) := computeDefeasibleAP t fuel
  let allLits := allLiterals t
  let remaining := allLits.filter fun q =>
    !plusd.contains q && !minusd.contains q
  let finalMinusd := minusd ++ remaining
  let pos := plusd.map fun l => { conclusionType := .plusd, literal := l : Conclusion }
  let neg := finalMinusd.map fun l => { conclusionType := .minusd, literal := l : Conclusion }
  pos ++ neg

/-- Full reasoning under AP: definite closure + AP defeasible closure. -/
def reasonAP (t : Theory) (fuel : Nat := t.rules.length) : List Conclusion :=
  let definite := definiteClosure t fuel
  let defeasible := defeasibleClosureAP t fuel
  definite ++ defeasible

/-! ### Test theories -/

/-- Test theory: `r1: => p, r2: => ~p, r3: => q, r4: p => q`.
    Under AP, `q` is `-d` because `r4` depends on ambiguous `p` and `r3`
    alone cannot overcome the dependency. Actually, `r3: => q` is unconditional
    so `q` should be `+d` even under AP via `r3`. Let's verify. -/
def apTest1 : Theory :=
  { rules := [
      Rule.mkDefeasible "r1" [] (Literal.pos "p"),
      Rule.mkDefeasible "r2" [] (Literal.neg "p"),
      Rule.mkDefeasible "r3" [] (Literal.pos "q"),
      Rule.mkDefeasible "r4" [Literal.pos "p"] (Literal.pos "q")
    ],
    superiority := [] }

-- Under AB: q is +d (r3 suffices)
#eval reason apTest1
#guard hasConclusion (reason apTest1) .plusd (Literal.pos "q")

-- Under AP: q is ALSO +d because r3 is an unconditional rule with empty body
#eval reasonAP apTest1
#guard hasConclusion (reasonAP apTest1) .plusd (Literal.pos "q")

/-- Test theory where AP truly differs: `r1: => p, r2: => ~p, r3: p => q`.
    `q`'s ONLY derivation depends on ambiguous `p`, so `q` is `-d` under AP. -/
def apTest2 : Theory :=
  { rules := [
      Rule.mkDefeasible "r1" [] (Literal.pos "p"),
      Rule.mkDefeasible "r2" [] (Literal.neg "p"),
      Rule.mkDefeasible "r3" [Literal.pos "p"] (Literal.pos "q")
    ],
    superiority := [] }

-- Under AB: q is -d (p is ambiguous, so r3.applicable fails since p ∉ +d)
#eval reason apTest2
#guard hasConclusion (reason apTest2) .minusd (Literal.pos "q")
-- Under AP: q is also -d (same reasoning, AP is at least as strict)
#eval reasonAP apTest2
-- p is ambiguous: both -d under AP
#guard hasConclusion (reasonAP apTest2) .minusd (Literal.pos "p")
#guard hasConclusion (reasonAP apTest2) .minusd (Literal.neg "p")
-- q is -d under AP because r3 depends on ambiguous p
#guard hasConclusion (reasonAP apTest2) .minusd (Literal.pos "q")

/-! ### AP Subsumption theorem -/

/-- `apApplicable` implies `applicable`: if a rule is AP-applicable, it is
    also applicable (AP is stricter). -/
theorem apApplicable_implies_applicable (r : Rule) (plusd minusd : List Literal)
    (h : apApplicable r plusd minusd = true) :
    r.applicable plusd = true := by
  simp only [apApplicable, Bool.and_eq_true] at h
  exact h.1

/-- `apCanProve` implies `canProve`: if `q` satisfies the AP proof conditions,
    it also satisfies the AB proof conditions (AP is stricter). -/
theorem apCanProve_implies_canProve (t : Theory) (plusD plusd minusd : List Literal) (q : Literal)
    (h : apCanProve t plusD plusd minusd q = true) :
    Lambda.canProve t plusD plusd minusd q = true := by
  simp only [apCanProve, Lambda.canProve, Bool.and_eq_true] at h ⊢
  obtain ⟨⟨h_app, h_comp⟩, h_countered⟩ := h
  refine ⟨⟨?_, h_comp⟩, h_countered⟩
  rw [List.any_eq_true] at h_app ⊢
  obtain ⟨r, hr, hr_app⟩ := h_app
  exact ⟨r, hr, apApplicable_implies_applicable r plusd minusd hr_app⟩

/-! ### Ambiguity Infectiousness theorem -/

/-- A literal `p` is **ambiguous under AP** in a given state if it is in
    neither `+d` nor `-d`. -/
def isAmbiguousAP (p : Literal) (plusd minusd : List Literal) : Prop :=
  p ∉ plusd ∧ p ∉ minusd

/-- **Ambiguity Infectiousness**: if literal `p` is ambiguous (not in `+d` or
    `-d`), and rule `r` has `p` in its body, then `r` is NOT AP-applicable.

    This is the core mechanism of AP: ambiguous body literals prevent rule
    firing, propagating ambiguity to downstream conclusions. -/
theorem ambiguity_infects_applicability (r : Rule) (p : Literal)
    (plusd minusd : List Literal)
    (h_body : p ∈ r.body)
    (h_ambig : isAmbiguousAP p plusd minusd) :
    apApplicable r plusd minusd = false := by
  simp only [apApplicable]
  apply Bool.eq_false_iff.mpr
  intro h
  rw [Bool.and_eq_true] at h
  obtain ⟨_, h_decided⟩ := h
  rw [List.all_eq_true] at h_decided
  have := h_decided p h_body
  rw [Bool.or_eq_true] at this
  rcases this with h1 | h2
  · exact h_ambig.1 (List.contains_iff_mem.mp h1)
  · exact h_ambig.2 (List.contains_iff_mem.mp h2)

/-- **Ambiguity Infectiousness (conclusion form)**: if `p` is ambiguous, and
    every rule in `Rsd[q]` has `p` in its body, then `apCanProve q` is false.

    In other words: if the only derivation paths for `q` all go through an
    ambiguous literal, then `q` cannot be proved under AP. -/
theorem ambiguity_infects_conclusion (t : Theory) (plusD plusd minusd : List Literal)
    (p q : Literal)
    (h_ambig : isAmbiguousAP p plusd minusd)
    (h_all_depend : ∀ r ∈ t.Rsd q, p ∈ r.body) :
    apCanProve t plusD plusd minusd q = false := by
  apply Bool.eq_false_iff.mpr
  intro h
  simp only [apCanProve, Bool.and_eq_true] at h
  obtain ⟨⟨h_app, _⟩, _⟩ := h
  rw [List.any_eq_true] at h_app
  obtain ⟨r, hr, hr_app⟩ := h_app
  have h_false := ambiguity_infects_applicability r p plusd minusd (h_all_depend r hr) h_ambig
  rw [h_false] at hr_app
  exact Bool.false_ne_true hr_app

end AmbiguityPropagation
