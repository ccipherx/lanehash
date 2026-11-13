//! Shared by the fuzz targets: every backend this CPU can run (as in tests/vectors.rs).
use lanehash::aes::dispatch::Backend;

pub fn backends() -> Vec<&'static Backend> {
    let mut v = vec![&lanehash::aes::dispatch::SPEC];
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("aes") {
            v.push(&lanehash::aes::dispatch::AESNI);
        }
        if std::is_x86_feature_detected!("vaes") && std::is_x86_feature_detected!("avx2") {
            v.push(&lanehash::aes::dispatch::VAES256);
        }
        if std::is_x86_feature_detected!("vaes") && std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx512vl") {
            v.push(&lanehash::aes::dispatch::VAESVL);
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("aes") {
            v.push(&lanehash::aes::dispatch::NEON);
        }
    }
    v
}

/// Every backend of the default function this CPU can run (as in tests/api.rs).
pub fn default_backends() -> Vec<&'static lanehash::dispatch::Backend> {
    let mut v = vec![&lanehash::dispatch::SPEC];
    #[cfg(target_arch = "x86_64")]
    {
        v.push(&lanehash::dispatch::SSE2);
        if lanehash::dispatch::has_avx2() {
            v.push(&lanehash::dispatch::AVX2);
        }
        if lanehash::dispatch::has_avx512() {
            v.push(&lanehash::dispatch::AVX512);
        }
    }
    #[cfg(target_arch = "aarch64")]
    v.push(&lanehash::dispatch::NEON);
    v
}
