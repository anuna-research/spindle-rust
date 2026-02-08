//! Temporal reasoning with Allen interval algebra
//!
//! This module implements Allen's Interval Algebra for temporal reasoning.
//! Allen's 13 interval relations allow precise specification of how
//! time intervals relate to each other:
//!
//! - before, after (disjoint with gap)
//! - meets, met-by (adjacent, no gap)
//! - overlaps, overlapped-by (partial intersection)
//! - starts, started-by (same start point)
//! - during, contains (containment)
//! - finishes, finished-by (same end point)
//! - equals (identical intervals)
//!
//! Reference: Allen, J.F. "Maintaining Knowledge about Temporal Intervals"
//!            Communications of the ACM, 26(11):832-843, 1983.

use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Represents a time point (can be a moment or infinity)
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Copy, Default)]
pub enum TimePoint {
    /// Negative infinity (beginning of time)
    #[default]
    NegInf,
    /// A specific moment in time (milliseconds since epoch)
    Moment(i64),
    /// Positive infinity (end of time)
    PosInf,
}

impl TimePoint {
    /// Create a time point from a numeric value
    pub fn from_millis(millis: i64) -> Self {
        TimePoint::Moment(millis)
    }

    /// Check if this is negative infinity
    pub fn is_neg_inf(&self) -> bool {
        matches!(self, TimePoint::NegInf)
    }

    /// Check if this is positive infinity
    pub fn is_pos_inf(&self) -> bool {
        matches!(self, TimePoint::PosInf)
    }

    /// Check if this is a finite moment
    pub fn is_finite(&self) -> bool {
        matches!(self, TimePoint::Moment(_))
    }
}

impl PartialEq for TimePoint {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (TimePoint::NegInf, TimePoint::NegInf) => true,
            (TimePoint::PosInf, TimePoint::PosInf) => true,
            (TimePoint::Moment(a), TimePoint::Moment(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for TimePoint {}

impl PartialOrd for TimePoint {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TimePoint {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (TimePoint::NegInf, TimePoint::NegInf) => Ordering::Equal,
            (TimePoint::NegInf, _) => Ordering::Less,
            (_, TimePoint::NegInf) => Ordering::Greater,
            (TimePoint::PosInf, TimePoint::PosInf) => Ordering::Equal,
            (TimePoint::PosInf, _) => Ordering::Greater,
            (_, TimePoint::PosInf) => Ordering::Less,
            (TimePoint::Moment(a), TimePoint::Moment(b)) => a.cmp(b),
        }
    }
}

impl Hash for TimePoint {
    fn hash<H: Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        if let TimePoint::Moment(v) = self {
            v.hash(state);
        }
    }
}

// impl Default for TimePoint removed since we derive it now

impl fmt::Display for TimePoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimePoint::NegInf => write!(f, "-inf"),
            TimePoint::PosInf => write!(f, "+inf"),
            TimePoint::Moment(v) => write!(f, "{v}"),
        }
    }
}

impl TimePoint {
    /// Convert a TimePoint to RFC3339 format string.
    ///
    /// Returns `None` for infinite time points.
    /// Returns `Some(rfc3339_string)` for finite moments.
    ///
    /// # Example
    /// ```
    /// use spindle_core::temporal::TimePoint;
    ///
    /// let tp = TimePoint::from_millis(1707220800000); // 2024-02-06T12:00:00Z
    /// assert!(tp.to_rfc3339().unwrap().contains("2024-02-06"));
    /// ```
    pub fn to_rfc3339(&self) -> Option<String> {
        match self {
            TimePoint::Moment(millis) => {
                use chrono::{DateTime, Utc};
                // Use div_euclid/rem_euclid to correctly handle negative timestamps (pre-1970)
                // For example: -1500ms should be -2s + 500ms, not -1s + (-500ms)
                let secs = millis.div_euclid(1000);
                let ms_remainder = millis.rem_euclid(1000) as u32;
                let nsecs = ms_remainder * 1_000_000;
                DateTime::from_timestamp(secs, nsecs)
                    .map(|dt: DateTime<Utc>| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
            }
            TimePoint::NegInf | TimePoint::PosInf => None,
        }
    }
}

/// Temporal interval with start and end time points
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Temporal {
    /// Start time of the interval
    pub start: TimePoint,
    /// End time of the interval
    pub end: TimePoint,
}

/// Empty temporal constant (unbounded interval)
pub const EMPTY_TEMPORAL: Temporal = Temporal {
    start: TimePoint::NegInf,
    end: TimePoint::PosInf,
};

impl Default for Temporal {
    fn default() -> Self {
        EMPTY_TEMPORAL
    }
}

impl Temporal {
    /// Create an empty temporal (unbounded interval from -inf to +inf)
    pub const fn empty() -> Self {
        EMPTY_TEMPORAL
    }

    /// Create a temporal with specific bounds
    ///
    /// If `start > end`, the bounds are automatically swapped.
    pub fn new(start: TimePoint, end: TimePoint) -> Self {
        if start > end {
            Self {
                start: end,
                end: start,
            }
        } else {
            Self { start, end }
        }
    }

    /// Create a temporal from numeric bounds (milliseconds)
    ///
    /// If `start > end`, the bounds are automatically swapped to ensure
    /// a valid interval. This prevents inverted intervals from causing
    /// incorrect temporal reasoning.
    pub fn from_bounds(start: i64, end: i64) -> Self {
        let (lo, hi) = if start > end {
            (end, start)
        } else {
            (start, end)
        };
        Self {
            start: TimePoint::Moment(lo),
            end: TimePoint::Moment(hi),
        }
    }

    /// Check if this is an empty (unbounded) temporal
    pub fn is_empty(&self) -> bool {
        matches!(
            (&self.start, &self.end),
            (TimePoint::NegInf, TimePoint::PosInf)
        )
    }

    /// Check if this temporal has meaningful time information
    pub fn has_info(&self) -> bool {
        !self.is_empty()
    }

    /// Check if this temporal intersects/overlaps with another (weak inequality)
    pub fn intersects(&self, other: &Temporal) -> bool {
        self.start <= other.end && other.start <= self.end
    }

    /// Compute the intersection of two temporals
    pub fn intersection(&self, other: &Temporal) -> Option<Temporal> {
        if !self.intersects(other) {
            return None;
        }

        let start = std::cmp::max(self.start, other.start);
        let end = std::cmp::min(self.end, other.end);

        Some(Temporal { start, end })
    }

    /// Compute the intersection of a list of temporals
    pub fn intersection_list(temporals: &[Temporal]) -> Option<Temporal> {
        if temporals.is_empty() {
            return Some(EMPTY_TEMPORAL);
        }

        let mut result = temporals[0].clone();
        for t in &temporals[1..] {
            result = result.intersection(t)?;
        }
        Some(result)
    }

    // =========================================================================
    // TIME-POINT QUERIES ("NOW" SUPPORT)
    // =========================================================================

    /// Check if a time point falls within this interval [start, end]
    pub fn active_at(&self, time: TimePoint) -> bool {
        self.start <= time && time <= self.end
    }

    /// Check if a time point (as i64) falls within this interval
    pub fn active_at_millis(&self, millis: i64) -> bool {
        self.active_at(TimePoint::Moment(millis))
    }

    /// Check if this interval is entirely in the past relative to a time point
    pub fn past_at(&self, time: TimePoint) -> bool {
        self.end < time
    }

    /// Check if this interval is entirely in the future relative to a time point
    pub fn future_at(&self, time: TimePoint) -> bool {
        self.start > time
    }

    // =========================================================================
    // ALLEN INTERVAL ALGEBRA - 13 RELATIONS
    // =========================================================================

    /// before: X ends before Y starts (with gap)
    /// ```text
    /// X: |-----|
    /// Y:           |-----|
    /// ```
    pub fn before(&self, other: &Temporal) -> bool {
        self.end < other.start
    }

    /// after: X starts after Y ends (inverse of before)
    /// ```text
    /// X:           |-----|
    /// Y: |-----|
    /// ```
    pub fn after(&self, other: &Temporal) -> bool {
        other.end < self.start
    }

    /// meets: X ends exactly when Y starts
    /// ```text
    /// X: |-----|
    /// Y:       |-----|
    /// ```
    pub fn meets(&self, other: &Temporal) -> bool {
        self.end == other.start
    }

    /// met_by: X starts exactly when Y ends (inverse of meets)
    /// ```text
    /// X:       |-----|
    /// Y: |-----|
    /// ```
    pub fn met_by(&self, other: &Temporal) -> bool {
        self.start == other.end
    }

    /// overlaps: X starts before Y, ends during Y
    /// ```text
    /// X: |-----|
    /// Y:     |-----|
    /// ```
    pub fn overlaps(&self, other: &Temporal) -> bool {
        self.start < other.start && other.start < self.end && self.end < other.end
    }

    /// overlapped_by: Y starts before X, ends during X (inverse of overlaps)
    /// ```text
    /// X:     |-----|
    /// Y: |-----|
    /// ```
    pub fn overlapped_by(&self, other: &Temporal) -> bool {
        other.overlaps(self)
    }

    /// starts: X and Y start together, X ends before Y
    /// ```text
    /// X: |---|
    /// Y: |-------|
    /// ```
    pub fn starts(&self, other: &Temporal) -> bool {
        self.start == other.start && self.end < other.end
    }

    /// started_by: X and Y start together, Y ends before X (inverse of starts)
    /// ```text
    /// X: |-------|
    /// Y: |---|
    /// ```
    pub fn started_by(&self, other: &Temporal) -> bool {
        other.starts(self)
    }

    /// during: X is completely contained within Y
    /// ```text
    /// X:   |---|
    /// Y: |-------|
    /// ```
    pub fn during(&self, other: &Temporal) -> bool {
        other.start < self.start && self.end < other.end
    }

    /// contains: Y is completely contained within X (inverse of during)
    /// ```text
    /// X: |-------|
    /// Y:   |---|
    /// ```
    pub fn contains(&self, other: &Temporal) -> bool {
        other.during(self)
    }

    /// finishes: X starts after Y but both end together
    /// ```text
    /// X:     |---|
    /// Y: |-------|
    /// ```
    pub fn finishes(&self, other: &Temporal) -> bool {
        other.start < self.start && self.end == other.end
    }

    /// finished_by: Y starts after X but both end together (inverse of finishes)
    /// ```text
    /// X: |-------|
    /// Y:     |---|
    /// ```
    pub fn finished_by(&self, other: &Temporal) -> bool {
        other.finishes(self)
    }

    /// equals: X and Y have identical start and end times
    /// ```text
    /// X: |-------|
    /// Y: |-------|
    /// ```
    pub fn equals(&self, other: &Temporal) -> bool {
        self.start == other.start && self.end == other.end
    }

    /// Determine which Allen relation holds between two intervals
    pub fn relation(&self, other: &Temporal) -> AllenRelation {
        if self.before(other) {
            AllenRelation::Before
        } else if self.after(other) {
            AllenRelation::After
        } else if self.meets(other) {
            AllenRelation::Meets
        } else if self.met_by(other) {
            AllenRelation::MetBy
        } else if self.overlaps(other) {
            AllenRelation::Overlaps
        } else if self.overlapped_by(other) {
            AllenRelation::OverlappedBy
        } else if self.starts(other) {
            AllenRelation::Starts
        } else if self.started_by(other) {
            AllenRelation::StartedBy
        } else if self.during(other) {
            AllenRelation::During
        } else if self.contains(other) {
            AllenRelation::Contains
        } else if self.finishes(other) {
            AllenRelation::Finishes
        } else if self.finished_by(other) {
            AllenRelation::FinishedBy
        } else {
            AllenRelation::Equals
        }
    }
}

impl fmt::Display for Temporal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            Ok(())
        } else {
            write!(f, "[{},{}]", self.start, self.end)
        }
    }
}

/// Allen interval relations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AllenRelation {
    /// t1 ends before t2 starts
    Before,
    /// t1 starts after t2 ends
    After,
    /// t1 ends exactly when t2 starts
    Meets,
    /// t1 starts exactly when t2 ends
    MetBy,
    /// t1 starts before t2, ends during t2
    Overlaps,
    /// t2 starts before t1, ends during t1
    OverlappedBy,
    /// t1 strictly contains t2
    Contains,
    /// t1 is strictly within t2
    During,
    /// t1 and t2 start together, t1 ends first
    Starts,
    /// t1 and t2 start together, t2 ends first
    StartedBy,
    /// t1 starts after t2, both end together
    Finishes,
    /// t2 starts after t1, both end together
    FinishedBy,
    /// t1 and t2 have same start and end
    Equals,
}

impl AllenRelation {
    /// Get the inverse relation
    pub fn inverse(&self) -> Self {
        match self {
            AllenRelation::Before => AllenRelation::After,
            AllenRelation::After => AllenRelation::Before,
            AllenRelation::Meets => AllenRelation::MetBy,
            AllenRelation::MetBy => AllenRelation::Meets,
            AllenRelation::Overlaps => AllenRelation::OverlappedBy,
            AllenRelation::OverlappedBy => AllenRelation::Overlaps,
            AllenRelation::Contains => AllenRelation::During,
            AllenRelation::During => AllenRelation::Contains,
            AllenRelation::Starts => AllenRelation::StartedBy,
            AllenRelation::StartedBy => AllenRelation::Starts,
            AllenRelation::Finishes => AllenRelation::FinishedBy,
            AllenRelation::FinishedBy => AllenRelation::Finishes,
            AllenRelation::Equals => AllenRelation::Equals,
        }
    }
}

impl fmt::Display for AllenRelation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            AllenRelation::Before => "before",
            AllenRelation::After => "after",
            AllenRelation::Meets => "meets",
            AllenRelation::MetBy => "met-by",
            AllenRelation::Overlaps => "overlaps",
            AllenRelation::OverlappedBy => "overlapped-by",
            AllenRelation::Contains => "contains",
            AllenRelation::During => "during",
            AllenRelation::Starts => "starts",
            AllenRelation::StartedBy => "started-by",
            AllenRelation::Finishes => "finishes",
            AllenRelation::FinishedBy => "finished-by",
            AllenRelation::Equals => "equals",
        };
        write!(f, "{s}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_temporal() {
        let t = Temporal::empty();
        assert!(t.is_empty());
        assert!(!t.has_info()); // Correct: empty/unbounded means no specific constraint info
    }

    #[test]
    fn test_from_bounds() {
        let t = Temporal::from_bounds(5, 10);
        assert!(t.has_info());
        assert_eq!(t.start, TimePoint::Moment(5));
        assert_eq!(t.end, TimePoint::Moment(10));
    }

    #[test]
    fn test_intersection() {
        let t1 = Temporal::from_bounds(0, 10);
        let t2 = Temporal::from_bounds(5, 15);
        let intersection = t1.intersection(&t2).unwrap();
        assert_eq!(intersection, Temporal::from_bounds(5, 10));
    }

    #[test]
    fn test_no_intersection() {
        let t1 = Temporal::from_bounds(0, 5);
        let t2 = Temporal::from_bounds(10, 15);
        assert!(t1.intersection(&t2).is_none());
    }

    #[test]
    fn test_allen_before() {
        let t1 = Temporal::from_bounds(0, 5);
        let t2 = Temporal::from_bounds(10, 15);
        assert!(t1.before(&t2));
        assert!(t2.after(&t1));
        assert_eq!(t1.relation(&t2), AllenRelation::Before);
    }

    #[test]
    fn test_allen_meets() {
        let t1 = Temporal::from_bounds(0, 5);
        let t2 = Temporal::from_bounds(5, 10);
        assert!(t1.meets(&t2));
        assert!(t2.met_by(&t1));
        assert_eq!(t1.relation(&t2), AllenRelation::Meets);
    }

    #[test]
    fn test_allen_overlaps() {
        let t1 = Temporal::from_bounds(0, 7);
        let t2 = Temporal::from_bounds(5, 15);
        assert!(t1.overlaps(&t2));
        assert!(t2.overlapped_by(&t1));
        assert_eq!(t1.relation(&t2), AllenRelation::Overlaps);
    }

    #[test]
    fn test_allen_during() {
        let t1 = Temporal::from_bounds(5, 10);
        let t2 = Temporal::from_bounds(0, 15);
        assert!(t1.during(&t2));
        assert!(t2.contains(&t1));
        assert_eq!(t1.relation(&t2), AllenRelation::During);
    }

    #[test]
    fn test_allen_starts() {
        let t1 = Temporal::from_bounds(0, 5);
        let t2 = Temporal::from_bounds(0, 10);
        assert!(t1.starts(&t2));
        assert!(t2.started_by(&t1));
        assert_eq!(t1.relation(&t2), AllenRelation::Starts);
    }

    #[test]
    fn test_allen_finishes() {
        let t1 = Temporal::from_bounds(5, 10);
        let t2 = Temporal::from_bounds(0, 10);
        assert!(t1.finishes(&t2));
        assert!(t2.finished_by(&t1));
        assert_eq!(t1.relation(&t2), AllenRelation::Finishes);
    }

    #[test]
    fn test_allen_equals() {
        let t1 = Temporal::from_bounds(0, 10);
        let t2 = Temporal::from_bounds(0, 10);
        assert!(t1.equals(&t2));
        assert_eq!(t1.relation(&t2), AllenRelation::Equals);
    }

    #[test]
    fn test_active_at() {
        let t = Temporal::from_bounds(5, 15);
        assert!(!t.active_at_millis(0));
        assert!(t.active_at_millis(5));
        assert!(t.active_at_millis(10));
        assert!(t.active_at_millis(15));
        assert!(!t.active_at_millis(20));
    }

    #[test]
    fn test_past_future_at() {
        let t = Temporal::from_bounds(5, 10);
        assert!(t.future_at(TimePoint::Moment(0)));
        assert!(!t.future_at(TimePoint::Moment(5)));
        assert!(t.past_at(TimePoint::Moment(15)));
        assert!(!t.past_at(TimePoint::Moment(10)));
    }

    #[test]
    fn test_relation_inverse() {
        assert_eq!(AllenRelation::Before.inverse(), AllenRelation::After);
        assert_eq!(AllenRelation::Meets.inverse(), AllenRelation::MetBy);
        assert_eq!(AllenRelation::Equals.inverse(), AllenRelation::Equals);
    }

    // =========================================================================
    // ALLEN INTERVAL ALGEBRA - COMPREHENSIVE TESTS FOR ALL 13 RELATIONS
    // Ported from spindle-racket/tests/temporal-allen-tests.rkt
    // =========================================================================

    /// Test intervals used throughout Allen relation tests:
    /// A = [0, 10]
    /// B = [15, 25]  (after A with gap)
    /// C = [10, 20]  (meets A)
    /// D = [5, 15]   (overlaps A)
    /// E = [0, 5]    (starts A)
    /// F = [2, 10]   (finishes A)
    /// G = [3, 7]    (during A)
    /// H = [0, 10]   (equals A)

    #[test]
    fn test_allen_before_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let b = Temporal::from_bounds(15, 25);

        assert!(a.before(&b), "A should be before B");
        assert!(!b.before(&a), "B should not be before A");
        assert!(!a.before(&a), "A should not be before itself");
        assert_eq!(a.relation(&b), AllenRelation::Before);
    }

    #[test]
    fn test_allen_after_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let b = Temporal::from_bounds(15, 25);

        assert!(b.after(&a), "B should be after A");
        assert!(!a.after(&b), "A should not be after B");
        assert!(!a.after(&a), "A should not be after itself");
        assert_eq!(b.relation(&a), AllenRelation::After);
    }

    #[test]
    fn test_allen_meets_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let c = Temporal::from_bounds(10, 20);

        assert!(a.meets(&c), "A should meet C");
        assert!(!c.meets(&a), "C should not meet A");
        assert!(!a.meets(&a), "A should not meet itself");
        assert_eq!(a.relation(&c), AllenRelation::Meets);
    }

    #[test]
    fn test_allen_met_by_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let c = Temporal::from_bounds(10, 20);

        assert!(c.met_by(&a), "C should be met by A");
        assert!(!a.met_by(&c), "A should not be met by C");
        assert_eq!(c.relation(&a), AllenRelation::MetBy);
    }

    #[test]
    fn test_allen_overlaps_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let d = Temporal::from_bounds(5, 15);

        assert!(a.overlaps(&d), "A should overlap D");
        assert!(
            !d.overlaps(&a),
            "D should not overlap A (it's overlapped-by)"
        );
        assert_eq!(a.relation(&d), AllenRelation::Overlaps);
    }

    #[test]
    fn test_allen_overlapped_by_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let d = Temporal::from_bounds(5, 15);

        assert!(d.overlapped_by(&a), "D should be overlapped by A");
        assert!(!a.overlapped_by(&d), "A should not be overlapped by D");
        assert_eq!(d.relation(&a), AllenRelation::OverlappedBy);
    }

    #[test]
    fn test_allen_starts_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let e = Temporal::from_bounds(0, 5);

        assert!(e.starts(&a), "E should start A");
        assert!(!a.starts(&e), "A should not start E");
        assert_eq!(e.relation(&a), AllenRelation::Starts);
    }

    #[test]
    fn test_allen_started_by_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let e = Temporal::from_bounds(0, 5);

        assert!(a.started_by(&e), "A should be started by E");
        assert!(!e.started_by(&a), "E should not be started by A");
        assert_eq!(a.relation(&e), AllenRelation::StartedBy);
    }

    #[test]
    fn test_allen_during_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let g = Temporal::from_bounds(3, 7);

        assert!(g.during(&a), "G should be during A");
        assert!(!a.during(&g), "A should not be during G");
        assert_eq!(g.relation(&a), AllenRelation::During);
    }

    #[test]
    fn test_allen_contains_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let g = Temporal::from_bounds(3, 7);

        assert!(a.contains(&g), "A should contain G");
        assert!(!g.contains(&a), "G should not contain A");
        assert_eq!(a.relation(&g), AllenRelation::Contains);
    }

    #[test]
    fn test_allen_finishes_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let f = Temporal::from_bounds(2, 10);

        assert!(f.finishes(&a), "F should finish A");
        assert!(!a.finishes(&f), "A should not finish F");
        assert_eq!(f.relation(&a), AllenRelation::Finishes);
    }

    #[test]
    fn test_allen_finished_by_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let f = Temporal::from_bounds(2, 10);

        assert!(a.finished_by(&f), "A should be finished by F");
        assert!(!f.finished_by(&a), "F should not be finished by A");
        assert_eq!(a.relation(&f), AllenRelation::FinishedBy);
    }

    #[test]
    fn test_allen_equals_comprehensive() {
        let a = Temporal::from_bounds(0, 10);
        let h = Temporal::from_bounds(0, 10);

        assert!(a.equals(&h), "A should equal H");
        assert!(h.equals(&a), "H should equal A");
        assert!(a.equals(&a), "A should equal itself");
        assert_eq!(a.relation(&h), AllenRelation::Equals);
    }

    #[test]
    fn test_allen_relation_returns_correct_symbol() {
        let a = Temporal::from_bounds(0, 10);
        let b = Temporal::from_bounds(15, 25);
        let c = Temporal::from_bounds(10, 20);
        let d = Temporal::from_bounds(5, 15);
        let e = Temporal::from_bounds(0, 5);
        let f = Temporal::from_bounds(2, 10);
        let g = Temporal::from_bounds(3, 7);
        let h = Temporal::from_bounds(0, 10);

        assert_eq!(a.relation(&b), AllenRelation::Before);
        assert_eq!(b.relation(&a), AllenRelation::After);
        assert_eq!(a.relation(&c), AllenRelation::Meets);
        assert_eq!(c.relation(&a), AllenRelation::MetBy);
        assert_eq!(a.relation(&d), AllenRelation::Overlaps);
        assert_eq!(d.relation(&a), AllenRelation::OverlappedBy);
        assert_eq!(e.relation(&a), AllenRelation::Starts);
        assert_eq!(a.relation(&e), AllenRelation::StartedBy);
        assert_eq!(g.relation(&a), AllenRelation::During);
        assert_eq!(a.relation(&g), AllenRelation::Contains);
        assert_eq!(f.relation(&a), AllenRelation::Finishes);
        assert_eq!(a.relation(&f), AllenRelation::FinishedBy);
        assert_eq!(a.relation(&h), AllenRelation::Equals);
    }

    #[test]
    fn test_allen_mutual_exclusivity() {
        // Exactly one Allen relation should hold between any two intervals
        let a = Temporal::from_bounds(0, 10);
        let b = Temporal::from_bounds(5, 15);

        let relations = vec![
            a.before(&b),
            a.after(&b),
            a.meets(&b),
            a.met_by(&b),
            a.overlaps(&b),
            a.overlapped_by(&b),
            a.starts(&b),
            a.started_by(&b),
            a.during(&b),
            a.contains(&b),
            a.finishes(&b),
            a.finished_by(&b),
            a.equals(&b),
        ];

        let true_count = relations.iter().filter(|&&r| r).count();
        assert_eq!(
            true_count, 1,
            "Exactly one Allen relation should hold, got {}",
            true_count
        );
    }

    #[test]
    fn test_allen_all_relation_inverses() {
        // Test all inverse pairs
        assert_eq!(AllenRelation::Before.inverse(), AllenRelation::After);
        assert_eq!(AllenRelation::After.inverse(), AllenRelation::Before);
        assert_eq!(AllenRelation::Meets.inverse(), AllenRelation::MetBy);
        assert_eq!(AllenRelation::MetBy.inverse(), AllenRelation::Meets);
        assert_eq!(
            AllenRelation::Overlaps.inverse(),
            AllenRelation::OverlappedBy
        );
        assert_eq!(
            AllenRelation::OverlappedBy.inverse(),
            AllenRelation::Overlaps
        );
        assert_eq!(AllenRelation::Starts.inverse(), AllenRelation::StartedBy);
        assert_eq!(AllenRelation::StartedBy.inverse(), AllenRelation::Starts);
        assert_eq!(AllenRelation::During.inverse(), AllenRelation::Contains);
        assert_eq!(AllenRelation::Contains.inverse(), AllenRelation::During);
        assert_eq!(AllenRelation::Finishes.inverse(), AllenRelation::FinishedBy);
        assert_eq!(AllenRelation::FinishedBy.inverse(), AllenRelation::Finishes);
        assert_eq!(AllenRelation::Equals.inverse(), AllenRelation::Equals);
    }

    // =========================================================================
    // POINT INTERVALS (EDGE CASES)
    // =========================================================================

    #[test]
    fn test_point_intervals() {
        let p1 = Temporal::from_bounds(5, 5);
        let p2 = Temporal::from_bounds(5, 5);
        let p3 = Temporal::from_bounds(10, 10);
        let p4 = Temporal::from_bounds(5, 10); // interval that P1 meets

        assert!(p1.equals(&p2), "Equal point intervals should be equal");
        assert!(p1.before(&p3), "[5,5] should be before [10,10]");
        assert!(p1.meets(&p4), "[5,5] should meet [5,10]");
    }

    #[test]
    fn test_infinite_intervals() {
        let inf = Temporal::new(TimePoint::NegInf, TimePoint::PosInf);
        let a = Temporal::from_bounds(0, 10);

        assert!(
            a.during(&inf),
            "Finite interval should be during infinite interval"
        );
        assert!(
            inf.contains(&a),
            "Infinite interval should contain finite interval"
        );
    }

    // =========================================================================
    // TEMPORAL STATE TRACKING - ACTIVE_AT, PAST_AT, FUTURE_AT
    // Ported from spindle-racket/tests/temporal-allen-tests.rkt
    // =========================================================================

    #[test]
    fn test_active_at_basic() {
        let t = Temporal::from_bounds(0, 10);

        assert!(t.active_at_millis(5), "5 is within [0,10]");
        assert!(t.active_at_millis(0), "0 is at start boundary");
        assert!(t.active_at_millis(10), "10 is at end boundary");
        assert!(!t.active_at_millis(15), "15 is after interval");
        assert!(!t.active_at_millis(-5), "-5 is before interval");
    }

    #[test]
    fn test_active_at_with_timepoint() {
        let t = Temporal::from_bounds(5, 15);

        assert!(t.active_at(TimePoint::Moment(10)), "10 is within interval");
        assert!(t.active_at(TimePoint::Moment(5)), "5 is at start boundary");
        assert!(t.active_at(TimePoint::Moment(15)), "15 is at end boundary");
        assert!(!t.active_at(TimePoint::Moment(0)), "0 is before interval");
        assert!(!t.active_at(TimePoint::Moment(20)), "20 is after interval");
    }

    #[test]
    fn test_active_at_with_infinite_interval() {
        let inf = Temporal::new(TimePoint::NegInf, TimePoint::PosInf);

        assert!(inf.active_at_millis(0), "0 is within infinite interval");
        assert!(
            inf.active_at_millis(1000000),
            "Any time is within infinite interval"
        );
        assert!(
            inf.active_at_millis(-1000000),
            "Negative time is within infinite interval"
        );
    }

    #[test]
    fn test_past_at_basic() {
        let t = Temporal::from_bounds(5, 10);

        assert!(t.past_at(TimePoint::Moment(15)), "interval ended before 15");
        assert!(!t.past_at(TimePoint::Moment(8)), "8 is during interval");
        assert!(
            !t.past_at(TimePoint::Moment(3)),
            "3 is before interval started"
        );
        assert!(
            !t.past_at(TimePoint::Moment(10)),
            "10 is at boundary, not past"
        );
    }

    #[test]
    fn test_future_at_basic() {
        let t = Temporal::from_bounds(5, 10);

        assert!(t.future_at(TimePoint::Moment(3)), "interval starts after 3");
        assert!(!t.future_at(TimePoint::Moment(8)), "8 is during interval");
        assert!(!t.future_at(TimePoint::Moment(15)), "15 is after interval");
        assert!(
            !t.future_at(TimePoint::Moment(5)),
            "5 is at boundary, not future"
        );
    }

    #[test]
    fn test_past_future_comprehensive() {
        let t = Temporal::from_bounds(10, 20);

        // Past tests
        assert!(t.past_at(TimePoint::Moment(25)), "interval is past at 25");
        assert!(t.past_at(TimePoint::Moment(21)), "interval is past at 21");
        assert!(!t.past_at(TimePoint::Moment(20)), "boundary is not past");
        assert!(!t.past_at(TimePoint::Moment(15)), "middle is not past");
        assert!(!t.past_at(TimePoint::Moment(5)), "before start is not past");

        // Future tests
        assert!(t.future_at(TimePoint::Moment(5)), "interval is future at 5");
        assert!(t.future_at(TimePoint::Moment(9)), "interval is future at 9");
        assert!(
            !t.future_at(TimePoint::Moment(10)),
            "boundary is not future"
        );
        assert!(!t.future_at(TimePoint::Moment(15)), "middle is not future");
        assert!(
            !t.future_at(TimePoint::Moment(25)),
            "after end is not future"
        );
    }

    // =========================================================================
    // MOMENT/INSTANT SYNTAX - TIMEPOINT TESTS
    // Ported from spindle-racket/tests/moment-temporal-tests.rkt
    // =========================================================================

    #[test]
    fn test_timepoint_from_millis() {
        let tp = TimePoint::from_millis(1000);
        assert!(tp.is_finite());
        assert!(!tp.is_neg_inf());
        assert!(!tp.is_pos_inf());

        if let TimePoint::Moment(v) = tp {
            assert_eq!(v, 1000);
        } else {
            panic!("Expected Moment variant");
        }
    }

    #[test]
    fn test_timepoint_infinity() {
        let neg = TimePoint::NegInf;
        let pos = TimePoint::PosInf;

        assert!(neg.is_neg_inf());
        assert!(!neg.is_pos_inf());
        assert!(!neg.is_finite());

        assert!(pos.is_pos_inf());
        assert!(!pos.is_neg_inf());
        assert!(!pos.is_finite());
    }

    #[test]
    fn test_timepoint_ordering() {
        let neg_inf = TimePoint::NegInf;
        let pos_inf = TimePoint::PosInf;
        let m1 = TimePoint::Moment(0);
        let m2 = TimePoint::Moment(100);
        let m3 = TimePoint::Moment(-100);

        // NegInf is less than everything else
        assert!(neg_inf < m3);
        assert!(neg_inf < m1);
        assert!(neg_inf < m2);
        assert!(neg_inf < pos_inf);

        // PosInf is greater than everything else
        assert!(pos_inf > m3);
        assert!(pos_inf > m1);
        assert!(pos_inf > m2);
        assert!(pos_inf > neg_inf);

        // Moments compare by value
        assert!(m3 < m1);
        assert!(m1 < m2);
        assert!(m3 < m2);
    }

    #[test]
    fn test_timepoint_equality() {
        assert_eq!(TimePoint::NegInf, TimePoint::NegInf);
        assert_eq!(TimePoint::PosInf, TimePoint::PosInf);
        assert_eq!(TimePoint::Moment(42), TimePoint::Moment(42));

        assert_ne!(TimePoint::NegInf, TimePoint::PosInf);
        assert_ne!(TimePoint::Moment(1), TimePoint::Moment(2));
        assert_ne!(TimePoint::Moment(0), TimePoint::NegInf);
    }

    #[test]
    fn test_timepoint_display() {
        assert_eq!(format!("{}", TimePoint::NegInf), "-inf");
        assert_eq!(format!("{}", TimePoint::PosInf), "+inf");
        assert_eq!(format!("{}", TimePoint::Moment(42)), "42");
        assert_eq!(format!("{}", TimePoint::Moment(-100)), "-100");
    }

    #[test]
    fn test_timepoint_to_rfc3339_post_1970() {
        // 2024-02-06T12:00:00.000Z = 1707220800000 ms
        let tp = TimePoint::from_millis(1707220800000);
        let rfc = tp.to_rfc3339().unwrap();
        assert!(rfc.starts_with("2024-02-06T12:00:00"));
        assert!(rfc.ends_with("Z"));
    }

    #[test]
    fn test_timepoint_to_rfc3339_with_millis() {
        // 2024-02-06T12:00:00.500Z
        let tp = TimePoint::from_millis(1707220800500);
        let rfc = tp.to_rfc3339().unwrap();
        assert!(rfc.contains(".500"), "Should include milliseconds: {rfc}");
    }

    #[test]
    fn test_timepoint_to_rfc3339_pre_1970() {
        // 1969-12-31T23:59:59.500Z = -500 ms (half second before epoch)
        let tp = TimePoint::from_millis(-500);
        let rfc = tp.to_rfc3339().unwrap();
        assert!(
            rfc.starts_with("1969-12-31T23:59:59"),
            "Pre-1970 timestamp should work: {rfc}"
        );
        assert!(
            rfc.contains(".500"),
            "Should have correct milliseconds: {rfc}"
        );
    }

    #[test]
    fn test_timepoint_to_rfc3339_pre_1970_full_second() {
        // 1969-12-31T23:59:58.000Z = -2000 ms
        let tp = TimePoint::from_millis(-2000);
        let rfc = tp.to_rfc3339().unwrap();
        assert!(
            rfc.starts_with("1969-12-31T23:59:58"),
            "Pre-1970 full second: {rfc}"
        );
    }

    #[test]
    fn test_timepoint_to_rfc3339_infinity_returns_none() {
        assert!(TimePoint::NegInf.to_rfc3339().is_none());
        assert!(TimePoint::PosInf.to_rfc3339().is_none());
    }

    #[test]
    fn test_timepoint_default() {
        let tp: TimePoint = Default::default();
        assert!(tp.is_neg_inf());
    }

    #[test]
    fn test_timepoint_hash() {
        use std::collections::HashSet;

        let mut set = HashSet::new();
        set.insert(TimePoint::Moment(1));
        set.insert(TimePoint::Moment(2));
        set.insert(TimePoint::NegInf);
        set.insert(TimePoint::PosInf);

        assert!(set.contains(&TimePoint::Moment(1)));
        assert!(set.contains(&TimePoint::Moment(2)));
        assert!(set.contains(&TimePoint::NegInf));
        assert!(set.contains(&TimePoint::PosInf));
        assert!(!set.contains(&TimePoint::Moment(3)));
    }

    // =========================================================================
    // TEMPORAL WINDOW PROPAGATION - INTERSECTION TESTS
    // Ported from spindle-racket/tests/spindle-temporal-tests.rkt
    // =========================================================================

    #[test]
    fn test_intersection_overlapping() {
        let t1 = Temporal::from_bounds(0, 20);
        let t2 = Temporal::from_bounds(10, 30);
        let intersection = t1.intersection(&t2).unwrap();

        assert_eq!(intersection.start, TimePoint::Moment(10));
        assert_eq!(intersection.end, TimePoint::Moment(20));
    }

    #[test]
    fn test_intersection_three_way() {
        let t1 = Temporal::from_bounds(0, 100);
        let t2 = Temporal::from_bounds(20, 80);
        let t3 = Temporal::from_bounds(40, 60);

        let intersection = Temporal::intersection_list(&[t1, t2, t3]).unwrap();

        assert_eq!(intersection.start, TimePoint::Moment(40));
        assert_eq!(intersection.end, TimePoint::Moment(60));
    }

    #[test]
    fn test_intersection_adjacent_intervals() {
        let t1 = Temporal::from_bounds(0, 10);
        let t2 = Temporal::from_bounds(10, 20);

        // With weak overlap semantics, [0,10] and [10,20] overlap at point 10
        let intersection = t1.intersection(&t2).unwrap();
        assert_eq!(intersection.start, TimePoint::Moment(10));
        assert_eq!(intersection.end, TimePoint::Moment(10));
    }

    #[test]
    fn test_intersection_universal_with_finite() {
        let t_universal = Temporal::new(TimePoint::NegInf, TimePoint::PosInf);
        let t_finite = Temporal::from_bounds(10, 20);

        let intersection = t_universal.intersection(&t_finite).unwrap();
        assert_eq!(intersection.start, TimePoint::Moment(10));
        assert_eq!(intersection.end, TimePoint::Moment(20));
    }

    #[test]
    fn test_intersection_list_empty() {
        let result = Temporal::intersection_list(&[]).unwrap();
        assert!(result.is_empty(), "Empty list should return EMPTY_TEMPORAL");
    }

    #[test]
    fn test_intersection_list_single() {
        let t = Temporal::from_bounds(5, 15);
        let result = Temporal::intersection_list(std::slice::from_ref(&t)).unwrap();

        assert_eq!(result.start, TimePoint::Moment(5));
        assert_eq!(result.end, TimePoint::Moment(15));
    }

    #[test]
    fn test_intersection_list_non_overlapping() {
        let t1 = Temporal::from_bounds(0, 10);
        let t2 = Temporal::from_bounds(20, 30);

        let result = Temporal::intersection_list(&[t1, t2]);
        assert!(
            result.is_none(),
            "Non-overlapping intervals should return None"
        );
    }

    #[test]
    fn test_intersection_list_chain_failure() {
        let t1 = Temporal::from_bounds(0, 10);
        let t2 = Temporal::from_bounds(5, 15);
        let t3 = Temporal::from_bounds(20, 30);

        let result = Temporal::intersection_list(&[t1, t2, t3]);
        assert!(
            result.is_none(),
            "Should fail when third interval doesn't overlap"
        );
    }

    #[test]
    fn test_intersects_basic() {
        let t1 = Temporal::from_bounds(0, 10);
        let t2 = Temporal::from_bounds(5, 15);
        let t3 = Temporal::from_bounds(20, 30);

        assert!(t1.intersects(&t2), "Overlapping intervals should intersect");
        assert!(
            !t1.intersects(&t3),
            "Disjoint intervals should not intersect"
        );
    }

    #[test]
    fn test_intersects_touching_boundaries() {
        let t1 = Temporal::from_bounds(0, 10);
        let t2 = Temporal::from_bounds(10, 20);

        assert!(
            t1.intersects(&t2),
            "Touching boundaries should intersect (weak inequality)"
        );
    }

    // =========================================================================
    // TEMPORAL EDGE CASES
    // =========================================================================

    #[test]
    fn test_half_infinite_temporals() {
        let t1 = Temporal::new(TimePoint::NegInf, TimePoint::Moment(15));
        let t2 = Temporal::new(TimePoint::Moment(10), TimePoint::PosInf);

        let intersection = t1.intersection(&t2).unwrap();
        assert_eq!(intersection.start, TimePoint::Moment(10));
        assert_eq!(intersection.end, TimePoint::Moment(15));
    }

    #[test]
    fn test_negative_time_values() {
        let t1 = Temporal::from_bounds(-100, -50);
        let t2 = Temporal::from_bounds(-75, -25);

        let intersection = t1.intersection(&t2).unwrap();
        assert_eq!(intersection.start, TimePoint::Moment(-75));
        assert_eq!(intersection.end, TimePoint::Moment(-50));
    }

    #[test]
    fn test_large_time_values() {
        let t1 = Temporal::from_bounds(1_000_000, 2_000_000);
        let t2 = Temporal::from_bounds(1_500_000, 2_500_000);

        let intersection = t1.intersection(&t2).unwrap();
        assert_eq!(intersection.start, TimePoint::Moment(1_500_000));
        assert_eq!(intersection.end, TimePoint::Moment(2_000_000));
    }

    #[test]
    fn test_point_interval_overlap() {
        let point = Temporal::from_bounds(5, 5);
        let interval = Temporal::from_bounds(0, 10);

        assert!(
            point.intersects(&interval),
            "Point should intersect with interval containing it"
        );

        let intersection = point.intersection(&interval).unwrap();
        assert_eq!(intersection.start, TimePoint::Moment(5));
        assert_eq!(intersection.end, TimePoint::Moment(5));
    }

    #[test]
    fn test_inverted_interval_auto_corrected() {
        // Regression: from_bounds(10, 5) should auto-swap to [5, 10]
        let t = Temporal::from_bounds(10, 5);
        assert_eq!(t.start, TimePoint::Moment(5));
        assert_eq!(t.end, TimePoint::Moment(10));
        assert!(t.active_at_millis(7));
    }

    #[test]
    fn test_inverted_interval_new_auto_corrected() {
        // Regression: Temporal::new with inverted TimePoints should auto-swap
        let t = Temporal::new(TimePoint::Moment(100), TimePoint::Moment(50));
        assert_eq!(t.start, TimePoint::Moment(50));
        assert_eq!(t.end, TimePoint::Moment(100));
    }

    #[test]
    fn test_equal_bounds_not_swapped() {
        // Equal bounds should remain unchanged
        let t = Temporal::from_bounds(5, 5);
        assert_eq!(t.start, TimePoint::Moment(5));
        assert_eq!(t.end, TimePoint::Moment(5));
    }

    #[test]
    fn test_timepoint_cmp_edge_cases() {
        use std::cmp::Ordering;

        // Test NegInf == NegInf
        assert_eq!(TimePoint::NegInf.cmp(&TimePoint::NegInf), Ordering::Equal);
        assert_eq!(TimePoint::NegInf, TimePoint::NegInf);

        // Test PosInf == PosInf
        assert_eq!(TimePoint::PosInf.cmp(&TimePoint::PosInf), Ordering::Equal);
        assert_eq!(TimePoint::PosInf, TimePoint::PosInf);

        // Test ordering between infinities and moments
        let moment = TimePoint::Moment(100);
        assert!(TimePoint::NegInf < moment);
        assert!(moment < TimePoint::PosInf);
        assert!(TimePoint::NegInf < TimePoint::PosInf);
    }
}
