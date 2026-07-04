import Lake
open Lake DSL

package SpindleLean where
  leanOptions := #[
    ⟨`pp.unicode.fun, true⟩,
    ⟨`autoImplicit, false⟩,
    ⟨`relaxedAutoImplicit, false⟩
  ]

require mathlib from git
  "https://github.com/leanprover-community/mathlib4" @ "v4.27.0"

-- IMPL-015: Core verification (types, closures, properties)
@[default_target]
lean_lib SpindleLean where
  srcDir := "."

lean_exe spindlelean where
  root := `Main

-- IMPL-016: Grounding, arithmetic, temporal, queries
lean_lib Spindle where
  roots := #[`Spindle]

lean_exe ArithOracle where
  root := `Spindle.DiffTest.ArithOracle
  supportInterpreter := true

lean_exe GroundingOracle where
  root := `Spindle.DiffTest.GroundingOracle
  supportInterpreter := true

lean_exe EndToEndOracle where
  root := `Spindle.DiffTest.EndToEndOracle
  supportInterpreter := true

lean_exe TrustOracle where
  root := `Spindle.DiffTest.TrustOracle
  supportInterpreter := true
