import Mathlib.Tactic
import Lanehash.Model

/-!
# T5: a difference in one half of a lane never vanishes in one step

`step a w = f (a ^ w)` with `f x = (x + lo32(x) * hi32(x)) mod 2^64`. Writing
`x = hi * 2^32 + lo`, `f x = (hi * (2^32 + lo) + lo) mod 2^64 = (hi * 2^32 + lo * (1 + hi)) mod 2^64`.

* A change of `lo` alone changes `f x` by `(lo' - lo) * (1 + hi)`, nonzero and below `2^64`.
* A change of `hi` alone changes it by `(hi' - hi) * (2^32 + lo)`, which is below `2^65`.
  It is a multiple of `2^64` only if it equals `2^64`, which needs `2^32 + lo` to be a
  power of two, so `lo = 0`, and then `hi' - hi = 2^32`, out of range.

So for every lane state and every XOR difference confined to one 32-bit half, the step
output differs.
-/

namespace Lanehash

/-- The lane map applied after the XOR: `step a w = stepX (a ^ w)`. -/
def stepX (x : Nat) : Nat := (x + (x % 2 ^ 32) * (x / 2 ^ 32)) % W

theorem step_eq_stepX (a w : Nat) : step a w = stepX (a ^^^ w) := rfl

/-- The step in halves. -/
theorem stepX_halves (x : Nat) :
    stepX x = (x / 2 ^ 32 * 2 ^ 32 + x % 2 ^ 32 * (1 + x / 2 ^ 32)) % W := by
  unfold stepX
  congr 1
  have h := Nat.div_add_mod x (2 ^ 32)
  nlinarith [h]

/-- Arithmetic core, low half: equal high halves, different low halves. -/
theorem low_half_core (hi lo lo' : Nat) (hhi : hi < 2 ^ 32) (hlo : lo < 2 ^ 32)
    (hlo' : lo' < 2 ^ 32) (hne : lo ≠ lo') :
    (hi * 2 ^ 32 + lo * (1 + hi)) % W ≠ (hi * 2 ^ 32 + lo' * (1 + hi)) % W := by
  intro h
  rcases Nat.lt_or_gt_of_ne hne with hlt | hlt
  · -- lo < lo'
    have hm : (hi * 2 ^ 32 + lo * (1 + hi)) ≡ (hi * 2 ^ 32 + lo' * (1 + hi)) [MOD W] := h
    have hle : hi * 2 ^ 32 + lo * (1 + hi) ≤ hi * 2 ^ 32 + lo' * (1 + hi) := by
      have := Nat.mul_le_mul_right (1 + hi) hlt.le
      omega
    have hd := (Nat.modEq_iff_dvd' hle).1 hm
    have hdiff : hi * 2 ^ 32 + lo' * (1 + hi) - (hi * 2 ^ 32 + lo * (1 + hi)) = (lo' - lo) * (1 + hi) := by
      rw [Nat.sub_mul]
      have := Nat.mul_le_mul_right (1 + hi) hlt.le
      omega
    rw [hdiff] at hd
    have hpos : 0 < (lo' - lo) * (1 + hi) := Nat.mul_pos (by omega) (by omega)
    have hlt2 : (lo' - lo) * (1 + hi) < W := by
      have h1 : lo' - lo ≤ 2 ^ 32 - 1 := by omega
      have h2 : 1 + hi ≤ 2 ^ 32 := by omega
      calc (lo' - lo) * (1 + hi) ≤ (2 ^ 32 - 1) * 2 ^ 32 := Nat.mul_le_mul h1 h2
        _ < W := by norm_num [W]
    have := Nat.eq_zero_of_dvd_of_lt hd hlt2
    omega
  · -- lo' < lo, symmetric
    have hm : (hi * 2 ^ 32 + lo' * (1 + hi)) ≡ (hi * 2 ^ 32 + lo * (1 + hi)) [MOD W] := h.symm
    have hle : hi * 2 ^ 32 + lo' * (1 + hi) ≤ hi * 2 ^ 32 + lo * (1 + hi) := by
      have := Nat.mul_le_mul_right (1 + hi) hlt.le
      omega
    have hd := (Nat.modEq_iff_dvd' hle).1 hm
    have hdiff : hi * 2 ^ 32 + lo * (1 + hi) - (hi * 2 ^ 32 + lo' * (1 + hi)) = (lo - lo') * (1 + hi) := by
      rw [Nat.sub_mul]
      have := Nat.mul_le_mul_right (1 + hi) hlt.le
      omega
    rw [hdiff] at hd
    have hpos : 0 < (lo - lo') * (1 + hi) := Nat.mul_pos (by omega) (by omega)
    have hlt2 : (lo - lo') * (1 + hi) < W := by
      have h1 : lo - lo' ≤ 2 ^ 32 - 1 := by omega
      have h2 : 1 + hi ≤ 2 ^ 32 := by omega
      calc (lo - lo') * (1 + hi) ≤ (2 ^ 32 - 1) * 2 ^ 32 := Nat.mul_le_mul h1 h2
        _ < W := by norm_num [W]
    have := Nat.eq_zero_of_dvd_of_lt hd hlt2
    omega

/-- If `d * e` is a positive multiple of `2^64` below `2^65`, with `d < 2^32` and
`2^32 ≤ e < 2^33`, contradiction. -/
theorem no_multiple (d e : Nat) (hd0 : 0 < d) (hd : d < 2 ^ 32) (he0 : 2 ^ 32 ≤ e)
    (he : e < 2 ^ 33) (hdvd : W ∣ d * e) : False := by
  obtain ⟨c, hc⟩ := hdvd
  have hpos : 0 < d * e := Nat.mul_pos hd0 (by omega)
  have hbig : d * e < 2 * W := by
    calc d * e < 2 ^ 32 * 2 ^ 33 := Nat.mul_lt_mul_of_lt_of_lt hd he
      _ = 2 * W := by norm_num [W]
  have hc1 : c = 1 := by
    rcases c with _ | _ | c
    · simp at hc
      omega
    · rfl
    · exfalso
      have : W * (c + 1 + 1) ≥ 2 * W := by nlinarith
      omega
  subst hc1
  have hc' : d * e = 2 ^ 64 := by simpa [W] using hc
  have hpow : e ∣ 2 ^ 64 := ⟨d, by rw [← hc', Nat.mul_comm]⟩
  obtain ⟨k, hk, rfl⟩ := (Nat.dvd_prime_pow Nat.prime_two).1 hpow
  have hk32 : 32 ≤ k := by
    by_contra h
    rw [not_le] at h
    have : 2 ^ k < 2 ^ 32 := Nat.pow_lt_pow_right (by norm_num) h
    omega
  have hk33 : k < 33 := by
    by_contra h
    rw [not_lt] at h
    have : 2 ^ 33 ≤ 2 ^ k := Nat.pow_le_pow_right (by norm_num) h
    omega
  have hk : k = 32 := by omega
  subst hk
  have : d * 2 ^ 32 = 2 ^ 32 * 2 ^ 32 := by
    have : d * 2 ^ 32 = W * 1 := hc
    simpa [W] using this
  have := Nat.eq_of_mul_eq_mul_right (by norm_num) this
  omega

/-- Arithmetic core, high half: equal low halves, different high halves. -/
theorem high_half_core (hi hi' lo : Nat) (hhi : hi < 2 ^ 32) (hhi' : hi' < 2 ^ 32)
    (hlo : lo < 2 ^ 32) (hne : hi ≠ hi') :
    (hi * 2 ^ 32 + lo * (1 + hi)) % W ≠ (hi' * 2 ^ 32 + lo * (1 + hi')) % W := by
  intro h
  have key : ∀ a b : Nat, a < b → b < 2 ^ 32 →
      (a * 2 ^ 32 + lo * (1 + a)) % W = (b * 2 ^ 32 + lo * (1 + b)) % W → False := by
    intro a b hab hb hab'
    have hm : (a * 2 ^ 32 + lo * (1 + a)) ≡ (b * 2 ^ 32 + lo * (1 + b)) [MOD W] := hab'
    have hle : a * 2 ^ 32 + lo * (1 + a) ≤ b * 2 ^ 32 + lo * (1 + b) := by nlinarith
    have hd := (Nat.modEq_iff_dvd' hle).1 hm
    have hdiff : b * 2 ^ 32 + lo * (1 + b) - (a * 2 ^ 32 + lo * (1 + a)) = (b - a) * (2 ^ 32 + lo) := by
      obtain ⟨t, rfl⟩ : ∃ t, b = a + t := ⟨b - a, by omega⟩
      simp only [Nat.add_sub_cancel_left]
      ring_nf
      omega
    rw [hdiff] at hd
    exact no_multiple (b - a) (2 ^ 32 + lo) (by omega) (by omega) (by omega) (by omega) hd
  rcases Nat.lt_or_gt_of_ne hne with hlt | hlt
  · exact key hi hi' hlt hhi' h
  · exact key hi' hi hlt hhi h.symm

/-- XOR with `d < 2^32` keeps the high half and XORs the low half. -/
theorem xor_low (x d : Nat) (hd : d < 2 ^ 32) :
    (x ^^^ d) / 2 ^ 32 = x / 2 ^ 32 ∧ (x ^^^ d) % 2 ^ 32 = (x % 2 ^ 32) ^^^ d := by
  constructor
  · rw [← Nat.shiftRight_eq_div_pow, ← Nat.shiftRight_eq_div_pow, Nat.shiftRight_xor_distrib]
    have : d >>> 32 = 0 := by rw [Nat.shiftRight_eq_div_pow]; exact Nat.div_eq_of_lt hd
    rw [this, Nat.xor_zero]
  · rw [← Nat.and_two_pow_sub_one_eq_mod, ← Nat.and_two_pow_sub_one_eq_mod, Nat.and_xor_distrib_right]
    congr 1
    rw [Nat.and_two_pow_sub_one_eq_mod, Nat.mod_eq_of_lt hd]

/-- XOR with `d * 2^32`, `d < 2^32`, keeps the low half and XORs the high half. -/
theorem xor_high (x d : Nat) :
    (x ^^^ (d * 2 ^ 32)) % 2 ^ 32 = x % 2 ^ 32 ∧ (x ^^^ (d * 2 ^ 32)) / 2 ^ 32 = (x / 2 ^ 32) ^^^ d := by
  have hs : d * 2 ^ 32 = d <<< 32 := by rw [Nat.shiftLeft_eq]
  constructor
  · rw [← Nat.and_two_pow_sub_one_eq_mod, ← Nat.and_two_pow_sub_one_eq_mod, Nat.and_xor_distrib_right]
    have : (d * 2 ^ 32) &&& (2 ^ 32 - 1) = 0 := by
      rw [Nat.and_two_pow_sub_one_eq_mod]; simp
    rw [this, Nat.xor_zero]
  · rw [← Nat.shiftRight_eq_div_pow, ← Nat.shiftRight_eq_div_pow, Nat.shiftRight_xor_distrib, hs,
      Nat.shiftLeft_shiftRight]

/-- **T5, low half.** For every 64-bit lane input `x` and every nonzero difference
`d < 2^32`, `step` gives different outputs on `x` and `x ^ d`. -/
theorem stepX_low_ne (x d : Nat) (hx : x < W) (hd0 : 0 < d) (hd : d < 2 ^ 32) :
    stepX (x ^^^ d) ≠ stepX x := by
  obtain ⟨hh, hl⟩ := xor_low x d hd
  rw [stepX_halves, stepX_halves, hh, hl]
  have hhi : x / 2 ^ 32 < 2 ^ 32 := by
    rw [Nat.div_lt_iff_lt_mul (by norm_num)]; simpa [W] using hx
  have hne : (x % 2 ^ 32) ^^^ d ≠ x % 2 ^ 32 := by
    intro h
    have : d = 0 := by
      have h2 := congrArg ((x % 2 ^ 32) ^^^ ·) h
      simpa [← Nat.xor_assoc] using h2
    omega
  have hlt : (x % 2 ^ 32) ^^^ d < 2 ^ 32 := Nat.xor_lt_two_pow (Nat.mod_lt _ (by norm_num)) hd
  exact low_half_core _ _ _ hhi hlt (Nat.mod_lt _ (by norm_num)) hne

/-- **T5, high half.** For every 64-bit lane input `x` and every nonzero `d < 2^32`,
`step` gives different outputs on `x` and `x ^ (d * 2^32)`. -/
theorem stepX_high_ne (x d : Nat) (hx : x < W) (hd0 : 0 < d) (hd : d < 2 ^ 32) :
    stepX (x ^^^ (d * 2 ^ 32)) ≠ stepX x := by
  obtain ⟨hl, hh⟩ := xor_high x d
  rw [stepX_halves, stepX_halves, hh, hl]
  have hhi : x / 2 ^ 32 < 2 ^ 32 := by
    rw [Nat.div_lt_iff_lt_mul (by norm_num)]; simpa [W] using hx
  have hne : (x / 2 ^ 32) ^^^ d ≠ x / 2 ^ 32 := by
    intro h
    have : d = 0 := by
      have h2 := congrArg ((x / 2 ^ 32) ^^^ ·) h
      simpa [← Nat.xor_assoc] using h2
    omega
  have hlt : (x / 2 ^ 32) ^^^ d < 2 ^ 32 := Nat.xor_lt_two_pow hhi hd
  exact high_half_core _ _ _ hlt hhi (Nat.mod_lt _ (by norm_num)) hne

/-- **T5 for `step`.** A nonzero input difference confined to one 32-bit half of a word
never vanishes in the lane step, whatever the lane state. -/
theorem step_one_half_difference (a w d : Nat) (ha : a < W) (hw : w < W) (hd0 : 0 < d)
    (hd : d < 2 ^ 32) :
    step a (w ^^^ d) ≠ step a w ∧ step a (w ^^^ d * 2 ^ 32) ≠ step a w := by
  have hx : a ^^^ w < W := Nat.xor_lt_two_pow (by simpa [W] using ha) (by simpa [W] using hw)
  constructor
  · rw [step_eq_stepX, step_eq_stepX, ← Nat.xor_assoc]
    exact stepX_low_ne _ _ hx hd0 hd
  · rw [step_eq_stepX, step_eq_stepX, ← Nat.xor_assoc]
    exact stepX_high_ne _ _ hx hd0 hd

end Lanehash
