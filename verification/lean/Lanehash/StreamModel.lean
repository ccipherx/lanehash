import Mathlib.Tactic

/-!
# T8: `Stream` (default function) for every sequence of updates

A model of `stream.rs` in input positions. Between loop passes of `update` the state is
`b` blocks absorbed, `L` bytes buffered, `D` the input position of the next byte of
`data` and `R` the bytes of `data` left. The invariant `Inv`: the buffer holds the input
from `start b` (0, or `512 b - 64` with the carry) up to `D`.

Each branch of `update` is one lemma (`direct_ok`, `append_ok`, `block_ok`), and
`finish_ok` covers what `finish128` hands the kernels. All obligations are linear, solved
by `omega`. The model is a hand transcription, linked to the code by
`stream_equals_one_shot_at_every_split` and the `default_stream` fuzz target.
-/

set_option linter.unusedSimpArgs false

namespace Lanehash.StreamModel

def carry (b : Nat) : Nat := if b = 0 then 0 else 64
def start (b : Nat) : Nat := if b = 0 then 0 else 512 * b - 64

structure St where
  b : Nat
  L : Nat
  D : Nat
  R : Nat

/-- At every loop head: the buffer holds input positions `[start b, D)`, it fits in the
640-byte array, it holds at least the carry, and every absorbed block has 64 more input
bytes after it. -/
def Inv (s : St) : Prop :=
  start s.b + s.L = s.D ∧ s.L ≤ 640 ∧ carry s.b ≤ s.L ∧ (s.b = 0 ∨ 512 * s.b + 64 ≤ s.D + s.R)

/-- At a return of `update`: also fewer than 576 pending bytes. -/
def Done (s : St) : Prop := Inv s ∧ s.R = 0 ∧ s.L - carry s.b < 576

theorem init (n : Nat) : Inv ⟨0, 0, 0, n⟩ := by
  simp [Inv, start, carry]

/-- Branch 1: no pending bytes and `R ≥ 576`. -/
theorem direct_ok (s : St) (h : Inv s) (hp : s.L = carry s.b) (hr : 576 ≤ s.R) :
    let k := (s.R - 64) / 512
    1 ≤ k ∧ s.D = 512 * s.b ∧ 512 * k + 64 ≤ s.R ∧
      start (s.b + k) = s.D + 512 * k - 64 ∧
      Done ⟨s.b + k, s.R - (512 * k - 64), s.D + s.R, 0⟩ := by
  obtain ⟨h1, h2, h3, h4⟩ := h
  dsimp only
  simp only [Done, Inv, start, carry] at *
  split_ifs at *
  all_goals try simp only [true_and, and_true, add_zero, zero_add] at *
  all_goals omega

/-- Branch 2: fewer than 576 pending plus new bytes. -/
theorem append_ok (s : St) (h : Inv s) (hlt : s.L - carry s.b + s.R < 576) :
    Done ⟨s.b, s.L + s.R, s.D + s.R, 0⟩ := by
  obtain ⟨h1, h2, h3, h4⟩ := h
  simp only [Done, Inv, start, carry] at *
  split_ifs at *
  all_goals try simp only [true_and, and_true, add_zero, zero_add] at *
  all_goals omega

/-- Branch 3: one block through the buffer. `take` bytes complete it, the block is input
positions `[512 b, 512 b + 512)` at buffer offset `carry b`, then the buffer is shifted
by `carry b + 448`. -/
theorem block_ok (s : St) (h : Inv s) (hge : 576 ≤ s.L - carry s.b + s.R) :
    let take := if s.L - carry s.b < 512 then 512 - (s.L - carry s.b) else 0
    take ≤ s.R ∧ start s.b + carry s.b = 512 * s.b ∧ carry s.b + 512 ≤ s.L + take ∧
      512 * (s.b + 1) + 64 ≤ s.D + s.R ∧
      Inv ⟨s.b + 1, s.L + take - (carry s.b + 448), s.D + take, s.R - take⟩ ∧
      (s.R - take) + (s.L + take - (carry s.b + 448) - carry (s.b + 1)) + 512 = s.R + (s.L - carry s.b) := by
  obtain ⟨h1, h2, h3, h4⟩ := h
  dsimp only
  simp only [Inv, start, carry] at *
  split_ifs at *
  all_goals try simp only [true_and, and_true, add_zero, zero_add] at *
  all_goals omega

/-- `finish128` after the last update, `n = D > 64` bytes in total. With no block absorbed,
the whole input is in the buffer (fewer than 576 bytes, inside the 640-byte array). Else
`r = m - 8 b` leading stripes start at buffer offset 64 (input position `512 b`), lie in
the buffer, `r ≤ 8`, and the closing stripe at buffer offset `L - 64` is input
`[n - 64, n)`. -/
theorem finish_ok (s : St) (h : Done s) (hn : 64 < s.D) :
    (s.b = 0 → s.L = s.D ∧ s.D < 576) ∧
    (s.b ≠ 0 →
      let m := (s.D - 1) / 64
      8 * s.b ≤ m ∧ m - 8 * s.b ≤ 8 ∧ 64 + 64 * (m - 8 * s.b) ≤ s.L ∧ 64 ≤ s.L ∧
        start s.b + 64 = 512 * s.b ∧ start s.b + (s.L - 64) = s.D - 64) := by
  obtain ⟨⟨h1, h2, h3, h4⟩, hr, h5⟩ := h
  dsimp only
  simp only [start, carry] at *
  split_ifs at *
  all_goals try simp only [true_and, and_true, add_zero, zero_add] at *
  all_goals omega

/-- `finish128` with `n ≤ 64`: no block was absorbed and the buffer is the whole input,
which the short path hashes. -/
theorem finish_short (s : St) (h : Done s) (hn : s.D ≤ 64) : s.b = 0 ∧ s.L = s.D := by
  obtain ⟨⟨h1, h2, h3, h4⟩, hr, h5⟩ := h
  simp only [start, carry] at *
  split_ifs at *
  all_goals try simp only [true_and, and_true, add_zero, zero_add] at *
  all_goals omega

end Lanehash.StreamModel
