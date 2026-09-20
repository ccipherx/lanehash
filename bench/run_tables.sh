#!/bin/sh
# Same-machine tables. Pinned to core 3; quality harnesses run on 6,7,8,10,11.
cd "$(dirname "$0")/.."
B=./target/release/bench; N=./target-nightly/release/bench
H=lanehash,aes,aes-vaes256,aes-aesni,aes-spec,gxhash,rapidhash-v3,xxh3,foldhash,ahash
taskset -c 3 $B throughput --rounds 5 --align 16 --hashes $H > docs/bench/throughput-L2-align16.csv 2>&1
taskset -c 3 $N throughput --rounds 5 --align 16 --hashes gxhash-hybrid > docs/bench/throughput-L2-align16-hybrid.csv 2>&1
taskset -c 3 $B throughput --rounds 5 --align 1 --hashes lanehash,aes,gxhash,rapidhash-v3,xxh3 --sizes 256,1024,4096,65536 > docs/bench/throughput-L2-align1.csv 2>&1
# L1-resident: working set = 16 KiB
for cfg in "256 64" "1024 16" "4096 4" "16384 1"; do set -- $cfg
  taskset -c 3 $B throughput --rounds 5 --align 16 --nbuf $2 --hashes lanehash,aes,aes-vaes256,aes-aesni,gxhash,rapidhash-v3,xxh3 --sizes $1 >> docs/bench/throughput-L1-align16.csv 2>&1
  taskset -c 3 $N throughput --rounds 5 --align 16 --nbuf $2 --hashes gxhash-hybrid --sizes $1 >> docs/bench/throughput-L1-align16.csv 2>&1
done
taskset -c 3 $B latency --rounds 5 --align 16 --hashes $H --sizes 4,8,16,32,64,128,256,1024 > docs/bench/latency-align16.csv 2>&1
taskset -c 3 $N latency --rounds 5 --align 16 --hashes gxhash-hybrid --sizes 4,8,16,32,64,128,256,1024 > docs/bench/latency-align16-hybrid.csv 2>&1
taskset -c 3 $B throughput --rounds 3 --align 16 --dram --hashes lanehash,aes,gxhash,rapidhash-v3,xxh3 --sizes 1048576 > docs/bench/throughput-dram.csv 2>&1
taskset -c 3 $N throughput --rounds 3 --align 16 --dram --hashes gxhash-hybrid --sizes 1048576 > docs/bench/throughput-dram-hybrid.csv 2>&1
echo TABLES_DONE > docs/bench/DONE
