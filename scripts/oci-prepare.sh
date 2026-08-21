#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Carl Allen
# SPDX-License-Identifier: AGPL-3.0-only

# The oci-image class's prepare script (.github#295): build the binary
# OUTSIDE the image, in this repository's mise-pinned toolchain, so the
# Dockerfile stays pure assembly. The org's repro gate measured the
# in-container cargo build nondeterministic while the same crates built
# bit-for-bit under this exact toolchain — the compile belongs where
# determinism is already proven.
#
# musl-static so the runtime stage can be `scratch`: no base userland,
# nothing to triage, nothing left in the image that can vary.
#
# Runs identically on both repro-gate legs; receives the architecture as
# $1 from build-oci-image.yml, which also supplies the reproducibility
# environment (SOURCE_DATE_EPOCH, CARGO_INCREMENTAL=0,
# remap-path-prefix, strip preserved for cargo-auditable).
set -euo pipefail

arch="${1:?architecture required (amd64|arm64)}"
case "${arch}" in
  amd64) target=x86_64-unknown-linux-musl ;;
  arm64) target=aarch64-unknown-linux-musl ;;
  *)
    echo "::error::oci-prepare: unknown architecture ${arch}" >&2
    exit 1
    ;;
esac

rustup target add "${target}"
# mimalloc is a real C dependency and ships by default (the allocator
# bench decided it: musl's malloc serialises the concurrent decode
# workload). Building it for a musl target needs musl-gcc, so the
# toolchain is installed unconditionally — identical environments beat
# conditional ones on a path that gets signed.
sudo apt-get update -qq
sudo apt-get install -y -qq musl-tools

# cargo-auditable is a build input like the toolchain itself, pinned in
# this repository's mise.toml — asserted, never installed here; an
# unpinned install on a release leg is a runner mutation.
command -v cargo-auditable > /dev/null || {
  echo "::error::cargo-auditable missing — pin cargo:cargo-auditable in mise.toml" >&2
  exit 1
}

# The revision the binary reports through `--version` and the
# iiif_build_info metric, read at compile time by option_env!. Derived
# from the checkout rather than passed in: the class checks out the tag,
# so both repro legs read the same commit and the stamp cannot make two
# builds of one tag differ.
IIIF_BUILD_REVISION=$(git rev-parse --short HEAD)
export IIIF_BUILD_REVISION

# `auditable`, matching every other Rust class: the shipped binary
# carries its dependency tree in the .dep-v0 linker section, so a
# scanner reading the IMAGE sees the Rust surface of the artifact inside
# it. Stripping is already disabled by the class env, which is what
# keeps that section alive.
cargo auditable build --release --locked --target "${target}" --bin iiif-server

# install(1) rather than cp: the mode is asserted, not inherited, so the
# COPY into the image cannot depend on the checkout's umask.
mkdir -p dist
install -m 0755 "target/${target}/release/iiif-server" dist/iiif-server
echo "::notice::oci-prepare: dist/iiif-server built for ${target} at ${IIIF_BUILD_REVISION}"
