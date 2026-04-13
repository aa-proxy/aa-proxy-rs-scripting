#!/usr/bin/env bash
set -euo pipefail

cargo component build --release

echo

echo "Built component: target/wasm32-wasip1/release/aa_proxy_test_hook.wasm"
echo "Copy it to /data/wasm-hooks/10_test_hook.wasm"
