//! Requires operator — minimal-fact abduction convenience wrapper.
//!
//! [`requires`] returns the smallest set of facts that, if assumed, would make
//! a given goal provable under the current theory.  It delegates to
//! [`super::abduce`] with `max_solutions = 1` and extracts the smallest
//! solution.

use crate::error::Result;
use crate::literal::Literal;
use crate::theory::Theory;

use super::abduce::abduce;

/// Convenience function: Get the minimal facts needed to prove a goal
pub fn requires(theory: &Theory, goal: &Literal) -> Result<Vec<Literal>> {
    let result = abduce(theory, goal, 1)?;
    Ok(result
        .smallest_solution()
        .map(|s| s.facts.iter().cloned().collect())
        .unwrap_or_default())
}
