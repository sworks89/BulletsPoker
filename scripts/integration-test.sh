#!/usr/bin/env bash
# Runs the PocketIC integration tests: the real poker canister against the
# real ICP ledger canister on a local replica.
# Downloads the PocketIC server and ledger wasm once into .pocket-ic/.
set -euo pipefail
cd "$(dirname "$0")/.."

POCKET_IC_VERSION=16.1.0
LEDGER_RELEASE=ledger-suite-icp-2025-08-29
DIR=.pocket-ic
mkdir -p "$DIR"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) ARCH=x86_64-linux ;;
  Linux-aarch64) ARCH=arm64-linux ;;
  Darwin-x86_64) ARCH=x86_64-darwin ;;
  Darwin-arm64) ARCH=arm64-darwin ;;
  *) echo "unsupported platform" >&2; exit 1 ;;
esac

# The server refuses to run unless the binary is named exactly "pocket-ic".
BIN="$DIR/$POCKET_IC_VERSION/pocket-ic"
if [ ! -x "$BIN" ]; then
  mkdir -p "$DIR/$POCKET_IC_VERSION"
  curl -fsSL "https://github.com/dfinity/pocketic/releases/download/$POCKET_IC_VERSION/pocket-ic-$ARCH.gz" | gunzip > "$BIN"
  chmod +x "$BIN"
fi
if [ ! -f "$DIR/icp-ledger-$LEDGER_RELEASE.wasm.gz" ]; then
  curl -fsSL "https://github.com/dfinity/ic/releases/download/$LEDGER_RELEASE/ledger-canister_notify-method.wasm.gz" -o "$DIR/icp-ledger-$LEDGER_RELEASE.wasm.gz"
fi

cargo build --release --target wasm32-unknown-unknown -p poker_backend

export POCKET_IC_BIN="$PWD/$BIN"
export LEDGER_WASM="$PWD/$DIR/icp-ledger-$LEDGER_RELEASE.wasm.gz"
export BACKEND_WASM="$PWD/target/wasm32-unknown-unknown/release/poker_backend.wasm"
cargo test -p poker_backend --test integration -- --nocapture --test-threads 1 "$@"
