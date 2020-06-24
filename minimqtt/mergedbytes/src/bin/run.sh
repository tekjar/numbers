#!/bin/sh

cargo run --release --bin smol | tee smol.txt
cargo run --release --bin tokio | tee tokio.txt


cargo run --release --bin smol -f 500  | tee smol.txt
cargo run --release --bin tokio -f 500 | tee tokio.txt
