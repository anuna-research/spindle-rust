import Lake
open Lake DSL

package SpindleArith where
  leanOptions := #[
    ⟨`pp.unicode.fun, true⟩,
    ⟨`relaxedAutoImplicit, false⟩
  ]

@[default_target]
lean_lib Spindle where
  roots := #[`Spindle]

lean_exe ArithOracle where
  root := `Spindle.DiffTest.ArithOracle
  supportInterpreter := true

lean_exe GroundingOracle where
  root := `Spindle.DiffTest.GroundingOracle
  supportInterpreter := true
