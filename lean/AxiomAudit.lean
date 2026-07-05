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

-- Trust layer results
#print axioms Spindle.Trust.diminish_eq_mul
#print axioms Spindle.Trust.diminish_antitone
#print axioms Spindle.Trust.diminish_pos
#print axioms Spindle.Trust.diminishAll_eq_prod
#print axioms Spindle.Trust.diminishAll_le_single
#print axioms Spindle.Trust.DerivationTree.weakestLink_le_child
#print axioms Spindle.Trust.DerivationTree.le_weakestLink
#print axioms Spindle.Trust.DerivationTree.diminishAll_weakestLink_le_child
#print axioms Spindle.Trust.linearDecay_antitone
#print axioms Spindle.Trust.stepDecay_antitone
#print axioms Spindle.Trust.DecayLaw.effective_mem_unit

-- Query, filter, family, and grammar results
#print axioms Spindle.Arith.requiresVerify_facts_mem
#print axioms Spindle.Arith.requiresVerify_rejected
#print axioms Spindle.Arith.mem_filterTemporal
#print axioms Spindle.Arith.filterTemporal_atemporal
#print axioms Family.FLit.famSat_iff
#print axioms Family.deltaStepWith_parity
#print axioms Family.twoSidedStep_P_subset_lambda
#print axioms Family.canProve2_mono
#print axioms Family.twoSidedClose_P_sound
#print axioms Family.twoSidedClose_disjoint
#print axioms Family.twoSided_consistent
#print axioms Spl.decode_encode_theory

-- Arithmetic / temporal / grounding results
#print axioms Spindle.Arith.AllenRelation.compose_sound
#print axioms Spindle.Arith.AllenRelation.holds_unique
#print axioms Spindle.Arith.Rule.groundInstances_complete
#print axioms Spindle.Arith.cross_operator_soundness
#print axioms Spindle.Arith.pipeline_soundness
#print axioms Spindle.Arith.semiNaive_terminates_theory
