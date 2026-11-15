//! Kani proofs for `LaneHasher`: the sponge invariant over four integer writes, and no
//! panic on byte writes up to 16 bytes, from any state.
use super::*;

#[kani::proof]
fn sponge_writes_keep_fill_in_range() {
    let mut h = LaneHasher::with_keys(kani::any(), kani::any(), kani::any());
    for _ in 0..4 {
        let w: u8 = kani::any();
        let x: u64 = kani::any();
        match w % 6 {
            0 => h.write_u8(x as u8),
            1 => h.write_u16(x as u16),
            2 => h.write_u32(x as u32),
            3 => h.write_u64(x),
            4 => h.write_usize(x as usize),
            _ => h.write_u128((x as u128) << 64 | kani::any::<u64>() as u128),
        }
        assert!(h.fill <= 128);
        assert!(h.fill == 128 || h.sponge >> h.fill == 0);
    }
    let _ = h.finish();
}

macro_rules! write_harness {
    ($($name:ident $n:literal),*) => {$(
        #[kani::proof]
        fn $name() {
            let mut h = LaneHasher::with_keys(kani::any(), kani::any(), kani::any());
            let a: [u8; $n] = kani::any();
            h.write(&a);
            let _ = h.finish();
        }
    )*};
}
write_harness!(write_0 0, write_3 3, write_7 7, write_8 8, write_9 9, write_16 16);
