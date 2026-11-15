import Lanehash.Constants

/-!
# An executable model of lanehash

Written from the documented algorithm, not transcribed from the Rust code. Words are
natural numbers below `2^64` and products are exact, so `lo` and `hi` are `% 2^64` and
`/ 2^64`. `Diff.lean` checks it against the crate.
-/

namespace Lanehash

def W : Nat := 2 ^ 64

/-- `lo(a * b) ^ hi(a * b)`. -/
def fold (a b : Nat) : Nat := (a * b % W) ^^^ (a * b / W)

/-- One lane step: `x = a ^ w`, then `x + lo32(x) * hi32(x)` modulo `2^64`. -/
def step (a w : Nat) : Nat :=
  let x := a ^^^ w
  (x + (x % 2 ^ 32) * (x / 2 ^ 32)) % W

/-- The protected multiply: `(a ^ lo(a * b), b ^ hi(a * b))`. -/
def mum (a b : Nat) : Nat × Nat := (a ^^^ (a * b % W), b ^^^ (a * b / W))

/-- Left rotation of a 64-bit word by `r < 64`. -/
def rotl (k r : Nat) : Nat := ((k <<< r) % W) ||| (k >>> (64 - r))

/-- Little-endian word of `len` bytes of `p` at `i`. -/
def le (p : Array Nat) (i len : Nat) : Nat :=
  (List.range len).foldl (fun acc j => acc + p[i + j]! * 256 ^ j) 0

def le64 (p : Array Nat) (i : Nat) : Nat := le p i 8
def le32 (p : Array Nat) (i : Nat) : Nat := le p i 4

/-! ## Constants (words of `C`, `spec.rs` and `short.rs`) -/

def lo64 (g : Nat) : Nat := le C[g]! 0 8
def hi64 (g : Nat) : Nat := le C[g]! 8 8
/-- Word `j` of the groups from `g` on: low half of `C[g]`, high half, low of `C[g+1]`... -/
def word (g j : Nat) : Nat := if j % 2 = 0 then lo64 (g + j / 2) else hi64 (g + j / 2)

def SEED0 : Nat := lo64 6
def SEED1 : Nat := hi64 6 ||| 1
/-- Fold secrets: 10 words from `C[1]`, then 9 words from `C[7]`. -/
def S (j : Nat) : Nat := if j < 10 then word 1 j else word 7 (j - 10)
/-- Chain start constants, 64 words from `C[12]`. -/
def CT (j : Nat) : Nat := word 12 j
/-- Merge constants, 8 words from `C[44]`. -/
def Q (j : Nat) : Nat := word 44 j
/-- Merge constant of chain `c`, lane `i` at index `8 c + i`: `Q[(i + c) mod 8]`. -/
def QT (j : Nat) : Nat := Q ((j % 8 + j / 8) % 8)
def KS (j : Nat) : Nat := word 48 j
def KF (j : Nat) : Nat := word 52 j
def ROT : List Nat := [0, 19, 38, 57]

/-! ## Seed secrets -/

structure Keys where
  ka : Nat
  kb : Nat
  ks : Nat

def keys (seed : Nat) : Keys :=
  let r := (seed ^^^ SEED0) * SEED1
  let lo := r % W
  let hi := r / W
  { ka := lo ^^^ KS 0, kb := lo ^^^ hi ^^^ KS 1, ks := lo ^^^ hi }

/-! ## 0 to 64 bytes -/

/-- The two words that carry all of `p`, `p.size ≤ 16`. -/
def words16 (p : Array Nat) : Nat × Nat :=
  let n := p.size
  if n = 0 then (0, 0)
  else if n ≤ 3 then (p[0]! + p[n / 2]! * 2 ^ 8 + p[n - 1]! * 2 ^ 16, 0)
  else if n ≤ 8 then (le32 p 0 + le32 p (n - 4) * 2 ^ 32, 0)
  else (le64 p 0, le64 p (n - 8))

/-- Final folds of the short path, low half and high half. -/
def finishShort (x y n : Nat) : Nat × Nat :=
  (fold (x ^^^ KF 0 ^^^ n) (y ^^^ KF 1), fold (x ^^^ KF 2) (y ^^^ KF 3 ^^^ n))

/-- Multiply `j` of a class: first operand keyed by `rotl ka (ROT j)`, second by
`rotl kb (ROT j)`, and `t` (the length for the last multiply, else 0) in the second. -/
def mulj (k : Keys) (j a b t : Nat) : Nat × Nat :=
  let r := ROT[j]!
  mum (a ^^^ rotl k.ka r) (b ^^^ (rotl k.kb r ^^^ t))

def short (p : Array Nat) (k : Keys) : Nat × Nat :=
  let n := p.size
  if n ≤ 16 then
    let (w0, w1) := words16 p
    let (x, y) := mulj k 0 w0 w1 n
    finishShort x y n
  else if n ≤ 32 then
    let (x0, y0) := mulj k 0 (le64 p 0) (le64 p 8) 0
    let (x1, y1) := mulj k 1 (le64 p (n - 16)) (le64 p (n - 8)) n
    finishShort (x0 ^^^ y1) (y0 ^^^ x1) n
  else
    let (x0, y0) := mulj k 0 (le64 p 0) (le64 p 8) 0
    let (x1, y1) := mulj k 1 (le64 p 16) (le64 p 24) 0
    let (x2, y2) := mulj k 2 (le64 p (n - 32)) (le64 p (n - 24)) 0
    let (x3, y3) := mulj k 3 (le64 p (n - 16)) (le64 p (n - 8)) n
    finishShort (x0 ^^^ y1 ^^^ x2 ^^^ y3) (y0 ^^^ x1 ^^^ y2 ^^^ x3) n

/-! ## 65 bytes and up -/

/-- One stripe at byte `off` into a chain, every word XORed with `t`. -/
def stripe (ch : Array Nat) (p : Array Nat) (off t : Nat) : Array Nat :=
  (List.range 8).foldl (fun ch i => ch.set! i (step ch[i]! (le64 p (off + 8 * i) ^^^ t))) ch

def merged (p : Array Nat) (ks : Nat) : Array Nat :=
  let n := p.size
  let m := (n - 1) / 64
  let init : Array (Array Nat) :=
    (Array.range 8).map fun c => (Array.range 8).map fun i => (CT (8 * c + i) + ks) % W
  let lead := (List.range m).foldl (fun st s => st.set! (s % 8) (stripe st[s % 8]! p (64 * s) 0)) init
  let st := lead.set! (m % 8) (stripe lead[m % 8]! p (n - 64) n)
  let used := min (m + 1) 8
  (Array.range 8).map fun i =>
    (List.range used).foldl (fun acc c => acc ^^^ step st[c]![i]! (QT (8 * c + i))) 0

def finishLong (mv : Array Nat) (ks n : Nat) : Nat × Nat :=
  let h := fold (mv[0]! ^^^ S 0) (mv[1]! ^^^ S 1) ^^^ fold (mv[2]! ^^^ S 2) (mv[3]! ^^^ S 3)
    ^^^ fold (mv[4]! ^^^ S 4) (mv[5]! ^^^ S 5) ^^^ fold (mv[6]! ^^^ S 6) (mv[7]! ^^^ S 7)
  let h2 := fold (mv[0]! ^^^ S 10) (mv[2]! ^^^ S 11) ^^^ fold (mv[1]! ^^^ S 12) (mv[3]! ^^^ S 13)
    ^^^ fold (mv[4]! ^^^ S 14) (mv[6]! ^^^ S 15) ^^^ fold (mv[5]! ^^^ S 16) (mv[7]! ^^^ S 17)
  (fold (h ^^^ n) (ks ^^^ S 8), fold (h2 ^^^ n) (ks ^^^ S 18))

/-! ## The functions -/

/-- `(low, high)` halves of the 128-bit hash. -/
def hash128 (p : Array Nat) (seed : Nat) : Nat × Nat :=
  let k := keys seed
  if p.size ≤ 64 then short p k else finishLong (merged p k.ks) k.ks p.size

def hash64 (p : Array Nat) (seed : Nat) : Nat := (hash128 p seed).1

def toBytes (x len : Nat) : Array Nat := (Array.range len).map fun j => x / 256 ^ j % 256

/-- SMHasher's verification value (`tests/vectors.rs`). -/
def verification64 : Nat :=
  let key := (Array.range 256)
  let hs := (List.range 256).foldl (fun acc i => acc ++ toBytes (hash64 (key.extract 0 i) (256 - i)) 8) #[]
  hash64 hs 0 % 2 ^ 32

def verification128 : Nat :=
  let key := (Array.range 256)
  let hs := (List.range 256).foldl (fun acc i =>
    let (l, h) := hash128 (key.extract 0 i) (256 - i)
    acc ++ toBytes (l + h * W) 16) #[]
  (hash128 hs 0).1 % 2 ^ 32

end Lanehash
