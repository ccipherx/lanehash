/* C port == Rust crate (via lanehash-ffi): lengths 0..=2100 and larger, six seeds,
 * random alignments, both widths, both functions. */
#include "lanehash.h"
#include "lanehash_aes.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern uint64_t lanehash64_c(const void *key, size_t len, uint64_t seed);
extern void lanehash128_c(const void *key, size_t len, uint64_t seed, void *out);
extern uint64_t lanehash_aes64_c(const void *key, size_t len, uint64_t seed);
extern void lanehash_aes128_c(const void *key, size_t len, uint64_t seed, void *out);

static uint64_t rng_s = UINT64_C(0x1234567890abcdef);
static uint64_t rng(void) { rng_s ^= rng_s << 13; rng_s ^= rng_s >> 7; rng_s ^= rng_s << 17; return rng_s; }

int main(void) {
    const size_t big = (1u << 20) + 64;
    uint8_t *buf = (uint8_t *)malloc(big);
    const uint64_t seeds[6] = {0, 1, UINT64_MAX, UINT64_C(0x8000000000000000), UINT64_C(0x243f6a8885a308d3), UINT64_C(0xdeadbeefcafef00d)};
    size_t lens[2200], nlens = 0, i, s;
    unsigned long checked = 0;
    for (i = 0; i < big; i++) buf[i] = (uint8_t)rng();
    for (i = 0; i <= 2100; i++) lens[nlens++] = i;
    { size_t extra[] = {4095, 4096, 4097, 8192, 65535, 65536, 65537, 524287, 524288, 524289, 1u << 20, (1u << 20) + 7};
      for (i = 0; i < sizeof extra / sizeof *extra; i++) lens[nlens++] = extra[i]; }
    for (i = 0; i < nlens; i++) {
        for (s = 0; s < 6; s++) {
            size_t off = (size_t)(rng() % 16), len = lens[i];
            const uint8_t *p = buf + off;
            uint8_t c128[16], r128[16];
            uint64_t c64 = lanehash64(p, len, seeds[s]), r64 = lanehash64_c(p, len, seeds[s]);
            lanehash128(p, len, seeds[s], c128);
            lanehash128_c(p, len, seeds[s], r128);
            if (c64 != r64 || memcmp(c128, r128, 16) != 0 || memcmp(c128, &c64, 8) != 0) {
                printf("MISMATCH len=%zu seed=%zu off=%zu: c64=%016llx rust64=%016llx\n", len, s, off, (unsigned long long)c64, (unsigned long long)r64);
                return 1;
            }
            c64 = lanehash_aes64(p, len, seeds[s]);
            r64 = lanehash_aes64_c(p, len, seeds[s]);
            lanehash_aes128(p, len, seeds[s], c128);
            lanehash_aes128_c(p, len, seeds[s], r128);
            if (c64 != r64 || memcmp(c128, r128, 16) != 0 || memcmp(c128, &c64, 8) != 0) {
                printf("aes MISMATCH len=%zu seed=%zu off=%zu: c64=%016llx rust64=%016llx\n", len, s, off, (unsigned long long)c64, (unsigned long long)r64);
                return 1;
            }
            checked++;
        }
    }
    printf("ok: %lu (length, seed) pairs identical to the Rust crate (lanehash and lanehash_aes)\n", checked);
    free(buf);
    return 0;
}
