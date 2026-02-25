# Hence Plan: Missing Retry Mechanism

## What happened

In `plans/arithmetic-module.spl`, the `tests-grounding` task was delegated to
an agent, the lease expired, and the supervisor marked it `stale-v1`. The
evaluator then auto-failed it with confidence 0.00. Because the plan had no
retry/reset path, the task became permanently stuck:

1. `failed-tests-grounding` (given fact from evaluator) — terminal state
2. `stale-v1-tests-grounding` (given fact from supervisor) — defeated the claim chain
3. Downstream propagation rules fired, blocking `tests-nfr`, `tests-pbt`,
   `tests-worked-examples`, and `v2-json-schema`

The readiness rule (`r-ready-tests-grounding`) still had all conditions met,
but the supervisor wouldn't re-delegate because of the `failed-*` marker.

## Workaround applied

A manual supervisor claim (`retry-v1-tests-grounding`) was appended to defeat
the failure/stale states and unblock downstream propagation via `prefer`
statements. See commit `18af700`.

## What to fix

### 1. Plan generator should emit retry scaffolding per task

Every task should have a retry rule template baked in at generation time:

```spl
;; Generated per-task retry scaffold
(normally r-retry-not-failed-{task} retry-{task} (not failed-{task}))
(normally r-retry-not-stale-{task} retry-{task} (not stale-v1-{task}))
```

This way, asserting `(given retry-{task})` in a later supervisor claim is all
that's needed — no manual rule authoring.

### 2. Supervisor should auto-retry on confidence 0.00

A stale-then-auto-failed task (confidence 0.00) is a timeout, not a real
failure. The supervisor should distinguish this from a genuine evaluation
failure (confidence > 0) and automatically assert `retry-*` for timeouts,
up to a configurable max-retries count.

### 3. Downstream propagation should be conditional on retry state

The propagation rules (`r-propagate-*-from-*-failed`) should have a guard:

```spl
(normally r-propagate-X-from-Y-failed
  (and failed-Y (not retry-Y))
  upstream-blocked-X)
```

This avoids needing per-downstream `prefer` statements in every retry claim.

### 4. Consider a `max-retries` counter

To prevent infinite retry loops, track attempt count and escalate to
`permanently-failed-*` after N attempts (e.g., 3).
