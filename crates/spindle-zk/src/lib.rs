//! Bounded, witness-independent proof compilation for Spindle policies.
//!
//! Includes an optional CLI and finite-domain stratified aggregate compilation.
//! Broader language coverage and independent cryptographic review remain pending.

mod circuit;
mod policy;
mod program;

pub use policy::{Claim, FactAuthenticity, Policy, Proof, Tag, VerifiedClaim, parse_facts};
pub use program::Program;

/// Errors are explicit; unsupported computation never becomes a trusted premise.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Source does not satisfy the SPL grammar.
    #[error("invalid SPL: {0}")]
    Parse(String),
    /// A source construct has no implementation in the selected proof profile.
    #[error("unsupported proof feature: {0}")]
    Unsupported(String),
    /// Input does not satisfy the policy contract.
    #[error("invalid proof input: {0}")]
    InvalidInput(String),
    /// Compilation or proving exceeded an explicit public limit.
    #[error("proof resource limit exceeded: {0}")]
    ResourceLimit(String),
    /// The requested constructive proof tag is not established.
    #[error("the requested claim is not established")]
    ClaimNotEstablished,
    /// Cryptographic backend could not produce the requested result.
    #[error("proof backend error: {0}")]
    Backend(String),
    /// The proof names a different policy from the verifier's trusted policy.
    #[error("proof policy does not match the trusted policy")]
    PolicyMismatch,
    /// Proof or expected claim does not match the proven statement.
    #[error("invalid proof or mismatched claim")]
    InvalidProof,
}

/// Result returned by the proof compiler.
pub type Result<T> = std::result::Result<T, Error>;
