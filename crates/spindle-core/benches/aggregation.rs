//! Aggregate preparation and full reasoning, excluding parsing and fixture setup.
//! Run: cargo bench -p spindle-core --bench aggregation
//! Smoke check all fixtures: cargo bench -p spindle-core --bench aggregation -- --test

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use spindle_core::{
    conclusion::Conclusion,
    pipeline::{PrepareOptions, prepare},
    reason::{reason, reason_prepared},
};
use spindle_parser::parse_spl;
use std::{collections::BTreeSet, fmt::Write, hint::black_box, time::Duration};

struct Fixture {
    source: String,
    expected: BTreeSet<String>,
}

// Unique row IDs retain equal contributions under aggregate set semantics.
// Round-robin groups keep the total row count fixed as group count changes.
fn fixture(rows: usize, groups: usize, reducer: &str, noise: usize, stages: usize) -> Fixture {
    assert!(groups > 0 && stages > 0);
    let mut source = String::new();
    let mut values = vec![Vec::new(); groups];
    for group in 0..groups {
        writeln!(source, "(given (group g{group}))").unwrap();
    }
    for row in 0..rows {
        let group = row % groups;
        let value = row % 17 + 1;
        values[group].push(value);
        writeln!(source, "(given (payment g{group} id{row} {value}))").unwrap();
    }
    for row in 0..noise {
        writeln!(source, "(given (unrelated noise{row}))").unwrap();
    }
    writeln!(
        source,
        "(normally aggregate0 (and (group ?g)
          (agg ?n {reducer} ?v (payment ?g ?id ?v))) (result0 ?g ?n))"
    )
    .unwrap();
    let mut expected = BTreeSet::new();
    for (group, values) in values.iter().enumerate() {
        let value = match reducer {
            "sum" => Some(values.iter().sum::<usize>()),
            "count" => Some(values.len()),
            "min-of" => values.iter().min().copied(),
            "max-of" => values.iter().max().copied(),
            _ => unreachable!(),
        };
        if let Some(value) = value {
            for stage in 0..stages {
                expected.insert(format!("(result{stage} g{group} {value})"));
            }
        }
    }
    // Later stages sum the preceding per-group result, preserving its value.
    for stage in 1..stages {
        let previous = stage - 1;
        writeln!(
            source,
            "(normally aggregate{stage} (and (group ?g)
              (agg ?n sum ?v (result{previous} ?g ?v))) (result{stage} ?g ?n))"
        )
        .unwrap();
    }
    Fixture { source, expected }
}

fn results(conclusions: &[Conclusion]) -> BTreeSet<String> {
    conclusions
        .iter()
        .filter(|c| c.is_positive() && c.literal.name().starts_with("result"))
        .map(|c| c.literal.to_spl())
        .collect()
}

fn bench_fixture(c: &mut Criterion, family: &str, case: &str, fixture: Fixture) {
    let theory = parse_spl(&fixture.source).expect("valid benchmark SPL");
    let prepared = prepare(&theory, PrepareOptions::default()).unwrap();
    assert_eq!(
        results(&reason_prepared(&prepared.theory).unwrap()),
        fixture.expected,
        "prepared fixture {family}/{case}"
    );
    assert_eq!(
        results(&reason(&theory).unwrap()),
        fixture.expected,
        "full reasoning fixture {family}/{case}"
    );

    let mut group = c.benchmark_group(format!("aggregation/{family}"));
    group.bench_function(BenchmarkId::new("prepare", case), |b| {
        b.iter(|| black_box(prepare(black_box(&theory), PrepareOptions::default()).unwrap()))
    });
    group.bench_function(BenchmarkId::new("reason", case), |b| {
        b.iter(|| black_box(reason(black_box(&theory)).unwrap()))
    });
    group.finish();
}

fn aggregation(c: &mut Criterion) {
    for reducer in ["sum", "count", "min-of", "max-of"] {
        for rows in [0, 100, 500, 1_000] {
            bench_fixture(
                c,
                "rows",
                &format!("{reducer}/{rows}"),
                fixture(rows, 1, reducer, 0, 1),
            );
        }
    }
    for groups in [1, 10, 100] {
        bench_fixture(
            c,
            "groups",
            &format!("500_rows/{groups}"),
            fixture(500, groups, "sum", 0, 1),
        );
    }
    for noise in [0, 500, 1_000] {
        bench_fixture(
            c,
            "unrelated",
            &format!("100_rows/{noise}"),
            fixture(100, 1, "sum", noise, 1),
        );
    }
    for stages in [1, 2, 4] {
        bench_fixture(
            c,
            "stages",
            &format!("100_rows_10_groups/{stages}"),
            fixture(100, 10, "sum", 0, stages),
        );
    }
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(1));
    targets = aggregation
}
criterion_main!(benches);
