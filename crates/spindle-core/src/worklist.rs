//! Worklist-based Semi-Naive Evaluation
//!
//! This module provides data structures for efficient incremental reasoning
//! using semi-naive evaluation. Instead of re-evaluating all rules on each
//! iteration, we only evaluate rules whose bodies could have become newly
//! satisfied due to recently derived conclusions.
//!
//! # Algorithm Overview
//!
//! ## Current Approach (Naive)
//!
//! ```text
//! while changed:
//!     changed = false
//!     for each candidate in all_candidates:
//!         if can_prove(candidate):
//!             partial.insert(candidate)
//!             changed = true
//! ```
//!
//! This is O(|candidates| × |iterations|) - checking everything each time.
//!
//! ## Semi-Naive Approach
//!
//! ```text
//! worklist = initial_triggered_rules()
//! while worklist not empty:
//!     rule = worklist.pop()
//!     if rule.body_satisfied(partial) and all_attacks_defeated(rule.head):
//!         partial.insert(rule.head)
//!         for each rule' triggered by rule.head:
//!             worklist.push(rule')
//! ```
//!
//! This is O(|rules| + |triggered|) - only checking relevant rules.
//!
//! # Data Structures
//!
//! ## TriggerIndex
//!
//! Maps each literal to the rules that have it in their body.
//! When a literal is proven, we look up which rules might now fire.
//!
//! ```text
//! TriggerIndex: LiteralId -> Vec<RuleLabel>
//!
//! Example:
//!   bird -> [r1: bird => flies, r2: bird => has_wings]
//!   penguin -> [r3: penguin => ~flies]
//! ```
//!
//! ## RuleState
//!
//! Tracks the remaining unsatisfied body literals for each rule.
//!
//! ```text
//! struct RuleState {
//!     remaining: usize,      // Body literals not yet in partial
//!     in_worklist: bool,     // Already queued for evaluation
//!     evaluated: bool,       // Already fired successfully
//! }
//! ```
//!
//! ## Worklist
//!
//! Priority queue of rules ready to be evaluated (remaining == 0).
//!
//! # Integration Points
//!
//! 1. Build TriggerIndex during theory construction (or lazily)
//! 2. Initialize RuleState for each rule with body.len()
//! 3. When delta/lambda computed, decrement remaining for each proven literal
//! 4. Queue rules where remaining hits 0
//! 5. Evaluate rules from worklist, checking attacks
//! 6. When head proven, trigger new rules

use std::collections::VecDeque;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::intern::LiteralId;
use crate::rule::RuleLabel;

/// Index mapping literals to rules they can trigger.
///
/// When a literal is added to the closure, we can quickly find
/// all rules that might now become satisfiable.
#[derive(Debug, Default)]
pub struct TriggerIndex {
    /// Maps body literal -> rules containing it in body
    body_triggers: FxHashMap<LiteralId, Vec<RuleLabel>>,
    /// Maps head literal -> rules producing it
    head_triggers: FxHashMap<LiteralId, Vec<RuleLabel>>,
}

impl TriggerIndex {
    /// Create a new empty trigger index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a trigger: when `literal` is proven, `rule` might fire.
    pub fn add_body_trigger(&mut self, literal: LiteralId, rule: RuleLabel) {
        self.body_triggers
            .entry(literal)
            .or_default()
            .push(rule);
    }

    /// Add a head trigger: `rule` produces `literal`.
    pub fn add_head_trigger(&mut self, literal: LiteralId, rule: RuleLabel) {
        self.head_triggers
            .entry(literal)
            .or_default()
            .push(rule);
    }

    /// Get rules triggered by proving a literal (rules with this in body).
    pub fn rules_triggered_by(&self, literal: LiteralId) -> &[RuleLabel] {
        self.body_triggers
            .get(&literal)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Get rules that produce a literal.
    pub fn rules_producing(&self, literal: LiteralId) -> &[RuleLabel] {
        self.head_triggers
            .get(&literal)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
}

/// State of a rule during worklist evaluation.
#[derive(Debug, Clone)]
pub struct RuleState {
    /// Number of body literals not yet proven
    pub remaining: usize,
    /// Whether rule is currently in worklist
    pub in_worklist: bool,
    /// Whether rule has successfully fired
    pub fired: bool,
}

impl RuleState {
    /// Create state for a rule with given body size.
    pub fn new(body_size: usize) -> Self {
        Self {
            remaining: body_size,
            in_worklist: false,
            fired: false,
        }
    }

    /// Check if rule body is satisfied (all literals proven).
    pub fn is_ready(&self) -> bool {
        self.remaining == 0 && !self.fired
    }
}

/// Worklist for semi-naive evaluation.
///
/// Contains rules that are ready to be evaluated (body satisfied).
#[derive(Debug, Default)]
pub struct RuleWorklist {
    /// Queue of rule labels to evaluate
    queue: VecDeque<RuleLabel>,
    /// State for each rule
    states: FxHashMap<RuleLabel, RuleState>,
    /// Rules currently in queue (for dedup)
    in_queue: FxHashSet<RuleLabel>,
}

impl RuleWorklist {
    /// Create a new empty worklist.
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize state for a rule.
    pub fn init_rule(&mut self, label: RuleLabel, body_size: usize) {
        self.states.insert(label, RuleState::new(body_size));
    }

    /// Decrement remaining count for a rule (called when body literal is proven).
    /// Returns true if rule just became ready.
    pub fn decrement(&mut self, label: &RuleLabel) -> bool {
        if let Some(state) = self.states.get_mut(label) {
            if state.remaining > 0 {
                state.remaining -= 1;
            }
            state.is_ready()
        } else {
            false
        }
    }

    /// Add a rule to the worklist if not already present.
    pub fn enqueue(&mut self, label: RuleLabel) {
        if !self.in_queue.contains(&label)
            && let Some(state) = self.states.get_mut(&label)
            && !state.fired {
            state.in_worklist = true;
            self.in_queue.insert(label.clone());
            self.queue.push_back(label);
        }
    }

    /// Pop the next rule to evaluate.
    pub fn pop(&mut self) -> Option<RuleLabel> {
        while let Some(label) = self.queue.pop_front() {
            self.in_queue.remove(&label);
            if let Some(state) = self.states.get_mut(&label) {
                state.in_worklist = false;
                if !state.fired && state.remaining == 0 {
                    return Some(label);
                }
            }
        }
        None
    }

    /// Mark a rule as fired.
    pub fn mark_fired(&mut self, label: &RuleLabel) {
        if let Some(state) = self.states.get_mut(label) {
            state.fired = true;
        }
    }

    /// Check if a rule has fired.
    pub fn has_fired(&self, label: &RuleLabel) -> bool {
        self.states
            .get(label)
            .map(|s| s.fired)
            .unwrap_or(false)
    }

    /// Check if worklist is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Get the state of a rule.
    pub fn state(&self, label: &RuleLabel) -> Option<&RuleState> {
        self.states.get(label)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::intern::intern;
    use crate::intern::LiteralId;

    fn lit(name: &str) -> LiteralId {
        LiteralId::positive(intern(name))
    }

    #[test]
    fn test_trigger_index() {
        let mut index = TriggerIndex::new();

        index.add_body_trigger(lit("bird"), "r1".to_string());
        index.add_body_trigger(lit("bird"), "r2".to_string());
        index.add_body_trigger(lit("penguin"), "r3".to_string());

        assert_eq!(index.rules_triggered_by(lit("bird")).len(), 2);
        assert_eq!(index.rules_triggered_by(lit("penguin")).len(), 1);
        assert_eq!(index.rules_triggered_by(lit("unknown")).len(), 0);
    }

    #[test]
    fn test_trigger_index_head_triggers() {
        let mut index = TriggerIndex::new();

        index.add_head_trigger(lit("flies"), "r1".to_string());
        index.add_head_trigger(lit("flies"), "r2".to_string());
        index.add_head_trigger(lit("swims"), "r3".to_string());

        assert_eq!(index.rules_producing(lit("flies")).len(), 2);
        assert_eq!(index.rules_producing(lit("swims")).len(), 1);
        assert_eq!(index.rules_producing(lit("unknown")).len(), 0);
    }

    #[test]
    fn test_worklist_basic() {
        let mut worklist = RuleWorklist::new();

        // Rule with 2 body literals
        worklist.init_rule("r1".to_string(), 2);

        // Not ready yet
        assert!(!worklist.decrement(&"r1".to_string()));

        // Now ready
        assert!(worklist.decrement(&"r1".to_string()));

        // Enqueue and pop
        worklist.enqueue("r1".to_string());
        assert_eq!(worklist.pop(), Some("r1".to_string()));
        assert!(worklist.is_empty());
    }

    #[test]
    fn test_worklist_fired() {
        let mut worklist = RuleWorklist::new();

        worklist.init_rule("r1".to_string(), 0);  // Empty body
        worklist.enqueue("r1".to_string());

        let label = worklist.pop().unwrap();
        worklist.mark_fired(&label);

        // Can't fire again
        worklist.enqueue("r1".to_string());
        assert!(worklist.pop().is_none());
    }

    #[test]
    fn test_worklist_has_fired() {
        let mut worklist = RuleWorklist::new();

        worklist.init_rule("r1".to_string(), 0);
        assert!(!worklist.has_fired(&"r1".to_string()));

        worklist.enqueue("r1".to_string());
        let label = worklist.pop().unwrap();
        worklist.mark_fired(&label);

        assert!(worklist.has_fired(&"r1".to_string()));
        // Unknown rule returns false
        assert!(!worklist.has_fired(&"unknown".to_string()));
    }

    #[test]
    fn test_worklist_state() {
        let mut worklist = RuleWorklist::new();

        worklist.init_rule("r1".to_string(), 2);

        let state = worklist.state(&"r1".to_string());
        assert!(state.is_some());
        let s = state.unwrap();
        assert_eq!(s.remaining, 2);
        assert!(!s.in_worklist);
        assert!(!s.fired);

        // Unknown rule returns None
        assert!(worklist.state(&"unknown".to_string()).is_none());
    }

    #[test]
    fn test_worklist_decrement_unknown_rule() {
        let mut worklist = RuleWorklist::new();
        // Decrementing unknown rule returns false
        assert!(!worklist.decrement(&"unknown".to_string()));
    }

    #[test]
    fn test_rule_state_is_ready() {
        let state = RuleState::new(0);
        assert!(state.is_ready());

        let state2 = RuleState::new(1);
        assert!(!state2.is_ready());
    }

    #[test]
    fn test_pop_with_remaining_zero_not_fired() {
        let mut worklist = RuleWorklist::new();

        // Initialize with empty body (remaining = 0)
        worklist.init_rule("r1".to_string(), 0);
        worklist.enqueue("r1".to_string());

        // Pop should return the rule since remaining=0 and not fired
        let result = worklist.pop();
        assert_eq!(result, Some("r1".to_string()));
    }

    #[test]
    fn test_pop_skips_rule_with_remaining_not_zero() {
        let mut worklist = RuleWorklist::new();

        // Rule with remaining > 0 should be skipped by pop
        worklist.init_rule("r1".to_string(), 1);
        worklist.enqueue("r1".to_string());

        // Pop should return None since remaining != 0
        let result = worklist.pop();
        assert!(result.is_none());
    }

    #[test]
    fn test_pop_skips_fired_rule() {
        let mut worklist = RuleWorklist::new();

        worklist.init_rule("r1".to_string(), 0);
        // Mark as fired before enqueueing
        worklist.mark_fired(&"r1".to_string());
        worklist.enqueue("r1".to_string());

        // Pop should skip fired rule
        let result = worklist.pop();
        assert!(result.is_none());
    }
}
