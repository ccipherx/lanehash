import Mathlib.Tactic
import Std.Tactic.BVDecide
import Lanehash.Model

/-!
# T1, T3, T4, T7 and constant facts

* T1: every input byte is read, in every length class.
* T3: for every pair of rotations of a class, `k ↦ rotl k r ^ rotl k s` is at most 4-to-1,
  so two multiplies' secrets never differ by a constant the attacker can rely on.
* T4: the T-function structure of `a ↦ a ^ lo(a * b)` for odd `b`.
* T7: the AES sandwich and differential chains agree, for any round function.
* Constant facts used by the arguments: distinct `CT`, `Q`, `S`, and per-lane distinct `QT`.

`bv_decide` is used for the rotation facts. It checks an LRAT certificate inside Lean
but relies on `Lean.ofReduceBool`, which is listed in the trusted base.
-/

namespace Lanehash

/-! ## T1: coverage -/

/-- Long path: the leading stripes `[0, 64 m)` and the closing stripe `[n - 64, n)` cover
every byte, and the leading stripes stay inside the input. -/
theorem long_cover (n i : Nat) (hn : 65 ≤ n) (_hi : i < n) :
    (i < 64 * ((n - 1) / 64) ∨ n - 64 ≤ i) ∧ 64 * ((n - 1) / 64) ≤ n - 1 := by
  omega

/-- 1 to 3 bytes: bytes `0`, `n / 2` and `n - 1` are all of them. -/
theorem cover_le3 (n i : Nat) (h1 : 1 ≤ n) (h3 : n ≤ 3) (_hi : i < n) :
    i = 0 ∨ i = n / 2 ∨ i = n - 1 := by omega

/-- 4 to 8 bytes: two 4-byte reads at `0` and `n - 4`. -/
theorem cover_le8 (n i : Nat) (h4 : 4 ≤ n) (h8 : n ≤ 8) (_hi : i < n) :
    i < 4 ∨ (n - 4 ≤ i ∧ i < n) := by omega

/-- 9 to 16 bytes: two 8-byte reads at `0` and `n - 8`. -/
theorem cover_le16 (n i : Nat) (h9 : 9 ≤ n) (h16 : n ≤ 16) (_hi : i < n) :
    i < 8 ∨ n - 8 ≤ i := by omega

/-- 17 to 32 bytes: words at `0, 8` and `n - 16, n - 8`. -/
theorem cover_le32 (n i : Nat) (h17 : 17 ≤ n) (h32 : n ≤ 32) (_hi : i < n) :
    i < 16 ∨ n - 16 ≤ i := by omega

/-- 33 to 64 bytes: words at `0..32` and `n - 32..n`. -/
theorem cover_le64 (n i : Nat) (h33 : 33 ≤ n) (h64 : n ≤ 64) (_hi : i < n) :
    i < 32 ∨ n - 32 ≤ i := by omega

/-! ## T3: rotation differences of the secrets

For `ROT = [0, 19, 38, 57]` the differences are 19, 38, 57 (gcd with 64: 1, 2, 1). Two
keys with the same rotation difference differ by a word fixed by the rotation, and those
are 0 and all ones (gcd 1) or the four 2-periodic words (gcd 2). So each value of the
difference has at most 4 preimages, and with `ka` uniform (`Keys.ka_injective`) any fixed
value has probability at most `4 / 2^64 = 2^-62`. -/

theorem rot_0_19 (k l : BitVec 64)
    (h : k.rotateLeft 0 ^^^ k.rotateLeft 19 = l.rotateLeft 0 ^^^ l.rotateLeft 19) :
    k ^^^ l = 0 ∨ k ^^^ l = -1 := by bv_decide

theorem rot_0_38 (k l : BitVec 64)
    (h : k.rotateLeft 0 ^^^ k.rotateLeft 38 = l.rotateLeft 0 ^^^ l.rotateLeft 38) :
    k ^^^ l = 0 ∨ k ^^^ l = -1 ∨ k ^^^ l = 0x5555555555555555#64 ∨ k ^^^ l = 0xAAAAAAAAAAAAAAAA#64 := by
  bv_decide

theorem rot_0_57 (k l : BitVec 64)
    (h : k.rotateLeft 0 ^^^ k.rotateLeft 57 = l.rotateLeft 0 ^^^ l.rotateLeft 57) :
    k ^^^ l = 0 ∨ k ^^^ l = -1 := by bv_decide

theorem rot_19_38 (k l : BitVec 64)
    (h : k.rotateLeft 19 ^^^ k.rotateLeft 38 = l.rotateLeft 19 ^^^ l.rotateLeft 38) :
    k ^^^ l = 0 ∨ k ^^^ l = -1 := by bv_decide

theorem rot_19_57 (k l : BitVec 64)
    (h : k.rotateLeft 19 ^^^ k.rotateLeft 57 = l.rotateLeft 19 ^^^ l.rotateLeft 57) :
    k ^^^ l = 0 ∨ k ^^^ l = -1 ∨ k ^^^ l = 0x5555555555555555#64 ∨ k ^^^ l = 0xAAAAAAAAAAAAAAAA#64 := by
  bv_decide

theorem rot_38_57 (k l : BitVec 64)
    (h : k.rotateLeft 38 ^^^ k.rotateLeft 57 = l.rotateLeft 38 ^^^ l.rotateLeft 57) :
    k ^^^ l = 0 ∨ k ^^^ l = -1 := by bv_decide

/-! ## T4: the protected multiply for odd second operands -/

theorem xor_msb_eq_add (x : BitVec 64) : x ^^^ 0x8000000000000000#64 = x + 0x8000000000000000#64 := by
  bv_decide

theorem msb_mul_odd (b : BitVec 64) (hb : b.getLsbD 0 = true) :
    0x8000000000000000#64 * b = 0x8000000000000000#64 := by bv_decide

/-- **T4, top bit.** For odd `b`, the low half `a ^ lo(a * b)` of `mum a b` does not depend
on the top bit of `a`: flipping it changes nothing. -/
theorem mum_lo_ignores_msb (a b : BitVec 64) (hb : b.getLsbD 0 = true) :
    (a ^^^ 0x8000000000000000#64) ^^^ ((a ^^^ 0x8000000000000000#64) * b) = a ^^^ (a * b) := by
  rw [xor_msb_eq_add a, BitVec.add_mul, msb_mul_odd b hb, ← xor_msb_eq_add a, ← xor_msb_eq_add (a * b)]
  rw [BitVec.xor_assoc, BitVec.xor_comm (a * b), ← BitVec.xor_assoc 0x8000000000000000#64 0x8000000000000000#64,
    BitVec.xor_self, BitVec.zero_xor]

/-- Bit 0 of a product is the AND of the operands' bits 0. -/
theorem mul_bit0 (a b : BitVec 64) : (a * b).getLsbD 0 = (a.getLsbD 0 && b.getLsbD 0) := by
  bv_decide

/-- **T4, bit 0.** For odd `b`, bit 0 of `a ^ lo(a * b)` is always 0. -/
theorem mum_lo_bit0 (a b : BitVec 64) (hb : b.getLsbD 0 = true) :
    (a ^^^ (a * b)).getLsbD 0 = false := by
  have h : (a * b).getLsbD 0 = (a.getLsbD 0 && b.getLsbD 0) := mul_bit0 a b
  rw [BitVec.getLsbD_xor, h, hb, Bool.and_true, Bool.xor_self]

/-! ## T7: AES sandwich and differential forms -/

/-- **T7.** With the round written as `R(s, k) = F(s) ^ k`, the sandwich chain
`s (k+1) = R(s k ^ b (k+1), b (k+1))` and the differential chain
`x (k+1) = R(x k, b (k+1) ^ b (k+2))`, started at `x 0 = s 0 ^ b 1`, satisfy
`x k = s k ^ b (k+1)` for every `k`, for every function `F`. -/
theorem sandwich_eq_differential (F : Nat → Nat) (b s x : Nat → Nat)
    (hs : ∀ k, s (k + 1) = F (s k ^^^ b (k + 1)) ^^^ b (k + 1))
    (hx0 : x 0 = s 0 ^^^ b 1)
    (hx : ∀ k, x (k + 1) = F (x k) ^^^ (b (k + 1) ^^^ b (k + 2))) :
    ∀ k, x k = s k ^^^ b (k + 1) := by
  intro k
  induction k with
  | zero => exact hx0
  | succ k ih =>
    rw [hx, ih, hs, Nat.xor_assoc]

/-! ## Constant facts -/

theorem CT_nodup : ((List.range 64).map CT).Nodup := by decide +kernel
theorem Q_nodup : ((List.range 8).map Q).Nodup := by decide +kernel
theorem S_nodup : ((List.range 19).map S).Nodup := by decide +kernel

/-- In every lane, the eight chains meet eight different merge constants. -/
theorem QT_lane_nodup : ∀ i < 8, ((List.range 8).map fun c => QT (8 * c + i)).Nodup := by
  decide +kernel

end Lanehash
