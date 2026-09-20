//! Incremental lanehash, equal to the one-shot function at every split.
use super::dispatch::{backend, Backend};
use super::spec::{self, Words, BLOCK, CHAINS, STRIPE};

/// Carry, one block not yet known to be leading, and the 64 bytes that make it so.
const BUF: usize = 64 + BLOCK + 64;

pub struct Stream {
    seed: u64,
    ks: u64,
    total: usize,
    /// Whole leading blocks absorbed into `st`.
    blocks: usize,
    st: Words,
    /// Input from byte `512 * blocks - carry` on: the last 64 bytes of the last absorbed
    /// block (the closing stripe may reach into it), then unabsorbed input.
    buf: [u8; BUF],
    buf_len: usize,
    backend: &'static Backend,
}

impl Stream {
    pub fn new(seed: u64) -> Self {
        let ks = spec::ks(seed);
        Stream {
            seed,
            ks,
            total: 0,
            blocks: 0,
            st: Words::from_state(&spec::init(ks)),
            buf: [0u8; BUF],
            buf_len: 0,
            backend: backend(),
        }
    }

    #[inline(always)]
    fn carry(&self) -> usize {
        if self.blocks > 0 { 64 } else { 0 }
    }

    /// A block is absorbed only once 64 more bytes follow it, so the closing stripe
    /// (the last 64 bytes of the input) is never inside an absorbed block.
    pub fn update(&mut self, mut data: &[u8]) {
        self.total += data.len();

        while !data.is_empty() {
            let carry = self.carry();

            if self.buf_len == carry && data.len() >= BLOCK + 64 {
                let k = (data.len() - 64) / BLOCK;
                // SAFETY: `data` holds `k` whole blocks plus 64 bytes.
                unsafe { (self.backend.absorb)(&mut self.st, data.as_ptr(), k) };
                self.blocks += k;

                let keep = &data[k * BLOCK - 64..];
                self.buf[..keep.len()].copy_from_slice(keep);
                self.buf_len = keep.len();
                return;
            }

            let take = data.len().min(carry + BLOCK + 64 - self.buf_len);
            self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
            self.buf_len += take;
            data = &data[take..];

            if self.buf_len == carry + BLOCK + 64 {
                // SAFETY: one whole block at `buf[carry..]`, 64 bytes behind it.
                unsafe {
                    (self.backend.absorb)(
                        &mut self.st,
                        self.buf.as_ptr().add(carry),
                        1,
                    )
                };
                self.blocks += 1;

                self.buf.copy_within(carry + BLOCK - 64..self.buf_len, 0);
                self.buf_len -= carry + BLOCK - 64;
            }
        }
    }

    pub fn finish128(&self) -> u128 {
        let n = self.total;

        if n <= spec::SHORT_MAX {
            return super::short::short128(&self.buf[..self.buf_len], self.seed);
        }

        if self.blocks == 0 {
            // SAFETY: the whole input (65..=639 bytes) is buffered.
            return unsafe { (self.backend.hash128)(self.buf.as_ptr(), n, self.ks) };
        }

        let m = (n - 1) / STRIPE;
        let r = m - CHAINS * self.blocks;
        debug_assert!(r <= CHAINS && self.buf_len >= 64 + r * STRIPE);

        // SAFETY: `buf[64..]` contains the remaining leading stripes and the final
        // 64 bytes of the input are at `buf[buf_len - 64..]`.
        unsafe {
            (self.backend.finish)(
                &self.st,
                self.buf.as_ptr().add(64),
                r,
                self.buf.as_ptr().add(self.buf_len - 64),
                n,
                self.ks,
            )
        }
    }

    /// The low half of [`finish128`](Self::finish128).
    pub fn finish64(&self) -> u64 {
        if self.total <= spec::SHORT_MAX {
            return super::short::short64(&self.buf[..self.buf_len], self.seed);
        }
        self.finish128() as u64
    }
}