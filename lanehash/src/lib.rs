//! lanehash: fast non-cryptographic 64/128-bit hashing. The AES-lane function lives in
//! [`aes`] and is re-exported at the crate root.
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::needless_range_loop)]

#[doc(hidden)]
pub mod constants;
pub mod aes;

pub use aes::{hash128, hash128_batch, hash64, hash64_batch, FixedState, LaneHasher, Stream};
#[cfg(feature = "std")]
pub use aes::RandomState;
