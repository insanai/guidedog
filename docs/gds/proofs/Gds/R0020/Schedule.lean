/-!
# The memory schedule of a parallel site build (GDS 0020)

`guidedog ssg build` builds every edition of every mount, then assembles the site. GDS 0020
runs the edition builds as child processes at the same time, as many as fit in memory. A
*job* is one edition build. The plan numbers the jobs `0 .. n-1` in the order the
sequential build runs them (mount order, then edition order).

Each job has a *weight*: the memory, in bytes, we reserve for it while it runs. The
*capacity* is nine tenths of the memory the machine had available when the build began.
A job starts when nothing runs, or when its weight fits beside the weights of the running
jobs and a slot is free; the environment may refuse a start for its own reasons (`room`:
the memory available now). A job above a failed job never starts, as in the sequential
build, which stops at its first failure.

We prove that the running weights stay within the capacity unless exactly one job runs,
that at most `max slots 1` jobs run at once, that every run ends after at most `2 n`
steps, and that a run that can go no further has the outcome of the sequential build:
every job succeeded, or the first failing job in plan order failed and every job before it
succeeded.

The outcome of a job is a fixed function `fails` of the plan. This is the independence
assumption of GDS 0020: a job's result depends on its inputs alone, and no job changes
another job's inputs.
-/

namespace Gds.R0020.Schedule

/-- Where a job is. -/
inductive Phase where
  /-- Not started. -/
  | waiting
  /-- Its child process runs. -/
  | running
  /-- Its child process ended with success. -/
  | succeeded
  /-- Its child process ended with failure. -/
  | failed
  deriving DecidableEq, Repr

/-- What the build knows before it starts a job. -/
structure Plan where
  /-- The number of jobs; jobs are `0 .. n-1` in plan order. -/
  n : Nat
  /-- The memory reserved for each job while it runs. -/
  weight : Nat -> Nat
  /-- The memory all running jobs may reserve together. -/
  capacity : Nat
  /-- The most jobs that run at once (`--jobs`, or the processors). -/
  slots : Nat
  /-- Whether each job fails: a function of the job alone (independence). -/
  fails : Nat -> Bool

/-- The jobs of a build. -/
structure State where
  /-- Where each job is. -/
  phase : Nat -> Phase

/-- `f` with the value `v` at `a`. -/
def set {α : Type} (f : Nat -> α) (a : Nat) (v : α) : Nat -> α :=
  fun x => if x = a then v else f x

@[simp] theorem set_same {α : Type} (f : Nat -> α) (a : Nat) (v : α) : set f a v a = v := by
  simp [set]

@[simp] theorem set_other {α : Type} (f : Nat -> α) (a x : Nat) (v : α) (h : Not (x = a)) :
    set f a v x = f x := by
  simp [set, h]

/-- Nothing has started. -/
def init : State where
  phase := fun _ => .waiting

/-- `total f m` is `f 0 + f 1 + ... + f (m - 1)`. -/
def total (f : Nat -> Nat) : Nat -> Nat
  | 0 => 0
  | m + 1 => total f m + f m

/-- The weight a job reserves in state `s`: its weight while it runs, else nothing. -/
def State.held (p : Plan) (s : State) (k : Nat) : Nat :=
  if s.phase k = .running then p.weight k else 0

/-- One for a running job. -/
def State.busy (s : State) (k : Nat) : Nat :=
  if s.phase k = .running then 1 else 0

/-- The memory the running jobs reserve. -/
def State.load (p : Plan) (s : State) : Nat := total (s.held p) p.n

/-- How many jobs run. -/
def State.count (p : Plan) (s : State) : Nat := total s.busy p.n

/-- One step of the build. `room s j` is the environment's consent to start `j` now; the
code asks whether the memory available now still has room for the job's weight. The
theorems hold whatever `room` says. -/
inductive Step (p : Plan) (room : State -> Nat -> Prop) : State -> State -> Prop where
  /-- A job starts: it waits, no job before it failed, and nothing runs, or its weight
  fits beside the running weights, a slot is free, and the environment consents. -/
  | start (s : State) (j : Nat) : j < p.n -> s.phase j = .waiting ->
      (forall k, s.phase k = .failed -> j < k) ->
      (s.count p = 0 \/ (s.load p + p.weight j ≤ p.capacity /\ s.count p < p.slots /\ room s j)) ->
      Step p room s { phase := set s.phase j .running }
  /-- A running job's child ends with success. -/
  | succeed (s : State) (j : Nat) : s.phase j = .running -> p.fails j = false ->
      Step p room s { phase := set s.phase j .succeeded }
  /-- A running job's child ends with failure. -/
  | fail (s : State) (j : Nat) : s.phase j = .running -> p.fails j = true ->
      Step p room s { phase := set s.phase j .failed }

/-- The states a run can reach. -/
inductive Reachable (p : Plan) (room : State -> Nat -> Prop) : State -> Prop where
  | init : Reachable p room init
  | step {s t : State} : Reachable p room s -> Step p room s t -> Reachable p room t

/-! ## Sums -/

/-- Changing `f` at one point `j` changes the total by the difference at `j`, if `j` is
counted. -/
theorem total_update (f g : Nat -> Nat) (j : Nat) (h : forall k, Not (k = j) -> f k = g k) :
    forall m, total g m + (if j < m then f j else 0) = total f m + (if j < m then g j else 0)
  | 0 => by simp [total]
  | m + 1 => by
    have ih := total_update f g j h m
    simp only [total]
    by_cases hm : m = j
    · subst hm
      have : Not (m < m) := Nat.lt_irrefl m
      simp only [this, if_false] at ih
      have : m < m + 1 := Nat.lt_succ_self m
      simp only [this, if_true]
      omega
    · have hfg := h m hm
      by_cases hj : j < m
      · have : j < m + 1 := Nat.lt_succ_of_lt hj
        simp only [hj, this, if_true] at ih ⊢
        omega
      · have : Not (j < m + 1) := by omega
        simp only [hj, this, if_false] at ih ⊢
        omega

/-- A total of zero has zero terms. -/
theorem total_zero (f : Nat -> Nat) :
    forall m, total f m = 0 -> forall k, k < m -> f k = 0
  | 0 => fun _ k hk => absurd hk (Nat.not_lt_zero k)
  | m + 1 => fun h k hk => by
    simp only [total] at h
    by_cases hkm : k = m
    · subst hkm
      omega
    · exact total_zero f m (by omega) k (by omega)

/-- A total whose terms are zero is zero. -/
theorem total_of_zero (f : Nat -> Nat) :
    forall m, (forall k, k < m -> f k = 0) -> total f m = 0
  | 0 => fun _ => rfl
  | m + 1 => fun h => by
    simp only [total]
    rw [total_of_zero f m (fun k hk => h k (by omega)), h m (Nat.lt_succ_self m)]

/-- A term is at most the total. -/
theorem term_le_total (f : Nat -> Nat) :
    forall m k, k < m -> f k ≤ total f m
  | 0 => fun k hk => absurd hk (Nat.not_lt_zero k)
  | m + 1 => fun k hk => by
    simp only [total]
    by_cases hkm : k = m
    · subst hkm
      omega
    · have := term_le_total f m k (by omega)
      omega

/-- A total of terms at most `c` is at most `c * m`. -/
theorem total_le (f : Nat -> Nat) (c : Nat) (h : forall k, f k ≤ c) :
    forall m, total f m ≤ c * m
  | 0 => by simp [total]
  | m + 1 => by
    simp only [total]
    have := total_le f c h m
    have := h m
    rw [Nat.mul_succ]
    omega

/-! ## What a step changes -/

/-- Changing one job's phase changes `held` at that job only. -/
theorem held_other (p : Plan) (s : State) (j : Nat) (v : Phase) :
    forall k, Not (k = j) -> State.held p { phase := set s.phase j v } k = s.held p k := by
  intro k hk
  simp [State.held, set_other _ _ _ _ hk]

/-- Changing one job's phase changes `busy` at that job only. -/
theorem busy_other (s : State) (j : Nat) (v : Phase) :
    forall k, Not (k = j) -> State.busy { phase := set s.phase j v } k = s.busy k := by
  intro k hk
  simp [State.busy, set_other _ _ _ _ hk]

/-- Starting a waiting job `j < n` adds its weight to the load. -/
theorem load_start (p : Plan) (s : State) (j : Nat) (hj : j < p.n)
    (hw : s.phase j = .waiting) :
    State.load p { phase := set s.phase j .running } = s.load p + p.weight j := by
  have h := total_update (State.held p { phase := set s.phase j .running }) (s.held p) j
    (held_other p s j .running) p.n
  simp only [hj, if_true, State.held, set_same, hw] at h
  simp only [State.load, State.held]
  simp at h
  omega

/-- Starting a waiting job `j < n` adds one to the count. -/
theorem count_start (p : Plan) (s : State) (j : Nat) (hj : j < p.n)
    (hw : s.phase j = .waiting) :
    State.count p { phase := set s.phase j .running } = s.count p + 1 := by
  have h := total_update (State.busy { phase := set s.phase j .running }) s.busy j
    (busy_other s j .running) p.n
  simp only [hj, if_true, State.busy, set_same, hw] at h
  simp only [State.count, State.busy]
  simp at h
  omega

/-- Ending a running job `j < n` takes its weight from the load. -/
theorem load_end (p : Plan) (s : State) (j : Nat) (hj : j < p.n) (v : Phase)
    (hr : s.phase j = .running) (hv : Not (v = .running)) :
    State.load p { phase := set s.phase j v } + p.weight j = s.load p := by
  have h := total_update (State.held p { phase := set s.phase j v }) (s.held p) j
    (held_other p s j v) p.n
  simp only [hj, if_true, State.held, set_same, hr, hv, if_false] at h
  simp only [State.load, State.held]
  simp at h
  omega

/-- Ending a running job `j < n` takes one from the count. -/
theorem count_end (p : Plan) (s : State) (j : Nat) (hj : j < p.n) (v : Phase)
    (hr : s.phase j = .running) (hv : Not (v = .running)) :
    State.count p { phase := set s.phase j v } + 1 = s.count p := by
  have h := total_update (State.busy { phase := set s.phase j v }) s.busy j
    (busy_other s j v) p.n
  simp only [hj, if_true, State.busy, set_same, hr, hv, if_false] at h
  simp only [State.count, State.busy]
  simp at h
  omega

/-- When no job runs, nothing is reserved. -/
theorem load_of_count_zero (p : Plan) (s : State) (h : s.count p = 0) : s.load p = 0 := by
  apply total_of_zero
  intro k hk
  have := total_zero s.busy p.n h k hk
  simp only [State.busy] at this
  simp only [State.held]
  split <;> simp_all

/-- A running job `j < n` is counted. -/
theorem count_pos (p : Plan) (s : State) (j : Nat) (hj : j < p.n)
    (hr : s.phase j = .running) : 0 < s.count p := by
  have := term_le_total s.busy p.n j hj
  simp only [State.busy, hr, if_true] at this
  exact this

/-! ## The invariant -/

/-- What holds of every reachable state. -/
structure Inv (p : Plan) (s : State) : Prop where
  /-- The running jobs reserve at most the capacity, or one job runs alone. -/
  memory : s.load p ≤ p.capacity \/ s.count p = 1
  /-- At most `max slots 1` jobs run. -/
  slots : s.count p ≤ max p.slots 1
  /-- Jobs outside the plan never start. -/
  outside : forall k, p.n ≤ k -> s.phase k = .waiting
  /-- A job that succeeded does not fail. -/
  succeededOk : forall k, s.phase k = .succeeded -> p.fails k = false
  /-- A job that failed fails. -/
  failedFails : forall k, s.phase k = .failed -> p.fails k = true

theorem inv_init (p : Plan) : Inv p init := by
  have h0 : init.count p = 0 := total_of_zero _ _ (fun k _ => by simp [State.busy, init])
  have hl : init.load p = 0 := load_of_count_zero p init h0
  refine ⟨Or.inl (by omega), by omega, fun _ _ => rfl, ?_, ?_⟩ <;> intro k hk <;>
    simp [init] at hk

/-- A running job is in the plan. -/
theorem Inv.inPlan {p : Plan} {s : State} (h : Inv p s) {j : Nat}
    (hr : s.phase j = .running) : j < p.n := by
  apply Nat.lt_of_not_le
  intro hn
  have := h.outside j hn
  rw [hr] at this
  cases this

/-- Ending a running job keeps the invariant. -/
theorem inv_end {p : Plan} {s : State} (h : Inv p s) (j : Nat) (v : Phase)
    (hr : s.phase j = .running) (hv : Not (v = .running))
    (hok : v = .succeeded -> p.fails j = false) (hbad : v = .failed -> p.fails j = true) :
    Inv p { phase := set s.phase j v } := by
  have hj := h.inPlan hr
  have hl := load_end p s j hj v hr hv
  have hc := count_end p s j hj v hr hv
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · rcases h.memory with hm | hm
    · exact Or.inl (by omega)
    · have h0 : State.count p { phase := set s.phase j v } = 0 := by omega
      have := load_of_count_zero p _ h0
      exact Or.inl (by omega)
  · have := h.slots
    omega
  · intro k hk
    have hkj : Not (k = j) := by omega
    simp [set_other _ _ _ _ hkj, h.outside k hk]
  · intro k hk
    by_cases hkj : k = j
    · subst hkj
      simp at hk
      exact hok hk
    · simp [set_other _ _ _ _ hkj] at hk
      exact h.succeededOk k hk
  · intro k hk
    by_cases hkj : k = j
    · subst hkj
      simp at hk
      exact hbad hk
    · simp [set_other _ _ _ _ hkj] at hk
      exact h.failedFails k hk

/-- Starting a job keeps the invariant, whatever the environment consents to. -/
theorem inv_start {p : Plan} {s : State} (h : Inv p s) (j : Nat) (hj : j < p.n)
    (hw : s.phase j = .waiting) (room : Prop)
    (hfit : s.count p = 0 \/ (s.load p + p.weight j ≤ p.capacity /\ s.count p < p.slots /\ room)) :
    Inv p { phase := set s.phase j .running } := by
  have hl := load_start p s j hj hw
  have hc := count_start p s j hj hw
  refine ⟨?_, ?_, ?_, ?_, ?_⟩
  · rcases hfit with h0 | ⟨hm, _, _⟩
    · exact Or.inr (by omega)
    · exact Or.inl (by omega)
  · rcases hfit with h0 | ⟨_, hs, _⟩
    · omega
    · omega
  · intro k hk
    have hkj : Not (k = j) := by omega
    simp [set_other _ _ _ _ hkj, h.outside k hk]
  · intro k hk
    by_cases hkj : k = j
    · subst hkj
      simp at hk
    · simp [set_other _ _ _ _ hkj] at hk
      exact h.succeededOk k hk
  · intro k hk
    by_cases hkj : k = j
    · subst hkj
      simp at hk
    · simp [set_other _ _ _ _ hkj] at hk
      exact h.failedFails k hk

/-- Every step keeps the invariant. -/
theorem inv_step {p : Plan} {room : State -> Nat -> Prop} {s t : State} (h : Inv p s)
    (hs : Step p room s t) : Inv p t := by
  cases hs with
  | start j hj hw _ hfit => exact inv_start h j hj hw (room s j) hfit
  | succeed j hr hf =>
    exact inv_end h j .succeeded hr (by simp) (fun _ => hf) (fun e => by cases e)
  | fail j hr hf =>
    exact inv_end h j .failed hr (by simp) (fun e => by cases e) (fun _ => hf)

/-- Lemma 1: the invariant holds in every reachable state. -/
theorem inv_reachable {p : Plan} {room : State -> Nat -> Prop} {s : State}
    (h : Reachable p room s) : Inv p s := by
  induction h with
  | init => exact inv_init p
  | step _ hs ih => exact inv_step ih hs

/-! ## Safety -/

/-- Theorem 1 (memory): the running jobs reserve at most the capacity, unless one job runs
alone. -/
theorem memory_bound {p : Plan} {room : State -> Nat -> Prop} {s : State}
    (h : Reachable p room s) : s.load p ≤ p.capacity \/ s.count p = 1 :=
  (inv_reachable h).memory

/-- Corollary 1a: when every weight fits in the capacity, the running jobs always reserve
at most the capacity. -/
theorem memory_bound_fitting {p : Plan} {room : State -> Nat -> Prop} {s : State}
    (h : Reachable p room s) (fit : forall k, p.weight k ≤ p.capacity) :
    s.load p ≤ p.capacity := by
  rcases memory_bound h with hm | h1
  · exact hm
  · -- One job runs: the load is its weight.
    have hinv := inv_reachable h
    by_cases hzero : s.load p = 0
    · omega
    · have hpos : 0 < total (s.held p) p.n := by
        simp only [State.load] at hzero
        omega
      -- Some job reserves something, so it runs; it is the only one.
      have hex : exists j, j < p.n /\ s.phase j = .running := by
        apply Classical.byContradiction
        intro hno
        have : total (s.held p) p.n = 0 := total_of_zero _ _ (fun k hk => by
          simp only [State.held]
          split
          · exact absurd ⟨k, hk, by assumption⟩ hno
          · rfl)
        omega
      obtain ⟨j, hj, hr⟩ := hex
      -- The load without `j` is zero, because the count without `j` is zero.
      have hl := load_end p s j hj .succeeded hr (by simp)
      have hc := count_end p s j hj .succeeded hr (by simp)
      have h0 : State.count p { phase := set s.phase j .succeeded } = 0 := by omega
      have := load_of_count_zero p _ h0
      have := fit j
      omega

/-- Theorem 2 (slots): at most `max slots 1` jobs run at once. -/
theorem slots_bound {p : Plan} {room : State -> Nat -> Prop} {s : State}
    (h : Reachable p room s) : s.count p ≤ max p.slots 1 :=
  (inv_reachable h).slots

/-! ## Termination -/

/-- How far a job is: 0 waiting, 1 running, 2 ended. -/
def rank : Phase -> Nat
  | .waiting => 0
  | .running => 1
  | .succeeded => 2
  | .failed => 2

/-- How far the build is. -/
def State.progress (p : Plan) (s : State) : Nat := total (fun k => rank (s.phase k)) p.n

/-- Moving job `j < n` from one phase to the next adds one to the progress. -/
theorem progress_update (p : Plan) (s : State) (j : Nat) (hj : j < p.n) (v : Phase)
    (hv : rank v = rank (s.phase j) + 1) :
    State.progress p { phase := set s.phase j v } = s.progress p + 1 := by
  have h := total_update (fun k => rank (set s.phase j v k)) (fun k => rank (s.phase k)) j
    (fun k hk => by simp [set_other _ _ _ _ hk]) p.n
  simp only [hj, if_true, set_same] at h
  simp only [State.progress]
  omega

/-- Every step adds one to the progress. -/
theorem progress_step {p : Plan} {room : State -> Nat -> Prop} {s t : State} (h : Inv p s)
    (hs : Step p room s t) : t.progress p = s.progress p + 1 := by
  cases hs with
  | start j hj hw _ _ => exact progress_update p s j hj .running (by simp [rank, hw])
  | succeed j hr _ =>
    exact progress_update p s j (h.inPlan hr) .succeeded (by simp [rank, hr])
  | fail j hr _ => exact progress_update p s j (h.inPlan hr) .failed (by simp [rank, hr])

/-- The progress is at most `2 n`. -/
theorem progress_le (p : Plan) (s : State) : s.progress p ≤ 2 * p.n :=
  total_le _ 2 (fun k => by cases s.phase k <;> simp [rank]) p.n

/-- The states a run reaches in exactly `m` steps. -/
inductive ReachIn (p : Plan) (room : State -> Nat -> Prop) : Nat -> State -> Prop where
  | init : ReachIn p room 0 init
  | step {m : Nat} {s t : State} : ReachIn p room m s -> Step p room s t ->
      ReachIn p room (m + 1) t

/-- After `m` steps the progress is `m` and the invariant holds. -/
theorem reachIn_progress {p : Plan} {room : State -> Nat -> Prop} {m : Nat} {s : State}
    (h : ReachIn p room m s) : s.progress p = m /\ Inv p s := by
  induction h with
  | init =>
    refine ⟨total_of_zero _ _ (fun k _ => by simp [init, rank]), inv_init p⟩
  | step _ hs ih =>
    exact ⟨by rw [progress_step ih.2 hs, ih.1], inv_step ih.2 hs⟩

/-- Theorem 3 (termination): every run has at most `2 n` steps. -/
theorem runs_are_short {p : Plan} {room : State -> Nat -> Prop} {m : Nat} {s : State}
    (h : ReachIn p room m s) : m ≤ 2 * p.n := by
  have := (reachIn_progress h).1
  have := progress_le p s
  omega

/-! ## The outcome -/

/-- No step is possible: the build has ended. -/
def Ended (p : Plan) (room : State -> Nat -> Prop) (s : State) : Prop :=
  forall t, Not (Step p room s t)

/-- When the build has ended nothing runs. -/
theorem ended_nothing_runs {p : Plan} {room : State -> Nat -> Prop} {s : State}
    (he : Ended p room s) (k : Nat) : Not (s.phase k = .running) := by
  intro hr
  cases hf : p.fails k with
  | false => exact he _ (Step.succeed s k hr hf)
  | true => exact he _ (Step.fail s k hr hf)

/-- When the build has ended, a waiting job of the plan has a failed job before it. -/
theorem ended_waiting {p : Plan} {room : State -> Nat -> Prop} {s : State}
    (he : Ended p room s) (j : Nat) (hj : j < p.n) (hw : s.phase j = .waiting) :
    exists k, s.phase k = .failed /\ k < j := by
  have h0 : s.count p = 0 := total_of_zero _ _ (fun k _ => by
    simp only [State.busy]
    split
    · exact absurd (by assumption) (ended_nothing_runs he k)
    · rfl)
  apply Classical.byContradiction
  intro hno
  apply he _ (Step.start s j hj hw (fun k hk => ?_) (Or.inl h0))
  apply Nat.lt_of_not_le
  intro hkj
  by_cases e : k = j
  · subst e
    rw [hw] at hk
    cases hk
  · exact hno ⟨k, hk, by omega⟩

/-- Theorem 4 (the outcome of the sequential build, success): when the build has ended and
no job failed, every job of the plan succeeded. -/
theorem ended_all_succeeded {p : Plan} {room : State -> Nat -> Prop} {s : State}
    (he : Ended p room s) (hnone : forall k, Not (s.phase k = .failed)) :
    forall j, j < p.n -> s.phase j = .succeeded := by
  intro j hj
  cases hp : s.phase j with
  | waiting =>
    obtain ⟨k, hk, _⟩ := ended_waiting he j hj hp
    exact absurd hk (hnone k)
  | running => exact absurd hp (ended_nothing_runs he j)
  | succeeded => rfl
  | failed => exact absurd hp (hnone j)

/-- Theorem 5 (the outcome of the sequential build, failure): when the build has ended and
`f` is the first job that failed, every job before `f` succeeded, `f` fails, and no job
before `f` fails. So `f` is the first job of the plan that fails, the job whose failure
the sequential build reports. -/
theorem ended_first_failure {p : Plan} {room : State -> Nat -> Prop} {s : State}
    (hr : Reachable p room s) (he : Ended p room s) (f : Nat) (hf : s.phase f = .failed)
    (hfirst : forall k, k < f -> Not (s.phase k = .failed)) :
    (forall k, k < f -> s.phase k = .succeeded) /\ p.fails f = true /\
      (forall k, k < f -> p.fails k = false) := by
  have hinv := inv_reachable hr
  have hfn : f < p.n := by
    apply Nat.lt_of_not_le
    intro hn
    have := hinv.outside f hn
    rw [hf] at this
    cases this
  have before : forall k, k < f -> s.phase k = .succeeded := by
    intro k hk
    cases hp : s.phase k with
    | waiting =>
      obtain ⟨i, hi, hik⟩ := ended_waiting he k (by omega) hp
      exact absurd hi (hfirst i (by omega))
    | running => exact absurd hp (ended_nothing_runs he k)
    | succeeded => rfl
    | failed => exact absurd hp (hfirst k hk)
  exact ⟨before, hinv.failedFails f hf, fun k hk => hinv.succeededOk k (before k hk)⟩

/-- Corollary 5a: when the build has ended and no job of the plan fails, every job
succeeded. -/
theorem ended_none_fails {p : Plan} {room : State -> Nat -> Prop} {s : State}
    (hr : Reachable p room s) (he : Ended p room s) (ok : forall k, k < p.n -> p.fails k = false) :
    forall j, j < p.n -> s.phase j = .succeeded := by
  have hinv := inv_reachable hr
  apply ended_all_succeeded he
  intro k hk
  have hfk := hinv.failedFails k hk
  by_cases hkn : k < p.n
  · rw [ok k hkn] at hfk
    cases hfk
  · have := hinv.outside k (by omega)
    rw [hk] at this
    cases this

/-- Theorem 6 (no job waits forever for memory): when nothing runs, a waiting job of the
plan with no failed job before it can start, whatever its weight and whatever the
environment says. With Theorem 3, every run reaches an ended state after at most `2 n`
steps, provided every child ends (assumption T), and Theorems 4 and 5 give its outcome. -/
theorem idle_start {p : Plan} {room : State -> Nat -> Prop} {s : State}
    (hidle : forall k, Not (s.phase k = .running)) (j : Nat) (hj : j < p.n)
    (hw : s.phase j = .waiting) (hok : forall k, s.phase k = .failed -> j < k) :
    Step p room s { phase := set s.phase j .running } := by
  have h0 : s.count p = 0 := total_of_zero _ _ (fun k _ => by
    simp only [State.busy]
    split
    · exact absurd (by assumption) (hidle k)
    · rfl)
  exact Step.start s j hj hw hok (Or.inl h0)

end Gds.R0020.Schedule
