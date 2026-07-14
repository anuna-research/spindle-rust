//! Pipeline integration tests (TEST-009).
//!
//! Verifies that the refactored composable `Pipeline` system produces
//! identical output to the `prepare()` free function for every test
//! fixture. Also exercises custom pipeline construction, individual
//! stage composition, and diagnostics/metadata population.

mod fixtures;

use std::collections::BTreeSet;

use spindle_core::conclusion::ConclusionType;
use spindle_core::pipeline::{
    Ground, MetadataVal, Pipeline, PipelineContext, PipelineStage, PrepareOptions, Severity,
    TemporalFilter, Validate, WildcardRewrite, prepare,
};
use spindle_core::reason::{reason, reason_prepared, reason_with_options};
use spindle_core::temporal::TimePoint;
use spindle_core::theory::Theory;

// ===========================================================================
// Helpers
// ===========================================================================

/// Normalise a set of conclusions into a deterministic, comparable form.
///
/// Each conclusion is rendered as `"{symbol} {literal}"` (e.g. `"+D bird"`)
/// and collected into a sorted `BTreeSet` so the comparison is
/// order-independent.
fn normalised_conclusions(
    conclusions: &[spindle_core::conclusion::Conclusion],
) -> BTreeSet<String> {
    conclusions.iter().map(|c| format!("{c}")).collect()
}

/// Run `Pipeline::default_pipeline()` on a theory and return the prepared
/// theory together with its context.
fn run_default_pipeline(theory: &Theory) -> (Theory, PipelineContext) {
    Pipeline::default_pipeline()
        .run(theory.clone())
        .expect("default pipeline should not error")
}

/// Run the `prepare()` free function with default options.
fn run_prepare_default(theory: &Theory) -> Theory {
    prepare(theory, PrepareOptions::default())
        .expect("prepare() should not error")
        .theory
}

// ===========================================================================
// 1. Pipeline::default_pipeline() vs prepare() — theory equivalence
// ===========================================================================

/// For each fixture, assert that `Pipeline::default_pipeline().run(theory)`
/// yields a theory with the same rule count as `prepare(theory, default)`.
///
/// Note: `prepare()` inserts the TemporalFilter only when a reference time
/// is provided, but the default pipeline omits it — so for the default
/// options (no reference time) the two paths should agree exactly.
macro_rules! assert_pipeline_vs_prepare_theory {
    ($name:ident, $fixture:expr) => {
        #[test]
        fn $name() {
            let theory = $fixture;

            let (pipeline_theory, _ctx) = run_default_pipeline(&theory);
            let prepare_theory = run_prepare_default(&theory);

            assert_eq!(
                pipeline_theory.rule_count(),
                prepare_theory.rule_count(),
                "Pipeline and prepare() should produce the same number of rules for {}",
                stringify!($name),
            );

            // Compare superiority counts
            assert_eq!(
                pipeline_theory.superiorities().len(),
                prepare_theory.superiorities().len(),
                "Pipeline and prepare() should preserve the same superiorities for {}",
                stringify!($name),
            );
        }
    };
}

assert_pipeline_vs_prepare_theory!(pipeline_vs_prepare_tweety, fixtures::tweety_triangle());
assert_pipeline_vs_prepare_theory!(pipeline_vs_prepare_nixon, fixtures::nixon_diamond());
assert_pipeline_vs_prepare_theory!(pipeline_vs_prepare_chain_5, fixtures::inheritance_chain(5));
assert_pipeline_vs_prepare_theory!(pipeline_vs_prepare_chain_0, fixtures::inheritance_chain(0));
assert_pipeline_vs_prepare_theory!(pipeline_vs_prepare_temporal, fixtures::temporal_theory());
assert_pipeline_vs_prepare_theory!(pipeline_vs_prepare_modal, fixtures::modal_theory());
assert_pipeline_vs_prepare_theory!(
    pipeline_vs_prepare_defeaters,
    fixtures::conflicting_defeaters()
);
assert_pipeline_vs_prepare_theory!(pipeline_vs_prepare_empty, fixtures::empty_theory());
assert_pipeline_vs_prepare_theory!(pipeline_vs_prepare_facts_only, fixtures::facts_only());

// ===========================================================================
// 2. Reasoning on both prepared theories yields identical conclusions
// ===========================================================================

macro_rules! assert_reasoning_equivalence {
    ($name:ident, $fixture:expr) => {
        #[test]
        fn $name() {
            let theory = $fixture;

            // Path A: Pipeline::default_pipeline().run() then reason_prepared()
            let (pipeline_theory, _) = run_default_pipeline(&theory);
            let conclusions_a = reason_prepared(&pipeline_theory)
                .expect("reason_prepared on pipeline output should succeed");

            // Path B: prepare() then reason_prepared()
            let prepare_theory = run_prepare_default(&theory);
            let conclusions_b = reason_prepared(&prepare_theory)
                .expect("reason_prepared on prepare output should succeed");

            let norm_a = normalised_conclusions(&conclusions_a);
            let norm_b = normalised_conclusions(&conclusions_b);

            assert_eq!(
                norm_a,
                norm_b,
                "Conclusions should be identical for {} regardless of preparation path.\n\
                 Pipeline-only: {:?}\n\
                 Prepare-only:  {:?}",
                stringify!($name),
                norm_a.difference(&norm_b).collect::<Vec<_>>(),
                norm_b.difference(&norm_a).collect::<Vec<_>>(),
            );
        }
    };
}

assert_reasoning_equivalence!(reason_equiv_tweety, fixtures::tweety_triangle());
assert_reasoning_equivalence!(reason_equiv_nixon, fixtures::nixon_diamond());
assert_reasoning_equivalence!(reason_equiv_chain_5, fixtures::inheritance_chain(5));
assert_reasoning_equivalence!(reason_equiv_chain_0, fixtures::inheritance_chain(0));
assert_reasoning_equivalence!(reason_equiv_temporal, fixtures::temporal_theory());
assert_reasoning_equivalence!(reason_equiv_modal, fixtures::modal_theory());
assert_reasoning_equivalence!(reason_equiv_defeaters, fixtures::conflicting_defeaters());
assert_reasoning_equivalence!(reason_equiv_empty, fixtures::empty_theory());
assert_reasoning_equivalence!(reason_equiv_facts_only, fixtures::facts_only());

// ===========================================================================
// 3. Custom pipeline with temporal filter matches prepare() with reference_time
// ===========================================================================

#[test]
fn temporal_pipeline_matches_prepare_inside_window() {
    let theory = fixtures::temporal_theory();
    let ref_time = TimePoint::from_millis(150); // inside [100, 200]

    // Path A: custom pipeline with temporal filter
    let pipeline = Pipeline::builder()
        .stage(TemporalFilter {
            reference_time: ref_time,
        })
        .stage(Validate::default())
        .stage(WildcardRewrite)
        .stage(Ground::default())
        .build();
    let (pipeline_theory, _) = pipeline.run(theory.clone()).unwrap();
    let conclusions_a = reason_prepared(&pipeline_theory).unwrap();

    // Path B: prepare() with reference_time
    let opts = PrepareOptions {
        reference_time: Some(ref_time),
        ..Default::default()
    };
    let result_b = prepare(&theory, opts).unwrap();
    let conclusions_b = reason_prepared(&result_b.theory).unwrap();

    let norm_a = normalised_conclusions(&conclusions_a);
    let norm_b = normalised_conclusions(&conclusions_b);

    assert_eq!(
        norm_a, norm_b,
        "Temporal pipeline should match prepare() with reference_time inside window"
    );
}

#[test]
fn temporal_pipeline_matches_prepare_outside_window() {
    let theory = fixtures::temporal_theory();
    let ref_time = TimePoint::from_millis(300); // outside [100, 200]

    // Path A: custom pipeline
    let pipeline = Pipeline::builder()
        .stage(TemporalFilter {
            reference_time: ref_time,
        })
        .stage(Validate::default())
        .stage(WildcardRewrite)
        .stage(Ground::default())
        .build();
    let (pipeline_theory, _) = pipeline.run(theory.clone()).unwrap();
    let conclusions_a = reason_prepared(&pipeline_theory).unwrap();

    // Path B: prepare()
    let opts = PrepareOptions {
        reference_time: Some(ref_time),
        ..Default::default()
    };
    let result_b = prepare(&theory, opts).unwrap();
    let conclusions_b = reason_prepared(&result_b.theory).unwrap();

    let norm_a = normalised_conclusions(&conclusions_a);
    let norm_b = normalised_conclusions(&conclusions_b);

    assert_eq!(
        norm_a, norm_b,
        "Temporal pipeline should match prepare() with reference_time outside window"
    );
}

#[test]
fn temporal_filtering_removes_rules_outside_window() {
    let theory = fixtures::temporal_theory();

    // At time 300, the on_leave fact [100,200] and the r2 rule [100,200]
    // should be filtered out. The theory should have fewer rules than the
    // original.
    let ref_time = TimePoint::from_millis(300);
    let pipeline = Pipeline::builder()
        .stage(TemporalFilter {
            reference_time: ref_time,
        })
        .build();

    let (filtered, _) = pipeline.run(theory.clone()).unwrap();
    assert!(
        filtered.rule_count() < theory.rule_count(),
        "Temporal filter at t=300 should remove at least one rule: \
         original={}, filtered={}",
        theory.rule_count(),
        filtered.rule_count(),
    );
}

#[test]
fn temporal_filtering_keeps_all_inside_window() {
    let theory = fixtures::temporal_theory();

    // At time 150, all rules (including those bounded [100,200] and [0,500])
    // are active.
    let ref_time = TimePoint::from_millis(150);
    let pipeline = Pipeline::builder()
        .stage(TemporalFilter {
            reference_time: ref_time,
        })
        .build();

    let (filtered, _) = pipeline.run(theory.clone()).unwrap();
    assert_eq!(
        filtered.rule_count(),
        theory.rule_count(),
        "Temporal filter at t=150 should keep all rules"
    );
}

// ===========================================================================
// 4. Each fixture passes through the pipeline without errors
// ===========================================================================

macro_rules! assert_pipeline_ok {
    ($name:ident, $fixture:expr) => {
        #[test]
        fn $name() {
            let theory = $fixture;
            let result = Pipeline::default_pipeline().run(theory);
            assert!(
                result.is_ok(),
                "Pipeline should succeed for {}: {:?}",
                stringify!($name),
                result.err(),
            );
        }
    };
}

assert_pipeline_ok!(pipeline_ok_tweety, fixtures::tweety_triangle());
assert_pipeline_ok!(pipeline_ok_nixon, fixtures::nixon_diamond());
assert_pipeline_ok!(pipeline_ok_chain_1, fixtures::inheritance_chain(1));
assert_pipeline_ok!(pipeline_ok_chain_10, fixtures::inheritance_chain(10));
assert_pipeline_ok!(pipeline_ok_temporal, fixtures::temporal_theory());
assert_pipeline_ok!(pipeline_ok_modal, fixtures::modal_theory());
assert_pipeline_ok!(pipeline_ok_defeaters, fixtures::conflicting_defeaters());
assert_pipeline_ok!(pipeline_ok_empty, fixtures::empty_theory());
assert_pipeline_ok!(pipeline_ok_facts_only, fixtures::facts_only());

// ===========================================================================
// 5. Stage composability — running stages individually matches a pipeline
// ===========================================================================

#[test]
fn individual_stages_match_pipeline_tweety() {
    let theory = fixtures::tweety_triangle();
    run_individual_vs_pipeline(theory);
}

#[test]
fn individual_stages_match_pipeline_nixon() {
    let theory = fixtures::nixon_diamond();
    run_individual_vs_pipeline(theory);
}

#[test]
fn individual_stages_match_pipeline_chain() {
    let theory = fixtures::inheritance_chain(3);
    run_individual_vs_pipeline(theory);
}

#[test]
fn individual_stages_match_pipeline_temporal() {
    let theory = fixtures::temporal_theory();
    run_individual_vs_pipeline(theory);
}

#[test]
fn individual_stages_match_pipeline_modal() {
    let theory = fixtures::modal_theory();
    run_individual_vs_pipeline(theory);
}

#[test]
fn individual_stages_match_pipeline_defeaters() {
    let theory = fixtures::conflicting_defeaters();
    run_individual_vs_pipeline(theory);
}

#[test]
fn individual_stages_match_pipeline_empty() {
    let theory = fixtures::empty_theory();
    run_individual_vs_pipeline(theory);
}

#[test]
fn individual_stages_match_pipeline_facts_only() {
    let theory = fixtures::facts_only();
    run_individual_vs_pipeline(theory);
}

/// Apply the three default stages individually (Validate, WildcardRewrite,
/// Ground) and compare the resulting theory + reasoning conclusions against
/// the single-shot `Pipeline::default_pipeline().run()` path.
fn run_individual_vs_pipeline(theory: Theory) {
    // Path A: individual stages
    let validate = Validate::default();
    let wildcard = WildcardRewrite;
    let ground = Ground::default();

    let mut ctx = PipelineContext::default();

    let t1 = validate
        .apply(theory.clone(), &mut ctx)
        .expect("validate should succeed");
    let t2 = wildcard
        .apply(t1, &mut ctx)
        .expect("wildcard rewrite should succeed");
    let t3 = ground.apply(t2, &mut ctx).expect("ground should succeed");

    // Path B: Pipeline::default_pipeline()
    let (pipeline_theory, _) = run_default_pipeline(&theory);

    // Compare rule counts
    assert_eq!(
        t3.rule_count(),
        pipeline_theory.rule_count(),
        "Individual stages should produce the same rule count as the pipeline"
    );

    // Compare reasoning output
    let conclusions_individual = reason_prepared(&t3).unwrap();
    let conclusions_pipeline = reason_prepared(&pipeline_theory).unwrap();

    assert_eq!(
        normalised_conclusions(&conclusions_individual),
        normalised_conclusions(&conclusions_pipeline),
        "Reasoning on individually-staged theory should match pipeline theory"
    );
}

// ===========================================================================
// 6. Diagnostics and metadata verification
// ===========================================================================

#[test]
fn pipeline_emits_validation_diagnostic() {
    let theory = fixtures::tweety_triangle();
    let (_, ctx) = run_default_pipeline(&theory);

    let has_validation_info = ctx
        .diagnostics
        .iter()
        .any(|d| d.stage == "validate" && d.severity == Severity::Info);
    assert!(
        has_validation_info,
        "Pipeline should emit a validation info diagnostic; got: {:?}",
        ctx.diagnostics,
    );
}

#[test]
fn pipeline_populates_grounding_metadata() {
    let theory = fixtures::tweety_triangle();
    let (_, ctx) = run_default_pipeline(&theory);

    // The grounding stage should have set grounding_performed
    let performed = ctx
        .metadata
        .get("grounding_performed")
        .and_then(|v| match v {
            MetadataVal::Bool(b) => Some(*b),
            _ => None,
        });
    assert_eq!(
        performed,
        Some(false),
        "grounding_performed metadata should be false for a ground theory"
    );

    // grounding_had_variables should be false for ground fixtures
    let had_vars = ctx
        .metadata
        .get("grounding_had_variables")
        .and_then(|v| match v {
            MetadataVal::Bool(b) => Some(*b),
            _ => None,
        });
    assert_eq!(
        had_vars,
        Some(false),
        "tweety triangle has no variables, so grounding_had_variables should be false"
    );
}

#[test]
fn pipeline_grounding_instances_zero_for_ground_theory() {
    let theory = fixtures::facts_only();
    let (_, ctx) = run_default_pipeline(&theory);

    let instances = ctx
        .metadata
        .get("grounding_instances")
        .and_then(|v| match v {
            MetadataVal::Usize(n) => Some(*n),
            _ => None,
        });
    assert_eq!(
        instances,
        Some(0),
        "Ground theory should produce 0 grounding instances"
    );
}

#[test]
fn pipeline_grounding_limit_not_hit_for_simple_theory() {
    let theory = fixtures::inheritance_chain(5);
    let (_, ctx) = run_default_pipeline(&theory);

    let limit_hit = ctx
        .metadata
        .get("grounding_limit_hit")
        .and_then(|v| match v {
            MetadataVal::Bool(b) => Some(*b),
            _ => None,
        });
    assert_eq!(
        limit_hit,
        Some(false),
        "Simple inheritance chain should not hit grounding limit"
    );
}

#[test]
fn temporal_pipeline_populates_evaluated_at_metadata() {
    let theory = fixtures::temporal_theory();
    let ref_time = TimePoint::from_millis(150);

    let pipeline = Pipeline::builder()
        .stage(TemporalFilter {
            reference_time: ref_time,
        })
        .stage(Validate::default())
        .stage(WildcardRewrite)
        .stage(Ground::default())
        .build();
    let (_, ctx) = pipeline.run(theory).unwrap();

    let eval_at = ctx.metadata.get("evaluated_at").and_then(|v| match v {
        MetadataVal::TimePoint(tp) => Some(*tp),
        _ => None,
    });
    assert_eq!(
        eval_at,
        Some(ref_time),
        "Temporal pipeline should set evaluated_at metadata"
    );
}

#[test]
fn prepare_grounding_report_matches_pipeline_metadata() {
    let theory = fixtures::tweety_triangle();

    // prepare() constructs a GroundingReport from context metadata
    let result = prepare(&theory, PrepareOptions::default()).unwrap();

    let (_, ctx) = run_default_pipeline(&theory);

    let pipeline_performed = ctx
        .metadata
        .get("grounding_performed")
        .and_then(|v| match v {
            MetadataVal::Bool(b) => Some(*b),
            _ => None,
        })
        .unwrap_or(false);

    assert_eq!(
        result.grounding_report.performed, pipeline_performed,
        "GroundingReport.performed should match pipeline metadata"
    );
}

// ===========================================================================
// 7. Equivalence with reason() convenience function
// ===========================================================================

macro_rules! assert_reason_convenience_equivalence {
    ($name:ident, $fixture:expr) => {
        #[test]
        fn $name() {
            let theory = $fixture;

            // Path A: reason() (which calls prepare + reason_prepared internally)
            let conclusions_a = reason(&theory).expect("reason() should succeed");

            // Path B: Pipeline::default_pipeline().run() + reason_prepared()
            let (pipeline_theory, _) = run_default_pipeline(&theory);
            let conclusions_b =
                reason_prepared(&pipeline_theory).expect("reason_prepared should succeed");

            assert_eq!(
                normalised_conclusions(&conclusions_a),
                normalised_conclusions(&conclusions_b),
                "reason() should match Pipeline::default_pipeline() + reason_prepared() for {}",
                stringify!($name),
            );
        }
    };
}

assert_reason_convenience_equivalence!(reason_api_equiv_tweety, fixtures::tweety_triangle());
assert_reason_convenience_equivalence!(reason_api_equiv_nixon, fixtures::nixon_diamond());
assert_reason_convenience_equivalence!(reason_api_equiv_chain, fixtures::inheritance_chain(5));
assert_reason_convenience_equivalence!(reason_api_equiv_temporal, fixtures::temporal_theory());
assert_reason_convenience_equivalence!(reason_api_equiv_modal, fixtures::modal_theory());
assert_reason_convenience_equivalence!(
    reason_api_equiv_defeaters,
    fixtures::conflicting_defeaters()
);
assert_reason_convenience_equivalence!(reason_api_equiv_empty, fixtures::empty_theory());
assert_reason_convenience_equivalence!(reason_api_equiv_facts_only, fixtures::facts_only());

// ===========================================================================
// 8. Builder API — stage ordering and insertion
// ===========================================================================

#[test]
fn builder_stage_at_inserts_correctly() {
    let theory = fixtures::tweety_triangle();

    // Build a pipeline where Ground is inserted before WildcardRewrite,
    // then WildcardRewrite is appended. This exercises the `stage_at` API.
    let pipeline = Pipeline::builder()
        .stage(Validate::default())
        .stage(Ground::default())
        .stage_at(1, WildcardRewrite) // insert WildcardRewrite at index 1 (after validate)
        .build();

    let result = pipeline.run(theory);
    assert!(
        result.is_ok(),
        "Pipeline with stage_at insertion should not error: {:?}",
        result.err(),
    );
}

#[test]
fn empty_pipeline_returns_theory_unchanged() {
    let theory = fixtures::tweety_triangle();
    let original_count = theory.rule_count();

    let pipeline = Pipeline::builder().build();
    let (result_theory, ctx) = pipeline.run(theory).unwrap();

    assert_eq!(
        result_theory.rule_count(),
        original_count,
        "An empty pipeline should not modify the theory"
    );
    assert!(
        ctx.diagnostics.is_empty(),
        "An empty pipeline should emit no diagnostics"
    );
    assert!(
        ctx.metadata.is_empty(),
        "An empty pipeline should set no metadata"
    );
}

// ===========================================================================
// 9. Partial pipeline — validate-only, ground-only
// ===========================================================================

#[test]
fn validate_only_pipeline_does_not_change_rule_count() {
    let theory = fixtures::tweety_triangle();
    let original_count = theory.rule_count();

    let pipeline = Pipeline::builder().stage(Validate::default()).build();
    let (result, _) = pipeline.run(theory).unwrap();

    assert_eq!(
        result.rule_count(),
        original_count,
        "Validate-only pipeline should not change rule count"
    );
}

#[test]
fn wildcard_only_pipeline_preserves_ground_theory() {
    let theory = fixtures::nixon_diamond();
    let original_count = theory.rule_count();

    let pipeline = Pipeline::builder().stage(WildcardRewrite).build();
    let (result, _) = pipeline.run(theory).unwrap();

    assert_eq!(
        result.rule_count(),
        original_count,
        "WildcardRewrite on a ground theory should not change rule count"
    );
}

#[test]
fn ground_only_pipeline_on_ground_theory_is_identity() {
    let theory = fixtures::tweety_triangle();
    let original_count = theory.rule_count();

    let pipeline = Pipeline::builder().stage(Ground::default()).build();
    let (result, ctx) = pipeline.run(theory).unwrap();

    assert_eq!(
        result.rule_count(),
        original_count,
        "Ground stage on a fully-ground theory should not change rule count"
    );

    // Should report no variables
    let had_vars = ctx
        .metadata
        .get("grounding_had_variables")
        .and_then(|v| match v {
            MetadataVal::Bool(b) => Some(*b),
            _ => None,
        });
    assert_eq!(had_vars, Some(false));
}

// ===========================================================================
// 10. reason_with_options temporal matches custom pipeline temporal
// ===========================================================================

#[test]
fn reason_with_options_temporal_matches_custom_pipeline() {
    let theory = fixtures::temporal_theory();
    let ref_time = TimePoint::from_millis(150);

    // Path A: reason_with_options
    let opts = PrepareOptions {
        reference_time: Some(ref_time),
        ..Default::default()
    };
    let conclusions_a = reason_with_options(&theory, opts).unwrap();

    // Path B: custom pipeline + reason_prepared
    let pipeline = Pipeline::builder()
        .stage(TemporalFilter {
            reference_time: ref_time,
        })
        .stage(Validate::default())
        .stage(WildcardRewrite)
        .stage(Ground::default())
        .build();
    let (prepared, _) = pipeline.run(theory).unwrap();
    let conclusions_b = reason_prepared(&prepared).unwrap();

    assert_eq!(
        normalised_conclusions(&conclusions_a),
        normalised_conclusions(&conclusions_b),
        "reason_with_options() should match custom temporal pipeline"
    );
}

// ===========================================================================
// 11. Deeper inheritance chain stress test
// ===========================================================================

#[test]
fn pipeline_handles_deep_chain() {
    let theory = fixtures::inheritance_chain(50);

    let (prepared, _) = run_default_pipeline(&theory);
    let conclusions = reason_prepared(&prepared).unwrap();

    // The chain should propagate through all 50 levels
    let has_p50 = conclusions.iter().any(|c| {
        c.conclusion_type == ConclusionType::DefeasiblyProvable && format!("{}", c.literal) == "p50"
    });

    assert!(
        has_p50,
        "inheritance_chain(50) should derive p50 through the pipeline"
    );
}

// ===========================================================================
// 12. PipelineResult evaluated_at matches reference_time
// ===========================================================================

#[test]
fn prepare_result_evaluated_at_matches_reference_time() {
    let theory = fixtures::temporal_theory();
    let ref_time = TimePoint::from_millis(150);

    let opts = PrepareOptions {
        reference_time: Some(ref_time),
        ..Default::default()
    };
    let result = prepare(&theory, opts).unwrap();

    assert_eq!(
        result.evaluated_at,
        Some(ref_time),
        "PipelineResult.evaluated_at should match the provided reference_time"
    );
}

#[test]
fn prepare_result_evaluated_at_none_without_reference_time() {
    let theory = fixtures::tweety_triangle();

    let result = prepare(&theory, PrepareOptions::default()).unwrap();

    assert_eq!(
        result.evaluated_at, None,
        "PipelineResult.evaluated_at should be None when no reference_time is provided"
    );
}

// ===========================================================================
// 13. Diagnostic collection across multiple stages
// ===========================================================================

#[test]
fn pipeline_collects_diagnostics_from_all_stages() {
    let theory = fixtures::tweety_triangle();

    let (_, ctx) = run_default_pipeline(&theory);

    // We expect at least diagnostics from validate and ground stages
    let stages: BTreeSet<&str> = ctx.diagnostics.iter().map(|d| d.stage).collect();

    assert!(
        stages.contains("validate"),
        "Pipeline should have a diagnostic from the validate stage; stages: {stages:?}",
    );
    assert!(
        stages.contains("ground"),
        "Pipeline should have a diagnostic from the ground stage; stages: {stages:?}",
    );
}

#[test]
fn temporal_pipeline_emits_temporal_diagnostic_when_rules_removed() {
    let theory = fixtures::temporal_theory();
    let ref_time = TimePoint::from_millis(300); // outside [100, 200]

    let pipeline = Pipeline::builder()
        .stage(TemporalFilter {
            reference_time: ref_time,
        })
        .build();
    let (_, ctx) = pipeline.run(theory).unwrap();

    let has_temporal_info = ctx
        .diagnostics
        .iter()
        .any(|d| d.stage == "temporal_filter" && d.severity == Severity::Info);
    assert!(
        has_temporal_info,
        "Temporal filter at t=300 should emit an info diagnostic about removed rules; got: {:?}",
        ctx.diagnostics,
    );
}

// ===========================================================================
// Declarations survive theory reconstruction (SPEC-024 CON-008)
// ===========================================================================

#[test]
fn predicate_declarations_survive_default_pipeline() {
    use spindle_core::vocabulary::{
        ArgumentDecl, DeclarationOrigin, PredicateDeclaration, PredicateSignature, PredicateSymbol,
        PrimitiveSort, Vocabulary, VocabularyDiagnostic,
    };

    // A theory with a used declared predicate and an unused declared predicate,
    // run through the default pipeline (wildcard rewrite + grounding).
    let mut theory = Theory::new();
    theory.add_fact("bird");
    theory.add_defeasible_rule(&["bird"], "flies");
    let symbol = PredicateSymbol::try_new("flies".into(), 0).unwrap();
    let sig = PredicateSignature::try_new(symbol, vec![]).unwrap();
    theory.add_predicate_declaration(PredicateDeclaration::new(
        sig,
        DeclarationOrigin::Programmatic,
    ));
    let declared = PredicateSymbol::try_new("who".into(), 1).unwrap();
    let sig = PredicateSignature::try_new(
        declared,
        vec![ArgumentDecl::new("x", PrimitiveSort::Symbol)],
    )
    .unwrap();
    theory.add_predicate_declaration(PredicateDeclaration::new(
        sig,
        DeclarationOrigin::Programmatic,
    ));
    let result = prepare(&theory, PrepareOptions::default()).unwrap();

    // Declarations are carried through every reconstruction stage.
    assert_eq!(result.theory.predicate_declarations().len(), 2);

    // The declared symbol stays declared in the derived vocabulary — it is not
    // downgraded to UndeclaredPredicate by preparation.
    let report = Vocabulary::derive(&result.theory);
    assert!(!report.diagnostics.iter().any(|d| matches!(
        d,
        VocabularyDiagnostic::UndeclaredPredicate { symbol } if *symbol == PredicateSymbol::try_new("flies".into(), 0).unwrap()
    )));
    let entry = report
        .vocabulary
        .entries
        .iter()
        .find(|e| e.symbol == declared)
        .expect("declared-but-unused symbol still catalogued after prepare");
    assert!(entry.declaration.is_some());
}

#[test]
fn temporal_filter_preserves_declarations() {
    use spindle_core::vocabulary::{
        DeclarationOrigin, PredicateDeclaration, PredicateSignature, PredicateSymbol,
    };

    let mut theory = fixtures::temporal_theory();
    let symbol = PredicateSymbol::try_new("declared-only".into(), 0).unwrap();
    let sig = PredicateSignature::try_new(symbol, vec![]).unwrap();
    theory.add_predicate_declaration(PredicateDeclaration::new(
        sig,
        DeclarationOrigin::Programmatic,
    ));

    let pipeline = Pipeline::builder()
        .stage(TemporalFilter {
            reference_time: TimePoint::from_millis(300),
        })
        .build();
    let (filtered, _) = pipeline.run(theory).unwrap();
    assert_eq!(filtered.predicate_declarations().len(), 1);
}
