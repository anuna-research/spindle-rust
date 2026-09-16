//! SPEC-025 TEST-011: package recognition and canonical claim boundaries.
use spindle_zk::{Claim, Error, Policy, Proof, Tag, parse_facts};

#[test]
fn zero_arity_special_form_names_roundtrip_as_claims() {
    for name in ["not", "must", "may", "forbidden", "during"] {
        for literal in [name.to_string(), format!("(not {name})")] {
            let source = format!("(given {literal})");
            let policy = Policy::compile(&source, "").unwrap();
            let claim = Claim::new(&literal, Tag::Definitely).unwrap();
            assert_eq!(claim, Claim::new(&claim.literal, claim.tag).unwrap());
            assert_eq!(
                parse_facts(&source).unwrap(),
                parse_facts(&format!("(given {})", claim.literal)).unwrap()
            );
            policy.claim_operation_count(&claim).unwrap();
            let proof = policy.prove(&[], &claim).unwrap();
            policy.verify(&proof, &claim).unwrap();
        }
    }
}

#[test]
fn canonical_claim_preserves_spl_control_characters() {
    for symbol in [
        "line\nbreak",
        "tab\tvalue",
        "carriage\rreturn",
        "nul\0value",
        "quote\"slash\\value",
    ] {
        let literal = format!(
            "(p \"{}\")",
            symbol.replace('\\', "\\\\").replace('"', "\\\"")
        );
        let original = parse_facts(&format!("(given {literal})")).unwrap();
        let claim = Claim::new(&literal, Tag::Defeasibly).unwrap();
        assert_eq!(claim, Claim::new(&claim.literal, claim.tag).unwrap());
        let reparsed = parse_facts(&format!("(given {})", claim.literal)).unwrap();
        assert_eq!(original, reparsed);
        let json = serde_json::to_vec(&claim).unwrap();
        assert_eq!(claim, serde_json::from_slice::<Claim>(&json).unwrap());
    }
}

#[test]
fn escaped_document_size_is_checked_before_compilation() {
    let individually_small = "\n".repeat(600_000);
    assert!(matches!(
        Policy::compile(&individually_small, ""),
        Err(Error::ResourceLimit(_))
    ));
    assert!(matches!(
        Policy::compile("", &individually_small),
        Err(Error::ResourceLimit(_))
    ));
    let combined = " ".repeat(600_000);
    assert!(matches!(
        Policy::compile(&combined, &combined),
        Err(Error::ResourceLimit(_))
    ));
}

#[test]
fn readers_reject_oversized_malformed_and_mutated_packages() {
    let oversized = vec![b' '; 1_048_577];
    assert!(matches!(
        Policy::from_json(&oversized),
        Err(Error::ResourceLimit(_))
    ));
    assert!(matches!(
        Proof::from_json(&oversized),
        Err(Error::ResourceLimit(_))
    ));
    for malformed in [
        b"".as_slice(),
        b"null",
        b"[]",
        b"{}",
        b"{\"version\":1,\"version\":1}",
    ] {
        assert!(Policy::from_json(malformed).is_err());
        assert!(Proof::from_json(malformed).is_err());
    }
    let policy = Policy::compile("(normally r a b)", "(given a)").unwrap();
    let encoded = policy.to_json().unwrap();
    assert_eq!(policy.id(), Policy::from_json(&encoded).unwrap().id());
    for field in ["version", "max_literals", "max_rules", "max_gates"] {
        let mut raw: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        raw[field] = 0.into();
        assert!(Policy::from_json(&serde_json::to_vec(&raw).unwrap()).is_err());
    }
    let mut raw: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    raw["aggregate_limits"][0] = 0.into();
    assert!(Policy::from_json(&serde_json::to_vec(&raw).unwrap()).is_err());
    raw = serde_json::from_slice(&encoded).unwrap();
    raw["unexpected"] = true.into();
    assert!(Policy::from_json(&serde_json::to_vec(&raw).unwrap()).is_err());
}
