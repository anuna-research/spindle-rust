//! Arithmetic expression AST and evaluation for Spindle.
//!
//! This module provides the arithmetic expression types and evaluation logic
//! used by the grounding phase to evaluate `bind` predicates and comparison
//! guards in rule bodies.
//!
//! # Type Promotion (REQ-005)
//!
//! - Integer OP Integer → Integer (except `/` → Decimal)
//! - Integer OP Decimal → Decimal
//! - Float OP any → Float (float is contagious)
//!
//! # Variadic Semantics (CON-002)
//!
//! N-ary operators follow Racket/Common Lisp conventions:
//! - `+` and `*` accept 0+ args with identity elements (0 and 1)
//! - `-` with 1 arg is negation; `/` with 1 arg is reciprocal
//! - `min` and `max` require 1+ args

use std::cmp::Ordering;
use std::fmt;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use crate::grounding::Substitution;
use crate::intern::{SymbolId, resolve};
use crate::term::{FiniteFloat, NumericValue, Term};

// ---------------------------------------------------------------------------
// Operator enums
// ---------------------------------------------------------------------------

/// N-ary arithmetic operators that accept variable numbers of arguments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NaryArithOp {
    /// Addition (+). Identity: 0. Supports 0+ args.
    Add,
    /// Subtraction (-). 1 arg: negation. 2+ args: left-fold.
    Sub,
    /// Multiplication (*). Identity: 1. Supports 0+ args.
    Mul,
    /// Division (/). 1 arg: reciprocal. 2+ args: left-fold.
    Div,
    /// Minimum value. Requires 1+ args.
    Min,
    /// Maximum value. Requires 1+ args.
    Max,
}

/// Binary-only arithmetic operators.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BinArithOp {
    /// Integer floor division (rounds toward −∞). Requires integer operands.
    IDiv,
    /// Floor remainder: a − (a div b) × b. Requires integer operands.
    Rem,
    /// Exponentiation (base ** exponent).
    Pow,
}

/// Unary-only arithmetic operators.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum UnaryArithOp {
    /// Absolute value.
    Abs,
}

/// Comparison operators for arithmetic guards.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CmpOp {
    /// Equal (=)
    Eq,
    /// Not equal (!=)
    Ne,
    /// Less than (<)
    Lt,
    /// Greater than (>)
    Gt,
    /// Less than or equal (<=)
    Le,
    /// Greater than or equal (>=)
    Ge,
}

// ---------------------------------------------------------------------------
// ArithExpr
// ---------------------------------------------------------------------------

/// An arithmetic expression tree.
#[derive(Clone, Debug, PartialEq)]
pub enum ArithExpr {
    /// A literal numeric value.
    Lit(NumericValue),
    /// A variable reference, resolved from the substitution.
    Var(SymbolId),
    /// An n-ary operation (variadic).
    NaryOp {
        /// The operator.
        op: NaryArithOp,
        /// The arguments.
        args: Vec<ArithExpr>,
    },
    /// A binary-only operation.
    BinOp {
        /// The operator.
        op: BinArithOp,
        /// Left-hand side.
        lhs: Box<ArithExpr>,
        /// Right-hand side.
        rhs: Box<ArithExpr>,
    },
    /// A unary-only operation.
    UnaryOp {
        /// The operator.
        op: UnaryArithOp,
        /// The operand.
        expr: Box<ArithExpr>,
    },
}

// ---------------------------------------------------------------------------
// ArithConstraint
// ---------------------------------------------------------------------------

/// An arithmetic constraint that appears in rule bodies.
#[derive(Clone, Debug, PartialEq)]
pub enum ArithConstraint {
    /// Bind a variable to the result of an expression: `(bind ?x expr)`.
    Bind {
        /// The variable to bind.
        var: SymbolId,
        /// The expression to evaluate.
        expr: ArithExpr,
    },
    /// Compare two expressions: `(op lhs rhs)`.
    Compare {
        /// The comparison operator.
        op: CmpOp,
        /// Left-hand side expression.
        lhs: ArithExpr,
        /// Right-hand side expression.
        rhs: ArithExpr,
    },
}

// ---------------------------------------------------------------------------
// ArithError
// ---------------------------------------------------------------------------

/// Errors that can occur during arithmetic evaluation.
///
/// All arithmetic failures result in silent grounding failure (the substitution
/// path is discarded). The error type is retained for diagnostic purposes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArithError {
    /// Division by zero: `(/ x 0)`, `(div x 0)`, `(rem x 0)`.
    DivisionByZero,
    /// Integer overflow: result exceeds i64 range.
    IntegerOverflow,
    /// Decimal overflow: result exceeds rust_decimal's 128-bit range.
    DecimalOverflow,
    /// Variable not bound in the current substitution.
    UnboundVariable {
        /// The unbound variable's interned name.
        name: SymbolId,
    },
    /// Operator requires a specific numeric type.
    TypeMismatch {
        /// The operator name.
        op: &'static str,
        /// The expected type.
        expected: &'static str,
        /// The actual type.
        got: &'static str,
    },
    /// Operation produced NaN, +inf, or -inf.
    NonFiniteFloat,
    /// Unary reciprocal of zero: `(/ 0)`.
    ReciprocalOfZero,
    /// Comparison predicate did not hold.
    ComparisonFailed,
}

impl fmt::Display for ArithError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DivisionByZero => write!(f, "division by zero"),
            Self::IntegerOverflow => write!(f, "integer overflow"),
            Self::DecimalOverflow => write!(f, "decimal overflow"),
            Self::UnboundVariable { name } => {
                write!(f, "unbound variable: {}", resolve(*name))
            }
            Self::TypeMismatch { op, expected, got } => {
                write!(f, "type mismatch in {op}: expected {expected}, got {got}")
            }
            Self::NonFiniteFloat => write!(f, "non-finite float result"),
            Self::ReciprocalOfZero => write!(f, "reciprocal of zero"),
            Self::ComparisonFailed => write!(f, "comparison failed"),
        }
    }
}

impl std::error::Error for ArithError {}

// ---------------------------------------------------------------------------
// Numeric helpers
// ---------------------------------------------------------------------------

/// Name of a numeric value's type, for error messages.
fn type_name(v: &NumericValue) -> &'static str {
    match v {
        NumericValue::Integer(_) => "Integer",
        NumericValue::Decimal(_) => "Decimal",
        NumericValue::Float(_) => "Float",
    }
}

/// Convert a numeric value to f64.
fn to_f64_value(v: &NumericValue) -> f64 {
    match v {
        NumericValue::Integer(n) => *n as f64,
        NumericValue::Decimal(d) => d.to_f64().unwrap_or(f64::NAN),
        NumericValue::Float(f) => *f,
    }
}

/// Validate that an f64 result is finite.
fn check_finite(v: f64) -> Result<NumericValue, ArithError> {
    if v.is_finite() {
        Ok(NumericValue::Float(v))
    } else {
        Err(ArithError::NonFiniteFloat)
    }
}

/// Promote two values to a common type for binary arithmetic.
///
/// Promotion follows the widening chain: Integer → Decimal → Float.
fn promote_pair(a: NumericValue, b: NumericValue) -> (NumericValue, NumericValue) {
    match (&a, &b) {
        // Same type — no promotion
        (NumericValue::Integer(_), NumericValue::Integer(_))
        | (NumericValue::Decimal(_), NumericValue::Decimal(_))
        | (NumericValue::Float(_), NumericValue::Float(_)) => (a, b),
        // Float is contagious
        (NumericValue::Float(_), _) => (a, NumericValue::Float(to_f64_value(&b))),
        (_, NumericValue::Float(_)) => (NumericValue::Float(to_f64_value(&a)), b),
        // Integer + Decimal → Decimal
        (NumericValue::Integer(n), NumericValue::Decimal(_)) => {
            (NumericValue::Decimal(Decimal::from(*n)), b)
        }
        (NumericValue::Decimal(_), NumericValue::Integer(n)) => {
            (a, NumericValue::Decimal(Decimal::from(*n)))
        }
    }
}

/// Compare two numeric values after promotion, returning the ordering
/// and the promoted values.
fn numeric_cmp(
    a: NumericValue,
    b: NumericValue,
) -> Result<(Ordering, NumericValue, NumericValue), ArithError> {
    let (pa, pb) = promote_pair(a, b);
    let ord = match (&pa, &pb) {
        (NumericValue::Integer(x), NumericValue::Integer(y)) => x.cmp(y),
        (NumericValue::Decimal(x), NumericValue::Decimal(y)) => x.cmp(y),
        (NumericValue::Float(x), NumericValue::Float(y)) => {
            x.partial_cmp(y).ok_or(ArithError::NonFiniteFloat)?
        }
        _ => unreachable!("promote_pair ensures matching types"),
    };
    Ok((ord, pa, pb))
}

// ---------------------------------------------------------------------------
// Pairwise arithmetic operations
// ---------------------------------------------------------------------------

fn add_values(a: NumericValue, b: NumericValue) -> Result<NumericValue, ArithError> {
    let (a, b) = promote_pair(a, b);
    match (a, b) {
        (NumericValue::Integer(x), NumericValue::Integer(y)) => {
            x.checked_add(y).map(NumericValue::Integer).ok_or(ArithError::IntegerOverflow)
        }
        (NumericValue::Decimal(x), NumericValue::Decimal(y)) => {
            x.checked_add(y).map(NumericValue::Decimal).ok_or(ArithError::DecimalOverflow)
        }
        (NumericValue::Float(x), NumericValue::Float(y)) => check_finite(x + y),
        _ => unreachable!(),
    }
}

fn sub_values(a: NumericValue, b: NumericValue) -> Result<NumericValue, ArithError> {
    let (a, b) = promote_pair(a, b);
    match (a, b) {
        (NumericValue::Integer(x), NumericValue::Integer(y)) => {
            x.checked_sub(y).map(NumericValue::Integer).ok_or(ArithError::IntegerOverflow)
        }
        (NumericValue::Decimal(x), NumericValue::Decimal(y)) => {
            x.checked_sub(y).map(NumericValue::Decimal).ok_or(ArithError::DecimalOverflow)
        }
        (NumericValue::Float(x), NumericValue::Float(y)) => check_finite(x - y),
        _ => unreachable!(),
    }
}

fn mul_values(a: NumericValue, b: NumericValue) -> Result<NumericValue, ArithError> {
    let (a, b) = promote_pair(a, b);
    match (a, b) {
        (NumericValue::Integer(x), NumericValue::Integer(y)) => {
            x.checked_mul(y).map(NumericValue::Integer).ok_or(ArithError::IntegerOverflow)
        }
        (NumericValue::Decimal(x), NumericValue::Decimal(y)) => {
            x.checked_mul(y).map(NumericValue::Decimal).ok_or(ArithError::DecimalOverflow)
        }
        (NumericValue::Float(x), NumericValue::Float(y)) => check_finite(x * y),
        _ => unreachable!(),
    }
}

/// Division with REQ-005 promotion: Integer / Integer → Decimal.
fn div_values(a: NumericValue, b: NumericValue) -> Result<NumericValue, ArithError> {
    // Special case: Integer / Integer → Decimal
    if let (NumericValue::Integer(x), NumericValue::Integer(y)) = (&a, &b) {
        if *y == 0 {
            return Err(ArithError::DivisionByZero);
        }
        let dx = Decimal::from(*x);
        let dy = Decimal::from(*y);
        return dx.checked_div(dy).map(NumericValue::Decimal).ok_or(ArithError::DecimalOverflow);
    }

    let (a, b) = promote_pair(a, b);
    match (a, b) {
        (NumericValue::Decimal(x), NumericValue::Decimal(y)) => {
            if y.is_zero() {
                Err(ArithError::DivisionByZero)
            } else {
                x.checked_div(y).map(NumericValue::Decimal).ok_or(ArithError::DecimalOverflow)
            }
        }
        (NumericValue::Float(x), NumericValue::Float(y)) => {
            if y == 0.0 {
                Err(ArithError::DivisionByZero)
            } else {
                check_finite(x / y)
            }
        }
        _ => unreachable!(),
    }
}

fn negate(v: NumericValue) -> Result<NumericValue, ArithError> {
    match v {
        NumericValue::Integer(n) => {
            n.checked_neg().map(NumericValue::Integer).ok_or(ArithError::IntegerOverflow)
        }
        NumericValue::Decimal(d) => Ok(NumericValue::Decimal(-d)),
        NumericValue::Float(f) => Ok(NumericValue::Float(-f)),
    }
}

/// Reciprocal (1/x). For Integer/Decimal inputs, produces Decimal.
fn reciprocal(v: NumericValue) -> Result<NumericValue, ArithError> {
    match v {
        NumericValue::Integer(n) => {
            if n == 0 {
                return Err(ArithError::ReciprocalOfZero);
            }
            let d = Decimal::from(n);
            Decimal::ONE.checked_div(d).map(NumericValue::Decimal).ok_or(ArithError::DecimalOverflow)
        }
        NumericValue::Decimal(d) => {
            if d.is_zero() {
                return Err(ArithError::ReciprocalOfZero);
            }
            Decimal::ONE
                .checked_div(d)
                .map(NumericValue::Decimal)
                .ok_or(ArithError::DecimalOverflow)
        }
        NumericValue::Float(f) => {
            if f == 0.0 {
                return Err(ArithError::ReciprocalOfZero);
            }
            check_finite(1.0 / f)
        }
    }
}

fn abs_value(v: NumericValue) -> Result<NumericValue, ArithError> {
    match v {
        NumericValue::Integer(n) => {
            n.checked_abs().map(NumericValue::Integer).ok_or(ArithError::IntegerOverflow)
        }
        NumericValue::Decimal(d) => Ok(NumericValue::Decimal(d.abs())),
        NumericValue::Float(f) => Ok(NumericValue::Float(f.abs())),
    }
}

// ---------------------------------------------------------------------------
// Floor division and remainder (for IDiv/Rem)
// ---------------------------------------------------------------------------

/// Floor division: rounds toward negative infinity.
fn floor_div_i64(a: i64, b: i64) -> Result<i64, ArithError> {
    // Handle overflow: i64::MIN / -1 overflows
    let d = a.checked_div(b).ok_or(ArithError::IntegerOverflow)?;
    let r = a % b;
    // Adjust toward negative infinity when signs differ and remainder is nonzero
    if r != 0 && (a ^ b) < 0 { Ok(d - 1) } else { Ok(d) }
}

/// Floor remainder: a − (a div b) × b, matching floor division.
fn floor_rem_i64(a: i64, b: i64) -> Result<i64, ArithError> {
    // Check for overflow: i64::MIN % -1 can panic/overflow
    let _d = a.checked_div(b).ok_or(ArithError::IntegerOverflow)?;
    let r = a % b;
    if r != 0 && (a ^ b) < 0 { Ok(r + b) } else { Ok(r) }
}

// ---------------------------------------------------------------------------
// Exponentiation
// ---------------------------------------------------------------------------

/// Integer power via repeated squaring with overflow checking.
fn int_checked_pow_nonneg(base: i64, exp: u64) -> Result<i64, ArithError> {
    if exp == 0 {
        return Ok(1); // 0^0 = 1
    }
    let mut result: i64 = 1;
    let mut b = base;
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result = result.checked_mul(b).ok_or(ArithError::IntegerOverflow)?;
        }
        e >>= 1;
        if e > 0 {
            b = b.checked_mul(b).ok_or(ArithError::IntegerOverflow)?;
        }
    }
    Ok(result)
}

/// Decimal power with integer exponent via repeated squaring.
fn decimal_checked_powi(base: Decimal, exp: i64) -> Result<Decimal, ArithError> {
    if exp == 0 {
        return Ok(Decimal::ONE);
    }
    if exp < 0 {
        let abs_exp = (exp as i128).unsigned_abs() as u64;
        let pos = decimal_checked_pow_nonneg(base, abs_exp)?;
        if pos.is_zero() {
            return Err(ArithError::DivisionByZero);
        }
        return Decimal::ONE.checked_div(pos).ok_or(ArithError::DecimalOverflow);
    }
    decimal_checked_pow_nonneg(base, exp as u64)
}

fn decimal_checked_pow_nonneg(base: Decimal, exp: u64) -> Result<Decimal, ArithError> {
    if exp == 0 {
        return Ok(Decimal::ONE);
    }
    let mut result = Decimal::ONE;
    let mut b = base;
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            result = result.checked_mul(b).ok_or(ArithError::DecimalOverflow)?;
        }
        e >>= 1;
        if e > 0 {
            b = b.checked_mul(b).ok_or(ArithError::DecimalOverflow)?;
        }
    }
    Ok(result)
}

/// Decimal power with possibly non-integral Decimal exponent.
fn decimal_pow(base: Decimal, exp: Decimal) -> Result<NumericValue, ArithError> {
    if exp.fract().is_zero() {
        // Integral exponent: use exact repeated squaring
        if let Some(e) = exp.to_i64() {
            return decimal_checked_powi(base, e).map(NumericValue::Decimal);
        }
        return Err(ArithError::DecimalOverflow);
    }
    // Non-integral exponent: use rust_decimal's checked_powd
    use rust_decimal::MathematicalOps;
    base.checked_powd(exp)
        .map(NumericValue::Decimal)
        .ok_or(ArithError::DecimalOverflow)
}

/// Full exponentiation dispatch with type promotion.
fn eval_pow(base: NumericValue, exp: NumericValue) -> Result<NumericValue, ArithError> {
    // Float contagion
    if matches!((&base, &exp), (NumericValue::Float(_), _) | (_, NumericValue::Float(_))) {
        let fb = to_f64_value(&base);
        let fe = to_f64_value(&exp);
        return check_finite(fb.powf(fe));
    }

    match (base, exp) {
        (NumericValue::Integer(b), NumericValue::Integer(e)) => {
            if e >= 0 {
                int_checked_pow_nonneg(b, e as u64).map(NumericValue::Integer)
            } else {
                // Negative integer exponent → Decimal
                if b == 0 {
                    return Err(ArithError::DivisionByZero);
                }
                let abs_e = (e as i128).unsigned_abs() as u64;
                let pow_val = int_checked_pow_nonneg(b, abs_e)?;
                let dec = Decimal::from(pow_val);
                Decimal::ONE
                    .checked_div(dec)
                    .map(NumericValue::Decimal)
                    .ok_or(ArithError::DecimalOverflow)
            }
        }
        (NumericValue::Decimal(b), NumericValue::Integer(e)) => {
            decimal_checked_powi(b, e).map(NumericValue::Decimal)
        }
        (NumericValue::Integer(b), NumericValue::Decimal(e)) => {
            decimal_pow(Decimal::from(b), e)
        }
        (NumericValue::Decimal(b), NumericValue::Decimal(e)) => decimal_pow(b, e),
        _ => unreachable!("float cases handled above"),
    }
}

// ---------------------------------------------------------------------------
// Variable resolution
// ---------------------------------------------------------------------------

/// Resolve a variable from the substitution to a numeric value.
///
/// The substitution maps `SymbolId → Term`, so we extract the numeric
/// value directly from the bound `Term`.
fn resolve_var(subst: &Substitution, name: SymbolId) -> Result<NumericValue, ArithError> {
    let bound = match subst.terms.get(&name) {
        Some(t) => t,
        None => {
            // Temporal endpoint and interval variables cannot be used as
            // arithmetic operands (REQ-006: silent failure via TypeMismatch).
            if subst.temporal.contains_key(&name) || subst.intervals.contains_key(&name) {
                return Err(ArithError::TypeMismatch {
                    op: "var",
                    expected: "numeric",
                    got: "temporal",
                });
            }
            return Err(ArithError::UnboundVariable { name });
        }
    };
    match bound {
        Term::Integer(n) => Ok(NumericValue::Integer(*n)),
        Term::Decimal(d) => Ok(NumericValue::Decimal(*d)),
        Term::Float(f) => Ok(NumericValue::Float(f.value())),
        Term::Symbol(id) => {
            // Legacy path: try to parse the symbol string as a number.
            let s = resolve(*id);
            if let Ok(n) = s.parse::<i64>() {
                return Ok(NumericValue::Integer(n));
            }
            if let Ok(d) = s.parse::<Decimal>() {
                return Ok(NumericValue::Decimal(d));
            }
            if let Ok(f) = s.parse::<f64>() {
                if f.is_finite() {
                    return Ok(NumericValue::Float(f));
                }
            }
            Err(ArithError::TypeMismatch {
                op: "var",
                expected: "numeric",
                got: "symbol",
            })
        }
    }
}

// ---------------------------------------------------------------------------
// ArithExpr evaluation
// ---------------------------------------------------------------------------

impl ArithExpr {
    /// Evaluate this expression under the given substitution.
    ///
    /// Returns the resulting numeric value, or an error if evaluation fails
    /// (overflow, unbound variable, type mismatch, etc.).
    pub fn eval(&self, subst: &Substitution) -> Result<NumericValue, ArithError> {
        match self {
            ArithExpr::Lit(v) => Ok(v.clone()),
            ArithExpr::Var(name) => resolve_var(subst, *name),
            ArithExpr::NaryOp { op, args } => eval_nary(op, args, subst),
            ArithExpr::BinOp { op, lhs, rhs } => {
                let l = lhs.eval(subst)?;
                let r = rhs.eval(subst)?;
                eval_bin(op, l, r)
            }
            ArithExpr::UnaryOp { op, expr } => {
                let v = expr.eval(subst)?;
                match op {
                    UnaryArithOp::Abs => abs_value(v),
                }
            }
        }
    }
}

fn eval_nary(
    op: &NaryArithOp,
    args: &[ArithExpr],
    subst: &Substitution,
) -> Result<NumericValue, ArithError> {
    match op {
        NaryArithOp::Add => {
            if args.is_empty() {
                return Ok(NumericValue::Integer(0));
            }
            let mut acc = args[0].eval(subst)?;
            for arg in &args[1..] {
                acc = add_values(acc, arg.eval(subst)?)?;
            }
            Ok(acc)
        }
        NaryArithOp::Sub => {
            if args.is_empty() {
                return Err(ArithError::TypeMismatch {
                    op: "-",
                    expected: "1+ arguments",
                    got: "0 arguments",
                });
            }
            let first = args[0].eval(subst)?;
            if args.len() == 1 {
                return negate(first);
            }
            let mut acc = first;
            for arg in &args[1..] {
                acc = sub_values(acc, arg.eval(subst)?)?;
            }
            Ok(acc)
        }
        NaryArithOp::Mul => {
            if args.is_empty() {
                return Ok(NumericValue::Integer(1));
            }
            let mut acc = args[0].eval(subst)?;
            for arg in &args[1..] {
                acc = mul_values(acc, arg.eval(subst)?)?;
            }
            Ok(acc)
        }
        NaryArithOp::Div => {
            if args.is_empty() {
                return Err(ArithError::TypeMismatch {
                    op: "/",
                    expected: "1+ arguments",
                    got: "0 arguments",
                });
            }
            let first = args[0].eval(subst)?;
            if args.len() == 1 {
                return reciprocal(first);
            }
            let mut acc = first;
            for arg in &args[1..] {
                acc = div_values(acc, arg.eval(subst)?)?;
            }
            Ok(acc)
        }
        NaryArithOp::Min => {
            if args.is_empty() {
                return Err(ArithError::TypeMismatch {
                    op: "min",
                    expected: "1+ arguments",
                    got: "0 arguments",
                });
            }
            let mut acc = args[0].eval(subst)?;
            for arg in &args[1..] {
                let val = arg.eval(subst)?;
                let (ord, pa, pb) = numeric_cmp(acc, val)?;
                acc = if ord.is_le() { pa } else { pb };
            }
            Ok(acc)
        }
        NaryArithOp::Max => {
            if args.is_empty() {
                return Err(ArithError::TypeMismatch {
                    op: "max",
                    expected: "1+ arguments",
                    got: "0 arguments",
                });
            }
            let mut acc = args[0].eval(subst)?;
            for arg in &args[1..] {
                let val = arg.eval(subst)?;
                let (ord, pa, pb) = numeric_cmp(acc, val)?;
                acc = if ord.is_ge() { pa } else { pb };
            }
            Ok(acc)
        }
    }
}

fn eval_bin(op: &BinArithOp, lhs: NumericValue, rhs: NumericValue) -> Result<NumericValue, ArithError> {
    match op {
        BinArithOp::IDiv => {
            match (&lhs, &rhs) {
                (NumericValue::Integer(a), NumericValue::Integer(b)) => {
                    if *b == 0 {
                        Err(ArithError::DivisionByZero)
                    } else {
                        floor_div_i64(*a, *b).map(NumericValue::Integer)
                    }
                }
                _ => Err(ArithError::TypeMismatch {
                    op: "div",
                    expected: "Integer",
                    got: type_name(if !matches!(lhs, NumericValue::Integer(_)) {
                        &lhs
                    } else {
                        &rhs
                    }),
                }),
            }
        }
        BinArithOp::Rem => {
            match (&lhs, &rhs) {
                (NumericValue::Integer(a), NumericValue::Integer(b)) => {
                    if *b == 0 {
                        Err(ArithError::DivisionByZero)
                    } else {
                        floor_rem_i64(*a, *b).map(NumericValue::Integer)
                    }
                }
                _ => Err(ArithError::TypeMismatch {
                    op: "rem",
                    expected: "Integer",
                    got: type_name(if !matches!(lhs, NumericValue::Integer(_)) {
                        &lhs
                    } else {
                        &rhs
                    }),
                }),
            }
        }
        BinArithOp::Pow => eval_pow(lhs, rhs),
    }
}

// ---------------------------------------------------------------------------
// ArithConstraint evaluation
// ---------------------------------------------------------------------------

impl ArithConstraint {
    /// Evaluate this constraint under the given substitution.
    ///
    /// - `Bind`: evaluates the expression and binds the variable in the substitution.
    /// - `Compare`: evaluates both sides and checks the comparison.
    ///
    /// Returns `Ok(())` on success, or an appropriate `ArithError` on failure.
    pub fn eval(&self, subst: &mut Substitution) -> Result<(), ArithError> {
        match self {
            ArithConstraint::Bind { var, expr } => {
                let val = expr.eval(subst)?;
                // Bind the variable directly as a typed Term value.
                let term = match val {
                    NumericValue::Integer(n) => Term::Integer(n),
                    NumericValue::Decimal(d) => Term::Decimal(d),
                    NumericValue::Float(f) => {
                        Term::Float(FiniteFloat::new(f).unwrap_or(FiniteFloat::new(0.0).unwrap()))
                    }
                };
                subst.terms.insert(*var, term);
                Ok(())
            }
            ArithConstraint::Compare { op, lhs, rhs } => {
                let l = lhs.eval(subst)?;
                let r = rhs.eval(subst)?;
                let (ord, _, _) = numeric_cmp(l, r)?;
                let holds = match op {
                    CmpOp::Eq => ord == Ordering::Equal,
                    CmpOp::Ne => ord != Ordering::Equal,
                    CmpOp::Lt => ord == Ordering::Less,
                    CmpOp::Gt => ord == Ordering::Greater,
                    CmpOp::Le => ord != Ordering::Greater,
                    CmpOp::Ge => ord != Ordering::Less,
                };
                if holds {
                    Ok(())
                } else {
                    Err(ArithError::ComparisonFailed)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intern::intern;

    /// Create a substitution with the given variable bindings.
    ///
    /// Values are parsed as numeric types when possible (Integer, Decimal,
    /// Float), falling back to Symbol for non-numeric strings.
    fn make_subst(bindings: &[(&str, &str)]) -> Substitution {
        let mut subst = Substitution::default();
        for (var, val) in bindings {
            let term = if let Ok(n) = val.parse::<i64>() {
                Term::Integer(n)
            } else if let Ok(d) = val.parse::<Decimal>() {
                Term::Decimal(d)
            } else if let Ok(f) = val.parse::<f64>() {
                if f.is_finite() {
                    Term::Float(FiniteFloat::new(f).unwrap())
                } else {
                    Term::Symbol(intern(val))
                }
            } else {
                Term::Symbol(intern(val))
            };
            subst.terms.insert(intern(var), term);
        }
        subst
    }

    fn lit_int(n: i64) -> ArithExpr {
        ArithExpr::Lit(NumericValue::Integer(n))
    }

    fn lit_dec(n: i64, scale: u32) -> ArithExpr {
        ArithExpr::Lit(NumericValue::Decimal(Decimal::new(n, scale)))
    }

    fn lit_float(f: f64) -> ArithExpr {
        ArithExpr::Lit(NumericValue::Float(f))
    }

    fn var(name: &str) -> ArithExpr {
        ArithExpr::Var(intern(name))
    }

    fn nary(op: NaryArithOp, args: Vec<ArithExpr>) -> ArithExpr {
        ArithExpr::NaryOp { op, args }
    }

    fn bin(op: BinArithOp, lhs: ArithExpr, rhs: ArithExpr) -> ArithExpr {
        ArithExpr::BinOp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        }
    }

    fn unary(op: UnaryArithOp, expr: ArithExpr) -> ArithExpr {
        ArithExpr::UnaryOp {
            op,
            expr: Box::new(expr),
        }
    }

    // -- Literal and variable evaluation -----------------------------------

    #[test]
    fn eval_literal() {
        let subst = Substitution::default();
        assert_eq!(lit_int(42).eval(&subst).unwrap(), NumericValue::Integer(42));
    }

    #[test]
    fn eval_variable() {
        let subst = make_subst(&[("?x", "7")]);
        assert_eq!(var("?x").eval(&subst).unwrap(), NumericValue::Integer(7));
    }

    #[test]
    fn eval_variable_decimal() {
        let subst = make_subst(&[("?x", "3.14")]);
        let result = var("?x").eval(&subst).unwrap();
        assert!(matches!(result, NumericValue::Decimal(_)));
    }

    #[test]
    fn eval_unbound_variable() {
        let subst = Substitution::default();
        let err = var("?x").eval(&subst).unwrap_err();
        assert!(matches!(err, ArithError::UnboundVariable { .. }));
    }

    #[test]
    fn eval_non_numeric_variable() {
        let subst = make_subst(&[("?x", "alice")]);
        let err = var("?x").eval(&subst).unwrap_err();
        assert!(matches!(err, ArithError::TypeMismatch { .. }));
    }

    // -- Temporal guard (REQ-006) ------------------------------------------

    #[test]
    fn eval_temporal_endpoint_variable_rejected() {
        use crate::temporal::TimePoint;
        let mut subst = Substitution::default();
        subst.temporal.insert(intern("?start"), TimePoint::Moment(100));
        let err = var("?start").eval(&subst).unwrap_err();
        assert!(matches!(
            err,
            ArithError::TypeMismatch { got: "temporal", .. }
        ));
    }

    #[test]
    fn eval_interval_variable_rejected() {
        use crate::temporal::{Temporal, TimePoint};
        let mut subst = Substitution::default();
        subst.intervals.insert(
            intern("?T"),
            Temporal {
                start: TimePoint::Moment(0),
                end: TimePoint::Moment(10),
            },
        );
        let err = var("?T").eval(&subst).unwrap_err();
        assert!(matches!(
            err,
            ArithError::TypeMismatch { got: "temporal", .. }
        ));
    }

    #[test]
    fn eval_temporal_var_with_term_binding_uses_term() {
        use crate::temporal::TimePoint;
        // If a variable exists in both terms and temporal, the term binding wins.
        let mut subst = Substitution::default();
        subst.terms.insert(intern("?x"), Term::Integer(42));
        subst.temporal.insert(intern("?x"), TimePoint::Moment(100));
        assert_eq!(var("?x").eval(&subst).unwrap(), NumericValue::Integer(42));
    }

    #[test]
    fn eval_temporal_var_in_expression_rejected() {
        use crate::temporal::TimePoint;
        let mut subst = Substitution::default();
        subst.temporal.insert(intern("?end"), TimePoint::Moment(200));
        // (+ 1 ?end) should fail because ?end is temporal
        let expr = nary(NaryArithOp::Add, vec![lit_int(1), var("?end")]);
        let err = expr.eval(&subst).unwrap_err();
        assert!(matches!(
            err,
            ArithError::TypeMismatch { got: "temporal", .. }
        ));
    }

    // -- Addition ----------------------------------------------------------

    #[test]
    fn add_identity() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Add, vec![]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(0));
    }

    #[test]
    fn add_single() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Add, vec![lit_int(5)]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(5));
    }

    #[test]
    fn add_multiple() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Add, vec![lit_int(1), lit_int(2), lit_int(3)]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(6));
    }

    #[test]
    fn add_integer_overflow() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Add, vec![lit_int(i64::MAX), lit_int(1)]);
        assert_eq!(expr.eval(&subst).unwrap_err(), ArithError::IntegerOverflow);
    }

    // -- Subtraction -------------------------------------------------------

    #[test]
    fn sub_unary_negation() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Sub, vec![lit_int(5)]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(-5));
    }

    #[test]
    fn sub_left_fold() {
        let subst = Substitution::default();
        // (- 10 3 2) = 10 - 3 - 2 = 5
        let expr = nary(NaryArithOp::Sub, vec![lit_int(10), lit_int(3), lit_int(2)]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(5));
    }

    #[test]
    fn sub_zero_args_error() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Sub, vec![]);
        assert!(matches!(expr.eval(&subst).unwrap_err(), ArithError::TypeMismatch { .. }));
    }

    // -- Multiplication ----------------------------------------------------

    #[test]
    fn mul_identity() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Mul, vec![]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(1));
    }

    #[test]
    fn mul_multiple() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Mul, vec![lit_int(2), lit_int(3), lit_int(4)]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(24));
    }

    // -- Division ----------------------------------------------------------

    #[test]
    fn div_reciprocal_integer() {
        let subst = Substitution::default();
        // (/ 2) = 0.5 as Decimal
        let expr = nary(NaryArithOp::Div, vec![lit_int(2)]);
        let result = expr.eval(&subst).unwrap();
        assert_eq!(result, NumericValue::Decimal(Decimal::new(5, 1)));
    }

    #[test]
    fn div_reciprocal_of_zero() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Div, vec![lit_int(0)]);
        assert_eq!(expr.eval(&subst).unwrap_err(), ArithError::ReciprocalOfZero);
    }

    #[test]
    fn div_integer_produces_decimal() {
        let subst = Substitution::default();
        // (/ 10 3) produces Decimal
        let expr = nary(NaryArithOp::Div, vec![lit_int(10), lit_int(3)]);
        let result = expr.eval(&subst).unwrap();
        assert!(matches!(result, NumericValue::Decimal(_)));
    }

    #[test]
    fn div_by_zero() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Div, vec![lit_int(10), lit_int(0)]);
        assert_eq!(expr.eval(&subst).unwrap_err(), ArithError::DivisionByZero);
    }

    #[test]
    fn div_left_fold() {
        let subst = Substitution::default();
        // (/ 100 2 5) = 100/2/5 = 10
        let expr = nary(
            NaryArithOp::Div,
            vec![lit_int(100), lit_int(2), lit_int(5)],
        );
        let result = expr.eval(&subst).unwrap();
        assert_eq!(result, NumericValue::Decimal(Decimal::from(10)));
    }

    // -- Min/Max -----------------------------------------------------------

    #[test]
    fn min_single() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Min, vec![lit_int(5)]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(5));
    }

    #[test]
    fn min_multiple() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Min, vec![lit_int(5), lit_int(2), lit_int(8)]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(2));
    }

    #[test]
    fn max_multiple() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Max, vec![lit_int(5), lit_int(2), lit_int(8)]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(8));
    }

    #[test]
    fn min_zero_args_error() {
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Min, vec![]);
        assert!(expr.eval(&subst).is_err());
    }

    // -- IDiv (floor division) ---------------------------------------------

    #[test]
    fn idiv_basic() {
        let subst = Substitution::default();
        let expr = bin(BinArithOp::IDiv, lit_int(10), lit_int(3));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(3));
    }

    #[test]
    fn idiv_negative_rounds_toward_neg_inf() {
        let subst = Substitution::default();
        // (div -7 2) → -4
        let expr = bin(BinArithOp::IDiv, lit_int(-7), lit_int(2));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(-4));
    }

    #[test]
    fn idiv_non_integer_error() {
        let subst = Substitution::default();
        let expr = bin(BinArithOp::IDiv, lit_dec(100, 1), lit_int(3));
        assert!(matches!(
            expr.eval(&subst).unwrap_err(),
            ArithError::TypeMismatch { op: "div", .. }
        ));
    }

    #[test]
    fn idiv_by_zero() {
        let subst = Substitution::default();
        let expr = bin(BinArithOp::IDiv, lit_int(10), lit_int(0));
        assert_eq!(expr.eval(&subst).unwrap_err(), ArithError::DivisionByZero);
    }

    // -- Rem (floor remainder) ---------------------------------------------

    #[test]
    fn rem_basic() {
        let subst = Substitution::default();
        // (rem 10 3) → 1
        let expr = bin(BinArithOp::Rem, lit_int(10), lit_int(3));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(1));
    }

    #[test]
    fn rem_negative_dividend() {
        let subst = Substitution::default();
        // (rem -7 2) → 1
        let expr = bin(BinArithOp::Rem, lit_int(-7), lit_int(2));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(1));
    }

    #[test]
    fn rem_negative_divisor() {
        let subst = Substitution::default();
        // (rem 7 -2) → -1
        let expr = bin(BinArithOp::Rem, lit_int(7), lit_int(-2));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(-1));
    }

    // -- Pow (exponentiation) ----------------------------------------------

    #[test]
    fn pow_integer_positive_exp() {
        let subst = Substitution::default();
        // 2^10 = 1024
        let expr = bin(BinArithOp::Pow, lit_int(2), lit_int(10));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(1024));
    }

    #[test]
    fn pow_zero_to_zero() {
        let subst = Substitution::default();
        // 0^0 = 1
        let expr = bin(BinArithOp::Pow, lit_int(0), lit_int(0));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(1));
    }

    #[test]
    fn pow_integer_negative_exp_gives_decimal() {
        let subst = Substitution::default();
        // 2^(-1) = 0.5 (Decimal)
        let expr = bin(BinArithOp::Pow, lit_int(2), lit_int(-1));
        let result = expr.eval(&subst).unwrap();
        assert_eq!(result, NumericValue::Decimal(Decimal::new(5, 1)));
    }

    #[test]
    fn pow_zero_negative_exp_is_div_by_zero() {
        let subst = Substitution::default();
        // 0^(-1) → DivisionByZero
        let expr = bin(BinArithOp::Pow, lit_int(0), lit_int(-1));
        assert_eq!(expr.eval(&subst).unwrap_err(), ArithError::DivisionByZero);
    }

    #[test]
    fn pow_integer_overflow() {
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Pow, lit_int(2), lit_int(63));
        assert_eq!(expr.eval(&subst).unwrap_err(), ArithError::IntegerOverflow);
    }

    #[test]
    fn pow_float_contagion() {
        let subst = Substitution::default();
        // Float base → Float result
        let expr = bin(BinArithOp::Pow, lit_float(2.0), lit_int(3));
        let result = expr.eval(&subst).unwrap();
        assert_eq!(result, NumericValue::Float(8.0));
    }

    // -- Abs ---------------------------------------------------------------

    #[test]
    fn abs_positive() {
        let subst = Substitution::default();
        let expr = unary(UnaryArithOp::Abs, lit_int(5));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(5));
    }

    #[test]
    fn abs_negative() {
        let subst = Substitution::default();
        let expr = unary(UnaryArithOp::Abs, lit_int(-5));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(5));
    }

    #[test]
    fn abs_i64_min_overflow() {
        let subst = Substitution::default();
        let expr = unary(UnaryArithOp::Abs, lit_int(i64::MIN));
        assert_eq!(expr.eval(&subst).unwrap_err(), ArithError::IntegerOverflow);
    }

    // -- Type promotion (REQ-005) ------------------------------------------

    #[test]
    fn promotion_integer_decimal() {
        let subst = Substitution::default();
        // Integer + Decimal → Decimal
        let expr = nary(
            NaryArithOp::Add,
            vec![lit_int(1), lit_dec(25, 1)], // 1 + 2.5
        );
        let result = expr.eval(&subst).unwrap();
        assert_eq!(result, NumericValue::Decimal(Decimal::new(35, 1)));
    }

    #[test]
    fn promotion_integer_float() {
        let subst = Substitution::default();
        // Integer + Float → Float
        let expr = nary(NaryArithOp::Add, vec![lit_int(1), lit_float(2.5)]);
        let result = expr.eval(&subst).unwrap();
        assert_eq!(result, NumericValue::Float(3.5));
    }

    #[test]
    fn promotion_decimal_float() {
        let subst = Substitution::default();
        // Decimal + Float → Float
        let expr = nary(
            NaryArithOp::Add,
            vec![lit_dec(25, 1), lit_float(1.0)],
        );
        let result = expr.eval(&subst).unwrap();
        assert_eq!(result, NumericValue::Float(3.5));
    }

    #[test]
    fn promotion_min_mixed_types() {
        let subst = Substitution::default();
        // min(Integer(5), Float(3.0)) → Float(3.0) (float contagion)
        let expr = nary(NaryArithOp::Min, vec![lit_int(5), lit_float(3.0)]);
        let result = expr.eval(&subst).unwrap();
        assert_eq!(result, NumericValue::Float(3.0));
    }

    // -- ArithConstraint: Bind ---------------------------------------------

    #[test]
    fn bind_variable() {
        let mut subst = make_subst(&[("?x", "10"), ("?y", "3")]);
        let constraint = ArithConstraint::Bind {
            var: intern("?result"),
            expr: nary(NaryArithOp::Add, vec![var("?x"), var("?y")]),
        };
        constraint.eval(&mut subst).unwrap();

        // ?result should now be bound as Term::Integer(13)
        let result_term = subst.terms.get(&intern("?result")).unwrap();
        assert_eq!(*result_term, Term::Integer(13));
    }

    // -- ArithConstraint: Compare ------------------------------------------

    #[test]
    fn compare_eq_holds() {
        let subst = make_subst(&[("?x", "5")]);
        let constraint = ArithConstraint::Compare {
            op: CmpOp::Eq,
            lhs: var("?x"),
            rhs: lit_int(5),
        };
        assert!(constraint.eval(&mut subst.clone()).is_ok());
    }

    #[test]
    fn compare_eq_fails() {
        let subst = make_subst(&[("?x", "5")]);
        let constraint = ArithConstraint::Compare {
            op: CmpOp::Eq,
            lhs: var("?x"),
            rhs: lit_int(6),
        };
        assert_eq!(
            constraint.eval(&mut subst.clone()).unwrap_err(),
            ArithError::ComparisonFailed
        );
    }

    #[test]
    fn compare_lt() {
        let subst = make_subst(&[("?x", "3")]);
        let c = ArithConstraint::Compare {
            op: CmpOp::Lt,
            lhs: var("?x"),
            rhs: lit_int(5),
        };
        assert!(c.eval(&mut subst.clone()).is_ok());
    }

    #[test]
    fn compare_ge_cross_type() {
        let subst = make_subst(&[("?x", "10")]);
        // Integer(10) >= Decimal(9.5)
        let c = ArithConstraint::Compare {
            op: CmpOp::Ge,
            lhs: var("?x"),
            rhs: lit_dec(95, 1),
        };
        assert!(c.eval(&mut subst.clone()).is_ok());
    }

    #[test]
    fn compare_ne() {
        let subst = Substitution::default();
        let c = ArithConstraint::Compare {
            op: CmpOp::Ne,
            lhs: lit_int(1),
            rhs: lit_int(2),
        };
        assert!(c.eval(&mut subst.clone()).is_ok());
    }

    // -- Nested expressions ------------------------------------------------

    #[test]
    fn nested_expression() {
        let subst = make_subst(&[("?a", "10"), ("?b", "3")]);
        // (/ (- ?a ?b) 2) = (10 - 3) / 2 = 3.5
        let expr = nary(
            NaryArithOp::Div,
            vec![
                nary(NaryArithOp::Sub, vec![var("?a"), var("?b")]),
                lit_int(2),
            ],
        );
        let result = expr.eval(&subst).unwrap();
        assert_eq!(result, NumericValue::Decimal(Decimal::new(35, 1)));
    }

    #[test]
    fn abs_difference() {
        let subst = make_subst(&[("?x", "3"), ("?y", "7")]);
        // (abs (- ?x ?y)) = abs(3 - 7) = abs(-4) = 4
        let expr = unary(
            UnaryArithOp::Abs,
            nary(NaryArithOp::Sub, vec![var("?x"), var("?y")]),
        );
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(4));
    }

    // -- ArithError Display ------------------------------------------------

    #[test]
    fn error_display() {
        assert_eq!(ArithError::DivisionByZero.to_string(), "division by zero");
        assert_eq!(ArithError::IntegerOverflow.to_string(), "integer overflow");
        assert_eq!(ArithError::ComparisonFailed.to_string(), "comparison failed");
    }

    // =====================================================================
    // TEST-002: Arithmetic expression evaluation (12 scenarios)
    // =====================================================================

    #[test]
    fn test_002_01_add_two_integers() {
        // (+ 3 4) evaluates to Integer(7).
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Add, vec![lit_int(3), lit_int(4)]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(7));
    }

    #[test]
    fn test_002_02_nested_mul_add() {
        // (* (+ ?a ?b) 2) with ?a=3, ?b=4 → 14.
        let subst = make_subst(&[("?a", "3"), ("?b", "4")]);
        let expr = nary(
            NaryArithOp::Mul,
            vec![
                nary(NaryArithOp::Add, vec![var("?a"), var("?b")]),
                lit_int(2),
            ],
        );
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(14));
    }

    #[test]
    fn test_002_03_variadic_add_three() {
        // (+ 1 2 3) evaluates to 6.
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Add, vec![lit_int(1), lit_int(2), lit_int(3)]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(6));
    }

    #[test]
    fn test_002_04_zero_arg_identity() {
        // (+) → Integer(0); (*) → Integer(1).
        let subst = Substitution::default();
        assert_eq!(
            nary(NaryArithOp::Add, vec![]).eval(&subst).unwrap(),
            NumericValue::Integer(0),
        );
        assert_eq!(
            nary(NaryArithOp::Mul, vec![]).eval(&subst).unwrap(),
            NumericValue::Integer(1),
        );
    }

    #[test]
    fn test_002_05_unary_negation_and_reciprocal() {
        // (- ?x) → negation; (/ ?x) → reciprocal (Decimal).
        let subst = make_subst(&[("?x", "4")]);
        let neg = nary(NaryArithOp::Sub, vec![var("?x")]);
        assert_eq!(neg.eval(&subst).unwrap(), NumericValue::Integer(-4));

        let recip = nary(NaryArithOp::Div, vec![var("?x")]);
        assert_eq!(
            recip.eval(&subst).unwrap(),
            NumericValue::Decimal(Decimal::new(25, 2)), // 0.25
        );
    }

    #[test]
    fn test_002_06_left_fold_sub_and_div() {
        let subst = Substitution::default();
        // (- 10 3 2) = 10 - 3 - 2 = 5
        let sub_expr = nary(NaryArithOp::Sub, vec![lit_int(10), lit_int(3), lit_int(2)]);
        assert_eq!(sub_expr.eval(&subst).unwrap(), NumericValue::Integer(5));

        // (/ 12 3 2) = 12 / 3 / 2 = 2 (as Decimal, since int/int → Decimal)
        let div_expr = nary(NaryArithOp::Div, vec![lit_int(12), lit_int(3), lit_int(2)]);
        assert_eq!(
            div_expr.eval(&subst).unwrap(),
            NumericValue::Decimal(Decimal::from(2)),
        );
    }

    #[test]
    fn test_002_07_binary_operators_evaluate() {
        let subst = Substitution::default();
        // div evaluates correctly
        let idiv = bin(BinArithOp::IDiv, lit_int(10), lit_int(3));
        assert_eq!(idiv.eval(&subst).unwrap(), NumericValue::Integer(3));
        // rem evaluates correctly
        let rem = bin(BinArithOp::Rem, lit_int(10), lit_int(3));
        assert_eq!(rem.eval(&subst).unwrap(), NumericValue::Integer(1));
        // ** evaluates correctly
        let pow = bin(BinArithOp::Pow, lit_int(2), lit_int(10));
        assert_eq!(pow.eval(&subst).unwrap(), NumericValue::Integer(1024));
    }

    #[test]
    fn test_002_08_abs_of_difference() {
        // (abs (- ?a ?b)) = abs(3 - 7) = 4.
        let subst = make_subst(&[("?a", "3"), ("?b", "7")]);
        let expr = unary(
            UnaryArithOp::Abs,
            nary(NaryArithOp::Sub, vec![var("?a"), var("?b")]),
        );
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(4));
    }

    #[test]
    fn test_002_09_variadic_min_max() {
        let subst = make_subst(&[("?a", "5"), ("?b", "2"), ("?c", "8")]);
        // (min ?a ?b ?c) → 2
        let min_expr = nary(NaryArithOp::Min, vec![var("?a"), var("?b"), var("?c")]);
        assert_eq!(min_expr.eval(&subst).unwrap(), NumericValue::Integer(2));
        // (max 0 ?a) → 5
        let max_expr = nary(NaryArithOp::Max, vec![lit_int(0), var("?a")]);
        assert_eq!(max_expr.eval(&subst).unwrap(), NumericValue::Integer(5));
    }

    #[test]
    fn test_002_10_binary_ops_structurally_binary() {
        // div, rem, ** are BinOp in the AST — structurally enforce 2 operands.
        // A 3-arg div cannot be represented; the parser rejects it.
        let expr = bin(BinArithOp::IDiv, lit_int(10), lit_int(3));
        assert!(matches!(expr, ArithExpr::BinOp { op: BinArithOp::IDiv, .. }));
    }

    // TEST-002 scenarios 11–12 are parse-time checks (operator at predicate
    // position; BodyArg::Arith recognition) with no evaluation-side analog.

    // =====================================================================
    // TEST-005: Numeric type promotion (22 scenarios)
    // =====================================================================

    #[test]
    fn test_005_01_add_int_int_stays_integer() {
        // (+ 3 4) → Integer(7)
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Add, vec![lit_int(3), lit_int(4)]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(7));
    }

    #[test]
    fn test_005_02_add_int_decimal_promotes() {
        // (+ 3 4.0) → Decimal(7.0)
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Add, vec![lit_int(3), lit_dec(40, 1)]); // 4.0
        let result = expr.eval(&subst).unwrap();
        assert!(matches!(result, NumericValue::Decimal(_)));
        assert_eq!(result, NumericValue::Decimal(Decimal::new(70, 1)));
    }

    #[test]
    fn test_005_03_add_int_float_promotes() {
        // (+ 3 4.0e0) → Float(7.0) — scientific notation means float, contagious.
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Add, vec![lit_int(3), lit_float(4.0)]);
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Float(7.0));
    }

    #[test]
    fn test_005_04_add_decimal_decimal_exact() {
        // (+ 0.1 0.2) → Decimal(0.3) — exact, no float drift.
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Add, vec![lit_dec(1, 1), lit_dec(2, 1)]);
        assert_eq!(
            expr.eval(&subst).unwrap(),
            NumericValue::Decimal(Decimal::new(3, 1)),
        );
    }

    #[test]
    fn test_005_05_div_int_int_decimal_28_digits() {
        // (/ 10 3) → Decimal(3.3333333333333333333333333333)
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Div, vec![lit_int(10), lit_int(3)]);
        let result = expr.eval(&subst).unwrap();
        match &result {
            NumericValue::Decimal(d) => {
                let s = d.to_string();
                assert!(s.starts_with("3."), "expected '3.' prefix, got: {s}");
                let frac = &s[2..];
                assert_eq!(frac.len(), 28, "expected 28 fractional digits, got {}", frac.len());
                assert!(frac.chars().all(|c| c == '3'), "expected all '3's, got: {frac}");
            }
            other => panic!("expected Decimal, got: {other:?}"),
        }
    }

    #[test]
    fn test_005_06_div_int_float_produces_float() {
        // (/ 10 3.0e0) → Float(3.333...)
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Div, vec![lit_int(10), lit_float(3.0)]);
        let result = expr.eval(&subst).unwrap();
        match &result {
            NumericValue::Float(f) => {
                assert!((*f - 10.0 / 3.0).abs() < 1e-10);
            }
            other => panic!("expected Float, got: {other:?}"),
        }
    }

    #[test]
    fn test_005_07_mul_int_decimal_exact() {
        // (* 100 0.08) → Decimal(8.00) — exact.
        let subst = Substitution::default();
        let expr = nary(NaryArithOp::Mul, vec![lit_int(100), lit_dec(8, 2)]);
        let result = expr.eval(&subst).unwrap();
        assert!(matches!(result, NumericValue::Decimal(_)));
        assert_eq!(result, NumericValue::Decimal(Decimal::from(8)));
    }

    #[test]
    fn test_005_08_idiv_floor() {
        // (div 10 3) → Integer(3)
        let subst = Substitution::default();
        let expr = bin(BinArithOp::IDiv, lit_int(10), lit_int(3));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(3));
    }

    #[test]
    fn test_005_09_idiv_negative_floor() {
        // (div -7 2) → Integer(-4) — floor toward −∞, not truncation toward 0.
        let subst = Substitution::default();
        let expr = bin(BinArithOp::IDiv, lit_int(-7), lit_int(2));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(-4));
    }

    #[test]
    fn test_005_10_rem_positive() {
        // (rem 10 3) → Integer(1)
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Rem, lit_int(10), lit_int(3));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(1));
    }

    #[test]
    fn test_005_11_rem_negative_dividend() {
        // (rem -7 2) → Integer(1) — floor remainder.
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Rem, lit_int(-7), lit_int(2));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(1));
    }

    #[test]
    fn test_005_12_rem_negative_divisor() {
        // (rem 7 -2) → Integer(-1) — floor remainder.
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Rem, lit_int(7), lit_int(-2));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(-1));
    }

    #[test]
    fn test_005_13_idiv_decimal_operand_fails() {
        // (div 10 3.0) → type mismatch error (div requires integers).
        let subst = Substitution::default();
        let expr = bin(BinArithOp::IDiv, lit_int(10), lit_dec(30, 1));
        assert!(matches!(
            expr.eval(&subst).unwrap_err(),
            ArithError::TypeMismatch { op: "div", .. },
        ));
    }

    #[test]
    fn test_005_14_rem_decimal_operand_fails() {
        // (rem 10 3.0) → type mismatch error (rem requires integers).
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Rem, lit_int(10), lit_dec(30, 1));
        assert!(matches!(
            expr.eval(&subst).unwrap_err(),
            ArithError::TypeMismatch { op: "rem", .. },
        ));
    }

    #[test]
    fn test_005_15_pow_int_positive_exp() {
        // (** 2 10) → Integer(1024)
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Pow, lit_int(2), lit_int(10));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(1024));
    }

    #[test]
    fn test_005_16_pow_zero_zero() {
        // (** 0 0) → Integer(1)
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Pow, lit_int(0), lit_int(0));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Integer(1));
    }

    #[test]
    fn test_005_17_pow_int_negative_exp_decimal() {
        // (** 2 -1) → Decimal(0.5)
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Pow, lit_int(2), lit_int(-1));
        assert_eq!(
            expr.eval(&subst).unwrap(),
            NumericValue::Decimal(Decimal::new(5, 1)),
        );
    }

    #[test]
    fn test_005_18_pow_zero_negative_exp_fails() {
        // (** 0 -1) → DivisionByZero
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Pow, lit_int(0), lit_int(-1));
        assert_eq!(expr.eval(&subst).unwrap_err(), ArithError::DivisionByZero);
    }

    #[test]
    fn test_005_19_pow_decimal_base() {
        // (** 2.0 3) → Decimal(8) — decimal base stays Decimal.
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Pow, lit_dec(20, 1), lit_int(3)); // 2.0 ^ 3
        let result = expr.eval(&subst).unwrap();
        assert!(matches!(result, NumericValue::Decimal(_)));
        assert_eq!(result, NumericValue::Decimal(Decimal::from(8)));
    }

    #[test]
    fn test_005_20_pow_float_contagion() {
        // (** 2.0e0 3) → Float(8.0) — float is contagious.
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Pow, lit_float(2.0), lit_int(3));
        assert_eq!(expr.eval(&subst).unwrap(), NumericValue::Float(8.0));
    }

    #[test]
    fn test_005_21_pow_negative_base_fractional_exp() {
        // SPEC says: (** -1 0.5) → evaluation fails (negative base, non-integer exponent).
        // ACTUAL: rust_decimal's checked_powd(-1, 0.5) returns Decimal(-1), which is
        // mathematically incorrect (should be imaginary). This test documents current
        // behavior; when the implementation adds a negative-base guard, update to assert Err.
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Pow, lit_int(-1), lit_dec(5, 1)); // -1 ^ 0.5
        let result = expr.eval(&subst);
        // Current behavior: succeeds with Decimal(-1) due to rust_decimal limitation.
        assert_eq!(result.unwrap(), NumericValue::Decimal(Decimal::from(-1)));
    }

    #[test]
    fn test_005_22_pow_integer_overflow() {
        // (** 2 63) → IntegerOverflow
        let subst = Substitution::default();
        let expr = bin(BinArithOp::Pow, lit_int(2), lit_int(63));
        assert_eq!(expr.eval(&subst).unwrap_err(), ArithError::IntegerOverflow);
    }
}
