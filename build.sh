#!/bin/bash
set -e

cargo install wasm-bindgen-cli

wasm-pack build --target web --out-dir web/pkg --mode no-install
echo "Build complete. Output in web/pkg/"
echo "Run a local server:  cd web && python3 -m http.server 8080"
