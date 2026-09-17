# WebAssembly

The `spindle-wasm` crate exposes a `Spindle` class to JavaScript. It uses the
same preparation and reasoning pipeline as the Rust library.

## Build locally

Install `wasm-pack`, then run a target from the repository root:

```sh
make wasm          # web target
make wasm-node     # Node.js target
make wasm-bundler  # bundler target
```

Use the generated JavaScript and WASM files under `crates/spindle-wasm/pkg`.
The examples below assume the web build is served from a `pkg` directory.

## Browser example

```javascript
import init, { Spindle } from './pkg/spindle_wasm.js';

await init();
const spindle = new Spindle();
try {
    spindle.parseSpl(`
        (given (payment alice first 10))
        (given (payment alice second 10))
        (normally total
          (agg ?total sum ?amount (payment ?person ?id ?amount))
          (total-payment ?total))
    `);
    const result = spindle.reasonV2();
    console.log(result.schema_version); // spindle.reason.v2
    for (const conclusion of result.conclusions) {
        if (conclusion.positive) {
            console.log(conclusion.conclusion_type, conclusion.literal_spl);
        }
    }
} finally {
    spindle.free();
}
```

Initialization is asynchronous; calls on an initialized `Spindle` are synchronous.
Parsing and reasoning failures throw JavaScript errors. Serve the generated assets
over HTTP with a server that serves WASM correctly.

## Reasoning output

`reason()` returns a structured `spindle.reason.v1` object, not a bare array.
`reasonV2()` returns the corresponding `spindle.reason.v2` object with typed
term arguments. Both include:

- `schema_version`, `evaluated_at`, and `grounding` statistics.
- `conclusions`, whose entries include `conclusion_type`, `literal_spl`,
  `literal_struct`, and `positive`.
- `diagnostics` and optional theory `stats`.

Use `result.conclusions` to iterate results. Use `getPositiveConclusions()` for
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

For structured predicates, arithmetic, metadata, and aggregates, use SPL input.
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
`requires` method. Use the Rust `requires_with_options` API or CLI `requires`
command for verification by full reasoning. See [Query Operators](../guides/queries.md).

## Application lifecycle

Initialize the module once, reuse a `Spindle` object for repeated runs, and call
`clear()` before rebuilding a theory programmatically. Release the instance with
`free()` when its owner is disposed. In a UI, store `reasonV2().conclusions` and
render `literal_spl`; keep initialization and error states visible to users.

Large grounding or aggregation jobs are synchronous and can block the UI thread.
Run them in a worker when needed and measure the generated bundle and workload
rather than relying on a fixed historical size estimate.
