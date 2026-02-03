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
            TimePoint::Moment(v) => write!(f, "{}", v),
        }
    }
}

/// Temporal interval with start and end time points
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
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

impl Temporal {
    /// Create an empty temporal (unbounded interval from -inf to +inf)
    pub const fn empty() -> Self {
        EMPTY_TEMPORAL
    }

    /// Create a temporal with specific bounds
    pub fn new(start: TimePoint, end: TimePoint) -> Self {
        Self { start, end }
    }

    /// Create a temporal from numeric bounds (milliseconds)
    pub fn from_bounds(start: i64, end: i64) -> Self {
        Self {
            start: TimePoint::Moment(start),
            end: TimePoint::Moment(end),
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
        write!(f, "{}", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_temporal() {
        let t = Temporal::empty();
        assert!(t.is_empty());
        assert!(!t.has_info());
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
}
