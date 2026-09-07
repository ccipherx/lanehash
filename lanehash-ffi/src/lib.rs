//! C ABI staticlib for the SMHasher / SMHasher3 shims (kept out of the no_std core crate).
//! C ABI for the SMHasher / SMHasher3 shims (built via `crate-type = staticlib`).
#[no_mangle]
pub unsafe extern "C" fn lanehash64_c(key: *const u8, len: usize, seed: u64) -> u64 {
    lanehash::hash64(core::slice::from_raw_parts(key, len), seed)
}
#[no_mangle]
pub unsafe extern "C" fn lanehash128_c(key: *const u8, len: usize, seed: u64, out: *mut u8) {
    let h = lanehash::hash128(core::slice::from_raw_parts(key, len), seed);
    core::ptr::copy_nonoverlapping(h.to_le_bytes().as_ptr(), out, 16);
}
