#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
./scripts/build-canvas.sh
export NBA_CANVAS_ASSET_DIR="${NBA_CANVAS_ASSET_DIR:-${CARGO_TARGET_DIR:-target}/canvas-web}"
exec cargo run -p app-server
