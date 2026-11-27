import Gds.R0020.Schedule
import Gds.R0020.Paths
import Gds.R0020.SharedFile

/-!
# The axioms the theorems of GDS 0020 depend on

`cargo xtask proofs` compiles this file and reads what it prints. A theorem may depend on
Lean's three standard axioms (`propext`, `Classical.choice`, `Quot.sound`) and on nothing
else; `sorryAx` marks an unfinished proof and fails the check.
-/

open Gds.R0020

#print axioms Schedule.inv_reachable
#print axioms Schedule.memory_bound
#print axioms Schedule.memory_bound_fitting
#print axioms Schedule.slots_bound
#print axioms Schedule.runs_are_short
#print axioms Schedule.ended_all_succeeded
#print axioms Schedule.ended_first_failure
#print axioms Schedule.ended_none_fails
#print axioms Schedule.idle_start
#print axioms Paths.split_unique
#print axioms Paths.names_fold
#print axioms Paths.jobs_disjoint
#print axioms Paths.jobs_disjoint_exact
#print axioms Paths.names_differ
#print axioms SharedFile.reader_sees_whole_file
#print axioms SharedFile.renamed_holds_a_value
#print axioms SharedFile.same_value
