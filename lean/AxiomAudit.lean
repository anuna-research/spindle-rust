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

-- Aggregation contract (finite finalized inputs; no Rust refinement claimed).
#print axioms Spindle.Aggregation.Reducer.eval_permutation
#print axioms Spindle.Aggregation.Reducer.required_fails_iff_empty
#print axioms Spindle.Aggregation.aggregate_permutation
#print axioms Spindle.Aggregation.path_bound
#print axioms Spindle.Aggregation.no_aggregate_cycle
#print axioms Spindle.Aggregation.aggregate_input_finalized
#print axioms Spindle.Aggregation.aggregate_frozen
#print axioms Spindle.Aggregation.unproved_excluded
#print axioms Spindle.Aggregation.carry_fact_iff
#print axioms Spindle.Aggregation.Pattern.applySubst_key
#print axioms Spindle.Aggregation.dependencies_valid_iff
#print axioms Spindle.Aggregation.fold_waits_for_producer
#print axioms Spindle.Aggregation.producers_same_stage
#print axioms Spindle.Aggregation.checkProgram_iff
#print axioms Spindle.Aggregation.extracted_certificate_sound
#print axioms Spindle.Aggregation.cacheStage_eq
#print axioms Spindle.Aggregation.inferStrata_sound
#print axioms Spindle.Aggregation.inferStrata_complete
#print axioms Spindle.Aggregation.inferStrata_none_iff
#print axioms Spindle.Aggregation.inferStrata_least
#print axioms Spindle.Aggregation.inferStrata_permutation
#print axioms Spindle.Aggregation.inferProgram_sound
#print axioms Spindle.Aggregation.inferProgram_complete
#print axioms Spindle.Aggregation.inferProgram_unstratifiable_iff
#print axioms Spindle.Aggregation.advanceStage_commits
#print axioms Spindle.Aggregation.advanceStage_valid
#print axioms Spindle.Aggregation.runStages_extends
#print axioms Spindle.Aggregation.runStages_valid
#print axioms Spindle.Aggregation.executeStages_valid
#print axioms Spindle.Aggregation.runStages_old_conclusion_iff
#print axioms Spindle.Aggregation.runStages_preserves_fold
#print axioms Properties.deltaClose_fixedpoint
#print axioms Properties.lambdaClose_fixedpoint
#print axioms Properties.partialClose_fixedpoint
#print axioms Properties.reason_fixedpoints
#print axioms Spindle.Aggregation.groundScheduled_iff
#print axioms Spindle.Aggregation.mem_reasonedStage
#print axioms Spindle.Aggregation.groundBackend_completed
#print axioms Spindle.Aggregation.groundBackend_runs
#print axioms Spindle.Aggregation.reason_reports
#print axioms Spindle.Aggregation.reason_agrees_on_closed_domains
#print axioms Spindle.Aggregation.groundPrefix_equivalent
#print axioms Spindle.Aggregation.reasonedStage_equivalent
#print axioms Spindle.Aggregation.executeGround_membership
#print axioms Spindle.Aggregation.encodeAtom_owner
#print axioms Spindle.Aggregation.encodeAtom_injective_on
#print axioms Spindle.Aggregation.encodeAtom_ne_guard
#print axioms Spindle.Aggregation.emitted_definite_requires_guard
#print axioms Spindle.Aggregation.lowerStages_preserves
#print axioms Spindle.Aggregation.lowerStages_completed_agree
#print axioms Spindle.Aggregation.lowerProgram_inferred
#print axioms Spindle.Aggregation.lowerProgram_scheduled
#print axioms Spindle.Aggregation.lowerProgram_covers
#print axioms Spindle.Aggregation.lowerProgram_prefix_equivalent
#print axioms Spindle.Aggregation.lowerProgram_execute_equivalent
#print axioms Spindle.Aggregation.evaluateProgram_correct

-- Source aggregate semantics and compiler refinement.
#print axioms Spindle.Aggregation.Source.Fold.rows_extensional
#print axioms Spindle.Aggregation.evalSchemaFold_iff
#print axioms Spindle.Aggregation.sourceFold_deterministic
#print axioms Spindle.Aggregation.checkCondition_iff
#print axioms Spindle.Aggregation.lowerInstance_correct
#print axioms Spindle.Aggregation.lowerInstance_complete
#print axioms Spindle.Aggregation.assignment_iff
#print axioms Spindle.Aggregation.lowerBatch_correct
#print axioms Spindle.Aggregation.lowerStages_correct
#print axioms Spindle.Aggregation.lowerProgram_source_correct
#print axioms Spindle.Aggregation.evaluateProgram_source_correct
