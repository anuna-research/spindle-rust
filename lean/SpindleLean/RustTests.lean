import SpindleLean.Reason

/-!
# SpindleLean.RustTests

Encodes 55 test theories from `crates/spindle-core/tests/` as Lean `#eval` and
`#guard` statements.  Each test builds a `Theory`, runs `reason`, and asserts
that the conclusions match the Rust test expectations.

## Coverage

| Category                | Count | Source files                                       |
|-------------------------|-------|----------------------------------------------------|
| Facts-only              |   3   | fixtures, proptest_reasoning                       |
| Strict chains           |   4   | fixtures, proptest_adversarial                     |
| Defeasible chains       |   4   | fixtures, proptest_adversarial                     |
| Superiority             |   7   | fixtures, proptest_adversarial                     |
| Defeaters               |   7   | fixtures, regression_known_bugs, proptest_adv.     |
| Symmetric conflicts     |   6   | fixtures, regression_known_bugs, proptest_adv.     |
| Ambiguity propagation   |   4   | proptest_adversarial                               |
| Empty theories          |   2   | fixtures                                           |
| Vacuous / empty-body    |   5   | regression_known_bugs, proptest_adversarial        |
| Mixed strict+defeasible |   4   | proptest_adversarial                               |
| Cycles & self-reference |   4   | proptest_adversarial, regression_known_bugs        |
| Diamond patterns        |   3   | proptest_adversarial                               |
| Conflicting facts       |   2   | proptest_adversarial                               |
-/

namespace RustTests

/-- Helper: check that a conclusion appears in the output. -/
def hasConclusion (cs : List Conclusion) (ct : ConclusionType) (l : Literal) : Bool :=
  cs.any fun c => c.conclusionType == ct && c.literal == l

/-- Negation of hasConclusion for readability. -/
def noConclusion (cs : List Conclusion) (ct : ConclusionType) (l : Literal) : Bool :=
  !hasConclusion cs ct l

-- ============================================================================
-- FACTS-ONLY (fixtures::facts_only, proptest_reasoning)
-- ============================================================================

-- 1. Three facts → all +D, all +d (facts are both definite and defeasible)
def factsOnly : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "bird"),
      Rule.mkFact "f2" (Literal.pos "penguin"),
      Rule.mkFact "f3" (Literal.pos "cold")
    ],
    superiority := [] }

#guard hasConclusion (reason factsOnly) .plusD (Literal.pos "bird")
#guard hasConclusion (reason factsOnly) .plusD (Literal.pos "penguin")
#guard hasConclusion (reason factsOnly) .plusD (Literal.pos "cold")
#guard hasConclusion (reason factsOnly) .plusd (Literal.pos "bird")
#guard hasConclusion (reason factsOnly) .plusd (Literal.pos "penguin")
#guard hasConclusion (reason factsOnly) .plusd (Literal.pos "cold")

-- 2. Single fact produces exactly one +D and one +d
def singleFactBird : Theory :=
  { rules := [Rule.mkFact "f1" (Literal.pos "bird")], superiority := [] }

#guard hasConclusion (reason singleFactBird) .plusD (Literal.pos "bird")
#guard hasConclusion (reason singleFactBird) .plusd (Literal.pos "bird")
#guard noConclusion (reason singleFactBird) .minusD (Literal.pos "bird")

-- 3. Negated fact: ~p as a fact
def negatedFact : Theory :=
  { rules := [Rule.mkFact "f1" (Literal.neg "p")], superiority := [] }

#guard hasConclusion (reason negatedFact) .plusD (Literal.neg "p")
#guard hasConclusion (reason negatedFact) .plusd (Literal.neg "p")

-- ============================================================================
-- EMPTY THEORIES (fixtures::empty_theory)
-- ============================================================================

-- 4. Empty theory: no rules → no conclusions
def emptyTheory : Theory := { rules := [], superiority := [] }

#guard (reason emptyTheory).isEmpty

-- 5. Theory with only superiority (no rules) → still empty conclusions
def supOnlyTheory : Theory := { rules := [], superiority := [("r1", "r2")] }

#guard (reason supOnlyTheory).isEmpty

-- ============================================================================
-- STRICT CHAINS (fixtures, proptest_adversarial)
-- ============================================================================

-- 6. Strict chain depth 1: a → b
def strictChain1 : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkStrict "r1" [Literal.pos "a"] (Literal.pos "b")
    ],
    superiority := [] }

#guard hasConclusion (reason strictChain1) .plusD (Literal.pos "a")
#guard hasConclusion (reason strictChain1) .plusD (Literal.pos "b")
#guard hasConclusion (reason strictChain1) .plusd (Literal.pos "b")

-- 7. Strict chain depth 3: a → b → c → d
def strictChain3 : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkStrict "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkStrict "r2" [Literal.pos "b"] (Literal.pos "c"),
      Rule.mkStrict "r3" [Literal.pos "c"] (Literal.pos "d")
    ],
    superiority := [] }

#guard hasConclusion (reason strictChain3) .plusD (Literal.pos "d")
#guard hasConclusion (reason strictChain3) .plusd (Literal.pos "d")

-- 8. Strict chain to negation: a → b → ~q
def strictChainNeg : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkStrict "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkStrict "r2" [Literal.pos "b"] (Literal.neg "q")
    ],
    superiority := [] }

#guard hasConclusion (reason strictChainNeg) .plusD (Literal.neg "q")

-- 9. Multi-body strict rule: a, b → c
def strictMultiBody : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkFact "f2" (Literal.pos "b"),
      Rule.mkStrict "r1" [Literal.pos "a", Literal.pos "b"] (Literal.pos "c")
    ],
    superiority := [] }

#guard hasConclusion (reason strictMultiBody) .plusD (Literal.pos "c")

-- ============================================================================
-- DEFEASIBLE CHAINS (fixtures::inheritance_chain, proptest_adversarial)
-- ============================================================================

-- 10. Defeasible chain depth 0: just fact p0
def defChain0 : Theory :=
  { rules := [Rule.mkFact "f1" (Literal.pos "p0")], superiority := [] }

#guard hasConclusion (reason defChain0) .plusD (Literal.pos "p0")
#guard hasConclusion (reason defChain0) .plusd (Literal.pos "p0")

-- 11. Defeasible chain depth 3: p0 ⇒ p1 ⇒ p2 ⇒ p3
def defChain3 : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "p0"),
      Rule.mkDefeasible "r1" [Literal.pos "p0"] (Literal.pos "p1"),
      Rule.mkDefeasible "r2" [Literal.pos "p1"] (Literal.pos "p2"),
      Rule.mkDefeasible "r3" [Literal.pos "p2"] (Literal.pos "p3")
    ],
    superiority := [] }

#guard hasConclusion (reason defChain3) .plusD (Literal.pos "p0")
#guard hasConclusion (reason defChain3) .minusD (Literal.pos "p1")
#guard hasConclusion (reason defChain3) .minusD (Literal.pos "p2")
#guard hasConclusion (reason defChain3) .minusD (Literal.pos "p3")
#guard hasConclusion (reason defChain3) .plusd (Literal.pos "p1")
#guard hasConclusion (reason defChain3) .plusd (Literal.pos "p2")
#guard hasConclusion (reason defChain3) .plusd (Literal.pos "p3")

-- 12. Defeasible chain depth 5
def defChain5 : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "p0"),
      Rule.mkDefeasible "r1" [Literal.pos "p0"] (Literal.pos "p1"),
      Rule.mkDefeasible "r2" [Literal.pos "p1"] (Literal.pos "p2"),
      Rule.mkDefeasible "r3" [Literal.pos "p2"] (Literal.pos "p3"),
      Rule.mkDefeasible "r4" [Literal.pos "p3"] (Literal.pos "p4"),
      Rule.mkDefeasible "r5" [Literal.pos "p4"] (Literal.pos "p5")
    ],
    superiority := [] }

#guard hasConclusion (reason defChain5) .plusd (Literal.pos "p5")

-- 13. Multi-body defeasible: a, b ⇒ c (both facts present)
def defMultiBody : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkFact "f2" (Literal.pos "b"),
      Rule.mkDefeasible "r1" [Literal.pos "a", Literal.pos "b"] (Literal.pos "c")
    ],
    superiority := [] }

#guard hasConclusion (reason defMultiBody) .plusd (Literal.pos "c")

-- ============================================================================
-- SUPERIORITY (fixtures::tweety_triangle, proptest_adversarial)
-- ============================================================================

-- 14. Tweety triangle: penguin ⇒ ~fly wins over bird ⇒ fly
def tweetyTriangle : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "bird"),
      Rule.mkFact "f2" (Literal.pos "penguin"),
      Rule.mkStrict "s1" [Literal.pos "penguin"] (Literal.pos "bird"),
      Rule.mkDefeasible "r1" [Literal.pos "bird"] (Literal.pos "fly"),
      Rule.mkDefeasible "r2" [Literal.pos "penguin"] (Literal.neg "fly")
    ],
    superiority := [("r2", "r1")] }

#guard hasConclusion (reason tweetyTriangle) .plusD (Literal.pos "bird")
#guard hasConclusion (reason tweetyTriangle) .plusD (Literal.pos "penguin")
#guard hasConclusion (reason tweetyTriangle) .plusd (Literal.neg "fly")
#guard noConclusion (reason tweetyTriangle) .plusd (Literal.pos "fly")

-- 15. Superiority resolves symmetric conflict: r1 > r2
def supResolves : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "q"),
      Rule.mkDefeasible "r2" [Literal.pos "a"] (Literal.neg "q")
    ],
    superiority := [("r1", "r2")] }

#guard hasConclusion (reason supResolves) .plusd (Literal.pos "q")
#guard noConclusion (reason supResolves) .plusd (Literal.neg "q")

-- 16. Superiority not transitive: r1>r2, r2>r3 does NOT imply r1>r3
def supNotTransitive : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkFact "f2" (Literal.pos "b"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "q"),
      Rule.mkDefeasible "r2" [Literal.pos "a"] (Literal.neg "q"),
      Rule.mkDefeasible "r3" [Literal.pos "b"] (Literal.pos "q")
    ],
    superiority := [("r1", "r2"), ("r2", "r3")] }

-- r1 > r2 defeats the attacker, so +d q
#guard hasConclusion (reason supNotTransitive) .plusd (Literal.pos "q")
-- ~q has attackers r1,r3 and r2 cannot defeat them → -d ~q
#guard noConclusion (reason supNotTransitive) .plusd (Literal.neg "q")

-- 17. Team defeat: r1>r2 but NOT r1>r3 → q still blocked
def teamDefeat : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkFact "f2" (Literal.pos "b"),
      Rule.mkFact "f3" (Literal.pos "c"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "q"),
      Rule.mkDefeasible "r2" [Literal.pos "b"] (Literal.neg "q"),
      Rule.mkDefeasible "r3" [Literal.pos "c"] (Literal.neg "q")
    ],
    superiority := [("r1", "r2")] }

-- r3 is undefeated → +d q should fail
#guard noConclusion (reason teamDefeat) .plusd (Literal.pos "q")

-- 18. Cross-rule superiority unblocks: r3 > r2 unblocks head for r1 too
def crossRuleSup : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkFact "f2" (Literal.pos "b"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "q"),
      Rule.mkDefeasible "r2" [Literal.pos "b"] (Literal.neg "q"),
      Rule.mkDefeasible "r3" [Literal.pos "b"] (Literal.pos "q")
    ],
    superiority := [("r3", "r2")] }

#guard hasConclusion (reason crossRuleSup) .plusd (Literal.pos "q")

-- 19. Superior rule with unsatisfied attacker: attacker body not met → no conflict
def supUnsatisfiedAttacker : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "q"),
      Rule.mkDefeasible "r2" [Literal.pos "b"] (Literal.neg "q")
    ],
    superiority := [("r1", "r2")] }

#guard hasConclusion (reason supUnsatisfiedAttacker) .plusd (Literal.pos "q")

-- 20. Empty-body superiority resolves conflict
def emptyBodySup : Theory :=
  { rules := [
      Rule.mkDefeasible "r1" [] (Literal.pos "q"),
      Rule.mkDefeasible "r2" [] (Literal.neg "q")
    ],
    superiority := [("r1", "r2")] }

#guard hasConclusion (reason emptyBodySup) .plusd (Literal.pos "q")

-- ============================================================================
-- DEFEATERS (fixtures::conflicting_defeaters, regression, proptest_adversarial)
-- ============================================================================

-- 21. Basic defeater: a ~> ~b blocks a ⇒ b
def basicDefeater : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkDefeater "d1" [Literal.pos "a"] (Literal.neg "b")
    ],
    superiority := [] }

#guard noConclusion (reason basicDefeater) .plusd (Literal.pos "b")
#guard noConclusion (reason basicDefeater) .plusd (Literal.neg "b")

-- 22. Defeater never proves its head alone
def defeaterAlone : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeater "d1" [Literal.pos "a"] (Literal.pos "q")
    ],
    superiority := [] }

#guard noConclusion (reason defeaterAlone) .plusd (Literal.pos "q")
#guard noConclusion (reason defeaterAlone) .plusD (Literal.pos "q")

-- 23. Multiple defeaters blocking same literal
def multiDefeaters : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "bird"),
      Rule.mkFact "f2" (Literal.pos "injured"),
      Rule.mkFact "f3" (Literal.pos "young"),
      Rule.mkDefeasible "r1" [Literal.pos "bird"] (Literal.pos "flies"),
      Rule.mkDefeater "d1" [Literal.pos "injured"] (Literal.neg "flies"),
      Rule.mkDefeater "d2" [Literal.pos "young"] (Literal.neg "flies")
    ],
    superiority := [] }

#guard noConclusion (reason multiDefeaters) .plusd (Literal.pos "flies")
#guard noConclusion (reason multiDefeaters) .plusd (Literal.neg "flies")

-- 24. Defeater blocked by superiority: r1 > d1 unblocks
def defeaterWithSup : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "q"),
      Rule.mkDefeater "d1" [Literal.pos "a"] (Literal.neg "q")
    ],
    superiority := [("r1", "d1")] }

#guard hasConclusion (reason defeaterWithSup) .plusd (Literal.pos "q")

-- 25. Empty-body defeater blocks empty-body defeasible
def emptyBodyDefeater : Theory :=
  { rules := [
      Rule.mkDefeasible "r1" [] (Literal.pos "q"),
      Rule.mkDefeater "d1" [] (Literal.neg "q")
    ],
    superiority := [] }

#guard noConclusion (reason emptyBodyDefeater) .plusd (Literal.pos "q")
#guard noConclusion (reason emptyBodyDefeater) .plusd (Literal.neg "q")

-- 26. Mutual defeaters prove nothing
def mutualDefeaters : Theory :=
  { rules := [
      Rule.mkDefeater "d1" [] (Literal.neg "q"),
      Rule.mkDefeater "d2" [] (Literal.pos "q")
    ],
    superiority := [] }

#guard noConclusion (reason mutualDefeaters) .plusd (Literal.pos "q")
#guard noConclusion (reason mutualDefeaters) .plusd (Literal.neg "q")

-- 27. Conflicting defeaters fixture: bird/injured/young
def conflictingDefeaters : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "bird"),
      Rule.mkFact "f2" (Literal.pos "injured"),
      Rule.mkFact "f3" (Literal.pos "young"),
      Rule.mkDefeasible "r1" [Literal.pos "bird"] (Literal.pos "flies"),
      Rule.mkDefeasible "r2" [Literal.pos "bird"] (Literal.pos "has_feathers"),
      Rule.mkDefeater "d1" [Literal.pos "injured"] (Literal.neg "flies"),
      Rule.mkDefeater "d2" [Literal.pos "young"] (Literal.neg "flies"),
      Rule.mkDefeater "d3" [Literal.pos "injured"] (Literal.neg "has_feathers")
    ],
    superiority := [] }

#guard noConclusion (reason conflictingDefeaters) .plusd (Literal.pos "flies")
#guard noConclusion (reason conflictingDefeaters) .plusd (Literal.pos "has_feathers")

-- ============================================================================
-- SYMMETRIC CONFLICTS / AMBIGUITY (fixtures::nixon_diamond, regression)
-- ============================================================================

-- 28. Nixon diamond: neither pacifist nor ~pacifist
def nixonDiamond : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "republican"),
      Rule.mkFact "f2" (Literal.pos "quaker"),
      Rule.mkDefeasible "r1" [Literal.pos "republican"] (Literal.neg "pacifist"),
      Rule.mkDefeasible "r2" [Literal.pos "quaker"] (Literal.pos "pacifist")
    ],
    superiority := [] }

#guard noConclusion (reason nixonDiamond) .plusd (Literal.pos "pacifist")
#guard noConclusion (reason nixonDiamond) .plusd (Literal.neg "pacifist")

-- 29. Minimal symmetric conflict: p ⇒ q vs p ⇒ ~q
def minSymConflict : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "p"),
      Rule.mkDefeasible "r1" [Literal.pos "p"] (Literal.pos "q"),
      Rule.mkDefeasible "r2" [Literal.pos "p"] (Literal.neg "q")
    ],
    superiority := [] }

#guard noConclusion (reason minSymConflict) .plusd (Literal.pos "q")
#guard noConclusion (reason minSymConflict) .plusd (Literal.neg "q")

-- 30. Empty-body symmetric conflict: ⇒ q vs ⇒ ~q
def emptyBodyConflict : Theory :=
  { rules := [
      Rule.mkDefeasible "r1" [] (Literal.pos "q"),
      Rule.mkDefeasible "r2" [] (Literal.neg "q")
    ],
    superiority := [] }

#guard noConclusion (reason emptyBodyConflict) .plusd (Literal.pos "q")
#guard noConclusion (reason emptyBodyConflict) .plusd (Literal.neg "q")

-- 31. Star topology: some heads conflict, some don't
def starMixed : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkDefeasible "r2" [Literal.pos "a"] (Literal.neg "b"),
      Rule.mkDefeasible "r3" [Literal.pos "a"] (Literal.pos "c"),
      Rule.mkDefeasible "r4" [Literal.pos "a"] (Literal.pos "d"),
      Rule.mkDefeasible "r5" [Literal.pos "a"] (Literal.neg "d")
    ],
    superiority := [] }

#guard noConclusion (reason starMixed) .plusd (Literal.pos "b")
#guard noConclusion (reason starMixed) .plusd (Literal.neg "b")
#guard hasConclusion (reason starMixed) .plusd (Literal.pos "c")
#guard noConclusion (reason starMixed) .plusd (Literal.pos "d")
#guard noConclusion (reason starMixed) .plusd (Literal.neg "d")

-- 32. Self-loop with conflict: p ⇒ q, q ⇒ q, p ⇒ ~q → ambiguity
def selfLoopConflict : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "p"),
      Rule.mkDefeasible "r1" [Literal.pos "p"] (Literal.pos "q"),
      Rule.mkDefeasible "r2" [Literal.pos "q"] (Literal.pos "q"),
      Rule.mkDefeasible "r3" [Literal.pos "p"] (Literal.neg "q")
    ],
    superiority := [] }

#guard noConclusion (reason selfLoopConflict) .plusd (Literal.pos "q")
#guard noConclusion (reason selfLoopConflict) .plusd (Literal.neg "q")

-- 33. Unsatisfied attacker doesn't block
def unsatisfiedAttacker : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "q"),
      Rule.mkDefeasible "r2" [Literal.pos "missing"] (Literal.neg "q")
    ],
    superiority := [] }

#guard hasConclusion (reason unsatisfiedAttacker) .plusd (Literal.pos "q")

-- ============================================================================
-- AMBIGUITY PROPAGATION (proptest_adversarial)
-- ============================================================================

-- 34. Cascading ambiguity: conflict on b blocks c, d, e
def cascadingAmbiguity : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkDefeasible "r2" [Literal.pos "a"] (Literal.neg "b"),
      Rule.mkDefeasible "r3" [Literal.pos "b"] (Literal.pos "c"),
      Rule.mkDefeasible "r4" [Literal.pos "c"] (Literal.pos "d"),
      Rule.mkDefeasible "r5" [Literal.pos "d"] (Literal.pos "e")
    ],
    superiority := [] }

#guard noConclusion (reason cascadingAmbiguity) .plusd (Literal.pos "b")
#guard noConclusion (reason cascadingAmbiguity) .plusd (Literal.pos "c")
#guard noConclusion (reason cascadingAmbiguity) .plusd (Literal.pos "d")
#guard noConclusion (reason cascadingAmbiguity) .plusd (Literal.pos "e")

-- 35. Ambiguity is localized: independent conclusions unaffected
def ambiguityLocalized : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [] (Literal.pos "p"),
      Rule.mkDefeasible "r2" [] (Literal.neg "p"),
      Rule.mkDefeasible "r3" [Literal.pos "a"] (Literal.pos "q")
    ],
    superiority := [] }

#guard noConclusion (reason ambiguityLocalized) .plusd (Literal.pos "p")
#guard noConclusion (reason ambiguityLocalized) .plusd (Literal.neg "p")
#guard hasConclusion (reason ambiguityLocalized) .plusd (Literal.pos "q")

-- 36. Ambiguity blocks downstream but not siblings
def ambiguityBlocksDownstream : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "m"),
      Rule.mkDefeasible "r2" [Literal.pos "a"] (Literal.neg "m"),
      Rule.mkDefeasible "r3" [Literal.pos "m"] (Literal.pos "downstream"),
      Rule.mkDefeasible "r4" [Literal.pos "a"] (Literal.pos "sibling")
    ],
    superiority := [] }

#guard noConclusion (reason ambiguityBlocksDownstream) .plusd (Literal.pos "m")
#guard noConclusion (reason ambiguityBlocksDownstream) .plusd (Literal.pos "downstream")
#guard hasConclusion (reason ambiguityBlocksDownstream) .plusd (Literal.pos "sibling")

-- 37. Floating conclusion: q has independent support despite p being ambiguous
def floatingConclusion : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkFact "f2" (Literal.pos "b"),
      Rule.mkFact "f3" (Literal.pos "c"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "p"),
      Rule.mkDefeasible "r2" [Literal.pos "b"] (Literal.neg "p"),
      Rule.mkDefeasible "r3" [Literal.pos "c"] (Literal.pos "q"),
      Rule.mkDefeasible "r4" [Literal.pos "p"] (Literal.pos "q")
    ],
    superiority := [] }

#guard noConclusion (reason floatingConclusion) .plusd (Literal.pos "p")
#guard hasConclusion (reason floatingConclusion) .plusd (Literal.pos "q")

-- ============================================================================
-- VACUOUS / EMPTY-BODY RULES (regression_known_bugs, proptest_adversarial)
-- ============================================================================

-- 38. Empty-body defeasible fires: ⇒ truth → +d truth
def emptyBodyFires : Theory :=
  { rules := [Rule.mkDefeasible "r1" [] (Literal.pos "truth")],
    superiority := [] }

#guard noConclusion (reason emptyBodyFires) .plusD (Literal.pos "truth")
#guard hasConclusion (reason emptyBodyFires) .plusd (Literal.pos "truth")

-- 39. Empty-body chains: ⇒ base, base ⇒ derived
def emptyBodyChains : Theory :=
  { rules := [
      Rule.mkDefeasible "r1" [] (Literal.pos "base"),
      Rule.mkDefeasible "r2" [Literal.pos "base"] (Literal.pos "derived")
    ],
    superiority := [] }

#guard hasConclusion (reason emptyBodyChains) .plusd (Literal.pos "base")
#guard hasConclusion (reason emptyBodyChains) .plusd (Literal.pos "derived")

-- 40. Empty-body strict rule (fact/axiom) → +D
def emptyBodyStrict : Theory :=
  { rules := [Rule.mkStrict "r1" [] (Literal.pos "axiom")],
    superiority := [] }

#guard hasConclusion (reason emptyBodyStrict) .plusD (Literal.pos "axiom")
#guard hasConclusion (reason emptyBodyStrict) .plusd (Literal.pos "axiom")

-- 41. Mixed empty-body with body attacker: ambiguity
def emptyBodyWithAttacker : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [] (Literal.pos "q"),
      Rule.mkDefeasible "r2" [Literal.pos "a"] (Literal.neg "q")
    ],
    superiority := [] }

#guard noConclusion (reason emptyBodyWithAttacker) .plusd (Literal.pos "q")
#guard noConclusion (reason emptyBodyWithAttacker) .plusd (Literal.neg "q")

-- 42. Multiple empty-body rules, independent heads
def multiEmptyBody : Theory :=
  { rules := [
      Rule.mkDefeasible "r1" [] (Literal.pos "a"),
      Rule.mkDefeasible "r2" [] (Literal.pos "b"),
      Rule.mkDefeasible "r3" [] (Literal.pos "c")
    ],
    superiority := [] }

#guard hasConclusion (reason multiEmptyBody) .plusd (Literal.pos "a")
#guard hasConclusion (reason multiEmptyBody) .plusd (Literal.pos "b")
#guard hasConclusion (reason multiEmptyBody) .plusd (Literal.pos "c")

-- ============================================================================
-- MIXED STRICT + DEFEASIBLE (proptest_adversarial)
-- ============================================================================

-- 43. Strict proves ~q, defeasible for q → q blocked
def strictBlocksDefeasible : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkFact "f2" (Literal.pos "trigger"),
      Rule.mkStrict "s1" [Literal.pos "a"] (Literal.neg "q"),
      Rule.mkDefeasible "r1" [Literal.pos "trigger"] (Literal.pos "q")
    ],
    superiority := [] }

#guard hasConclusion (reason strictBlocksDefeasible) .plusD (Literal.neg "q")
#guard noConclusion (reason strictBlocksDefeasible) .plusd (Literal.pos "q")

-- 44. Strict immune to defeasible attacker
def strictImmune : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkStrict "s1" [Literal.pos "a"] (Literal.pos "q"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.neg "q")
    ],
    superiority := [] }

#guard hasConclusion (reason strictImmune) .plusD (Literal.pos "q")
#guard noConclusion (reason strictImmune) .plusd (Literal.neg "q")

-- 45. Multi-hop strict chain blocks defeasible
def multiHopStrictBlocks : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkFact "f2" (Literal.pos "b"),
      Rule.mkStrict "s1" [Literal.pos "a"] (Literal.pos "c"),
      Rule.mkStrict "s2" [Literal.pos "c"] (Literal.pos "d"),
      Rule.mkStrict "s3" [Literal.pos "d"] (Literal.neg "q"),
      Rule.mkDefeasible "r1" [Literal.pos "b"] (Literal.pos "q")
    ],
    superiority := [] }

#guard hasConclusion (reason multiHopStrictBlocks) .plusD (Literal.pos "c")
#guard hasConclusion (reason multiHopStrictBlocks) .plusD (Literal.pos "d")
#guard hasConclusion (reason multiHopStrictBlocks) .plusD (Literal.neg "q")
#guard noConclusion (reason multiHopStrictBlocks) .plusd (Literal.pos "q")

-- 46. Conflicting strict chains: +D q and +D ~q → neither +d
def conflictingStrict : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkFact "f2" (Literal.pos "b"),
      Rule.mkStrict "s1" [Literal.pos "a"] (Literal.pos "q"),
      Rule.mkStrict "s2" [Literal.pos "b"] (Literal.neg "q")
    ],
    superiority := [] }

#guard hasConclusion (reason conflictingStrict) .plusD (Literal.pos "q")
#guard hasConclusion (reason conflictingStrict) .plusD (Literal.neg "q")
#guard noConclusion (reason conflictingStrict) .plusd (Literal.pos "q")
#guard noConclusion (reason conflictingStrict) .plusd (Literal.neg "q")

-- ============================================================================
-- CYCLES & SELF-REFERENCE (proptest_adversarial)
-- ============================================================================

-- 47. Self-referential with fact: a ⇒ a harmless when a is fact
def selfRefWithFact : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "a")
    ],
    superiority := [] }

#guard hasConclusion (reason selfRefWithFact) .plusD (Literal.pos "a")
#guard hasConclusion (reason selfRefWithFact) .plusd (Literal.pos "a")

-- 48. Self-referential without fact: a ⇒ a should NOT prove a
def selfRefNoFact : Theory :=
  { rules := [Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "a")],
    superiority := [] }

#guard noConclusion (reason selfRefNoFact) .plusd (Literal.pos "a")

-- 49. Circular rules without fact: c0 ⇒ c1 ⇒ c2 ⇒ c0 → nothing proved
def circularNoFact : Theory :=
  { rules := [
      Rule.mkDefeasible "r1" [Literal.pos "c0"] (Literal.pos "c1"),
      Rule.mkDefeasible "r2" [Literal.pos "c1"] (Literal.pos "c2"),
      Rule.mkDefeasible "r3" [Literal.pos "c2"] (Literal.pos "c0")
    ],
    superiority := [] }

#guard noConclusion (reason circularNoFact) .plusd (Literal.pos "c0")
#guard noConclusion (reason circularNoFact) .plusd (Literal.pos "c1")
#guard noConclusion (reason circularNoFact) .plusd (Literal.pos "c2")

-- 50. Circular with fact seed: c0 fact → all downstream proved
def circularWithFact : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "c0"),
      Rule.mkDefeasible "r1" [Literal.pos "c0"] (Literal.pos "c1"),
      Rule.mkDefeasible "r2" [Literal.pos "c1"] (Literal.pos "c2"),
      Rule.mkDefeasible "r3" [Literal.pos "c2"] (Literal.pos "c0")
    ],
    superiority := [] }

#guard hasConclusion (reason circularWithFact) .plusd (Literal.pos "c0")
#guard hasConclusion (reason circularWithFact) .plusd (Literal.pos "c1")
#guard hasConclusion (reason circularWithFact) .plusd (Literal.pos "c2")

-- ============================================================================
-- DIAMOND PATTERNS (proptest_adversarial)
-- ============================================================================

-- 51. Diamond: two paths to same conclusion, no conflict → +d d
def diamondConverge : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkDefeasible "r2" [Literal.pos "a"] (Literal.pos "c"),
      Rule.mkDefeasible "r3" [Literal.pos "b"] (Literal.pos "d"),
      Rule.mkDefeasible "r4" [Literal.pos "c"] (Literal.pos "d")
    ],
    superiority := [] }

#guard hasConclusion (reason diamondConverge) .plusd (Literal.pos "d")

-- 52. Diamond with one path blocked: b ambiguous, c uncontested → d via c
def diamondOneBlocked : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkDefeasible "r2" [Literal.pos "a"] (Literal.pos "c"),
      Rule.mkDefeasible "r3" [Literal.pos "a"] (Literal.neg "b"),
      Rule.mkDefeasible "r4" [Literal.pos "b"] (Literal.pos "d"),
      Rule.mkDefeasible "r5" [Literal.pos "c"] (Literal.pos "d")
    ],
    superiority := [] }

#guard noConclusion (reason diamondOneBlocked) .plusd (Literal.pos "b")
#guard hasConclusion (reason diamondOneBlocked) .plusd (Literal.pos "c")
#guard hasConclusion (reason diamondOneBlocked) .plusd (Literal.pos "d")

-- 53. Diamond with both paths blocked: d unprovable
def diamondBothBlocked : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "a"),
      Rule.mkDefeasible "r1" [Literal.pos "a"] (Literal.pos "b"),
      Rule.mkDefeasible "r2" [Literal.pos "a"] (Literal.neg "b"),
      Rule.mkDefeasible "r3" [Literal.pos "a"] (Literal.pos "c"),
      Rule.mkDefeasible "r4" [Literal.pos "a"] (Literal.neg "c"),
      Rule.mkDefeasible "r5" [Literal.pos "b"] (Literal.pos "d"),
      Rule.mkDefeasible "r6" [Literal.pos "c"] (Literal.pos "d")
    ],
    superiority := [] }

#guard noConclusion (reason diamondBothBlocked) .plusd (Literal.pos "b")
#guard noConclusion (reason diamondBothBlocked) .plusd (Literal.pos "c")
#guard noConclusion (reason diamondBothBlocked) .plusd (Literal.pos "d")

-- ============================================================================
-- CONFLICTING FACTS (proptest_adversarial)
-- ============================================================================

-- 54. Conflicting facts: both +D p and +D ~p → neither +d
def conflictingFacts : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "p"),
      Rule.mkFact "f2" (Literal.neg "p")
    ],
    superiority := [] }

#guard hasConclusion (reason conflictingFacts) .plusD (Literal.pos "p")
#guard hasConclusion (reason conflictingFacts) .plusD (Literal.neg "p")
#guard noConclusion (reason conflictingFacts) .plusd (Literal.pos "p")
#guard noConclusion (reason conflictingFacts) .plusd (Literal.neg "p")

-- 55. Duplicate body: rule [p, p, q] ⇒ r with missing q should NOT fire
def dupBodyMissing : Theory :=
  { rules := [
      Rule.mkFact "f1" (Literal.pos "p"),
      Rule.mkDefeasible "r1" [Literal.pos "p", Literal.pos "p", Literal.pos "q"]
        (Literal.pos "r")
    ],
    superiority := [] }

#guard noConclusion (reason dupBodyMissing) .plusd (Literal.pos "r")

end RustTests
