/-!
# The files of two jobs are different files (GDS 0020)

A job of a site build is one edition of one mount: a mount name `m` and a language `l`.
Its child process writes below the folder `ssg-projects` of the build folder, and only to
these names:

* `m/l`, the edition's output folder;
* `m/.l.guidedog`, the state folder that `guidedog build` keeps beside its output folder;
* `m/.l.log` and `m/.l.peak`, the job's report and its peak memory.

conf.toml makes mount names unique ignoring ASCII case and keeps `/` out of them; it makes
the languages of one mount's editions unique ignoring ASCII case (and `-` against `_`),
and allows only letters, digits, `_` and `-` in them. We prove that two different jobs
never name the same file, also on a file system that ignores ASCII case (macOS, Windows).

Names are lists of bytes. `fold` lowers ASCII capitals, as `to_ascii_lowercase` does.
-/

namespace Gds.R0020.Paths

/-- The byte `/`. -/
def slash : Nat := 47

/-- The byte `.`. -/
def dot : Nat := 46

/-- A byte with an ASCII capital lowered. -/
def lower (b : Nat) : Nat := if 65 ≤ b /\ b ≤ 90 then b + 32 else b

/-- A name as a file system that ignores ASCII case compares it. -/
def fold (s : List Nat) : List Nat := s.map lower

/-- Lowering never makes a byte below `a` from another byte. -/
theorem lower_small {b c : Nat} (hc : c < 97) (h : lower b = c) : b = c := by
  unfold lower at h
  split at h <;> omega

theorem fold_append (a b : List Nat) : fold (a ++ b) = fold a ++ fold b := by
  simp [fold]

theorem fold_cons (x : Nat) (a : List Nat) : fold (x :: a) = lower x :: fold a := rfl

theorem lower_slash : lower slash = slash := by decide

theorem lower_dot : lower dot = dot := by decide

/-- A byte below `a` that a name lacks, its folded name lacks too. -/
theorem not_mem_fold {c : Nat} (hc : c < 97) {s : List Nat} (h : Not (c ∈ s)) :
    Not (c ∈ fold s) := by
  intro hm
  simp only [fold, List.mem_map] at hm
  obtain ⟨b, hb, hl⟩ := hm
  rw [lower_small hc hl] at hb
  exact h hb

/-- Lemma 1: a separator that the first parts lack splits a name in one way only. -/
theorem split_unique (sep : Nat) :
    forall (a b c d : List Nat), Not (sep ∈ a) -> Not (sep ∈ c) ->
      a ++ sep :: b = c ++ sep :: d -> a = c /\ b = d
  | [], b, [], d, _, _, h => by simp at h; exact ⟨rfl, h⟩
  | [], b, y :: c, d, _, hc, h => by
    simp at h
    exact absurd (by rw [h.1]; simp) hc
  | x :: a, b, [], d, ha, _, h => by
    simp at h
    exact absurd (by rw [← h.1]; simp) ha
  | x :: a, b, y :: c, d, ha, hc, h => by
    simp at h
    have ha' : Not (sep ∈ a) := fun m => ha (by simp [m])
    have hc' : Not (sep ∈ c) := fun m => hc (by simp [m])
    have := split_unique sep a b c d ha' hc' h.2
    exact ⟨by rw [h.1, this.1], this.2⟩

/-- A name a job writes in its mount's folder: the output folder, or a dotted name. -/
inductive Name where
  /-- `l`, the output folder. -/
  | output
  /-- `.l.suffix`: the state folder (`guidedog`), the report (`log`), the peak (`peak`). -/
  | dotted (suffix : List Nat)

/-- The bytes of a name for the language `l`. -/
def Name.bytes (l : List Nat) : Name -> List Nat
  | .output => l
  | .dotted suffix => dot :: (l ++ dot :: suffix)

/-- What conf.toml allows in a language: some bytes, none of them `.` or `/`. -/
structure Language (l : List Nat) : Prop where
  nonempty : Not (l = [])
  noDot : Not (dot ∈ l)
  noSlash : Not (slash ∈ l)

/-- Lemma 2: the folded names of two languages are equal only when the folded languages
are. -/
theorem names_fold {l₁ l₂ : List Nat} (h₁ : Language l₁) (h₂ : Language l₂) (n₁ n₂ : Name)
    (h : fold (n₁.bytes l₁) = fold (n₂.bytes l₂)) : fold l₁ = fold l₂ := by
  cases n₁ with
  | output =>
    cases n₂ with
    | output => exact h
    | dotted s =>
      -- `l₁` starts with a byte that is not `.`; the dotted name starts with `.`.
      cases hl : l₁ with
      | nil => exact absurd hl h₁.nonempty
      | cons x rest =>
        simp only [Name.bytes, hl, fold_cons] at h
        have hx := lower_small (by decide) (List.cons.inj h).1
        exact absurd (by rw [hl, hx, lower_dot]; simp) h₁.noDot
  | dotted s₁ =>
    cases n₂ with
    | output =>
      cases hl : l₂ with
      | nil => exact absurd hl h₂.nonempty
      | cons x rest =>
        simp only [Name.bytes, hl, fold_cons] at h
        have hx := lower_small (by decide) (List.cons.inj h).1.symm
        exact absurd (by rw [hl, hx, lower_dot]; simp) h₂.noDot
    | dotted s₂ =>
      simp only [Name.bytes, fold_cons, fold_append, lower_dot] at h
      have h' := (List.cons.inj h).2
      exact (split_unique dot (fold l₁) (fold s₁) (fold l₂) (fold s₂)
        (not_mem_fold (by decide) h₁.noDot) (not_mem_fold (by decide) h₂.noDot) h').1

/-- Theorem 7 (disjoint files): for two jobs whose mount names differ ignoring case, or
whose languages do, no name of one is a name of the other, ignoring case. The names may
hold any bytes after the language; only the mount names must lack `/`. -/
theorem jobs_disjoint {m₁ m₂ l₁ l₂ : List Nat} (hm₁ : Not (slash ∈ m₁)) (hm₂ : Not (slash ∈ m₂))
    (hl₁ : Language l₁) (hl₂ : Language l₂)
    (hdiff : Not (fold m₁ = fold m₂) \/ Not (fold l₁ = fold l₂))
    (n₁ n₂ : Name) :
    Not (fold (m₁ ++ slash :: n₁.bytes l₁) = fold (m₂ ++ slash :: n₂.bytes l₂)) := by
  intro h
  simp only [fold_append, fold_cons, lower_slash] at h
  have split := split_unique slash (fold m₁) (fold (n₁.bytes l₁)) (fold m₂)
    (fold (n₂.bytes l₂)) (not_mem_fold (by decide) hm₁) (not_mem_fold (by decide) hm₂) h
  rcases hdiff with hd | hd
  · exact hd split.1
  · exact hd (names_fold hl₁ hl₂ n₁ n₂ split.2)

/-- Corollary 7a: without case folding too. -/
theorem jobs_disjoint_exact {m₁ m₂ l₁ l₂ : List Nat} (hm₁ : Not (slash ∈ m₁))
    (hm₂ : Not (slash ∈ m₂)) (hl₁ : Language l₁) (hl₂ : Language l₂)
    (hdiff : Not (fold m₁ = fold m₂) \/ Not (fold l₁ = fold l₂)) (n₁ n₂ : Name) :
    Not (m₁ ++ slash :: n₁.bytes l₁ = m₂ ++ slash :: n₂.bytes l₂) := by
  intro h
  exact jobs_disjoint hm₁ hm₂ hl₁ hl₂ hdiff n₁ n₂ (by rw [h])

/-- Lemma 3: the names of one job differ from each other when their suffixes do. -/
theorem names_differ {l : List Nat} (hl : Language l) (s₁ s₂ : List Nat)
    (hs : Not (s₁ = s₂)) :
    Not (Name.output.bytes l = (Name.dotted s₁).bytes l) /\
      Not ((Name.dotted s₁).bytes l = (Name.dotted s₂).bytes l) := by
  constructor
  · intro h
    cases hc : l with
    | nil => exact hl.nonempty hc
    | cons x rest =>
      simp only [Name.bytes, hc] at h
      have := (List.cons.inj h).1
      exact hl.noDot (by rw [hc, this]; simp)
  · intro h
    simp only [Name.bytes] at h
    have := split_unique dot l s₁ l s₂ hl.noDot hl.noDot (List.cons.inj h).2
    exact hs this.2

end Gds.R0020.Paths
