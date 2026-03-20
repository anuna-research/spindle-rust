import SpindleLean.Literal

/-!
# SpindleLean.Conclusion

Defines the `ConclusionType` inductive and the `Conclusion` type for defeasible logic.

## Rust correspondence

`ConclusionType` corresponds to `crates/spindle-core/src/conclusion.rs :: ConclusionType`.

| Lean variant   | Rust variant                | Symbol | Notes                              |
|---------------|-----------------------------|--------|------------------------------------|
| `plusD`       | `DefinitelyProvable`        | `+D`   | Proven via facts and strict rules  |
| `minusD`      | `DefinitelyNotProvable`     | `-D`   | Cannot be proven definitely        |
| `plusd`       | `DefeasiblyProvable`        | `+d`   | Proven defeasibly, not defeated    |
| `minusd`      | `DefeasiblyNotProvable`     | `-d`   | Cannot be proven defeasibly        |

`Conclusion` corresponds to `crates/spindle-core/src/conclusion.rs :: Conclusion`,
stripped to the two fields relevant for propositional DL(d) verification:

| Lean field         | Rust field         | Notes                                    |
|-------------------|--------------------|------------------------------------------|
| `conclusionType`  | `conclusion_type`  | The strength/polarity of the conclusion  |
| `literal`         | `literal`          | The literal this conclusion is about     |

The Rust struct carries an additional `rule_label` field that is orthogonal to the
core defeasible-logic properties verified here and is therefore omitted.
-/

/-- The type of a conclusion in defeasible logic, indicating both
    the strength (definite vs defeasible) and polarity (provable vs not provable). -/
inductive ConclusionType where
  /-- Definitely provable (`+D`): derived via facts and strict rules. -/
  | plusD
  /-- Definitely not provable (`-D`): cannot be proven via strict rules. -/
  | minusD
  /-- Defeasibly provable (`+d`): derived via defeasible rules, not defeated. -/
  | plusd
  /-- Defeasibly not provable (`-d`): cannot be proven defeasibly. -/
  | minusd
  deriving DecidableEq, BEq, Hashable

namespace ConclusionType

/-- The symbol string for a conclusion type, matching the Rust `symbol()` method. -/
def symbol : ConclusionType → String
  | plusD  => "+D"
  | minusD => "-D"
  | plusd  => "+d"
  | minusd => "-d"

/-- Whether the conclusion is positive (provable). -/
def isPositive : ConclusionType → Bool
  | plusD  => true
  | plusd  => true
  | _      => false

/-- Whether the conclusion is definite (as opposed to defeasible). -/
def isDefinite : ConclusionType → Bool
  | plusD  => true
  | minusD => true
  | _      => false

instance : Repr ConclusionType where
  reprPrec ct _ := ct.symbol

instance : ToString ConclusionType where
  toString ct := ct.symbol

end ConclusionType

/-- A conclusion: a conclusion type paired with the literal it is about. -/
structure Conclusion where
  /-- The strength/polarity of the conclusion. -/
  conclusionType : ConclusionType
  /-- The literal this conclusion is about. -/
  literal        : Literal
  deriving DecidableEq, BEq

namespace Conclusion

instance : Repr Conclusion where
  reprPrec c _ := c.conclusionType.symbol ++ " " ++ repr c.literal

instance : ToString Conclusion where
  toString c := c.conclusionType.symbol ++ " " ++ repr c.literal |>.pretty

end Conclusion
