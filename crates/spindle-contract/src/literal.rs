//! JSON-serializable literal transport types

use serde::{Deserialize, Serialize};

use spindle_core::literal::Literal;
use spindle_core::temporal::{Temporal, TimePoint};

/// JSON-serializable literal representation (contract-compliant).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiteralStructJson {
    pub mode: ModeJson,
    pub negated: bool,
    pub functor: String,
    pub args: Vec<String>,
    pub temporal: TemporalJson,
}

/// JSON-serializable mode structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeJson {
    pub name: Option<String>,
    pub negation: bool,
}

/// JSON-serializable temporal structure.
/// Maps NegInf/PosInf to null per contract schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalJson {
    pub start: Option<i64>,
    pub end: Option<i64>,
}

impl From<&Literal> for LiteralStructJson {
    fn from(literal: &Literal) -> Self {
        Self {
            mode: ModeJson::from(&literal.mode),
            negated: literal.negation,
            functor: literal.name().to_string(),
            args: literal.predicates().iter().map(|s| s.to_string()).collect(),
            temporal: TemporalJson::from(&literal.temporal),
        }
    }
}

impl From<&spindle_core::mode::Mode> for ModeJson {
    fn from(mode: &spindle_core::mode::Mode) -> Self {
        Self {
            name: mode.name.clone(),
            negation: mode.negation,
        }
    }
}

impl From<&Temporal> for TemporalJson {
    fn from(temporal: &Temporal) -> Self {
        Self {
            start: match temporal.start {
                TimePoint::Moment(v) => Some(v),
                TimePoint::NegInf | TimePoint::PosInf => None,
            },
            end: match temporal.end {
                TimePoint::Moment(v) => Some(v),
                TimePoint::NegInf | TimePoint::PosInf => None,
            },
        }
    }
}
