#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(cd -- "$script_dir/.." && pwd)"
lean_dir="$repo_root/lean"

cd "$lean_dir"

echo "==> Building Lean libraries and oracle executables with warnings as errors"
build_output="$(mktemp)"
trap 'rm -f "$build_output"' EXIT
lake build --wfail 2>&1 | tee "$build_output"
lake build --wfail spindlelean TrustOracle ArithOracle GroundingOracle EndToEndOracle 2>&1 | tee -a "$build_output"

if grep -nE "declaration uses 'sorry'|sorryAx" "$build_output"; then
  echo "error: Lean build contains admitted proofs" >&2
  exit 1
fi

echo "==> Scanning Lean source for admitted proof terms"
admitted_terms="$(
  find . -path './.lake' -prune -o -name '*.lean' -print0 \
    | xargs -0 awk '
      FNR == 1 { block = 0 }

      function strip_comments(line,    i, two, out) {
        out = ""
        for (i = 1; i <= length(line); i++) {
          two = substr(line, i, 2)
          if (block > 0) {
            if (two == "/-") {
              block++
              i++
            } else if (two == "-/") {
              block--
              i++
            }
          } else if (two == "--") {
            break
          } else if (two == "/-") {
            block++
            i++
          } else {
            out = out substr(line, i, 1)
          }
        }
        return out
      }

      {
        code = strip_comments($0)
        if (code ~ /(^|[^[:alnum:]_'\''])sorry([^[:alnum:]_'\'']|$)/ ||
            code ~ /(^|[^[:alnum:]_'\''])admit([^[:alnum:]_'\'']|$)/) {
          print FILENAME ":" FNR ":" code
        }
      }
    '
)"

if [ -n "$admitted_terms" ]; then
  echo "$admitted_terms" >&2
  echo "error: Lean source contains admitted proof terms" >&2
  exit 1
fi

echo "==> Checking for local Lean axiom/constant declarations"
local_axioms="$(
  find . -path './.lake' -prune -o -name '*.lean' -print0 \
    | xargs -0 awk '
      FNR == 1 { block = 0 }

      function strip_comments(line,    i, two, out) {
        out = ""
        for (i = 1; i <= length(line); i++) {
          two = substr(line, i, 2)
          if (block > 0) {
            if (two == "/-") {
              block++
              i++
            } else if (two == "-/") {
              block--
              i++
            }
          } else if (two == "--") {
            break
          } else if (two == "/-") {
            block++
            i++
          } else {
            out = out substr(line, i, 1)
          }
        }
        return out
      }

      {
        code = strip_comments($0)
        if (code ~ /^[[:space:]]*(axiom|constant)[[:space:]]/) {
          print FILENAME ":" FNR ":" code
        }
      }
    '
)"

if [ -n "$local_axioms" ]; then
  echo "$local_axioms" >&2
  echo "error: local Lean axiom/constant declarations are not allowed" >&2
  exit 1
fi

echo "==> Scanning for vacuous theorem statements"
# `VacuityLint.lean` is not a lake target (its `#lint` output is intentionally
# an elaboration error), so nothing else builds its `Batteries.Tactic.Lint`
# import. `lake env lean` only sets LEAN_PATH — it does not build. Build the
# module explicitly or the import fails wherever the mathlib cache is cold.
lake build Batteries.Tactic.Lint

# `lake env lean` exits non-zero when the linter reports findings, so capture
# its output rather than let `set -e` abort here.
lint_output="$(lake env lean VacuityLint.lean 2>&1 || true)"

# Guard the "#lint silently lints nothing" trap: a bare `#lint` sees only the
# current file (zero declarations) and passes vacuously. Assert that both
# packages were actually scanned before trusting a clean result.
for pkg in Spindle SpindleLean; do
  if ! printf '%s\n' "$lint_output" | grep -q "in $pkg with 2 linters"; then
    printf '%s\n' "$lint_output" >&2
    echo "error: vacuity linter did not run over $pkg" >&2
    exit 1
  fi
done

# Derived `Repr` instances always report an unused precedence argument.
lint_hits="$(printf '%s\n' "$lint_output" | grep '^#check' | grep -v 'instRepr' || true)"

if [ -n "$lint_hits" ]; then
  printf '%s\n' "$lint_hits" >&2
  echo "error: Lean declaration has a tautological statement or an unused hypothesis" >&2
  echo "hint: fix the statement, or tag it @[nolint unusedArguments] with a reason" >&2
  exit 1
fi

echo "==> Running Lean axiom audit"
audit_output="$(mktemp)"
trap 'rm -f "$build_output" "$audit_output"' EXIT
lake env lean AxiomAudit.lean | tee "$audit_output"

awk '
BEGIN {
  allowed["propext"] = 1
  allowed["Classical.choice"] = 1
  allowed["Quot.sound"] = 1
  collecting = 0
  failed = 0
}

function trim(s) {
  gsub(/^[[:space:]]+|[[:space:]]+$/, "", s)
  return s
}

function check_axioms(text,    n, parts, i, axiom) {
  gsub(/^[[:space:]]*\[/, "", text)
  gsub(/\][[:space:]]*$/, "", text)
  n = split(text, parts, ",")
  for (i = 1; i <= n; i++) {
    axiom = trim(parts[i])
    if (axiom != "" && !(axiom in allowed)) {
      print "error: unexpected Lean axiom in AxiomAudit output: " axiom > "/dev/stderr"
      failed = 1
    }
  }
}

/does not depend on any axioms/ {
  next
}

collecting {
  buffer = buffer " " $0
  if ($0 ~ /\]/) {
    check_axioms(buffer)
    collecting = 0
    buffer = ""
  }
  next
}

/depends on axioms:/ {
  sub(/^.*depends on axioms:[[:space:]]*/, "", $0)
  buffer = $0
  if ($0 ~ /\]/) {
    check_axioms(buffer)
    buffer = ""
  } else {
    collecting = 1
  }
}

END {
  if (collecting) {
    print "error: unterminated axiom list in AxiomAudit output" > "/dev/stderr"
    failed = 1
  }
  exit failed
}
' "$audit_output"

echo "==> Lean verification gate passed: no admitted proofs or nonstandard axioms"
