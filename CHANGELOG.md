# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows pre-1.0 Semantic Versioning (`0.y.z`).

## [Unreleased]

### Added
- Verified `requires` core API:
  - `requires_with_options(theory, goal, options)`
  - `RequiresOptions`, `RequiresResult`, `RequiresSearchStatus`, `RequiresVerificationStats`
- New CLI contract schema: `spindle.requires.v2`.
- New core test suite: `crates/spindle-core/tests/requires_verified_tests.rs`.

### Changed
- `requires` is now verified-by-default in core and CLI.
- `requires --json` emits `spindle.requires.v2` only.
- `spindle capabilities --json` now advertises `schemas.requires = spindle.requires.v2`.
- Core `requires()` compatibility wrapper now delegates to verified logic.

### Fixed
- Eliminated false-positive `requires` candidates that fail under full defeasible reasoning.
- Corrected `BudgetExhausted` classification for duplicate raw-candidate edge cases.
- Added defensive collision handling for injected verification fact labels.

### Migration
- Clients validating `requires` JSON should migrate from `spindle.requires.v1` to `spindle.requires.v2`.
- In v2, `satisfied=false` with `solutions=[]` is valid.
