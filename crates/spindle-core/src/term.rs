//! Term representation for Spindle's typed value system.
//!
//! Terms represent the values that can appear in literals and expressions.
//! The [`Term`] enum supports four value types:
//!
//! - **Symbol**: An interned string identifier (via [`SymbolId`])
//! - **Integer**: A 64-bit signed integer
//! - **Decimal**: An arbitrary-precision decimal (via [`rust_decimal::Decimal`])
//! - **Float**: A canonicalized IEEE 754 float (via [`FiniteFloat`])
//!
//! # Numeric Cross-Type Comparison (REQ-005)
//!
//! The [`Term::numeric_eq`] method supports cross-type numeric equality:
//! `Integer(2)` equals `Decimal(2.0)` equals `Float(2.0)`. Promotion follows
//! the widening chain: Integer → Decimal → Float.
//!
//! # Float Canonicalization
//!
//! [`FiniteFloat`] rejects NaN and infinity, normalizes `-0.0` to `+0.0`,
//! and provides stable `Eq` + `Hash` implementations suitable for use in
//! hash maps and sets.

use std::fmt;
use std::hash::{Hash, Hasher};

use rust_decimal::Decimal;

use crate::intern::{SymbolId, resolve};

// ---------------------------------------------------------------------------
// FiniteFloat — canonicalized, hashable f64 wrapper
// ---------------------------------------------------------------------------

/// A finite IEEE 754 f64 that rejects NaN/Inf and normalizes `-0.0` → `+0.0`.
///
/// This wrapper provides stable `Eq` and `Hash` implementations by ensuring:
/// 1. Only finite values are representable (no NaN, no ±Inf).
/// 2. Negative zero is canonicalized to positive zero.
///
/// These invariants guarantee that `==` is reflexive and `Hash` is consistent
/// with `Eq`, making `FiniteFloat` safe for use as a hash-map key.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct FiniteFloat(f64);

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for FiniteFloat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let v = f64::deserialize(deserializer)?;
        FiniteFloat::new(v)
            .ok_or_else(|| serde::de::Error::custom(format!("expected finite float, got {v}")))
    }
}

impl FiniteFloat {
    /// Create a new `FiniteFloat`, returning `None` if the value is NaN or infinite.
    ///
    /// Negative zero is silently normalized to positive zero.
    #[inline]
    pub fn new(value: f64) -> Option<Self> {
        if value.is_finite() {
            Some(Self(canonicalize(value)))
        } else {
            None
        }
    }

    /// Return the inner f64 value.
    #[inline]
    pub fn value(self) -> f64 {
        self.0
    }
}

/// Normalize `-0.0` to `+0.0`, leave everything else unchanged.
#[inline]
fn canonicalize(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v }
}

impl PartialEq for FiniteFloat {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for FiniteFloat {}

impl PartialOrd for FiniteFloat {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FiniteFloat {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl Hash for FiniteFloat {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

impl fmt::Debug for FiniteFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FiniteFloat({:?})", self.0)
    }
}

impl fmt::Display for FiniteFloat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Term
// ---------------------------------------------------------------------------

/// A typed value in the Spindle logic system.
///
/// Terms appear as arguments in literals and as results of arithmetic
/// expressions. The four variants cover the value space needed by the
/// defeasible-logic language.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Term {
    /// An interned symbolic name.
    Symbol(SymbolId),
    /// A 64-bit signed integer.
    Integer(i64),
    /// An arbitrary-precision decimal value.
    Decimal(Decimal),
    /// A canonicalized finite IEEE 754 float.
    Float(FiniteFloat),
}

impl Term {
    /// Returns `true` if this term holds a numeric value (Integer, Decimal, or Float).
    #[inline]
    pub fn is_numeric(&self) -> bool {
        matches!(self, Term::Integer(_) | Term::Decimal(_) | Term::Float(_))
    }

    /// Cross-type numeric equality (REQ-005).
    ///
    /// Two numeric terms are equal when their values are mathematically equal
    /// after widening promotion. Non-numeric terms are never numerically equal.
    ///
    /// Promotion chain: `Integer` → `Decimal` → `Float`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use spindle_core::term::Term;
    /// # use rust_decimal::Decimal;
    /// let a = Term::Integer(2);
    /// let b = Term::Decimal(Decimal::new(20, 1)); // 2.0
    /// assert!(a.numeric_eq(&b));
    /// ```
    pub fn numeric_eq(&self, other: &Term) -> bool {
        match (self.to_numeric_value(), other.to_numeric_value()) {
            (Some(a), Some(b)) => a.numeric_eq(&b),
            _ => false,
        }
    }

    /// Convert this term to a [`NumericValue`], returning `None` for symbols.
    pub fn to_numeric_value(&self) -> Option<NumericValue> {
        match self {
            Term::Symbol(_) => None,
            Term::Integer(n) => Some(NumericValue::Integer(*n)),
            Term::Decimal(d) => Some(NumericValue::Decimal(*d)),
            Term::Float(f) => Some(NumericValue::Float(f.value())),
        }
    }
}

impl fmt::Display for Term {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Term::Symbol(id) => write!(f, "{}", resolve(*id)),
            Term::Integer(n) => write!(f, "{n}"),
            Term::Decimal(d) => write!(f, "{d}"),
            Term::Float(v) => write!(f, "{v}"),
        }
    }
}

// Convenience conversions ──────────────────────────────────────────────────

impl From<SymbolId> for Term {
    #[inline]
    fn from(id: SymbolId) -> Self {
        Term::Symbol(id)
    }
}

impl From<i64> for Term {
    #[inline]
    fn from(n: i64) -> Self {
        Term::Integer(n)
    }
}

impl From<Decimal> for Term {
    #[inline]
    fn from(d: Decimal) -> Self {
        Term::Decimal(d)
    }
}

impl From<FiniteFloat> for Term {
    #[inline]
    fn from(f: FiniteFloat) -> Self {
        Term::Float(f)
    }
}

// ---------------------------------------------------------------------------
// NumericValue — for arithmetic expression results
// ---------------------------------------------------------------------------

/// A numeric value used as the result of arithmetic expression evaluation.
///
/// Unlike [`Term`], this enum has no `Symbol` variant — it is guaranteed to
/// be numeric. It supports the same cross-type comparison semantics as
/// [`Term::numeric_eq`].
#[derive(Clone, Debug)]
pub enum NumericValue {
    /// A 64-bit signed integer.
    Integer(i64),
    /// An arbitrary-precision decimal.
    Decimal(Decimal),
    /// An IEEE 754 double. Must be finite (callers are responsible for
    /// validation before constructing this variant).
    Float(f64),
}

impl NumericValue {
    /// Cross-type numeric equality.
    ///
    /// Promotion: when comparing mixed types the narrower operand is widened.
    ///
    /// - `Integer` vs `Decimal` → promote integer to `Decimal`
    /// - `Integer` vs `Float`   → promote integer to `f64`
    /// - `Decimal` vs `Float`   → promote decimal to `f64`
    pub fn numeric_eq(&self, other: &NumericValue) -> bool {
        match (self, other) {
            // Same type — direct comparison
            (NumericValue::Integer(a), NumericValue::Integer(b)) => a == b,
            (NumericValue::Decimal(a), NumericValue::Decimal(b)) => a == b,
            (NumericValue::Float(a), NumericValue::Float(b)) => a == b,
            // Cross-type promotion
            (NumericValue::Integer(a), NumericValue::Decimal(b))
            | (NumericValue::Decimal(b), NumericValue::Integer(a)) => Decimal::from(*a) == *b,
            (NumericValue::Integer(a), NumericValue::Float(b))
            | (NumericValue::Float(b), NumericValue::Integer(a)) => (*a as f64) == *b,
            (NumericValue::Decimal(a), NumericValue::Float(b))
            | (NumericValue::Float(b), NumericValue::Decimal(a)) => decimal_to_f64(*a) == *b,
        }
    }
}

/// Best-effort conversion from Decimal to f64.
#[inline]
fn decimal_to_f64(d: Decimal) -> f64 {
    use rust_decimal::prelude::ToPrimitive;
    d.to_f64().unwrap_or(f64::NAN)
}

impl fmt::Display for NumericValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NumericValue::Integer(n) => write!(f, "{n}"),
            NumericValue::Decimal(d) => write!(f, "{d}"),
            NumericValue::Float(v) => write!(f, "{v}"),
        }
    }
}

// NumericValue ↔ Term conversions ─────────────────────────────────────────

impl TryFrom<NumericValue> for Term {
    type Error = &'static str;

    fn try_from(nv: NumericValue) -> Result<Self, Self::Error> {
        match nv {
            NumericValue::Integer(n) => Ok(Term::Integer(n)),
            NumericValue::Decimal(d) => Ok(Term::Decimal(d)),
            NumericValue::Float(f) => FiniteFloat::new(f)
                .map(Term::Float)
                .ok_or("non-finite float in numeric conversion"),
        }
    }
}

impl PartialEq for NumericValue {
    fn eq(&self, other: &Self) -> bool {
        self.numeric_eq(other)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intern::intern;

    // -- FiniteFloat tests -------------------------------------------------

    #[test]
    fn finite_float_rejects_nan() {
        assert!(FiniteFloat::new(f64::NAN).is_none());
    }

    #[test]
    fn finite_float_rejects_infinity() {
        assert!(FiniteFloat::new(f64::INFINITY).is_none());
        assert!(FiniteFloat::new(f64::NEG_INFINITY).is_none());
    }

    #[test]
    fn finite_float_normalizes_neg_zero() {
        let a = FiniteFloat::new(0.0).unwrap();
        let b = FiniteFloat::new(-0.0).unwrap();
        assert_eq!(a, b);
        assert!(b.value().is_sign_positive());
    }

    #[test]
    fn finite_float_eq_and_hash() {
        use std::collections::HashSet;

        let a = FiniteFloat::new(1.5).unwrap();
        let b = FiniteFloat::new(1.5).unwrap();
        assert_eq!(a, b);

        let mut set = HashSet::new();
        set.insert(a);
        assert!(set.contains(&b));
    }

    #[test]
    fn finite_float_ordering() {
        let a = FiniteFloat::new(-1.0).unwrap();
        let b = FiniteFloat::new(0.0).unwrap();
        let c = FiniteFloat::new(1.0).unwrap();
        assert!(a < b);
        assert!(b < c);
    }

    #[test]
    fn finite_float_display() {
        let f = FiniteFloat::new(3.14).unwrap();
        assert_eq!(format!("{f}"), "3.14");
    }

    // -- Term tests --------------------------------------------------------

    #[test]
    fn term_is_numeric() {
        assert!(!Term::Symbol(intern("x")).is_numeric());
        assert!(Term::Integer(42).is_numeric());
        assert!(Term::Decimal(Decimal::new(314, 2)).is_numeric());
        assert!(Term::Float(FiniteFloat::new(1.0).unwrap()).is_numeric());
    }

    #[test]
    fn term_display() {
        let sym = Term::Symbol(intern("hello"));
        assert_eq!(format!("{sym}"), "hello");

        let int = Term::Integer(-7);
        assert_eq!(format!("{int}"), "-7");

        let dec = Term::Decimal(Decimal::new(150, 2));
        assert_eq!(format!("{dec}"), "1.50");

        let flt = Term::Float(FiniteFloat::new(2.5).unwrap());
        assert_eq!(format!("{flt}"), "2.5");
    }

    #[test]
    fn term_clone_and_eq() {
        let t = Term::Integer(99);
        let t2 = t.clone();
        assert_eq!(t, t2);
    }

    #[test]
    fn term_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(Term::Integer(1));
        set.insert(Term::Integer(2));
        set.insert(Term::Integer(1)); // duplicate

        assert_eq!(set.len(), 2);
    }

    // -- Cross-type numeric equality (REQ-005) -----------------------------

    #[test]
    fn numeric_eq_same_type() {
        assert!(Term::Integer(5).numeric_eq(&Term::Integer(5)));
        assert!(!Term::Integer(5).numeric_eq(&Term::Integer(6)));

        let d = Decimal::new(250, 2);
        assert!(Term::Decimal(d).numeric_eq(&Term::Decimal(d)));
    }

    #[test]
    fn numeric_eq_integer_decimal() {
        let int = Term::Integer(3);
        let dec = Term::Decimal(Decimal::new(30, 1)); // 3.0
        assert!(int.numeric_eq(&dec));
        assert!(dec.numeric_eq(&int));
    }

    #[test]
    fn numeric_eq_integer_float() {
        let int = Term::Integer(7);
        let flt = Term::Float(FiniteFloat::new(7.0).unwrap());
        assert!(int.numeric_eq(&flt));
        assert!(flt.numeric_eq(&int));
    }

    #[test]
    fn numeric_eq_decimal_float() {
        let dec = Term::Decimal(Decimal::new(25, 1)); // 2.5
        let flt = Term::Float(FiniteFloat::new(2.5).unwrap());
        assert!(dec.numeric_eq(&flt));
        assert!(flt.numeric_eq(&dec));
    }

    #[test]
    fn numeric_eq_symbol_never_equal() {
        let sym = Term::Symbol(intern("x"));
        let int = Term::Integer(0);
        assert!(!sym.numeric_eq(&int));
        assert!(!sym.numeric_eq(&sym));
    }

    // -- NumericValue tests ------------------------------------------------

    #[test]
    fn numeric_value_display() {
        assert_eq!(format!("{}", NumericValue::Integer(42)), "42");
        assert_eq!(
            format!("{}", NumericValue::Decimal(Decimal::new(99, 1))),
            "9.9"
        );
        assert_eq!(format!("{}", NumericValue::Float(1.23)), "1.23");
    }

    #[test]
    fn numeric_value_eq_cross_type() {
        let a = NumericValue::Integer(10);
        let b = NumericValue::Decimal(Decimal::new(100, 1)); // 10.0
        assert_eq!(a, b);
    }

    // -- NumericValue ↔ Term conversion ------------------------------------

    #[test]
    fn term_to_numeric_value() {
        assert!(Term::Symbol(intern("x")).to_numeric_value().is_none());
        assert_eq!(
            Term::Integer(5).to_numeric_value().unwrap(),
            NumericValue::Integer(5)
        );
    }

    #[test]
    fn numeric_value_to_term() {
        let nv = NumericValue::Integer(42);
        let t: Term = nv.try_into().unwrap();
        assert_eq!(t, Term::Integer(42));

        let nv = NumericValue::Decimal(Decimal::new(10, 0));
        let t: Term = nv.try_into().unwrap();
        assert_eq!(t, Term::Decimal(Decimal::new(10, 0)));

        let nv = NumericValue::Float(3.14);
        let t: Term = nv.try_into().unwrap();
        assert!(matches!(t, Term::Float(_)));

        // Non-finite floats must be rejected
        assert!(Term::try_from(NumericValue::Float(f64::NAN)).is_err());
        assert!(Term::try_from(NumericValue::Float(f64::INFINITY)).is_err());
        assert!(Term::try_from(NumericValue::Float(f64::NEG_INFINITY)).is_err());
    }

    // -- From impls --------------------------------------------------------

    #[test]
    fn from_impls() {
        let _: Term = SymbolId::EMPTY.into();
        let _: Term = 42i64.into();
        let _: Term = Decimal::ONE.into();
        let _: Term = FiniteFloat::new(1.0).unwrap().into();
    }

    // =====================================================================
    // TEST-001: Numeric literal terms (10 scenarios)
    // =====================================================================

    #[test]
    fn test_001_01_integer_literal() {
        let t = Term::Integer(42);
        assert!(t.is_numeric());
        assert_eq!(t, Term::Integer(42));
        assert_eq!(format!("{t}"), "42");
    }

    #[test]
    fn test_001_02_negative_integer() {
        let t = Term::Integer(-100);
        assert!(t.is_numeric());
        assert_eq!(t, Term::Integer(-100));
        assert_eq!(format!("{t}"), "-100");
    }

    #[test]
    fn test_001_03_decimal_literal_exact() {
        // 0.15 as Decimal is exact — not a float approximation.
        let d = Decimal::new(15, 2); // 0.15
        let t = Term::Decimal(d);
        assert!(t.is_numeric());
        assert_eq!(format!("{t}"), "0.15");
        // Prove exactness: 0.15 × 100 = exactly 15
        assert_eq!(d * Decimal::from(100), Decimal::from(15));
    }

    #[test]
    fn test_001_04_decimal_leading_zero_exact() {
        // 0.08 must be exact Decimal, not 0.07999... as float.
        let d = Decimal::new(8, 2); // 0.08
        let t = Term::Decimal(d);
        assert_eq!(format!("{t}"), "0.08");
        // Prove exactness: 0.08 × 100 = exactly 8
        assert_eq!(d * Decimal::from(100), Decimal::from(8));
    }

    #[test]
    fn test_001_05_float_scientific_notation() {
        // 1.5e3 = 1500.0 stored as Float.
        let ff = FiniteFloat::new(1.5e3).unwrap();
        let t = Term::Float(ff);
        assert!(t.is_numeric());
        assert_eq!(ff.value(), 1500.0);
    }

    #[test]
    fn test_001_06_float_without_decimal_point() {
        // 1e6 = 1000000.0 stored as Float.
        let ff = FiniteFloat::new(1e6).unwrap();
        let t = Term::Float(ff);
        assert!(t.is_numeric());
        assert_eq!(ff.value(), 1_000_000.0);
    }

    #[test]
    fn test_001_07_symbol_and_integer_coexist() {
        use std::collections::HashSet;
        let sym = Term::Symbol(intern("item-a"));
        let int = Term::Integer(1);
        assert!(!sym.is_numeric());
        assert!(int.is_numeric());
        assert_ne!(sym, int);
        // Both variants work correctly in a hash set.
        let mut set = HashSet::new();
        set.insert(sym);
        set.insert(int);
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_001_08_integer_overflow_boundaries() {
        // i64::MAX and i64::MIN are representable as Term::Integer.
        let max_term = Term::Integer(i64::MAX);
        let min_term = Term::Integer(i64::MIN);
        assert_eq!(max_term, Term::Integer(9_223_372_036_854_775_807));
        assert_eq!(min_term, Term::Integer(-9_223_372_036_854_775_808));
        assert_ne!(max_term, min_term);
    }

    #[test]
    fn test_001_09_decimal_precision_28_digits() {
        // rust_decimal supports up to 28–29 significant digits.
        use std::str::FromStr;
        let s = "1234567890123456789012345678";
        let d = Decimal::from_str(s).unwrap();
        let t = Term::Decimal(d);
        assert!(t.is_numeric());
        assert_eq!(d.to_string(), s);
    }

    #[test]
    fn test_001_10_decimal_division_28_frac_digits() {
        // (/ 10 3) → Decimal(3.3333333333333333333333333333) — 28 '3's.
        let result = Decimal::from(10) / Decimal::from(3);
        let t = Term::Decimal(result);
        assert!(t.is_numeric());
        let s = result.to_string();
        assert!(s.starts_with("3."), "expected '3.' prefix, got: {s}");
        let frac = &s[2..];
        assert_eq!(
            frac.len(),
            28,
            "expected 28 fractional digits, got {}",
            frac.len()
        );
        assert!(
            frac.chars().all(|c| c == '3'),
            "expected all '3's, got: {frac}"
        );
    }
}
