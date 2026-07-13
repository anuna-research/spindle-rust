//! Canonical test fixture library for spindle-core.
//!
//! Provides reusable builder functions that return fully constructed
//! `Theory` values representing common defeasible logic scenarios.
//!
//! # Usage
//!
//! From an integration test file in `crates/spindle-core/tests/`:
//!
//! ```ignore
//! mod fixtures;
//! use fixtures::*;
//!
//! #[test]
//! fn my_test() {
//!     let theory = tweety_triangle();
//!     // ... reason about the theory ...
//! }
//! ```

#![allow(dead_code)]

use spindle_core::literal::Literal;
use spindle_core::mode::Mode;
use spindle_core::rule::Rule;
use spindle_core::temporal::Temporal;
use spindle_core::theory::Theory;

// ---------------------------------------------------------------------------
// tweety_triangle — the classic defeasible logic example
// ---------------------------------------------------------------------------

/// The Tweety Triangle: birds normally fly, penguins are birds,
/// penguins normally do not fly, and the penguin rule is superior.
///
/// Facts:
///   - f1: >> tweety_is_a_bird   (tweety is a bird)
///   - f2: >> penguin             (tweety is a penguin)
///
/// Rules:
///   - r1: bird => flies          (birds normally fly)
///   - r2: penguin => ~flies      (penguins normally do not fly)
///   - s1: penguin -> bird        (penguins are birds — strict)
///
/// Superiority:
///   - r2 > r1                    (penguin rule beats bird rule)
///
/// Expected: +d flies is blocked, +d ~flies holds.
pub fn tweety_triangle() -> Theory {
    let mut t = Theory::new();

    // Facts
    t.add_fact("bird");
    t.add_fact("penguin");

    // Strict: penguin -> bird
    t.add_strict_rule(&["penguin"], "bird");

    // Defeasible: bird => flies
    let r1 = t.add_defeasible_rule(&["bird"], "flies");

    // Defeasible: penguin => ~flies
    let r2 = t.add_defeasible_rule(&["penguin"], "~flies");

    // Superiority: r2 > r1
    t.add_superiority(&r2, &r1);

    t
}

// ---------------------------------------------------------------------------
// nixon_diamond — ambiguity with no resolution
// ---------------------------------------------------------------------------

/// The Nixon Diamond: republicans are normally not pacifists,
/// quakers are normally pacifists, and there is no superiority
/// relation to resolve the conflict.
///
/// Facts:
///   - f1: >> republican
///   - f2: >> quaker
///
/// Rules:
///   - r1: republican => ~pacifist
///   - r2: quaker     => pacifist
///
/// Superiority: none
///
/// Expected: both +d pacifist and +d ~pacifist are blocked (ambiguity).
pub fn nixon_diamond() -> Theory {
    let mut t = Theory::new();

    t.add_fact("republican");
    t.add_fact("quaker");

    t.add_defeasible_rule(&["republican"], "~pacifist");
    t.add_defeasible_rule(&["quaker"], "pacifist");

    t
}

// ---------------------------------------------------------------------------
// inheritance_chain — configurable depth chain of defeasible rules
// ---------------------------------------------------------------------------

/// A linear chain of defeasible rules of configurable depth.
///
/// Produces a single fact `p0` and `depth` defeasible rules:
///   - p0 => p1
///   - p1 => p2
///   - ...
///   - p(depth-1) => p(depth)
///
/// Useful for stress-testing forward chaining and checking that
/// conclusions propagate through arbitrarily long rule chains.
///
/// # Panics
///
/// Does not panic. A depth of 0 returns a theory with just the
/// base fact `p0`.
pub fn inheritance_chain(depth: usize) -> Theory {
    let mut t = Theory::new();

    // Base fact
    t.add_fact("p0");

    // Chain of defeasible rules
    for i in 0..depth {
        let body = format!("p{i}");
        let head = format!("p{}", i + 1);
        t.add_defeasible_rule(&[body.as_str()], &head);
    }

    t
}

// ---------------------------------------------------------------------------
// temporal_theory — theory with temporal bounds on rules and facts
// ---------------------------------------------------------------------------

/// A theory with temporal bounds demonstrating time-scoped reasoning.
///
/// Facts:
///   - employee  (always true — unbounded)
///   - on_leave  (active during [100, 200] only)
///
/// Rules:
///   - r1: employee => works          (unbounded — employees normally work)
///   - r2: on_leave => ~works         (active [100, 200] — on leave means not working)
///   - r3: employee => has_benefits   (active [0, 500] — benefits during employment period)
///
/// Superiority:
///   - r2 > r1  (leave overrides work obligation within its temporal window)
///
/// This fixture exercises temporal intersection during reasoning:
/// outside [100, 200] only r1 fires; inside [100, 200] both fire
/// but r2 wins via superiority.
pub fn temporal_theory() -> Theory {
    let mut t = Theory::new();

    // Unbounded fact
    t.add_fact("employee");

    // Temporally bounded fact: on_leave active [100, 200]
    let on_leave_lit = Literal::new(
        "on_leave",
        false,
        Mode::empty(),
        Temporal::from_bounds(100, 200),
        vec![],
    );
    let on_leave_rule = Rule::fact("t_f1", on_leave_lit);
    t.add_rule(on_leave_rule);

    // r1: employee => works (unbounded)
    let r1 = t.add_defeasible_rule(&["employee"], "works");

    // r2: on_leave => ~works (temporal [100, 200])
    let r2_body = vec![Literal::simple("on_leave")];
    let r2_head = Literal::new(
        "works",
        true, // negated
        Mode::empty(),
        Temporal::empty(),
        vec![],
    );
    let mut r2_rule = Rule::defeasible("t_r2", r2_body, r2_head);
    r2_rule.temporal = Temporal::from_bounds(100, 200);
    t.add_rule(r2_rule);

    // r3: employee => has_benefits (temporal [0, 500])
    let r3_body = vec![Literal::simple("employee")];
    let r3_head = Literal::simple("has_benefits");
    let mut r3_rule = Rule::defeasible("t_r3", r3_body, r3_head);
    r3_rule.temporal = Temporal::from_bounds(0, 500);
    t.add_rule(r3_rule);

    // Superiority: r2 > r1
    t.add_superiority("t_r2", &r1);

    t
}

// ---------------------------------------------------------------------------
// modal_theory — theory with deontic modal operators
// ---------------------------------------------------------------------------

/// A theory with modal operators (obligation, permission, forbidden).
///
/// Facts:
///   - citizen
///   - driver
///
/// Rules:
///   - r1: citizen => [O] pay_taxes       (citizens are obligated to pay taxes)
///   - r2: driver  => [O] have_license    (drivers are obligated to have a license)
///   - r3: citizen => [P] vote            (citizens are permitted to vote)
///   - r4: driver  => [F] drive_drunk     (drivers are forbidden from driving drunk)
///   - r5: citizen => [P] free_speech     (citizens are permitted free speech)
///
/// This fixture exercises modal operator propagation through the
/// reasoning engine.
pub fn modal_theory() -> Theory {
    let mut t = Theory::new();

    // Facts
    t.add_fact("citizen");
    t.add_fact("driver");

    // r1: citizen => [O] pay_taxes
    let r1_body = vec![Literal::simple("citizen")];
    let mut r1_head = Literal::simple("pay_taxes");
    r1_head.mode = Mode::obligation();
    let r1 = Rule::defeasible("m_r1", r1_body, r1_head);
    t.add_rule(r1);

    // r2: driver => [O] have_license
    let r2_body = vec![Literal::simple("driver")];
    let mut r2_head = Literal::simple("have_license");
    r2_head.mode = Mode::obligation();
    let r2 = Rule::defeasible("m_r2", r2_body, r2_head);
    t.add_rule(r2);

    // r3: citizen => [P] vote
    let r3_body = vec![Literal::simple("citizen")];
    let mut r3_head = Literal::simple("vote");
    r3_head.mode = Mode::permission();
    let r3 = Rule::defeasible("m_r3", r3_body, r3_head);
    t.add_rule(r3);

    // r4: driver => [F] drive_drunk
    let r4_body = vec![Literal::simple("driver")];
    let mut r4_head = Literal::simple("drive_drunk");
    r4_head.mode = Mode::forbidden();
    let r4 = Rule::defeasible("m_r4", r4_body, r4_head);
    t.add_rule(r4);

    // r5: citizen => [P] free_speech
    let r5_body = vec![Literal::simple("citizen")];
    let mut r5_head = Literal::simple("free_speech");
    r5_head.mode = Mode::permission();
    let r5 = Rule::defeasible("m_r5", r5_body, r5_head);
    t.add_rule(r5);

    t
}

// ---------------------------------------------------------------------------
// conflicting_defeaters — multiple defeaters interacting
// ---------------------------------------------------------------------------

/// A theory where multiple defeaters interact to block conclusions.
///
/// Facts:
///   - bird
///   - injured
///   - young
///
/// Rules:
///   - r1: bird    => flies         (birds normally fly)
///   - d1: injured ~> ~flies        (defeater: injury blocks flight)
///   - d2: young   ~> ~flies        (defeater: youth blocks flight)
///   - r2: bird    => has_feathers  (birds normally have feathers)
///   - d3: injured ~> ~has_feathers (defeater: injury blocks feathers)
///
/// Superiority: none (defeaters don't participate in superiority)
///
/// Expected: `flies` is blocked by both defeaters d1 and d2;
/// `has_feathers` is blocked by defeater d3. Neither defeater
/// produces a positive conclusion for ~flies or ~has_feathers
/// (defeaters only block, they do not prove).
pub fn conflicting_defeaters() -> Theory {
    let mut t = Theory::new();

    // Facts
    t.add_fact("bird");
    t.add_fact("injured");
    t.add_fact("young");

    // Defeasible rules
    t.add_defeasible_rule(&["bird"], "flies");
    t.add_defeasible_rule(&["bird"], "has_feathers");

    // Defeaters
    t.add_defeater(&["injured"], "~flies");
    t.add_defeater(&["young"], "~flies");
    t.add_defeater(&["injured"], "~has_feathers");

    t
}

// ---------------------------------------------------------------------------
// empty_theory — edge case: no rules, no facts
// ---------------------------------------------------------------------------

/// An empty theory with no rules, no facts, and no superiority relations.
///
/// Useful as an edge-case test: reasoning over an empty theory should
/// produce zero conclusions without errors.
pub fn empty_theory() -> Theory {
    Theory::new()
}

// ---------------------------------------------------------------------------
// facts_only — theory with only facts (no rules)
// ---------------------------------------------------------------------------

/// A theory containing only facts and no rules.
///
/// Facts:
///   - bird
///   - penguin
///   - cold
///
/// No rules, no superiority relations.
///
/// Expected: the reasoner should derive definite conclusions (+D) for
/// each fact but no defeasible conclusions (+d) since there are no
/// defeasible rules.
pub fn facts_only() -> Theory {
    let mut t = Theory::new();

    t.add_fact("bird");
    t.add_fact("penguin");
    t.add_fact("cold");

    t
}
