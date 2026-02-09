# Spindle Integration Specs

This file is the entrypoint for the spindle integration documentation set.
This file is non-normative.

## Document Map

1. Contract (authoritative wire/API contract):
   - `specs/SPINDLE-CONTRACT.md`
2. Spindle-rust implementation work:
   - `specs/SPINDLE-RUST-IMPLEMENTATION.md`
3. gleg adapter and workflow integration behavior:
   - `specs/GLEG-SPINDLE-INTEGRATION.md`

## Precedence Rules

1. Wire-format and CLI behavior conflicts:
   - `specs/SPINDLE-CONTRACT.md` wins.
2. Spindle-rust implementation task sequencing conflicts:
   - `specs/SPINDLE-RUST-IMPLEMENTATION.md` wins.
3. gleg adapter/workflow behavior conflicts:
   - `specs/GLEG-SPINDLE-INTEGRATION.md` wins.

## Relationship to Existing RFCs

`specs/spindle-rust-fixes-spec.md` remains a deep technical RFC for engine correctness and parser/reasoner internals. It is implementation guidance, not the authoritative wire contract for gleg integration.

If `specs/spindle-rust-fixes-spec.md` and `specs/SPINDLE-CONTRACT.md` disagree on observable CLI/schema behavior, `specs/SPINDLE-CONTRACT.md` wins.
