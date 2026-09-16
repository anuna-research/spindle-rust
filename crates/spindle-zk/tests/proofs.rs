//! SPEC-025 TEST-003/004/005: real proof roundtrip and statement mutation.
use spindle_zk::{Claim, Policy, Proof, Tag, parse_facts};

#[test]
fn claim_projection_eliminates_disconnected_inference() {
    let policy = Policy::compile(
        "(normally r a q) (normally s b unrelated) (normally cycle unrelated b)",
        "(given a) (given b)",
    )
    .unwrap();
    let claim = Claim::new("q", Tag::Defeasibly).unwrap();
    assert!(policy.claim_operation_count(&claim).unwrap() < policy.operation_count());
    assert!(
        policy
            .claim_operation_count(&Claim::new("missing", Tag::Defeasibly).unwrap())
            .is_err()
    );
}

#[test]
fn actual_proof_roundtrip_and_tampering() {
    let policy = Policy::compile("(normally r bird flies)", "(given bird)").unwrap();
    let claim = Claim::new("flies", Tag::Defeasibly).unwrap();
    let proof = policy
        .prove(&parse_facts("(given bird)").unwrap(), &claim)
        .unwrap();
    let policy = Policy::from_json(&policy.to_json().unwrap()).unwrap();
    let encoded = proof.to_json().unwrap();
    let proof = Proof::from_json(&encoded).unwrap();
    let verified = policy.verify(&proof, &claim).unwrap();
    assert_eq!(verified.claim, claim);
    assert_eq!(verified.policy_id, policy.id());
    assert_eq!(
        verified.fact_authenticity,
        spindle_zk::FactAuthenticity::Unattested
    );

    for extend in [false, true] {
        let mut raw: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        let bytes = raw["proof"].as_array_mut().unwrap();
        if extend {
            bytes.extend(std::iter::repeat_n(serde_json::json!(0), 32));
        } else {
            bytes.truncate(bytes.len() - 32);
        }
        let malformed = Proof::from_json(&serde_json::to_vec(&raw).unwrap()).unwrap();
        assert!(policy.verify(&malformed, &claim).is_err());
    }

    let mut raw: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    raw["commitment"][0] = (raw["commitment"][0].as_u64().unwrap() ^ 1).into();
    let changed = Proof::from_json(&serde_json::to_vec(&raw).unwrap()).unwrap();
    assert!(policy.verify(&changed, &claim).is_err());

    let mut raw: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    raw["proof"][0] = (raw["proof"][0].as_u64().unwrap() ^ 1).into();
    let changed = Proof::from_json(&serde_json::to_vec(&raw).unwrap()).unwrap();
    assert!(policy.verify(&changed, &claim).is_err());

    let wrong_claim = Claim::new("bird", Tag::Defeasibly).unwrap();
    let mut raw: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    raw["claim"] = serde_json::to_value(&wrong_claim).unwrap();
    let changed = Proof::from_json(&serde_json::to_vec(&raw).unwrap()).unwrap();
    assert!(policy.verify(&changed, &wrong_claim).is_err());

    let other = Policy::compile(
        "(normally r bird flies) (normally s flies bird)",
        "(given bird)",
    )
    .unwrap();
    assert!(matches!(
        other.verify(&proof, &claim),
        Err(spindle_zk::Error::PolicyMismatch)
    ));
    let wrong_tag = Claim::new("flies", Tag::NotDefeasibly).unwrap();
    let mut raw: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    raw["claim"] = serde_json::to_value(&wrong_tag).unwrap();
    let changed = Proof::from_json(&serde_json::to_vec(&raw).unwrap()).unwrap();
    assert!(matches!(
        policy.verify(&changed, &wrong_tag),
        Err(spindle_zk::Error::InvalidProof)
    ));
    let second = policy
        .prove(&parse_facts("(given bird)").unwrap(), &claim)
        .unwrap();
    let second = policy.verify(&second, &claim).unwrap();
    assert_ne!(verified.fact_commitment, second.fact_commitment);
    assert!(!String::from_utf8(encoded).unwrap().contains("witness"));
}

#[test]
fn missing_evidence_cannot_produce_a_proof() {
    let policy = Policy::compile("(normally r bird flies)", "(given bird)").unwrap();
    assert!(
        policy
            .prove(&[], &Claim::new("flies", Tag::Defeasibly).unwrap())
            .is_err()
    );
}

#[test]
fn claim_does_not_erase_modal_or_temporal_meaning() {
    assert!(Claim::new("(must p)", Tag::Defeasibly).is_err());
    assert!(Claim::new("(during p 0 10)", Tag::Defeasibly).is_err());
    assert!(Claim::new("(p) (must q)", Tag::Defeasibly).is_err());
}

#[test]
fn private_parse_errors_do_not_echo_source() {
    let error = parse_facts("(given (secret-person-472 918273645)").unwrap_err();
    assert!(!error.to_string().contains("secret-person-472"));
    assert!(!error.to_string().contains("918273645"));
}

#[test]
fn proof_length_does_not_reveal_private_fact_selection() {
    let policy = Policy::compile("(given public)", "(given private)").unwrap();
    let claim = Claim::new("public", Tag::Definitely).unwrap();
    let absent = policy.prove(&[], &claim).unwrap();
    let present = policy
        .prove(&parse_facts("(given private)").unwrap(), &claim)
        .unwrap();
    let absent_verified = policy.verify(&absent, &claim).unwrap();
    let present_verified = policy.verify(&present, &claim).unwrap();
    assert_ne!(
        absent_verified.fact_commitment,
        present_verified.fact_commitment
    );
    let absent: serde_json::Value = serde_json::from_slice(&absent.to_json().unwrap()).unwrap();
    let present: serde_json::Value = serde_json::from_slice(&present.to_json().unwrap()).unwrap();
    // Decimal JSON byte spellings vary with randomized contents; the opaque
    // cryptographic transcript's byte count must not vary with private inputs.
    assert_eq!(
        absent["proof"].as_array().unwrap().len(),
        present["proof"].as_array().unwrap().len()
    );
    for package in [&absent, &present] {
        let fields: std::collections::BTreeSet<_> = package
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            fields,
            ["version", "policy_id", "claim", "commitment", "proof"]
                .into_iter()
                .collect()
        );
    }
}
