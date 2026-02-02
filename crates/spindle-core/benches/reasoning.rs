//! Performance benchmarks for spindle-core hot paths
//!
//! Run with: cargo bench --package spindle-core
//!
//! These benchmarks measure the performance of:
//! - Literal operations (literal_id vs canonical_name)
//! - Superiority lookups (indexed O(1) vs linear O(n))
//! - Reasoning with various theory sizes
//! - Closure computations (delta, lambda, partial)

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use spindle_core::prelude::*;
use spindle_core::reason::reason;
use spindle_core::scalable::reason_scalable;
use std::collections::HashSet;

/// Create a theory with N facts
fn create_fact_theory(n: usize) -> Theory {
    let mut theory = Theory::new();
    for i in 0..n {
        let fact = Rule::fact(
            format!("f{}", i),
            Literal::simple(format!("prop{}", i)),
        );
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

// =============================================================================
// LITERAL BENCHMARKS
// =============================================================================

fn bench_literal_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("literal_ops");

    let lit = Literal::simple("test_literal_name");
    let neg_lit = Literal::negated("test_literal_name");

    group.bench_function("literal_id", |b| {
        b.iter(|| black_box(lit.literal_id()))
    });

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

        group.bench_with_input(
            BenchmarkId::new("indexed_lookup", size),
            size,
            |b, _| {
                b.iter(|| {
                    // Check all superiority relations
                    for i in 0..*size {
                        black_box(theory.is_superior(
                            &format!("r{}", i),
                            &format!("s{}", i),
                        ));
                    }
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// REASONING BENCHMARKS
// =============================================================================

fn bench_reason_facts(c: &mut Criterion) {
    let mut group = c.benchmark_group("reason_facts");

    for size in [10, 100, 500].iter() {
        let theory = create_fact_theory(*size);

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("reason", size),
            &theory,
            |b, theory| {
                b.iter(|| black_box(reason(theory)))
            },
        );
    }

    group.finish();
}

fn bench_reason_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("reason_chain");

    for size in [10, 50, 100].iter() {
        let theory = create_chain_theory(*size);

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("reason", size),
            &theory,
            |b, theory| {
                b.iter(|| black_box(reason(theory)))
            },
        );
    }

    group.finish();
}

fn bench_reason_wide(c: &mut Criterion) {
    let mut group = c.benchmark_group("reason_wide");

    for size in [10, 50, 100].iter() {
        let theory = create_wide_theory(*size);

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("reason", size),
            &theory,
            |b, theory| {
                b.iter(|| black_box(reason(theory)))
            },
        );
    }

    group.finish();
}

fn bench_reason_conflicts(c: &mut Criterion) {
    let mut group = c.benchmark_group("reason_conflicts");

    for size in [10, 50, 100].iter() {
        let theory = create_conflict_theory(*size);

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("reason", size),
            &theory,
            |b, theory| {
                b.iter(|| black_box(reason(theory)))
            },
        );
    }

    group.finish();
}

// =============================================================================
// SCALABLE REASONING BENCHMARKS
// =============================================================================

fn bench_reason_scalable(c: &mut Criterion) {
    let mut group = c.benchmark_group("reason_scalable");

    for size in [10, 50, 100].iter() {
        let theory = create_wide_theory(*size);

        group.throughput(Throughput::Elements(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("reason_scalable", size),
            &theory,
            |b, theory| {
                b.iter(|| black_box(reason_scalable(theory)))
            },
        );
    }

    group.finish();
}

// =============================================================================
// MAIN
// =============================================================================

criterion_group!(
    benches,
    bench_literal_operations,
    bench_superiority_lookup,
    bench_reason_facts,
    bench_reason_chain,
    bench_reason_wide,
    bench_reason_conflicts,
    bench_reason_scalable,
);

criterion_main!(benches);
