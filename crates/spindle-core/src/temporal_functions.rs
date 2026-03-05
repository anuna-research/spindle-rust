//! Temporal extension functions for date/time operations.
//!
//! These functions are registered in the prelude and available from `bind` expressions.

use chrono::{Datelike, NaiveDate, Timelike, TimeZone};

use crate::function_registry::{
    Arity, EvalError, ExtensionFunction, FunctionRegistry, FunctionSignature,
};
use crate::intern::intern;
use crate::term::Term;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn check_arity(sig: &FunctionSignature, args: &[Term]) -> Result<(), EvalError> {
    if !sig.arity.accepts(args.len()) {
        return Err(EvalError::ArithError(
            crate::arith::ArithError::ArityMismatch {
                name: sig.name,
                expected: sig.arity.to_string(),
                got: args.len(),
            },
        ));
    }
    Ok(())
}

macro_rules! temporal_fn {
    ($struct_name:ident, $fn_name:expr, $arity:expr, $desc:expr) => {
        pub(crate) struct $struct_name {
            sig: FunctionSignature,
        }

        impl $struct_name {
            pub(crate) fn new() -> Self {
                Self {
                    sig: FunctionSignature {
                        name: intern($fn_name),
                        arity: $arity,
                        description: $desc,
                    },
                }
            }
        }
    };
}

// ---------------------------------------------------------------------------
// datetime(Date, Time, Offset) → Datetime
// ---------------------------------------------------------------------------

temporal_fn!(
    DatetimeFn,
    "datetime",
    Arity::Fixed(3),
    "construct Datetime from Date, Time, Offset"
);

impl ExtensionFunction for DatetimeFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let date = match &args[0] {
            Term::Date(d) => *d,
            other => {
                return Err(EvalError::TypeError(format!(
                    "datetime: expected Date as first arg, got {other:?}"
                )));
            }
        };
        let time_minutes = match &args[1] {
            Term::Time(m) => *m,
            other => {
                return Err(EvalError::TypeError(format!(
                    "datetime: expected Time as second arg, got {other:?}"
                )));
            }
        };
        let offset_minutes = match &args[2] {
            Term::Offset(m) => *m,
            other => {
                return Err(EvalError::TypeError(format!(
                    "datetime: expected Offset as third arg, got {other:?}"
                )));
            }
        };

        let h = (time_minutes / 60) as u32;
        let m = (time_minutes % 60) as u32;
        let offset = chrono::FixedOffset::east_opt(offset_minutes as i32 * 60)
            .ok_or_else(|| EvalError::EvalFailed("Invalid offset".to_string()))?;
        let naive_dt = date
            .and_hms_opt(h, m, 0)
            .ok_or_else(|| EvalError::EvalFailed("Invalid datetime".to_string()))?;
        let dt = offset
            .from_local_datetime(&naive_dt)
            .single()
            .ok_or_else(|| {
                EvalError::EvalFailed("Ambiguous or invalid local datetime".to_string())
            })?;
        Ok(Term::Datetime(dt))
    }
}

// ---------------------------------------------------------------------------
// date-of(Datetime) → Date
// ---------------------------------------------------------------------------

temporal_fn!(
    DateOfFn,
    "date-of",
    Arity::Fixed(1),
    "extract local date from Datetime"
);

impl ExtensionFunction for DateOfFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        match &args[0] {
            Term::Datetime(dt) => Ok(Term::Date(dt.date_naive())),
            other => Err(EvalError::TypeError(format!(
                "date-of: expected Datetime, got {other:?}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// time-of(Datetime) → Time
// ---------------------------------------------------------------------------

temporal_fn!(
    TimeOfFn,
    "time-of",
    Arity::Fixed(1),
    "extract local time from Datetime"
);

impl ExtensionFunction for TimeOfFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        match &args[0] {
            Term::Datetime(dt) => {
                let h = dt.hour() as u16;
                let m = dt.minute() as u16;
                Ok(Term::Time(h * 60 + m))
            }
            other => Err(EvalError::TypeError(format!(
                "time-of: expected Datetime, got {other:?}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// day-of-week(Date) → Symbol
// ---------------------------------------------------------------------------

temporal_fn!(
    DayOfWeekFn,
    "day-of-week",
    Arity::Fixed(1),
    "day of week as symbol (:monday .. :sunday)"
);

impl ExtensionFunction for DayOfWeekFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let date = match &args[0] {
            Term::Date(d) => *d,
            Term::Datetime(dt) => dt.date_naive(),
            other => {
                return Err(EvalError::TypeError(format!(
                    "day-of-week: expected Date or Datetime, got {other:?}"
                )));
            }
        };
        let day_name = match date.weekday() {
            chrono::Weekday::Mon => ":monday",
            chrono::Weekday::Tue => ":tuesday",
            chrono::Weekday::Wed => ":wednesday",
            chrono::Weekday::Thu => ":thursday",
            chrono::Weekday::Fri => ":friday",
            chrono::Weekday::Sat => ":saturday",
            chrono::Weekday::Sun => ":sunday",
        };
        Ok(Term::Symbol(intern(day_name)))
    }
}

// ---------------------------------------------------------------------------
// year-of(Date) → Integer
// ---------------------------------------------------------------------------

temporal_fn!(
    YearOfFn,
    "year-of",
    Arity::Fixed(1),
    "extract year from Date"
);

impl ExtensionFunction for YearOfFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let date = match &args[0] {
            Term::Date(d) => *d,
            Term::Datetime(dt) => dt.date_naive(),
            other => {
                return Err(EvalError::TypeError(format!(
                    "year-of: expected Date, got {other:?}"
                )));
            }
        };
        Ok(Term::Integer(date.year() as i64))
    }
}

// ---------------------------------------------------------------------------
// month-of(Date) → Integer
// ---------------------------------------------------------------------------

temporal_fn!(
    MonthOfFn,
    "month-of",
    Arity::Fixed(1),
    "extract month (1-12) from Date"
);

impl ExtensionFunction for MonthOfFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let date = match &args[0] {
            Term::Date(d) => *d,
            Term::Datetime(dt) => dt.date_naive(),
            other => {
                return Err(EvalError::TypeError(format!(
                    "month-of: expected Date, got {other:?}"
                )));
            }
        };
        Ok(Term::Integer(date.month() as i64))
    }
}

// ---------------------------------------------------------------------------
// day-of-month(Date) → Integer
// ---------------------------------------------------------------------------

temporal_fn!(
    DayOfMonthFn,
    "day-of-month",
    Arity::Fixed(1),
    "extract day of month (1-31) from Date"
);

impl ExtensionFunction for DayOfMonthFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let date = match &args[0] {
            Term::Date(d) => *d,
            Term::Datetime(dt) => dt.date_naive(),
            other => {
                return Err(EvalError::TypeError(format!(
                    "day-of-month: expected Date, got {other:?}"
                )));
            }
        };
        Ok(Term::Integer(date.day() as i64))
    }
}

// ---------------------------------------------------------------------------
// hours-between(Datetime, Datetime) → Decimal
// ---------------------------------------------------------------------------

temporal_fn!(
    HoursBetweenFn,
    "hours-between",
    Arity::Fixed(2),
    "signed hours between two Datetimes (dt2-dt1)"
);

impl ExtensionFunction for HoursBetweenFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let (dt1, dt2) = match (&args[0], &args[1]) {
            (Term::Datetime(a), Term::Datetime(b)) => (a, b),
            _ => {
                return Err(EvalError::TypeError(
                    "hours-between: expected two Datetimes".to_string(),
                ));
            }
        };
        let diff_minutes = (*dt2 - *dt1).num_minutes();
        // Return as Decimal with 2dp: minutes / 60
        let dec = rust_decimal::Decimal::from(diff_minutes) / rust_decimal::Decimal::from(60);
        Ok(Term::Decimal(dec))
    }
}

// ---------------------------------------------------------------------------
// minutes-between(Datetime, Datetime) → Integer
// ---------------------------------------------------------------------------

temporal_fn!(
    MinutesBetweenFn,
    "minutes-between",
    Arity::Fixed(2),
    "signed minutes between two Datetimes (dt2-dt1)"
);

impl ExtensionFunction for MinutesBetweenFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let (dt1, dt2) = match (&args[0], &args[1]) {
            (Term::Datetime(a), Term::Datetime(b)) => (a, b),
            _ => {
                return Err(EvalError::TypeError(
                    "minutes-between: expected two Datetimes".to_string(),
                ));
            }
        };
        Ok(Term::Integer((*dt2 - *dt1).num_minutes()))
    }
}

// ---------------------------------------------------------------------------
// days-between(Date, Date) → Integer
// ---------------------------------------------------------------------------

temporal_fn!(
    DaysBetweenFn,
    "days-between",
    Arity::Fixed(2),
    "signed days between two Dates (date2-date1)"
);

impl ExtensionFunction for DaysBetweenFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let (d1, d2) = match (&args[0], &args[1]) {
            (Term::Date(a), Term::Date(b)) => (a, b),
            _ => {
                return Err(EvalError::TypeError(
                    "days-between: expected two Dates".to_string(),
                ));
            }
        };
        Ok(Term::Integer((*d2 - *d1).num_days()))
    }
}

// ---------------------------------------------------------------------------
// duration-hours(Duration) → Decimal
// ---------------------------------------------------------------------------

temporal_fn!(
    DurationHoursFn,
    "duration-hours",
    Arity::Fixed(1),
    "duration total hours as Decimal"
);

impl ExtensionFunction for DurationHoursFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        match &args[0] {
            Term::Duration(m) => {
                let dec = rust_decimal::Decimal::from(*m) / rust_decimal::Decimal::from(60);
                Ok(Term::Decimal(dec))
            }
            other => Err(EvalError::TypeError(format!(
                "duration-hours: expected Duration, got {other:?}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// duration-minutes(Duration) → Integer
// ---------------------------------------------------------------------------

temporal_fn!(
    DurationMinutesFn,
    "duration-minutes",
    Arity::Fixed(1),
    "duration total minutes as Integer"
);

impl ExtensionFunction for DurationMinutesFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        match &args[0] {
            Term::Duration(m) => Ok(Term::Integer(*m)),
            other => Err(EvalError::TypeError(format!(
                "duration-minutes: expected Duration, got {other:?}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// add-months(Date, Integer) → Date
// ---------------------------------------------------------------------------

temporal_fn!(
    AddMonthsFn,
    "add-months",
    Arity::Fixed(2),
    "add calendar months to a Date (day clamped)"
);

impl ExtensionFunction for AddMonthsFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let date = match &args[0] {
            Term::Date(d) => *d,
            other => {
                return Err(EvalError::TypeError(format!(
                    "add-months: expected Date, got {other:?}"
                )));
            }
        };
        let months = match &args[1] {
            Term::Integer(n) => i32::try_from(*n).map_err(|_| {
                EvalError::TypeError("add-months: months value out of range".to_string())
            })?,
            other => {
                return Err(EvalError::TypeError(format!(
                    "add-months: expected Integer, got {other:?}"
                )));
            }
        };

        let result = add_months_clamped(date, months)
            .ok_or_else(|| EvalError::EvalFailed("add-months: result out of range".to_string()))?;
        Ok(Term::Date(result))
    }
}

// ---------------------------------------------------------------------------
// add-years(Date, Integer) → Date
// ---------------------------------------------------------------------------

temporal_fn!(
    AddYearsFn,
    "add-years",
    Arity::Fixed(2),
    "add calendar years to a Date (day clamped)"
);

impl ExtensionFunction for AddYearsFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let date = match &args[0] {
            Term::Date(d) => *d,
            other => {
                return Err(EvalError::TypeError(format!(
                    "add-years: expected Date, got {other:?}"
                )));
            }
        };
        let years = match &args[1] {
            Term::Integer(n) => i32::try_from(*n).map_err(|_| {
                EvalError::TypeError("add-years: years value out of range".to_string())
            })?,
            other => {
                return Err(EvalError::TypeError(format!(
                    "add-years: expected Integer, got {other:?}"
                )));
            }
        };

        let result = add_months_clamped(date, years * 12)
            .ok_or_else(|| EvalError::EvalFailed("add-years: result out of range".to_string()))?;
        Ok(Term::Date(result))
    }
}

// ---------------------------------------------------------------------------
// months-between(Date, Date) → Integer
// ---------------------------------------------------------------------------

temporal_fn!(
    MonthsBetweenFn,
    "months-between",
    Arity::Fixed(2),
    "complete calendar months between two Dates"
);

impl ExtensionFunction for MonthsBetweenFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let (d1, d2) = match (&args[0], &args[1]) {
            (Term::Date(a), Term::Date(b)) => (*a, *b),
            _ => {
                return Err(EvalError::TypeError(
                    "months-between: expected two Dates".to_string(),
                ));
            }
        };
        Ok(Term::Integer(complete_months_between(d1, d2)))
    }
}

// ---------------------------------------------------------------------------
// years-between(Date, Date) → Integer
// ---------------------------------------------------------------------------

temporal_fn!(
    YearsBetweenFn,
    "years-between",
    Arity::Fixed(2),
    "complete calendar years between two Dates"
);

impl ExtensionFunction for YearsBetweenFn {
    fn signature(&self) -> &FunctionSignature {
        &self.sig
    }

    fn eval(&self, args: &[Term]) -> Result<Term, EvalError> {
        check_arity(&self.sig, args)?;
        let (d1, d2) = match (&args[0], &args[1]) {
            (Term::Date(a), Term::Date(b)) => (*a, *b),
            _ => {
                return Err(EvalError::TypeError(
                    "years-between: expected two Dates".to_string(),
                ));
            }
        };
        Ok(Term::Integer(complete_months_between(d1, d2) / 12))
    }
}

// ---------------------------------------------------------------------------
// Calendar arithmetic helpers
// ---------------------------------------------------------------------------

/// Add `months` calendar months to `date`, clamping the day to the last valid day.
fn add_months_clamped(date: NaiveDate, months: i32) -> Option<NaiveDate> {
    let total_months = date.year() * 12 + date.month() as i32 - 1 + months;
    let new_year = total_months.div_euclid(12);
    let new_month = (total_months.rem_euclid(12) + 1) as u32;

    // Clamp day to last valid day of the target month
    let max_day = days_in_month(new_year, new_month)?;
    let day = date.day().min(max_day);
    NaiveDate::from_ymd_opt(new_year, new_month, day)
}

/// Number of days in a given month.
fn days_in_month(year: i32, month: u32) -> Option<u32> {
    // Get the first day of the next month, then subtract one day
    let next = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1)?
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1)?
    };
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    Some((next - first).num_days() as u32)
}

/// Count complete calendar months between two dates (signed: d2 - d1).
fn complete_months_between(d1: NaiveDate, d2: NaiveDate) -> i64 {
    let month_diff =
        (d2.year() as i64 - d1.year() as i64) * 12 + d2.month() as i64 - d1.month() as i64;

    // If the day of d2 hasn't reached d1's day, subtract one month
    if month_diff > 0 && d2.day() < d1.day() {
        month_diff - 1
    } else if month_diff < 0 && d2.day() > d1.day() {
        month_diff + 1
    } else {
        month_diff
    }
}

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

/// Register all temporal extension functions into the given registry.
pub(crate) fn register_temporal_functions(registry: &mut FunctionRegistry) {
    registry.register(Box::new(DatetimeFn::new()));
    registry.register(Box::new(DateOfFn::new()));
    registry.register(Box::new(TimeOfFn::new()));
    registry.register(Box::new(DayOfWeekFn::new()));
    registry.register(Box::new(YearOfFn::new()));
    registry.register(Box::new(MonthOfFn::new()));
    registry.register(Box::new(DayOfMonthFn::new()));
    registry.register(Box::new(HoursBetweenFn::new()));
    registry.register(Box::new(MinutesBetweenFn::new()));
    registry.register(Box::new(DaysBetweenFn::new()));
    registry.register(Box::new(DurationHoursFn::new()));
    registry.register(Box::new(DurationMinutesFn::new()));
    registry.register(Box::new(AddMonthsFn::new()));
    registry.register(Box::new(AddYearsFn::new()));
    registry.register(Box::new(MonthsBetweenFn::new()));
    registry.register(Box::new(YearsBetweenFn::new()));
}
