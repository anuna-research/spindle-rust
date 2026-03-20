import SpindleLean.DiffTest.Oracle

/-- Lean oracle executable for differential testing.
    Reads a JSON-encoded theory from stdin, computes all conclusions
    via `reason`, and writes JSON conclusions to stdout. -/
def main : IO Unit :=
  SpindleLean.DiffTest.oracleMain
