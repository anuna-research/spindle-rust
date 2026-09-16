//! Explicit rejection limits are errors, never negative reasoning results.
use spindle_zk::{Error, Program, parse_facts};
use std::time::Instant;

#[test]
fn rule_and_literal_limits_fail_closed() {
    let source = (0..257)
        .map(|i| format!("(normally r{i} (and) q)"))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(matches!(
        Program::compile(&spindle_parser::parse_spl(&source).unwrap(), &[]),
        Err(Error::ResourceLimit(_))
    ));
    let schema = (0..65)
        .map(|i| format!("(given p{i})"))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(matches!(
        Program::compile(&spindle_core::Theory::new(), &parse_facts(&schema).unwrap()),
        Err(Error::ResourceLimit(_))
    ));
}

#[test]
#[ignore = "dense public-policy compilation resource measurement"]
fn dense_boundary_compilation_profile() {
    let inputs = parse_facts(
        &(0..64)
            .map(|i| format!("(given p{i})"))
            .collect::<Vec<_>>()
            .join(" "),
    )
    .unwrap();
    for rules in [64, 128, 256] {
        let source = (0..rules)
            .map(|i| {
                let head = i % 64;
                let a = (head + 1 + i / 64) % 64;
                let b = (head + 7 + 3 * (i / 64)) % 64;
                format!("(normally r{i} (and p{a} p{b}) p{head})")
            })
            .collect::<Vec<_>>()
            .join(" ");
        let theory = spindle_parser::parse_spl(&source).unwrap();
        let start = Instant::now();
        let result = Program::compile(&theory, &inputs);
        let elapsed = start.elapsed().as_millis();
        match result {
            Ok(program) => {
                assert!(program.operation_count() <= 1_000_000);
                println!(
                    "{}",
                    serde_json::json!({"rules":rules,"literals_with_complements":128,"compile_ms":elapsed,"operations":program.operation_count(),"result":"compiled"})
                );
            }
            Err(Error::ResourceLimit(message)) => {
                println!(
                    "{}",
                    serde_json::json!({"rules":rules,"literals_with_complements":128,"compile_ms":elapsed,"result":"resource_limit","message":message})
                );
            }
            Err(error) => panic!("unexpected error for dense public policy: {error}"),
        }
    }
}
