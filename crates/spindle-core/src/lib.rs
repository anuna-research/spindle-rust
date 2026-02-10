//! Spindle Core - Defeasible Logic Reasoning Engine
//!
//! This crate provides the core data structures and reasoning algorithms
//! for defeasible logic, ported from SPINdle-Racket v1.7.0.
//!
//! # Overview
//!
//! Defeasible logic is a non-monotonic reasoning system that allows
//! conclusions to be defeated by stronger evidence. This implementation
//! supports:
//!
//! - Four rule types: facts, strict rules, defeasible rules, and defeaters
//! - Two provability levels: definite (+D/-D) and defeasible (+d/-d)
//! - Superiority relations for conflict resolution
//! - Standard DL(d) and scalable DL(d||) reasoning modes
//!
//! # Example
//!
//! ```rust
//! use spindle_core::prelude::*;
//!
//! // Create a theory about birds
//! let mut theory = Theory::new();
//!
//! // Add facts
//! theory.add_fact("bird");
//! theory.add_fact("penguin");
//!
//! // Add rules
//! let r1 = theory.add_defeasible_rule(&["bird"], "flies");
//! let r2 = theory.add_defeasible_rule(&["penguin"], "~flies");
//!
//! // r2 > r1 (penguins override general bird behavior)
//! theory.add_superiority(&r2, &r1);
//!
//! // Reason and get conclusions
//! let conclusions = theory.reason();
//! ```

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

pub mod claims;
pub mod conclusion;
pub mod error;
pub mod explanation;
pub mod grounding;
pub mod index;
pub mod intern;
pub mod literal;
pub mod mining;
pub mod mode;
pub mod pipeline;
pub mod query;
pub mod reason;
pub mod rule;
pub mod scalable;
pub mod superiority;
pub mod temporal;
pub mod theory;
pub mod trust;
pub mod worklist;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::claims::ClaimsBlock;
    pub use crate::conclusion::{Conclusion, ConclusionType};
    pub use crate::error::{ErrorCategory, Result, SpindleError};
    pub use crate::intern::{
        LiteralId, SymbolId, intern, intern_literal, resolve, resolve_literal,
    };
    pub use crate::literal::{Literal, LiteralName};
    pub use crate::mode::Mode;
    pub use crate::pipeline::{PipelineResult, PrepareOptions};
    pub use crate::query::{QueryResult, QueryStatus, query, query_with_options};
    pub use crate::reason::{reason, reason_with_options};
    pub use crate::rule::{Rule, RuleLabel, RuleType};
    pub use crate::superiority::{Superiority, SuperiorityIndex};
    pub use crate::temporal::{Temporal, TimePoint};
    pub use crate::theory::{Meta, MetaValue, Theory};
}

pub use prelude::*;
