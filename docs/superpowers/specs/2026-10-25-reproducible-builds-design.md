# Reproducible builds for the game canister

## Why

The verifiable shuffle (`docs/provably-fair-threat-model.md`) still trusts that
the live game canister runs the published code. A canister running different
code could choose among seeds or read cards. Reproducible builds close that gap
for anyone willing to check: rebuild the public source and get the same module
hash that the Internet Computer reports for the live canister. A badge in the
app makes the result visible.

## Goals

- **Reproducible build.** Anyone can build the game canister (`poker_backend`)
  from the public repository and get a byte-identical installable module. That
  module has the same SHA-256 the IC reports as the canister's module hash.
- **Release pipeline.** Pushing a version tag (`v*`) makes GitHub Actions:
  - build the module twice in the pinned environment;
  - check that the two builds are identical;
  - publish a GitHub Release with the module, its hash, the Candid file and the
    commit.
- **Release-only deploys.** The owner deploys only released modules, from their
  own machine. The script refuses anything whose hash doesn't match the release.
- **In-app badge.** It shows whether the live game canister's module hash
  matches a published release, and who can still upgrade it.

## Non-goals

- The website and frontend assets (the owner chose the game canister only, for
  now).
- Removing upgrade control (SNS or blackholing are separate decisions).
- Signing releases, or third-party attestation beyond "rebuild it yourself".

## Anti-goals

- A badge that says "verified" when it can't actually tell. Every state must be
  honest, including "couldn't check" and "unverified".
- Moving the controller key into CI. It stays on the owner's machine.

## Decisions taken with the owner

- **Scope:** the game canister only.
- **The repository is public** (`sworks89/BulletsPoker`). It must stay public:
  the reproduce steps and the badge's release lookup both depend on that.
- **Audience:** a badge in the app for now. The written reproduce steps are for
  anyone who wants to go further.
- **Deploy model:** GitHub builds and publishes, and the owner deploys from
  their own machine with `./scripts/deploy-release.sh <tag>`. That script
  downloads the release module, checks its hash, and installs it with the
  owner's local `icp` identity. Local development keeps using `icp deploy`.

## How it works

### The build

- **Pinned environment:** a Dockerfile in the repo
  (`build/reproducible/Dockerfile`), pinned throughout:
  - the base image by digest;
  - the Rust toolchain by exact version, with the `wasm32-unknown-unknown`
    target;
  - `ic-wasm` and any other tools by exact version.
- **One entry point:** `scripts/reproducible-build.sh` builds inside that image
  and writes:
  - `out/poker_backend.wasm.gz`
  - `out/poker_backend.wasm.gz.sha256`
  - `out/poker_backend.did`
- **Determinism:**
  - `cargo build --locked --release --target wasm32-unknown-unknown`, with
    path remapping so the checkout location doesn't leak into the binary;
  - `SOURCE_DATE_EPOCH` set from the commit;
  - fixed `ic-wasm` steps (metadata such as `candid:service` set
    deterministically);
  - `gzip -n`, so there's no name or timestamp in the archive.
- **Same module as `icp deploy`.** The module must be functionally equivalent
  to what the icp-cli Rust recipe produces today: same features, metadata and
  shrinking. The builder checks this against the recipe's output and documents
  any difference.
- **The hash.** The IC's module hash is the SHA-256 of the module as installed.
  If the gzipped module is installed, the published hash is of the `.wasm.gz`
  bytes. The builder confirms on a local network that the hash
  `icp canister status` reports equals the published one.

### Releases

- **Trigger:** a new workflow, `.github/workflows/release.yml`, runs on `v*`
  tags.
- **Permissions:** `contents: write` only in this workflow. The existing CI
  stays read-only.
- **Pinning:** actions are pinned by SHA, matching the current CI style.
- **Steps:** build twice (two separate image runs, in a clean workspace each
  time), compare the bytes, fail on any difference, then create the Release
  with:
  - the three files;
  - the source commit;
  - the toolchain versions;
  - the reproduce steps in the release notes.
- **Normal CI:** a reproducibility check (build twice, compare) also runs on
  pull requests, so a change that breaks determinism is caught before release.

### Deploying a release

- **Script:** `scripts/deploy-release.sh <tag>`.
- **Steps:**
  1. Download the release assets with `gh release download`, or from the public
     URLs.
  2. Recompute the SHA-256.
  3. Refuse to continue on a mismatch.
  4. Show the current and the new module hash, and ask for confirmation.
  5. Install with `icp` to mainnet as an upgrade. Keep the existing install and
     upgrade arguments the backend needs, and check what `icp deploy -e ic`
     passes today.
  6. Afterwards, read the live module hash back and confirm it equals the
     release hash.
- **No CI involvement:** the controller identity is the owner's local `icp`
  identity.

### The badge

- **Where:** in the app footer, or in the area next to the existing build and
  status information.
- **The live hash:** read in the browser through the IC's certified
  `read_state` (`canister/<id>/module_hash` and `canister/<id>/controllers`),
  using `@icp-sdk/core`'s canister-status support. The agent verifies the
  subnet's certificate, so the website can't fake the hash.
- **The expected hashes:** read from the public GitHub Releases API for this
  repository, which is third-party data and not the operator's site. Use the
  release asset `poker_backend.wasm.gz.sha256`, cache the result for a short
  time, and handle rate limits.
- **Which releases count:** any published release, excluding drafts and
  pre-releases (*my call*). When the live module matches a release older than
  the newest one, the badge says so: "matches release v1.3.0 (newer: v1.4.0)".
  Rolling back to an old release stays visible instead of looking current.
- **States:**
  - **"Game code matches release vX.Y.Z"**, linking to the release and the
    reproduce steps.
  - **"Game code doesn't match any published release"** (unverified build).
  - **"Couldn't check"**, when the network or GitHub is unreachable. This is
    never shown as a failure.
- **Upgrade notice:** in every state, a line such as "The owner can still
  upgrade this code (controllers: …)", from the certified controllers list.
- **Honest wording:**
  - The badge is shown by the website, which the operator also controls, so the
    strongest check is to rebuild yourself. The badge links to how.
  - It covers the game canister only, not the website.
  - On local networks it shows "Not checked (local network)".

### Docs

- `docs/reproducible-builds.md` covers:
  - how to reproduce a release (`docker`, `scripts/reproducible-build.sh`,
    compare with the release and with the live hash);
  - how to release and deploy;
  - what the badge proves and doesn't.
- `docs/provably-fair-threat-model.md` is updated where it lists verifiable
  builds as future work. That threat (a dealer choosing among seeds through a
  code change) becomes "detectable for anyone checking the module hash, while
  the canister remains upgradeable".

## Constraints

- Follow existing CI conventions: actions pinned by SHA, read-only by default.
- No secrets in any workflow except the default `GITHUB_TOKEN`, which is needed
  to publish the release.
- The frontend uses no new npm dependency. `@icp-sdk/core` is already a
  dependency. A `fetch` to `api.github.com` must be allowed by the app's CSP;
  check `.ic-assets` and the CSP config, and update them if needed.
- The current `icp deploy` flow for local development keeps working.

## Left to the builder

- Image choice and exact pins.
- Whether the deterministic build drives `cargo` and `ic-wasm` directly or
  through the icp-cli recipe in the container, as long as the result is
  reproducible and equivalent.
- Badge placement and styling.
- Release note format.
