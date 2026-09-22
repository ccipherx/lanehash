/* C port vs Rust crate: cycles per hash at five sizes, L2-resident, RDPRU APERF. */
#include "lanehash.h"
#include "lanehash_aes.h"
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>
extern uint64_t lanehash64_c(const void *key, size_t len, uint64_t seed);
extern uint64_t lanehash_aes64_c(const void *key, size_t len, uint64_t seed);
static inline uint64_t aperf(void) { uint32_t lo, hi; __asm__ volatile("rdpru" : "=a"(lo), "=d"(hi) : "c"(1)); return ((uint64_t)hi << 32) | lo; }
typedef uint64_t (*hf)(const void *, size_t, uint64_t);
static double run(hf f, uint8_t **bufs, int nb, size_t len) {
    uint64_t sink = 0, best = UINT64_MAX; int t, i, r;
    for (t = 0; t < 7; t++) { uint64_t c0 = aperf(); for (r = 0; r < 200; r++) for (i = 0; i < nb; i++) sink ^= f(bufs[i], len, (uint64_t)r); uint64_t c = aperf() - c0; if (c < best) best = c; }
    if (sink == 42) printf("x");
    return (double)best / (200.0 * nb);
}
int main(void) {
    size_t sizes[] = {16, 256, 1024, 4096, 65536}; int nb = 64, i; uint8_t *bufs[64];
    for (i = 0; i < nb; i++) { bufs[i] = malloc(65536 + 64); for (size_t j = 0; j < 65536 + 64; j++) bufs[i][j] = (uint8_t)(j * 31 + i); }
    for (size_t s = 0; s < 5; s++) { int n = sizes[s] >= 65536 ? 8 : nb; double c = run(lanehash64, bufs, n, sizes[s]), r = run(lanehash64_c, bufs, n, sizes[s]);
        printf("lanehash %6zu B: C port %7.2f cyc/hash (%5.2f B/cyc) | Rust %7.2f cyc/hash (%5.2f B/cyc)\n", sizes[s], c, sizes[s] / c, r, sizes[s] / r); }
    for (size_t s = 0; s < 5; s++) { int n = sizes[s] >= 65536 ? 8 : nb; double c = run(lanehash_aes64, bufs, n, sizes[s]), r = run(lanehash_aes64_c, bufs, n, sizes[s]);
        printf("aes      %6zu B: C port %7.2f cyc/hash (%5.2f B/cyc) | Rust %7.2f cyc/hash (%5.2f B/cyc)\n", sizes[s], c, sizes[s] / c, r, sizes[s] / r); }
    return 0;
}
