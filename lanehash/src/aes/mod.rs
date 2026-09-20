//! The AES-lane function (SMHasher `lanehash_aes64`): AES lanes over 64 bytes (AES-NI /
//! VAES / NEON at runtime, T-table software AES otherwise), a multiply-fold path below.
//! `spec` is the definition; every backend equals it. Faster than the crate root only in
//! bulk on machines with VAES.
//!
//! Public API: [`hash64`], [`hash128`], their batched forms, [`Stream`], and
//! [`FixedState`] / [`RandomState`] for `HashMap`. The hidden modules are kept public for
//! the tests and the benchmark harness.
#[doc(hidden)]
#[macro_use]
pub mod lanes;
#[doc(hidden)]
pub mod short;
#[doc(hidden)]
pub mod spec;
#[doc(hidden)]
pub mod soft;
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

/// Length regimes (bytes); the lane count depends only on the length.
pub const SHORT_MAX: usize = 64;
pub const L4_MAX: usize = 256;
pub const L8_MAX: usize = 1024;
