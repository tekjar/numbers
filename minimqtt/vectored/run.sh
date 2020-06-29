#!/bin/sh

# cargo run --release --bin smol | tee throughput.txt
cargo run --release --bin tokio -- -n 1000000  | tee throughput.txt


# cargo run --release --bin smol -- -f 500  | tee -a throughput.txt
cargo run --release --bin tokio -- -f 500 -n 1000000 | tee -a throughput.txt


# cargo run --release --bin smol -- -f 1000  | tee -a throughput.txt
# cargo run --release --bin tokio -- -f 1000 | tee -a throughput.txt
