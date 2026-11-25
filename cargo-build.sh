#!/bin/sh
# Fallback Cargo build used when cargo-ex is not installed.
# Usage: cargo-build.sh PROJECT_DIR TARGET_DIR OUTPUT_DIR BINARY_NAME...
set -eu

project_dir="$1"; shift
target_dir="$1"; shift
output_dir="$1"; shift

cd "$project_dir"
cargo build --release --target-dir "$target_dir"

for file in "$@"; do
    cp "$target_dir/release/$file" "$output_dir/"
done
