# AGENTS.md

Agent-focused guidance for the **spindle-rust** workspace — a defeasible logic
reasoning engine (Rust port of SPINdle).

## Project structure

Cargo workspace with five crates under `crates/`:

| Crate | Purpose |
|---|---|
| `spindle-core` | Core reasoning engine, theory types, algorithms |
| `spindle-parser` | Lexer/parser for `.dfl` / `.spl` theory files |
| `spindle-cli` | Command-line interface (`spindle` binary) |
| `spindle-contract` | JSON contract schemas for CLI I/O |
| `spindle-wasm` | WebAssembly bindings via `wasm-pack` |

Shared settings live in the root `Cargo.toml` (`[workspace.package]`,
`[workspace.dependencies]`). Rust **edition 2024** — requires toolchain 1.87+.

## Setup commands

```sh
make build          # cargo build --all
make install        # install the CLI locally (cargo install)
make wasm           # build WASM for web target
make doc-open       # generate & open rustdoc
```

No additional system dependencies beyond a Rust 1.87+ toolchain. WASM builds
additionally require `wasm-pack`.

## Testing instructions

```sh
make test           # cargo test --all
make check          # format check + clippy (zero warnings policy)
make bench          # criterion benchmarks (spindle-core, ~1-2 min)
make bench-scaling  # large-scale benchmarks (algorithm crossover points)
```

- Always run `make check` before committing — CI will reject format or clippy
  failures.
- CI runs on Woodpecker (`.woodpecker/ci.yaml`): fmt → clippy → test → wasm
  build, using `rust:1.88-bookworm`.
- When adding or changing behavior, add or update tests following existing
  patterns in the same crate.

## Code style

- **Formatting**: `cargo fmt` (rustfmt defaults). Run `make fmt-fix` to
  auto-format.
- **Linting**: `cargo clippy --all -- -D warnings`. Zero warnings policy — do
  not add `#[allow(...)]` without justification.
- **Naming & layout**: follow the conventions already present in the crate you
  are modifying. Do not reorganize modules unless specifically asked.
- **Dependencies**: prefer workspace-level deps in root `Cargo.toml`. Only add
  new crates when clearly justified.
- **Documentation**: keep public API docs accurate. If you change a public
  function signature, update its doc comment.

## Commit & PR guidelines

- Use **semantic commit messages**: `feat:`, `fix:`, `refactor:`, `chore:`,
  `docs:`, `test:`, `perf:`.
- Pre-commit checklist:
  1. `make fmt-fix`
  2. `make check` (fmt + clippy pass)
  3. `make test` (all tests pass)
- Keep commits focused — one logical change per commit.
- PR titles should be concise and descriptive (under 70 characters).

## Development tips

- The `examples/` directory contains sample `.dfl` theory files useful for
  manual testing.
- Benchmarks use Criterion; results go to `target/criterion/`. Use
  `make bench-compare` to diff between commits.
- Memory profiling: `make bench-memory` generates `dhat-heap.json`.
- For WASM work, additional targets: `make wasm-node`, `make wasm-bundler`.

## Security considerations

- Do not commit secrets, tokens, or credentials. The `private/` directory is
  gitignored for local-only files.
- The project is licensed LGPL-3.0-or-later — be mindful of dependency
  licenses when adding crates.
