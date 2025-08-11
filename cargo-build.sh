#!/bin/sh

    set -eu
    
    project_dir="$1"; shift
    target_dir="$1"; shift
    output_dir="$1"; shift
    
    cd "$project_dir"
    cargo build --release --target-dir "$target_dir"

    for file in "$@"; do
        cp "$target_dir/release/$file" "$output_dir"
    done
    
