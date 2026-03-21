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
