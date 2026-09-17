# WebAssembly

The `spindle-wasm` crate exposes a `Spindle` class to JavaScript. It uses the
same preparation and reasoning pipeline as the Rust library.

The [browser how-to](../guides/run-in-browser.md) covers initialization, object lifetime, error handling, and UI responsiveness.

## Build targets

Builds require `wasm-pack`. Each target writes generated JavaScript and WASM files under `crates/spindle-wasm/pkg`.

| Command | Target |
|---|---|
| `make wasm` | Web |
| `make wasm-node` | Node.js |
| `make wasm-bundler` | Bundler |

Initialization is asynchronous. Instance methods run synchronously and throw JavaScript errors on parsing or reasoning failure.

## Reasoning output

`reason()` returns a structured `spindle.reason.v1` object, not a bare array.
`reasonV2()` returns the corresponding `spindle.reason.v2` object with typed
term arguments. Both include:

- `schema_version`, `evaluated_at`, and `grounding` statistics.
- `conclusions`, whose entries include `conclusion_type`, `literal_spl`,
  `literal_struct`, and `positive`.
- `diagnostics` and theory `stats`.

`result.conclusions` contains the result entries. `getPositiveConclusions()` returns
an array of positive literal strings. The shared DTO definitions live in
`spindle-contract`; WASM returns the result as a JavaScript object rather than serialized JSON text.

## Theory operations

| Method | Purpose |
|---|---|
| `parseSpl(source)` | Replace the current theory with parsed SPL |
| `addFact(name)` | Add a fact; return its label |
| `addStrictRule(body, head)` | Add a strict rule; return its label |
| `addDefeasibleRule(body, head)` | Add a defeasible rule; return its label |
| `addDefeater(body, head)` | Add a defeater; return its label |
| `addSuperiority(superior, inferior)` | Add a priority between rule labels |
| `ruleCount()` / `getRules()` | Inspect the theory |
| `clear()` | Remove the theory's rules and facts |
| `reasonSpl(source)` | Parse and return formatted textual reasoning output |
| `free()` | Release the WASM object when finished |

SPL input supports structured predicates, arithmetic, metadata, and aggregates.
The builtin prelude is available. `registerExtensions(json)` registers portable
lookup functions and named aggregators (see below). Arbitrary JavaScript callbacks
are not supported. See [Aggregation](../guides/aggregation.md) for supported
values and preparation restrictions.

## Query methods

```javascript
const status = spindle.query('flies');
// status.status: "provable", "refuted", or "unknown"
const hypothetical = spindle.whatIf(['bird'], 'flies');
// hypothetical.provable: boolean; new_conclusions: string[]
const why = spindle.whyNot('flies');
// why.is_provable, why.would_derive, why.blockers
const candidates = spindle.abduce('flies', 3);
// candidates.solutions: { facts: string[], facts_struct, rules_used: string[], confidence }[]
```

`whatIf` does not mutate the original theory and deduplicates newly positive
literals. `whyNot` uses grounded rules for variable-headed diagnostics.
Bounded temporal goals use exact windows; atemporal goals use family matching.

`abduce` returns raw candidates. `requires(goal, maxSolutions)` verifies candidates
by full reasoning and returns the same `spindle.requires.v2` envelope as the CLI.
Both require a positive solution limit. `explain(literal)` returns the CLI's
`spindle.explain.v1` envelope with a grounded proof tree or null.
See [Query Operators](../guides/queries.md).

All query methods accept ground SPL literals, including `(p 2)`, `(not (p 2))`,
modal forms and `(during (p 2) 100 200)`. Legacy `p(2)` and `~p(2)` are also
accepted, now with numeric types preserved. Malformed literals throw an error.

## Trust, vocabulary and preparation

```javascript
const weighted = spindle.reasonWithTrust(true); // typed v2; false selects v1
const vocabulary = spindle.vocabulary(); // spindle.vocabulary/1
spindle.setReferenceTime('2026-09-17T00:00:00Z');
const atTime = spindle.query('active');
spindle.setReferenceTime(null); // remove the as-of filter
```

Trust output adds `trust_degree`, optional `trust_sources`, and `trust_details`
to each conclusion. Details contain ordered `diminished_by` challenges (label,
degree before and after, reduction) and an `above_threshold` object. Ordinary
`reason()` and `reasonV2()` remain unweighted.

`vocabulary()` derives the original theory's declarations, observed predicate
symbols, argument profiles, descriptions, provenance and diagnostics. A symbol's
`functor` and `arity` expose structural predicate identity.

Reference time and extension registrations apply to reasoning and query methods
and persist across `parseSpl`, `reasonSpl` and `clear`. Existing aggregate
restrictions still apply: temporal preparation options are rejected for aggregates.

## Portable extensions

`registerExtensions(json)` replaces the instance's custom registry atomically;
invalid input preserves the previous registry. The JSON format and runnable
example are documented in the [CLI reference](../reference/cli.md#portable-extensions).
Pass an empty `functions`/`aggregators` document to clear registrations.
Lookup matching preserves term types. Missing rows return evaluation failure;
ordinary grounding discards that candidate binding.
Rust embedding continues to support arbitrary pure `ExtensionFunction` implementations.
