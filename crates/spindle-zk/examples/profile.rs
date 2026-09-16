//! Reproducible smoke measurements, not a service-level performance guarantee.
//! cargo run -p spindle-zk --example profile -- 0 8 63
use spindle_zk::{Claim, Policy, Tag, parse_facts};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let counts = std::env::args()
        .skip(1)
        .map(|arg| arg.parse::<usize>())
        .collect::<Result<Vec<_>, _>>()?;
    let counts = if counts.is_empty() {
        vec![0, 8, 63]
    } else {
        counts
    };
    for inputs in counts {
        if inputs > 63 {
            return Err("profile allows at most 63 candidate facts plus the public query".into());
        }
        let schema = (0..inputs)
            .map(|i| format!("(given (candidate {i}))"))
            .collect::<Vec<_>>()
            .join(" ");
        let start = Instant::now();
        let policy = Policy::compile("(given query)", &schema)?;
        let compile_ms = start.elapsed().as_millis();
        let facts = parse_facts(&schema)?;
        let claim = Claim::new("query", Tag::Definitely)?;
        let start = Instant::now();
        let proof = policy.prove(&facts, &claim)?;
        let prove_ms = start.elapsed().as_millis();
        let start = Instant::now();
        policy.verify(&proof, &claim)?;
        let verify_ms = start.elapsed().as_millis();
        let proof_json = proof.to_json()?;
        let package: serde_json::Value = serde_json::from_slice(&proof_json)?;
        println!(
            "{}",
            serde_json::json!({
                "profile":"public-fact-with-private-commitment", "debug_assertions":cfg!(debug_assertions),
                "candidate_facts":inputs, "operations":policy.operation_count(),
                "compile_ms":compile_ms, "prove_ms":prove_ms, "verify_ms":verify_ms,
                "policy_json_bytes":policy.to_json()?.len(), "proof_json_bytes":proof_json.len(),
                "transcript_bytes":package["proof"].as_array().unwrap().len(),
            })
        );
    }
    Ok(())
}
