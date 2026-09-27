//! Shared transport types for Spindle CLI and WASM output
//!
//! This crate owns the serialization-oriented data transfer objects (DTOs)
//! used by both `spindle-cli` and `spindle-wasm` for structured output.
//! It also adapts portable extension declarations to the core registry.

pub mod diagnostic;
pub mod error;
pub mod literal;
pub mod reason;
pub mod term;
pub mod vocabulary;

pub mod query;

pub mod extensions;
