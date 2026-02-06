//! Performance benchmarks for spindle-core
//!
//! Run with:
//!   cargo bench --package spindle-core              # Quick benchmarks
//!   cargo bench --package spindle-core -- --ignored # Full benchmarks (slower)
//!
//! Or filter specific groups:
//!   cargo bench -- "standard_vs_scalable"
//!
//! Benchmarks cover:
//! - Literal operations (ID vs string lookups)
//! - Superiority index lookups
//! - Standard reasoning (facts, chains, conflicts, wide theories)
//! - Scalable reasoning algorithm
//! - Temporal reasoning (Allen interval relations)
//! - Variable grounding
//! - Query operators (what-if, why-not, abduction)
//! - Explanation generation
//! - Parser performance (DFL and SPL formats)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use spindle_core::explanation::explain;
use spindle_core::grounding::ground_theory;
use spindle_core::mode::Mode;
use spindle_core::prelude::*;
use spindle_core::query::{abduce, what_if, why_not, HypotheticalClaim};
use spindle_core::reason::reason;
use spindle_core::scalable::reason_scalable;
use spindle_core::temporal::{Temporal, TimePoint};
use spindle_core::trust::{Source, TrustDerivationNode, TrustPolicy};
use spindle_parser::{parse_dfl, parse_spl};
use std::collections::HashSet;
use std::time::Duration;

// =============================================================================
// THEORY GENERATORS
// =============================================================================

/// Create a theory with N facts
fn create_fact_theory(n: usize) -> Theory {
    let mut theory = Theory::new();
    for i in 0..n {
        let fact = Rule::fact(format!("f{}", i), Literal::simple(format!("prop{}", i)));
        theory.add_rule(fact);
    }
    theory
}

/// Create a chain theory: p0 => p1, p1 => p2, ..., p(n-1) => pn
fn create_chain_theory(n: usize) -> Theory {
    let mut theory = Theory::new();

    // Initial fact
    theory.add_rule(Rule::fact("f0", Literal::simple("p0")));

    // Chain of defeasible rules
    for i in 0..n {
        let rule = Rule::defeasible(
            format!("r{}", i),
            vec![Literal::simple(format!("p{}", i))],
            Literal::simple(format!("p{}", i + 1)),
        );
        theory.add_rule(rule);
    }
    theory
}

/// Create a wide theory with many independent rules
fn create_wide_theory(n: usize) -> Theory {
    let mut theory = Theory::new();

    for i in 0..n {
        // Each group has a fact and two conflicting rules
        let base = format!("base{}", i);
        let prop = format!("prop{}", i);

        theory.add_rule(Rule::fact(format!("f{}", i), Literal::simple(&base)));

        // r_i: base_i => prop_i
        theory.add_rule(Rule::defeasible(
            format!("r{}", i),
            vec![Literal::simple(&base)],
            Literal::simple(&prop),
        ));

        // s_i: base_i => ~prop_i
        theory.add_rule(Rule::defeasible(
            format!("s{}", i),
            vec![Literal::simple(&base)],
            Literal::negated(&prop),
        ));

        // r_i > s_i (resolve conflict)
        theory.add_superiority(&format!("r{}", i), &format!("s{}", i));
    }
    theory
}

/// Create a conflict theory with N conflicting rule pairs
fn create_conflict_theory(n: usize) -> Theory {
    let mut theory = Theory::new();

    // Base fact
    theory.add_rule(Rule::fact("f0", Literal::simple("base")));

    for i in 0..n {
        let prop = format!("prop{}", i);

        // r_i: base => prop_i
        theory.add_rule(Rule::defeasible(
            format!("r{}", i),
            vec![Literal::simple("base")],
            Literal::simple(&prop),
        ));

        // s_i: base => ~prop_i
        theory.add_rule(Rule::defeasible(
            format!("s{}", i),
            vec![Literal::simple("base")],
            Literal::negated(&prop),
        ));

        // r_i > s_i
        theory.add_superiority(&format!("r{}", i), &format!("s{}", i));
    }
    theory
}

/// Create a theory with variables for grounding benchmarks
fn create_grounding_theory(n_entities: usize, n_rules: usize) -> Theory {
    let mut theory = Theory::new();

    // Add ground facts: person(entity_i) for each entity
    for i in 0..n_entities {
        theory.add_rule(Rule::fact(
            format!("f_person_{}", i),
            Literal::new(
                "person",
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![format!("entity{}", i)],
            ),
        ));
    }

    // Add relationships between entities
    for i in 0..n_entities {
        for j in 0..n_entities {
            if i != j && (i + j) % 3 == 0 {
                theory.add_rule(Rule::fact(
                    format!("f_knows_{}_{}", i, j),
                    Literal::new(
                        "knows",
                        false,
                        Mode::empty(),
                        Temporal::empty(),
                        vec![format!("entity{}", i), format!("entity{}", j)],
                    ),
                ));
            }
        }
    }

    // Add rules with variables
    for i in 0..n_rules {
        // Rule: person(?x), knows(?x, ?y) => connected(?x, ?y)
        theory.add_rule(Rule::defeasible(
            format!("r_connected_{}", i),
            vec![
                Literal::new(
                    "person",
                    false,
                    Mode::empty(),
                    Temporal::empty(),
                    vec!["?x".to_string()],
                ),
                Literal::new(
                    "knows",
                    false,
                    Mode::empty(),
                    Temporal::empty(),
                    vec!["?x".to_string(), "?y".to_string()],
                ),
            ],
            Literal::new(
                format!("connected{}", i),
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?x".to_string(), "?y".to_string()],
            ),
        ));
    }

    theory
}

/// Create a theory suitable for query benchmarks (with blocked conclusions)
fn create_query_theory() -> Theory {
    let mut theory = Theory::new();

    // Software development scenario
    theory.add_rule(Rule::fact("f1", Literal::simple("code_written")));
    theory.add_rule(Rule::fact("f2", Literal::simple("has_tests")));
    // tests_pass is NOT a fact - simulates missing condition

    // r1: code_written, has_tests, tests_pass => ready_review
    theory.add_rule(Rule::defeasible(
        "r_review",
        vec![
            Literal::simple("code_written"),
            Literal::simple("has_tests"),
            Literal::simple("tests_pass"),
        ],
        Literal::simple("ready_review"),
    ));

    // r2: ready_review => can_merge
    theory.add_rule(Rule::defeasible(
        "r_merge",
        vec![Literal::simple("ready_review")],
        Literal::simple("can_merge"),
    ));

    // Conflicting rule
    theory.add_rule(Rule::defeasible(
        "r_no_merge",
        vec![Literal::simple("has_blocker")],
        Literal::negated("can_merge"),
    ));

    theory.add_superiority("r_no_merge", "r_merge");

    theory
}

/// Generate DFL format string for N rules
fn generate_dfl(n: usize) -> String {
    let mut dfl = String::new();

    // Facts
    for i in 0..n {
        dfl.push_str(&format!("f{}: >> prop{}\n", i, i));
    }

    // Rules
    for i in 0..n {
        dfl.push_str(&format!("r{}: prop{} => derived{}\n", i, i, i));
    }

    // Some superiorities
    for i in 0..n / 2 {
        dfl.push_str(&format!("r{} > r{}\n", i * 2, i * 2 + 1));
    }

    dfl
}

/// Generate SPL format string for N rules
fn generate_spl(n: usize) -> String {
    let mut spl = String::new();

    // Facts
    for i in 0..n {
        spl.push_str(&format!("(given prop{})\n", i));
    }

    // Rules
    for i in 0..n {
        spl.push_str(&format!("(normally r{} prop{} derived{})\n", i, i, i));
    }

    // Some superiorities
    for i in 0..n / 2 {
        spl.push_str(&format!("(prefer r{} r{})\n", i * 2, i * 2 + 1));
    }

    spl
}

// =============================================================================
// LITERAL BENCHMARKS
// =============================================================================

fn bench_literal_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("literal_ops");

    let lit = Literal::simple("test_literal_name");
    let neg_lit = Literal::negated("test_literal_name");

    group.bench_function("literal_id", |b| b.iter(|| black_box(lit.literal_id())));

    group.bench_function("literal_id_negated", |b| {
        b.iter(|| black_box(neg_lit.literal_id()))
    });

    group.bench_function("canonical_name", |b| {
        b.iter(|| black_box(lit.canonical_name()))
    });

    group.bench_function("canonical_name_negated", |b| {
        b.iter(|| black_box(neg_lit.canonical_name()))
    });

    // HashSet operations comparison
    let mut id_set: HashSet<LiteralId> = HashSet::new();
    let mut str_set: HashSet<String> = HashSet::new();

    for i in 0..1000 {
        let l = Literal::simple(format!("lit{}", i));
        id_set.insert(l.literal_id());
        str_set.insert(l.canonical_name());
    }

    group.bench_function("hashset_contains_literal_id", |b| {
        b.iter(|| black_box(id_set.contains(&lit.literal_id())))
    });

    group.bench_function("hashset_contains_canonical", |b| {
        b.iter(|| black_box(str_set.contains(&lit.canonical_name())))
    });

    group.finish();
}

// =============================================================================
// SUPERIORITY INDEX BENCHMARKS
// =============================================================================

fn bench_superiority_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("superiority");

    for size in [10, 100, 1000].iter() {
        let theory = create_conflict_theory(*size);

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("indexed_lookup", size), size, |b, _| {
            b.iter(|| {
                // Check all superiority relations
                for i in 0..*size {
                    black_box(theory.is_superior(&format!("r{}", i), &format!("s{}", i)));
                }
            })
        });
    }

    group.finish();
}

// =============================================================================
// REASONING BENCHMARKS
// =============================================================================

fn bench_reason_facts(c: &mut Criterion) {
    let mut group = c.benchmark_group("reason_facts");

    for size in [100, 500, 1_000].iter() {
        let theory = create_fact_theory(*size);
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("standard", size), &theory, |b, theory| {
            b.iter(|| black_box(reason(theory)))
        });
    }

    group.finish();
}

fn bench_reason_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("reason_chain");

    for size in [50, 100, 250].iter() {
        let theory = create_chain_theory(*size);
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("standard", size), &theory, |b, theory| {
            b.iter(|| black_box(reason(theory)))
        });
    }

    group.finish();
}

fn bench_reason_wide(c: &mut Criterion) {
    let mut group = c.benchmark_group("reason_wide");

    for size in [100, 250, 500].iter() {
        let theory = create_wide_theory(*size);
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("standard", size), &theory, |b, theory| {
            b.iter(|| black_box(reason(theory)))
        });
    }

    group.finish();
}

fn bench_reason_conflicts(c: &mut Criterion) {
    let mut group = c.benchmark_group("reason_conflicts");

    for size in [100, 250, 500].iter() {
        let theory = create_conflict_theory(*size);
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("standard", size), &theory, |b, theory| {
            b.iter(|| black_box(reason(theory)))
        });
    }

    group.finish();
}

// =============================================================================
// SCALABLE REASONING BENCHMARKS
// =============================================================================

fn bench_reason_scalable(c: &mut Criterion) {
    let mut group = c.benchmark_group("reason_scalable");

    for size in [100, 250, 500].iter() {
        let theory = create_wide_theory(*size);
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::new("scalable", size), &theory, |b, theory| {
            b.iter(|| {
                let grounded = ground_theory(theory);
                let indexed = spindle_core::index::IndexedTheory::build(&grounded);
                black_box(reason_scalable(&indexed))
            })
        });
    }

    group.finish();
}

fn bench_standard_vs_scalable(c: &mut Criterion) {
    let mut group = c.benchmark_group("standard_vs_scalable");

    // Quick comparison at key sizes
    for size in [100, 250, 500].iter() {
        let theory = create_wide_theory(*size);
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("standard", size), &theory, |b, theory| {
            b.iter(|| black_box(reason(theory)))
        });

        group.bench_with_input(BenchmarkId::new("scalable", size), &theory, |b, theory| {
            b.iter(|| {
                let grounded = ground_theory(theory);
                let indexed = spindle_core::index::IndexedTheory::build(&grounded);
                black_box(reason_scalable(&indexed))
            })
        });
    }

    group.finish();
}

/// Large-scale benchmarks for finding algorithm crossover points
/// Run with: cargo bench -- "scaling"
fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling");
    // Reduce samples for expensive benchmarks
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(3));

    for size in [500, 1_000, 2_000, 5_000].iter() {
        let theory = create_wide_theory(*size);
        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("standard", size), &theory, |b, theory| {
            b.iter(|| black_box(reason(theory)))
        });

        group.bench_with_input(BenchmarkId::new("scalable", size), &theory, |b, theory| {
            b.iter(|| {
                let grounded = ground_theory(theory);
                let indexed = spindle_core::index::IndexedTheory::build(&grounded);
                black_box(reason_scalable(&indexed))
            })
        });
    }

    group.finish();
}

// =============================================================================
// TEMPORAL REASONING BENCHMARKS
// =============================================================================

fn bench_temporal_relations(c: &mut Criterion) {
    let mut group = c.benchmark_group("temporal");

    // Create test intervals
    let intervals: Vec<Temporal> = (0..100)
        .map(|i| Temporal::from_bounds(i * 10, i * 10 + 15))
        .collect();

    let t1 = Temporal::from_bounds(0, 100);
    let t2 = Temporal::from_bounds(50, 150);

    group.bench_function("relation_check", |b| b.iter(|| black_box(t1.relation(&t2))));

    group.bench_function("intersection", |b| {
        b.iter(|| black_box(t1.intersection(&t2)))
    });

    group.bench_function("intersects", |b| b.iter(|| black_box(t1.intersects(&t2))));

    group.bench_function("active_at", |b| {
        let point = TimePoint::from_millis(75);
        b.iter(|| black_box(t1.active_at(point)))
    });

    // Benchmark all 13 Allen relations
    group.bench_function("all_relations", |b| {
        b.iter(|| {
            for i in &intervals {
                for j in &intervals {
                    black_box(i.relation(j));
                }
            }
        })
    });

    // Intersection list
    for size in [5, 10, 20].iter() {
        let overlapping: Vec<Temporal> = (0..*size)
            .map(|i| Temporal::from_bounds(i * 5, i * 5 + 50))
            .collect();

        group.bench_with_input(
            BenchmarkId::new("intersection_list", size),
            &overlapping,
            |b, intervals| b.iter(|| black_box(Temporal::intersection_list(intervals))),
        );
    }

    group.finish();
}

// =============================================================================
// GROUNDING BENCHMARKS
// =============================================================================

fn bench_grounding(c: &mut Criterion) {
    let mut group = c.benchmark_group("grounding");

    // Vary number of entities (creates O(n²) relationships)
    for entities in [10, 20, 30].iter() {
        let theory = create_grounding_theory(*entities, 3);
        group.throughput(Throughput::Elements(*entities as u64));
        group.bench_with_input(
            BenchmarkId::new("entities", entities),
            &theory,
            |b, theory| b.iter(|| black_box(ground_theory(theory))),
        );
    }

    group.finish();
}

// =============================================================================
// TRUST BENCHMARKS
// =============================================================================

fn bench_trust(c: &mut Criterion) {
    let mut group = c.benchmark_group("trust");

    // Policy creation and lookup
    group.bench_function("policy_creation", |b| {
        b.iter(|| {
            black_box(
                TrustPolicy::new(0.5)
                    .with_trust("alice", 0.9)
                    .with_trust("bob", 0.7)
                    .with_trust("carol", 0.8)
                    .with_threshold("high", 0.8)
                    .with_threshold("medium", 0.5),
            )
        })
    });

    let policy = TrustPolicy::new(0.5)
        .with_trust("alice", 0.9)
        .with_trust("bob", 0.7)
        .with_threshold("high", 0.8);

    group.bench_function("trust_lookup", |b| {
        b.iter(|| black_box(policy.get_trust("alice")))
    });

    group.bench_function("threshold_check", |b| {
        b.iter(|| black_box(policy.is_above_threshold(0.85, "high")))
    });

    // Weakest link computation in trust trees
    for depth in [3, 5, 10].iter() {
        let tree = build_trust_tree(*depth);

        group.bench_with_input(BenchmarkId::new("weakest_link", depth), &tree, |b, tree| {
            b.iter(|| black_box(tree.weakest_link_trust()))
        });
    }

    group.finish();
}

/// Build a trust derivation tree of given depth
fn build_trust_tree(depth: usize) -> TrustDerivationNode {
    fn build_level(depth: usize, trust: f64) -> TrustDerivationNode {
        let node = TrustDerivationNode::new(Literal::simple(format!("level{}", depth)), trust)
            .with_source(Source::new(format!("source{}", depth)));

        if depth == 0 {
            node
        } else {
            node.with_children(vec![
                build_level(depth - 1, trust * 0.95),
                build_level(depth - 1, trust * 0.9),
            ])
        }
    }

    build_level(depth, 1.0)
}

// =============================================================================
// QUERY OPERATOR BENCHMARKS
// =============================================================================

fn bench_query_operators(c: &mut Criterion) {
    let mut group = c.benchmark_group("query");

    let theory = create_query_theory();

    // What-if queries
    group.bench_function("what_if_single", |b| {
        b.iter(|| {
            black_box(what_if(
                &theory,
                vec![HypotheticalClaim::new(Literal::simple("tests_pass"))],
                &Literal::simple("ready_review"),
            ))
        })
    });

    group.bench_function("what_if_multiple", |b| {
        b.iter(|| {
            black_box(what_if(
                &theory,
                vec![
                    HypotheticalClaim::new(Literal::simple("tests_pass")),
                    HypotheticalClaim::new(Literal::simple("docs_updated")),
                ],
                &Literal::simple("can_merge"),
            ))
        })
    });

    // Why-not queries
    group.bench_function("why_not", |b| {
        b.iter(|| black_box(why_not(&theory, &Literal::simple("ready_review"))))
    });

    // Abduction queries
    for max_solutions in [1, 5, 10].iter() {
        group.bench_with_input(
            BenchmarkId::new("abduce", max_solutions),
            max_solutions,
            |b, max| b.iter(|| black_box(abduce(&theory, &Literal::simple("can_merge"), *max))),
        );
    }

    group.finish();
}

// =============================================================================
// EXPLANATION BENCHMARKS
// =============================================================================

fn bench_explanation(c: &mut Criterion) {
    let mut group = c.benchmark_group("explanation");

    // Simple theory
    let mut simple_theory = Theory::new();
    simple_theory.add_rule(Rule::fact("f1", Literal::simple("bird")));
    simple_theory.add_rule(Rule::defeasible(
        "r1",
        vec![Literal::simple("bird")],
        Literal::simple("flies"),
    ));
    reason(&simple_theory); // Compute conclusions

    group.bench_function("explain_simple", |b| {
        b.iter(|| black_box(explain(&simple_theory, &Literal::simple("flies"))))
    });

    // Chain theory (deeper proof trees)
    let chain_theory = create_chain_theory(10);
    reason(&chain_theory);

    group.bench_function("explain_chain", |b| {
        b.iter(|| black_box(explain(&chain_theory, &Literal::simple("p10"))))
    });

    // Conflict theory (shows conflict resolution)
    let conflict_theory = create_conflict_theory(5);
    reason(&conflict_theory);

    group.bench_function("explain_conflict", |b| {
        b.iter(|| black_box(explain(&conflict_theory, &Literal::simple("prop0"))))
    });

    // Output format benchmarks
    if let Some(exp) = explain(&chain_theory, &Literal::simple("p5")) {
        group.bench_function("to_natural_language", |b| {
            b.iter(|| black_box(exp.to_natural_language()))
        });

        group.bench_function("to_json", |b| b.iter(|| black_box(exp.to_json())));

        group.bench_function("to_dot", |b| b.iter(|| black_box(exp.to_dot())));
    }

    group.finish();
}

// =============================================================================
// PARSER BENCHMARKS
// =============================================================================

fn bench_parser(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser");

    for size in [100, 500, 1_000].iter() {
        let dfl_input = generate_dfl(*size);
        let spl_input = generate_spl(*size);

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(BenchmarkId::new("dfl", size), &dfl_input, |b, input| {
            b.iter(|| black_box(parse_dfl(input)))
        });

        group.bench_with_input(BenchmarkId::new("spl", size), &spl_input, |b, input| {
            b.iter(|| black_box(parse_spl(input)))
        });
    }

    group.finish();
}

// =============================================================================
// MAIN
// =============================================================================

// Quick benchmarks (default) - runs in ~1-2 minutes
criterion_group!(
    quick_benches,
    bench_literal_operations,
    bench_superiority_lookup,
    bench_reason_facts,
    bench_reason_chain,
    bench_reason_wide,
    bench_reason_conflicts,
    bench_reason_scalable,
    bench_standard_vs_scalable,
    bench_temporal_relations,
    bench_grounding,
    bench_trust,
    bench_query_operators,
    bench_explanation,
    bench_parser,
);

// Large-scale benchmarks - run with: cargo bench -- "scaling"
criterion_group!(scaling_benches, bench_scaling,);

criterion_main!(quick_benches, scaling_benches);
