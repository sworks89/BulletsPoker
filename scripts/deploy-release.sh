#!/usr/bin/env bash
# Upgrades the game canister (poker_backend) to a published GitHub Release, and
# to nothing else.
#
#   scripts/deploy-release.sh <tag> [--network ic] [--identity NAME]
#
# The default network is the local one; pass --network ic for mainnet. The
# icp identity is the current default one, or NAME. The script:
#   1. refuses a draft or a pre-release (or a release it can't find);
#   2. downloads poker_backend.wasm.gz, poker_backend.wasm.gz.sha256 and
#      poker_backend.did from the release into a temporary directory;
#   3. checks the module's SHA-256 against the .sha256 asset, the release body
#      line "poker_backend.wasm.gz sha256: <hex>" and the asset digest GitHub
#      reports;
#   4. checks that the current icp identity is a controller, and shows the
#      live module hash next to the new one;
#   5. asks you to type "deploy <tag>";
#   6. installs in upgrade mode, with no install arguments (post_upgrade takes
#      none and icp.yaml sets none, the same as `icp deploy` does);
#   7. reads the live module hash back and checks it equals the release hash.
# Any mismatch stops it with a non-zero exit.
#
# It never builds, never runs in CI, and never installs a local file, except:
#
#   --from-dir DIR   FOR TESTING ONLY. Uses DIR/release.json (the GitHub API
#                    release JSON) and the three assets in DIR instead of
#                    downloading them. Refused with --network ic.
set -euo pipefail

REPO=sworks89/BulletsPoker
CANISTER=poker_backend
WASM=poker_backend.wasm.gz
SUM=poker_backend.wasm.gz.sha256
DID=poker_backend.did
ANONYMOUS=2vxsx-fae

die() { echo "deploy-release: REFUSED: $*" >&2; exit 1; }
# After the install has started, a stop is a failure, not a refusal: the canister's state may have changed.
fail() { echo "deploy-release: FAILED: $*" >&2; exit 1; }
usage() { echo "usage: scripts/deploy-release.sh <tag> [--network ic] [--identity NAME]" >&2; exit 2; }

# Never in CI: the controller key must never be there.
if [ -n "${CI:-}" ] && [ "${CI:-}" != "false" ]; then die "CI=$CI: this script never runs in CI"; fi
if [ -n "${GITHUB_ACTIONS:-}" ]; then die "GITHUB_ACTIONS is set: this script never runs in CI"; fi

TAG=
NETWORK=local
FROM_DIR=
IDENTITY=
while [ $# -gt 0 ]; do
  case "$1" in
    --network) [ $# -ge 2 ] || usage; NETWORK=$2; shift 2 ;;
    --network=*) NETWORK=${1#--network=}; shift ;;
    --identity) [ $# -ge 2 ] || usage; IDENTITY=$2; shift 2 ;;
    --identity=*) IDENTITY=${1#--identity=}; shift ;;
    --from-dir) [ $# -ge 2 ] || usage; FROM_DIR=$2; shift 2 ;;
    --from-dir=*) FROM_DIR=${1#--from-dir=}; shift ;;
    -h|--help) sed -n '2,/^set -euo/p' "$0" | sed '$d'; exit 0 ;;
    -*) echo "unknown option: $1" >&2; usage ;;
    *) [ -z "$TAG" ] || usage; TAG=$1; shift ;;
  esac
done
[ -n "$TAG" ] || usage

case "$NETWORK" in
  ic|local) ;;
  *) die "unknown network '$NETWORK' (use ic, or omit it for local)" ;;
esac
# A tag goes into URLs and the confirmation; allow only plain release tags.
# Tags with '-' are pre-releases (see release.yml) and are never deployed.
[[ "$TAG" =~ ^v[0-9A-Za-z._+]+$ ]] || die "'$TAG' is not a release tag (expected e.g. v1.4.0; pre-release tags with '-' are not deployed)"
if [ -n "$FROM_DIR" ] && [ "$NETWORK" = ic ]; then
  die "--from-dir is for testing only and is never used with --network ic"
fi
if [ -n "$FROM_DIR" ]; then
  abs=$(cd "$FROM_DIR" 2>/dev/null && pwd) || die "--from-dir: no directory $FROM_DIR"
  FROM_DIR=$abs
fi

for tool in icp curl python3 sha256sum gzip; do
  command -v "$tool" >/dev/null || die "$tool is not installed"
done

# Target exactly the canister named in this repository's icp.yaml, on exactly
# the environment chosen above.
unset ICP_NETWORK ICP_ENVIRONMENT ICP_PROJECT_ROOT
cd "$(dirname "$0")/.."
[ -f icp.yaml ] || die "no icp.yaml in $(pwd)"

TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT

# ------------------------------------------------------------------ acquire
if [ -n "$FROM_DIR" ]; then
  echo "TEST MODE: using $FROM_DIR instead of the GitHub Release (local network only)"
  [ -f "$FROM_DIR/release.json" ] || die "missing $FROM_DIR/release.json"
  cp "$FROM_DIR/release.json" "$TMP/release.json"
  for f in "$WASM" "$SUM" "$DID"; do
    [ -f "$FROM_DIR/$f" ] || die "release asset $f is missing"
    cp "$FROM_DIR/$f" "$TMP/$f"
  done
else
  API=https://api.github.com/repos/$REPO/releases/tags/$TAG
  echo "Fetching $API"
  code=$(curl -sS --proto '=https' --tlsv1.2 -o "$TMP/release.json" -w '%{http_code}' \
    -H 'Accept: application/vnd.github+json' "$API") || die "could not reach the GitHub API"
  case "$code" in
    200) ;;
    404) die "no published release $TAG (it doesn't exist, or it is a draft)" ;;
    *) die "GitHub API returned HTTP $code for $TAG" ;;
  esac
fi

# Read the release JSON (untrusted data). Writes meta.txt as KEY=VALUE lines
# and body.txt; fails if a field is missing or of the wrong type.
python3 -I - "$TMP/release.json" "$TMP" "$WASM" "$SUM" "$DID" <<'PY' || die "release JSON is malformed"
import json, sys
path, out, *names = sys.argv[1:]
with open(path, encoding="utf-8") as f:
    r = json.load(f)
assert isinstance(r, dict)
assert isinstance(r.get("tag_name"), str)
assert isinstance(r.get("draft"), bool) and isinstance(r.get("prerelease"), bool)
body = r.get("body") or ""
assert isinstance(body, str)
def login(o):
    u = o.get("login") if isinstance(o, dict) else None
    return u if isinstance(u, str) else ""
assets = r.get("assets")
assert isinstance(assets, list)
by_name = {}
for a in assets:
    assert isinstance(a, dict) and isinstance(a.get("name"), str)
    by_name.setdefault(a["name"], []).append(a)
lines = [
    "TAG_NAME=" + r["tag_name"],
    "DRAFT=" + str(r["draft"]).lower(),
    "PRERELEASE=" + str(r["prerelease"]).lower(),
    "AUTHOR=" + login(r.get("author")),
]
for i, n in enumerate(names):
    found = by_name.get(n, [])
    if len(found) != 1:
        lines.append(f"ASSET{i}=missing")
        continue
    a = found[0]
    url = a.get("browser_download_url") or ""
    digest = a.get("digest") or ""
    assert isinstance(url, str) and isinstance(digest, str)
    lines.append(f"ASSET{i}=present")
    lines.append(f"URL{i}=" + url)
    lines.append(f"UPLOADER{i}=" + login(a.get("uploader")))
    lines.append(f"DIGEST{i}=" + digest)
for l in lines:
    assert "\n" not in l and "\r" not in l
with open(out + "/meta.txt", "w", encoding="utf-8") as f:
    f.write("\n".join(lines) + "\n")
with open(out + "/body.txt", "w", encoding="utf-8") as f:
    f.write(body.replace("\r\n", "\n"))
PY
meta() { sed -n "s/^$1=//p" "$TMP/meta.txt"; }

[ "$(meta TAG_NAME)" = "$TAG" ] || die "the release JSON is for tag '$(meta TAG_NAME)', not $TAG"
[ "$(meta DRAFT)" = false ] || die "$TAG is a draft release"
[ "$(meta PRERELEASE)" = false ] || die "$TAG is a pre-release"
# Only what the release workflow built and published counts. A release made or
# edited by hand (even by a repository writer) is not deployed.
CI_LOGIN='github-actions[bot]'
[ "$(meta AUTHOR)" = "$CI_LOGIN" ]   || die "$TAG was not built by CI: its author is '$(meta AUTHOR)', not $CI_LOGIN; releases not built by CI are not deployed"

i=0
for f in "$WASM" "$SUM" "$DID"; do
  [ "$(meta "ASSET$i")" = present ] || die "release asset $f is missing (or listed more than once) in $TAG"
  [ "$(meta "UPLOADER$i")" = "$CI_LOGIN" ]     || die "$f was not built by CI: it was uploaded by '$(meta "UPLOADER$i")', not $CI_LOGIN; releases not built by CI are not deployed"
  i=$((i + 1))
done
i=0
for f in "$WASM" "$SUM" "$DID"; do
  if [ -z "$FROM_DIR" ]; then
    want=https://github.com/$REPO/releases/download/$TAG/$f
    [ "$(meta "URL$i")" = "$want" ] || die "asset $f has an unexpected download URL: $(meta "URL$i")"
    echo "Downloading $want"
    curl -fsSL --proto '=https' --tlsv1.2 -o "$TMP/$f" "$want" || die "could not download $f"
  fi
  [ -s "$TMP/$f" ] || die "release asset $f is empty"
  i=$((i + 1))
done

# ------------------------------------------------------------------- hashes
HASH=$(sha256sum "$TMP/$WASM" | cut -d' ' -f1)

# The .sha256 asset: exactly one sha256sum line for the bare file name.
[ "$(wc -l <"$TMP/$SUM")" -eq 1 ] || die "$SUM must contain exactly one line"
SUMLINE=$(cat "$TMP/$SUM")
[[ "$SUMLINE" =~ ^([0-9a-f]{64})\ \ poker_backend\.wasm\.gz$ ]] \
  || die "$SUM is not a sha256sum line for $WASM: $SUMLINE"
[ "${BASH_REMATCH[1]}" = "$HASH" ] \
  || die "hash mismatch: $WASM is $HASH but $SUM says ${BASH_REMATCH[1]}"

# The release body: exactly one machine line, equal to the file's hash.
BODYHASHES=$(sed -n 's/^poker_backend\.wasm\.gz sha256: \([0-9a-f]\{64\}\)$/\1/p' "$TMP/body.txt")
[ -n "$BODYHASHES" ] || die "the release body has no line 'poker_backend.wasm.gz sha256: <hex>'"
[ "$(printf '%s\n' "$BODYHASHES" | wc -l)" -eq 1 ] || die "the release body has more than one hash line"
[ "$BODYHASHES" = "$HASH" ] \
  || die "hash mismatch: $WASM is $HASH but the release body says $BODYHASHES"

# GitHub's own digest of the uploaded asset, when the API reports one.
DIGEST=$(meta DIGEST0)
if [ -n "$DIGEST" ]; then
  [ "$DIGEST" = "sha256:$HASH" ] || die "hash mismatch: $WASM is $HASH but GitHub's asset digest is $DIGEST"
fi

# It must be a gzipped wasm module.
gzip -t "$TMP/$WASM" 2>/dev/null || die "$WASM is not a valid gzip file"
[ "$(gzip -dc "$TMP/$WASM" | head -c 4 | od -An -tx1 | tr -d ' \n')" = 0061736d ] \
  || die "$WASM does not contain a wasm module"

echo "Release $TAG checked: $WASM sha256 $HASH"

# --------------------------------------------------------------- the target
ENV_ARGS=(-e "$NETWORK")
ID_ARGS=()
if [ -n "$IDENTITY" ]; then
  [[ "$IDENTITY" =~ ^[A-Za-z0-9._-]+$ ]] || die "bad identity name: $IDENTITY"
  ID_ARGS=(--identity "$IDENTITY")
fi

CANISTER_ID=$(icp canister status "$CANISTER" "${ENV_ARGS[@]}" --id-only) \
  || die "no $CANISTER canister ID for the $NETWORK environment in this project. Record the existing canister once with: icp canister link $CANISTER <canister-id> -e $NETWORK  (confirm the ID independently first, e.g. on the IC dashboard; every deploy trusts it). See docs/reproducible-builds.md, 'Link the canister (once)'."
[ -n "$CANISTER_ID" ] || die "no $CANISTER canister ID for the $NETWORK environment; link it with: icp canister link $CANISTER <canister-id> -e $NETWORK (confirm the ID independently first). See docs/reproducible-builds.md, 'Link the canister (once)'."

# Prints "<module hash or none> <controller> <controller> ..." from a status read.
live_status() {
  local json
  json=$(icp canister status "$CANISTER" "${ENV_ARGS[@]}" "${ID_ARGS[@]}" --json) || return 1
  printf '%s' "$json" | python3 -I -c '
import json, re, sys
s = json.load(sys.stdin)
if s.get("id") != sys.argv[1]:
    sys.exit("status is for canister %r, not %s" % (s.get("id"), sys.argv[1]))
h = s.get("module_hash")
if h is None:
    h = "none"
else:
    h = h.lower()
    h = h[2:] if h.startswith("0x") else h
    if not re.fullmatch(r"[0-9a-f]{64}", h):
        sys.exit("bad module_hash: " + h)
# A controller gets the full status (controllers under "settings"); anyone
# else gets the public state-tree view (controllers at the top level).
c = s.get("controllers")
if c is None and isinstance(s.get("settings"), dict):
    c = s["settings"].get("controllers")
if not isinstance(c, list) or not all(isinstance(x, str) and re.fullmatch(r"[a-z0-9-]+", x) for x in c):
    sys.exit("bad controllers")
print(" ".join([h] + c))
' "$CANISTER_ID"
}

STATUS=$(live_status) || die "could not read the status of $CANISTER on $NETWORK"
read -r LIVE CONTROLLERS <<<"$STATUS"

ME=$(icp identity principal "${ID_ARGS[@]}") || die "could not read the current icp identity"
[ -n "$ME" ] || die "could not read the current icp identity"
[ "$ME" != "$ANONYMOUS" ] || die "the current icp identity is anonymous; select the controller identity (--identity NAME, or icp identity default NAME)"
case " $CONTROLLERS " in
  *" $ME "*) ;;
  *) die "the current identity $ME is not a controller of $CANISTER_ID (controllers: ${CONTROLLERS:-none})" ;;
esac

[ "$LIVE" != none ] || die "$CANISTER_ID has no module installed; this script only upgrades"
if [ "$LIVE" = "$HASH" ]; then
  echo "$CANISTER_ID already runs $TAG ($HASH). Nothing to do."
  exit 0
fi

# ------------------------------------------------------------- confirmation
cat <<EOF

  Network:     $NETWORK
  Canister:    $CANISTER ($CANISTER_ID)
  Identity:    $ME
  Live module: $LIVE
  New module:  $HASH  ($TAG)
  Mode:        upgrade (no install arguments)

EOF
if [ "$NETWORK" = ic ] && [ ! -t 0 ]; then
  die "mainnet needs a typed confirmation from a terminal"
fi
printf 'Type "deploy %s" to install it: ' "$TAG"
ANSWER=
read -r ANSWER || true
[ "$ANSWER" = "deploy $TAG" ] || die "not confirmed; nothing was installed"

# ------------------------------------------------------------------ install
# Upgrade mode only, and no --args: post_upgrade takes no arguments, and
# init's InitArgs must never be resent. No --yes, so icp's own Candid
# compatibility prompt still applies.
icp canister install "$CANISTER" "${ENV_ARGS[@]}" "${ID_ARGS[@]}" --mode upgrade --wasm "$TMP/$WASM" \
  || fail "icp canister install failed; check the canister's module hash before retrying (icp canister status $CANISTER -e $NETWORK)"

STATUS=$(live_status) || fail "the install may have completed, but the module hash could not be read back; check it with: icp canister status $CANISTER -e $NETWORK"
read -r AFTER _ <<<"$STATUS"
if [ "$AFTER" != "$HASH" ]; then
  echo "deploy-release: FAILED: the live module hash is $AFTER, not the release hash $HASH" >&2
  exit 1
fi
echo "Done: $CANISTER ($CANISTER_ID) on $NETWORK runs $TAG, module hash $AFTER"
