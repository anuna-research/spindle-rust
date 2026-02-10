//! Spindle Parser - DFL and SPL Format Parsing
//!
//! This crate provides parsers for the two input formats supported by Spindle:
//!
//! - **DFL (Defeasible Logic Format)**: A textual format for defeasible logic theories
//! - **SPL (Spindle Lisp)**: A LISP-based DSL for expressing theories
//!
//! # DFL Format Example
//!
//! ```text
//! # Facts
//! f1: >> bird
//! f2: >> penguin
//!
//! # Defeasible rules
//! r1: bird => flies
//! r2: penguin => -flies
//!
//! # Superiority
//! r2 > r1
//! ```
//!
//! # SPL Format Example
//!
//! ```lisp
//! (given bird)
//! (given penguin)
//! (normally r1 bird flies)
//! (normally r2 penguin (not flies))
//! (prefer r2 r1)
//! ```

#![warn(missing_docs)]

pub mod dfl;
pub mod error;
pub mod spl;

pub use dfl::parse_dfl;
pub use error::{ParseError, ParserFormat};
pub use spl::parse_spl;

#[cfg(test)]
mod format_equivalence_tests;
