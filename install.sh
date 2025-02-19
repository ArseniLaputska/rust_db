#/bin/bash

cargo expand --package objc-wrapper --lib >expanded.rs
cbindgen expanded.rs -l c >rust.h
rm expanded.rs
cargo lipo --release --targets aarch64-apple-ios

path=/Desktop/dev/rust_sqlite/

mkdir -p $path/rust_sqlite/include/
# cp rust.h $path/test-rust-objc/include/rust.h

mkdir -p $path/rust_sqlite/libs/
cp target/aarch64-apple-ios/release/libobjc_wrapper.a $path/rust_sqlite/libs/libapilib.a
