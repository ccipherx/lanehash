//! wasm-bindgen bindings (built with `wasm-pack`). `hash64` / `hash128` / `Stream` are the
//! default function: its simd128 backend in a `-C target-feature=+simd128` build, the
//! scalar reference otherwise, same bits. `aes_hash64` / `aes_hash128` / `AesStream` are
//! the AES function on its software backend (WebAssembly has no AES instruction).
use wasm_bindgen::prelude::*;

/// 64-bit hash of `bytes` under `seed` (a `BigInt` in JS).
#[wasm_bindgen]
pub fn hash64(bytes: &[u8], seed: u64) -> u64 {
    lanehash::hash64(bytes, seed)
}

/// 128-bit hash of `bytes` under `seed`, as 16 little-endian bytes.
#[wasm_bindgen]
pub fn hash128(bytes: &[u8], seed: u64) -> Vec<u8> {
    lanehash::hash128(bytes, seed).to_le_bytes().to_vec()
}

/// Incremental hashing: `update` any number of times, then `finish64` / `finish128`.
#[wasm_bindgen]
pub struct Stream(lanehash::Stream);

#[wasm_bindgen]
impl Stream {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64) -> Stream {
        Stream(lanehash::Stream::new(seed))
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    pub fn finish64(&self) -> u64 {
        self.0.finish64()
    }

    pub fn finish128(&self) -> Vec<u8> {
        self.0.finish128().to_le_bytes().to_vec()
    }
}

/// 64-bit AES-lane hash (`lanehash::aes`) of `bytes` under `seed`.
#[wasm_bindgen]
pub fn aes_hash64(bytes: &[u8], seed: u64) -> u64 {
    lanehash::aes::hash64(bytes, seed)
}

/// 128-bit AES-lane hash of `bytes` under `seed`, as 16 little-endian bytes.
#[wasm_bindgen]
pub fn aes_hash128(bytes: &[u8], seed: u64) -> Vec<u8> {
    lanehash::aes::hash128(bytes, seed).to_le_bytes().to_vec()
}

/// Incremental AES-lane hashing.
#[wasm_bindgen]
pub struct AesStream(lanehash::aes::Stream);

#[wasm_bindgen]
impl AesStream {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64) -> AesStream {
        AesStream(lanehash::aes::Stream::new(seed))
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    pub fn finish64(&self) -> u64 {
        self.0.finish64()
    }

    pub fn finish128(&self) -> Vec<u8> {
        self.0.finish128().to_le_bytes().to_vec()
    }
}
