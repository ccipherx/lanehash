/* Bodies for the two GCC SSE2 builtins CBMC lacks, from Intel's definitions of PMULUDQ
 * and PSRLQ. Trusted. */
typedef int lh_v4si __attribute__((__vector_size__(16)));
typedef long long lh_v2di __attribute__((__vector_size__(16)));

lh_v2di __builtin_ia32_pmuludq128(lh_v4si a, lh_v4si b) {
    lh_v2di r;
    r[0] = (long long)((unsigned long long)(unsigned)a[0] * (unsigned long long)(unsigned)b[0]);
    r[1] = (long long)((unsigned long long)(unsigned)a[2] * (unsigned long long)(unsigned)b[2]);
    return r;
}

lh_v2di __builtin_ia32_psrlqi128(lh_v2di a, int n) {
    lh_v2di r;
    r[0] = n > 63 ? 0 : (long long)((unsigned long long)a[0] >> n);
    r[1] = n > 63 ? 0 : (long long)((unsigned long long)a[1] >> n);
    return r;
}
