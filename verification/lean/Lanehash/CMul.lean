import Mathlib.Tactic
import Lanehash.Model

/-!
# The C port's 64x64 multiply without `__int128`

`lanehash-c/lanehash.c`, `lanehash_mul128` in the `#else` branch (built by `make test`
as `test_noint128`), with every `uint64_t` operation reduced modulo `2^64` as C does:

```
a0 = (uint32_t)a, a1 = a >> 32, b0 = (uint32_t)b, b1 = b >> 32
p00 = a0 * b0, p01 = a0 * b1, p10 = a1 * b0, p11 = a1 * b1
mid = (p00 >> 32) + (uint32_t)p01 + (uint32_t)p10
lo = (mid << 32) | (uint32_t)p00
hi = p11 + (p01 >> 32) + (p10 >> 32) + (mid >> 32)
```

`mul128C_correct`: for all 64-bit `a` and `b` this is the exact 128-bit product. CBMC is
weak at multiplier equivalence, which is why this one is proven here.
-/

namespace Lanehash

def mul128C (a b : Nat) : Nat × Nat :=
  let a0 := a % 2 ^ 32
  let a1 := a / 2 ^ 32
  let b0 := b % 2 ^ 32
  let b1 := b / 2 ^ 32
  let p00 := a0 * b0 % W
  let p01 := a0 * b1 % W
  let p10 := a1 * b0 % W
  let p11 := a1 * b1 % W
  let mid := (p00 / 2 ^ 32 + p01 % 2 ^ 32 + p10 % 2 ^ 32) % W
  ((mid * 2 ^ 32 % W) ||| (p00 % 2 ^ 32), (p11 + p01 / 2 ^ 32 + p10 / 2 ^ 32 + mid / 2 ^ 32) % W)

/-- `x * 2^32 ||| r = x * 2^32 + r` when `r < 2^32`. -/
theorem or_low (x r : Nat) (hr : r < 2 ^ 32) : x * 2 ^ 32 ||| r = x * 2 ^ 32 + r := by
  rw [← Nat.shiftLeft_eq, Nat.shiftLeft_add_eq_or_of_lt hr]

theorem mul128C_correct (a b : Nat) (ha : a < W) (hb : b < W) :
    mul128C a b = (a * b % W, a * b / W) := by
  have hW : W = 2 ^ 32 * 2 ^ 32 := by norm_num [W]
  obtain ⟨a1, a0, rfl, ha0⟩ : ∃ a1 a0, a = a1 * 2 ^ 32 + a0 ∧ a0 < 2 ^ 32 :=
    ⟨a / 2 ^ 32, a % 2 ^ 32, by omega, Nat.mod_lt _ (by norm_num)⟩
  obtain ⟨b1, b0, rfl, hb0⟩ : ∃ b1 b0, b = b1 * 2 ^ 32 + b0 ∧ b0 < 2 ^ 32 :=
    ⟨b / 2 ^ 32, b % 2 ^ 32, by omega, Nat.mod_lt _ (by norm_num)⟩
  have ha1 : a1 < 2 ^ 32 := by
    have h : a1 * 2 ^ 32 + a0 < 2 ^ 64 := ha
    omega
  have hb1 : b1 < 2 ^ 32 := by
    have h : b1 * 2 ^ 32 + b0 < 2 ^ 64 := hb
    omega
  have ma : (a1 * 2 ^ 32 + a0) % 2 ^ 32 = a0 := by rw [Nat.mul_comm, Nat.mul_add_mod, Nat.mod_eq_of_lt ha0]
  have da : (a1 * 2 ^ 32 + a0) / 2 ^ 32 = a1 := by
    rw [Nat.mul_comm, Nat.mul_add_div (by norm_num), Nat.div_eq_of_lt ha0, Nat.add_zero]
  have mb : (b1 * 2 ^ 32 + b0) % 2 ^ 32 = b0 := by rw [Nat.mul_comm, Nat.mul_add_mod, Nat.mod_eq_of_lt hb0]
  have db : (b1 * 2 ^ 32 + b0) / 2 ^ 32 = b1 := by
    rw [Nat.mul_comm, Nat.mul_add_div (by norm_num), Nat.div_eq_of_lt hb0, Nat.add_zero]
  have small : ∀ x y : Nat, x < 2 ^ 32 → y < 2 ^ 32 → x * y < W := fun x y hx hy => by
    rw [hW]; exact Nat.mul_lt_mul'' hx hy
  have e00 := Nat.mod_eq_of_lt (small _ _ ha0 hb0)
  have e01 := Nat.mod_eq_of_lt (small _ _ ha0 hb1)
  have e10 := Nat.mod_eq_of_lt (small _ _ ha1 hb0)
  have e11 := Nat.mod_eq_of_lt (small _ _ ha1 hb1)
  simp only [mul128C, ma, da, mb, db, e00, e01, e10, e11]
  have q00 : a0 * b0 / 2 ^ 32 < 2 ^ 32 := by
    rw [Nat.div_lt_iff_lt_mul (by norm_num)]; exact Nat.mul_lt_mul'' ha0 hb0
  have r01 : a0 * b1 % 2 ^ 32 < 2 ^ 32 := Nat.mod_lt _ (by norm_num)
  have r10 : a1 * b0 % 2 ^ 32 < 2 ^ 32 := Nat.mod_lt _ (by norm_num)
  have hmidW : a0 * b0 / 2 ^ 32 + a0 * b1 % 2 ^ 32 + a1 * b0 % 2 ^ 32 < W := by
    simp only [W]; omega
  rw [Nat.mod_eq_of_lt hmidW]
  generalize hmd : a0 * b0 / 2 ^ 32 + a0 * b1 % 2 ^ 32 + a1 * b0 % 2 ^ 32 = mid
  -- the schoolbook identity, in the integers
  have hab : (a1 * 2 ^ 32 + a0) * (b1 * 2 ^ 32 + b0)
      = (a1 * b1 + a0 * b1 / 2 ^ 32 + a1 * b0 / 2 ^ 32 + mid / 2 ^ 32) * W
        + ((mid % 2 ^ 32) * 2 ^ 32 + a0 * b0 % 2 ^ 32) := by
    have h00 := Nat.div_add_mod (a0 * b0) (2 ^ 32)
    have h01 := Nat.div_add_mod (a0 * b1) (2 ^ 32)
    have h10 := Nat.div_add_mod (a1 * b0) (2 ^ 32)
    have hm := Nat.div_add_mod mid (2 ^ 32)
    simp only [W]
    generalize a0 * b0 / 2 ^ 32 = Q00 at *
    generalize a0 * b0 % 2 ^ 32 = R00 at *
    generalize a0 * b1 / 2 ^ 32 = Q01 at *
    generalize a0 * b1 % 2 ^ 32 = R01 at *
    generalize a1 * b0 / 2 ^ 32 = Q10 at *
    generalize a1 * b0 % 2 ^ 32 = R10 at *
    generalize mid / 2 ^ 32 = QM at *
    generalize mid % 2 ^ 32 = RM at *
    subst hmd
    linear_combination h00.symm + 2 ^ 32 * h01.symm + 2 ^ 32 * h10.symm + 2 ^ 32 * hm.symm
  have hlo : (mid % 2 ^ 32) * 2 ^ 32 + a0 * b0 % 2 ^ 32 < W := by
    have h1 : mid % 2 ^ 32 < 2 ^ 32 := Nat.mod_lt _ (by norm_num)
    have h2 : a0 * b0 % 2 ^ 32 < 2 ^ 32 := Nat.mod_lt _ (by norm_num)
    calc (mid % 2 ^ 32) * 2 ^ 32 + a0 * b0 % 2 ^ 32 < (mid % 2 ^ 32) * 2 ^ 32 + 2 ^ 32 :=
          Nat.add_lt_add_left h2 _
      _ = (mid % 2 ^ 32 + 1) * 2 ^ 32 := by ring
      _ ≤ 2 ^ 32 * 2 ^ 32 := Nat.mul_le_mul_right _ h1
      _ = W := hW.symm
  have hmod : (a1 * 2 ^ 32 + a0) * (b1 * 2 ^ 32 + b0) % W = (mid % 2 ^ 32) * 2 ^ 32 + a0 * b0 % 2 ^ 32 := by
    rw [hab, Nat.mul_add_mod_of_lt hlo]
  have hdiv : (a1 * 2 ^ 32 + a0) * (b1 * 2 ^ 32 + b0) / W
      = a1 * b1 + a0 * b1 / 2 ^ 32 + a1 * b0 / 2 ^ 32 + mid / 2 ^ 32 := by
    rw [hab, Nat.mul_comm _ W, Nat.mul_add_div (by norm_num [W]), Nat.div_eq_of_lt hlo, Nat.add_zero]
  have hhi : a1 * b1 + a0 * b1 / 2 ^ 32 + a1 * b0 / 2 ^ 32 + mid / 2 ^ 32 < W := by
    rw [← hdiv]
    exact Nat.div_lt_of_lt_mul (Nat.mul_lt_mul'' ha hb)
  have hshift : mid * 2 ^ 32 % W = (mid % 2 ^ 32) * 2 ^ 32 := by
    rw [hW, Nat.mul_mod_mul_right]
  rw [hshift, or_low _ _ (Nat.mod_lt _ (by norm_num)), Nat.mod_eq_of_lt hhi, hmod, hdiv]

end Lanehash
