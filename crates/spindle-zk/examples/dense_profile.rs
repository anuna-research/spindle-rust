//! End-to-end dense inference measurements, including fresh keys on each call.
//! cargo run -p spindle-zk --release --example dense_profile -- 8 16 32 64
use spindle_zk::{Claim, Policy, Tag, parse_facts};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let counts = std::env::args()
        .skip(1)
        .map(|arg| arg.parse::<usize>())
        .collect::<Result<Vec<_>, _>>()?;
    let counts = if counts.is_empty() { vec![8] } else { counts };
    for atoms in counts {
        if !(8..=64).contains(&atoms) {
            return Err("dense profile requires between 8 and 64 atoms".into());
        }
        let schema = (0..atoms)
            .map(|i| format!("(given p{i})"))
            .collect::<Vec<_>>()
            .join(" ");
        let source = (0..4 * atoms)
            .map(|i| {
                let head = i % atoms;
                let a = (head + 1 + i / atoms) % atoms;
                let b = (head + 7 + 3 * (i / atoms)) % atoms;
                format!("(normally r{i} (and p{a} p{b}) p{head})")
            })
            .collect::<Vec<_>>()
            .join(" ");
        let start = Instant::now();
        let policy = Policy::compile(&source, &schema)?;
        let compile_ms = start.elapsed().as_millis();
        // Leave the query absent from private facts: proving requires inference.
        let facts = parse_facts(
            &(1..atoms)
                .map(|i| format!("(given p{i})"))
                .collect::<Vec<_>>()
                .join(" "),
        )?;
        let claim = Claim::new("p0", Tag::Defeasibly)?;
        let claim_operations = policy.claim_operation_count(&claim)?;
        eprintln!(
            "dense atoms={atoms} operations={} claim_operations={claim_operations} proving",
            policy.operation_count()
        );
        let start = Instant::now();
        let proof = policy.prove(&facts, &claim)?;
        let prove_ms = start.elapsed().as_millis();
        eprintln!("dense atoms={atoms} prove_ms={prove_ms} verifying");
        let start = Instant::now();
        policy.verify(&proof, &claim)?;
        let verify_ms = start.elapsed().as_millis();
        let package: serde_json::Value = serde_json::from_slice(&proof.to_json()?)?;
        println!(
            "{}",
            serde_json::json!({
                "profile":"dense-cyclic-inference", "debug_assertions":cfg!(debug_assertions),
                "atoms":atoms, "rules":4 * atoms, "operations":policy.operation_count(),
                "claim_operations":claim_operations,
                "compile_ms":compile_ms, "prove_ms":prove_ms, "verify_ms":verify_ms,
                "transcript_bytes":package["proof"].as_array().unwrap().len(),
            })
        );
    }
    Ok(())
}
