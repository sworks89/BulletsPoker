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

# Pinned checksums. The ledger's is DFINITY's published SHA256SUMS for the
# release. PocketIC publishes none, so its hash is the binary this project was
# tested with (x86_64 Linux); other platforms are checked if you set
# POCKET_IC_SHA256, and warned about otherwise.
LEDGER_SHA256=51f4be010f23064137defacd627ffbec024c5133210c68ca3b80ab8f257101d6
case "$ARCH" in
  x86_64-linux) POCKET_IC_SHA256="${POCKET_IC_SHA256:-b44e1eccd66e02328146b3e209e3403e3a3428a25d25555aca2f67500e5ae9db}" ;;
esac

sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }
verify() {
  local file=$1 want=$2
  if [ -z "$want" ]; then
    echo "warning: no pinned checksum for $file on $ARCH; set POCKET_IC_SHA256 to verify it" >&2
    return
  fi
  local got
  got=$(sha256 "$file")
  if [ "$got" != "$want" ]; then
    echo "checksum mismatch for $file: expected $want, got $got" >&2
    rm -f "$file"
    exit 1
  fi
}

# The server refuses to run unless the binary is named exactly "pocket-ic".
BIN="$DIR/$POCKET_IC_VERSION/pocket-ic"
if [ ! -x "$BIN" ]; then
  mkdir -p "$DIR/$POCKET_IC_VERSION"
  curl -fsSL "https://github.com/dfinity/pocketic/releases/download/$POCKET_IC_VERSION/pocket-ic-$ARCH.gz" | gunzip > "$BIN.download"
  verify "$BIN.download" "${POCKET_IC_SHA256:-}"
  chmod +x "$BIN.download"
  mv "$BIN.download" "$BIN"
fi
verify "$BIN" "${POCKET_IC_SHA256:-}"
LEDGER="$DIR/icp-ledger-$LEDGER_RELEASE.wasm.gz"
if [ ! -f "$LEDGER" ]; then
  curl -fsSL "https://github.com/dfinity/ic/releases/download/$LEDGER_RELEASE/ledger-canister_notify-method.wasm.gz" -o "$LEDGER"
fi
verify "$LEDGER" "$LEDGER_SHA256"

cargo build --release --target wasm32-unknown-unknown -p poker_backend

export POCKET_IC_BIN="$PWD/$BIN"
export LEDGER_WASM="$PWD/$DIR/icp-ledger-$LEDGER_RELEASE.wasm.gz"
export BACKEND_WASM="$PWD/target/wasm32-unknown-unknown/release/poker_backend.wasm"
cargo test -p poker_backend --test integration -- --nocapture --test-threads 1 "$@"
