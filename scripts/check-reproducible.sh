#!/usr/bin/env bash
# Builds poker_backend twice with scripts/reproducible-build.sh, into two
# temporary output directories and from two different build paths inside the
# container, and fails with a byte-diff summary if the modules differ. It also
# fails if a build path, the cargo home or a user/home path shows up in the
# module.
#
#   scripts/check-reproducible.sh [--keep DIR]
#
# --keep DIR copies the first (checked) build's three files into DIR after the
# comparison succeeds, so a release publishes bytes that were actually compared.
set -euo pipefail
CALLER_DIR=$PWD
cd "$(dirname "$0")/.."

KEEP=
while [ $# -gt 0 ]; do
  case "$1" in
    --keep) KEEP=${2:?--keep needs a directory}; case "$KEEP" in /*) ;; *) KEEP=$CALLER_DIR/$KEEP ;; esac; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
A=$TMP/a
B=$TMP/b
WORK_A=/build
WORK_B=/tmp/another-place/xyz

bash scripts/reproducible-build.sh --out "$A" --work-dir "$WORK_A"
bash scripts/reproducible-build.sh --out "$B" --work-dir "$WORK_B"

HA=$(cut -d' ' -f1 "$A/poker_backend.wasm.gz.sha256")
HB=$(cut -d' ' -f1 "$B/poker_backend.wasm.gz.sha256")
echo "build 1 ($WORK_A): $HA"
echo "build 2 ($WORK_B): $HB"

status=0
gunzip -c "$A/poker_backend.wasm.gz" > "$TMP/a.wasm"
gunzip -c "$B/poker_backend.wasm.gz" > "$TMP/b.wasm"

if [ "$HA" != "$HB" ] || ! cmp -s "$A/poker_backend.wasm.gz" "$B/poker_backend.wasm.gz"; then
  echo "NOT REPRODUCIBLE: the two builds differ" >&2
  for f in poker_backend.wasm.gz; do
    echo "  $f: $(stat -c %s "$A/$f") vs $(stat -c %s "$B/$f") bytes" >&2
  done
  echo "  wasm: $(stat -c %s "$TMP/a.wasm") vs $(stat -c %s "$TMP/b.wasm") bytes," \
       "$(cmp -l "$TMP/a.wasm" "$TMP/b.wasm" 2>/dev/null | wc -l) differing bytes" >&2
  echo "  first differences in the wasm (offset, octal byte in build 1, build 2):" >&2
  cmp -l "$TMP/a.wasm" "$TMP/b.wasm" 2>/dev/null | head -10 | sed 's/^/    /' >&2 || true
  status=1
fi
cmp -s "$A/poker_backend.did" "$B/poker_backend.did" || { echo "the .did files differ" >&2; status=1; }

# Nothing machine- or user-specific may reach the module.
for needle in "$WORK_A/" "$WORK_B/" "cargo-home" "/usr/local/" "/home/" "/root/" "/mnt/" "/Users/"; do
  [ -n "$needle" ] || continue
  if grep -q -a -F -- "$needle" "$TMP/a.wasm" "$TMP/b.wasm"; then
    echo "LEAK: the module contains \"$needle\"" >&2
    status=1
  fi
done

if [ "$status" -eq 0 ]; then
  if [ -n "$KEEP" ]; then
    mkdir -p "$KEEP"
    cp "$A/poker_backend.wasm.gz" "$A/poker_backend.wasm.gz.sha256" "$A/poker_backend.did" "$KEEP/"
  fi
  echo "reproducible: $HA  poker_backend.wasm.gz"
fi
exit "$status"
