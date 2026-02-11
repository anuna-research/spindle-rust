# WebAssembly Integration

Spindle compiles to WebAssembly for use in browsers and Node.js.

## Building

### Prerequisites

```bash
cargo install wasm-pack
```

### Build for Web

```bash
cd crates/spindle-wasm
wasm-pack build --target web --release
```

### Build for Node.js

```bash
wasm-pack build --target nodejs --release
```

### Build for Bundlers (webpack, etc.)

```bash
wasm-pack build --target bundler --release
```

## Installation

### npm/yarn

```bash
npm install spindle-wasm
# or
yarn add spindle-wasm
```

### From Local Build

```javascript
// Point to your build output
import init, { Spindle } from './pkg/spindle_wasm.js';
```

## Browser Usage

### ES Modules

```html
<script type="module">
import init, { Spindle } from './pkg/spindle_wasm.js';

async function main() {
    await init();

    const spindle = new Spindle();
    spindle.addFact("bird");
    spindle.addFact("penguin");
    spindle.addDefeasibleRule(["bird"], "flies");
    spindle.addDefeasibleRule(["penguin"], "~flies");
    spindle.addSuperiority("r2", "r1");

    const conclusions = spindle.reason();
    console.log(conclusions);
}

main();
</script>
```

### With Bundler (Vite, webpack)

```typescript
import init, { Spindle } from 'spindle-wasm';

async function setup() {
    await init();
    return new Spindle();
}

const spindle = await setup();
```

## Node.js Usage

```javascript
const { Spindle } = require('spindle-wasm');

const spindle = new Spindle();

spindle.parseSpl(`
    f1: >> bird
    f2: >> penguin
    r1: bird => flies
    r2: penguin => ~flies
    r2 > r1
`);

const conclusions = spindle.reason();
console.log(conclusions);
```

## API Reference

### Constructor

```typescript
const spindle = new Spindle();
```

Creates a new empty theory.

### Adding Facts

```typescript
spindle.addFact("bird");
spindle.addFact("~guilty");  // Negated fact
```

### Adding Rules

```typescript
// Defeasible rules
spindle.addDefeasibleRule(["bird"], "flies");
spindle.addDefeasibleRule(["bird", "healthy"], "strong_flyer");

// Strict rules
spindle.addStrictRule(["penguin"], "bird");

// Defeaters
spindle.addDefeater(["broken_wing"], "flies");
```

### Superiority

```typescript
spindle.addSuperiority("r2", "r1");  // r2 > r1
```

### Parsing

```typescript
// Parse SPL
spindle.parseSpl(`
    f1: >> bird
    r1: bird => flies
`);

// Parse SPL
spindle.parseSpl(`
    (given bird)
    (normally r1 bird flies)
`);
```

### Reasoning

```typescript
const conclusions = spindle.reason();
// Returns: Array of conclusion objects

// Each conclusion:
{
    conclusion_type: "+D" | "-D" | "+d" | "-d",
    literal: string,
    positive: boolean
}
```

### Query

```typescript
// Check if a literal is provable
const result = spindle.query("flies");
// Returns: { status: "provable" | "not_provable", literal, conclusion_type }
```

### What-If

```typescript
const result = spindle.whatIf(["wounded"], "~flies");
// Returns: { provable: boolean, new_conclusions: Array, changed_conclusions: Array }
```

### Why-Not

```typescript
const explanation = spindle.whyNot("flies");
// Returns: { literal, is_provable, would_derive, blockers: Array }
```

### Abduction

```typescript
const solutions = spindle.abduce("goal", 3);
// Returns: { goal, solutions: Array<{ facts: string[], rules_used: string[], confidence: number }> }
```

### Reset

```typescript
spindle.clear();  // Clear all rules and facts
```

## TypeScript Types

```typescript
interface Conclusion {
    conclusion_type: "+D" | "-D" | "+d" | "-d";
    literal: string;
    positive: boolean;
}

interface QueryResult {
    status: "provable" | "not_provable";
    literal: string;
    conclusion_type?: string;
}

interface WhatIfResult {
    provable: boolean;
    new_conclusions: Conclusion[];
    changed_conclusions: ChangedConclusion[];
}

interface WhyNotExplanation {
    literal: string;
    is_provable: boolean;
    would_derive: string | null;
    blockers: Blocker[];
}

interface Blocker {
    blocking_type: "MissingPremise" | "Defeated" | "Contradicted";
    rule_label: string;
    blocking_rule: string | null;
    explanation: string;
}

interface AbductionResult {
    goal: string;
    solutions: AbductionSolution[];
}

interface AbductionSolution {
    facts: string[];
    rules_used: string[];
    confidence: number;
}

interface ChangedConclusion {
    literal: string;
    old_type: string;
    new_type: string;
}
```

## Complete Example

```typescript
import init, { Spindle } from 'spindle-wasm';

async function reasonAboutPenguins() {
    await init();

    const spindle = new Spindle();

    // Build theory
    spindle.parseSpl(`
        f1: >> bird
        f2: >> penguin

        r1: bird => flies
        r2: bird => has_feathers
        r3: penguin => ~flies
        r4: penguin => swims

        r3 > r1
    `);

    // Reason
    const conclusions = spindle.reason();

    // Filter positive conclusions
    const positive = conclusions.filter(c =>
        c.conclusion_type === "+D" || c.conclusion_type === "+d"
    );

    console.log("Provable:", positive.map(c => c.literal));

    // Query specific literal
    const fliesResult = spindle.query("flies");
    console.log("Does it fly?", fliesResult.status);

    // What-if analysis
    const whatIf = spindle.whatIf(["super_bird"], "flies");
    console.log("With super_bird:", whatIf.provable);

    // Explain why not
    if (fliesResult.status === "not_provable") {
        const explanation = spindle.whyNot("flies");
        console.log("Why not flies:", explanation.blockers);
    }
}

reasonAboutPenguins();
```

## React Integration

```tsx
import { useState, useEffect } from 'react';
import init, { Spindle } from 'spindle-wasm';

function useSpindle() {
    const [spindle, setSpindle] = useState<Spindle | null>(null);
    const [loading, setLoading] = useState(true);

    useEffect(() => {
        init().then(() => {
            setSpindle(new Spindle());
            setLoading(false);
        });
    }, []);

    return { spindle, loading };
}

function ReasoningComponent() {
    const { spindle, loading } = useSpindle();
    const [conclusions, setConclusions] = useState([]);

    const reason = () => {
        if (!spindle) return;

        spindle.reset();
        spindle.addFact("bird");
        spindle.addDefeasibleRule(["bird"], "flies");

        const result = spindle.reason();
        setConclusions(result);
    };

    if (loading) return <div>Loading WASM...</div>;

    return (
        <div>
            <button onClick={reason}>Reason</button>
            <ul>
                {conclusions.map((c, i) => (
                    <li key={i}>{c.conclusion_type} {c.literal}</li>
                ))}
            </ul>
        </div>
    );
}
```

## Performance Notes

1. **Initialize once**: Call `init()` once at startup
2. **Reuse Spindle instances**: Create once, reset between uses
3. **Batch operations**: Add all rules before reasoning
4. **Use explicit superiority for conflict-heavy theories** to avoid unresolved ties

## Bundle Size

Approximate sizes (gzipped):
- Core WASM: ~150KB
- JavaScript bindings: ~10KB

## Browser Compatibility

Requires WebAssembly support:
- Chrome 57+
- Firefox 52+
- Safari 11+
- Edge 16+
- Node.js 8+
