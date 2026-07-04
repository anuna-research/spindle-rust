-- Root module: imports every Spindle.Arith module so `lake build Spindle`
-- type-checks the whole library (no orphaned, unverified proofs).
import Spindle.Arith.Types
import Spindle.Arith.Promotion
import Spindle.Arith.Term
import Spindle.Arith.Eval
import Spindle.Arith.Constraint
import Spindle.Arith.Substitution
import Spindle.Arith.GroundLiteral
import Spindle.Arith.GroundRule
import Spindle.Arith.Matching
import Spindle.Arith.TimePoint
import Spindle.Arith.Interval
import Spindle.Arith.IntervalSet
import Spindle.Arith.AllenRelation
import Spindle.Arith.Composition
import Spindle.Arith.HerbrandBase
import Spindle.Arith.SemiNaive
import Spindle.Arith.GroundingCompat
import Spindle.Arith.GroundingCompleteness
import Spindle.Arith.TemporalGrounding
import Spindle.Arith.Abduce
import Spindle.Arith.WhatIf
import Spindle.Arith.WhyNot
import Spindle.Arith.QuerySoundness
import Spindle.Trust.Diminish
import Spindle.Trust.WeakestLink
import Spindle.Trust.Decay
