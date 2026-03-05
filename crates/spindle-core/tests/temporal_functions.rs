//! Unit tests for temporal extension functions.

use chrono::{FixedOffset, NaiveDate, TimeZone};
use spindle_core::function_registry::{EvalError, FunctionRegistry};
use spindle_core::intern::intern;
use spindle_core::term::Term;

fn eval(name: &str, args: &[Term]) -> Result<Term, EvalError> {
    let reg = FunctionRegistry::with_prelude();
    let func = reg
        .get(intern(name))
        .expect(&format!("function '{}' not registered", name));
    func.eval(args)
}

fn date(y: i32, m: u32, d: u32) -> Term {
    Term::Date(NaiveDate::from_ymd_opt(y, m, d).unwrap())
}

fn time(h: u16, m: u16) -> Term {
    Term::Time(h * 60 + m)
}

fn offset(minutes: i16) -> Term {
    Term::Offset(minutes)
}

fn dur(minutes: i64) -> Term {
    Term::Duration(minutes)
}

fn dt(y: i32, mo: u32, d: u32, h: u32, mi: u32, off_h: i32) -> Term {
    let fo = FixedOffset::east_opt(off_h * 3600).unwrap();
    let naive = NaiveDate::from_ymd_opt(y, mo, d)
        .unwrap()
        .and_hms_opt(h, mi, 0)
        .unwrap();
    Term::Datetime(fo.from_local_datetime(&naive).unwrap())
}

// ===================== datetime construction =====================

#[test]
fn datetime_construction() {
    let result = eval(
        "datetime",
        &[date(2025, 7, 15), time(14, 0), offset(10 * 60)],
    )
    .unwrap();
    let expected = dt(2025, 7, 15, 14, 0, 10);
    assert_eq!(result, expected);
}

#[test]
fn datetime_utc() {
    let result = eval("datetime", &[date(2025, 1, 1), time(0, 0), offset(0)]).unwrap();
    let expected = dt(2025, 1, 1, 0, 0, 0);
    assert_eq!(result, expected);
}

#[test]
fn datetime_negative_offset() {
    let result = eval(
        "datetime",
        &[date(2025, 7, 15), time(9, 30), offset(-5 * 60)],
    )
    .unwrap();
    let expected = dt(2025, 7, 15, 9, 30, -5);
    assert_eq!(result, expected);
}

#[test]
fn datetime_wrong_type() {
    assert!(eval("datetime", &[Term::Integer(1), time(0, 0), offset(0)]).is_err());
}

// ===================== date-of =====================

#[test]
fn date_of_extracts_local_date() {
    let result = eval("date-of", &[dt(2025, 7, 15, 14, 0, 10)]).unwrap();
    assert_eq!(result, date(2025, 7, 15));
}

#[test]
fn date_of_wrong_type() {
    assert!(eval("date-of", &[Term::Integer(1)]).is_err());
}

// ===================== time-of =====================

#[test]
fn time_of_extracts_local_time() {
    let result = eval("time-of", &[dt(2025, 7, 15, 14, 30, 10)]).unwrap();
    assert_eq!(result, time(14, 30));
}

// ===================== day-of-week =====================

#[test]
fn day_of_week_monday() {
    // 2025-07-14 is a Monday
    let result = eval("day-of-week", &[date(2025, 7, 14)]).unwrap();
    assert_eq!(result, Term::Symbol(intern(":monday")));
}

#[test]
fn day_of_week_sunday() {
    // 2025-07-13 is a Sunday
    let result = eval("day-of-week", &[date(2025, 7, 13)]).unwrap();
    assert_eq!(result, Term::Symbol(intern(":sunday")));
}

#[test]
fn day_of_week_from_datetime() {
    let result = eval("day-of-week", &[dt(2025, 7, 14, 10, 0, 0)]).unwrap();
    assert_eq!(result, Term::Symbol(intern(":monday")));
}

// ===================== year-of, month-of, day-of-month =====================

#[test]
fn year_of() {
    assert_eq!(
        eval("year-of", &[date(2025, 7, 15)]).unwrap(),
        Term::Integer(2025)
    );
}

#[test]
fn month_of() {
    assert_eq!(
        eval("month-of", &[date(2025, 7, 15)]).unwrap(),
        Term::Integer(7)
    );
}

#[test]
fn day_of_month() {
    assert_eq!(
        eval("day-of-month", &[date(2025, 7, 15)]).unwrap(),
        Term::Integer(15)
    );
}

// ===================== hours-between =====================

#[test]
fn hours_between_same_offset() {
    let dt1 = dt(2025, 7, 15, 9, 0, 10);
    let dt2 = dt(2025, 7, 15, 17, 30, 10);
    let result = eval("hours-between", &[dt1, dt2]).unwrap();
    // 8.5 hours = 510 minutes / 60
    if let Term::Decimal(d) = result {
        let val: f64 = d.to_string().parse().unwrap();
        assert!((val - 8.5).abs() < 0.001, "expected 8.5, got {d}");
    } else {
        panic!("expected Decimal, got {result:?}");
    }
}

#[test]
fn hours_between_different_offsets() {
    // 9:00 AEST (+10) = 23:00 UTC previous day
    // 9:00 AWST (+8) = 01:00 UTC same day
    // UTC diff: 01:00 - 23:00 = 2 hours
    let dt1 = dt(2025, 7, 15, 9, 0, 10);
    let dt2 = dt(2025, 7, 15, 9, 0, 8);
    let result = eval("hours-between", &[dt1, dt2]).unwrap();
    if let Term::Decimal(d) = result {
        assert_eq!(d.to_string(), "2");
    } else {
        panic!("expected Decimal, got {result:?}");
    }
}

#[test]
fn hours_between_negative() {
    let dt1 = dt(2025, 7, 15, 17, 0, 10);
    let dt2 = dt(2025, 7, 15, 9, 0, 10);
    let result = eval("hours-between", &[dt1, dt2]).unwrap();
    if let Term::Decimal(d) = result {
        let f: f64 = d.to_string().parse().unwrap();
        assert!(f < 0.0, "expected negative, got {f}");
    } else {
        panic!("expected Decimal");
    }
}

// ===================== minutes-between =====================

#[test]
fn minutes_between() {
    let dt1 = dt(2025, 7, 15, 9, 0, 10);
    let dt2 = dt(2025, 7, 15, 9, 45, 10);
    assert_eq!(
        eval("minutes-between", &[dt1, dt2]).unwrap(),
        Term::Integer(45)
    );
}

// ===================== days-between =====================

#[test]
fn days_between() {
    let d1 = date(2025, 7, 1);
    let d2 = date(2025, 7, 15);
    assert_eq!(eval("days-between", &[d1, d2]).unwrap(), Term::Integer(14));
}

#[test]
fn days_between_negative() {
    let d1 = date(2025, 7, 15);
    let d2 = date(2025, 7, 1);
    assert_eq!(eval("days-between", &[d1, d2]).unwrap(), Term::Integer(-14));
}

// ===================== duration-hours / duration-minutes =====================

#[test]
fn duration_hours() {
    let result = eval("duration-hours", &[dur(150)]).unwrap(); // 2.5 hours
    if let Term::Decimal(d) = result {
        let val: f64 = d.to_string().parse().unwrap();
        assert!((val - 2.5).abs() < 0.001, "expected 2.5, got {d}");
    } else {
        panic!("expected Decimal");
    }
}

#[test]
fn duration_minutes() {
    assert_eq!(
        eval("duration-minutes", &[dur(150)]).unwrap(),
        Term::Integer(150)
    );
}

// ===================== add-months =====================

#[test]
fn add_months_normal() {
    let result = eval("add-months", &[date(2025, 1, 15), Term::Integer(3)]).unwrap();
    assert_eq!(result, date(2025, 4, 15));
}

#[test]
fn add_months_clamped() {
    // Jan 31 + 1 month → Feb 28 (non-leap)
    let result = eval("add-months", &[date(2025, 1, 31), Term::Integer(1)]).unwrap();
    assert_eq!(result, date(2025, 2, 28));
}

#[test]
fn add_months_leap_year() {
    // Jan 31, 2024 + 1 month → Feb 29 (leap year)
    let result = eval("add-months", &[date(2024, 1, 31), Term::Integer(1)]).unwrap();
    assert_eq!(result, date(2024, 2, 29));
}

#[test]
fn add_months_negative() {
    let result = eval("add-months", &[date(2025, 3, 15), Term::Integer(-2)]).unwrap();
    assert_eq!(result, date(2025, 1, 15));
}

#[test]
fn add_months_cross_year() {
    let result = eval("add-months", &[date(2025, 11, 15), Term::Integer(3)]).unwrap();
    assert_eq!(result, date(2026, 2, 15));
}

// ===================== add-years =====================

#[test]
fn add_years_normal() {
    let result = eval("add-years", &[date(2025, 7, 15), Term::Integer(1)]).unwrap();
    assert_eq!(result, date(2026, 7, 15));
}

#[test]
fn add_years_leap_day() {
    // Feb 29, 2024 + 1 year → Feb 28, 2025 (clamped)
    let result = eval("add-years", &[date(2024, 2, 29), Term::Integer(1)]).unwrap();
    assert_eq!(result, date(2025, 2, 28));
}

// ===================== months-between =====================

#[test]
fn months_between_complete() {
    let d1 = date(2025, 1, 15);
    let d2 = date(2025, 4, 15);
    assert_eq!(eval("months-between", &[d1, d2]).unwrap(), Term::Integer(3));
}

#[test]
fn months_between_incomplete() {
    // Jan 15 to Apr 14 = 2 complete months (not 3)
    let d1 = date(2025, 1, 15);
    let d2 = date(2025, 4, 14);
    assert_eq!(eval("months-between", &[d1, d2]).unwrap(), Term::Integer(2));
}

#[test]
fn months_between_negative() {
    let d1 = date(2025, 4, 15);
    let d2 = date(2025, 1, 15);
    assert_eq!(
        eval("months-between", &[d1, d2]).unwrap(),
        Term::Integer(-3)
    );
}

// ===================== years-between =====================

#[test]
fn years_between() {
    let d1 = date(2020, 7, 15);
    let d2 = date(2025, 7, 15);
    assert_eq!(eval("years-between", &[d1, d2]).unwrap(), Term::Integer(5));
}

#[test]
fn years_between_incomplete() {
    let d1 = date(2020, 7, 15);
    let d2 = date(2025, 7, 14);
    assert_eq!(eval("years-between", &[d1, d2]).unwrap(), Term::Integer(4));
}

// ===================== Temporal arithmetic builtins =====================

#[test]
fn add_datetime_duration() {
    let result = eval("+", &[dt(2025, 7, 15, 9, 0, 10), dur(90)]).unwrap();
    assert_eq!(result, dt(2025, 7, 15, 10, 30, 10));
}

#[test]
fn add_duration_datetime() {
    // Commutative
    let result = eval("+", &[dur(90), dt(2025, 7, 15, 9, 0, 10)]).unwrap();
    assert_eq!(result, dt(2025, 7, 15, 10, 30, 10));
}

#[test]
fn add_duration_duration() {
    assert_eq!(eval("+", &[dur(60), dur(30)]).unwrap(), dur(90));
}

#[test]
fn sub_datetime_duration() {
    let result = eval("-", &[dt(2025, 7, 15, 10, 30, 10), dur(90)]).unwrap();
    assert_eq!(result, dt(2025, 7, 15, 9, 0, 10));
}

#[test]
fn sub_datetime_datetime() {
    let dt1 = dt(2025, 7, 15, 9, 0, 10);
    let dt2 = dt(2025, 7, 15, 17, 30, 10);
    let result = eval("-", &[dt2, dt1]).unwrap();
    assert_eq!(result, dur(510)); // 8h30m = 510 minutes
}

#[test]
fn sub_date_date() {
    let result = eval("-", &[date(2025, 7, 15), date(2025, 7, 1)]).unwrap();
    assert_eq!(result, Term::Integer(14));
}

#[test]
fn sub_duration_duration() {
    assert_eq!(eval("-", &[dur(90), dur(60)]).unwrap(), dur(30));
}

#[test]
fn mul_duration_number() {
    assert_eq!(eval("*", &[dur(60), Term::Integer(3)]).unwrap(), dur(180));
}

#[test]
fn mul_number_duration() {
    assert_eq!(eval("*", &[Term::Integer(3), dur(60)]).unwrap(), dur(180));
}

#[test]
fn div_duration_number() {
    assert_eq!(eval("/", &[dur(180), Term::Integer(3)]).unwrap(), dur(60));
}

#[test]
fn div_duration_duration() {
    let result = eval("/", &[dur(180), dur(60)]).unwrap();
    if let Term::Decimal(d) = result {
        assert_eq!(d.to_string(), "3");
    } else {
        panic!("expected Decimal");
    }
}

// ===================== Temporal min/max =====================

#[test]
fn min_dates() {
    let result = eval("min", &[date(2025, 7, 15), date(2025, 1, 1)]).unwrap();
    assert_eq!(result, date(2025, 1, 1));
}

#[test]
fn max_dates() {
    let result = eval("max", &[date(2025, 7, 15), date(2025, 1, 1)]).unwrap();
    assert_eq!(result, date(2025, 7, 15));
}

#[test]
fn min_datetimes() {
    let dt1 = dt(2025, 7, 15, 9, 0, 10);
    let dt2 = dt(2025, 7, 15, 17, 0, 10);
    let result = eval("min", &[dt1.clone(), dt2]).unwrap();
    assert_eq!(result, dt1);
}

#[test]
fn max_durations() {
    let result = eval("max", &[dur(60), dur(120), dur(30)]).unwrap();
    assert_eq!(result, dur(120));
}
