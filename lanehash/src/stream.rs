//! Streaming API equal to the one-shot function for every length.
use crate::dispatch::{backend, Backend};
use crate::lanes::State;
use crate::spec::Block;

const L: usize = 16;
const STEP: usize = 16 * L; // 256
/// Buffer: enough to hold everything while total <= L8_MAX, plus one step of slack.
const BUF: usize = crate::L8_MAX + 16 + STEP;

pub unsafe fn absorb16_spec(state: &mut [[u8; 16]; 16], p: *const u8, nblocks: usize) {
    let mut st = State::<Block, 16, 16>::from_blocks(state);
    st.absorb(p, nblocks);
    st.store(state);
}
pub unsafe fn finish16_spec(state: &[[u8; 16]; 16], c: *const u8, len: usize) -> u128 {
    State::<Block, 16, 16>::from_blocks(state).finish(c, len)
}

pub struct Stream {
    seed: u64,
    total: usize,
    /// Regular blocks already absorbed (always a multiple of L).
    absorbed_blocks: usize,
    state: [[u8; 16]; 16],
    buf: [u8; BUF],
    buf_len: usize,
    backend: &'static Backend,
}

impl Stream {
    pub fn new(seed: u64) -> Self {
        let mut state = [[0u8; 16]; 16];
        for i in 0..L {
            state[i] = <Block as crate::lanes::Lanes>::init(seed, i);
        }
        Stream { seed, total: 0, absorbed_blocks: 0, state, buf: [0u8; BUF], buf_len: 0, backend: backend() }
    }

    /// Invariant: `absorbed_blocks * 16 + buf_len == total`; everything absorbed is a
    /// whole number of steps, and `buf` holds the rest. Once `total > L8_MAX`, whole
    /// steps are absorbed straight from `data` (only the tail is copied); while
    /// `total <= L8_MAX` everything is buffered for the one-shot path in `finish`.
    pub fn update(&mut self, mut data: &[u8]) {
        self.total += data.len();
        if self.total <= crate::L8_MAX {
            self.buf[self.buf_len..self.buf_len + data.len()].copy_from_slice(data);
            self.buf_len += data.len();
            return;
        }
        // Blocks before ceil(total/16) - L are regular for every final length >= total,
        // and the last STEP bytes must stay buffered (the closing region overlaps the
        // last regular block when the length is not a multiple of 16).
        let pending = self.total - self.absorbed_blocks * 16;
        let limit = ((self.total + 15) / 16 - L) * 16;
        let steps = ((limit - self.absorbed_blocks * 16) / STEP).min(pending.saturating_sub(STEP) / STEP);
        let mut left = steps;
        let mut consumed = 0;
        // 1. whole steps already in the buffer (its start is step-aligned)
        let from_buf = left.min(self.buf_len / STEP);
        if from_buf > 0 {
            unsafe { (self.backend.absorb16)(&mut self.state, self.buf.as_ptr(), from_buf * L) };
            consumed = from_buf * STEP;
            left -= from_buf;
        }
        // 2. a partial step in the buffer: top it up from `data` and absorb it
        let part = self.buf_len - consumed;
        if left > 0 && part > 0 {
            let need = STEP - part;
            self.buf[self.buf_len..self.buf_len + need].copy_from_slice(&data[..need]);
            self.buf_len += need;
            data = &data[need..];
            unsafe { (self.backend.absorb16)(&mut self.state, self.buf.as_ptr().add(consumed), L) };
            consumed += STEP;
            left -= 1;
        }
        // 3. the rest straight from the caller's slice
        if left > 0 {
            unsafe { (self.backend.absorb16)(&mut self.state, data.as_ptr(), left * L) };
            data = &data[left * STEP..];
        }
        self.absorbed_blocks += steps * L;
        // 4. keep the unabsorbed tail (< 2 * STEP + 16 bytes)
        self.buf.copy_within(consumed..self.buf_len, 0);
        self.buf_len -= consumed;
        self.buf[self.buf_len..self.buf_len + data.len()].copy_from_slice(data);
        self.buf_len += data.len();
    }

    pub fn finish128(&self) -> u128 {
        if self.total <= crate::L8_MAX {
            return crate::hash128(&self.buf[..self.buf_len], self.seed);
        }
        let n = (self.total + 15) / 16;
        let m = n - L;
        let remaining = m - self.absorbed_blocks;
        let mut state = self.state;
        if remaining > 0 {
            unsafe { (self.backend.absorb16)(&mut state, self.buf.as_ptr(), remaining) };
        }
        // Closing region: last STEP bytes of the input == last STEP bytes of the buffer.
        let c = unsafe { self.buf.as_ptr().add(self.buf_len - STEP) };
        unsafe { (self.backend.finish16)(&state, c, self.total) }
    }

    pub fn finish64(&self) -> u64 {
        self.finish128() as u64
    }
}
