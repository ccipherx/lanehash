//! `LaneHasher` under an arbitrary sequence of `Hasher` writes: no panic, no
//! out-of-bounds read (`write_long` indexes from both ends of the slice), and the
//! same sequence on a second hasher from the same `FixedState` gives the same result.
#![no_main]
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use std::hash::{BuildHasher, Hasher};

#[derive(Arbitrary, Debug)]
enum Op<'a> {
    Bytes(&'a [u8]),
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    U128(u128),
    Usize(usize),
}

#[derive(Arbitrary, Debug)]
struct Input<'a> {
    seed: u64,
    ops: Vec<Op<'a>>,
}

fn apply(h: &mut lanehash::aes::LaneHasher, op: &Op) {
    match *op {
        Op::Bytes(b) => {
            let b = b.to_vec(); // exact-size allocation: an over-read is an ASan error
            h.write(&b)
        }
        Op::U8(v) => h.write_u8(v),
        Op::U16(v) => h.write_u16(v),
        Op::U32(v) => h.write_u32(v),
        Op::U64(v) => h.write_u64(v),
        Op::U128(v) => h.write_u128(v),
        Op::Usize(v) => h.write_usize(v),
    }
}

fuzz_target!(|input: Input| {
    let bh = lanehash::aes::FixedState::new(input.seed);
    let (mut a, mut b) = (bh.build_hasher(), bh.build_hasher());
    for op in &input.ops {
        apply(&mut a, op);
        apply(&mut b, op);
    }
    assert_eq!(a.finish(), b.finish());
});
