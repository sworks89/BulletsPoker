#!/usr/bin/env bash
# Builds the game canister (poker_backend) reproducibly, in the pinned Docker
# image from build/reproducible/Dockerfile.
#
#   scripts/reproducible-build.sh [--out DIR]
#
# Writes DIR/poker_backend.wasm.gz (the module that gets installed),
# DIR/poker_backend.wasm.gz.sha256 and DIR/poker_backend.did, and prints the
# hash. DIR defaults to out/ in the repo; a relative DIR is taken from the
# current directory.
#
# The source is the committed HEAD (git archive), not the working tree, so
# uncommitted edits and checkout line endings can't change the result. The
# archive is unpacked and built inside the container; nothing in the repo is
# written to. See docs/reproducible-builds.md.
#
# --work-dir PATH (container path, default /build) is where the copy is built.
# It doesn't change the output; scripts/check-reproducible.sh varies it to
# prove that.
set -euo pipefail
CALLER_DIR=$PWD
cd "$(dirname "$0")/.."

OUT=out
WORK=/build
while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT=${2:?--out needs a directory}; case "$OUT" in /*) ;; *) OUT=$CALLER_DIR/$OUT ;; esac; shift 2 ;;
    --work-dir) WORK=${2:?--work-dir needs a path}; shift 2 ;;
    -h|--help) sed -n '2,19p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
case "$WORK" in
  /*) ;;
  *) echo "--work-dir must be an absolute container path" >&2; exit 2 ;;
esac
if ! [[ "$WORK" =~ ^[A-Za-z0-9/._-]+$ ]]; then
  echo "--work-dir may only contain letters, digits and / . _ -" >&2; exit 2
fi

command -v docker >/dev/null || { echo "docker not found" >&2; exit 1; }
command -v git >/dev/null || { echo "git not found" >&2; exit 1; }

IMAGE=bulletspoker-reproducible:$(sha256sum build/reproducible/Dockerfile | cut -c1-16)
if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
  echo "building image $IMAGE" >&2
  docker build --platform linux/amd64 -t "$IMAGE" build/reproducible >&2
fi

COMMIT=$(git rev-parse HEAD)
# The build uses the committed tree, not the working copy. Say so when build
# inputs differ (line-ending-only differences are ignored).
git -c core.autocrlf=true diff --quiet HEAD -- Cargo.toml Cargo.lock rust-toolchain.toml src/poker_backend build/reproducible   || echo "warning: building committed HEAD; uncommitted changes are not included" >&2
SOURCE_DATE_EPOCH=$(git log -1 --format=%ct HEAD)
mkdir -p "$OUT"
OUT=$(cd "$OUT" && pwd)
echo "building poker_backend from commit $COMMIT" >&2

# The steps of the icp-cli @dfinity/rust@v3.3.0 recipe as icp.yaml configures
# it (candid only: no shrink, no compress), plus --locked and gzip -n -9.
# Runs in the container; reads the source archive on stdin.
# shellcheck disable=SC2016
INNER='
set -euo pipefail
export TZ=UTC LC_ALL=C.UTF-8
SRC=$WORK/src
export CARGO_HOME=$WORK/cargo-home
mkdir -p "$SRC" "$CARGO_HOME"
tar -x -C "$SRC" --no-same-owner --no-same-permissions
cd "$SRC"

# Toolchain pins (also checked by the image itself).
case "$(cargo --version)" in "cargo 1.94.1 "*) ;; *) echo "unexpected $(cargo --version)" >&2; exit 1 ;; esac
case "$(rustc --version)" in "rustc 1.94.1 "*) ;; *) echo "unexpected $(rustc --version)" >&2; exit 1 ;; esac
[ "$(ic-wasm --version)" = "ic-wasm 0.11.1" ] || { echo "unexpected $(ic-wasm --version)" >&2; exit 1; }

# Paths that could reach the module (panic locations, debug strings) are
# rewritten to fixed prefixes, so the build directory never shows.
export RUSTFLAGS="--remap-path-prefix=$SRC=/poker --remap-path-prefix=$CARGO_HOME=/cargo"
cargo build --locked --release --target wasm32-unknown-unknown -p poker_backend

W=$WORK/poker_backend.wasm
cp target/wasm32-unknown-unknown/release/poker_backend.wasm "$W"
ic-wasm "$W" -o "$W" metadata "cargo:version" -d "$(cargo --version)" --keep-name-section
ic-wasm "$W" -o "$W" metadata "template:type" -d "rust" --keep-name-section
ic-wasm "$W" -o "$W" metadata "candid:service" -f src/poker_backend/poker_backend.did -v public --keep-name-section

gzip -n -9 -c "$W" > /out/poker_backend.wasm.gz
cp src/poker_backend/poker_backend.did /out/poker_backend.did
(cd /out && sha256sum poker_backend.wasm.gz > poker_backend.wasm.gz.sha256)
chown "$HOST_UID:$HOST_GID" /out/poker_backend.wasm.gz /out/poker_backend.wasm.gz.sha256 /out/poker_backend.did
'

git -c core.autocrlf=false archive --format=tar "$COMMIT" \
    Cargo.toml Cargo.lock rust-toolchain.toml src/poker_backend \
  | docker run --rm -i --platform linux/amd64 \
      -e WORK="$WORK" -e SOURCE_DATE_EPOCH="$SOURCE_DATE_EPOCH" \
      -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" \
      -v "$OUT:/out" "$IMAGE" bash -c "$INNER"

cat "$OUT/poker_backend.wasm.gz.sha256"
