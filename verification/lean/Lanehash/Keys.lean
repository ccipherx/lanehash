import Mathlib.Tactic
import Lanehash.Model

/-!
# T2: the distribution of the seed secrets

`keys seed`: `(lo, hi)` are the halves of `(seed ^ SEED0) * SEED1`, `ka = lo ^ KS0`,
`kb = lo ^ hi ^ KS1`, `ks = lo ^ hi`.

* `SEED1` is odd, so `x ↦ x * SEED1 mod 2^64` is a bijection: `ka` is an injective, hence
  bijective, function of the 64-bit seed. A uniform seed gives a uniform `ka`, and so a
  uniform `rotl ka r` for every rotation.
* `SEED1 > 2^63`, so `x ↦ ⌊x * SEED1 / 2^64⌋` takes each value at most twice on 64-bit
  `x`. Since `ka ^ kb = hi ^ KS0 ^ KS1`, a uniform seed makes `ka ^ kb` take any value with
  probability at most `2 / 2^64 = 2^-63`.
* Nothing comparable is proven for `kb` alone.
-/

namespace Lanehash

theorem SEED1_val : SEED1 = 0xf76c91b34799a125 := by decide +kernel

theorem SEED1_odd : SEED1 % 2 = 1 := by rw [SEED1_val]

theorem SEED1_big : 2 ^ 63 < SEED1 ∧ SEED1 < W := by
  rw [SEED1_val]
  norm_num [W]

/-- The inverse of `SEED1` modulo `2^64`. -/
def SEED1_inv : Nat := 0x8295b13a00fb92ad

theorem SEED1_inv_spec : SEED1 * SEED1_inv % W = 1 := by
  rw [SEED1_val]
  norm_num [SEED1_inv, W]

/-- Multiplication by `SEED1` is injective modulo `2^64`. -/
theorem mul_SEED1_inj (x y : Nat) (hx : x < W) (hy : y < W)
    (h : x * SEED1 % W = y * SEED1 % W) : x = y := by
  have h1 : x * SEED1 * SEED1_inv % W = y * SEED1 * SEED1_inv % W := by
    rw [Nat.mul_mod, h, ← Nat.mul_mod]
  have hx' : x * SEED1 * SEED1_inv % W = x := by
    rw [Nat.mul_assoc, Nat.mul_mod, SEED1_inv_spec, Nat.mul_one, Nat.mod_mod, Nat.mod_eq_of_lt hx]
  have hy' : y * SEED1 * SEED1_inv % W = y := by
    rw [Nat.mul_assoc, Nat.mul_mod, SEED1_inv_spec, Nat.mul_one, Nat.mod_mod, Nat.mod_eq_of_lt hy]
  rw [hx', hy'] at h1
  exact h1

theorem SEED0_lt : SEED0 < W := by
  have : SEED0 = 0x997f2cf145907cba := by decide +kernel
  rw [this]
  norm_num [W]

/-- **`ka` is an injective function of the 64-bit seed.** With finitely many seeds and
values it is a bijection, so a uniform seed gives a uniform `ka`. -/
theorem ka_injective (s t : Nat) (hs : s < W) (ht : t < W)
    (h : (keys s).ka = (keys t).ka) : s = t := by
  simp only [keys] at h
  have hlo : (s ^^^ SEED0) * SEED1 % W = (t ^^^ SEED0) * SEED1 % W := by
    have := congrArg (· ^^^ KS 0) h
    simpa [Nat.xor_assoc] using this
  have hxs : s ^^^ SEED0 < W := Nat.xor_lt_two_pow (by simpa [W] using hs) (by simpa [W] using SEED0_lt)
  have hxt : t ^^^ SEED0 < W := Nat.xor_lt_two_pow (by simpa [W] using ht) (by simpa [W] using SEED0_lt)
  have := mul_SEED1_inj _ _ hxs hxt hlo
  have h2 := congrArg (· ^^^ SEED0) this
  simpa [Nat.xor_assoc] using h2

/-- Two 64-bit multiplicands at distance 2 or more give different high halves. -/
theorem hi_far (x z : Nat) (hxz : x + 2 ≤ z) :
    x * SEED1 / W ≠ z * SEED1 / W := by
  intro h
  have hS := SEED1_big.1
  -- both products lie in [q W, (q + 1) W), but they are at least 2 SEED1 > W apart
  have h1 := Nat.div_mul_le_self (x * SEED1) W
  have h2 := Nat.lt_div_mul_add (a := z * SEED1) (b := W) (by norm_num [W])
  rw [← h] at h2
  have h3 : x * SEED1 + 2 * SEED1 ≤ z * SEED1 := by nlinarith
  have h4 : W < 2 * SEED1 := by norm_num [W] at hS ⊢; omega
  nlinarith

/-- **`hi` takes each value at most twice.** No three distinct 64-bit multiplicands share
the high half of their product with `SEED1`. -/
theorem hi_fiber_le_two (x y z : Nat) (hxy : x < y) (hyz : y < z)
    (h1 : x * SEED1 / W = y * SEED1 / W) (h2 : y * SEED1 / W = z * SEED1 / W) : False :=
  hi_far x z (by omega) (h1.trans h2)

/-- `ka ^ kb` depends only on `hi`. -/
theorem ka_xor_kb (seed : Nat) :
    (keys seed).ka ^^^ (keys seed).kb = (seed ^^^ SEED0) * SEED1 / W ^^^ KS 0 ^^^ KS 1 := by
  simp only [keys]
  generalize (seed ^^^ SEED0) * SEED1 % W = lo
  generalize (seed ^^^ SEED0) * SEED1 / W = hi
  generalize KS 0 = a
  generalize KS 1 = b
  calc lo ^^^ a ^^^ (lo ^^^ hi ^^^ b) = (lo ^^^ lo) ^^^ (hi ^^^ a ^^^ b) := by ac_rfl
    _ = hi ^^^ a ^^^ b := by rw [Nat.xor_self, Nat.zero_xor]

end Lanehash
