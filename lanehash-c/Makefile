# C port (lanehash and lanehash_aes) in several ISA variants, each checked against the
# Rust crate. Needs ../target/release/liblanehash_ffi.a
# (RUSTFLAGS="-C target-cpu=native" cargo build --release -p lanehash-ffi).
CC ?= cc
CFLAGS ?= -O2 -std=c99 -Wall -Wextra -pedantic
FFI = ../target/release/liblanehash_ffi.a
LIBS = -lpthread -ldl -lm

all: test

SRC = lanehash.c lanehash_aes.c test_vs_rust.c
DEPS = lanehash.c lanehash.h lanehash_constants.h lanehash_aes.c lanehash_aes.h lanehash_aes_constants.h lanehash_aes_ttables.h test_vs_rust.c

# scalar + software AES; SSE2 + AES-NI; AVX2 + VAES; AVX-512 + VAES
test_portable: $(DEPS)
	$(CC) $(CFLAGS) -DLANEHASH_PORTABLE -DLANEHASH_AES_PORTABLE $(SRC) $(FFI) $(LIBS) -o $@
test_aesni: $(DEPS)
	$(CC) $(CFLAGS) -maes -msse4.1 $(SRC) $(FFI) $(LIBS) -o $@
test_vaes: $(DEPS)
	$(CC) $(CFLAGS) -maes -mavx2 -mvaes $(SRC) $(FFI) $(LIBS) -o $@
test_avx512: $(DEPS)
	$(CC) $(CFLAGS) -maes -mavx2 -mvaes -mavx512f $(SRC) $(FFI) $(LIBS) -o $@

test_noint128: $(DEPS)
	$(CC) $(CFLAGS) -DLANEHASH_PORTABLE -DLANEHASH_NO_INT128 -DLANEHASH_AES_PORTABLE -DLANEHASH_AES_NO_INT128 $(SRC) $(FFI) $(LIBS) -o $@

# SSE2 + AES-NI at -O0 under ASan and UBSan, stopping at the first report
test_sanitize: $(DEPS)
	$(CC) $(CFLAGS) -O0 -g -fsanitize=address,undefined -fno-sanitize-recover=all -maes -msse4.1 $(SRC) $(FFI) $(LIBS) -o $@

# NEON, lanehash only (lanehash_aes has no ARM backend and stays on software AES):
# AArch64 pairs registers (uzp/umlal2), LANEHASH_NEON_NARROW is the ARMv7 form
test_neon: $(DEPS)
	$(CC) $(CFLAGS) $(SRC) $(FFI) $(LIBS) -o $@
test_neon_narrow: $(DEPS)
	$(CC) $(CFLAGS) -DLANEHASH_NEON_NARROW $(SRC) $(FFI) $(LIBS) -o $@
test_neon_sanitize: $(DEPS)
	$(CC) $(CFLAGS) -O0 -g -fsanitize=address,undefined -fno-sanitize-recover=all $(SRC) $(FFI) $(LIBS) -o $@

ifneq (,$(filter arm64 aarch64,$(shell uname -m)))
TESTS = test_portable test_noint128 test_neon test_neon_narrow test_neon_sanitize
else
TESTS = test_portable test_noint128 test_aesni test_vaes test_avx512 test_sanitize
endif

test: $(TESTS)
	for t in $(TESTS); do ./$$t || exit 1; done

speed: lanehash.c lanehash_aes.c speed.c
	$(CC) $(CFLAGS) -march=native lanehash.c lanehash_aes.c speed.c $(FFI) $(LIBS) -o $@

clean:
	rm -f test_portable test_noint128 test_aesni test_vaes test_avx512 test_sanitize \
	    test_neon test_neon_narrow test_neon_sanitize speed
.PHONY: all test clean
