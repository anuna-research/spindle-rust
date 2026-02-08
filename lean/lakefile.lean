import Lake
open Lake DSL

package SpindleLean where
  leanOptions := #[
    ⟨`autoImplicit, false⟩
  ]

require mathlib from git
  "https://github.com/leanprover-community/mathlib4" @ "v4.27.0"

@[default_target]
lean_lib SpindleLean where
  srcDir := "."

lean_exe spindlelean where
  root := `Main
