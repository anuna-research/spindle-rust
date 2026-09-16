//! SPEC-025 CON-003: optional proof compiler CLI. Filesystem effects live here.
use crate::cli::{error::CliError, output::CommandOutput};
use clap::{Subcommand, ValueEnum};
use spindle_zk::{Claim, Policy, Proof, Tag, parse_facts};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum ProofTag {
    Definite,
    NotDefinite,
    Defeasible,
    NotDefeasible,
}

impl From<ProofTag> for Tag {
    fn from(value: ProofTag) -> Self {
        match value {
            ProofTag::Definite => Tag::Definitely,
            ProofTag::NotDefinite => Tag::NotDefinitely,
            ProofTag::Defeasible => Tag::Defeasibly,
            ProofTag::NotDefeasible => Tag::NotDefeasibly,
        }
    }
}

#[derive(Debug, Subcommand)]
pub(crate) enum ZkCommand {
    /// Check whether a policy fits the supported proof profile
    Check {
        policy: PathBuf,
        /// Public fact-only schema of permitted private inputs
        #[arg(long)]
        inputs: PathBuf,
    },
    /// Compile a reusable public proof policy
    Compile {
        policy: PathBuf,
        /// Public fact-only schema of permitted private inputs
        #[arg(long)]
        inputs: PathBuf,
        /// New output file; existing files are never overwritten
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Prove a claim using a private fact file
    Prove {
        policy: PathBuf,
        #[arg(long)]
        facts: PathBuf,
        #[arg(long)]
        claim: String,
        #[arg(long, value_enum, default_value = "defeasible")]
        tag: ProofTag,
        #[arg(short, long)]
        out: PathBuf,
    },
    /// Verify against an independently selected policy and expected claim
    Verify {
        proof: PathBuf,
        #[arg(long)]
        policy: PathBuf,
        #[arg(long)]
        expect: String,
        #[arg(long, value_enum, default_value = "defeasible")]
        tag: ProofTag,
    },
}

fn zk_error(error: spindle_zk::Error) -> CliError {
    let message = error.to_string();
    match error {
        spindle_zk::Error::ResourceLimit(_) => CliError::resource("ZK_RESOURCE_LIMIT", message),
        spindle_zk::Error::Backend(_) => CliError::execution("ZK_BACKEND_ERROR", message),
        spindle_zk::Error::InvalidProof => CliError::validation("ZK_INVALID_PROOF", message),
        spindle_zk::Error::PolicyMismatch => CliError::validation("ZK_POLICY_MISMATCH", message),
        spindle_zk::Error::ClaimNotEstablished => {
            CliError::validation("ZK_CLAIM_NOT_ESTABLISHED", message)
        }
        spindle_zk::Error::Unsupported(_) => {
            CliError::validation("ZK_UNSUPPORTED_FEATURE", message)
        }
        spindle_zk::Error::Parse(_) | spindle_zk::Error::InvalidInput(_) => {
            CliError::validation("ZK_INVALID_INPUT", message)
        }
    }
}

fn read(path: &Path) -> Result<Vec<u8>, CliError> {
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|_| CliError::validation("ZK_READ_ERROR", "Cannot open input file"))?
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(|_| CliError::validation("ZK_READ_ERROR", "Cannot read input file"))?;
    if bytes.len() > 1_048_576 {
        return Err(CliError::resource(
            "ZK_RESOURCE_LIMIT",
            "Input file exceeds 1 MiB",
        ));
    }
    Ok(bytes)
}

fn source(path: &Path) -> Result<String, CliError> {
    String::from_utf8(read(path)?)
        .map_err(|_| CliError::validation("ZK_INVALID_INPUT", "Input file is not UTF-8"))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), CliError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|_| CliError::execution("ZK_WRITE_ERROR", "Cannot create output file"))?;
    temporary
        .write_all(bytes)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|_| CliError::execution("ZK_WRITE_ERROR", "Cannot write output file"))?;
    temporary.persist_noclobber(path).map_err(|_| {
        CliError::execution(
            "ZK_WRITE_ERROR",
            "Cannot publish output file; choose a new writable path",
        )
    })?;
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn run(command: ZkCommand, json: bool) -> Result<CommandOutput, CliError> {
    let result = match command {
        ZkCommand::Check { policy, inputs } => {
            let policy = Policy::compile(&source(&policy)?, &source(&inputs)?).map_err(zk_error)?;
            serde_json::json!({"action":"check", "status":"supported", "policy_id":hex(&policy.id()), "operations":policy.operation_count(), "profile":"bounded-nonmodal-dl-partial", "fact_authenticity":"unattested"})
        }
        ZkCommand::Compile {
            policy,
            inputs,
            out,
        } => {
            let policy = Policy::compile(&source(&policy)?, &source(&inputs)?).map_err(zk_error)?;
            write_new(&out, &policy.to_json().map_err(zk_error)?)?;
            serde_json::json!({"action":"compile", "status":"compiled", "policy_id":hex(&policy.id()), "operations":policy.operation_count(), "output":out, "fact_authenticity":"unattested"})
        }
        ZkCommand::Prove {
            policy,
            facts,
            claim,
            tag,
            out,
        } => {
            let policy = Policy::from_json(&read(&policy)?).map_err(zk_error)?;
            let claim = Claim::new(&claim, tag.into()).map_err(zk_error)?;
            let facts = parse_facts(&source(&facts)?).map_err(zk_error)?;
            let disclosure = serde_json::json!({
                "public":["policy", "candidate input literals", "claim including identifiers", "randomized fact commitment"],
                "private":["selected facts", "commitment blinding", "intermediate reasoning"],
            });
            if !json {
                eprintln!(
                    "Public claim: {} {}\nPublic: policy, input schema, randomized fact commitment\nPrivate: selected facts, commitment blinding, and intermediate reasoning\nGenerating proof…",
                    claim.tag.conclusion_type().symbol(),
                    claim.literal
                );
            }
            let proof = policy.prove(&facts, &claim).map_err(zk_error)?;
            write_new(&out, &proof.to_json().map_err(zk_error)?)?;
            serde_json::json!({"action":"prove", "status":"proved", "policy_id":hex(&policy.id()), "claim":claim, "output":out, "disclosure":disclosure, "fact_authenticity":"unattested"})
        }
        ZkCommand::Verify {
            proof,
            policy,
            expect,
            tag,
        } => {
            let policy = Policy::from_json(&read(&policy)?).map_err(zk_error)?;
            let proof = Proof::from_json(&read(&proof)?).map_err(zk_error)?;
            let expected = Claim::new(&expect, tag.into()).map_err(zk_error)?;
            let verified = policy.verify(&proof, &expected).map_err(zk_error)?;
            serde_json::json!({"action":"verify", "status":"valid", "policy_id":hex(&verified.policy_id), "claim":verified.claim, "fact_commitment":hex(&verified.fact_commitment), "fact_authenticity":verified.fact_authenticity})
        }
    };
    if json {
        CommandOutput::json(result)
    } else {
        let mut text = format!(
            "{}\nPolicy: {}",
            result["status"].as_str().unwrap_or(""),
            result["policy_id"].as_str().unwrap_or("")
        );
        if let Some(claim) = result.get("claim") {
            text.push_str(&format!(
                "\nClaim: {} {}",
                claim["tag"].as_str().unwrap_or(""),
                claim["literal"].as_str().unwrap_or("")
            ));
        }
        if let Some(output) = result.get("output").and_then(|v| v.as_str()) {
            text.push_str(&format!("\nSaved: {output}"));
        }
        if let Some(operations) = result.get("operations").and_then(|v| v.as_u64()) {
            text.push_str(&format!("\nOperations: {operations}"));
        }
        for (field, label) in [
            ("profile", "Profile"),
            ("fact_commitment", "Fact commitment"),
        ] {
            if let Some(value) = result.get(field).and_then(|v| v.as_str()) {
                text.push_str(&format!("\n{label}: {value}"));
            }
        }
        text.push_str("\nFact authenticity: unattested");
        Ok(CommandOutput::text(text))
    }
}
