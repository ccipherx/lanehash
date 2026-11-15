import Lanehash.Model

/-! Checks the model against the crate: the two SMHasher verification values, then every
line `len seed data_seed lo hi` of the corpus from `verification/diff`, whose input is
regenerated with the same xorshift64. Exit code 1 on any mismatch. -/

open Lanehash

def xorshift (s : Nat) : Nat :=
  let s := (s ^^^ (s <<< 13)) % W
  let s := s ^^^ (s >>> 7)
  (s ^^^ (s <<< 17)) % W

def gen (len ds : Nat) : Array Nat := Id.run do
  let mut s := ds
  let mut out := Array.mkEmpty len
  for _ in [0:len] do
    s := xorshift s
    out := out.push (s % 256)
  return out

def main (args : List String) : IO UInt32 := do
  let lines ← IO.FS.lines args.head!
  let mut bad := 0
  let mut n := 0
  for (name, got, want) in [("64", verification64, 0x36B435A3), ("128", verification128, 0xB5F2A65D)] do
    IO.println s!"verification value {name}-bit: {got} (expected {want})"
    if got != want then bad := bad + 1
  for l in lines do
    match (l.splitOn " ").map String.toNat! with
    | [len, seed, ds, lo, hi] =>
      let (a, b) := hash128 (gen len ds) seed
      n := n + 1
      if a != lo || b != hi then
        bad := bad + 1
        if bad ≤ 5 then IO.println s!"mismatch: {l} model {a} {b}"
    | _ => pure ()
  IO.println s!"{n} cases, {bad} mismatches"
  return (if bad == 0 then 0 else 1)
