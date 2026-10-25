#!/usr/bin/env bash
# Prints the GitHub Release body for a reproducible build.
#
#   scripts/release-notes.sh <tag> <out-dir>
#
# <out-dir> is the --out directory of scripts/reproducible-build.sh. The body
# carries the machine-readable line the app's build badge parses, exactly
#   poker_backend.wasm.gz sha256: <64 lowercase hex>
# on a line of its own. The commit is HEAD of the checkout (override with
# COMMIT=<sha> for testing).
set -euo pipefail
cd "$(dirname "$0")/.."

TAG=${1:?usage: release-notes.sh <tag> <out-dir>}
DIR=${2:?usage: release-notes.sh <tag> <out-dir>}

[[ "$TAG" =~ ^v[A-Za-z0-9._+-]+$ ]] || { echo "bad tag: $TAG" >&2; exit 1; }
SUMFILE=$DIR/poker_backend.wasm.gz.sha256
[ -f "$SUMFILE" ] || { echo "missing $SUMFILE" >&2; exit 1; }
HASH=$(cut -d' ' -f1 "$SUMFILE")
[[ "$HASH" =~ ^[0-9a-f]{64}$ ]] || { echo "bad hash in $SUMFILE" >&2; exit 1; }
# The hash must be that of the file being released.
[ "$(sha256sum "$DIR/poker_backend.wasm.gz" | cut -d' ' -f1)" = "$HASH" ] \
  || { echo "$SUMFILE does not match poker_backend.wasm.gz" >&2; exit 1; }

COMMIT=${COMMIT:-$(git rev-parse HEAD)}
[[ "$COMMIT" =~ ^[0-9a-f]{40}$ ]] || { echo "bad commit: $COMMIT" >&2; exit 1; }
BASE=$(sed -n 's/^ARG BASE=\(rust@sha256:[0-9a-f]\{64\}\)$/\1/p' build/reproducible/Dockerfile)
[ -n "$BASE" ] || { echo "no base image pin in Dockerfile" >&2; exit 1; }
ICWASM=$(sed -n 's/^# ic-wasm \([0-9][0-9.]*\), statically.*/\1/p' build/reproducible/Dockerfile)
RUST=$(sed -n 's/^ENV RUSTUP_TOOLCHAIN=\(.*\)$/\1/p' build/reproducible/Dockerfile)
[ -n "$ICWASM" ] && [ -n "$RUST" ] || { echo "toolchain pins not found in Dockerfile" >&2; exit 1; }

cat <<BODY
Game canister (\`poker_backend\`) built reproducibly from commit \`$COMMIT\`.

poker_backend.wasm.gz sha256: $HASH

This is the SHA-256 of \`poker_backend.wasm.gz\`, the exact bytes installed on
the canister, so it is also the \`module_hash\` the Internet Computer reports.

## Toolchain pins

- Base image: \`$BASE\` (rust, linux/amd64)
- Rust: $RUST
- ic-wasm: $ICWASM
- Dependencies: \`Cargo.lock\`, built with \`--locked\`

## Reproduce

\`\`\`bash
git clone https://github.com/sworks89/BulletsPoker.git
cd BulletsPoker
git checkout $TAG && scripts/reproducible-build.sh
cat out/poker_backend.wasm.gz.sha256
\`\`\`

The printed hash must equal the one above. See docs/reproducible-builds.md.
BODY
