//! Spindle Parser - SPL Format Parsing
//!
//! This crate provides an SPL (Spindle Lisp) parser for defeasible logic theories.
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

pub mod error;
pub mod spl;

pub use error::{ParseError, ParserFormat};
pub use spl::parse_spl;
