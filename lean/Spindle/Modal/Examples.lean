import Spindle.Modal.Properties

namespace Spindle.Modal.Examples

def permission : Lit := { atom := "play", mode := .permission }
def prohibition : Lit := { atom := "play", mode := .forbidden }
def obligation : Lit := { atom := "play", mode := .obligation }

def preferredPermission : MTheory := ⟨[
  ⟨"no-play", .defeasible, [], prohibition⟩,
  ⟨"hat-play", .defeasible, [{ atom := "hat" }], permission⟩,
  ⟨"hat", .fact, [], { atom := "hat" }⟩], [("hat-play", "no-play")]⟩

set_option maxRecDepth 100000 in
set_option maxHeartbeats 4000000 in
theorem preferred_permission_wins :
    has (reason preferredPermission) .defeasiblyProvable permission = true ∧
    has (reason preferredPermission) .defeasiblyNotProvable prohibition.normalize = true := by
  decide

def oppositePermissions : MTheory := ⟨[
  ⟨"yes", .defeasible, [], permission⟩,
  ⟨"no", .defeasible, [], { permission with inner := true }⟩], []⟩

set_option maxRecDepth 100000 in
set_option maxHeartbeats 4000000 in
theorem opposite_permissions_both_provable :
    has (reason oppositePermissions) .defeasiblyProvable permission = true ∧
    has (reason oppositePermissions) .defeasiblyProvable { permission with inner := true } = true := by
  decide

def weakPermission : MTheory := ⟨[
  ⟨"yes", .defeasible, [], obligation⟩,
  ⟨"no", .defeasible, [], prohibition⟩,
  ⟨"weak", .defeasible, [prohibition.negate], { atom := "allowed" }⟩], []⟩

set_option maxRecDepth 100000 in
set_option maxHeartbeats 4000000 in
theorem weak_permission_is_not_strong :
    has (reason weakPermission) .defeasiblyProvable { atom := "allowed" } = true ∧
    has (reason weakPermission) .defeasiblyProvable permission = false := by
  decide

end Spindle.Modal.Examples
