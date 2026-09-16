#![cfg(feature = "zk")]
//! SPEC-025 TEST-007/011/012: CLI proof workflow and bounded filesystem effects.

#[test]
fn aggregate_json_workflow_matches_library_and_text_verifier() {
    let dir = tempfile::tempdir().unwrap();
    let source = "(aggregate-domain 0 1 2 3) (normally total (agg ?n sum ?v (row ?v)) (total ?n))";
    let inputs = "(given (row 1)) (given (row 2))";
    std::fs::write(dir.path().join("rules.spl"), source).unwrap();
    std::fs::write(dir.path().join("inputs.spl"), inputs).unwrap();
    std::fs::write(dir.path().join("facts.spl"), inputs).unwrap();
    let run = |args: &[&str]| {
        let mut command = assert_cmd::cargo::cargo_bin_cmd!("spindle");
        command.current_dir(dir.path()).args(args).assert()
    };
    for (args, status) in [
        (
            vec!["zk", "check", "rules.spl", "--inputs", "inputs.spl"],
            "supported",
        ),
        (
            vec![
                "zk",
                "compile",
                "rules.spl",
                "--inputs",
                "inputs.spl",
                "--out",
                "policy.json",
            ],
            "compiled",
        ),
        (
            vec![
                "zk",
                "prove",
                "policy.json",
                "--facts",
                "facts.spl",
                "--claim",
                "(total 3)",
                "--out",
                "proof.json",
            ],
            "proved",
        ),
        (
            vec![
                "zk",
                "verify",
                "proof.json",
                "--policy",
                "policy.json",
                "--expect",
                "(total 3)",
            ],
            "valid",
        ),
    ] {
        let mut args = args;
        args.push("--json");
        let output = run(&args).success();
        let result: serde_json::Value =
            serde_json::from_slice(&output.get_output().stdout).unwrap();
        assert_eq!(result["status"], status);
        assert_eq!(result["fact_authenticity"], "unattested");
        if status == "proved" {
            assert!(
                result["disclosure"]["public"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v == "claim including identifiers")
            );
        }
    }
    let library = spindle_zk::Policy::compile(source, inputs).unwrap();
    let compiled =
        spindle_zk::Policy::from_json(&std::fs::read(dir.path().join("policy.json")).unwrap())
            .unwrap();
    assert_eq!(library.id(), compiled.id());
    let proof_bytes = std::fs::read(dir.path().join("proof.json")).unwrap();
    let proof = spindle_zk::Proof::from_json(&proof_bytes).unwrap();
    let claim = spindle_zk::Claim::new("(total 3)", spindle_zk::Tag::Defeasibly).unwrap();
    library.verify(&proof, &claim).unwrap();
    assert!(!String::from_utf8(proof_bytes).unwrap().contains("row"));
    let text = run(&[
        "zk",
        "verify",
        "proof.json",
        "--policy",
        "policy.json",
        "--expect",
        "(total 3)",
    ])
    .success();
    let text = String::from_utf8_lossy(&text.get_output().stdout);
    assert!(text.contains("valid"));
    assert!(text.contains("Claim: +d (total 3)"));
    assert!(text.contains("Fact commitment: "));
    assert!(text.contains("Fact authenticity: unattested"));
}

#[test]
fn four_commands_roundtrip_and_fail_closed() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("rules.spl"), "(normally r bird flies)").unwrap();
    std::fs::write(dir.path().join("inputs.spl"), "(given bird)").unwrap();
    std::fs::write(dir.path().join("facts.spl"), "(given bird)").unwrap();
    let run = |args: &[&str]| {
        let mut command = assert_cmd::cargo::cargo_bin_cmd!("spindle");
        command.current_dir(dir.path()).args(args).assert()
    };
    run(&[
        "zk",
        "check",
        "rules.spl",
        "--inputs",
        "inputs.spl",
        "--json",
    ])
    .success();
    run(&[
        "zk",
        "compile",
        "rules.spl",
        "--inputs",
        "inputs.spl",
        "--out",
        "policy.json",
    ])
    .success();
    let policy_bytes = std::fs::read(dir.path().join("policy.json")).unwrap();
    run(&[
        "zk",
        "compile",
        "rules.spl",
        "--inputs",
        "inputs.spl",
        "--out",
        "policy.json",
    ])
    .failure();
    assert_eq!(
        std::fs::read(dir.path().join("policy.json")).unwrap(),
        policy_bytes
    );
    run(&[
        "zk",
        "prove",
        "policy.json",
        "--facts",
        "facts.spl",
        "--claim",
        "flies",
        "--out",
        "proof.json",
    ])
    .success();
    let verified = run(&[
        "zk",
        "verify",
        "proof.json",
        "--policy",
        "policy.json",
        "--expect",
        "flies",
        "--json",
    ])
    .success();
    let json: serde_json::Value = serde_json::from_slice(&verified.get_output().stdout).unwrap();
    assert_eq!(json["status"], "valid");
    assert_eq!(json["fact_authenticity"], "unattested");
    let other = spindle_zk::Policy::compile("(given flies)", "").unwrap();
    std::fs::write(
        dir.path().join("other-policy.json"),
        other.to_json().unwrap(),
    )
    .unwrap();
    let mismatch = run(&[
        "zk",
        "verify",
        "proof.json",
        "--policy",
        "other-policy.json",
        "--expect",
        "flies",
        "--json",
    ])
    .failure();
    let mismatch = String::from_utf8_lossy(&mismatch.get_output().stdout);
    assert!(mismatch.contains("ZK_POLICY_MISMATCH"));
    run(&[
        "zk",
        "verify",
        "proof.json",
        "--policy",
        "policy.json",
        "--expect",
        "bird",
    ])
    .failure();
    std::fs::write(dir.path().join("empty.spl"), "").unwrap();
    run(&[
        "zk",
        "prove",
        "policy.json",
        "--facts",
        "empty.spl",
        "--claim",
        "flies",
        "--out",
        "failed.json",
    ])
    .failure();
    assert!(!dir.path().join("failed.json").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("facts.spl")).unwrap(),
        "(given bird)"
    );
}
