# lanehash-wasm

WebAssembly bindings for the `lanehash` crate, built with `wasm-bindgen` / `wasm-pack`.
`hash64` / `hash128` / `Stream` are the default function; build with
`RUSTFLAGS="-C target-feature=+simd128"` for its simd128 backend (the scalar reference
otherwise, same bits). `aes_hash64` / `aes_hash128` / `AesStream` are the AES function on
its software backend, the slower one here.

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack

cd lanehash-wasm
wasm-pack build --release --target nodejs   # or: --target web, --target bundler
```

The package lands in `pkg/` (`lanehash_wasm.js`, `lanehash_wasm_bg.wasm`, TypeScript
declarations). `pkg/` is git-ignored.

## API

Seeds and 64-bit results are `BigInt`; 128-bit results are 16 little-endian bytes.

```js
const { hash64, hash128, Stream } = require("./pkg/lanehash_wasm.js");

const data = new TextEncoder().encode("hello");
hash64(data, 0n);        // bigint
hash128(data, 0n);       // Uint8Array(16)

const s = new Stream(0n);
s.update(data.subarray(0, 2));
s.update(data.subarray(2));
s.finish64() === hash64(data, 0n);   // true
s.free();
```
