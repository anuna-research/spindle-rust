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
The builtin prelude is available; the current JavaScript API does not expose host
extension registration. See [Aggregation](../guides/aggregation.md) for supported
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
// candidates.solutions: { facts: string[], rules_used: string[], confidence }[]
```

`whatIf` does not mutate the original theory and deduplicates newly positive
literals. `whyNot` uses grounded rules for variable-headed diagnostics.
Bounded temporal goals use exact windows; atemporal goals use family matching.

`abduce` returns raw candidates, not verified requirements. The WASM class has no
`requires` method. The Rust `requires_with_options` API and CLI `requires`
command verify candidates by full reasoning. See [Query Operators](../guides/queries.md).
