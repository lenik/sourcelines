#!/bin/sh

    set -eu
    
    project_dir="$1"; shift
    target_dir="$1"; shift
    output="$1"; shift
    
    cd "$project_dir"
    cargo build --release --target-dir "$target_dir"
    cp "$target_dir/release/wcc" "$output"
    
