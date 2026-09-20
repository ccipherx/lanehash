//! lanehash: fast non-cryptographic 64/128-bit hashing, two functions with one API.
//!
//! The crate root is **lanehash** itself, the default choice: chained NH-32 stripes
//! (`spec`, 65 bytes and up) and a multiply-fold short path (`short`, 0–64 bytes), no
//! instruction-set requirement, one definition reproduced bit-for-bit by the scalar
//! reference and the SSE2, AVX2, AVX-512, NEON and wasm simd128 backends. The original
//! AES-lane function lives in [`aes`] with the same shape of API; it is faster only in
//! bulk on machines with VAES.
//!
//! Public API: [`hash64`], [`hash128`], [`Stream`] for incremental hashing, and
//! [`FixedState`] / [`RandomState`] for `HashMap`. The hidden modules are implementation
//! detail, kept public for the reference tests and the benchmark harness.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::needless_range_loop)]

#[doc(hidden)]
pub mod constants;
#[doc(hidden)]
pub mod dispatch;
#[doc(hidden)]
pub mod hasher;
#[doc(hidden)]
pub mod short;
#[doc(hidden)]
pub mod spec;
#[doc(hidden)]
pub mod stream;
#[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
#[doc(hidden)]
pub mod x86;
#[cfg(all(target_arch = "aarch64", not(feature = "force-fallback")))]
#[doc(hidden)]
pub mod arm;
#[cfg(all(target_arch = "wasm32", target_feature = "simd128", not(feature = "force-fallback")))]
#[doc(hidden)]
pub mod wasm;
pub mod aes;

pub use dispatch::{hash128, hash64};
pub use hasher::{FixedState, LaneHasher};
#[cfg(feature = "std")]
pub use hasher::RandomState;
pub use stream::Stream;
