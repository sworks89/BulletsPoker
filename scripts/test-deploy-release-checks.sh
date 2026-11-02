#!/usr/bin/env bash
# Tests the release-JSON checks of scripts/deploy-release.sh that run before
# anything touches icp: only a release published by CI is accepted.
#
#   bash scripts/test-deploy-release-checks.sh
#
# Uses --from-dir (local network only), so nothing is installed anywhere. A
# release that passes the checks goes on to look up the local canister, which
# is expected to stop it later with a different message; this test only checks
# that the CI-author refusals are (or are not) the reason.
set -uo pipefail
cd "$(dirname "$0")/.."

T=$(mktemp -d)
trap 'rm -rf "$T"' EXIT
printf '\0asm\1\0\0\0' | gzip -n -9 > "$T/poker_backend.wasm.gz"
HASH=$(sha256sum "$T/poker_backend.wasm.gz" | cut -d' ' -f1)
echo "$HASH  poker_backend.wasm.gz" > "$T/poker_backend.wasm.gz.sha256"
echo 'service : {}' > "$T/poker_backend.did"

# release_json <release-author> <wasm-uploader> <sum-uploader> <did-uploader>
release_json() {
  python3 -I - "$HASH" "$@" > "$T/release.json" <<'PY'
import json, sys
h, author, *up = sys.argv[1:]
names = ["poker_backend.wasm.gz", "poker_backend.wasm.gz.sha256", "poker_backend.did"]
base = "https://github.com/sworks89/BulletsPoker/releases/download/v1.0.0/"
def who(l):
    return {"login": l} if l else None
print(json.dumps({
    "tag_name": "v1.0.0", "draft": False, "prerelease": False,
    "author": who(author),
    "body": "poker_backend.wasm.gz sha256: " + h + "\n",
    "assets": [{"name": n, "uploader": who(u), "browser_download_url": base + n,
                "digest": "sha256:" + h if i == 0 else None}
               for i, (n, u) in enumerate(zip(names, up))],
}))
PY
}

fail=0
BOT='github-actions[bot]'
# expect <name> <want: refused|passes> <author> <uploaders...>
expect() {
  local name=$1 want=$2; shift 2
  release_json "$@"
  local out
  out=$(bash scripts/deploy-release.sh v1.0.0 --from-dir "$T" 2>&1 </dev/null)
  local refused=no
  grep -q 'not built by CI' <<<"$out" && refused=yes
  if { [ "$want" = refused ] && [ "$refused" = yes ]; } || { [ "$want" = passes ] && [ "$refused" = no ]; }; then
    echo "ok   - $name"
  else
    echo "FAIL - $name (wanted $want)"; echo "$out" | sed 's/^/       /'; fail=1
  fi
}

expect "CI-built release is not refused for its author" passes "$BOT" "$BOT" "$BOT" "$BOT"
expect "release authored by a person is refused" refused someone "$BOT" "$BOT" "$BOT"
expect "release with no author is refused" refused "" "$BOT" "$BOT" "$BOT"
expect "wasm asset uploaded by a person is refused" refused "$BOT" someone "$BOT" "$BOT"
expect "wasm asset with no uploader is refused" refused "$BOT" "" "$BOT" "$BOT"
expect "sha256 asset uploaded by a person is refused" refused "$BOT" "$BOT" someone "$BOT"
exit $fail
