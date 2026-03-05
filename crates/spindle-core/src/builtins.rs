//! Built-in arithmetic functions registered via the extension function mechanism.
//!
//! Each built-in operator (`+`, `-`, `*`, `/`, `min`, `max`, `div`, `rem`, `**`, `abs`,
//! `round`, `floor`, `ceil`) is implemented as an [`ExtensionFunction`] and registered
//! in the prelude registry.

use chrono::Duration as ChronoDuration;
use rust_decimal::Decimal;

use crate::arith::{
    ArithError, abs_value, add_values, div_values, eval_pow, floor_div_i64, floor_rem_i64,
    mul_values, negate, numeric_cmp, reciprocal, sub_values, type_name,
};
use crate::function_registry::{
    Arity, EvalError, ExtensionFunction, FunctionRegistry, FunctionSignature,
};
use crate::intern::intern;
use crate::term::{NumericValue, Term};

// ---------------------------------------------------------------------------
// Helper: Term -> NumericValue, NumericValue -> Term
// ---------------------------------------------------------------------------

fn to_nv(t: &Term) -> Result<NumericValue, EvalError> {
    t.to_numeric_value()
        .ok_or_else(|| EvalError::TypeError(format!("expected numeric, got {t:?}")))
}

fn nv_to_term(nv: NumericValue) -> Result<Term, EvalError> {
    Term::try_from(nv).map_err(|e| EvalError::EvalFailed(e.to_string()))
}

fn check_arity(sig: &FunctionSignature, args: &[Term]) -> Result<(), EvalError> {
    if !sig.arity.accepts(args.len()) {
        return Err(EvalError::ArithError(ArithError::ArityMismatch {
            name: sig.name,
            expected: sig.arity.to_string(),
            got: args.len(),
        }));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// AddFn (+)
// ---------------------------------------------------------------------------

pub(crate) struct AddFn {
    sig: FunctionSignature,
}

impl AddFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("+"),
                arity: Arity::Range(0, usize::MAX),
                description: "addition (variadic)",
            },
        }
    }
}

impl ExtensionFunction for AddFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        if args.is_empty() {
            return Ok(Term::Integer(0));
        }
        // Binary temporal addition cases
        if args.len() == 2 {
            if let Some(result) = try_temporal_add(&args[0], &args[1])? {
                return Ok(result);
            }
        }
        let mut acc = to_nv(&args[0])?;
        for arg in &args[1..] {
            acc = add_values(acc, to_nv(arg)?)?;
        }
        nv_to_term(acc)
    }
}

// ---------------------------------------------------------------------------
// SubFn (-)
// ---------------------------------------------------------------------------

pub(crate) struct SubFn {
    sig: FunctionSignature,
}

impl SubFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("-"),
                arity: Arity::Range(1, usize::MAX),
                description: "subtraction / negation (variadic)",
            },
        }
    }
}

impl ExtensionFunction for SubFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        // Binary temporal subtraction cases
        if args.len() == 2 {
            if let Some(result) = try_temporal_sub(&args[0], &args[1])? {
                return Ok(result);
            }
        }
        // Safe: check_arity guarantees at least 1 argument
        let first = to_nv(&args[0])?;
        if args.len() == 1 {
            return nv_to_term(negate(first)?);
        }
        let mut acc = first;
        for arg in &args[1..] {
            acc = sub_values(acc, to_nv(arg)?)?;
        }
        nv_to_term(acc)
    }
}

// ---------------------------------------------------------------------------
// MulFn (*)
// ---------------------------------------------------------------------------

pub(crate) struct MulFn {
    sig: FunctionSignature,
}

impl MulFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("*"),
                arity: Arity::Range(0, usize::MAX),
                description: "multiplication (variadic)",
            },
        }
    }
}

impl ExtensionFunction for MulFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        if args.is_empty() {
            return Ok(Term::Integer(1));
        }
        // Binary temporal multiplication cases
        if args.len() == 2 {
            if let Some(result) = try_temporal_mul(&args[0], &args[1])? {
                return Ok(result);
            }
        }
        let mut acc = to_nv(&args[0])?;
        for arg in &args[1..] {
            acc = mul_values(acc, to_nv(arg)?)?;
        }
        nv_to_term(acc)
    }
}

// ---------------------------------------------------------------------------
// DivFn (/)
// ---------------------------------------------------------------------------

pub(crate) struct DivFn {
    sig: FunctionSignature,
}

impl DivFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("/"),
                arity: Arity::Range(1, usize::MAX),
                description: "division / reciprocal (variadic)",
            },
        }
    }
}

impl ExtensionFunction for DivFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        // Binary temporal division cases
        if args.len() == 2 {
            if let Some(result) = try_temporal_div(&args[0], &args[1])? {
                return Ok(result);
            }
        }
        // Safe: check_arity guarantees at least 1 argument
        let first = to_nv(&args[0])?;
        if args.len() == 1 {
            return nv_to_term(reciprocal(first)?);
        }
        let mut acc = first;
        for arg in &args[1..] {
            acc = div_values(acc, to_nv(arg)?)?;
        }
        nv_to_term(acc)
    }
}

// ---------------------------------------------------------------------------
// MinFn (min)
// ---------------------------------------------------------------------------

pub(crate) struct MinFn {
    sig: FunctionSignature,
}

impl MinFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("min"),
                arity: Arity::Range(1, usize::MAX),
                description: "minimum (variadic)",
            },
        }
    }
}

impl ExtensionFunction for MinFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        // Check if all args are the same temporal type
        if args.first().map_or(false, |a| a.is_temporal()) {
            return temporal_min_max(args, true);
        }
        // Safe: check_arity guarantees at least 1 argument
        let mut acc = to_nv(&args[0])?;
        for arg in &args[1..] {
            let val = to_nv(arg)?;
            let (ord, pa, pb) = numeric_cmp(acc, val)?;
            acc = if ord.is_le() { pa } else { pb };
        }
        nv_to_term(acc)
    }
}

// ---------------------------------------------------------------------------
// MaxFn (max)
// ---------------------------------------------------------------------------

pub(crate) struct MaxFn {
    sig: FunctionSignature,
}

impl MaxFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("max"),
                arity: Arity::Range(1, usize::MAX),
                description: "maximum (variadic)",
            },
        }
    }
}

impl ExtensionFunction for MaxFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        // Check if all args are the same temporal type
        if args.first().map_or(false, |a| a.is_temporal()) {
            return temporal_min_max(args, false);
        }
        // Safe: check_arity guarantees at least 1 argument
        let mut acc = to_nv(&args[0])?;
        for arg in &args[1..] {
            let val = to_nv(arg)?;
            let (ord, pa, pb) = numeric_cmp(acc, val)?;
            acc = if ord.is_ge() { pa } else { pb };
        }
        nv_to_term(acc)
    }
}

// ---------------------------------------------------------------------------
// IDivFn (div)
// ---------------------------------------------------------------------------

pub(crate) struct IDivFn {
    sig: FunctionSignature,
}

impl IDivFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("div"),
                arity: Arity::Fixed(2),
                description: "integer floor division",
            },
        }
    }
}

impl ExtensionFunction for IDivFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let lhs = to_nv(&args[0])?;
        let rhs = to_nv(&args[1])?;

        match (&lhs, &rhs) {
            (NumericValue::Integer(a), NumericValue::Integer(b)) => {
                if *b == 0 {
                    Err(ArithError::DivisionByZero.into())
                } else {
                    Ok(Term::Integer(floor_div_i64(*a, *b)?))
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
            }
            .into()),
        }
    }
}

// ---------------------------------------------------------------------------
// RemFn (rem)
// ---------------------------------------------------------------------------

pub(crate) struct RemFn {
    sig: FunctionSignature,
}

impl RemFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("rem"),
                arity: Arity::Fixed(2),
                description: "integer floor remainder",
            },
        }
    }
}

impl ExtensionFunction for RemFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let lhs = to_nv(&args[0])?;
        let rhs = to_nv(&args[1])?;

        match (&lhs, &rhs) {
            (NumericValue::Integer(a), NumericValue::Integer(b)) => {
                if *b == 0 {
                    Err(ArithError::DivisionByZero.into())
                } else {
                    Ok(Term::Integer(floor_rem_i64(*a, *b)?))
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
            }
            .into()),
        }
    }
}

// ---------------------------------------------------------------------------
// PowFn (**)
// ---------------------------------------------------------------------------

pub(crate) struct PowFn {
    sig: FunctionSignature,
}

impl PowFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("**"),
                arity: Arity::Fixed(2),
                description: "exponentiation",
            },
        }
    }
}

impl ExtensionFunction for PowFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let base = to_nv(&args[0])?;
        let exp = to_nv(&args[1])?;
        nv_to_term(eval_pow(base, exp)?)
    }
}

// ---------------------------------------------------------------------------
// AbsFn (abs)
// ---------------------------------------------------------------------------

pub(crate) struct AbsFn {
    sig: FunctionSignature,
}

impl AbsFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("abs"),
                arity: Arity::Fixed(1),
                description: "absolute value",
            },
        }
    }
}

impl ExtensionFunction for AbsFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let v = to_nv(&args[0])?;
        nv_to_term(abs_value(v)?)
    }
}

// ---------------------------------------------------------------------------
// RoundFn (round)
// ---------------------------------------------------------------------------

pub(crate) struct RoundFn {
    sig: FunctionSignature,
}

impl RoundFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("round"),
                arity: Arity::Fixed(2),
                description: "banker's rounding (half-to-even) to dp decimal places",
            },
        }
    }
}

impl ExtensionFunction for RoundFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        use rust_decimal::RoundingStrategy;

        let value = to_nv(&args[0])?;
        let dp_nv = to_nv(&args[1])?;

        // dp must be a non-negative integer
        let dp = match &dp_nv {
            NumericValue::Integer(n) => {
                if *n < 0 {
                    return Err(ArithError::TypeMismatch {
                        op: "round",
                        expected: "non-negative integer dp",
                        got: "negative integer",
                    }
                    .into());
                }
                u32::try_from(*n).map_err(|_| {
                    EvalError::from(ArithError::TypeMismatch {
                        op: "round",
                        expected: "non-negative integer dp (0..4294967295)",
                        got: "integer too large for dp",
                    })
                })?
            }
            _ => {
                return Err(ArithError::TypeMismatch {
                    op: "round",
                    expected: "Integer dp",
                    got: type_name(&dp_nv),
                }
                .into());
            }
        };

        // Convert value to Decimal for rounding
        let dec = match &value {
            NumericValue::Integer(n) => Decimal::from(*n),
            NumericValue::Decimal(d) => *d,
            NumericValue::Float(f) => {
                Decimal::try_from(*f).map_err(|_| EvalError::from(ArithError::NonFiniteFloat))?
            }
        };

        let rounded = dec.round_dp_with_strategy(dp, RoundingStrategy::MidpointNearestEven);
        Ok(Term::Decimal(rounded))
    }
}

// ---------------------------------------------------------------------------
// FloorFn (floor)
// ---------------------------------------------------------------------------

pub(crate) struct FloorFn {
    sig: FunctionSignature,
}

impl FloorFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("floor"),
                arity: Arity::Fixed(1),
                description: "largest integer <= value",
            },
        }
    }
}

impl ExtensionFunction for FloorFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        use rust_decimal::prelude::ToPrimitive;

        let value = to_nv(&args[0])?;
        match &value {
            NumericValue::Integer(n) => Ok(Term::Integer(*n)),
            NumericValue::Decimal(d) => {
                let floored = d.floor();
                let n = floored.to_i64().ok_or(ArithError::IntegerOverflow)?;
                Ok(Term::Integer(n))
            }
            NumericValue::Float(f) => {
                let floored = f.floor();
                if !floored.is_finite() {
                    return Err(ArithError::NonFiniteFloat.into());
                }
                let n = floored as i64;
                if (n as f64) != floored {
                    return Err(ArithError::IntegerOverflow.into());
                }
                Ok(Term::Integer(n))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CeilFn (ceil)
// ---------------------------------------------------------------------------

pub(crate) struct CeilFn {
    sig: FunctionSignature,
}

impl CeilFn {
    pub(crate) fn new() -> Self {
        Self {
            sig: FunctionSignature {
                name: intern("ceil"),
                arity: Arity::Fixed(1),
                description: "smallest integer >= value",
            },
        }
    }
}

impl ExtensionFunction for CeilFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        use rust_decimal::prelude::ToPrimitive;

        let value = to_nv(&args[0])?;
        match &value {
            NumericValue::Integer(n) => Ok(Term::Integer(*n)),
            NumericValue::Decimal(d) => {
                let ceiled = d.ceil();
                let n = ceiled.to_i64().ok_or(ArithError::IntegerOverflow)?;
                Ok(Term::Integer(n))
            }
            NumericValue::Float(f) => {
                let ceiled = f.ceil();
                if !ceiled.is_finite() {
                    return Err(ArithError::NonFiniteFloat.into());
                }
                let n = ceiled as i64;
                if (n as f64) != ceiled {
                    return Err(ArithError::IntegerOverflow.into());
                }
                Ok(Term::Integer(n))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Temporal arithmetic helpers
// ---------------------------------------------------------------------------

/// Convert a numeric Term to an f64 for scaling operations.
fn term_to_f64(t: &Term) -> Option<f64> {
    match t {
        Term::Integer(n) => Some(*n as f64),
        Term::Decimal(d) => {
            use rust_decimal::prelude::ToPrimitive;
            d.to_f64()
        }
        Term::Float(f) => Some(f.value()),
        _ => None,
    }
}

/// Try temporal addition. Returns Ok(Some(result)) if a temporal case matched,
/// Ok(None) if not temporal, or Err on type errors.
fn try_temporal_add(a: &Term, b: &Term) -> Result<Option<Term>, EvalError> {
    match (a, b) {
        // Datetime + Duration → Datetime
        (Term::Datetime(dt), Term::Duration(m)) | (Term::Duration(m), Term::Datetime(dt)) => {
            let result = *dt + ChronoDuration::minutes(*m);
            Ok(Some(Term::Datetime(result)))
        }
        // Date + Duration → Date (whole days only)
        (Term::Date(d), Term::Duration(m)) | (Term::Duration(m), Term::Date(d)) => {
            if *m % 1440 != 0 {
                return Err(EvalError::TypeError(
                    "Date + Duration requires duration to be whole days (multiple of 1440 minutes)"
                        .to_string(),
                ));
            }
            let days = *m / 1440;
            let result = *d + ChronoDuration::days(days);
            Ok(Some(Term::Date(result)))
        }
        // Duration + Duration → Duration
        (Term::Duration(a), Term::Duration(b)) => Ok(Some(Term::Duration(
            a.checked_add(*b)
                .ok_or_else(|| EvalError::EvalFailed("Duration overflow".to_string()))?,
        ))),
        _ => Ok(None),
    }
}

/// Try temporal subtraction. Returns Ok(Some(result)) if a temporal case matched.
fn try_temporal_sub(a: &Term, b: &Term) -> Result<Option<Term>, EvalError> {
    match (a, b) {
        // Datetime - Duration → Datetime
        (Term::Datetime(dt), Term::Duration(m)) => {
            let result = *dt - ChronoDuration::minutes(*m);
            Ok(Some(Term::Datetime(result)))
        }
        // Datetime - Datetime → Duration (signed difference in minutes)
        (Term::Datetime(dt1), Term::Datetime(dt2)) => {
            let diff = (*dt1 - *dt2).num_minutes();
            Ok(Some(Term::Duration(diff)))
        }
        // Date - Duration → Date (whole days only)
        (Term::Date(d), Term::Duration(m)) => {
            if *m % 1440 != 0 {
                return Err(EvalError::TypeError(
                    "Date - Duration requires duration to be whole days (multiple of 1440 minutes)"
                        .to_string(),
                ));
            }
            let days = *m / 1440;
            let result = *d - ChronoDuration::days(days);
            Ok(Some(Term::Date(result)))
        }
        // Date - Date → Integer (signed days)
        (Term::Date(d1), Term::Date(d2)) => {
            let diff = (*d1 - *d2).num_days();
            Ok(Some(Term::Integer(diff)))
        }
        // Duration - Duration → Duration
        (Term::Duration(a), Term::Duration(b)) => Ok(Some(Term::Duration(
            a.checked_sub(*b)
                .ok_or_else(|| EvalError::EvalFailed("Duration overflow".to_string()))?,
        ))),
        _ => Ok(None),
    }
}

/// Try temporal multiplication. Returns Ok(Some(result)) if a temporal case matched.
fn try_temporal_mul(a: &Term, b: &Term) -> Result<Option<Term>, EvalError> {
    match (a, b) {
        // Duration * Number → Duration
        (Term::Duration(m), other) | (other, Term::Duration(m)) => {
            let scale = term_to_f64(other).ok_or_else(|| {
                EvalError::TypeError(format!("Cannot multiply Duration by {other:?}"))
            })?;
            let result = (*m as f64 * scale) as i64;
            Ok(Some(Term::Duration(result)))
        }
        _ => Ok(None),
    }
}

/// Try temporal division. Returns Ok(Some(result)) if a temporal case matched.
fn try_temporal_div(a: &Term, b: &Term) -> Result<Option<Term>, EvalError> {
    match (a, b) {
        // Duration / Number → Duration
        (Term::Duration(m), other) if other.to_numeric_value().is_some() => {
            let scale = term_to_f64(other).ok_or_else(|| {
                EvalError::TypeError(format!("Cannot divide Duration by {other:?}"))
            })?;
            if scale == 0.0 {
                return Err(EvalError::from(ArithError::DivisionByZero));
            }
            let result = (*m as f64 / scale) as i64;
            Ok(Some(Term::Duration(result)))
        }
        // Duration / Duration → Decimal (ratio)
        (Term::Duration(a), Term::Duration(b)) => {
            if *b == 0 {
                return Err(EvalError::from(ArithError::DivisionByZero));
            }
            let ratio = Decimal::from(*a) / Decimal::from(*b);
            Ok(Some(Term::Decimal(ratio)))
        }
        _ => Ok(None),
    }
}

/// Temporal min/max: compare using temporal_cmp. `is_min` selects min behavior.
fn temporal_min_max(args: &[Term], is_min: bool) -> Result<Term, EvalError> {
    let mut best = args[0].clone();
    for arg in &args[1..] {
        let ord = best.temporal_cmp(arg).ok_or_else(|| {
            EvalError::TypeError(format!(
                "Cannot compare {best:?} with {arg:?} — mixed temporal types"
            ))
        })?;
        if (is_min && ord.is_gt()) || (!is_min && ord.is_lt()) {
            best = arg.clone();
        }
    }
    Ok(best)
}

// ---------------------------------------------------------------------------
// Prelude registration
// ---------------------------------------------------------------------------

/// Register all built-in arithmetic functions into the given registry.
pub(crate) fn register_builtins(registry: &mut FunctionRegistry) {
    registry.register(Box::new(AddFn::new()));
    registry.register(Box::new(SubFn::new()));
    registry.register(Box::new(MulFn::new()));
    registry.register(Box::new(DivFn::new()));
    registry.register(Box::new(MinFn::new()));
    registry.register(Box::new(MaxFn::new()));
    registry.register(Box::new(IDivFn::new()));
    registry.register(Box::new(RemFn::new()));
    registry.register(Box::new(PowFn::new()));
    registry.register(Box::new(AbsFn::new()));
    registry.register(Box::new(RoundFn::new()));
    registry.register(Box::new(FloorFn::new()));
    registry.register(Box::new(CeilFn::new()));
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    fn eval_builtin(name: &str, args: &[Term]) -> Result<Term, EvalError> {
        let mut reg = FunctionRegistry::new();
        register_builtins(&mut reg);
        let func = reg.get(intern(name)).unwrap();
        func.eval(args)
    }

    // ===================== + =====================

    #[test]
    fn add_zero_args() {
        assert_eq!(eval_builtin("+", &[]).unwrap(), Term::Integer(0));
    }

    #[test]
    fn add_one_arg() {
        assert_eq!(
            eval_builtin("+", &[Term::Integer(5)]).unwrap(),
            Term::Integer(5)
        );
    }

    #[test]
    fn add_two_ints() {
        assert_eq!(
            eval_builtin("+", &[Term::Integer(3), Term::Integer(4)]).unwrap(),
            Term::Integer(7)
        );
    }

    #[test]
    fn add_type_promotion() {
        let result =
            eval_builtin("+", &[Term::Integer(1), Term::Decimal(Decimal::new(25, 1))]).unwrap();
        assert_eq!(result, Term::Decimal(Decimal::new(35, 1)));
    }

    // ===================== - =====================

    #[test]
    fn sub_negation() {
        assert_eq!(
            eval_builtin("-", &[Term::Integer(5)]).unwrap(),
            Term::Integer(-5)
        );
    }

    #[test]
    fn sub_two_args() {
        assert_eq!(
            eval_builtin("-", &[Term::Integer(10), Term::Integer(3)]).unwrap(),
            Term::Integer(7)
        );
    }

    #[test]
    fn sub_three_args() {
        assert_eq!(
            eval_builtin(
                "-",
                &[Term::Integer(10), Term::Integer(3), Term::Integer(2)]
            )
            .unwrap(),
            Term::Integer(5)
        );
    }

    // ===================== * =====================

    #[test]
    fn mul_zero_args() {
        assert_eq!(eval_builtin("*", &[]).unwrap(), Term::Integer(1));
    }

    #[test]
    fn mul_two_args() {
        assert_eq!(
            eval_builtin("*", &[Term::Integer(3), Term::Integer(4)]).unwrap(),
            Term::Integer(12)
        );
    }

    // ===================== / =====================

    #[test]
    fn div_reciprocal() {
        let result = eval_builtin("/", &[Term::Integer(2)]).unwrap();
        assert_eq!(result, Term::Decimal(Decimal::new(5, 1)));
    }

    #[test]
    fn div_two_ints() {
        let result = eval_builtin("/", &[Term::Integer(10), Term::Integer(4)]).unwrap();
        assert_eq!(result, Term::Decimal(Decimal::new(25, 1)));
    }

    #[test]
    fn div_by_zero() {
        let result = eval_builtin("/", &[Term::Integer(1), Term::Integer(0)]);
        assert!(result.is_err());
    }

    // ===================== min =====================

    #[test]
    fn min_single() {
        assert_eq!(
            eval_builtin("min", &[Term::Integer(5)]).unwrap(),
            Term::Integer(5)
        );
    }

    #[test]
    fn min_three() {
        assert_eq!(
            eval_builtin(
                "min",
                &[Term::Integer(5), Term::Integer(2), Term::Integer(8)]
            )
            .unwrap(),
            Term::Integer(2)
        );
    }

    // ===================== max =====================

    #[test]
    fn max_three() {
        assert_eq!(
            eval_builtin(
                "max",
                &[Term::Integer(5), Term::Integer(2), Term::Integer(8)]
            )
            .unwrap(),
            Term::Integer(8)
        );
    }

    // ===================== div (idiv) =====================

    #[test]
    fn idiv_positive() {
        assert_eq!(
            eval_builtin("div", &[Term::Integer(10), Term::Integer(3)]).unwrap(),
            Term::Integer(3)
        );
    }

    #[test]
    fn idiv_negative_floor() {
        assert_eq!(
            eval_builtin("div", &[Term::Integer(-7), Term::Integer(2)]).unwrap(),
            Term::Integer(-4)
        );
    }

    #[test]
    fn idiv_type_error() {
        let result = eval_builtin(
            "div",
            &[Term::Decimal(Decimal::new(10, 0)), Term::Integer(3)],
        );
        assert!(result.is_err());
    }

    // ===================== rem =====================

    #[test]
    fn rem_positive() {
        assert_eq!(
            eval_builtin("rem", &[Term::Integer(10), Term::Integer(3)]).unwrap(),
            Term::Integer(1)
        );
    }

    #[test]
    fn rem_negative_floor() {
        assert_eq!(
            eval_builtin("rem", &[Term::Integer(-7), Term::Integer(2)]).unwrap(),
            Term::Integer(1)
        );
    }

    // ===================== ** =====================

    #[test]
    fn pow_int() {
        assert_eq!(
            eval_builtin("**", &[Term::Integer(2), Term::Integer(10)]).unwrap(),
            Term::Integer(1024)
        );
    }

    #[test]
    fn pow_negative_exp() {
        let result = eval_builtin("**", &[Term::Integer(2), Term::Integer(-1)]).unwrap();
        assert_eq!(result, Term::Decimal(Decimal::new(5, 1)));
    }

    // ===================== abs =====================

    #[test]
    fn abs_positive() {
        assert_eq!(
            eval_builtin("abs", &[Term::Integer(5)]).unwrap(),
            Term::Integer(5)
        );
    }

    #[test]
    fn abs_negative() {
        assert_eq!(
            eval_builtin("abs", &[Term::Integer(-5)]).unwrap(),
            Term::Integer(5)
        );
    }

    #[test]
    fn abs_decimal() {
        assert_eq!(
            eval_builtin("abs", &[Term::Decimal(Decimal::new(-314, 2))]).unwrap(),
            Term::Decimal(Decimal::new(314, 2))
        );
    }

    // ===================== overflow =====================

    #[test]
    fn add_overflow() {
        let result = eval_builtin("+", &[Term::Integer(i64::MAX), Term::Integer(1)]);
        assert!(result.is_err());
    }

    #[test]
    fn reciprocal_of_zero() {
        let result = eval_builtin("/", &[Term::Integer(0)]);
        assert!(result.is_err());
    }

    // ===================== arity safety =====================

    #[test]
    fn div_wrong_arity() {
        let result = eval_builtin("div", &[Term::Integer(1)]);
        assert!(result.is_err());
        let result = eval_builtin(
            "div",
            &[Term::Integer(1), Term::Integer(2), Term::Integer(3)],
        );
        assert!(result.is_err());
    }

    #[test]
    fn rem_wrong_arity() {
        let result = eval_builtin("rem", &[Term::Integer(1)]);
        assert!(result.is_err());
        let result = eval_builtin(
            "rem",
            &[Term::Integer(1), Term::Integer(2), Term::Integer(3)],
        );
        assert!(result.is_err());
    }

    #[test]
    fn pow_wrong_arity() {
        let result = eval_builtin("**", &[Term::Integer(2)]);
        assert!(result.is_err());
        let result = eval_builtin(
            "**",
            &[Term::Integer(2), Term::Integer(3), Term::Integer(4)],
        );
        assert!(result.is_err());
    }

    #[test]
    fn abs_wrong_arity() {
        let result = eval_builtin("abs", &[]);
        assert!(result.is_err());
        let result = eval_builtin("abs", &[Term::Integer(1), Term::Integer(2)]);
        assert!(result.is_err());
    }

    #[test]
    fn round_wrong_arity() {
        let result = eval_builtin("round", &[Term::Integer(1)]);
        assert!(result.is_err());
        let result = eval_builtin(
            "round",
            &[Term::Integer(1), Term::Integer(2), Term::Integer(3)],
        );
        assert!(result.is_err());
    }

    #[test]
    fn floor_wrong_arity() {
        let result = eval_builtin("floor", &[]);
        assert!(result.is_err());
        let result = eval_builtin("floor", &[Term::Integer(1), Term::Integer(2)]);
        assert!(result.is_err());
    }

    #[test]
    fn ceil_wrong_arity() {
        let result = eval_builtin("ceil", &[]);
        assert!(result.is_err());
        let result = eval_builtin("ceil", &[Term::Integer(1), Term::Integer(2)]);
        assert!(result.is_err());
    }

    #[test]
    fn sub_zero_args() {
        let result = eval_builtin("-", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn div_slash_zero_args() {
        let result = eval_builtin("/", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn min_zero_args() {
        let result = eval_builtin("min", &[]);
        assert!(result.is_err());
    }

    #[test]
    fn max_zero_args() {
        let result = eval_builtin("max", &[]);
        assert!(result.is_err());
    }

    // ===================== large dp =====================

    #[test]
    fn round_large_dp() {
        let result = eval_builtin(
            "round",
            &[Term::Decimal(Decimal::new(314, 2)), Term::Integer(i64::MAX)],
        );
        assert!(result.is_err());
    }

    // ===================== mixed-type min/max =====================

    #[test]
    fn min_mixed_int_decimal() {
        let result = eval_builtin(
            "min",
            &[Term::Integer(5), Term::Decimal(Decimal::new(30, 1))],
        )
        .unwrap();
        assert_eq!(result, Term::Decimal(Decimal::new(30, 1)));
    }

    #[test]
    fn max_mixed_int_decimal() {
        let result = eval_builtin(
            "max",
            &[Term::Integer(5), Term::Decimal(Decimal::new(30, 1))],
        )
        .unwrap();
        // numeric_cmp promotes Integer(5) to Decimal(5) when comparing mixed types
        assert_eq!(result, Term::Decimal(Decimal::new(5, 0)));
    }

    // ===================== round/floor/ceil unit =====================

    #[test]
    fn round_basic() {
        let result = eval_builtin(
            "round",
            &[Term::Decimal(Decimal::new(314, 2)), Term::Integer(1)],
        )
        .unwrap();
        assert_eq!(result, Term::Decimal(Decimal::new(31, 1)));
    }

    #[test]
    fn round_half_even() {
        let result = eval_builtin(
            "round",
            &[Term::Decimal(Decimal::new(25, 1)), Term::Integer(0)],
        )
        .unwrap();
        assert_eq!(result, Term::Decimal(Decimal::new(2, 0)));
    }

    #[test]
    fn floor_decimal() {
        let result = eval_builtin("floor", &[Term::Decimal(Decimal::new(314, 2))]).unwrap();
        assert_eq!(result, Term::Integer(3));
    }

    #[test]
    fn floor_negative_decimal() {
        let result = eval_builtin("floor", &[Term::Decimal(Decimal::new(-314, 2))]).unwrap();
        assert_eq!(result, Term::Integer(-4));
    }

    #[test]
    fn ceil_decimal() {
        let result = eval_builtin("ceil", &[Term::Decimal(Decimal::new(314, 2))]).unwrap();
        assert_eq!(result, Term::Integer(4));
    }

    #[test]
    fn ceil_negative_decimal() {
        let result = eval_builtin("ceil", &[Term::Decimal(Decimal::new(-314, 2))]).unwrap();
        assert_eq!(result, Term::Integer(-3));
    }

    #[test]
    fn floor_integer_passthrough() {
        assert_eq!(
            eval_builtin("floor", &[Term::Integer(7)]).unwrap(),
            Term::Integer(7)
        );
    }

    #[test]
    fn ceil_integer_passthrough() {
        assert_eq!(
            eval_builtin("ceil", &[Term::Integer(7)]).unwrap(),
            Term::Integer(7)
        );
    }
}
