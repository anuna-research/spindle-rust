# Query Operators

Spindle provides three query operators for interactive reasoning: what-if, why-not, and abduction.

## What-If Queries

**What-if** performs hypothetical reasoning: "What would be concluded if these facts were true?"

### API

```rust
use spindle_core::query::{what_if, HypotheticalClaim};
use spindle_core::literal::Literal;

let hypotheticals = vec![
    HypotheticalClaim::new(Literal::simple("wounded"))
];
let goal = Literal::negated("flies");
let result = what_if(&theory, hypotheticals, &goal);
```

### How It Works

1. Create a copy of the theory
2. Add hypothetical facts
3. Reason over the modified theory
4. Return new conclusions

### Example

Original theory:
```lisp
(given bird)
(normally r1 bird flies)
(normally r2 wounded (not flies))
(prefer r2 r1)
```

Query: What if `wounded` is true?

```rust
let hypotheticals = vec![HypotheticalClaim::new(Literal::simple("wounded"))];
let goal = Literal::negated("flies");
let result = what_if(&theory, hypotheticals, &goal);
// result.is_provable() = true
// result.new_conclusions = [Literal("~flies")]
```

### Use Cases

- Exploring consequences of decisions
- Scenario analysis
- Testing rule interactions

## Why-Not Queries

**Why-not** explains why a literal is NOT provable.

### API

```rust
use spindle_core::query::why_not;
use spindle_core::literal::Literal;

let literal = Literal::simple("flies");
let explanation = why_not(&theory, &literal);
```

### How It Works

1. Check if the literal is actually unprovable
2. Find rules that could prove it
3. For each rule, identify what's blocking it:
   - Missing body literals
   - Defeated by superior rules
   - Blocked by defeaters

### Example

Theory:
```lisp
(given bird)
(given penguin)
(normally r1 bird flies)
(normally r2 penguin (not flies))
(prefer r2 r1)
```

Query: Why is `flies` not provable?

```rust
let literal = Literal::simple("flies");
let explanation = why_not(&theory, &literal);
// explanation.literal = Literal("flies")
// explanation.would_derive = Some("r1")
// explanation.blocked_by = [BlockingCondition { ... }]
```

### Result Structure

```rust
struct WhyNotResult {
    literal: Literal,              // What we're asking about
    would_derive: Option<String>,  // Rule that could prove it
    blocked_by: Vec<BlockingCondition>,  // What's blocking
}

struct BlockingCondition {
    blocking_type: BlockingType,   // Type of blocking
    rule_label: String,            // The blocked rule
    missing_literals: Vec<Literal>, // Missing premises (if applicable)
    blocking_rule: Option<String>, // Blocking rule (if applicable)
    explanation: String,           // Human-readable explanation
}

enum BlockingType {
    MissingPremise,    // Body not satisfied
    Defeated,          // Defeated by defeater
    Contradicted,      // Complement is proven
}
```

### Use Cases

- Debugging unexpected results
- Understanding rule interactions
- Explaining decisions to users

## Abduction

**Abduction** finds hypotheses that would make a goal provable.

### API

```rust
use spindle_core::query::abduce;
use spindle_core::literal::Literal;

let goal = Literal::simple("goal");
let result = abduce(&theory, &goal, max_solutions);
```

### How It Works

1. Start with the goal literal
2. Find rules that could prove it
3. For each rule, identify missing body literals
4. Recursively abduce missing literals
5. Return minimal sets of assumptions

### Example

Theory:
```lisp
(normally r1 bird flies)
(normally r2 penguin (not flies))
(normally r3 (and bird healthy) strong-flyer)
(prefer r2 r1)
```

Query: What facts would make `flies` provable?

```rust
let goal = Literal::simple("flies");
let result = abduce(&theory, &goal, 3);
// result.solutions[0].facts = HashSet { Literal("bird") }
```

### Solution Structure

```rust
struct AbductionResult {
    goal: Literal,
    solutions: Vec<AbductionSolution>,
}

struct AbductionSolution {
    facts: HashSet<Literal>,      // Facts to assume
    rules_used: HashSet<String>,  // Rules involved
    confidence: f64,              // Trust score (if weighted)
}
```

### Handling Conflicts

Abduction considers superiority:

```lisp
(given penguin)
(normally r1 bird flies)
(normally r2 penguin (not flies))
(prefer r2 r1)
```

Abducing `flies` might return:
- `["bird", "healthy"]` with a rule `r3: bird, healthy => flies` where `r3 > r2`
- Or indicate that `flies` cannot be achieved given `penguin`

### Use Cases

- Diagnosis: What conditions explain symptoms?
- Planning: What actions achieve a goal?
- Debugging: What facts would fix this?

## Combined Queries

### Debugging Workflow

1. **Observe**: `flies` is not provable
2. **Why-not**: Discover r2 is blocking
3. **What-if**: Test `what_if(["healthy"])`
4. **Abduce**: Find minimal fix

### Example Session

```rust
// 1. Check current state
let conclusions = theory.reason();
assert!(!is_provable(&conclusions, "flies"));

// 2. Understand why
let why = why_not(&theory, "flies");
println!("Blocked by: {:?}", why.blockers);

// 3. Explore hypotheticals
let hypo = what_if(&theory, &["super_bird"], "flies");
if hypo.provable {
    println!("Would work if super_bird");
}

// 4. Find minimal solution
let solutions = abduce(&theory, "flies", 3);
println!("Minimal fixes: {:?}", solutions);
```

## WebAssembly API

The WASM bindings expose these operators:

```typescript
import { Spindle } from 'spindle-wasm';

const spindle = new Spindle();
spindle.parseSpl(`
  f1: >> bird
  f2: >> penguin
  r1: bird => flies
  r2: penguin => ~flies
  r2 > r1
`);

// What-if
const whatIf = spindle.whatIf(["healthy"], "flies");
console.log(whatIf);

// Why-not
const whyNot = spindle.whyNot("flies");
console.log(whyNot);

// Abduction
const abduce = spindle.abduce("flies", 3);
console.log(abduce);
```

## Limitations

1. **Depth limits**: Abduction has a configurable depth limit to avoid infinite search
2. **Minimal solutions**: Abduction returns minimal sets, not all possible sets
3. **Grounded queries**: Queries work on grounded theories; variables must be bound
4. **Performance**: Deep abduction can be expensive for large theories
