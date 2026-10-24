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

```bash
icp canister status poker_backend -e ic     # shows the module hash
```

or look up the canister on the ICP dashboard. If the module hash equals the
hash you built, the canister runs exactly the code at that commit.

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
