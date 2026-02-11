# Agent Prompt: Error Module Implementation

You are implementing the Spindle error module plan. Your work is tracked in a defeasible logic plan file at `plans/error-module.spl` and on hence.run.

## Project

Spindle is a Rust defeasible logic reasoning engine. It is a Cargo workspace with four crates:

- `spindle-core` — reasoning algorithms, pipeline, error types, query engine
- `spindle-parser` — SPL format parsers, parser error types
- `spindle-cli` — command-line interface (clap-based, ~1400-line main.rs)
- `spindle-wasm` — WebAssembly bindings

You are on branch `feature/error-module`.

## Specifications

Read these specs before starting work:

1. `specs/IMPL-010-spindle-simplification-cleanup.md` — Cleanup plan (Phases A–E): spec hygiene, CLI decomposition, shared contract crate, error module enablement, CI simplification.
2. `specs/ERROR-MODULE-SPEC.md` — SPEC-010 v0.6.0: RFC 9457 Problem Details error model, error taxonomy, rendering rules, message quality principles, testing strategy.

## Plan Management

The plan is managed with the `hence` CLI tool. The plan file is `plans/error-module.spl`.

### Key commands

```bash
# See what's ready to work on
hence next plans/error-module.spl

# See full board state
hence board plans/error-module.spl

# Read a task's metadata (description, target files, requirements)
hence describe plans/error-module.spl task-<name>

# Claim a task (mark in-progress)
hence claim plans/error-module.spl <task-name>

# Mark a task complete
hence complete plans/error-module.spl <task-name>

# See what's blocked and why
hence why-not plans/error-module.spl ready-<task-name>

# See overall status
hence status plans/error-module.spl
```

### Remote plan (hence.run)

The plan is also hosted remotely. You can use the remote URL interchangeably with the local file path:

- Append (agents): `https://hence.run/p/cphz_BczTmiFzAgFwq4BTAEAAAAAaZPADKHtiZXnSNpJ0ZpbVpm9jW1myhAVeWaPP6Lm4UZOd1rR`

```bash
hence complete https://hence.run/p/cphz_BczTmiFzAgFwq4BTAEAAAAAaZPADKHtiZXnSNpJ0ZpbVpm9jW1myhAVeWaPP6Lm4UZOd1rR <task-name>
```

Always update both local and remote when completing tasks.

## Workflow

For each task:

1. **Check what's ready:** `hence next plans/error-module.spl`
2. **Read the task metadata:** `hence describe plans/error-module.spl task-<name>` — this gives you the description, target files, requirements it implements, and effort estimate.
3. **Claim it:** `hence claim plans/error-module.spl <task-name>`
4. **Read the target files** before making changes. Understand existing code and patterns.
5. **Implement the task.** Follow the spec requirements listed in the task's `implements` or `verifies` metadata. Keep changes minimal and focused.
6. **Run tests:** `cargo test -p <crate>` for the affected crate. All tests must pass.
7. **Complete it:** `hence complete plans/error-module.spl <task-name>` (local) and also update remote.
8. **Check the board:** `hence next plans/error-module.spl` — completing a task may unblock others.

## Phase Overview

| Phase | PR | What |
|---|---|---|
| **A** (3 tasks) | PR-1 | Fix spec cross-references, fix woodpecker `--format json` → `--json`, document contract crate design |
| **B** (8 tasks) | PR-2 | Decompose `spindle-cli/src/main.rs` into `cli/{app,input,output,error,commands}` modules, fix double-prepare bug, verify behavior parity |
| **C** (4 tasks) | PR-3 | Create `spindle-contract` crate, migrate duplicate DTOs from CLI and WASM, add parity tests |
| **D1** (6 tasks) | PR-4a | Add `ErrorCategory` enum, `code()`/`category()` methods, `#[non_exhaustive]`, `From<ParseError>`, `Send+Sync+'static` assertions |
| **D2** (6 tasks) | PR-4b | Add `ProblemDetails`, `SourceContext`, `ErrorReport`, `Diagnostic`, `From` conversions |
| **D3** (5 tasks) | PR-4c | Implement `render_human()`, `render_json()`, replace `CliError`, add `--debug-errors` and `--explain CODE` |
| **D4** (1 task) | PR-4d | Wire WASM error rendering through `ProblemDetails` |
| **D5** (12 tasks) | PR-4e | Full test suite: construction, JSON envelope, redaction, taxonomy, source chain, non-exhaustive, send+sync, source context, uniform format, hint quality, tone, title/detail |
| **E** (3 tasks) | PR-5 | CI contract gates, reduce test overlap, validate doc examples |

## Critical Rules

1. **No behavior changes during Phase B.** The CLI decomposition must produce identical stdout/stderr/exit-code for all inputs. This is a pure structural refactor.
2. **Existing tests must stay green.** Run `cargo test --workspace` before committing. Never skip failing tests.
3. **Follow the spec's error taxonomy exactly.** Exit codes, error codes, and category mappings are defined in SPEC-010 Section 10.2 and must match precisely.
4. **RFC 9457 compliance.** `ProblemDetails` must follow the RFC structure. `status` field is intentionally omitted (CLI, not HTTP). `type` uses `tag:spindle.dev,2026:error:{CODE}` URI scheme.
5. **Library vs presentation boundary.** Core/parser crates return typed `Result<T, E>`. Presentation crates (CLI/WASM) convert to `ProblemDetails` at the boundary. Never put `ProblemDetails` in library crates.
6. **Message quality.** User-facing text must follow the 13 principles in SPEC-010 Section 11.1: no jargon, no blame, honest uncertainty, actionable hints, uniform format, state the violated constraint.
7. **Commit after each task or logical group.** Use descriptive commit messages. Push to `feature/error-module`.

## Key Files

- `crates/spindle-core/src/error.rs` — `SpindleError` enum (phases D1)
- `crates/spindle-parser/src/error.rs` — `ParseError` enum (phases D1)
- `crates/spindle-cli/src/main.rs` — monolithic CLI entry point (phases B, D3)
- `crates/spindle-wasm/src/lib.rs` — WASM bindings with duplicate DTOs (phases C, D4)
- `crates/spindle-core/src/reason.rs` — `reason()` with double-prepare bug (phase B)
- `.woodpecker/release.yaml` — stale `--format json` flag (phase A)
- `specs/ERROR-MODULE-SPEC.md` — full error module specification
- `specs/IMPL-010-spindle-simplification-cleanup.md` — cleanup plan

## Getting Started

```bash
cd /Users/anuna-02/Code/spindle-rust
git checkout feature/error-module
hence board plans/error-module.spl
hence next plans/error-module.spl
```

Phase A has 3 independent tasks ready now: `fix-contract-refs`, `fix-woodpecker-flags`, `plan-contract-crate`. Start there.
