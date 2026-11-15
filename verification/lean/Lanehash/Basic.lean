import Lanehash.Step
import Lanehash.Keys
import Lanehash.Algebra
import Lanehash.CMul
import Lanehash.StreamModel
import Lanehash.SingleStripe

/-! The axioms each main theorem rests on, printed on every build. Only `propext`,
`Classical.choice` and `Quot.sound` are expected, plus `Lean.ofReduceBool` for the theorems
proven with `bv_decide` (the rotation facts and the T4 bit lemmas). -/

open Lanehash

#print axioms step_one_half_difference
#print axioms ka_injective
#print axioms hi_fiber_le_two
#print axioms ka_xor_kb
#print axioms long_cover
#print axioms rot_0_19
#print axioms rot_0_38
#print axioms rot_19_57
#print axioms mum_lo_ignores_msb
#print axioms mum_lo_bit0
#print axioms sandwich_eq_differential
#print axioms CT_nodup
#print axioms QT_lane_nodup
#print axioms mul128C_correct
#print axioms Lanehash.StreamModel.direct_ok
#print axioms Lanehash.StreamModel.block_ok
#print axioms Lanehash.StreamModel.finish_ok
#print axioms Lanehash.merged_eq_single
#print axioms Lanehash.fold_comm
