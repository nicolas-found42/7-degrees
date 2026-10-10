#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
canvas_target="${CARGO_TARGET_DIR:-target}"
cargo build -p canvas-view --target wasm32-unknown-unknown --release
wasm-bindgen "$canvas_target/wasm32-unknown-unknown/release/canvas_view.wasm" --target web --out-dir "$canvas_target/canvas-web" --out-name canvas_view
