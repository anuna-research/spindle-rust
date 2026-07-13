//! Shared transport types for Spindle CLI and WASM output
//!
//! This crate owns the serialization-oriented data transfer objects (DTOs)
//! used by both `spindle-cli` and `spindle-wasm` for structured output.
//! It contains no reasoning logic.

pub mod diagnostic;
pub mod error;
pub mod literal;
pub mod reason;
pub mod term;
pub mod vocabulary;
