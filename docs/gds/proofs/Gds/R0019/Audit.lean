import Gds.R0019.FileLock

/-!
# The axioms the theorems of GDS 0019 depend on

`cargo xtask proofs` compiles this file and reads what it prints. A theorem may depend on
Lean's three standard axioms (`propext`, `Classical.choice`, `Quot.sound`) and on nothing
else; `sorryAx` marks an unfinished proof and fails the check.
-/

open Gds.R0019.FileLock

#print axioms inv_reachable
#print axioms mutual_exclusion
#print axioms no_false_busy
#print axioms no_false_busy_alive
#print axioms held_is_refused
#print axioms reader_sees_whole
#print axioms no_lost_update
#print axioms dead_holds_nothing
#print axioms lock_is_free
#print axioms taker_succeeds
#print axioms dead_copies_never_grow
#print axioms crash_recovers
#print axioms close_alone_refuses_falsely
