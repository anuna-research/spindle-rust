//! Integration tests for trust policy end-to-end: SPL parsing → pipeline → reasoning → weighted conclusions.

use spindle_core::conclusion::ConclusionType;
use spindle_core::pipeline::{PrepareOptions, compute_weighted_conclusions, prepare};
use spindle_core::reason::reason_prepared;
use spindle_core::temporal::TimePoint;
use spindle_core::trust::{Source, WeightedConclusion};
use spindle_parser::parse_spl;

// 2024-01-01T00:00:00Z in milliseconds
const T0: i64 = 1_704_067_200_000;

/// Helper: parse SPL, prepare, reason, compute weighted conclusions.
fn weighted_from_spl(spl: &str, reference_time: Option<TimePoint>) -> Vec<WeightedConclusion> {
    let theory = parse_spl(spl).expect("SPL should parse");
    let opts = PrepareOptions {
        reference_time,
        ..Default::default()
    };
    let result = prepare(&theory, opts).expect("prepare should succeed");
    let policy = result.theory.trust_policy().clone();
    let conclusions = reason_prepared(&result.theory).expect("reasoning should succeed");
    compute_weighted_conclusions(&conclusions, &result.theory, &policy, reference_time)
}

/// Find a weighted conclusion by literal name and conclusion type.
fn find_wc<'a>(
    wcs: &'a [WeightedConclusion],
    name: &str,
    ctype: ConclusionType,
) -> Option<&'a WeightedConclusion> {
    wcs.iter()
        .find(|wc| wc.literal.name() == name && wc.conclusion_type == ctype)
}

// ---------------------------------------------------------------------------
// Basic trust: (trusts) + (claims) directives
// ---------------------------------------------------------------------------

#[test]
fn test_single_source_fact_trust() {
    let spl = r#"
        (trusts alice 0.8)
        (claims alice (given bird))
    "#;
    let wcs = weighted_from_spl(spl, None);
    let wc =
        find_wc(&wcs, "bird", ConclusionType::DefinitelyProvable).expect("+D bird should exist");
    assert!(
        (wc.degree - 0.8).abs() < 1e-10,
        "Expected trust 0.8, got {}",
        wc.degree
    );
    assert!(wc.sources.contains(&Source::new("alice")));
}

#[test]
fn test_two_sources_independent_facts() {
    let spl = r#"
        (trusts alice 0.9)
        (trusts bob 0.4)
        (claims alice (given bird))
        (claims bob (given fish))
    "#;
    let wcs = weighted_from_spl(spl, None);

    let bird = find_wc(&wcs, "bird", ConclusionType::DefinitelyProvable).unwrap();
    assert!(
        (bird.degree - 0.9).abs() < 1e-10,
        "bird trust should be 0.9, got {}",
        bird.degree
    );

    let fish = find_wc(&wcs, "fish", ConclusionType::DefinitelyProvable).unwrap();
    assert!(
        (fish.degree - 0.4).abs() < 1e-10,
        "fish trust should be 0.4, got {}",
        fish.degree
    );
}

// ---------------------------------------------------------------------------
// Weakest-link propagation through derivation chains
// ---------------------------------------------------------------------------

#[test]
fn test_weakest_link_two_step_chain() {
    // alice (trust 0.6) claims bird. bob (trust 0.9) provides rule bird => flies.
    // flies should get min(0.6, 0.9) = 0.6
    let spl = r#"
        (trusts alice 0.6)
        (trusts bob 0.9)
        (claims alice (given bird))
        (claims bob (normally r1 (bird) (flies)))
    "#;
    let wcs = weighted_from_spl(spl, None);
    let flies =
        find_wc(&wcs, "flies", ConclusionType::DefeasiblyProvable).expect("+d flies should exist");
    assert!(
        (flies.degree - 0.6).abs() < 1e-10,
        "Expected weakest-link 0.6, got {}",
        flies.degree
    );
    assert!(flies.sources.contains(&Source::new("alice")));
    assert!(flies.sources.contains(&Source::new("bob")));
}

#[test]
fn test_weakest_link_three_step_chain() {
    // a(0.9) → b(0.4) → c(0.8) ⇒ min = 0.4
    let spl = r#"
        (trusts s1 0.9)
        (trusts s2 0.4)
        (trusts s3 0.8)
        (claims s1 (given a))
        (claims s2 (normally r1 (a) (b)))
        (claims s3 (normally r2 (b) (c)))
    "#;
    let wcs = weighted_from_spl(spl, None);
    let c = find_wc(&wcs, "c", ConclusionType::DefeasiblyProvable).expect("+d c should exist");
    assert!(
        (c.degree - 0.4).abs() < 1e-10,
        "Expected weakest-link 0.4, got {}",
        c.degree
    );
    assert_eq!(c.sources.len(), 3);
}

// ---------------------------------------------------------------------------
// Thresholds
// ---------------------------------------------------------------------------

#[test]
fn test_threshold_evaluation() {
    let spl = r#"
        (trusts alice 0.4)
        (threshold action 0.7)
        (threshold warn 0.3)
        (claims alice (given bird))
    "#;
    let wcs = weighted_from_spl(spl, None);
    let bird = find_wc(&wcs, "bird", ConclusionType::DefinitelyProvable).unwrap();
    assert_eq!(
        bird.is_above_threshold("action"),
        Some(false),
        "0.4 should be below action threshold 0.7"
    );
    assert_eq!(
        bird.is_above_threshold("warn"),
        Some(true),
        "0.4 should be above warn threshold 0.3"
    );
}

#[test]
fn test_threshold_with_weakest_link() {
    // High-trust rule derives from low-trust fact → weakest link below threshold
    let spl = r#"
        (trusts low 0.3)
        (trusts high 0.95)
        (threshold action 0.5)
        (claims low (given premise))
        (claims high (normally r1 (premise) (conclusion)))
    "#;
    let wcs = weighted_from_spl(spl, None);
    let conc = find_wc(&wcs, "conclusion", ConclusionType::DefeasiblyProvable).unwrap();
    assert!(
        (conc.degree - 0.3).abs() < 1e-10,
        "Weakest link should be 0.3, got {}",
        conc.degree
    );
    assert_eq!(
        conc.is_above_threshold("action"),
        Some(false),
        "0.3 should be below action threshold 0.5"
    );
}

// ---------------------------------------------------------------------------
// Decay models with :at timestamps
// ---------------------------------------------------------------------------

#[test]
fn test_exponential_decay_halves_at_half_life() {
    // Claim at 2024-01-01T00:00:00Z, reference 1 hour later.
    // Half-life = 3600s → multiplier = 0.5, base trust = 0.8 → effective = 0.4
    let spl = r#"
        (trusts alice 0.8)
        (decays alice exponential 3600)
        (claims alice :at "2024-01-01T00:00:00Z" (given bird))
    "#;
    let ref_time = Some(TimePoint::from_millis(T0 + 3_600_000)); // +1 hour
    let wcs = weighted_from_spl(spl, ref_time);
    let bird = find_wc(&wcs, "bird", ConclusionType::DefinitelyProvable).unwrap();
    assert!(
        (bird.degree - 0.4).abs() < 1e-6,
        "Expected 0.8 * 0.5 = 0.4 after one half-life, got {}",
        bird.degree
    );
}

#[test]
fn test_exponential_decay_no_decay_at_same_time() {
    // Claim and reference at the same time → no decay → full trust
    let spl = r#"
        (trusts alice 0.8)
        (decays alice exponential 3600)
        (claims alice :at "2024-01-01T00:00:00Z" (given bird))
    "#;
    let ref_time = Some(TimePoint::from_millis(T0)); // same time
    let wcs = weighted_from_spl(spl, ref_time);
    let bird = find_wc(&wcs, "bird", ConclusionType::DefinitelyProvable).unwrap();
    assert!(
        (bird.degree - 0.8).abs() < 1e-6,
        "Expected 0.8 (no decay at age 0), got {}",
        bird.degree
    );
}

#[test]
fn test_linear_decay() {
    // Base trust 1.0, linear decay at 0.001/sec, age = 500s → multiplier = 0.5
    let spl = r#"
        (trusts sensor 1.0)
        (decays sensor linear 0.001)
        (claims sensor :at "2024-01-01T00:00:00Z" (given temperature_ok))
    "#;
    let ref_time = Some(TimePoint::from_millis(T0 + 500_000)); // +500s
    let wcs = weighted_from_spl(spl, ref_time);
    let temp = find_wc(&wcs, "temperature_ok", ConclusionType::DefinitelyProvable).unwrap();
    assert!(
        (temp.degree - 0.5).abs() < 1e-6,
        "Expected 1.0 * 0.5 = 0.5, got {}",
        temp.degree
    );
}

#[test]
fn test_step_decay_before_cutoff() {
    // Step function with 1-day cutoff. Claim 1 hour old → still valid (multiplier 1.0)
    let spl = r#"
        (trusts agent 0.7)
        (decays agent step 86400)
        (claims agent :at "2024-01-01T00:00:00Z" (given active))
    "#;
    let ref_time = Some(TimePoint::from_millis(T0 + 3_600_000)); // +1 hour
    let wcs = weighted_from_spl(spl, ref_time);
    let active = find_wc(&wcs, "active", ConclusionType::DefinitelyProvable).unwrap();
    assert!(
        (active.degree - 0.7).abs() < 1e-6,
        "Expected 0.7 (before cutoff), got {}",
        active.degree
    );
}

#[test]
fn test_step_decay_after_cutoff() {
    // Step function with 1-day cutoff. Claim 2 days old → expired (multiplier 0.0)
    let spl = r#"
        (trusts agent 0.7)
        (decays agent step 86400)
        (claims agent :at "2024-01-01T00:00:00Z" (given active))
    "#;
    let ref_time = Some(TimePoint::from_millis(T0 + 2 * 86_400_000)); // +2 days
    let wcs = weighted_from_spl(spl, ref_time);
    let active = find_wc(&wcs, "active", ConclusionType::DefinitelyProvable).unwrap();
    assert!(
        active.degree.abs() < 1e-6,
        "Expected 0.0 (past cutoff), got {}",
        active.degree
    );
}

// ---------------------------------------------------------------------------
// Decay + weakest-link interaction
// ---------------------------------------------------------------------------

#[test]
fn test_decay_weakest_link_chain() {
    // Old claim (trust 0.8 decayed to ~0.4) feeds into fresh rule (trust 0.9, no decay)
    // Weakest link = min(0.4, 0.9) = 0.4
    let spl = r#"
        (trusts alice 0.8)
        (trusts bob 0.9)
        (decays alice exponential 3600)
        (claims alice :at "2024-01-01T00:00:00Z" (given bird))
        (claims bob (normally r1 (bird) (flies)))
    "#;
    let ref_time = Some(TimePoint::from_millis(T0 + 3_600_000)); // +1 hour
    let wcs = weighted_from_spl(spl, ref_time);
    let flies =
        find_wc(&wcs, "flies", ConclusionType::DefeasiblyProvable).expect("+d flies should exist");
    // alice decayed: 0.8 * 0.5 = 0.4; bob no timestamp → no decay → 0.9
    // weakest link = 0.4
    assert!(
        (flies.degree - 0.4).abs() < 1e-6,
        "Expected weakest-link 0.4 (decayed alice), got {}",
        flies.degree
    );
}

// ---------------------------------------------------------------------------
// No trust directives → default trust (0.0 per TrustPolicy derive(Default))
// ---------------------------------------------------------------------------

#[test]
fn test_no_trust_directives_uses_default() {
    let spl = r#"
        (given bird)
    "#;
    let wcs = weighted_from_spl(spl, None);
    let bird =
        find_wc(&wcs, "bird", ConclusionType::DefinitelyProvable).expect("+D bird should exist");
    assert!(
        bird.degree.abs() < 1e-10,
        "Expected default 0.0, got {}",
        bird.degree
    );
}

// ---------------------------------------------------------------------------
// Trust preserved through grounding
// ---------------------------------------------------------------------------

#[test]
fn test_trust_preserved_through_grounding() {
    // Grounded theory: variable rule should still carry trust via template_label fallback
    let spl = r#"
        (trusts alice 0.7)
        (trusts bob 0.85)
        (claims alice (given (p a)))
        (claims bob (normally r1 (p ?X) (q ?X)))
    "#;
    let wcs = weighted_from_spl(spl, None);
    // q(a) should be derived via grounded r1, with weakest-link = min(0.7, 0.85) = 0.7
    let q = wcs
        .iter()
        .find(|wc| {
            wc.literal.name() == "q"
                && wc.conclusion_type == ConclusionType::DefeasiblyProvable
                && wc.literal.predicates().iter().any(|s| s == "a")
        })
        .expect("+d q(a) should exist");
    assert!(
        (q.degree - 0.7).abs() < 1e-10,
        "Expected weakest-link 0.7 through grounding, got {}",
        q.degree
    );
    assert!(q.sources.contains(&Source::new("alice")));
    assert!(q.sources.contains(&Source::new("bob")));
}

// ---------------------------------------------------------------------------
// Multiple sources for one conclusion
// ---------------------------------------------------------------------------

#[test]
fn test_source_collection_across_chain() {
    let spl = r#"
        (trusts s1 0.9)
        (trusts s2 0.8)
        (trusts s3 0.7)
        (claims s1 (given a))
        (claims s2 (normally r1 (a) (b)))
        (claims s3 (normally r2 (b) (c)))
    "#;
    let wcs = weighted_from_spl(spl, None);
    let c = find_wc(&wcs, "c", ConclusionType::DefeasiblyProvable).unwrap();
    assert_eq!(c.sources.len(), 3, "All three chain sources should appear");
    assert!(c.sources.contains(&Source::new("s1")));
    assert!(c.sources.contains(&Source::new("s2")));
    assert!(c.sources.contains(&Source::new("s3")));
}

// ---------------------------------------------------------------------------
// Negative conclusions get default trust
// ---------------------------------------------------------------------------

#[test]
fn test_negative_conclusion_default_trust() {
    let spl = r#"
        (trusts alice 0.9)
        (claims alice (given bird))
        (normally r1 (fish) (swims))
    "#;
    let wcs = weighted_from_spl(spl, None);
    // fish is not provable, so -d fish should exist with default trust (0.0)
    let neg = wcs
        .iter()
        .find(|wc| wc.literal.name() == "fish" && !wc.conclusion_type.is_positive());
    assert!(neg.is_some(), "-d fish or -D fish should exist");
    assert!(
        neg.unwrap().degree.abs() < 1e-10,
        "Negative conclusion should get default trust 0.0, got {}",
        neg.unwrap().degree
    );
}

// ---------------------------------------------------------------------------
// Multiple derivations of the same literal
// ---------------------------------------------------------------------------

#[test]
fn test_multiple_derivations_picks_strongest_proof() {
    // "a" is proven both as a fact (+D, from alice at 0.6) and via a defeasible rule (+d, from bob at 0.9).
    // A downstream rule from charlie (0.95) uses "a" in its body.
    // The trust tree for "result" should follow the +D proof of "a" (stronger type),
    // giving weakest-link = min(0.6, 0.95) = 0.6, not min(0.9, ..., 0.95).
    let spl = r#"
        (trusts alice 0.6)
        (trusts bob 0.9)
        (trusts charlie 0.95)
        (claims alice (given a))
        (claims bob (normally r_alt (b) (a)))
        (claims bob (given b))
        (claims charlie (normally r_use (a) (result)))
    "#;
    let wcs = weighted_from_spl(spl, None);
    let result = find_wc(&wcs, "result", ConclusionType::DefeasiblyProvable)
        .expect("+d result should exist");
    // "a" has +D (from alice's fact, trust 0.6) and +d (from bob's rule, trust min(0.9, 0.9)=0.9)
    // best_conclusion picks +D. Chain: result(charlie 0.95) <- a(alice 0.6) → min = 0.6
    assert!(
        (result.degree - 0.6).abs() < 1e-10,
        "Expected 0.6 (weakest-link via +D proof of a), got {}",
        result.degree
    );
    assert!(result.sources.contains(&Source::new("alice")));
    assert!(result.sources.contains(&Source::new("charlie")));
}
