import Lanehash.Model

/-!
# The single-stripe form of the long path

For 65 to 512 bytes, `m = (n - 1) / 64 < 8`, so every chain sees at most one stripe. The
Rust reference `spec.rs` (function `lanes`, branch `m < CHAINS`) then skips the state
array: for each stripe `s` in `0..=m` and lane `i` it XORs
`step(step(CT[8 s + i] + ks, le64(c, i) ^ x), QT[8 s + i])` into the merged lane `i`,
where `c` is the stripe at `64 s` with `x = 0` for `s < m`, and the closing stripe
`[n - 64, n)` with `x = n` for `s = m`.

`mergedSingle` transcribes that branch, and `merged_eq_single` proves it equal to `merged`
of `Model.lean`. Both sides reduce, lane by lane, to the XOR over `s ≤ m` of
`contrib p ks s i`.
-/

namespace Lanehash

/-- Merged lanes of `n` bytes for `65 ≤ n ≤ 512`, in the form of the `m < 8` branch of
`spec.rs`: stripe step and merge step back to back for each stripe, no state array. -/
def mergedSingle (p : Array Nat) (ks : Nat) : Array Nat :=
  let n := p.size
  let m := (n - 1) / 64
  (List.range (m + 1)).foldl (fun mv s =>
    let off := if s < m then 64 * s else n - 64
    let x := if s < m then 0 else n
    (List.range 8).foldl (fun mv i =>
      mv.set! i (mv[i]! ^^^ step (step ((CT (8 * s + i) + ks) % W) (le64 p (off + 8 * i) ^^^ x))
        (QT (8 * s + i)))) mv)
    (Array.replicate 8 0)

/-- What stripe `s` adds to merged lane `i` in the single-stripe form. -/
def contrib (p : Array Nat) (ks s i : Nat) : Nat :=
  let n := p.size
  let m := (n - 1) / 64
  let off := if s < m then 64 * s else n - 64
  let x := if s < m then 0 else n
  step (step ((CT (8 * s + i) + ks) % W) (le64 p (off + 8 * i) ^^^ x)) (QT (8 * s + i))

/-! ## Folds over `List.range k` -/

/-- A left fold over `0, 1, .., k - 1` that writes `f i v` at index `ix i = i`, where `v`
is the value there before the write. Each index is written once, so the result holds
`f j a[j]!` at `j < k` and `a[j]!` elsewhere. -/
theorem foldl_set_range {α : Type} [Inhabited α] (ix : Nat → Nat) (f : Nat → α → α)
    (a : Array α) (k : Nat) (hk : k ≤ a.size) (hix : ∀ i, i < k → ix i = i) :
    ((List.range k).foldl (fun b i => b.set! (ix i) (f i b[ix i]!)) a).size = a.size ∧
    ∀ j, ((List.range k).foldl (fun b i => b.set! (ix i) (f i b[ix i]!)) a)[j]! =
      if j < k then f j a[j]! else a[j]! := by
  induction k with
  | zero => simp
  | succ k ih =>
    obtain ⟨hs, hg⟩ := ih (by omega) (fun i hi => hix i (by omega))
    have hk' : ix k = k := hix k (by omega)
    rw [List.range_succ (n := k), List.foldl_append]
    simp only [List.foldl_cons, List.foldl_nil, hk']
    refine ⟨by rw [Array.size_set!, hs], fun j => ?_⟩
    by_cases hj : j = k
    · subst hj
      rw [Array.getElem!_set!_self _ _ _ (by omega), hg]
      simp
    · rw [Array.getElem!_set!_ne _ _ _ _ (Ne.symm hj), hg]
      by_cases hjk : j < k
      · simp [hjk, show j < k + 1 by omega]
      · simp [hjk, show ¬ j < k + 1 by omega]

theorem foldl_range_congr {β : Type} (f g : β → Nat → β) (k : Nat) (x : β)
    (h : ∀ b c, c < k → f b c = g b c) :
    (List.range k).foldl f x = (List.range k).foldl g x := by
  induction k with
  | zero => rfl
  | succ k ih =>
    rw [List.range_succ (n := k), List.foldl_append, List.foldl_append,
      ih (fun b c hc => h b c (by omega))]
    simp only [List.foldl_cons, List.foldl_nil]
    exact h _ _ (by omega)

/-! ## One stripe -/

theorem stripe_get (ch p : Array Nat) (off t i : Nat) (h : 8 ≤ ch.size) (hi : i < 8) :
    (stripe ch p off t)[i]! = step ch[i]! (le64 p (off + 8 * i) ^^^ t) := by
  have := (foldl_set_range (fun i => i) (fun i v => step v (le64 p (off + 8 * i) ^^^ t)) ch 8 h
    (fun _ _ => rfl)).2 i
  simp only [hi, ↓reduceIte] at this
  exact this

/-! ## The single-stripe loop -/

/-- After stripes `0, .., k - 1` of the single-stripe loop, lane `i` is the XOR of what
they add to it. -/
theorem single_fold (g : Nat → Nat → Nat) (k : Nat) :
    ((List.range k).foldl (fun mv s =>
        (List.range 8).foldl (fun mv i => mv.set! i (mv[i]! ^^^ g s i)) mv)
        (Array.replicate 8 0)).size = 8 ∧
    ∀ i, i < 8 → ((List.range k).foldl (fun mv s =>
        (List.range 8).foldl (fun mv i => mv.set! i (mv[i]! ^^^ g s i)) mv)
        (Array.replicate 8 0))[i]! = (List.range k).foldl (fun acc s => acc ^^^ g s i) 0 := by
  induction k with
  | zero =>
    refine ⟨by simp, fun i hi => ?_⟩
    simp [hi]
  | succ k ih =>
    obtain ⟨hs, hg⟩ := ih
    simp only [List.range_succ (n := k), List.foldl_append, List.foldl_cons, List.foldl_nil]
    obtain ⟨hs', hg'⟩ := foldl_set_range (fun i => i) (fun i v => v ^^^ g k i)
      ((List.range k).foldl (fun mv s =>
        (List.range 8).foldl (fun mv i => mv.set! i (mv[i]! ^^^ g s i)) mv)
        (Array.replicate 8 0)) 8 (by omega) (fun _ _ => rfl)
    refine ⟨by rw [hs', hs], fun i hi => ?_⟩
    rw [hg' i, hg i hi]
    simp only [hi, ↓reduceIte]

theorem mergedSingle_size (p : Array Nat) (ks : Nat) : (mergedSingle p ks).size = 8 :=
  (single_fold (contrib p ks) ((p.size - 1) / 64 + 1)).1

theorem mergedSingle_get (p : Array Nat) (ks i : Nat) (hi : i < 8) :
    (mergedSingle p ks)[i]! =
      (List.range ((p.size - 1) / 64 + 1)).foldl (fun acc s => acc ^^^ contrib p ks s i) 0 :=
  (single_fold (contrib p ks) ((p.size - 1) / 64 + 1)).2 i hi

/-! ## The chain form -/

theorem merged_size (p : Array Nat) (ks : Nat) : (merged p ks).size = 8 := by
  simp [merged]

theorem merged_get (p : Array Nat) (ks i : Nat) (h1 : 65 ≤ p.size) (h2 : p.size ≤ 512)
    (hi : i < 8) :
    (merged p ks)[i]! =
      (List.range ((p.size - 1) / 64 + 1)).foldl (fun acc s => acc ^^^ contrib p ks s i) 0 := by
  have hm8 : (p.size - 1) / 64 < 8 := by omega
  let init : Array (Array Nat) :=
    (Array.range 8).map fun c => (Array.range 8).map fun i => (CT (8 * c + i) + ks) % W
  have hinit_size : init.size = 8 := by simp [init]
  have hinit : ∀ c, c < 8 →
      init[c]!.size = 8 ∧ ∀ j, j < 8 → init[c]![j]! = (CT (8 * c + j) + ks) % W := by
    intro c hc
    refine ⟨by simp [init, hc], fun j hj => ?_⟩
    simp [init, hc, hj]
  obtain ⟨hlead_size, hlead⟩ := foldl_set_range (fun s => s % 8)
    (fun s ch => stripe ch p (64 * s) 0) init ((p.size - 1) / 64) (by omega)
    (fun s hs => Nat.mod_eq_of_lt (by omega))
  let lead := (List.range ((p.size - 1) / 64)).foldl
    (fun st s => st.set! (s % 8) (stripe st[s % 8]! p (64 * s) 0)) init
  let st := lead.set! ((p.size - 1) / 64 % 8)
    (stripe lead[(p.size - 1) / 64 % 8]! p (p.size - 64) p.size)
  show ((Array.range 8).map fun i => (List.range (min ((p.size - 1) / 64 + 1) 8)).foldl
    (fun acc c => acc ^^^ step st[c]![i]! (QT (8 * c + i))) 0)[i]! = _
  have hlead' : ∀ j, lead[j]! =
      if j < (p.size - 1) / 64 then stripe init[j]! p (64 * j) 0 else init[j]! := hlead
  have hlead_size' : lead.size = 8 := hinit_size ▸ hlead_size
  have hmod : (p.size - 1) / 64 % 8 = (p.size - 1) / 64 := Nat.mod_eq_of_lt hm8
  rw [getElem!_pos _ _ (by simpa using hi)]
  simp only [Array.getElem_map, Array.getElem_range]
  rw [show min ((p.size - 1) / 64 + 1) 8 = (p.size - 1) / 64 + 1 by omega]
  apply foldl_range_congr
  intro acc c hc
  congr 1
  obtain ⟨hcs, hcv⟩ := hinit c (by omega)
  by_cases hcm : c < (p.size - 1) / 64
  · have hst : st[c]! = stripe init[c]! p (64 * c) 0 := by
      show (lead.set! _ _)[c]! = _
      rw [Array.getElem!_set!_ne _ _ _ _ (by omega), hlead' c]
      simp only [hcm, ↓reduceIte]
    rw [hst, stripe_get _ _ _ _ _ (by omega) hi, hcv i hi]
    simp only [contrib, hcm, ↓reduceIte]
  · have hceq : c = (p.size - 1) / 64 := by omega
    have hst : st[c]! = stripe init[c]! p (p.size - 64) p.size := by
      show (lead.set! _ _)[c]! = _
      rw [hmod, hceq, Array.getElem!_set!_self _ _ _ (by omega), hlead' _]
      simp
    rw [hst, stripe_get _ _ _ _ _ (by omega) hi, hcv i hi]
    simp only [contrib, hcm, ↓reduceIte]

/-! ## The two forms agree -/

/-- For every input of 65 to 512 bytes (`m < 8`) and every `ks`, the chain form `merged`
and the single-stripe form `mergedSingle` give the same merged lanes. -/
theorem merged_eq_single (p : Array Nat) (ks : Nat) (h1 : 65 ≤ p.size) (h2 : p.size ≤ 512) :
    merged p ks = mergedSingle p ks := by
  apply Array.ext
  · rw [merged_size, mergedSingle_size]
  · intro i hi1 hi2
    have hi : i < 8 := by rwa [merged_size] at hi1
    rw [← getElem!_pos (merged p ks) i hi1, ← getElem!_pos (mergedSingle p ks) i hi2,
      merged_get p ks i h1 h2 hi, mergedSingle_get p ks i hi]

/-- `fold` is symmetric: `lo(a b) ^ hi(a b)` only depends on the product. -/
theorem fold_comm (a b : Nat) : fold a b = fold b a := by
  unfold fold
  rw [Nat.mul_comm]

end Lanehash
