# Pinned build environment for the reproducible poker_backend build.
# Used by scripts/reproducible-build.sh. Everything that can change the output
# is pinned: the base image by digest (it fixes rustc/cargo 1.94.1 and gzip
# 1.12), the wasm32 standard library by the toolchain version, and ic-wasm by
# the SHA-256 of its release tarball.

# rust:1.94.1-slim-bookworm, linux/amd64.
ARG BASE=rust@sha256:cf9dd0ec73e75f827fe59123fff9dc65af1a1c8363c3c31ee8d7f8ad0b6a5fb2

# Throwaway stage: unpack the checksum-verified ic-wasm release. The packages
# installed here (xz-utils) never reach the final image.
FROM --platform=linux/amd64 ${BASE} AS ic-wasm
RUN apt-get update && apt-get install -y --no-install-recommends xz-utils && rm -rf /var/lib/apt/lists/*
# ic-wasm 0.11.1, statically linked (musl).
ADD --checksum=sha256:86d784f95c74284d8748aec6ebc6c15d57992dae74b53902f64cc01b727cd2be \
    https://github.com/dfinity/ic-wasm/releases/download/0.11.1/ic-wasm-x86_64-unknown-linux-musl.tar.xz /tmp/ic-wasm.tar.xz
RUN mkdir /tmp/ic-wasm && tar -xJf /tmp/ic-wasm.tar.xz -C /tmp/ic-wasm --strip-components=1 \
    && install -m 0755 /tmp/ic-wasm/ic-wasm /usr/local/bin/ic-wasm \
    && ic-wasm --version

FROM --platform=linux/amd64 ${BASE}
# rust-toolchain.toml says "stable"; this pins the exact toolchain instead.
ENV RUSTUP_TOOLCHAIN=1.94.1
RUN rustup target add --toolchain 1.94.1 wasm32-unknown-unknown
COPY --from=ic-wasm /usr/local/bin/ic-wasm /usr/local/bin/ic-wasm
