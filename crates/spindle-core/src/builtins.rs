//! Built-in arithmetic functions registered via the extension function mechanism.
//!
//! Each built-in operator (`+`, `-`, `*`, `/`, `min`, `max`, `div`, `rem`, `**`, `abs`,
//! `round`, `floor`, `ceil`) is implemented as an [`ExtensionFunction`] and registered
//! in the prelude registry.

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

fn arith_err(e: ArithError) -> EvalError {
    EvalError::ArithError(e)
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
        if args.is_empty() {
            return Ok(Term::Integer(0));
        }
        let mut acc = to_nv(&args[0])?;
        for arg in &args[1..] {
            acc = add_values(acc, to_nv(arg)?).map_err(arith_err)?;
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
        if args.is_empty() {
            return Err(arith_err(ArithError::TypeMismatch {
                op: "-",
                expected: "1+ arguments",
                got: "0 arguments",
            }));
        }
        let first = to_nv(&args[0])?;
        if args.len() == 1 {
            return nv_to_term(negate(first).map_err(arith_err)?);
        }
        let mut acc = first;
        for arg in &args[1..] {
            acc = sub_values(acc, to_nv(arg)?).map_err(arith_err)?;
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
        if args.is_empty() {
            return Ok(Term::Integer(1));
        }
        let mut acc = to_nv(&args[0])?;
        for arg in &args[1..] {
            acc = mul_values(acc, to_nv(arg)?).map_err(arith_err)?;
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
        if args.is_empty() {
            return Err(arith_err(ArithError::TypeMismatch {
                op: "/",
                expected: "1+ arguments",
                got: "0 arguments",
            }));
        }
        let first = to_nv(&args[0])?;
        if args.len() == 1 {
            return nv_to_term(reciprocal(first).map_err(arith_err)?);
        }
        let mut acc = first;
        for arg in &args[1..] {
            acc = div_values(acc, to_nv(arg)?).map_err(arith_err)?;
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
        if args.is_empty() {
            return Err(arith_err(ArithError::TypeMismatch {
                op: "min",
                expected: "1+ arguments",
                got: "0 arguments",
            }));
        }
        let mut acc = to_nv(&args[0])?;
        for arg in &args[1..] {
            let val = to_nv(arg)?;
            let (ord, pa, pb) = numeric_cmp(acc, val).map_err(arith_err)?;
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
        if args.is_empty() {
            return Err(arith_err(ArithError::TypeMismatch {
                op: "max",
                expected: "1+ arguments",
                got: "0 arguments",
            }));
        }
        let mut acc = to_nv(&args[0])?;
        for arg in &args[1..] {
            let val = to_nv(arg)?;
            let (ord, pa, pb) = numeric_cmp(acc, val).map_err(arith_err)?;
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
        let lhs = to_nv(&args[0])?;
        let rhs = to_nv(&args[1])?;

        match (&lhs, &rhs) {
            (NumericValue::Integer(a), NumericValue::Integer(b)) => {
                if *b == 0 {
                    Err(arith_err(ArithError::DivisionByZero))
                } else {
                    let result = floor_div_i64(*a, *b).map_err(arith_err)?;
                    Ok(Term::Integer(result))
                }
            }
            _ => Err(arith_err(ArithError::TypeMismatch {
                op: "div",
                expected: "Integer",
                got: type_name(if !matches!(lhs, NumericValue::Integer(_)) {
                    &lhs
                } else {
                    &rhs
                }),
            })),
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
        let lhs = to_nv(&args[0])?;
        let rhs = to_nv(&args[1])?;

        match (&lhs, &rhs) {
            (NumericValue::Integer(a), NumericValue::Integer(b)) => {
                if *b == 0 {
                    Err(arith_err(ArithError::DivisionByZero))
                } else {
                    let result = floor_rem_i64(*a, *b).map_err(arith_err)?;
                    Ok(Term::Integer(result))
                }
            }
            _ => Err(arith_err(ArithError::TypeMismatch {
                op: "rem",
                expected: "Integer",
                got: type_name(if !matches!(lhs, NumericValue::Integer(_)) {
                    &lhs
                } else {
                    &rhs
                }),
            })),
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
        let base = to_nv(&args[0])?;
        let exp = to_nv(&args[1])?;
        nv_to_term(eval_pow(base, exp).map_err(arith_err)?)
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
        let v = to_nv(&args[0])?;
        nv_to_term(abs_value(v).map_err(arith_err)?)
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
        use rust_decimal::RoundingStrategy;

        let value = to_nv(&args[0])?;
        let dp_nv = to_nv(&args[1])?;

        // dp must be a non-negative integer
        let dp = match &dp_nv {
            NumericValue::Integer(n) => {
                if *n < 0 {
                    return Err(arith_err(ArithError::TypeMismatch {
                        op: "round",
                        expected: "non-negative integer dp",
                        got: "negative integer",
                    }));
                }
                *n as u32
            }
            _ => {
                return Err(arith_err(ArithError::TypeMismatch {
                    op: "round",
                    expected: "Integer dp",
                    got: type_name(&dp_nv),
                }));
            }
        };

        // Convert value to Decimal for rounding
        let dec = match &value {
            NumericValue::Integer(n) => Decimal::from(*n),
            NumericValue::Decimal(d) => *d,
            NumericValue::Float(f) => {
                Decimal::try_from(*f).map_err(|_| arith_err(ArithError::NonFiniteFloat))?
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
        use rust_decimal::prelude::ToPrimitive;

        let value = to_nv(&args[0])?;
        match &value {
            NumericValue::Integer(n) => Ok(Term::Integer(*n)),
            NumericValue::Decimal(d) => {
                let floored = d.floor();
                let n = floored
                    .to_i64()
                    .ok_or_else(|| arith_err(ArithError::IntegerOverflow))?;
                Ok(Term::Integer(n))
            }
            NumericValue::Float(f) => {
                let floored = f.floor();
                if !floored.is_finite() {
                    return Err(arith_err(ArithError::NonFiniteFloat));
                }
                let n = floored as i64;
                // Check round-trip
                if (n as f64) != floored {
                    return Err(arith_err(ArithError::IntegerOverflow));
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
        use rust_decimal::prelude::ToPrimitive;

        let value = to_nv(&args[0])?;
        match &value {
            NumericValue::Integer(n) => Ok(Term::Integer(*n)),
            NumericValue::Decimal(d) => {
                let ceiled = d.ceil();
                let n = ceiled
                    .to_i64()
                    .ok_or_else(|| arith_err(ArithError::IntegerOverflow))?;
                Ok(Term::Integer(n))
            }
            NumericValue::Float(f) => {
                let floored = f.ceil();
                if !floored.is_finite() {
                    return Err(arith_err(ArithError::NonFiniteFloat));
                }
                let n = floored as i64;
                // Check round-trip
                if (n as f64) != floored {
                    return Err(arith_err(ArithError::IntegerOverflow));
                }
                Ok(Term::Integer(n))
            }
        }
    }
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
    use crate::term::NumericValue;
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
}
