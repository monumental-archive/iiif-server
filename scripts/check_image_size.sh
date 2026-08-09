#!/bin/sh
# SPDX-FileCopyrightText: 2026 Carl Allen
# SPDX-License-Identifier: AGPL-3.0-only

# Assert the container image is still small enough for the claim the README
# makes about it.
#
# README.md and docs/bench/cantaloupe-eval.md compare this image against the
# incumbent's 769 MB. A comparison nothing checks is a number that rots — a
# dependency bump that quietly links in ten more megabytes would invalidate a
# published claim without turning anything red. This makes that a build
# failure instead.
#
# The ceiling is deliberately loose: today's image is about 15.6 MB, so 25 MB
# leaves room for honest growth (a toolchain bump, another codec) while still
# catching a regression large enough to make "one static binary" a lie.
# Raising it is allowed. Raising it silently is not, and that is the point.
#
# Measured with `docker export` — the flattened, uncompressed root filesystem.
# Deliberately not `docker image inspect --format '{{.Size}}'`: that field is
# the unpacked layer total under the classic image store and the *compressed*
# blob total under the containerd store, so it would mean different things on
# different runners. `docker export` means one thing everywhere. It includes
# the few kilobytes of scaffolding container creation writes (/proc, /sys,
# /etc/mtab), which is noise against a ceiling in the tens of megabytes.
#
# The compressed pull size, which is what the README quotes, is roughly 40% of
# this and is not asserted: it depends on registry-side layer compression, so
# it cannot be measured from a local image without lying about the method.
# This number bounds it.
#
# usage: scripts/check_image_size.sh [IMAGE_REF] [CEILING_MB]
set -eu

image=${1:-iiif-server:local}
ceiling_mb=${2:-25}
ceiling=$((ceiling_mb * 1000 * 1000))

container=""
tarball=$(mktemp)

cleanup() {
  [ -z "${container}" ] || docker rm -f "${container}" > /dev/null 2>&1 || true
  rm -f "${tarball}"
}
trap cleanup EXIT

fail() {
  echo "FAIL: $*" >&2
  exit 1
}

container=$(docker create "${image}") || fail "could not create a container from ${image}"
docker export "${container}" > "${tarball}" || fail "docker export ${image} failed"

bytes=$(wc -c < "${tarball}")
bytes=$((bytes))
[ "${bytes}" -gt 0 ] || fail "exported filesystem was empty"

echo "--- ${image}: uncompressed image size"
awk -v b="${bytes}" -v c="${ceiling}" 'BEGIN {
  printf "    %d bytes (%.1f MB, %.1f MiB), ceiling %.0f MB, %.0f%% of it used\n",
    b, b / 1000000, b / 1048576, c / 1000000, 100 * b / c
}'

if [ "${bytes}" -gt "${ceiling}" ]; then
  echo "" >&2
  echo "    \`docker history ${image}\` shows which layer grew. If the growth is" >&2
  echo "    intended, raise the ceiling here and update the size figures in" >&2
  echo "    README.md, docs/deployment.md and docs/bench/cantaloupe-eval.md in" >&2
  echo "    the same change — they are the claims this gate exists to protect." >&2
  echo "" >&2
  fail "${image} exceeds the ${ceiling_mb} MB ceiling"
fi

echo "PASS: ${image} is under the ${ceiling_mb} MB ceiling"
