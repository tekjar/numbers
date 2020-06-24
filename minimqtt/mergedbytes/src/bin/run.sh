#!/bin/sh

cargo run --release --bin smol | tee 1.txt
cargo run --release --bin tokio | tee 1.txt

cargo run --release --bin smol -- -f 500  | tee -a 2.txt
cargo run --release --bin tokio -- -f 500 | tee -a 2.txt
