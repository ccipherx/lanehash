//! Kani proofs for the 0 to 64 byte path. One harness per length, over an array of
//! exactly that length, so a read past the input is a read past the allocation.
use super::*;

fn le32_ref(p: &[u8], i: usize) -> u64 {
    u32::from_le_bytes(p[i..i + 4].try_into().unwrap()) as u64
}

/// `words16` with checked reads and branches.
fn words16_ref(p: &[u8]) -> (u64, u64) {
    let len = p.len();
    if len >= 9 {
        (le64(p, 0), le64(p, len - 8))
    } else if len >= 4 {
        (le32_ref(p, 0) | le32_ref(p, len - 4) << 32, 0)
    } else if len > 0 {
        ((p[0] as u64) | ((p[len / 2] as u64) << 8) | ((p[len - 1] as u64) << 16), 0)
    } else {
        (0, 0)
    }
}

macro_rules! words16_harness {
    ($($name:ident $n:literal),*) => {$(
        /// `words16` reads only inside its input and equals the branchy reference.
        #[kani::proof]
        fn $name() {
            let a: [u8; $n] = kani::any();
            assert_eq!(words16(&a), words16_ref(&a));
        }
    )*};
}
words16_harness!(words16_0 0, words16_1 1, words16_2 2, words16_3 3, words16_4 4, words16_5 5, words16_6 6, words16_7 7, words16_8 8, words16_9 9, words16_10 10, words16_11 11, words16_12 12, words16_13 13, words16_14 14, words16_15 15, words16_16 16);

macro_rules! class_harness {
    ($($name:ident $n:literal),*) => {$(
        /// Reads stay inside the input and nothing panics, for every input and secrets,
        /// in both widths.
        #[kani::proof]
        fn $name() {
            let a: [u8; $n] = kani::any();
            let (ka, kb): (u64, u64) = (kani::any(), kani::any());
            if $n <= 16 {
                let _ = (class_le16::<false>(&a, ka, kb), class_le16::<true>(&a, ka, kb));
            } else if $n <= 32 {
                let _ = (class_le32::<false>(&a, ka, kb), class_le32::<true>(&a, ka, kb));
            } else {
                let _ = (class_le64::<false>(&a, ka, kb), class_le64::<true>(&a, ka, kb));
            }
        }
    )*};
}
class_harness!(class_0 0, class_1 1, class_2 2, class_3 3, class_4 4, class_5 5, class_6 6, class_7 7, class_8 8, class_9 9, class_10 10, class_11 11, class_12 12, class_13 13, class_14 14, class_15 15, class_16 16, class_17 17, class_18 18, class_19 19, class_20 20, class_21 21, class_22 22, class_23 23, class_24 24, class_25 25, class_26 26, class_27 27, class_28 28, class_29 29, class_30 30, class_31 31, class_32 32, class_33 33, class_34 34, class_35 35, class_36 36, class_37 37, class_38 38, class_39 39, class_40 40, class_41 41, class_42 42, class_43 43, class_44 44, class_45 45, class_46 46, class_47 47, class_48 48, class_49 49, class_50 50, class_51 51, class_52 52, class_53 53, class_54 54, class_55 55, class_56 56, class_57 57, class_58 58, class_59 59, class_60 60, class_61 61, class_62 62, class_63 63, class_64 64);

/// A public second operand of 1 erases the first.
#[kani::proof]
fn mum_by_one_erases() {
    let a: u64 = kani::any();
    assert_eq!(mum(a, 1), (0, 1));
}
