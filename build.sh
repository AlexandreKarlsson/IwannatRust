#!/usr/bin/env bash
# Build the web UI (Dioxus → WASM) and then the CLI with the UI embedded.
set -euo pipefail
cd "$(dirname "$0")"
if ! command -v dx >/dev/null 2>&1; then
  if [ -x "$HOME/.cargo/bin/dx" ]; then export PATH="$HOME/.cargo/bin:$PATH"; else
    echo "dioxus CLI not found. Install with: cargo install dioxus-cli --version 0.7.10 --locked" >&2; exit 1; fi
fi
rm -rf target/dx/iwr-ui/release/web/public
( cd crates/iwr-ui && dx build --release )
cargo build --release -p iwr
echo "built: target/release/iwr"
