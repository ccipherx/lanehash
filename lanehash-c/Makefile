# Builds the C port three ways (portable, AES-NI, VAES+AVX2) and checks each against
# the Rust crate. Needs ../target/release/liblanehash_ffi.a
# (RUSTFLAGS="-C target-cpu=native" cargo build --release -p lanehash-ffi).
CC ?= cc
CFLAGS ?= -O2 -std=c99 -Wall -Wextra -pedantic
FFI = ../target/release/liblanehash_ffi.a
LIBS = -lpthread -ldl -lm

all: test

test_portable: lanehash.c lanehash.h lanehash_constants.h test_vs_rust.c
	$(CC) $(CFLAGS) -DLANEHASH_PORTABLE lanehash.c test_vs_rust.c $(FFI) $(LIBS) -o $@
test_aesni: lanehash.c lanehash.h lanehash_constants.h test_vs_rust.c
	$(CC) $(CFLAGS) -maes -msse4.1 lanehash.c test_vs_rust.c $(FFI) $(LIBS) -o $@
test_vaes: lanehash.c lanehash.h lanehash_constants.h test_vs_rust.c
	$(CC) $(CFLAGS) -maes -mavx2 -mvaes lanehash.c test_vs_rust.c $(FFI) $(LIBS) -o $@

test_noint128: lanehash.c lanehash.h lanehash_constants.h test_vs_rust.c
	$(CC) $(CFLAGS) -DLANEHASH_PORTABLE -DLANEHASH_NO_INT128 lanehash.c test_vs_rust.c $(FFI) $(LIBS) -o $@

test: test_portable test_noint128 test_aesni test_vaes
	./test_portable && ./test_noint128 && ./test_aesni && ./test_vaes

clean:
	rm -f test_portable test_noint128 test_aesni test_vaes
.PHONY: all test clean
