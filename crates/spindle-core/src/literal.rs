//! Literals - the fundamental units of defeasible reasoning
//!
//! A literal represents a proposition that can be true or false,
//! optionally with modal operators and temporal bounds.
//!
//! # Performance
//!
//! Literal names are stored as interned `SymbolId` values (4 bytes)
//! instead of heap-allocated `String`s (24 bytes). This eliminates
//! allocations in hot paths of the reasoning engine.

use std::fmt;
use std::hash::{Hash, Hasher};

use crate::intern::{LiteralId, SymbolId, intern, resolve};
use crate::mode::Mode;
use crate::temporal::{Temporal, TemporalExpr};
use crate::term::Term;

/// A newtype wrapping `SymbolId` for internal literal-name storage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(transparent)]
pub struct InternedLiteralName(pub(crate) SymbolId);

impl InternedLiteralName {
    /// The empty / default literal name (wraps `SymbolId::EMPTY`).
    pub const EMPTY: InternedLiteralName = InternedLiteralName(SymbolId::EMPTY);

    /// Create an interned literal name from a string.
    #[inline]
    pub fn intern(name: &str) -> Self {
        InternedLiteralName(intern(name))
    }

    /// Resolve this name back to a string slice.
    #[inline]
    pub fn resolve(self) -> &'static str {
        resolve(self.0)
    }

    /// Access the underlying `SymbolId`.
    #[inline]
    pub const fn symbol_id(self) -> SymbolId {
        self.0
    }
}

impl From<SymbolId> for InternedLiteralName {
    #[inline]
    fn from(id: SymbolId) -> Self {
        InternedLiteralName(id)
    }
}

impl From<InternedLiteralName> for SymbolId {
    #[inline]
    fn from(name: InternedLiteralName) -> Self {
        name.0
    }
}

impl From<&str> for InternedLiteralName {
    /// Create an interned literal name from a string.
    #[inline]
    fn from(s: &str) -> Self {
        InternedLiteralName::intern(s)
    }
}

impl From<String> for InternedLiteralName {
    /// Create an interned literal name from a string.
    #[inline]
    fn from(s: String) -> Self {
        InternedLiteralName::intern(&s)
    }
}

impl AsRef<str> for InternedLiteralName {
    #[inline]
    fn as_ref(&self) -> &str {
        self.resolve()
    }
}

impl PartialEq<str> for InternedLiteralName {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.resolve() == other
    }
}

impl PartialEq<&str> for InternedLiteralName {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.resolve() == *other
    }
}

impl PartialEq<String> for InternedLiteralName {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.resolve() == other.as_str()
    }
}

impl std::fmt::Display for InternedLiteralName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.resolve())
    }
}

/// Public literal-name type (kept source-compatible with pre-refactor API).
pub type LiteralName = String;

/// Backward-compatible alias for callers that used this name.
pub type LiteralNameString = LiteralName;

/// A literal in defeasible logic
///
/// Literals are the atomic propositions that rules reason about.
/// They can be negated, have modal operators, temporal bounds,
/// and predicate arguments.
///
/// # Performance
///
/// The name is stored as an interned `SymbolId` (4 bytes, Copy) instead
/// of a `String` (24 bytes, heap-allocated). Use `literal_id()` for
/// O(1) HashSet/HashMap operations instead of `canonical_name()`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct Literal {
    /// The interned name of the literal (e.g., "flies", "bird")
    name_id: InternedLiteralName,
    /// Whether this literal is negated
    pub negation: bool,
    /// Modal operator (if any)
    pub mode: Mode,
    /// Temporal bounds (if any)
    pub temporal: Temporal,
    /// Pre-grounding temporal expression with possible variables.
    ///
    /// When `Some`, this literal was parsed with temporal variable endpoints
    /// (e.g., `(during lit ?t1 ?t2)`). After grounding resolves the variables,
    /// this is converted to a concrete `temporal` and set back to `None`.
    pub temporal_expr: Option<TemporalExpr>,
    /// Predicate arguments (e.g., for parent(alice, bob))
    /// Stored as typed [`Term`] values to support symbols, integers, decimals, and floats.
    predicate_args: Vec<Term>,
    /// Interval variable for single-variable `(during ...)` binding.
    ///
    /// When set, this literal was parsed with `(during (lit) ?T)` and the
    /// interval variable `?T` will be bound to the matched fact's full
    /// `Temporal` during grounding. Mutually exclusive with `temporal_expr`.
    pub interval_var: Option<SymbolId>,
}

/// Stable, semantic literal representation for JSON outputs
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralStruct {
    /// Modal operator information.
    pub mode: Mode,
    /// Whether the literal is negated.
    pub negated: bool,
    /// Predicate functor name.
    pub functor: String,
    /// Predicate arguments.
    pub args: Vec<String>,
    /// Temporal bounds for the literal.
    pub temporal: Temporal,
}

impl Literal {
    /// Create a simple positive literal
    pub fn simple(name: impl AsRef<str>) -> Self {
        Self {
            name_id: InternedLiteralName::intern(name.as_ref()),
            negation: false,
            mode: Mode::empty(),
            temporal: Temporal::empty(),
            temporal_expr: None,
            predicate_args: Vec::new(),
            interval_var: None,
        }
    }

    /// Create a negated literal
    pub fn negated(name: impl AsRef<str>) -> Self {
        Self {
            name_id: InternedLiteralName::intern(name.as_ref()),
            negation: true,
            mode: Mode::empty(),
            temporal: Temporal::empty(),
            temporal_expr: None,
            predicate_args: Vec::new(),
            interval_var: None,
        }
    }

    /// Create a literal with full specification
    pub fn new(
        name: impl AsRef<str>,
        negation: bool,
        mode: Mode,
        temporal: Temporal,
        predicates: Vec<String>,
    ) -> Self {
        Self {
            name_id: InternedLiteralName::intern(name.as_ref()),
            negation,
            mode,
            temporal,
            temporal_expr: None,
            predicate_args: predicates.iter().map(|s| Term::Symbol(intern(s))).collect(),
            interval_var: None,
        }
    }

    /// Create a literal with a temporal expression (pre-grounding form).
    ///
    /// The concrete `temporal` field is left empty; it will be resolved
    /// during grounding when variable bindings become available.
    pub fn new_with_temporal_expr(
        name: impl AsRef<str>,
        negation: bool,
        mode: Mode,
        temporal_expr: TemporalExpr,
        predicates: Vec<String>,
    ) -> Self {
        // If the expression is fully concrete, resolve it immediately
        let (temporal, expr) = if let Some(concrete) = temporal_expr.to_concrete() {
            (concrete, None)
        } else {
            (Temporal::empty(), Some(temporal_expr))
        };
        Self {
            name_id: InternedLiteralName::intern(name.as_ref()),
            negation,
            mode,
            temporal,
            temporal_expr: expr,
            predicate_args: predicates.iter().map(|s| Term::Symbol(intern(s))).collect(),
            interval_var: None,
        }
    }

    /// Create a literal directly from pre-built [`Term`] arguments (zero string allocation).
    ///
    /// This is the optimized constructor for use when terms are already
    /// available, such as during grounding operations.
    ///
    /// Accepts anything convertible to `InternedLiteralName` (including
    /// `SymbolId`) for the functor name.
    #[inline]
    pub fn from_ids(
        name_id: impl Into<InternedLiteralName>,
        negation: bool,
        mode: Mode,
        temporal: Temporal,
        predicate_args: Vec<Term>,
    ) -> Self {
        Self {
            name_id: name_id.into(),
            negation,
            mode,
            temporal,
            temporal_expr: None,
            predicate_args,
            interval_var: None,
        }
    }

    /// Return the complement (negation flipped) of this literal
    ///
    /// This is a cheap operation as it only flips a boolean and
    /// copies the SymbolId (no heap allocation).
    pub fn complement(&self) -> Self {
        Self {
            name_id: self.name_id,
            negation: !self.negation,
            mode: self.mode.clone(),
            temporal: self.temporal.clone(),
            temporal_expr: self.temporal_expr.clone(),
            predicate_args: self.predicate_args.clone(),
            interval_var: self.interval_var,
        }
    }

    /// Get the literal name as a string slice
    ///
    /// This resolves the interned name back to its string.
    #[inline]
    pub fn name(&self) -> &'static str {
        self.name_id.resolve()
    }

    /// Get the public literal-name representation.
    ///
    /// This allocates a `String` and is kept for source compatibility.
    #[inline]
    pub fn literal_name(&self) -> LiteralName {
        self.name().to_string()
    }

    /// Get the interned name wrapper (non-allocating).
    #[inline]
    pub fn interned_name(&self) -> InternedLiteralName {
        self.name_id
    }

    /// Get the interned name ID as a raw `SymbolId` (for advanced use / backward compat).
    #[inline]
    pub fn name_id(&self) -> SymbolId {
        self.name_id.symbol_id()
    }

    /// Get predicates as strings (for display/serialization).
    ///
    /// Symbol terms are resolved to their interned string; non-symbol terms
    /// (Integer, Decimal, Float) use their `Display` representation.
    pub fn predicates(&self) -> Vec<&'static str> {
        self.predicate_args
            .iter()
            .map(|t| match t {
                Term::Symbol(id) => resolve(*id),
                // Leak a short string for non-symbol terms so the return type
                // stays `&'static str` (same as the symbol branch).  These are
                // rare in practice; the majority of call sites that care about
                // efficiency should use `predicate_args()` instead.
                other => {
                    let s = other.to_string();
                    Box::leak(s.into_boxed_str()) as &'static str
                }
            })
            .collect()
    }

    /// Get predicate arguments as [`Term`] slices (for efficient comparison).
    #[inline]
    pub fn predicate_args(&self) -> &[Term] {
        &self.predicate_args
    }

    /// Check if this literal is positive (not negated)
    #[inline]
    pub fn is_positive(&self) -> bool {
        !self.negation
    }

    /// Check if this literal is negated
    #[inline]
    pub fn is_negated(&self) -> bool {
        self.negation
    }

    /// Check if this literal has modal operators
    #[inline]
    pub fn is_modal(&self) -> bool {
        !self.mode.is_empty()
    }

    /// Check if this literal has temporal bounds
    #[inline]
    pub fn is_temporal(&self) -> bool {
        !self.temporal.is_empty()
    }

    /// Check if this literal has predicate arguments
    #[inline]
    pub fn is_predicate(&self) -> bool {
        !self.predicate_args.is_empty()
    }

    /// Check if this literal has unresolved temporal variables.
    #[inline]
    pub fn has_temporal_variables(&self) -> bool {
        self.temporal_expr.is_some() || self.interval_var.is_some()
    }

    /// Get the LiteralId for this literal (name + negation combined).
    ///
    /// **WARNING:** This ID is based on name + negation ONLY. It ignores
    /// predicate arguments, modes, and temporals. Do NOT use this for
    /// reasoning correctness checks where p(a) vs p(b) matters.
    ///
    /// This is the preferred method for HashSet/HashMap keys when only
    /// the name/arity matters (e.g. legacy lookups), as it's
    /// a 4-byte Copy type with O(1) hashing.
    ///
    /// # Example
    ///
    /// ```rust
    /// use spindle_core::prelude::*;
    /// use std::collections::HashSet;
    ///
    /// let mut proven: HashSet<LiteralId> = HashSet::new();
    /// let bird = Literal::simple("bird");
    ///
    /// proven.insert(bird.name_literal_id());
    /// assert!(proven.contains(&bird.name_literal_id()));
    /// ```
    #[inline]
    pub fn name_literal_id(&self) -> LiteralId {
        LiteralId::new(self.name_id.symbol_id(), self.negation)
    }

    /// Get the canonical name for indexing (includes negation)
    ///
    /// **Note:** This method allocates a new String. For performance-critical
    /// code, use `literal_id()` instead which returns a Copy type.
    ///
    /// Kept for backward compatibility with existing code.
    pub fn canonical_name(&self) -> String {
        if self.negation {
            format!("~{}", self.name())
        } else {
            self.name().to_owned()
        }
    }

    /// Render this literal in canonical SPL s-expression form
    pub fn to_spl(&self) -> String {
        let mut parts = Vec::with_capacity(1 + self.predicate_args.len());
        parts.push(render_spl_atom(self.name()));
        for arg in &self.predicate_args {
            parts.push(render_spl_atom(&arg.to_string()));
        }

        let mut expr = format!("({})", parts.join(" "));

        if self.negation {
            expr = format!("(not {expr})");
        }

        if !self.mode.is_empty() {
            let tag = match self.mode.name.as_deref() {
                Some("O") => "must",
                Some("P") => "may",
                Some("F") => "forbidden",
                Some(name) => name,
                None => "",
            };

            if !tag.is_empty() {
                if self.mode.negation {
                    expr = format!("(not ({tag} {expr}))");
                } else {
                    expr = format!("({tag} {expr})");
                }
            }
        }

        if let Some(var_id) = self.interval_var {
            expr = format!("(during {expr} {})", crate::intern::resolve(var_id));
        } else if let Some(ref texpr) = self.temporal_expr {
            let start_spl = match &texpr.start {
                crate::temporal::TimeExpr::Const(tp) => tp.to_spl(),
                crate::temporal::TimeExpr::Var(id) => crate::intern::resolve(*id).to_string(),
            };
            let end_spl = match &texpr.end {
                crate::temporal::TimeExpr::Const(tp) => tp.to_spl(),
                crate::temporal::TimeExpr::Var(id) => crate::intern::resolve(*id).to_string(),
            };
            expr = format!("(during {expr} {start_spl} {end_spl})");
        } else if !self.temporal.is_empty() {
            expr = format!(
                "(during {expr} {} {})",
                self.temporal.start.to_spl(),
                self.temporal.end.to_spl()
            );
        }

        expr
    }
}

fn is_spl_atom_char(c: char) -> bool {
    c.is_alphanumeric()
        || c == '-'
        || c == '_'
        || c == '?'
        || c == '~'
        || c == ':'
        || c == '.'
        || c == '+'
}

pub(crate) fn render_spl_atom(atom: &str) -> String {
    let needs_quotes = atom.is_empty() || atom.chars().any(|c| !is_spl_atom_char(c));
    if !needs_quotes {
        return atom.to_string();
    }

    let mut escaped = String::with_capacity(atom.len() + 2);
    for c in atom.chars() {
        match c {
            '\\' => {
                escaped.push('\\');
                escaped.push('\\');
            }
            '"' => {
                escaped.push('\\');
                escaped.push('"');
            }
            _ => escaped.push(c),
        }
    }

    format!("\"{escaped}\"")
}

impl From<&Literal> for LiteralStruct {
    fn from(literal: &Literal) -> Self {
        Self {
            mode: literal.mode.clone(),
            negated: literal.negation,
            functor: literal.name().to_string(),
            args: literal
                .predicate_args
                .iter()
                .map(|t| t.to_string())
                .collect(),
            temporal: literal.temporal.clone(),
        }
    }
}

impl From<Literal> for LiteralStruct {
    fn from(literal: Literal) -> Self {
        Self::from(&literal)
    }
}

impl PartialEq for Literal {
    fn eq(&self, other: &Self) -> bool {
        self.name_id == other.name_id
            && self.negation == other.negation
            && self.mode == other.mode
            && self.predicate_args == other.predicate_args
        // Note: temporal is not included in equality for reasoning purposes
    }
}

impl Eq for Literal {}

impl Hash for Literal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name_id.hash(state);
        self.negation.hash(state);
        self.mode.name.hash(state);
        self.mode.negation.hash(state);
        self.predicate_args.hash(state);
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Mode prefix
        if !self.mode.is_empty() {
            write!(f, "{}", self.mode)?;
        }

        // Negation
        if self.negation {
            write!(f, "~")?;
        }

        // Name
        write!(f, "{}", self.name())?;

        // Predicates
        if !self.predicate_args.is_empty() {
            let preds: Vec<_> = self.predicate_args.iter().map(|t| t.to_string()).collect();
            write!(f, "({})", preds.join(", "))?;
        }

        // Temporal
        if !self.temporal.is_empty() {
            write!(f, "{}", self.temporal)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::temporal::TimePoint;

    #[test]
    fn test_simple_literal() {
        let lit = Literal::simple("bird");
        assert_eq!(lit.name(), "bird");
        assert!(!lit.negation);
        assert!(lit.mode.is_empty());
    }

    #[test]
    fn test_negated_literal() {
        let lit = Literal::negated("flies");
        assert_eq!(lit.name(), "flies");
        assert!(lit.negation);
    }

    #[test]
    fn test_complement() {
        let lit = Literal::simple("bird");
        let comp = lit.complement();
        assert_eq!(comp.name(), "bird");
        assert!(comp.negation);

        let comp2 = comp.complement();
        assert!(!comp2.negation);
    }

    #[test]
    fn test_display() {
        let lit = Literal::simple("bird");
        assert_eq!(format!("{lit}"), "bird");

        let neg = Literal::negated("flies");
        assert_eq!(format!("{neg}"), "~flies");
    }

    #[test]
    fn test_to_spl_quotes_atoms_with_spaces_and_escapes() {
        let lit = Literal::new(
            "p",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                "doc 1".to_string(),
                "quote\"me".to_string(),
                "path\\to".to_string(),
            ],
        );

        assert_eq!(lit.to_spl(), "(p \"doc 1\" \"quote\\\"me\" \"path\\\\to\")");
    }

    #[test]
    fn test_name_literal_id() {
        let pos = Literal::simple("bird");
        let neg = Literal::negated("bird");

        assert!(!pos.name_literal_id().is_negated());
        assert!(neg.name_literal_id().is_negated());
        assert_eq!(
            pos.name_literal_id().symbol(),
            neg.name_literal_id().symbol()
        );
        assert_eq!(pos.name_literal_id(), neg.name_literal_id().complement());
    }

    #[test]
    fn test_name_literal_id_in_hashset() {
        use std::collections::HashSet;

        let mut set: HashSet<LiteralId> = HashSet::new();
        let bird = Literal::simple("bird");
        let not_bird = Literal::negated("bird");
        let flies = Literal::simple("flies");

        set.insert(bird.name_literal_id());
        set.insert(flies.name_literal_id());

        assert!(set.contains(&bird.name_literal_id()));
        assert!(set.contains(&flies.name_literal_id()));
        assert!(!set.contains(&not_bird.name_literal_id()));
    }

    #[test]
    fn test_canonical_name_compat() {
        let pos = Literal::simple("bird");
        let neg = Literal::negated("flies");

        assert_eq!(pos.canonical_name(), "bird");
        assert_eq!(neg.canonical_name(), "~flies");
    }

    #[test]
    fn test_predicates() {
        let lit = Literal::new(
            "parent",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["alice".to_string(), "bob".to_string()],
        );

        let preds = lit.predicates();
        assert_eq!(preds, vec!["alice", "bob"]);
    }

    #[test]
    fn test_is_positive() {
        let pos = Literal::simple("bird");
        let neg = Literal::negated("bird");

        assert!(pos.is_positive());
        assert!(!neg.is_positive());
    }

    #[test]
    fn test_is_modal() {
        let simple = Literal::simple("bird");
        assert!(!simple.is_modal());

        let modal = Literal::new("pay", false, Mode::obligation(), Temporal::empty(), vec![]);
        assert!(modal.is_modal());
    }

    #[test]
    fn test_is_temporal() {
        let simple = Literal::simple("bird");
        assert!(!simple.is_temporal());

        let temporal = Literal::new(
            "valid",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(2024), TimePoint::Moment(2025)),
            vec![],
        );
        assert!(temporal.is_temporal());
    }

    #[test]
    fn test_is_predicate() {
        let simple = Literal::simple("bird");
        assert!(!simple.is_predicate());

        let pred = Literal::new(
            "parent",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["alice".to_string()],
        );
        assert!(pred.is_predicate());
    }

    #[test]
    fn test_display_with_mode() {
        let modal = Literal::new("pay", false, Mode::obligation(), Temporal::empty(), vec![]);
        assert_eq!(format!("{modal}"), "[O]pay");
    }

    #[test]
    fn test_display_with_predicates() {
        let pred = Literal::new(
            "parent",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["alice".to_string(), "bob".to_string()],
        );
        assert_eq!(format!("{pred}"), "parent(alice, bob)");
    }

    #[test]
    fn test_display_with_temporal() {
        let temporal = Literal::new(
            "valid",
            false,
            Mode::empty(),
            Temporal::new(TimePoint::Moment(2024), TimePoint::Moment(2025)),
            vec![],
        );
        let display = format!("{temporal}");
        assert!(display.contains("valid"));
        assert!(display.contains("["));
    }

    #[test]
    fn test_literal_name_source_compatibility() {
        let name: LiteralName = "bird".to_string();
        let copy: String = name.clone();
        assert_eq!(name, "bird");
        assert_eq!(copy, "bird");
    }

    #[test]
    fn test_literal_name_method_returns_string() {
        let lit = Literal::simple("bird");
        let name: LiteralName = lit.literal_name();
        assert_eq!(name, "bird".to_string());
    }
}
