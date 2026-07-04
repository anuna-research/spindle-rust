import SpindleLean
import Spindle
import Spindle.Arith.GroundingCompleteness
import Spindle.Arith.QuerySoundness
import Spindle.Arith.Abduce
import Spindle.Arith.WhatIf
import Spindle.Arith.WhyNot

-- Core SDL results
#print axioms Properties.reason_plusD_sound
#print axioms Properties.reason_plusD_complete
#print axioms Properties.faithful_plusd_forward
#print axioms Properties.faithful_plusd_backward
#print axioms Properties.faithful_plusD_forward
#print axioms Properties.faithful_plusD_backward
#print axioms Properties.delta_confluence
#print axioms Properties.deltaClose_converges_bound
#print axioms Properties.ambiguity_blocks_both
#print axioms Properties.partial_consistent
#print axioms Properties.partial_consistent_no_superiority

-- Arithmetic / temporal / grounding results
#print axioms Spindle.Arith.AllenRelation.compose_sound
#print axioms Spindle.Arith.AllenRelation.holds_unique
#print axioms Spindle.Arith.Rule.groundInstances_complete
#print axioms Spindle.Arith.cross_operator_soundness
#print axioms Spindle.Arith.pipeline_soundness
#print axioms Spindle.Arith.semiNaive_terminates_theory
