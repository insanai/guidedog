/-!
# A file that several jobs write with the same bytes (GDS 0020)

Two jobs of a site build may write one file outside their own folders: a compiled
translation catalog (`.mo`) beside its `.po` in the source tree, when both jobs build the
same project in the same language (the HTML and the PDF edition). `gd_host::publish`
writes it: it creates a stage file with a name nobody else uses (`create_new`), writes
the bytes there, and renames it over the file.

The bytes a writer writes are a function of the `.po` file, which no job writes, and of
the writer's settings (`gettext_allow_fuzzy_translations`), so writer `w` writes a value
`v w` of its own. We prove that a reader of the file sees its old contents or one
writer's value, whole, never a mixture. When every writer has the same settings, they
write one value, and once any writer has renamed, the file holds it for good.

A writer is a natural number. A file's contents are a value and a flag that says whether
every byte is there.
-/

namespace Gds.R0020.SharedFile

/-- The shared file and the writers' stage files. -/
structure State where
  /-- The file, as a reader sees it. -/
  file : Nat × Bool
  /-- Each writer's stage file, if it has one. -/
  stage : Nat -> Option (Nat × Bool)
  /-- Whether a writer has renamed its stage file over the file. -/
  renamed : Nat -> Bool

/-- `f` with the value `v` at `a`. -/
def set {α : Type} (f : Nat -> α) (a : Nat) (v : α) : Nat -> α :=
  fun x => if x = a then v else f x

/-- Before the build: the file holds `old`, whole, or a whole file the build does not
know; no stage file exists. -/
def init (old : Nat) : State where
  file := (old, true)
  stage := fun _ => none
  renamed := fun _ => false

/-- One step of one writer. Writer `w` writes the value `v w`. -/
inductive Step (v : Nat -> Nat) : State -> State -> Prop where
  /-- The writer creates its own stage file, empty (`create_new`). -/
  | create (s : State) (w : Nat) : s.stage w = none ->
      Step v s { s with stage := set s.stage w (some (v w, false)) }
  /-- The writer writes every byte of its stage file. -/
  | write (s : State) (w : Nat) : s.stage w = some (v w, false) ->
      Step v s { s with stage := set s.stage w (some (v w, true)) }
  /-- The writer renames its whole stage file over the file. -/
  | rename (s : State) (w : Nat) (x : Nat × Bool) : s.stage w = some x -> x.2 = true ->
      Step v s { s with
        file := x, stage := set s.stage w none, renamed := set s.renamed w true }

/-- The states the writers reach. -/
inductive Reachable (old : Nat) (v : Nat -> Nat) : State -> Prop where
  | init : Reachable old v (init old)
  | step {s t : State} : Reachable old v s -> Step v s t -> Reachable old v t

/-- What holds of every reachable state. -/
structure Inv (old : Nat) (v : Nat -> Nat) (s : State) : Prop where
  /-- A writer's stage file holds the writer's value. -/
  stages : forall w x, s.stage w = some x -> x.1 = v w
  /-- The file is whole and holds its old contents or some writer's value. -/
  file : s.file = (old, true) \/ exists w, s.file = (v w, true)

theorem inv_init (old : Nat) (v : Nat -> Nat) : Inv old v (init old) := by
  refine ⟨?_, Or.inl rfl⟩
  intro w x hx
  simp [init] at hx

theorem inv_step {old : Nat} {v : Nat -> Nat} {s t : State} (h : Inv old v s)
    (hs : Step v s t) : Inv old v t := by
  cases hs with
  | create w _ =>
    refine ⟨?_, h.file⟩
    intro u x hx
    by_cases e : u = w
    · subst e
      simp [set] at hx
      rw [← hx]
    · simp [set, e] at hx
      exact h.stages u x hx
  | write w _ =>
    refine ⟨?_, h.file⟩
    intro u x hx
    by_cases e : u = w
    · subst e
      simp [set] at hx
      rw [← hx]
    · simp [set, e] at hx
      exact h.stages u x hx
  | rename w x hx hwhole =>
    have hv := h.stages w x hx
    have hfile : x = (v w, true) := by
      cases x
      simp_all
    refine ⟨?_, Or.inr ⟨w, hfile⟩⟩
    intro u y hy
    by_cases e : u = w
    · subst e
      simp [set] at hy
    · simp [set, e] at hy
      exact h.stages u y hy

theorem inv_reachable {old : Nat} {v : Nat -> Nat} {s : State} (h : Reachable old v s) :
    Inv old v s := by
  induction h with
  | init => exact inv_init old v
  | step _ hs ih => exact inv_step ih hs

/-- Theorem 8 (a shared file stays whole): a reader sees the old file or one writer's
file, whole. -/
theorem reader_sees_whole_file {old : Nat} {v : Nat -> Nat} {s : State}
    (h : Reachable old v s) : s.file = (old, true) \/ exists w, s.file = (v w, true) :=
  (inv_reachable h).file

/-- After a rename, the file holds some writer's value: every rename writes one. -/
theorem renamed_holds_a_value {old : Nat} {v : Nat -> Nat} {s : State}
    (h : Reachable old v s) : forall w, s.renamed w = true -> exists u, s.file = (v u, true) := by
  induction h with
  | init => intro w hw; simp [init] at hw
  | step hr hs ih =>
    intro w hw
    cases hs with
    | create u _ => exact ih w hw
    | write u _ => exact ih w hw
    | rename u x hx _ =>
      have hv := (inv_reachable hr).stages u x hx
      refine ⟨u, ?_⟩
      cases x
      simp_all

/-- Corollary 8a: when every writer writes the same value `c`, a reader sees the old file
or `c`, and once a writer has renamed, the file holds `c` for good. -/
theorem same_value {old c : Nat} {v : Nat -> Nat} (hc : forall w, v w = c) {s : State}
    (h : Reachable old v s) :
    (s.file = (old, true) \/ s.file = (c, true)) /\
      (forall w, s.renamed w = true -> s.file = (c, true)) := by
  constructor
  · rcases reader_sees_whole_file h with ho | ⟨w, hw⟩
    · exact Or.inl ho
    · exact Or.inr (by rw [hw, hc w])
  · intro w hw
    obtain ⟨u, hu⟩ := renamed_holds_a_value h w hw
    rw [hu, hc u]

end Gds.R0020.SharedFile
