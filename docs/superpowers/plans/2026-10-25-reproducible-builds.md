# Reproducible Builds Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:**
- Anyone can rebuild the game canister (`poker_backend`) from the public source and get the exact module the IC runs.
- Tagged releases are built and published by CI.
- The owner deploys only released modules.
- A badge in the app shows whether the live module matches a release.

**Architecture:**
- **Build:** a pinned Docker image and one build script produce `poker_backend.wasm.gz`, its SHA-256 and the `.did` file.
- **CI:** builds twice and compares the results on every PR. On `v*` tags, a release workflow publishes a GitHub Release.
- **Deploy:** a local script downloads the release, checks it and installs it on mainnet with the owner's identity.
- **Badge:** the frontend reads the live module hash and controllers through the IC's certified `read_state`, and reads the release hashes from the GitHub Releases API.

**Tech Stack:** Docker, bash, GitHub Actions, icp-cli 1.7.0, Rust (wasm32), `ic-wasm`, React + TypeScript (`@icp-sdk/core`), vitest.

**Spec:** `docs/superpowers/specs/2026-10-25-reproducible-builds-design.md`

## Global Constraints

- **Pinning:**
  - Base image by digest.
  - Rust toolchain by exact version. 1.94.1 is what local builds use today; confirm before choosing.
  - `ic-wasm` by exact version (0.11.1 locally). Any other tool also by exact version.
  - GitHub Actions by full commit SHA, with a version comment, as in `ci.yml`.
- **Determinism:**
  - `cargo build --locked --release --target wasm32-unknown-unknown -p poker_backend`.
  - `RUSTFLAGS` with `--remap-path-prefix` for the workspace and the cargo home.
  - `SOURCE_DATE_EPOCH` set from the commit.
  - Fixed `ic-wasm` steps.
  - `gzip -n -9`.
  - The output must not depend on the checkout path, user, time or machine.
- **Same module as today.** The reproducible module must match what the icp-cli `@dfinity/rust` recipe does in features, metadata (e.g. `candid:service`) and shrinking. The recipe template is cached under `~/.local/share/icp-cli/pkg/recipes/*/recipe.hbs` in WSL. Read it and mirror its steps.
- **The hash:** the published hash is the SHA-256 of `poker_backend.wasm.gz`, the exact bytes that get installed. Task 1 confirms it equals the `module_hash` the IC reports after install.
- **Secrets:**
  - The release workflow uses only `GITHUB_TOKEN`, with `contents: write`, in that workflow only.
  - `ci.yml` stays read-only.
  - No controller key in CI, ever.
- **How the badge gets the hash.** Browsers can't read release asset downloads cross-origin (the redirect has no CORS headers), but `api.github.com` allows it. So:
  - each Release body carries a machine-readable line, exactly `poker_backend.wasm.gz sha256: <64 lowercase hex>`;
  - the badge reads that line from `GET https://api.github.com/repos/sworks89/BulletsPoker/releases` (unauthenticated);
  - the badge also cross-checks the asset's `digest` field (`sha256:…`) when the API returns one.
- **CSP:** add `https://api.github.com` to `connect-src` in `src/poker_frontend/public/_headers`. Nothing else changes.
- **Badge states and copy** (exact meaning; the wording can be polished):
  - matches the latest release `vX`;
  - matches an older release `vX` (newer: `vY`);
  - doesn't match any published release;
  - couldn't check (network or GitHub unavailable; never shown as a failure);
  - not checked (local network).

  Always add: "The owner can still upgrade this code (controllers: …)". Always link: "Check it yourself", pointing to `docs/reproducible-builds.md` on GitHub.
- **Which releases count:** drafts and pre-releases are excluded.
- **Commands:** run them inside WSL, as `MSYS_NO_PATHCONV=1 wsl -d Ubuntu-24.04 -- bash -lc 'cd /mnt/c/BulletsPoker && …'`.
  - Is Docker available in WSL? Check first. If not, stop and report NEEDS_CONTEXT; don't install system packages without asking.
  - Backend: `cargo test`.
  - Frontend: `cd src/poker_frontend && npx vitest run && npm run build`. No new npm dependencies.
- **Shell script line endings:** keep LF (`.gitattributes` has `*.sh text eol=lf`). Run new scripts with `bash`.

## Review Focus

1. **Really reproducible:** two builds from different checkout paths, at different times, give identical bytes. Nothing in the module leaks a path, a timestamp or a username.
2. **The hash is the right one:** the published hash equals the `module_hash` the IC reports after installing that file, which is checked on the local network.
3. **The deploy script can't install the wrong thing:**
   - it refuses on a hash mismatch, a missing asset, a draft or a pre-release tag, or an identity that isn't a controller;
   - it confirms before installing;
   - it verifies the live hash afterwards;
   - it never passes install arguments that would reinitialise state (upgrade mode only).
4. **The badge can't be fooled cheaply:**
   - the live hash comes from a certificate-verified `read_state`, not from a canister method;
   - a release body whose hash line disagrees with the asset `digest` is treated as no match;
   - errors give "couldn't check", never green.
5. **Release workflow safety:**
   - it runs only on `v*` tags;
   - it builds twice from clean workspaces and fails on a difference;
   - write permission exists only there;
   - no other workflow can publish.

---

### Task 1: Reproducible build

**Files:**
- Create:
  - `build/reproducible/Dockerfile`
  - `scripts/reproducible-build.sh`
  - `scripts/check-reproducible.sh`
  - `docs/reproducible-builds.md`, a first version with build and reproduce steps
- Modify: `.gitignore`, to add `out/`.

**Interfaces:**
- **`scripts/reproducible-build.sh [--out DIR]`:**
  - builds the image if needed and runs the build inside it, with the repo mounted read-only and a copy built inside the container;
  - writes `DIR/poker_backend.wasm.gz`, `DIR/poker_backend.wasm.gz.sha256` (the `sha256sum` format) and `DIR/poker_backend.did`;
  - prints the hash.
- **`scripts/check-reproducible.sh`:** runs two builds into two different temporary directories, from two different copy paths, and exits non-zero with a byte-diff summary if they differ.

- [ ] **Step 1: Read the icp-cli Rust recipe** and record its exact steps in `docs/reproducible-builds.md` (shrink, metadata, gzip or not). Note any step the reproducible build does differently, and why.
- [ ] **Step 2: Write and run `check-reproducible.sh`.** Expected: identical hashes. If they differ, find the cause (paths, timestamps, metadata) and fix it. Never mask the difference.
- [ ] **Step 3: Check against the IC.** On the local network:
  - install the produced module (`icp canister install poker_backend --wasm out/poker_backend.wasm.gz --mode reinstall` on local only, or the icp-cli equivalent; check `icp canister install --help`);
  - read `icp canister status poker_backend`;
  - confirm the module hash equals the published hash;
  - deal a hand (or run the PocketIC suite against this module through the integration-test script's `BACKEND_WASM`) to confirm the module works.

  Expected: the hashes are equal and the tests pass. If the local network isn't running, use the PocketIC suite. It can install a given wasm and read `module_hash` through `canister_status`, so add that assertion to an integration test.
- [ ] **Step 4: Run** the full `cargo test`, and run the PocketIC suite with the reproducible module. Expected: everything passes.
- [ ] **Step 5: Commit** "Build the game canister reproducibly".

### Task 2: CI reproducibility check and release workflow

**Files:**
- Modify: `.github/workflows/ci.yml`, adding a `reproducible` job.
- Create: `.github/workflows/release.yml`.

**Interfaces:**
- **The `reproducible` job:** runs `scripts/check-reproducible.sh` on every push and PR. It is read-only.
- **`release.yml`:**
  - runs on push of tags matching `v*`, with `permissions: contents: write`;
  - checks out the tag, runs the check, then builds once more into `out/`;
  - creates the Release with `gh release create <tag>`. It is not a draft and not a pre-release, unless the tag contains `-`, which makes it a pre-release. It uploads the three files.
  - The release body includes:
    - the exact machine line `poker_backend.wasm.gz sha256: <hex>`;
    - the commit SHA;
    - the toolchain pins;
    - the reproduce steps (`git checkout <tag> && scripts/reproducible-build.sh`).
  - It fails if the tag doesn't point at a commit on `main`.

- [ ] **Step 1: Write both workflows.** Validate the YAML with `python3 -c 'import yaml…'` in WSL. Lint with `actionlint` if available, but don't install it without asking.
- [ ] **Step 2: Dry-run the release steps locally:** run the same commands with `gh release create --draft` replaced by an echo, and check that the body renders with a correct hash line. Don't publish a real release.
- [ ] **Step 3: Commit** "Check reproducibility in CI and publish releases from tags".

### Task 3: Release-only deploy script

**Files:**
- Create: `scripts/deploy-release.sh`.
- Modify: `docs/reproducible-builds.md`, adding the deploy section.

**Interfaces:**
- **`scripts/deploy-release.sh <tag> [--network ic]`:**
  1. Refuse a draft or pre-release (`gh release view <tag> --json isDraft,isPrerelease`).
  2. Download the assets to a temporary directory.
  3. Check the SHA-256 against both the `.sha256` asset and the body line.
  4. Show the current live module hash (from `icp canister status poker_backend -e ic`) next to the new one.
  5. Require a typed confirmation ("deploy v1.4.0").
  6. Install in **upgrade** mode only, with the same upgrade arguments `icp deploy -e ic` uses today (check `icp.yaml` and the backend's `post_upgrade`; `InitArgs` must not be resent if that would reset anything).
  7. Read the module hash back and compare it with the release.
  8. Exit non-zero on any mismatch, with a clear message.
- **Rules:**
  - It never builds.
  - It never accepts a local file.
  - It never runs in CI. Refuse when `CI=true`.

- [ ] **Step 1: Write the script.** Test every refusal path against the local network or with stubbed commands: a wrong hash, a missing asset, a draft, `CI=true`, and a declined confirmation. Also test one real local upgrade with a module built by Task 1, using a locally faked release directory behind a `--from-dir` test flag that's documented as for testing only and refused with `--network ic`.
- [ ] **Step 2: Commit** "Deploy only published releases".

### Task 4: Build badge in the app

**Files:**
- Create:
  - `src/poker_frontend/src/buildCheck.ts` and `buildCheck.test.ts`, with the pure logic.
  - `src/poker_frontend/src/components/BuildBadge.tsx`.
- Modify:
  - `src/poker_frontend/src/App.tsx`: the footer.
  - `src/poker_frontend/public/_headers`: CSP `connect-src` gains `https://api.github.com`.
  - `styles.css`.

**Interfaces:**
- **`buildCheck.ts`:**
  - `parseReleases(json: unknown): Release[]`. Each `Release` is `{ tag, published_at, hash, assetDigest?, url }`. It skips drafts and pre-releases, and releases with no valid hash line. If the asset digest and the body hash disagree, the release is marked `conflict` and never counts as a match.
  - `classify(liveHash: Uint8Array | null, releases: Release[], network: 'ic' | 'local'): BadgeState`. The result is one of:
    - `{ kind: 'latest', tag }`
    - `{ kind: 'older', tag, newest }`
    - `{ kind: 'none' }`
    - `{ kind: 'unknown', reason }`
    - `{ kind: 'local' }`
  - `controllersText(principals: string[]): string`.
- **`BuildBadge`:**
  - Reads the live module hash and controllers with `@icp-sdk/core`'s canister-status `read_state` support (find it in node_modules, e.g. `CanisterStatus.request({ canisterId, agent, paths: ['module_hash', 'controllers'] })`). It uses the app's existing agent, so certificate verification is the agent's.
  - Fetches the releases API and caches the result in `sessionStorage` for 10 minutes, inside try/catch.
  - Rate-limit or network errors give `unknown`.
  - Renders the state, the controllers line and the "Check it yourself" link.

- [ ] **Step 1: Write the failing tests** for `parseReleases` and `classify`:
  - latest
  - older plus newest
  - none
  - drafts and pre-releases ignored
  - a malformed or missing hash line
  - a digest conflict giving no match
  - `null` live hash giving unknown
  - local
- [ ] **Step 2: Run** them. Expected: FAIL.
- [ ] **Step 3: Implement.** Check that the CSP change is the only security-header change.
- [ ] **Step 4: Run** `npx vitest run && npm run build`. Expected: everything passes.
- [ ] **Step 5: Commit** "Show whether the game canister matches a published release".

### Task 5: Documentation and threat model

**Files:**
- Modify:
  - `docs/reproducible-builds.md`: finish it with how to reproduce, how to release, how to deploy, and what the badge does and doesn't prove.
  - `docs/provably-fair-threat-model.md`: threat 6 (dealer choosing among seeds) and threat 8 (upgrades), plus the future-work section, which marks verifiable builds as done.
  - `README.md`: a short section linking the doc.

**Content (binding):**
- The badge is shown by the operator's website, so the strongest check is to rebuild yourself and compare with the IC's module hash (`icp canister info` or the dashboard). Give the exact commands.
- A matching hash proves the running code is the published code. It doesn't prove the code is honest (that needs review), and it doesn't stop future upgrades. The controllers line shows who can upgrade.
- Threat 6 becomes: residual, but code changes are detectable by anyone checking the module hash against releases. A malicious release would itself be public and reviewable.

- [ ] **Step 1: Write the docs.** Check every command against Tasks 1–4.
- [ ] **Step 2: Commit** "Document reproducible builds and update the threat model".
