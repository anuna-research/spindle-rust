import SpindleLean.Reason

/-!
# SpindleLean.Tests

Smoke tests: at least 10 test theories exercising the `reason` function via
`#eval` and `#guard` statements.

Coverage: empty theory, single fact, strict chain, defeasible chain,
superiority override, defeater blocking, symmetric conflict, multiple
conclusions, mixed strict+defeasible, vacuous body rules.
-/

namespace SmokeTests

/-- Helper: check that a conclusion appears in the output. -/
def hasConclusion (cs : List Conclusion) (ct : ConclusionType) (l : Literal) : Bool :=
  cs.any fun c => c.conclusionType == ct && c.literal == l

-- ============================================================================
-- 1. Empty theory
-- ============================================================================

def emptyTheory : Theory := { rules := [], superiority := [] }

#eval reason emptyTheory
-- Expected: [] (no rules, no conclusions)

#guard (reason emptyTheory).isEmpty

-- ============================================================================
-- 2. Single fact
-- ============================================================================

def singleFact : Theory :=
  { rules := [Rule.mkFact "f1" (Literal.pos "a")], superiority := [] }

#eval reason singleFact
-- Expected: +D a, +d a

#guard hasConclusion (reason singleFact) .plusD (Literal.pos "a")
#guard hasConclusion (reason singleFact) .plusd (Literal.pos "a")
-- No -D a since a is definitely provable
#guard !hasConclusion (reason singleFact) .minusD (Literal.pos "a")

-- ============================================================================
-- 3. Strict chain: a → b → c
-- ============================================================================

def strictChain : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkStrict "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkStrict "r2" [Literal.pos "b"] (Literal.pos "c")
    ],
    superiority := [] }

#eval reason strictChain
-- Expected: +D a, +D b, +D c, +d a, +d b, +d c

#guard hasConclusion (reason strictChain) .plusD (Literal.pos "a")
#guard hasConclusion (reason strictChain) .plusD (Literal.pos "b")
#guard hasConclusion (reason strictChain) .plusD (Literal.pos "c")
#guard hasConclusion (reason strictChain) .plusd (Literal.pos "a")
#guard hasConclusion (reason strictChain) .plusd (Literal.pos "b")
#guard hasConclusion (reason strictChain) .plusd (Literal.pos "c")

-- ============================================================================
-- 4. Defeasible chain: a ⇒ b ⇒ c
-- ============================================================================

def defeasibleChain : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkDefeasible "r2" [Literal.pos "b"] (Literal.pos "c")
    ],
    superiority := [] }

#eval reason defeasibleChain
-- Expected: +D a, -D b, -D c, +d a, +d b, +d c

#guard hasConclusion (reason defeasibleChain) .plusD (Literal.pos "a")
#guard hasConclusion (reason defeasibleChain) .minusD (Literal.pos "b")
#guard hasConclusion (reason defeasibleChain) .minusD (Literal.pos "c")
#guard hasConclusion (reason defeasibleChain) .plusd (Literal.pos "a")
#guard hasConclusion (reason defeasibleChain) .plusd (Literal.pos "b")
#guard hasConclusion (reason defeasibleChain) .plusd (Literal.pos "c")

-- ============================================================================
-- 5. Superiority override: penguin doesn't fly
-- ============================================================================

def superiorityOverride : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "bird"),
      Rule.mkFact "f2" (Literal.pos "penguin"),
      Rule.mkDefeasible "r1" [Literal.pos "bird"] (Literal.pos "fly"),
      Rule.mkDefeasible "r2" [Literal.pos "penguin"] (Literal.neg "fly")
    ],
    superiority := [("r2", "r1")] }

#eval reason superiorityOverride
-- Expected: r2 > r1 so ¬fly wins: +d ¬fly, -d fly

#guard hasConclusion (reason superiorityOverride) .plusD (Literal.pos "bird")
#guard hasConclusion (reason superiorityOverride) .plusD (Literal.pos "penguin")
#guard hasConclusion (reason superiorityOverride) .plusd (Literal.neg "fly")
#guard hasConclusion (reason superiorityOverride) .minusd (Literal.pos "fly")

-- ============================================================================
-- 6. Defeater blocking: a ~> ¬b blocks a => b
-- ============================================================================

def defeaterBlocking : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkDefeater "r2" [Literal.pos "a"] (Literal.neg "b")
    ],
    superiority := [] }

#eval reason defeaterBlocking
-- Defeater blocks +d b but doesn't prove ¬b itself
-- Expected: -d b, -d ¬b

#guard hasConclusion (reason defeaterBlocking) .plusD (Literal.pos "a")
#guard hasConclusion (reason defeaterBlocking) .minusd (Literal.pos "b")
-- Defeater cannot prove its head, so ¬b is also -d
#guard hasConclusion (reason defeaterBlocking) .minusd (Literal.neg "b")

-- ============================================================================
-- 7. Symmetric conflict: a => b vs a => ¬b, no superiority
-- ============================================================================

def symmetricConflict : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkDefeasible "r2" [Literal.pos "a"] (Literal.neg "b")
    ],
    superiority := [] }

#eval reason symmetricConflict
-- Ambiguity blocking: neither wins → both -d

#guard hasConclusion (reason symmetricConflict) .minusd (Literal.pos "b")
#guard hasConclusion (reason symmetricConflict) .minusd (Literal.neg "b")
#guard !hasConclusion (reason symmetricConflict) .plusd (Literal.pos "b")
#guard !hasConclusion (reason symmetricConflict) .plusd (Literal.neg "b")

-- ============================================================================
-- 8. Multiple conclusions: a => b, a => c (independent)
-- ============================================================================

def multipleConclusions : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkDefeasible "r2" [Literal.pos "a"] (Literal.pos "c")
    ],
    superiority := [] }

#eval reason multipleConclusions
-- No conflict: both b and c should be +d

#guard hasConclusion (reason multipleConclusions) .plusd (Literal.pos "a")
#guard hasConclusion (reason multipleConclusions) .plusd (Literal.pos "b")
#guard hasConclusion (reason multipleConclusions) .plusd (Literal.pos "c")

-- ============================================================================
-- 9. Mixed strict + defeasible: a →strict b, b ⇒ c
-- ============================================================================

def mixedStrictDefeasible : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkStrict "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkDefeasible "r2" [Literal.pos "b"] (Literal.pos "c")
    ],
    superiority := [] }

#eval reason mixedStrictDefeasible
-- a and b are +D (facts + strict), c is +d via defeasible

#guard hasConclusion (reason mixedStrictDefeasible) .plusD (Literal.pos "a")
#guard hasConclusion (reason mixedStrictDefeasible) .plusD (Literal.pos "b")
#guard hasConclusion (reason mixedStrictDefeasible) .minusD (Literal.pos "c")
#guard hasConclusion (reason mixedStrictDefeasible) .plusd (Literal.pos "b")
#guard hasConclusion (reason mixedStrictDefeasible) .plusd (Literal.pos "c")

-- ============================================================================
-- 10. Vacuous body rules: defeasible rule with empty body (always fires)
-- ============================================================================

def vacuousBody : Theory :=
  { rules := [
      Rule.mkDefeasible "r1" [] (Literal.pos "b")
    ],
    superiority := [] }

#eval reason vacuousBody
-- Defeasible rule with empty body: not a fact (not strict), but always applicable
-- Expected: -D b (not strict), +d b (defeasible with empty body fires)

#guard hasConclusion (reason vacuousBody) .minusD (Literal.pos "b")
#guard hasConclusion (reason vacuousBody) .plusd (Literal.pos "b")

-- ============================================================================
-- 11. Strict override of defeasible: strict ¬p defeats defeasible p
-- ============================================================================

def strictOverridesDefeasible : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "p"),
      Rule.mkStrict "r2" [Literal.pos "a"] (Literal.neg "p")
    ],
    superiority := [] }

#eval reason strictOverridesDefeasible
-- ¬p is +D via strict rule → p cannot be +d (condition 2 fails: +D ~p)

#guard hasConclusion (reason strictOverridesDefeasible) .plusD (Literal.neg "p")
#guard hasConclusion (reason strictOverridesDefeasible) .minusd (Literal.pos "p")
#guard hasConclusion (reason strictOverridesDefeasible) .plusd (Literal.neg "p")

-- ============================================================================
-- 12. Diamond: two paths to same conclusion
-- ============================================================================

def diamond : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkDefeasible "r2" [Literal.pos "a"] (Literal.pos "c"),
      Rule.mkDefeasible "r3" [Literal.pos "b"] (Literal.pos "d"),
      Rule.mkDefeasible "r4" [Literal.pos "c"] (Literal.pos "d")
    ],
    superiority := [] }

#eval reason diamond
-- Both paths lead to d, no conflict → +d d

#guard hasConclusion (reason diamond) .plusd (Literal.pos "b")
#guard hasConclusion (reason diamond) .plusd (Literal.pos "c")
#guard hasConclusion (reason diamond) .plusd (Literal.pos "d")

end SmokeTests
