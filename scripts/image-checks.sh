#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Carl Allen
# SPDX-License-Identifier: AGPL-3.0-only

# The oci-image class's smoke test: everything this repository asserts
# about a built image, in one executable, so the class runs all of it at
# both of the moments it runs a smoke test — on the loaded image before
# anything is published, and again on the bytes pulled back from the
# registry by digest.
#
# Receives the image reference as $1 (a local tag, then a registry
# digest), which is the whole reason both assertions live here: the size
# ceiling and the serving behaviour are claims about the image a
# stranger pulls, and only the second invocation can prove that.
set -euo pipefail

image="${1:?image reference required}"
here=$(cd "$(dirname "$0")" && pwd)

# Does it serve IIIF, report its version, answer its own healthcheck and
# stop on SIGTERM — under the hardened flags docs/deployment.md tells
# operators to use.
"${here}/smoke_image.sh" "${image}"

# The under-25MB claim README.md and docs/bench/cantaloupe-eval.md make
# against the incumbent's 769 MB (their #113). A comparison nothing
# checks is a number that rots.
"${here}/check_image_size.sh" "${image}"

printf 'image-checks: ok — %s serves IIIF and is under its size ceiling\n' "${image}"
