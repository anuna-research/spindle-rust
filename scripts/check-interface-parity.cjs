// Usage: cargo build -p spindle-cli
// wasm-pack build crates/spindle-wasm --target nodejs --out-dir pkg-node
// node scripts/check-interface-parity.cjs [path/to/spindle_wasm.js] [path/to/spindle]
const assert = require('node:assert/strict');
const {readFileSync} = require('node:fs');
const {resolve} = require('node:path');
const {execFileSync} = require('node:child_process');
const {Spindle} = require(resolve(process.argv[2] || 'crates/spindle-wasm/pkg-node/spindle_wasm.js'));
const cliPath = resolve(process.argv[3] || 'target/debug/spindle');
const root = resolve(__dirname, '..');
const read = path => readFileSync(resolve(root, path), 'utf8');
function cli(source, ...args) {
  return JSON.parse(execFileSync(cliPath, [...args, '--stdin', '--json'], {input: source, encoding: 'utf8'}));
}
let comparisons = 0;
function equal(actual, expected, context) {
  assert.deepEqual(actual, expected, context);
  comparisons++;
}
const engine = new Spindle();
try {
  for (const file of ['examples/aggregation.spl', 'examples/trust-diminishment.spl']) {
    const source = read(file);
    engine.parseSpl(source);
    equal(engine.reason(), cli(source, 'reason'), `${file}: v1`);
    equal(engine.reasonV2(), cli(source, 'reason', '--v2'), `${file}: v2`);
    equal(engine.vocabulary(), cli(source, 'vocabulary'), `${file}: vocabulary`);
    if (file.includes('trust')) {
      equal(engine.reasonWithTrust(false), cli(source, 'reason', '--trust'), 'trust v1');
      equal(engine.reasonWithTrust(true), cli(source, 'reason', '--trust', '--v2'), 'trust v2');
    }
  }
  let source = read('crates/spindle-core/tests/fixtures/issue_37_witness.spl');
  engine.parseSpl(source);
  equal(engine.explain('(tests-passed rev-b)'), cli(source, 'explain', '(tests-passed rev-b)'), 'grounded proof');
  equal(engine.explain('missing'), cli(source, 'explain', 'missing'), 'missing proof');
  for (source of ['(normally r p q)', '(normally r p q) (given (not q))', '(given q)', '(normally r p q) (normally s x q)']) {
    engine.parseSpl(source);
    equal(engine.requires('q', 1), cli(source, 'requires', 'q', '--max', '1'), 'verified requirements');
    equal(engine.abduce('q', 2), cli(source, 'abduce', 'q', '--max', '2'), 'raw candidates');
    equal(engine.whatIf(['p'], 'q'), cli(source, 'what-if', 'q', '--given', 'p'), 'what-if');
  }
  source = '(given (p 2)) (given (during q 100 200))';
  engine.parseSpl(source);
  for (const goal of ['p(2)', '(p 2)', '(during q 100 200)']) {
    equal(engine.query(goal).status, cli(source, 'query', goal).status, `query ${goal}`);
  }
  const at = '1970-01-01T00:00:00.150Z';
  engine.setReferenceTime(at);
  equal(engine.reasonV2(), cli(source, 'reason', '--v2', '--at', at), 'reference time');
  engine.setReferenceTime(null);
  engine.registerExtensions(read('examples/lookup-functions.json'));
  source = read('examples/lookup-functions.spl');
  engine.parseSpl(source);
  const extensions = resolve(root, 'examples/lookup-functions.json');
  equal(engine.reasonV2(), cli(source, 'reason', '--v2', '--extensions', extensions), 'portable extensions');
  equal(engine.query('(classification-of alice small)').status,
    cli(source, 'query', '(classification-of alice small)', '--extensions', extensions).status, 'extension query');
  console.log(`${comparisons} CLI/WASM parity comparisons passed`);
} finally {
  engine.free();
}
