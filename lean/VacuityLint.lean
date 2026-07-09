/-
  Vacuity linter for the spindle formalisation.

  Run by `scripts/check-lean-verification.sh`. Catches the failure mode where a
  proof is valid but the *statement* says nothing about the objects it names:

    synTaut         — the statement is a syntactic tautology
    unusedArguments — a binder the proof never consumes, which usually means
                      the theorem is not about what its name claims

  IMPORTANT: bare `#lint` only sees declarations in the CURRENT file, of which
  this file has none — it would report "0 declarations … All linting checks
  passed!" and gate nothing. The `in <Package>` clause is what makes it lint the
  libraries. The gate script asserts both packages were actually scanned.

  Derived `Repr` instances always report an unused precedence argument; the gate
  script filters `instRepr*` out. Deliberate unused hypotheses (e.g. ones that
  pin an operator's domain) are tagged `@[nolint unusedArguments]` at the
  declaration with a comment explaining why.
-/
import SpindleLean
import Spindle
import Batteries.Tactic.Lint

open Batteries.Tactic.Lint

#lint only synTaut unusedArguments in Spindle
#lint only synTaut unusedArguments in SpindleLean
