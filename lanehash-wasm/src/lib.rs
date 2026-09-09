//! wasm-bindgen bindings for JavaScript (built with `wasm-pack`). On wasm32 the core
//! crate runs its portable `spec` backend: WebAssembly has no AES instruction.
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
