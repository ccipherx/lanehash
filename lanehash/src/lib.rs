//! lanehash: a fast non-cryptographic 64/128-bit hash. Inputs over 64 bytes go
//! through AES lanes (AES-NI / VAES / NEON at runtime, portable fallback), shorter
//! inputs through a 64x64->128 multiply-fold path. The definition of the function
//! is the portable `spec` module; every SIMD backend reproduces it bit-for-bit.
//!
//! Public API: [`hash64`], [`hash128`], their batched forms, [`Stream`] for
//! incremental hashing, and [`FixedState`] / [`RandomState`] for `HashMap`.
//! The remaining modules are implementation detail (hidden from the docs, kept
//! public for the reference tests and the benchmark harness).
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::needless_range_loop)]

#[doc(hidden)]
pub mod constants;
#[doc(hidden)]
#[macro_use]
pub mod lanes;
#[doc(hidden)]
pub mod short;
#[doc(hidden)]
pub mod spec;
#[cfg(all(target_arch = "x86_64", not(feature = "force-fallback")))]
#[doc(hidden)]
pub mod x86;
#[cfg(all(target_arch = "aarch64", not(feature = "force-fallback")))]
#[doc(hidden)]
pub mod arm;
#[doc(hidden)]
pub mod dispatch;
#[doc(hidden)]
pub mod stream;
#[doc(hidden)]
pub mod hasher;

pub use dispatch::{hash128, hash128_batch, hash64, hash64_batch};
pub use hasher::{FixedState, LaneHasher};
#[cfg(feature = "std")]
pub use hasher::RandomState;
pub use stream::Stream;

/// Length regimes (bytes). The lane count depends only on the length.
pub const SHORT_MAX: usize = 64;
pub const L4_MAX: usize = 256;
pub const L8_MAX: usize = 1024;
