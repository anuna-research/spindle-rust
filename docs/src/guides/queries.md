# Query Operators

`query` checks a literal, `why_not` identifies blockers, and `what_if` evaluates hypothetical facts.
`requires_with_options` verifies proposed facts by rerunning the reasoner.
`abduce` supplies raw candidates.

## Rust example

```rust
use spindle_core::{Literal, Theory};
use spindle_core::query::{
    query, why_not, what_if, HypotheticalClaim,
    requires_with_options, RequiresOptions,
};

fn main() -> spindle_core::error::Result<()> {
    let mut theory = Theory::new();
    theory.add_defeasible_rule(&["bird"], "flies");
    let goal = Literal::simple("flies");

    let current = query(&theory, &goal)?;
    println!("Status: {}", current.status);
    let why = why_not(&theory, &goal)?;
    println!("Blockers: {:?}", why.blocked_by);

    let hypothetical = what_if(
        &theory,
        vec![HypotheticalClaim::new(Literal::simple("bird"))],
        &goal,
    )?;
    assert!(hypothetical.is_provable());

    let required = requires_with_options(&theory, &goal, RequiresOptions {
        max_solutions: 3,
        max_raw_candidates: 1000,
    })?;
    for solution in &required.solutions {
        println!("Assume: {:?}", solution.facts);
    }
    println!("Search: {:?}", required.search_status);
    Ok(())
}
```

`query` returns `Provable`, `Refuted` (the complement is provable), or `Unknown`.
These statuses summarize positive evidence; `Unknown` does not mean the engine
has proved a negative tag. See [Conclusions](../concepts/conclusions.md).

## Hypothetical reasoning

`what_if` clones the theory, adds the supplied facts, and compares the new result
with the baseline. `new_conclusions` contains newly provable literals without
repeating a literal proved at both `+D` and `+d`. Distinct temporal windows and
typed terms are preserved. The original theory is unchanged.

## Explaining missing conclusions

`why_not` examines grounded rules, so a query such as `(flies opus)` can find a
rule written with `(flies ?x)` in its head. Its `blocked_by` entries identify
missing premises, defeat, contradiction, or undetermined conditions. Superiority
uses source template labels. The explanation system also resolves grounded labels
to templates when constructing proof trees.

## Abduction and verified requirements

`abduce(&theory, &goal, max_solutions)?` returns candidate assumptions. A candidate
can fail under full conflict resolution. `requires_with_options` returns verified
solutions. Verification injects each candidate fact-set and reruns reasoning,
retaining only candidates that establish the goal.

Each `AbductionSolution` contains:

- `facts: Vec<Literal>`: deterministic, deduplicated assumptions preserving typed
  terms and distinct temporal windows.
- `rules_used`: the rules associated with that solution's fact-set.
- `confidence`: currently initialized to `1.0`; this value is not a calibrated probability or proof that the candidate establishes the goal.

`RequiresResult` reports `already_provable`, `solutions`, `search_status`, and
verification counters (`raw_examined`, `accepted`, `rejected`). `BoundedComplete`
means the available search finished or the requested solution count was reached;
it does not promise exhaustive enumeration. `BudgetExhausted` means further raw
candidates existed beyond the budget. Duplicate fact-sets consume one budget slot.
An unproved goal with no accepted solutions is a valid result.

## Temporal matching

Bounded goals use exact temporal windows. A query for `p@[1,10]` does not match
`p@[20,30]`, atemporal `p`, or even a containing window `p@[0,20]`.
Atemporal goals match any member of the same literal family. This applies to
`query`, `requires`, `what_if`, and `abduce`.

`query_with_match_mode(&theory, &goal, QueryMatchMode::Family)` in
`spindle_core::query` explicitly selects family-wide matching. See [Temporal Reasoning](temporal.md).

## CLI and WebAssembly

```sh
spindle query flies theory.spl --json
spindle why-not flies theory.spl --json
spindle requires flies theory.spl --max 3 --json
```

`requires --json` emits `spindle.requires.v2`; an unsatisfied goal can have an empty
solution list. The CLI has no standalone `what-if` or
`abduce` command. The WASM `Spindle` object exposes `query`, `whatIf`, `whyNot`,
and raw `abduce`; it does not expose the verified `requires` API.

```javascript
const hypothetical = spindle.whatIf(["bird"], "flies");
const blockers = spindle.whyNot("flies");
const candidates = spindle.abduce("flies", 3);
```

See the [CLI reference](../reference/cli.md) and
[WebAssembly guide](../integration/wasm.md) for output formats.
