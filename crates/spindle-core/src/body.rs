//! Body literal types for rule bodies (SPEC-017 §7.4).
//!
//! Rule bodies contain two kinds of elements:
//!
//! - **Logic literals** – fact-matching patterns like `(bird ?X)` or
//!   `(not (flies ?X))`. These mirror [`Literal`] but allow arithmetic
//!   expressions in argument positions via [`BodyArg`].
//!
//! - **Arithmetic constraints** – `(bind ?x expr)` and comparison guards
//!   like `(> ?x 10)`, represented by [`ArithConstraint`].
//!
//! The top-level [`BodyLiteral`] enum unifies both under a single type
//! suitable for `Vec<BodyLiteral>` rule bodies.
//!
//! # Design Notes
//!
//! - `BodyLogicLiteral` mirrors the fields of [`Literal`] but uses
//!   `Vec<BodyArg>` instead of `Vec<Term>` for predicate arguments,
//!   allowing arithmetic expressions in argument positions.
//!
//! - `ArithConstraint` has **no** negation flag: REQ-011 makes negation
//!   of arithmetic a parse error, so it is excluded from the AST.

use std::fmt;

use crate::arith::{ArithConstraint, ArithExpr};
use crate::intern::SymbolId;
use crate::literal::{InternedLiteralName, Literal, render_spl_atom};
use crate::mode::Mode;
use crate::temporal::{Temporal, TemporalExpr};
use crate::term::Term;

// ---------------------------------------------------------------------------
// BodyArg
// ---------------------------------------------------------------------------

/// An argument in a body logic literal.
///
/// Body literals allow both concrete terms and arithmetic expressions in
/// predicate argument positions:
///
/// - `(cost ?item (+ ?base ?tax))` → `[BodyArg::Term(?item), BodyArg::Arith(+ ?base ?tax)]`
/// - `(parent alice bob)` → `[BodyArg::Term(alice), BodyArg::Term(bob)]`
#[derive(Clone, Debug, PartialEq)]
pub enum BodyArg {
    /// A concrete term (symbol, integer, decimal, or float).
    Term(Term),
    /// An arithmetic expression that will be evaluated during grounding.
    Arith(ArithExpr),
}

impl fmt::Display for BodyArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BodyArg::Term(t) => write!(f, "{t}"),
            BodyArg::Arith(expr) => write!(f, "{expr}"),
        }
    }
}

impl From<Term> for BodyArg {
    #[inline]
    fn from(t: Term) -> Self {
        BodyArg::Term(t)
    }
}

impl From<ArithExpr> for BodyArg {
    #[inline]
    fn from(expr: ArithExpr) -> Self {
        BodyArg::Arith(expr)
    }
}

// ---------------------------------------------------------------------------
// BodyLogicLiteral
// ---------------------------------------------------------------------------

/// A logic literal in a rule body, used for fact-matching patterns.
///
/// This mirrors [`Literal`](crate::literal::Literal) but uses `Vec<BodyArg>`
/// for predicate arguments, allowing arithmetic expressions alongside
/// concrete terms.
///
/// # Examples (conceptual SPL)
///
/// ```text
/// (bird ?X)              → positive, name="bird", args=[?X]
/// (not (flies ?X))       → negated,  name="flies", args=[?X]
/// (cost ?item (+ ?a ?b)) → positive, name="cost",  args=[?item, (+ ?a ?b)]
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct BodyLogicLiteral {
    /// The interned predicate name.
    name_id: InternedLiteralName,
    /// Whether this literal is negated (`(not ...)`).
    pub negation: bool,
    /// Modal operator (if any).
    pub mode: Mode,
    /// Concrete temporal bounds (if any).
    pub temporal: Temporal,
    /// Pre-grounding temporal expression with possible variables.
    pub temporal_expr: Option<TemporalExpr>,
    /// Predicate arguments, which may include arithmetic expressions.
    predicate_args: Vec<BodyArg>,
    /// Interval variable for single-variable `(during ...)` binding.
    pub interval_var: Option<SymbolId>,
}

impl BodyLogicLiteral {
    /// Create a simple positive body literal with no arguments.
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

    /// Create a negated body literal with no arguments.
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

    /// Create a body literal with full specification.
    pub fn new(
        name: impl AsRef<str>,
        negation: bool,
        mode: Mode,
        temporal: Temporal,
        predicate_args: Vec<BodyArg>,
    ) -> Self {
        Self {
            name_id: InternedLiteralName::intern(name.as_ref()),
            negation,
            mode,
            temporal,
            temporal_expr: None,
            predicate_args,
            interval_var: None,
        }
    }

    /// Create a body literal directly from an interned name and pre-built args.
    #[inline]
    pub fn from_ids(
        name_id: impl Into<InternedLiteralName>,
        negation: bool,
        mode: Mode,
        temporal: Temporal,
        predicate_args: Vec<BodyArg>,
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

    /// Get the predicate name as a string slice.
    #[inline]
    pub fn name(&self) -> &'static str {
        self.name_id.resolve()
    }

    /// Get the interned name wrapper.
    #[inline]
    pub fn interned_name(&self) -> InternedLiteralName {
        self.name_id
    }

    /// Get the interned name ID as a raw `SymbolId`.
    #[inline]
    pub fn name_id(&self) -> SymbolId {
        self.name_id.symbol_id()
    }

    /// Get predicate arguments.
    #[inline]
    pub fn predicate_args(&self) -> &[BodyArg] {
        &self.predicate_args
    }

    /// Check if this literal is positive (not negated).
    #[inline]
    pub fn is_positive(&self) -> bool {
        !self.negation
    }

    /// Check if this literal is negated.
    #[inline]
    pub fn is_negated(&self) -> bool {
        self.negation
    }

    /// Check if this literal has modal operators.
    #[inline]
    pub fn is_modal(&self) -> bool {
        !self.mode.is_empty()
    }

    /// Check if this literal has temporal bounds.
    #[inline]
    pub fn is_temporal(&self) -> bool {
        !self.temporal.is_empty()
    }

    /// Check if this literal has predicate arguments.
    #[inline]
    pub fn is_predicate(&self) -> bool {
        !self.predicate_args.is_empty()
    }

    /// Check if this literal has unresolved temporal variables.
    #[inline]
    pub fn has_temporal_variables(&self) -> bool {
        self.temporal_expr.is_some() || self.interval_var.is_some()
    }

    /// Check if any argument is an arithmetic expression.
    pub fn has_arith_args(&self) -> bool {
        self.predicate_args
            .iter()
            .any(|a| matches!(a, BodyArg::Arith(_)))
    }

    /// Get predicates as strings (for display/serialization).
    ///
    /// Term arguments are resolved to their interned string; arithmetic
    /// expressions use their Display representation.
    pub fn predicates(&self) -> Vec<String> {
        self.predicate_args
            .iter()
            .map(|a| match a {
                BodyArg::Term(t) => match t {
                    Term::Symbol(id) => crate::intern::resolve(*id).to_string(),
                    other => other.to_string(),
                },
                BodyArg::Arith(expr) => expr.to_string(),
            })
            .collect()
    }

    /// Convert this body logic literal to a [`Literal`].
    ///
    /// Arithmetic expression arguments are dropped; only `BodyArg::Term`
    /// arguments survive the conversion.
    pub fn to_literal(&self) -> Literal {
        let terms: Vec<Term> = self
            .predicate_args
            .iter()
            .filter_map(|a| match a {
                BodyArg::Term(t) => Some(t.clone()),
                BodyArg::Arith(_) => None,
            })
            .collect();
        let mut lit = Literal::from_ids(
            self.name_id,
            self.negation,
            self.mode.clone(),
            self.temporal.clone(),
            terms,
        );
        lit.temporal_expr = self.temporal_expr.clone();
        lit.interval_var = self.interval_var;
        lit
    }

    /// Render this literal in canonical SPL s-expression form.
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

impl From<Literal> for BodyLogicLiteral {
    fn from(lit: Literal) -> Self {
        let args: Vec<BodyArg> = lit
            .predicate_args()
            .iter()
            .map(|t| BodyArg::Term(t.clone()))
            .collect();
        let mut bll = BodyLogicLiteral::from_ids(
            lit.interned_name(),
            lit.negation,
            lit.mode.clone(),
            lit.temporal.clone(),
            args,
        );
        bll.temporal_expr = lit.temporal_expr.clone();
        bll.interval_var = lit.interval_var;
        bll
    }
}

impl fmt::Display for BodyLogicLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.mode.is_empty() {
            write!(f, "{}", self.mode)?;
        }
        if self.negation {
            write!(f, "~")?;
        }
        write!(f, "{}", self.name())?;
        if !self.predicate_args.is_empty() {
            let args: Vec<_> = self.predicate_args.iter().map(|a| a.to_string()).collect();
            write!(f, "({})", args.join(", "))?;
        }
        if !self.temporal.is_empty() {
            write!(f, "{}", self.temporal)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// BodyLiteral
// ---------------------------------------------------------------------------

/// A single element in a rule body.
///
/// Rule bodies are heterogeneous sequences of logic literals (for
/// fact-matching) and arithmetic constraints (for binding variables
/// and comparing values).
///
/// # Variants
///
/// - `Logic` – a predicate pattern that matches against facts/conclusions
/// - `Arithmetic` – a `bind` or comparison constraint
///
/// Note: `Arithmetic` has **no** negation wrapper. REQ-011 specifies
/// that `(not (bind ...))` and `(not (> ...))` are parse errors.
#[derive(Clone, Debug, PartialEq)]
pub enum BodyLiteral {
    /// A logic literal for fact-matching.
    Logic(BodyLogicLiteral),
    /// An arithmetic constraint (bind or compare).
    Arithmetic(ArithConstraint),
}

impl BodyLiteral {
    /// Create a simple positive logic body literal (convenience for construction).
    #[inline]
    pub fn simple(name: impl AsRef<str>) -> Self {
        BodyLiteral::Logic(BodyLogicLiteral::simple(name))
    }

    /// Create a negated logic body literal (convenience for construction).
    #[inline]
    pub fn negated(name: impl AsRef<str>) -> Self {
        BodyLiteral::Logic(BodyLogicLiteral::negated(name))
    }

    /// Returns `true` if this is a logic literal.
    #[inline]
    pub fn is_logic(&self) -> bool {
        matches!(self, BodyLiteral::Logic(_))
    }

    /// Returns `true` if this is an arithmetic constraint.
    #[inline]
    pub fn is_arithmetic(&self) -> bool {
        matches!(self, BodyLiteral::Arithmetic(_))
    }

    /// If this is a logic literal, return a reference to it.
    #[inline]
    pub fn as_logic(&self) -> Option<&BodyLogicLiteral> {
        match self {
            BodyLiteral::Logic(lit) => Some(lit),
            _ => None,
        }
    }

    /// If this is an arithmetic constraint, return a reference to it.
    #[inline]
    pub fn as_arithmetic(&self) -> Option<&ArithConstraint> {
        match self {
            BodyLiteral::Arithmetic(c) => Some(c),
            _ => None,
        }
    }

    /// Get the predicate name (logic literals only; returns `"<arith>"` for arithmetic).
    #[inline]
    pub fn name(&self) -> &'static str {
        match self {
            BodyLiteral::Logic(lit) => lit.name(),
            BodyLiteral::Arithmetic(_) => "<arith>",
        }
    }

    /// Negation-aware canonical name (e.g. `"~flies"` or `"bird"`).
    ///
    /// Arithmetic constraints return `"<arith>"`.
    pub fn canonical_name(&self) -> String {
        match self {
            BodyLiteral::Logic(lit) => {
                if lit.is_negated() {
                    format!("~{}", lit.name())
                } else {
                    lit.name().to_owned()
                }
            }
            BodyLiteral::Arithmetic(_) => "<arith>".to_owned(),
        }
    }

    /// Check if this body literal is negated.
    ///
    /// Arithmetic constraints are never negated (REQ-011).
    #[inline]
    pub fn is_negated(&self) -> bool {
        match self {
            BodyLiteral::Logic(lit) => lit.is_negated(),
            BodyLiteral::Arithmetic(_) => false,
        }
    }

    /// Check if this body literal is positive (not negated).
    #[inline]
    pub fn is_positive(&self) -> bool {
        !self.is_negated()
    }

    /// Check if this body literal has temporal bounds.
    #[inline]
    pub fn is_temporal(&self) -> bool {
        match self {
            BodyLiteral::Logic(lit) => lit.is_temporal(),
            BodyLiteral::Arithmetic(_) => false,
        }
    }

    /// Check if this body literal has unresolved temporal variables.
    #[inline]
    pub fn has_temporal_variables(&self) -> bool {
        match self {
            BodyLiteral::Logic(lit) => lit.has_temporal_variables(),
            BodyLiteral::Arithmetic(_) => false,
        }
    }

    /// Render this body literal in canonical SPL s-expression form.
    pub fn to_spl(&self) -> String {
        match self {
            BodyLiteral::Logic(lit) => lit.to_spl(),
            BodyLiteral::Arithmetic(c) => c.to_string(),
        }
    }

    /// Get predicates as strings (logic literals only; returns empty for arithmetic).
    pub fn predicates(&self) -> Vec<String> {
        match self {
            BodyLiteral::Logic(lit) => lit.predicates(),
            BodyLiteral::Arithmetic(_) => Vec::new(),
        }
    }
}

impl From<BodyLogicLiteral> for BodyLiteral {
    #[inline]
    fn from(lit: BodyLogicLiteral) -> Self {
        BodyLiteral::Logic(lit)
    }
}

impl From<ArithConstraint> for BodyLiteral {
    #[inline]
    fn from(c: ArithConstraint) -> Self {
        BodyLiteral::Arithmetic(c)
    }
}

impl From<Literal> for BodyLiteral {
    #[inline]
    fn from(lit: Literal) -> Self {
        BodyLiteral::Logic(BodyLogicLiteral::from(lit))
    }
}

impl fmt::Display for BodyLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BodyLiteral::Logic(lit) => write!(f, "{lit}"),
            BodyLiteral::Arithmetic(c) => write!(f, "{c}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arith::{ArithExpr, CmpOp};
    use crate::intern::intern;
    use crate::term::{NumericValue, Term};

    // -- BodyArg tests ------------------------------------------------------

    #[test]
    fn body_arg_term_display() {
        let arg = BodyArg::Term(Term::Symbol(intern("alice")));
        assert_eq!(format!("{arg}"), "alice");
    }

    #[test]
    fn body_arg_arith_display() {
        let arg = BodyArg::Arith(ArithExpr::Var(intern("?x")));
        assert_eq!(format!("{arg}"), "?x");
    }

    #[test]
    fn body_arg_from_term() {
        let t = Term::Integer(42);
        let arg: BodyArg = t.clone().into();
        assert_eq!(arg, BodyArg::Term(t));
    }

    #[test]
    fn body_arg_from_arith_expr() {
        let expr = ArithExpr::Lit(NumericValue::Integer(5));
        let arg: BodyArg = expr.clone().into();
        assert_eq!(arg, BodyArg::Arith(expr));
    }

    // -- BodyLogicLiteral tests ---------------------------------------------

    #[test]
    fn simple_body_logic_literal() {
        let lit = BodyLogicLiteral::simple("bird");
        assert_eq!(lit.name(), "bird");
        assert!(!lit.negation);
        assert!(lit.is_positive());
        assert!(!lit.is_negated());
        assert!(!lit.is_predicate());
        assert!(!lit.is_modal());
        assert!(!lit.is_temporal());
    }

    #[test]
    fn negated_body_logic_literal() {
        let lit = BodyLogicLiteral::negated("flies");
        assert_eq!(lit.name(), "flies");
        assert!(lit.negation);
        assert!(lit.is_negated());
        assert!(!lit.is_positive());
    }

    #[test]
    fn body_logic_literal_with_args() {
        let lit = BodyLogicLiteral::new(
            "parent",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(intern("alice"))),
                BodyArg::Term(Term::Symbol(intern("bob"))),
            ],
        );
        assert!(lit.is_predicate());
        assert!(!lit.has_arith_args());
        assert_eq!(lit.predicate_args().len(), 2);
    }

    #[test]
    fn body_logic_literal_with_arith_arg() {
        let lit = BodyLogicLiteral::new(
            "cost",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(intern("?item"))),
                BodyArg::Arith(ArithExpr::Var(intern("?total"))),
            ],
        );
        assert!(lit.has_arith_args());
    }

    #[test]
    fn body_logic_literal_display() {
        let lit = BodyLogicLiteral::new(
            "parent",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec![
                BodyArg::Term(Term::Symbol(intern("alice"))),
                BodyArg::Term(Term::Symbol(intern("bob"))),
            ],
        );
        assert_eq!(format!("{lit}"), "parent(alice, bob)");
    }

    #[test]
    fn body_logic_literal_display_negated() {
        let lit = BodyLogicLiteral::negated("flies");
        assert_eq!(format!("{lit}"), "~flies");
    }

    #[test]
    fn body_logic_literal_from_ids() {
        let name_id = InternedLiteralName::intern("test");
        let lit =
            BodyLogicLiteral::from_ids(name_id, false, Mode::empty(), Temporal::empty(), vec![]);
        assert_eq!(lit.name(), "test");
        assert_eq!(lit.interned_name(), name_id);
        assert_eq!(lit.name_id(), name_id.symbol_id());
    }

    // -- BodyLiteral tests --------------------------------------------------

    #[test]
    fn body_literal_logic_variant() {
        let logic = BodyLogicLiteral::simple("bird");
        let bl = BodyLiteral::Logic(logic.clone());
        assert!(bl.is_logic());
        assert!(!bl.is_arithmetic());
        assert_eq!(bl.as_logic(), Some(&logic));
        assert_eq!(bl.as_arithmetic(), None);
    }

    #[test]
    fn body_literal_arithmetic_variant() {
        let constraint = ArithConstraint::Bind {
            var: intern("?x"),
            expr: ArithExpr::Lit(NumericValue::Integer(42)),
        };
        let bl = BodyLiteral::Arithmetic(constraint.clone());
        assert!(bl.is_arithmetic());
        assert!(!bl.is_logic());
        assert_eq!(bl.as_arithmetic(), Some(&constraint));
        assert_eq!(bl.as_logic(), None);
    }

    #[test]
    fn body_literal_from_logic() {
        let logic = BodyLogicLiteral::simple("test");
        let bl: BodyLiteral = logic.into();
        assert!(bl.is_logic());
    }

    #[test]
    fn body_literal_from_arith_constraint() {
        let constraint = ArithConstraint::Compare {
            op: CmpOp::Gt,
            lhs: ArithExpr::Var(intern("?x")),
            rhs: ArithExpr::Lit(NumericValue::Integer(10)),
        };
        let bl: BodyLiteral = constraint.into();
        assert!(bl.is_arithmetic());
    }

    #[test]
    fn body_literal_display_logic() {
        let bl = BodyLiteral::Logic(BodyLogicLiteral::simple("bird"));
        assert_eq!(format!("{bl}"), "bird");
    }

    #[test]
    fn body_literal_display_arithmetic() {
        let bl = BodyLiteral::Arithmetic(ArithConstraint::Bind {
            var: intern("?x"),
            expr: ArithExpr::Lit(NumericValue::Integer(42)),
        });
        let display = format!("{bl}");
        assert!(display.contains("bind"));
    }

    #[test]
    fn body_literal_clone_eq() {
        let bl1 = BodyLiteral::Logic(BodyLogicLiteral::simple("bird"));
        let bl2 = bl1.clone();
        assert_eq!(bl1, bl2);
    }

    #[test]
    fn body_logic_literal_modal() {
        let lit =
            BodyLogicLiteral::new("pay", false, Mode::obligation(), Temporal::empty(), vec![]);
        assert!(lit.is_modal());
        let display = format!("{lit}");
        assert_eq!(display, "[O]pay");
    }
}
