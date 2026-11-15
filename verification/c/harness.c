/* CBMC harness for the C port: every length in [MINLEN, MAXLEN], every key and seed. The
 * key is a heap block of exactly `len` bytes, so a read past the input is a read past the
 * object. */
#include <stdint.h>
#include <stdlib.h>
#include "../../lanehash-c/lanehash.h"
#include "../../lanehash-c/lanehash_aes.h"

size_t nondet_size_t(void);
uint64_t nondet_u64(void);

int main(void) {
    /* a constant: CBMC does not reduce `len >= 0 && len <= 0` to one and then explores
     * the long path */
#if MINLEN == MAXLEN
    size_t len = MINLEN;
#else
    size_t len = nondet_size_t();
    __CPROVER_assume(len >= MINLEN && len <= MAXLEN);
#endif
    uint8_t *key = malloc(len ? len : 1);
    __CPROVER_assume(key != 0);
    uint64_t seed = nondet_u64();
    uint8_t out[16];
#ifndef AES_ONLY
    (void)lanehash64(key, len, seed);
    lanehash128(key, len, seed, out);
#endif
#ifdef WITH_AES
    (void)lanehash_aes64(key, len, seed);
    lanehash_aes128(key, len, seed, out);
#endif
    free(key);
    return 0;
}
