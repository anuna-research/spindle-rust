# How to run a theory in the browser

This guide uses the web target of `spindle-wasm`.

1. Install `wasm-pack`.
2. From the repository root, build the web target:

   ```sh
   make wasm
   ```

3. Copy the generated files from `crates/spindle-wasm/pkg` into your application's `pkg` directory.
4. Serve your application over HTTP with a server that serves WASM correctly.
5. Run this code from a JavaScript module beside `pkg`:

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

The console shows `spindle.reason.v2` and positive conclusions, including `(total-payment 20)`.

## Repeated runs

Initialize the module once with `await init()`.
Reuse a `Spindle` object while its owner remains active.
Before rebuilding a theory programmatically, call `clear()`.
When its owner is disposed, call `free()`.
The example's `finally` block releases its single-use instance even after an error.

## User interfaces

Store `reasonV2().conclusions` in application state.
Render each conclusion's `literal_spl` value.
Display initialization progress while `init()` runs.
Catch parsing and reasoning errors at the application boundary.
Display those errors to the user.

Calls on an initialized instance run synchronously.
During representative grounding and aggregation jobs, measure input responsiveness and rendering delays.
If these calls interrupt interaction or animation, run them in a worker.
Measure the generated bundle size for your build; historical size estimates do not describe every application.

The [WebAssembly reference](../integration/wasm.md) describes methods, return values, and build targets.
