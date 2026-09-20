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
