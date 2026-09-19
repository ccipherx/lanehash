//! Software AES rounds must match each other and the hardware implementation.

/// T-table (`soft`) matches the byte-wise (`spec`) definition on random blocks and every byte value at every position.
#[test]
fn ttable_round_equals_spec() {
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    let mut rng = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    for _ in 0..if cfg!(miri) { 2_000 } else { 65_536 } {
        let mut x = [0u8; 16];
        let mut k = [0u8; 16];
        for i in 0..16 {
            x[i] = rng() as u8;
            k[i] = rng() as u8;
        }
        assert_eq!(lanehash::soft::aes_round(x, k), lanehash::spec::aes_round(x, k));
    }
    for pos in 0..16 {
        for v in 0..=255u8 {
            let mut x = [0u8; 16];
            x[pos] = v;
            assert_eq!(lanehash::soft::aes_round(x, [0; 16]), lanehash::spec::aes_round(x, [0; 16]), "pos {pos} value {v}");
        }
    }
}
#[cfg(target_arch = "x86_64")]
#[test]
fn software_round_equals_aesni() {
    use core::arch::x86_64::*;
    if !std::is_x86_feature_detected!("aes") {
        eprintln!("no AES-NI; skipped");
        return;
    }
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    let mut rng = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    for _ in 0..if cfg!(miri) { 2_000 } else { 200_000 } {
        let mut x = [0u8; 16];
        let mut k = [0u8; 16];
        for i in 0..16 {
            x[i] = rng() as u8;
            k[i] = rng() as u8;
        }
        let want = lanehash::spec::aes_round(x, k);
        let got = unsafe {
            let r = _mm_aesenc_si128(_mm_loadu_si128(x.as_ptr() as *const __m128i), _mm_loadu_si128(k.as_ptr() as *const __m128i));
            let mut o = [0u8; 16];
            _mm_storeu_si128(o.as_mut_ptr() as *mut __m128i, r);
            o
        };
        assert_eq!(got, want);
    }
}

/// `R(x, k) = vaesmcq_u8(vaeseq_u8(x, 0)) ^ k`, and the
/// absorb form `vaesmcq_u8(vaeseq_u8(t, b)) ^ b == R(t ^ b, b)`.
#[cfg(target_arch = "aarch64")]
#[test]
fn software_round_equals_neon() {
    use core::arch::aarch64::*;
    if !std::arch::is_aarch64_feature_detected!("aes") {
        eprintln!("no Armv8 AES; skipped");
        return;
    }
    let mut s = 0x9E37_79B9_7F4A_7C15u64;
    let mut rng = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        s
    };
    for _ in 0..if cfg!(miri) { 2_000 } else { 200_000 } {
        let mut x = [0u8; 16];
        let mut k = [0u8; 16];
        for i in 0..16 {
            x[i] = rng() as u8;
            k[i] = rng() as u8;
        }
        let (got, got_absorb) = unsafe {
            let (xv, kv) = (vld1q_u8(x.as_ptr()), vld1q_u8(k.as_ptr()));
            let r = veorq_u8(vaesmcq_u8(vaeseq_u8(xv, vdupq_n_u8(0))), kv);
            let a = veorq_u8(vaesmcq_u8(vaeseq_u8(xv, kv)), kv);
            let (mut o, mut oa) = ([0u8; 16], [0u8; 16]);
            vst1q_u8(o.as_mut_ptr(), r);
            vst1q_u8(oa.as_mut_ptr(), a);
            (o, oa)
        };
        assert_eq!(got, lanehash::spec::aes_round(x, k));
        let mut xk = [0u8; 16];
        for i in 0..16 {
            xk[i] = x[i] ^ k[i];
        }
        assert_eq!(got_absorb, lanehash::spec::aes_round(xk, k));
    }
}
