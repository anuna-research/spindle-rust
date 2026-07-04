# PR Fix Notes

This document captures issues found during PR review and the suggested fixes.

> **Resolution status (2026-07-04):** All three findings are resolved in the
> restructured CLI (`crates/spindle-cli/src/cli/`):
>
> 1. **Resolved by removal** — the legacy no-subcommand path no longer exists;
>    `Cli.command` is a required subcommand (`cli/app.rs`), so there is no
>    schema-version-less `reason` path.
> 2. **Fixed** — `validate`/`stats` dispatch with `stdin || cli.stdin`
>    (`main.rs:110`, `main.rs:113`), honouring the global flag in either position.
> 3. **Fixed** — `CommandOutput::json` returns `Result` and maps serialization
>    failure to `CliError::execution("JSON_SERIALIZATION_ERROR")`; the
>    `emit_and_exit` boundary emits a JSON error envelope with a non-zero exit
>    code, including the text-under-`--json` case (`JSON_OUTPUT_EXPECTED`).
>
> The open questions below were settled by the restructure: the global `--stdin`
> is honoured everywhere, and strict JSON output is guaranteed on all `--json` paths.

## Findings

### 1) Legacy `reason` path drops `schema_version` on errors

**Impact:** JSON error envelopes for legacy `spindle [FILE]` / `spindle --stdin` (no subcommand) do not include `schema_version`, even though they are `reason` semantics. This breaks the contract rule that schema commands include `schema_version` on error.

**Where:**
- `crates/spindle-cli/src/main.rs:1206` (schema_version/json selection)
- `crates/spindle-cli/src/main.rs:1232` (defaults to `None` when no subcommand)

**Suggested fix:**
- Treat the legacy path as `reason` for contract purposes.
- When `cli.command` is `None`, set `schema_version = Some("spindle.reason.v1")`.

---

### 2) Global `--stdin` is ignored for `validate`/`stats`

**Impact:** `spindle --stdin validate` and `spindle --stdin stats` parse the global flag but dispatch with `stdin = false` (since only the subcommand flag is used), leading to `MISSING_INPUT_SOURCE`.

**Where:**
- `crates/spindle-cli/src/main.rs:349` (global `--stdin`)
- `crates/spindle-cli/src/main.rs:371` (subcommand `validate` stdin)
- `crates/spindle-cli/src/main.rs:379` (subcommand `stats` stdin)
- `crates/spindle-cli/src/main.rs:1267` (dispatch uses only subcommand stdin)

**Suggested fix options:**
- Preferred: remove the subcommand-level `--stdin` flags and use only the global one everywhere.
- Minimal: when dispatching `validate`/`stats`, use `stdin || cli.stdin` to honor either position.

---

### 3) `--json` can emit non-JSON on serialization failure

**Impact:** If `serde_json::to_value` fails (e.g., non-finite floats), `CommandOutput::json` falls back to `Text`, and the boundary prints it with exit 0. That violates the contract expectation that `--json` always emits JSON.

**Where:**
- `crates/spindle-cli/src/main.rs:116` (CommandOutput::json fallback)

**Suggested fix:**
- Convert serialization failure into a `CliError::execution` so `emit_and_exit` emits a JSON error envelope with exit code 3.

---

## Open Questions

- Should `spindle --stdin validate` and `spindle --stdin stats` be officially supported as equivalent to `spindle validate --stdin`? If yes, use the global flag consistently.
- Do we require strict JSON output guarantees for all `--json` paths, or is text fallback acceptable in the rare serialization failure case?

