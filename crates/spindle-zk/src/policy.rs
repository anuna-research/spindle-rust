use crate::{
    Error, Program, Result,
    circuit::ProofCircuit,
    program::{
        AGGREGATE_LIMITS, MAX_GATES, MAX_LITERALS, MAX_RULES, TAGS, check_literal, literal_spl,
    },
};
use ff::{Field, PrimeField};
use halo2_proofs::{
    circuit::Value,
    pasta::{EqAffine, Fp},
    plonk::{Circuit, SingleVerifier, create_proof, keygen_pk, keygen_vk, verify_proof},
    poly::commitment::Params,
    transcript::{Blake2bRead, Blake2bWrite, Challenge255},
};
use rand_core::OsRng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use spindle_core::{conclusion::ConclusionType, literal::Literal, rule::RuleType};
use spindle_parser::parse_spl_bounded;

const VERSION: u32 = 1;
const MAX_PACKAGE_BYTES: usize = 1_048_576;

/// A public proof tag. Missing positive evidence is not a negative tag.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Tag {
    /// Definite proof.
    #[serde(rename = "+D")]
    Definitely,
    /// Constructive definite disproof.
    #[serde(rename = "-D")]
    NotDefinitely,
    /// Defeasible proof (the CLI default).
    #[serde(rename = "+d")]
    Defeasibly,
    /// Constructive defeasible disproof.
    #[serde(rename = "-d")]
    NotDefeasibly,
}

impl Tag {
    fn index(self) -> usize {
        match self {
            Self::Definitely => 0,
            Self::NotDefinitely => 1,
            Self::Defeasibly => 2,
            Self::NotDefeasibly => 3,
        }
    }
    /// Corresponding existing engine tag.
    pub fn conclusion_type(self) -> ConclusionType {
        TAGS[self.index()]
    }
}

/// An independently expected, public claim.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Claim {
    /// Canonical SPL literal, including any public identifiers.
    pub literal: String,
    /// Proof strength.
    pub tag: Tag,
}

impl Claim {
    /// Recognize one literal with the existing SPL parser.
    pub fn new(literal: &str, tag: Tag) -> Result<Self> {
        let facts = parse_facts(&format!("(given {literal})"))?;
        if facts.len() != 1 {
            return Err(Error::InvalidInput("expected one claim literal".into()));
        }
        check_literal(&facts[0])?;
        Ok(Self {
            literal: literal_spl(&facts[0]),
            tag,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyDocument {
    version: u32,
    semantics: String,
    source: String,
    inputs: String,
    max_literals: usize,
    max_rules: usize,
    max_gates: usize,
    aggregate_limits: [usize; 4],
}

/// Reusable public policy. Verification derives the circuit from this policy,
/// never from keys or parameters supplied with a received proof.
pub struct Policy {
    document: PolicyDocument,
    program: Program,
    id: [u8; 32],
}

/// Portable proof package containing no private facts or inference trace.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proof {
    version: u32,
    policy_id: [u8; 32],
    claim: Claim,
    commitment: [u8; 32],
    proof: Vec<u8>,
}

/// Values authenticated by verification, not copied from unchecked metadata.
#[derive(Debug)]
pub struct VerifiedClaim {
    /// The expected claim that the verifier authenticated.
    pub claim: Claim,
    /// Identity of the trusted policy used for verification.
    pub policy_id: [u8; 32],
    /// Randomized commitment to the complete private-input presence vector.
    pub fact_commitment: [u8; 32],
    /// Proof validity does not authenticate the provenance or truth of facts.
    pub fact_authenticity: FactAuthenticity,
}

/// Provenance status is separate from correctness of the reasoning proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FactAuthenticity {
    /// No credential or issuer-signature proof has been verified.
    Unattested,
}

fn check_size(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_PACKAGE_BYTES {
        return Err(Error::ResourceLimit("input exceeds 1 MiB".into()));
    }
    Ok(())
}

fn encode_package(value: &impl Serialize) -> Result<Vec<u8>> {
    let bytes = serde_json::to_vec(value).map_err(|e| Error::InvalidInput(e.to_string()))?;
    check_size(&bytes)?;
    Ok(bytes)
}

/// Recognize a fact-only SPL document. Rules cannot be injected as private input.
pub fn parse_facts(source: &str) -> Result<Vec<Literal>> {
    check_size(source.as_bytes())?;
    let theory = parse_spl_bounded(source, 64)
        .map_err(|_| Error::Parse("invalid private fact syntax; source omitted".into()))?;
    if !theory.superiorities().is_empty()
        || !theory.metadata().is_empty()
        || !theory.predicate_declarations().is_empty()
        || !theory.predicate_metadata().is_empty()
        || theory.aggregate_domain().is_some()
        || !theory.trust_policy().trust_map.is_empty()
        || !theory.trust_policy().thresholds.is_empty()
        || !theory.trust_policy().decay_map.is_empty()
        || theory
            .rules()
            .any(|r| r.rule_type != RuleType::Fact || r.head.len() != 1 || !r.body.is_empty())
    {
        return Err(Error::InvalidInput(
            "expected a fact-only SPL document".into(),
        ));
    }
    Ok(theory.rules().map(|r| r.head[0].clone()).collect())
}

impl Policy {
    /// Compile a policy and a fact-only public declaration of allowable inputs.
    pub fn compile(source: &str, inputs: &str) -> Result<Self> {
        check_size(source.as_bytes())?;
        check_size(inputs.as_bytes())?;
        let document = PolicyDocument {
            version: VERSION,
            semantics: "spindle.dl-partial.halo2-ipa.v1".into(),
            source: source.into(),
            inputs: inputs.into(),
            max_literals: MAX_LITERALS,
            max_rules: MAX_RULES,
            max_gates: MAX_GATES,
            aggregate_limits: AGGREGATE_LIMITS,
        };
        // Apply the reader's bound to the complete escaped document before
        // expensive compilation, not independently to its source fields only.
        let bytes = encode_package(&document)?;
        let theory = parse_spl_bounded(source, 64).map_err(|e| Error::Parse(e.to_string()))?;
        let program = Program::compile(&theory, &parse_facts(inputs)?)?;
        let id = Sha256::digest(bytes).into();
        Ok(Self {
            document,
            program,
            id,
        })
    }

    /// Stable identity of this exact policy source, input schema, and profile.
    pub fn id(&self) -> [u8; 32] {
        self.id
    }

    /// Number of compiled inference operations.
    pub fn operation_count(&self) -> usize {
        self.program.operation_count()
    }

    /// Boolean operations retained for a public claim, including all committed
    /// inputs and global validity checks. This is not a time or memory estimate.
    pub fn claim_operation_count(&self, claim: &Claim) -> Result<usize> {
        Ok(self.circuit(claim)?.program.operation_count())
    }

    /// Serialize public policy material. No proving secrets are stored.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        encode_package(&self.document)
    }

    /// Parse a bounded policy document and recompile its relation.
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        check_size(bytes)?;
        let document: PolicyDocument =
            serde_json::from_slice(bytes).map_err(|e| Error::InvalidInput(e.to_string()))?;
        if document.version != VERSION
            || document.semantics != "spindle.dl-partial.halo2-ipa.v1"
            || document.max_literals != MAX_LITERALS
            || document.max_rules != MAX_RULES
            || document.max_gates != MAX_GATES
            || document.aggregate_limits != AGGREGATE_LIMITS
        {
            return Err(Error::Unsupported("policy version or semantics".into()));
        }
        Self::compile(&document.source, &document.inputs)
    }

    fn circuit(&self, claim: &Claim) -> Result<ProofCircuit> {
        let canonical = Claim::new(&claim.literal, claim.tag)?;
        if &canonical != claim {
            return Err(Error::InvalidInput("noncanonical claim".into()));
        }
        let literal = self
            .program
            .literals
            .iter()
            .position(|l| literal_spl(l) == claim.literal)
            .ok_or_else(|| Error::InvalidInput("claim is outside the policy vocabulary".into()))?;
        let policy_id = std::array::from_fn(|i| {
            let mut repr = [0; 32];
            repr[..16].copy_from_slice(&self.id[16 * i..16 * (i + 1)]);
            Option::<Fp>::from(Fp::from_repr(repr)).expect("128-bit digest limb fits Fp")
        });
        Ok(ProofCircuit {
            program: self.program.for_claim(literal, claim.tag.index()),
            values: None,
            nonce: Value::unknown(),
            policy_id,
            literal,
            tag: claim.tag.index(),
        })
    }

    /// Prove an established claim using selected private facts and fresh blinding.
    pub fn prove(&self, facts: &[Literal], claim: &Claim) -> Result<Proof> {
        let mut circuit = self.circuit(claim)?;
        let witness = circuit.program.witness(facts)?;
        if !witness[circuit.program.outputs[circuit.literal][circuit.tag]] {
            return Err(Error::ClaimNotEstablished);
        }
        let nonce = Fp::random(OsRng);
        let commitment = circuit.commitment(nonce, &witness);
        circuit.values = Some(witness.iter().map(|&v| Fp::from(u64::from(v))).collect());
        circuit.nonce = Value::known(nonce);
        let instance = [
            circuit.policy_id[0],
            circuit.policy_id[1],
            Fp::from(circuit.literal as u64),
            Fp::from(circuit.tag as u64),
            commitment,
        ];
        let expected_proof_len = circuit.proof_len();
        let params: Params<EqAffine> = Params::new(circuit.k());
        let empty = circuit.without_witnesses();
        let vk = keygen_vk(&params, &empty).map_err(|e| Error::Backend(e.to_string()))?;
        let pk = keygen_pk(&params, vk, &empty).map_err(|e| Error::Backend(e.to_string()))?;
        let mut transcript = Blake2bWrite::<_, EqAffine, Challenge255<_>>::init(Vec::new());
        create_proof(
            &params,
            &pk,
            &[circuit],
            &[&[&instance]],
            OsRng,
            &mut transcript,
        )
        .map_err(|e| Error::Backend(e.to_string()))?;
        let transcript = transcript.finalize();
        if transcript.len() != expected_proof_len || !empty.encoding_is_valid(&transcript) {
            return Err(Error::Backend(
                "proof encoding model does not match the pinned circuit".into(),
            ));
        }
        let proof = Proof {
            version: VERSION,
            policy_id: self.id,
            claim: claim.clone(),
            commitment: commitment.to_repr(),
            proof: transcript,
        };
        proof.to_json()?;
        Ok(proof)
    }

    /// Verify against this trusted policy and an independently expected claim.
    pub fn verify(&self, proof: &Proof, expected: &Claim) -> Result<VerifiedClaim> {
        if proof.policy_id != self.id {
            return Err(Error::PolicyMismatch);
        }
        if proof.version != VERSION || &proof.claim != expected {
            return Err(Error::InvalidProof);
        }
        if proof.proof.len() > MAX_PACKAGE_BYTES {
            return Err(Error::ResourceLimit("proof size".into()));
        }
        // Curve points and scalars have 32-byte canonical encodings. This
        // rejects obviously incomplete transcripts before parameter generation.
        if proof.proof.len() < 32 || !proof.proof.len().is_multiple_of(32) {
            return Err(Error::InvalidProof);
        }
        let commitment =
            Option::<Fp>::from(Fp::from_repr(proof.commitment)).ok_or(Error::InvalidProof)?;
        let circuit = self.circuit(expected)?;
        // Reject even canonically aligned truncation/extension before curve
        // parameter generation and keygen. Public layout measurement remains
        // bounded by the compiler limits; it is not a constant-time operation.
        if proof.proof.len() < 32 * (2 * circuit.k() as usize + 3)
            || proof.proof.len() != circuit.proof_len()
            || !circuit.encoding_is_valid(&proof.proof)
        {
            return Err(Error::InvalidProof);
        }
        let instance = [
            circuit.policy_id[0],
            circuit.policy_id[1],
            Fp::from(circuit.literal as u64),
            Fp::from(circuit.tag as u64),
            commitment,
        ];
        let params: Params<EqAffine> = Params::new(circuit.k());
        let vk = keygen_vk(&params, &circuit).map_err(|e| Error::Backend(e.to_string()))?;
        let mut reader = proof.proof.as_slice();
        let mut transcript = Blake2bRead::<_, EqAffine, Challenge255<_>>::init(&mut reader);
        verify_proof(
            &params,
            &vk,
            SingleVerifier::new(&params),
            &[&[&instance]],
            &mut transcript,
        )
        .map_err(|_| Error::InvalidProof)?;
        if !reader.is_empty() {
            return Err(Error::InvalidProof);
        }
        Ok(VerifiedClaim {
            claim: expected.clone(),
            policy_id: self.id,
            fact_commitment: proof.commitment,
            fact_authenticity: FactAuthenticity::Unattested,
        })
    }
}

impl Proof {
    /// Serialize public statement and opaque cryptographic proof bytes only.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        encode_package(self)
    }

    /// Recognize a bounded, versioned proof document.
    pub fn from_json(bytes: &[u8]) -> Result<Self> {
        check_size(bytes)?;
        let proof: Self =
            serde_json::from_slice(bytes).map_err(|e| Error::InvalidInput(e.to_string()))?;
        if proof.version != VERSION {
            return Err(Error::Unsupported("proof version".into()));
        }
        Ok(proof)
    }
}

#[cfg(test)]
mod encoding_tests {
    use super::*;

    #[test]
    fn every_transcript_word_is_recognized_before_setup() {
        let policy = Policy::compile("(normally r a q)", "(given a)").unwrap();
        let claim = Claim::new("q", Tag::Defeasibly).unwrap();
        let proof = policy
            .prove(&parse_facts("(given a)").unwrap(), &claim)
            .unwrap();
        let circuit = policy.circuit(&claim).unwrap();
        assert!(circuit.encoding_is_valid(&proof.proof));
        for start in (0..proof.proof.len()).step_by(32) {
            let mut malformed = proof.proof.clone();
            malformed[start..start + 32].fill(255);
            assert!(
                !circuit.encoding_is_valid(&malformed),
                "accepted noncanonical word at {start}"
            );
        }
    }
}
