#!/bin/bash
set -e
wasm-pack build --target web --out-dir web/pkg
echo "Build complete. Output in web/pkg/"
echo "Run a local server:  cd web && python3 -m http.server 8080"
