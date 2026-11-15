//! Incremental lanehash, equal to the one-shot function at every split.
use super::dispatch::{backend, Backend};
use super::spec::{self, Words, BLOCK, CHAINS, STRIPE};

/// Carry, one block not yet known to be leading, and the 64 bytes that make it so.
const BUF: usize = 64 + BLOCK + 64;

/// `total + len` for both `Stream` types, panicking instead of wrapping.
#[inline(always)]
pub(crate) fn add_len(total: usize, len: usize) -> usize {
    match total.checked_add(len) {
        Some(t) => t,
        None => too_long(),
    }
}
#[cold]
#[inline(never)]
fn too_long() -> ! {
    panic!("lanehash: stream input longer than usize::MAX bytes")
}

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
    /// (the last 64 bytes of the input) is never inside an absorbed block. Bytes already
    /// buffered complete their block through the buffer (two blocks at most per call),
    /// everything after that is absorbed straight from `data`: with a buffer that kept
    /// 64 bytes of look-ahead, a stream fed in 4 KiB pieces went through the buffer block
    /// by block, at a third of the one-shot rate.
    ///
    /// # Panics
    ///
    /// If the total input exceeds `usize::MAX` bytes, reachable only on 32-bit targets.
    pub fn update(&mut self, mut data: &[u8]) {
        self.total = crate::stream::add_len(self.total, data.len());

        loop {
            let carry = self.carry();
            let pending = self.buf_len - carry;

            if pending == 0 && data.len() >= BLOCK + 64 {
                let k = (data.len() - 64) / BLOCK;
                // SAFETY: `data` holds `k` whole blocks plus 64 bytes.
                unsafe { (self.backend.absorb)(&mut self.st, data.as_ptr(), k) };
                self.blocks += k;

                let keep = &data[k * BLOCK - 64..];
                self.buf[..keep.len()].copy_from_slice(keep);
                self.buf_len = keep.len();
                return;
            }

            if pending + data.len() < BLOCK + 64 {
                // no block has its 64 bytes of look-ahead yet
                self.buf[self.buf_len..self.buf_len + data.len()].copy_from_slice(data);
                self.buf_len += data.len();
                return;
            }

            // one block through the buffer: take only the bytes that complete it
            if pending < BLOCK {
                let take = BLOCK - pending;
                self.buf[self.buf_len..self.buf_len + take].copy_from_slice(&data[..take]);
                self.buf_len += take;
                data = &data[take..];
            }
            // SAFETY: one whole block at `buf[carry..]`.
            unsafe { (self.backend.absorb)(&mut self.st, self.buf.as_ptr().add(carry), 1) };
            self.blocks += 1;

            // the block's last 64 bytes become the carry, what followed it stays pending
            self.buf.copy_within(carry + BLOCK - 64..self.buf_len, 0);
            self.buf_len -= carry + BLOCK - 64;
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

#[cfg(all(test, target_pointer_width = "32"))]
mod tests {
    use super::*;

    /// A stream that has seen `total` bytes, skipping the middle by advancing the
    /// counters in whole blocks.
    fn at(total: usize) -> Stream {
        let mut st = Stream::new(7);
        st.update(&[0u8; 2000]);
        let skip = (total - 4000 - st.total) / BLOCK * BLOCK;
        st.total += skip;
        st.blocks += skip / BLOCK;
        let rest = vec![0u8; total - st.total];
        st.update(&rest);
        st
    }

    #[test]
    fn length_near_usize_max() {
        for below in 0..20 {
            let _ = at(usize::MAX - below).finish128();
        }
    }

    #[test]
    #[should_panic(expected = "longer than usize::MAX")]
    fn length_past_usize_max() {
        let mut st = at(usize::MAX - 100);
        st.update(&[0u8; 300]);
    }
}
