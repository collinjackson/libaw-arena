#!/usr/bin/env bash
# Build the example bots to bots/out/NAME.wasm (wasm32-unknown-unknown: no WASI, no imports
# beyond libaw.query). Needs: rustup target add wasm32-unknown-unknown
set -euo pipefail
cd "$(dirname "$0")/../bots"
mkdir -p out
for b in random greedy baseline; do
  (cd "$b" && cargo build --release --target wasm32-unknown-unknown --quiet)
  cp "$b/target/wasm32-unknown-unknown/release/bot_$b.wasm" "out/$b.wasm"
  echo "out/$b.wasm $(wc -c < "out/$b.wasm") bytes"
done
