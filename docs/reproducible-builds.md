# Reproducible builds

Anyone can rebuild the game canister (`poker_backend`) from this repository and
get, byte for byte, the module the Internet Computer runs. The published hash is
the SHA-256 of `poker_backend.wasm.gz`, the exact file that gets installed, and
that is also the `module_hash` the IC reports for the canister.

## Build it

You need git, bash and Docker (Linux, or WSL on Windows; the image is
`linux/amd64`).

```bash
git clone https://github.com/sworks89/BulletsPoker.git
cd BulletsPoker
git checkout <tag or commit>
scripts/reproducible-build.sh            # or: --out DIR (default out/)
```

It writes:

- `out/poker_backend.wasm.gz`: the module to install;
- `out/poker_backend.wasm.gz.sha256`: its hash, in `sha256sum` format;
- `out/poker_backend.did`: the Candid interface.

and prints the hash. Check it with `sha256sum out/poker_backend.wasm.gz`.

To check that the build really is reproducible on your machine, run
`scripts/check-reproducible.sh`. It builds twice, from two different build
directories into two different output directories, and fails with a byte-diff
summary if the modules differ, or if a build path, the cargo home, or a home
or user path appears in the module.

## Compare with the live canister

The strongest check is the one you run yourself. The badge in the app's footer
is drawn by the operator's own website, so it is a convenience, not evidence
(see "What the badge shows" below). To check without trusting the site:

```bash
# 1. Rebuild the release (needs Docker; see above).
git checkout v1.4.0 && scripts/reproducible-build.sh
sha256sum out/poker_backend.wasm.gz

# 2. The hash the release publishes: the line "poker_backend.wasm.gz sha256: <hex>"
#    in the release notes on GitHub, or the .sha256 asset.

# 3. The hash the Internet Computer reports for the live canister. No identity
#    or project is needed, only the canister ID (confirm that ID independently,
#    see "Link the canister" below):
icp canister status <canister-id> -n ic --public
```

`--public` reads the public part of the canister's status from the IC's state
tree, so it works for anyone, not only controllers. It prints `Module hash:
0x<hex>`. The ICP dashboard shows the same value. Without the `0x`, the three
hashes (your build, the release, the IC) must be identical. If they are, the
canister runs exactly the code at that tag, built from the source you can read.
None of these steps goes through the game's website.

## Local builds are not releases

`rust-toolchain.toml` says `stable`, so a plain local `icp deploy` builds with
whatever stable Rust you have installed, from your working tree, and without
path remapping or gzip. That module will usually differ from the pinned 1.94.1
release build. This is expected: only releases are reproducible, and only
releases are meant for mainnet (see "Deploy a release").

The first release deploy also changes the mainnet module hash even if the code
is unchanged. The release module is the gzipped one, and the IC hashes the
installed bytes, so it differs from the raw wasm that `icp deploy` installed
before.

## Publish a release

1. Merge to `main`. The `reproducible` job in `.github/workflows/ci.yml` runs
   `scripts/check-reproducible.sh` on every push and pull request.
2. Tag a commit on `main` with a `v*` tag and push the tag:

   ```bash
   git tag v1.4.0 && git push origin v1.4.0
   ```

3. `.github/workflows/release.yml` runs on the tag. It fails if the tagged
   commit isn't on `main`. It runs `scripts/check-reproducible.sh --keep out`
   (two builds, byte comparison, and the first build is kept), writes the notes
   with `scripts/release-notes.sh`, and creates the GitHub Release with
   `poker_backend.wasm.gz`, `poker_backend.wasm.gz.sha256` and
   `poker_backend.did`. A tag containing `-` (for example `v1.4.0-rc1`) becomes a
   pre-release; the workflow never creates a draft.

The notes carry the toolchain pins and one machine-readable line, on a line of
its own:

```
poker_backend.wasm.gz sha256: <64 lowercase hex>
```

The release workflow uses only GitHub's built-in token. CI holds no controller
key and never touches the canister.

## What the build does

The source is the committed tree (`git archive` of `HEAD`), never the working
tree, so uncommitted edits and checkout line endings (CRLF on Windows) can't
change the result. Only the build inputs are copied: `Cargo.toml`,
`Cargo.lock`, `rust-toolchain.toml` and `src/poker_backend/`. The archive is
streamed into a fresh container, unpacked and built there. Nothing in the
repository is written; only the output directory is mounted.

### Pins

| What | Pin |
| --- | --- |
| Base image | `rust:1.94.1-slim-bookworm`, `linux/amd64`, by digest `sha256:cf9dd0ec73e75f827fe59123fff9dc65af1a1c8363c3c31ee8d7f8ad0b6a5fb2` |
| Rust toolchain | 1.94.1 (`RUSTUP_TOOLCHAIN=1.94.1` overrides the `stable` channel in `rust-toolchain.toml`); the build checks `cargo --version` and `rustc --version` |
| wasm32 standard library | `rustup target add --toolchain 1.94.1 wasm32-unknown-unknown` |
| ic-wasm | 0.11.1, the static `x86_64-unknown-linux-musl` release, verified by the tarball's SHA-256 `86d784f95c74284d8748aec6ebc6c15d57992dae74b53902f64cc01b727cd2be` |
| gzip | 1.12, from the pinned base image |
| Rust dependencies | `Cargo.lock`, with `--locked` |

See `build/reproducible/Dockerfile`.

### Determinism

- `cargo build --locked --release --target wasm32-unknown-unknown -p poker_backend`.
- `RUSTFLAGS="--remap-path-prefix=<build dir>/src=/poker --remap-path-prefix=<cargo home>=/cargo"`.
  Panic locations and other strings from the code and its dependencies carry
  source paths; this rewrites them to fixed prefixes. Without it, two builds
  from different directories differ (about 965,000 bytes of the wasm) and the
  module contains the build paths.
- `SOURCE_DATE_EPOCH` is the commit time, `TZ=UTC`, `LC_ALL=C.UTF-8`. Nothing
  in the build currently reads the time; these are there in case something
  does.
- Fixed `ic-wasm` steps (below), then `gzip -n -9`, which writes no file name
  and no timestamp.
- Each build gets a fresh container: a fresh target directory and cargo home,
  so nothing is reused from an earlier build.

### Steps, compared with the icp-cli recipe

`icp.yaml` builds the canister with the `@dfinity/rust@v3.3.0` recipe,
configured with only `candid: src/poker_backend/poker_backend.did`, so its
`locked`, `shrink` and `compress` options are off. The recipe then runs:

1. `cargo build --package poker_backend --target wasm32-unknown-unknown --release`
2. copy `target/wasm32-unknown-unknown/release/poker_backend.wasm` to the output
3. `ic-wasm … metadata "cargo:version" -d "$(cargo --version)" --keep-name-section`
4. `ic-wasm … metadata "template:type" -d "rust" --keep-name-section`
5. `ic-wasm … metadata "candid:service" -f src/poker_backend/poker_backend.did -v public --keep-name-section`
6. no shrink, no gzip.

The reproducible build runs the same steps in the same order, with the same
metadata (`cargo:version` = `cargo 1.94.1 (29ea6fb6a 2026-03-24)` and
`template:type` = `rust`, both private; `candid:service` public) and the name
section kept. Differences:

- **`--locked`**: the build fails instead of updating `Cargo.lock`. With an
  up-to-date lock file the output is the same.
- **Path remapping** (above): only the embedded path strings change. Compared
  with the recipe's steps run on the same commit outside Docker, `ic-wasm info`
  is identical (functions, exports, metadata) except the data section, which
  is 1,024 bytes smaller because the remapped paths are shorter.
- **gzip -n -9**: the recipe installs the raw wasm (its `compress` option is
  off; when on, it uses `gzip --no-name` at the default level). The released
  module is gzipped, about 0.9 MB instead of 4.1 MB, and the IC accepts it
  directly. The IC's `module_hash` is the SHA-256 of the installed bytes, so it
  is the hash of the `.wasm.gz`, and differs from the hash of a raw-wasm
  `icp deploy`.

## Tests

`scripts/integration-test.sh` runs the PocketIC suite against a module you
choose:

```bash
BACKEND_WASM=out/poker_backend.wasm.gz scripts/integration-test.sh
```

The test `module_hash_is_the_sha256_of_the_installed_bytes` installs the
module and checks that the `module_hash` from `canister_status` equals the
SHA-256 of the file and the hash in `poker_backend.wasm.gz.sha256`.

## Deploy a release

The game canister on mainnet is only ever upgraded to a published release,
with `scripts/deploy-release.sh`, run by the owner on their own machine with
their own `icp` identity (in WSL on Windows). CI never deploys and never holds
a controller key.

```bash
scripts/deploy-release.sh v1.4.0 --network ic                    # current icp identity
scripts/deploy-release.sh v1.4.0 --network ic --identity owner   # or a named one
```

Without `--network ic` it targets the local network. It needs `icp`, `curl`,
`python3`, `sha256sum` and `gzip`; it doesn't need `gh`. It:

1. Refuses to run in CI (`CI` set to anything but `false`, or `GITHUB_ACTIONS`
   set), and refuses a tag that isn't a plain release tag (`v1.4.0`; tags with
   `-` are pre-releases).
2. Reads the release from
   `https://api.github.com/repos/sworks89/BulletsPoker/releases/tags/<tag>`.
   A draft is not visible there (404), and a draft or pre-release is refused.
3. Downloads `poker_backend.wasm.gz`, `poker_backend.wasm.gz.sha256` and
   `poker_backend.did` from the release into a temporary directory. A missing
   asset stops it.
4. Checks that the SHA-256 of `poker_backend.wasm.gz` equals the hash in the
   `.sha256` asset, the release body line `poker_backend.wasm.gz sha256: <hex>`
   (exactly one such line) and GitHub's own `digest` of the asset, when the API
   reports one; and that the file is a gzipped wasm module.
5. Resolves the `poker_backend` canister ID for the environment from icp-cli's
   ID mapping for this project (`icp canister status poker_backend -e ic
   --id-only`). If this checkout has no `ic` mapping, for example because the
   canister was never deployed with icp-cli from it, the script refuses (see
   "Link the canister" below); it has no flag to name a canister ID by hand.
   It then reads the canister's status: the live module hash and the
   controllers. It refuses the anonymous identity and any identity that
   isn't a controller, and refuses a canister with no module (it only
   upgrades). If the live hash already equals the release, it stops there.
6. Shows the network, canister ID, identity, live module hash and new module
   hash, and asks you to type `deploy <tag>`. On mainnet the answer must come
   from a terminal, so it can't be piped in.
7. Runs `icp canister install poker_backend -e ic --mode upgrade --wasm <file>`
   with no install arguments: `post_upgrade` takes none and `icp.yaml` sets
   none, so this is what `icp deploy -e ic` sends on an upgrade. `InitArgs` is
   never sent. There is no `--yes`, so icp's own Candid compatibility prompt
   still applies.
8. Reads the module hash back and fails, with a non-zero exit, unless it
   equals the release hash.

It never builds and never installs a local file. The only exception is
`--from-dir DIR`, **for testing only**: it takes `DIR/release.json` (in the
GitHub API format) and the three assets from `DIR` instead of downloading
them, and it is refused with `--network ic`. For example, on a local network:

```bash
scripts/check-reproducible.sh --keep /tmp/rel   # then write /tmp/rel/release.json
scripts/deploy-release.sh v9.9.9 --from-dir /tmp/rel --identity <local controller>
```

### Link the canister (once)

`scripts/deploy-release.sh` takes the canister ID from icp-cli's ID mapping for
the `ic` environment. A checkout that never ran `icp deploy -e ic` has none, so
record the existing mainnet canister once:

```bash
icp canister link poker_backend <canister-id> -e ic
```

(`--force` overwrites an ID already recorded.) Confirm the ID independently
before you link it, for example on the ICP dashboard or against the ID your
users see, because every later deploy trusts it. The command writes
`.icp/data/mappings/ic.ids.json`. That path is not covered by `.gitignore`
(only `.icp/cache/` is), so you can commit the file to share the mapping.
If you don't, each checkout needs the link once. Check it with
`icp canister status poker_backend -e ic --id-only`.

## What the badge shows

The app's footer badge (`src/poker_frontend/src/buildCheck.ts` and
`components/BuildBadge.tsx`) tells a visitor whether the live game canister's
module hash equals a published release. On the live network it:

- reads `module_hash` and `controllers` from the IC's certified `read_state`,
  checked by the agent against the IC's root key, which is pinned to the
  mainnet key and not taken from a cookie or the page;
- reads the releases from the GitHub API (`/repos/sworks89/BulletsPoker/releases`,
  one unauthenticated request, cached for 2 minutes in the tab). GitHub lists
  30 releases per page and the badge reads only that first page, so only the
  latest 30 releases can match;
- counts a release only if it isn't a draft or pre-release, its body has
  exactly one `poker_backend.wasm.gz sha256: <hex>` line, and the
  `poker_backend.wasm.gz` asset is part of the release. If GitHub reports a
  digest for that asset and it disagrees with the body (or can't be read), the
  release is ignored.

Its states:

| Badge | Meaning |
| --- | --- |
| matches the latest published release `vX` | the live hash equals the newest release's hash |
| matches the published release `vX`, a newer one exists | the live hash equals an older release |
| does not match any published release | no usable release has the live hash |
| could not check | no releases could be read, the hash couldn't be read, or a request failed |
| not checked on a local network | the page is served from localhost |

Under it, one line shows the controllers: "The owner can still upgrade this
code (controllers: ...)", "No controllers: this code can't be upgraded", or
"Couldn't read who can upgrade this code" when the read failed (an unreadable
list is never shown as "none").

### What it does and doesn't prove

- It is drawn by the operator's website. A site that lies could show a green
  badge, so the strongest check is to rebuild yourself and compare with the
  IC's module hash (see "Compare with the live canister").
- A matching hash proves the running code is the code in the published
  release, which anyone can rebuild and read. It doesn't prove the code is
  honest or fair; that needs review of the source.
- It doesn't stop future upgrades. While there are controllers, they can
  install different code; the controllers line shows who they are. A new
  release or an unlisted module would change the badge, but only for someone
  who looks.
- It covers the game canister (`poker_backend`) only, not the frontend canister
  that serves the website.
