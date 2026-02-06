//! Memory profiling for spindle-core
//!
//! Run with: cargo run --example memory_profile --features dhat-heap --release
//!
//! This produces a dhat-heap.json file that can be viewed at:
//! https://nnethercote.github.io/dh_view/dh_view.html

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use spindle_core::explanation::explain;
use spindle_core::grounding::ground_theory;
use spindle_core::mode::Mode;
use spindle_core::prelude::*;
use spindle_core::query::{abduce, what_if, why_not, HypotheticalClaim};
use spindle_core::reason::reason;
use spindle_core::scalable::reason_scalable;
use spindle_core::temporal::Temporal;

/// Create a wide theory with N independent rule groups
fn create_wide_theory(n: usize) -> Theory {
    let mut theory = Theory::new();

    for i in 0..n {
        let base = format!("base{}", i);
        let prop = format!("prop{}", i);

        theory.add_rule(Rule::fact(format!("f{}", i), Literal::simple(&base)));

        theory.add_rule(Rule::defeasible(
            format!("r{}", i),
            vec![Literal::simple(&base)],
            Literal::simple(&prop),
        ));

        theory.add_rule(Rule::defeasible(
            format!("s{}", i),
            vec![Literal::simple(&base)],
            Literal::negated(&prop),
        ));

        theory.add_superiority(&format!("r{}", i), &format!("s{}", i));
    }
    theory
}

/// Create a chain theory
fn create_chain_theory(n: usize) -> Theory {
    let mut theory = Theory::new();
    theory.add_rule(Rule::fact("f0", Literal::simple("p0")));

    for i in 0..n {
        theory.add_rule(Rule::defeasible(
            format!("r{}", i),
            vec![Literal::simple(format!("p{}", i))],
            Literal::simple(format!("p{}", i + 1)),
        ));
    }
    theory
}

/// Create a theory with variables for grounding
fn create_grounding_theory(n_entities: usize) -> Theory {
    let mut theory = Theory::new();

    for i in 0..n_entities {
        theory.add_rule(Rule::fact(
            format!("f_person_{}", i),
            Literal::new(
                "person",
                false,
                Mode::empty(),
                Temporal::empty(),
                vec![format!("e{}", i)],
            ),
        ));
    }

    for i in 0..n_entities {
        for j in 0..n_entities {
            if i != j && (i + j) % 3 == 0 {
                theory.add_rule(Rule::fact(
                    format!("f_knows_{}_{}", i, j),
                    Literal::new(
                        "knows",
                        false,
                        Mode::empty(),
                        Temporal::empty(),
                        vec![format!("e{}", i), format!("e{}", j)],
                    ),
                ));
            }
        }
    }

    theory.add_rule(Rule::defeasible(
        "r_connected",
        vec![
            Literal::new(
                "person",
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?x".to_string()],
            ),
            Literal::new(
                "knows",
                false,
                Mode::empty(),
                Temporal::empty(),
                vec!["?x".to_string(), "?y".to_string()],
            ),
        ],
        Literal::new(
            "connected",
            false,
            Mode::empty(),
            Temporal::empty(),
            vec!["?x".to_string(), "?y".to_string()],
        ),
    ));

    theory
}

fn main() {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    println!("=== Memory Profiling for spindle-core ===\n");

    // Profile standard reasoning
    println!("1. Standard reasoning (100 rule groups)...");
    {
        let theory = create_wide_theory(100);
        let _conclusions = reason(&theory);
    }

    // Profile scalable reasoning
    println!("2. Scalable reasoning (100 rule groups)...");
    {
        let theory = create_wide_theory(100);
        let grounded = ground_theory(&theory);
        let indexed = spindle_core::index::IndexedTheory::build(&grounded);
        let _conclusions = reason_scalable(&indexed);
    }

    // Profile chain reasoning
    println!("3. Chain reasoning (50 rules)...");
    {
        let theory = create_chain_theory(50);
        let _conclusions = reason(&theory);
    }

    // Profile grounding
    println!("4. Grounding (20 entities)...");
    {
        let theory = create_grounding_theory(20);
        let _grounded = ground_theory(&theory);
    }

    // Profile query operators
    println!("5. Query operators...");
    {
        let mut theory = Theory::new();
        theory.add_rule(Rule::fact("f1", Literal::simple("code_written")));
        theory.add_rule(Rule::fact("f2", Literal::simple("has_tests")));
        theory.add_rule(Rule::defeasible(
            "r_review",
            vec![
                Literal::simple("code_written"),
                Literal::simple("has_tests"),
                Literal::simple("tests_pass"),
            ],
            Literal::simple("ready_review"),
        ));

        // What-if
        let _result = what_if(
            &theory,
            vec![HypotheticalClaim::new(Literal::simple("tests_pass"))],
            &Literal::simple("ready_review"),
        );

        // Why-not
        let _result = why_not(&theory, &Literal::simple("ready_review"));

        // Abduction
        let _result = abduce(&theory, &Literal::simple("ready_review"), 5);
    }

    // Profile explanation generation
    println!("6. Explanation generation...");
    {
        let theory = create_chain_theory(10);
        reason(&theory);
        let _exp = explain(&theory, &Literal::simple("p10"));
    }

    println!("\n=== Profiling complete ===");
    println!("View results at: https://nnethercote.github.io/dh_view/dh_view.html");

    #[cfg(not(feature = "dhat-heap"))]
    println!("\nNote: Run with --features dhat-heap to enable memory profiling");
}
