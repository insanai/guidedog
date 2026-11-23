/-!
# The file lock (GDS 0019, "File locks released by an explicit unlock")

Guidedog serializes the commands that change a small state file (the plugin registry, a
publisher's settings, a discussion catalog, a site's sources) with an advisory lock on a
lock file: `flock` on Unix and `LockFileEx` on Windows, through `std::fs::File::try_lock`.
The system keeps such a lock with the open file, and releases it by itself only when the
last descriptor of that open file closes. A child process holds a copy of every descriptor
from its creation until it starts its program. So a holder that only closes its descriptor
has not released the lock while such a copy lives, and the next taker is told that a lock
nobody holds is held.

We model the lock file, the takers, the copies in children, crashes and the update of the
state file as a transition system. `Step true` is the algorithm of
`crates/gd-plugins/src/files/lock.rs`: a holder unlocks, then closes. `Step false` adds the
algorithm we replace: a holder that only closes. We prove an invariant of `Step true` and
derive mutual exclusion, the absence of a false refusal, crash recovery, and that readers
see whole files and writers lose no update. We prove that `Step false` reaches a false
refusal.

A taker is a natural number. It stands for one call of `lock_exclusive` and the open file
that call creates; a second call is another taker. Which takers share a process does not
matter to the model: a child creation that copies several descriptors is several `fork`
steps, and the end of a process is a `crash` step of each of its takers.
-/

namespace Gds.R0019.FileLock

/-- Where a taker is in its call. -/
inductive Phase where
  /-- Not started. -/
  | idle
  /-- It opened the lock file and has not asked for the lock. -/
  | opened
  /-- It took the lock and has not released it: the critical section. -/
  | held
  /-- It unlocked and has not closed its descriptor. -/
  | released
  /-- It closed its descriptor: after a release, or after a refusal. -/
  | done
  /-- Its process ended without releasing. The system closed its descriptor. -/
  | dead
  deriving DecidableEq, Repr

/-- How far a holder is with the file it writes beside the state file. -/
inductive Stage where
  /-- Nothing written. -/
  | empty
  /-- Some bytes written. -/
  | part
  /-- Every byte written and synced. -/
  | whole
  deriving DecidableEq, Repr

/-- The lock file, its takers, and the state file the lock protects. A file's contents
are a number and a flag that says whether every byte is there. -/
structure State where
  /-- Where each taker is. -/
  phase : Nat -> Phase
  /-- Whether the system holds the lock for a taker's open file. -/
  locked : Nat -> Bool
  /-- How many copies of a taker's descriptor live in children that have not yet started
  their programs. -/
  copies : Nat -> Nat
  /-- What a holder read from the state file. -/
  seen : Nat -> Option Nat
  /-- How far a holder is with the file beside. -/
  stage : Nat -> Stage
  /-- The state file, as a reader sees it. -/
  file : Nat × Bool
  /-- The file beside the state file, if there is one. -/
  next : Option (Nat × Bool)

/-- `f` with the value `v` at `a`. -/
def set {α : Type} (f : Nat -> α) (a : Nat) (v : α) : Nat -> α :=
  fun x => if x = a then v else f x

@[simp] theorem set_same {α : Type} (f : Nat -> α) (a : Nat) (v : α) : set f a v a = v := by
  simp [set]

@[simp] theorem set_other {α : Type} (f : Nat -> α) (a x : Nat) (v : α) (h : Not (x = a)) :
    set f a v x = f x := by
  simp [set, h]

/-- Nothing has happened: no taker has started, and the state file is whole. -/
def init : State where
  phase := fun _ => .idle
  locked := fun _ => false
  copies := fun _ => 0
  seen := fun _ => none
  stage := fun _ => .empty
  file := (0, true)
  next := none

/-- The taker has a descriptor of its own open. -/
def State.isOpen (s : State) (a : Nat) : Prop :=
  s.phase a = .opened \/ s.phase a = .held \/ s.phase a = .released

/-- The lock of `a`'s open file after `a`'s own descriptor closes: the system releases it
when no copy is left, and keeps it otherwise. -/
def State.afterClose (s : State) (a : Nat) : Bool :=
  if s.copies a = 0 then false else s.locked a

/-- The lock of `a`'s open file after one copy closes: the system releases it when that
was the last descriptor. -/
def State.afterExec (s : State) (a : Nat) : Bool :=
  if s.copies a = 1 /\ (s.phase a = .done \/ s.phase a = .dead) then false else s.locked a

/-- One step of the system. With `explicit = true` a holder releases by `unlock` and then
`close`. With `explicit = false` it may also release by `closeHeld` alone, as the code did
before GDS 0019. -/
inductive Step (explicit : Bool) : State -> State -> Prop where
  /-- A taker opens the lock file: an open file of its own. -/
  | openFile (s : State) (a : Nat) : s.phase a = .idle ->
      Step explicit s { s with phase := set s.phase a .opened }
  /-- `try_lock` succeeds: no open file holds the lock. -/
  | acquire (s : State) (a : Nat) : s.phase a = .opened -> (forall b, s.locked b = false) ->
      Step explicit s { s with
        phase := set s.phase a .held, locked := set s.locked a true,
        seen := set s.seen a none, stage := set s.stage a .empty }
  /-- `try_lock` fails: some open file holds the lock. The taker closes its descriptor
  and reports the lock as held. -/
  | refuse (s : State) (a : Nat) : s.phase a = .opened -> (exists b, s.locked b = true) ->
      Step explicit s { s with
        phase := set s.phase a .done, locked := set s.locked a (s.afterClose a) }
  /-- The holder unlocks: the lock is released for every copy of its descriptor. -/
  | unlock (s : State) (a : Nat) : s.phase a = .held ->
      Step explicit s { s with
        phase := set s.phase a .released, locked := set s.locked a false }
  /-- The holder closes its descriptor after unlocking. -/
  | close (s : State) (a : Nat) : s.phase a = .released ->
      Step explicit s { s with
        phase := set s.phase a .done, locked := set s.locked a (s.afterClose a) }
  /-- The replaced algorithm: the holder closes its descriptor without unlocking. -/
  | closeHeld (s : State) (a : Nat) : explicit = false -> s.phase a = .held ->
      Step explicit s { s with
        phase := set s.phase a .done, locked := set s.locked a (s.afterClose a) }
  /-- A child is created and holds a copy of the taker's descriptor. -/
  | fork (s : State) (a : Nat) : s.isOpen a ->
      Step explicit s { s with copies := set s.copies a (s.copies a + 1) }
  /-- A child starts its program or exits: its copy closes. -/
  | exec (s : State) (a : Nat) : 0 < s.copies a ->
      Step explicit s { s with
        copies := set s.copies a (s.copies a - 1), locked := set s.locked a (s.afterExec a) }
  /-- The taker's process ends: the system closes its descriptor. -/
  | crash (s : State) (a : Nat) : s.isOpen a ->
      Step explicit s { s with
        phase := set s.phase a .dead, locked := set s.locked a (s.afterClose a) }
  /-- The holder reads the state file. -/
  | read (s : State) (a : Nat) : s.phase a = .held ->
      Step explicit s { s with seen := set s.seen a (some s.file.1) }
  /-- The holder starts the file beside, replacing one a crashed holder left. -/
  | draft (s : State) (a u v : Nat) : s.phase a = .held -> s.seen a = some u ->
      Step explicit s { s with next := some (v, false), stage := set s.stage a .part }
  /-- The holder writes and syncs every byte of the file beside. -/
  | sync (s : State) (a v : Nat) : s.phase a = .held -> s.stage a = .part ->
      s.next = some (v, false) ->
      Step explicit s { s with next := some (v, true), stage := set s.stage a .whole }
  /-- The holder renames the file beside over the state file. -/
  | rename (s : State) (a : Nat) (w : Nat × Bool) : s.phase a = .held ->
      s.stage a = .whole -> s.next = some w ->
      Step explicit s { s with
        file := w, next := none, stage := set s.stage a .empty, seen := set s.seen a none }

/-- The states a run can reach. -/
inductive Reachable (explicit : Bool) : State -> Prop where
  | init : Reachable explicit init
  | step {s t : State} : Reachable explicit s -> Step explicit s t -> Reachable explicit t

/-- What holds of every reachable state of the algorithm. -/
structure Inv (s : State) : Prop where
  /-- The system gives the lock to one open file at a time. -/
  one : forall a b, s.locked a = true -> s.locked b = true -> a = b
  /-- A taker in its critical section has the system's lock. -/
  heldLocked : forall a, s.phase a = .held -> s.locked a = true
  /-- The system's lock belongs to a taker in its critical section, or to a dead taker
  whose descriptor a child still copies. -/
  lockedOwner : forall a, s.locked a = true ->
    s.phase a = .held \/ (s.phase a = .dead /\ 0 < s.copies a)
  /-- The state file is whole. -/
  fileWhole : s.file.2 = true
  /-- A holder that wrote every byte finds its file beside, whole. -/
  staged : forall a, s.phase a = .held -> s.stage a = .whole ->
    exists v, s.next = some (v, true)
  /-- What a holder read is what the state file holds. -/
  current : forall a v, s.phase a = .held -> s.seen a = some v -> s.file.1 = v
  /-- A holder that writes has read. -/
  hasRead : forall a, s.phase a = .held -> Not (s.stage a = .empty) ->
    exists v, s.seen a = some v

theorem inv_init : Inv init := by
  constructor <;> simp [init]

/-- Two takers in their critical sections are one taker, by the invariant. -/
theorem Inv.sameHolder {s : State} (h : Inv s) {a b : Nat}
    (ha : s.phase a = .held) (hb : s.phase b = .held) : a = b :=
  h.one a b (h.heldLocked a ha) (h.heldLocked b hb)

/-- Closes a goal that follows by rewriting with the hypotheses. -/
macro "auto" : tactic =>
  `(tactic| first
    | (simp_all [State.afterClose, State.afterExec, State.isOpen]; done)
    | (subst_vars; simp_all [State.afterClose, State.afterExec, State.isOpen]; done))

/-- Proves a clause of the invariant about the taker `x` after a step of the taker `a`:
it states the old invariant `h` of both, then splits on whether they are one taker. -/
macro "agent" h:ident s:ident x:ident a:ident : tactic =>
  `(tactic| (
    have oxa := Inv.one $h $x $a
    have wx := Inv.lockedOwner $h $x
    have wa := Inv.lockedOwner $h $a
    have lx := Inv.heldLocked $h $x
    have la := Inv.heldLocked $h $a
    have sx := Inv.staged $h $x
    have sa := Inv.staged $h $a
    have cx := Inv.current $h $x
    have ca := Inv.current $h $a
    have rx := Inv.hasRead $h $x
    have ra := Inv.hasRead $h $a
    by_cases ex : $x = $a <;>
      first
      | auto
      | (subst_vars
         cases hp : State.phase $s $x <;>
           (try simp_all [State.afterClose, State.afterExec, State.isOpen]) <;> (try omega))))

/-- As `agent`, for a clause about two takers `x` and `y`. -/
macro "pair" h:ident x:ident y:ident a:ident : tactic =>
  `(tactic| (
    have oxy := Inv.one $h $x $y
    have oxa := Inv.one $h $x $a
    have oya := Inv.one $h $y $a
    have wx := Inv.lockedOwner $h $x
    have wy := Inv.lockedOwner $h $y
    have wa := Inv.lockedOwner $h $a
    have la := Inv.heldLocked $h $a
    by_cases ex : $x = $a <;> by_cases ey : $y = $a <;> auto))

/-- Opening the lock file keeps the invariant. -/
theorem inv_openFile {s : State} (h : Inv s) (a : Nat) (ha : s.phase a = .idle) :
    Inv { s with phase := set s.phase a .opened } := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro x y hx hy
    pair h x y a
  · intro x hx
    agent h s x a
  · intro x hx
    agent h s x a
  · have hfile := h.fileWhole
    have sa := h.staged a
    auto
  · intro x hx hw'
    agent h s x a
  · intro x v' hx hv
    agent h s x a
  · intro x hx hne
    agent h s x a

/-- Taking the lock keeps the invariant. -/
theorem inv_acquire {s : State} (h : Inv s) (a : Nat) (_ha : s.phase a = .opened)
    (hfree : forall b, s.locked b = false) :
    Inv { s with
        phase := set s.phase a .held, locked := set s.locked a true,
        seen := set s.seen a none, stage := set s.stage a .empty } := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro x y hx hy
    pair h x y a
  · intro x hx
    agent h s x a
  · intro x hx
    agent h s x a
  · have hfile := h.fileWhole
    have sa := h.staged a
    auto
  · intro x hx hw'
    agent h s x a
  · intro x v' hx hv
    agent h s x a
  · intro x hx hne
    agent h s x a

/-- A refusal keeps the invariant. -/
theorem inv_refuse {s : State} (h : Inv s) (a : Nat) (_ha : s.phase a = .opened) :
    Inv { s with
        phase := set s.phase a .done, locked := set s.locked a (s.afterClose a) } := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro x y hx hy
    pair h x y a
  · intro x hx
    agent h s x a
  · intro x hx
    agent h s x a
  · have hfile := h.fileWhole
    have sa := h.staged a
    auto
  · intro x hx hw'
    agent h s x a
  · intro x v' hx hv
    agent h s x a
  · intro x hx hne
    agent h s x a

/-- Unlocking keeps the invariant. -/
theorem inv_unlock {s : State} (h : Inv s) (a : Nat) (ha : s.phase a = .held) :
    Inv { s with
        phase := set s.phase a .released, locked := set s.locked a false } := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro x y hx hy
    pair h x y a
  · intro x hx
    agent h s x a
  · intro x hx
    agent h s x a
  · have hfile := h.fileWhole
    have sa := h.staged a
    auto
  · intro x hx hw'
    agent h s x a
  · intro x v' hx hv
    agent h s x a
  · intro x hx hne
    agent h s x a

/-- Closing after the unlock keeps the invariant. -/
theorem inv_close {s : State} (h : Inv s) (a : Nat) (ha : s.phase a = .released) :
    Inv { s with
        phase := set s.phase a .done, locked := set s.locked a (s.afterClose a) } := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro x y hx hy
    pair h x y a
  · intro x hx
    agent h s x a
  · intro x hx
    agent h s x a
  · have hfile := h.fileWhole
    have sa := h.staged a
    auto
  · intro x hx hw'
    agent h s x a
  · intro x v' hx hv
    agent h s x a
  · intro x hx hne
    agent h s x a

/-- A child that copies the descriptor keeps the invariant. -/
theorem inv_fork {s : State} (h : Inv s) (a : Nat) (ha : s.isOpen a) :
    Inv { s with copies := set s.copies a (s.copies a + 1) } := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro x y hx hy
    pair h x y a
  · intro x hx
    agent h s x a
  · intro x hx
    agent h s x a
  · have hfile := h.fileWhole
    have sa := h.staged a
    auto
  · intro x hx hw'
    agent h s x a
  · intro x v' hx hv
    agent h s x a
  · intro x hx hne
    agent h s x a

/-- A child that closes its copy keeps the invariant. -/
theorem inv_exec {s : State} (h : Inv s) (a : Nat) (ha : 0 < s.copies a) :
    Inv { s with
        copies := set s.copies a (s.copies a - 1), locked := set s.locked a (s.afterExec a) } := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro x y hx hy
    pair h x y a
  · intro x hx
    agent h s x a
  · intro x hx
    agent h s x a
  · have hfile := h.fileWhole
    have sa := h.staged a
    auto
  · intro x hx hw'
    agent h s x a
  · intro x v' hx hv
    agent h s x a
  · intro x hx hne
    agent h s x a

/-- The end of a process keeps the invariant. -/
theorem inv_crash {s : State} (h : Inv s) (a : Nat) (ha : s.isOpen a) :
    Inv { s with
        phase := set s.phase a .dead, locked := set s.locked a (s.afterClose a) } := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro x y hx hy
    pair h x y a
  · intro x hx
    agent h s x a
  · intro x hx
    agent h s x a
  · have hfile := h.fileWhole
    have sa := h.staged a
    auto
  · intro x hx hw'
    agent h s x a
  · intro x v' hx hv
    agent h s x a
  · intro x hx hne
    agent h s x a

/-- Reading the state file keeps the invariant. -/
theorem inv_read {s : State} (h : Inv s) (a : Nat) (ha : s.phase a = .held) :
    Inv { s with seen := set s.seen a (some s.file.1) } := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro x y hx hy
    pair h x y a
  · intro x hx
    agent h s x a
  · intro x hx
    agent h s x a
  · have hfile := h.fileWhole
    have sa := h.staged a
    auto
  · intro x hx hw'
    agent h s x a
  · intro x v' hx hv
    agent h s x a
  · intro x hx hne
    agent h s x a

/-- Starting the file beside keeps the invariant. -/
theorem inv_draft {s : State} (h : Inv s) (a u v : Nat) (ha : s.phase a = .held)
    (hs : s.seen a = some u) :
    Inv { s with next := some (v, false), stage := set s.stage a .part } := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro x y hx hy
    pair h x y a
  · intro x hx
    agent h s x a
  · intro x hx
    agent h s x a
  · have hfile := h.fileWhole
    have sa := h.staged a
    auto
  · intro x hx hw'
    agent h s x a
  · intro x v' hx hv
    agent h s x a
  · intro x hx hne
    agent h s x a

/-- Completing the file beside keeps the invariant. -/
theorem inv_sync {s : State} (h : Inv s) (a v : Nat) (ha : s.phase a = .held)
    (hp : s.stage a = .part) (_hn : s.next = some (v, false)) :
    Inv { s with next := some (v, true), stage := set s.stage a .whole } := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro x y hx hy
    pair h x y a
  · intro x hx
    agent h s x a
  · intro x hx
    agent h s x a
  · have hfile := h.fileWhole
    have sa := h.staged a
    auto
  · intro x hx hw'
    agent h s x a
  · intro x v' hx hv
    agent h s x a
  · intro x hx hne
    agent h s x a

/-- The rename keeps the invariant. -/
theorem inv_rename {s : State} (h : Inv s) (a : Nat) (w : Nat × Bool) (ha : s.phase a = .held)
    (hw : s.stage a = .whole) (hn : s.next = some w) :
    Inv { s with
        file := w, next := none, stage := set s.stage a .empty, seen := set s.seen a none } := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · intro x y hx hy
    pair h x y a
  · intro x hx
    agent h s x a
  · intro x hx
    agent h s x a
  · obtain ⟨v, hv⟩ := h.staged a ha hw
    rw [hn] at hv
    cases hv
    rfl
  · intro x hx hw'
    agent h s x a
  · intro x v' hx hv
    agent h s x a
  · intro x hx hne
    agent h s x a

/-- Every step of the algorithm keeps the invariant. -/
theorem inv_step {s t : State} (h : Inv s) (st : Step true s t) : Inv t := by
  cases st with
  | closeHeld a hx ha => exact absurd hx (by decide)
  | openFile a ha => exact inv_openFile h a ha
  | acquire a ha hfree => exact inv_acquire h a ha hfree
  | refuse a ha _ => exact inv_refuse h a ha
  | unlock a ha => exact inv_unlock h a ha
  | close a ha => exact inv_close h a ha
  | fork a ha => exact inv_fork h a ha
  | exec a ha => exact inv_exec h a ha
  | crash a ha => exact inv_crash h a ha
  | read a ha => exact inv_read h a ha
  | draft a u v ha hs => exact inv_draft h a u v ha hs
  | sync a v ha hp hn => exact inv_sync h a v ha hp hn
  | rename a w ha hw hn => exact inv_rename h a w ha hw hn

/-- Lemma 1. The invariant holds of every reachable state of the algorithm. -/
theorem inv_reachable {s : State} (r : Reachable true s) : Inv s := by
  induction r with
  | init => exact inv_init
  | step _ st ih => exact inv_step ih st

/-! ## Safety -/

/-- Theorem 1, mutual exclusion. Two takers in their critical sections are one taker. -/
theorem mutual_exclusion {s : State} (r : Reachable true s) {a b : Nat}
    (ha : s.phase a = .held) (hb : s.phase b = .held) : a = b :=
  (inv_reachable r).sameHolder ha hb

/-- Theorem 2, no false refusal. When the system refuses a taker, some taker is in its
critical section, or a taker died there and a child still copies its descriptor. -/
theorem no_false_busy {s : State} (r : Reachable true s) (busy : exists b, s.locked b = true) :
    exists b, s.phase b = .held \/ (s.phase b = .dead /\ 0 < s.copies b) :=
  let ⟨b, hb⟩ := busy
  ⟨b, (inv_reachable r).lockedOwner b hb⟩

/-- Corollary 2a. While no taker has died, a refusal means a taker is in its critical
section. -/
theorem no_false_busy_alive {s : State} (r : Reachable true s)
    (alive : forall b, Not (s.phase b = .dead)) (busy : exists b, s.locked b = true) :
    exists b, s.phase b = .held := by
  obtain ⟨b, hb⟩ := no_false_busy r busy
  cases hb with
  | inl h => exact ⟨b, h⟩
  | inr h => exact absurd h.1 (alive b)

/-- Corollary 2b. A lock that is held is refused: while a taker is in its critical
section, `try_lock` cannot succeed. -/
theorem held_is_refused {s : State} (r : Reachable true s) {b : Nat}
    (hb : s.phase b = .held) : Not (forall c, s.locked c = false) := by
  intro free
  have := (inv_reachable r).heldLocked b hb
  simp [free b] at this

/-- Theorem 3, readers see whole files. The state file is whole in every state. -/
theorem reader_sees_whole {s : State} (r : Reachable true s) : s.file.2 = true :=
  (inv_reachable r).fileWhole

/-- Theorem 4, no lost update. A holder about to rename has read the state file, the
state file still holds what it read, and the file beside is whole. -/
theorem no_lost_update {s : State} (r : Reachable true s) {a : Nat}
    (ha : s.phase a = .held) (hw : s.stage a = .whole) :
    exists u v, s.seen a = some u /\ s.file.1 = u /\ s.next = some (v, true) := by
  have h := inv_reachable r
  obtain ⟨u, hu⟩ := h.hasRead a ha (by simp [hw])
  obtain ⟨v, hv⟩ := h.staged a ha hw
  exact ⟨u, v, hu, h.current a u ha hu, hv⟩

/-! ## Crash recovery and progress -/

/-- Theorem 5, crash recovery. A dead taker whose descriptor no child copies holds no
lock. -/
theorem dead_holds_nothing {s : State} (r : Reachable true s) {b : Nat}
    (hd : s.phase b = .dead) (hc : s.copies b = 0) : s.locked b = false := by
  cases hl : s.locked b with
  | false => rfl
  | true =>
    cases (inv_reachable r).lockedOwner b hl with
    | inl h => simp [hd] at h
    | inr h => omega

/-- Theorem 6, the lock is free. When no taker is in its critical section and no child
copies a dead taker's descriptor, no open file holds the lock. -/
theorem lock_is_free {s : State} (r : Reachable true s)
    (nobody : forall b, Not (s.phase b = .held))
    (drained : forall b, s.phase b = .dead -> s.copies b = 0) :
    forall b, s.locked b = false := by
  intro b
  cases hl : s.locked b with
  | false => rfl
  | true =>
    cases (inv_reachable r).lockedOwner b hl with
    | inl h => exact absurd h (nobody b)
    | inr h => have := drained b h.1; omega

/-- Corollary 6a, progress of a taker. In such a state a taker that opened the lock file
takes the lock in one step. -/
theorem taker_succeeds {s : State} (r : Reachable true s)
    (nobody : forall b, Not (s.phase b = .held))
    (drained : forall b, s.phase b = .dead -> s.copies b = 0)
    {a : Nat} (ha : s.phase a = .opened) :
    exists t, Step true s t /\ t.phase a = .held :=
  ⟨_, Step.acquire s a ha (lock_is_free r nobody drained), by simp⟩

/-- Lemma 7. No step revives a dead taker or adds a copy of its descriptor. This holds of
the replaced algorithm too. -/
theorem dead_copies_never_grow {e : Bool} {s t : State} (st : Step e s t) {b : Nat}
    (hd : s.phase b = .dead) : t.phase b = .dead /\ t.copies b <= s.copies b := by
  cases st with
  | fork a ha =>
    by_cases eb : b = a
    · subst eb; simp [State.isOpen, hd] at ha
    · simp [eb, hd]
  | exec a ha =>
    by_cases eb : b = a
    · subst eb; simp [hd]
    · simp [eb, hd]
  | crash a ha =>
    by_cases eb : b = a
    · subst eb; simp [State.isOpen, hd] at ha
    · simp [eb, hd]
  | openFile a ha | acquire a ha _ | refuse a ha _ | unlock a ha | close a ha
  | closeHeld a _ ha =>
    by_cases eb : b = a
    · subst eb; simp [hd] at ha
    · simp [eb, hd]
  | read a ha | draft a u v ha _ | sync a v ha _ _ | rename a w ha _ _ => simp [hd]

/-- The state after the children that copy `b`'s descriptor closed `n` copies. -/
def execs (s : State) (b : Nat) : Nat -> State
  | 0 => s
  | n + 1 =>
    execs { s with
      copies := set s.copies b (s.copies b - 1), locked := set s.locked b (s.afterExec b) } b n

/-- Theorem 8, bounded recovery. From a state where the taker `b` is dead, the `exec`
steps of the children that copy its descriptor, as many as there are copies, lead to a
reachable state where `b` holds no lock. -/
theorem crash_recovers {s : State} (r : Reachable true s) {b : Nat}
    (hd : s.phase b = .dead) :
    Reachable true (execs s b (s.copies b)) /\ (execs s b (s.copies b)).locked b = false := by
  suffices h : forall (n : Nat) (s : State), Reachable true s -> s.phase b = .dead ->
      s.copies b = n ->
      Reachable true (execs s b n) /\ (execs s b n).locked b = false from
    h (s.copies b) s r hd rfl
  intro n
  induction n with
  | zero =>
    intro s r hd hc
    exact ⟨r, dead_holds_nothing r hd hc⟩
  | succ n ih =>
    intro s r hd hc
    have st : Step true s _ := Step.exec s b (by omega)
    exact ih _ (r.step st) (by simp [hd]) (by simp [hc])

/-! ## The replaced algorithm -/

/-- Theorem 9. A holder that only closes its descriptor is refuted: the replaced
algorithm reaches a state where the system refuses every taker, though no taker is in its
critical section and none has died. A child copied the descriptor of taker 0 before it
closed. -/
theorem close_alone_refuses_falsely :
    exists s, Reachable false s /\ (forall b, Not (s.phase b = .held)) /\
      (forall b, Not (s.phase b = .dead)) /\ exists b, s.locked b = true := by
  have r0 : Reachable false init := Reachable.init
  have r1 := r0.step (Step.openFile init 0 (by simp [init]))
  have r2 := r1.step (Step.acquire _ 0 (by simp) (by simp [init]))
  have r3 := r2.step (Step.fork _ 0 (by simp [State.isOpen]))
  have r4 := r3.step (Step.closeHeld _ 0 rfl (by simp))
  refine ⟨_, r4, ?_, ?_, 0, ?_⟩
  · intro b
    by_cases eb : b = 0 <;> simp [eb, init]
  · intro b
    by_cases eb : b = 0 <;> simp [eb, init]
  · simp [State.afterClose]

end Gds.R0019.FileLock
