# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working
with code in this repository.

## What this is

A complete, scope-frozen implementation of the IIIF Image API (3.0 + 2.1,
level 2 plus all optional features) as one static Rust binary. Pure Rust
everywhere untrusted input is parsed — zero C in the product (dev-time
fixture generation is the only exemption). AGPL-3.0-only; contributions
are signed off under the DCO, and there is no contributor agreement.

The feature surface is deliberately finished:
[docs/design-spec.md](docs/design-spec.md) lists pre-refusals (AVIF/JXL,
auth, Presentation API, per-image metadata) that are declined in advance.
Do not add features; correctness, security, docs, and test coverage are
the welcome categories.

## Governance: this repository conforms, it does not decide

`monumental-archive/.github` is the org's conformance root and the only
authority on tooling, CI, releases and settings. Its CLAUDE.md,
`docs/migration-playbook.md`, `docs/release.md` and `scaffold/` are the
shape; this repository carries the smallest possible footprint of it and
adapts to the canon, never the reverse.

Concretely, and this is the thing to internalise before changing
anything here:

- **There is no repo CI.** `.github/workflows/ci.yml` is a six-line
  caller of the shared org gate, pinned by SHA. The linters, their
  versions and their settings arrive with that pin.
- **There is no repo lint config.** clippy, rustfmt, typos, yamllint,
  rumdl and the rest are configured once in the canon's `mise/` and
  delivered to the tool at run time (`ORG_BELT_DIR`). A repo-local
  `clippy.toml` is refused outright by `lint:rust`. Do not reintroduce
  one.
- **There is no task runner but mise.** `mise run ci` locally is exactly
  what CI runs — same tools, same versions, same order, from the same
  lockfile.
- **Releases are the canon's**, phase 1 (`release.yml`) and phase 2
  (`publish.yml`) both calling shared workflows. Merging the Release PR
  is the commitment point. Nothing here builds, signs or publishes.

`mise.toml` holds only what is genuinely this repository's: the Rust
toolchains, the build inputs the artifact classes assert, and the tasks
below.

## Commands

`mise install` once per clone, then `mise run hooks:install`.

- `mise run ci` — the whole gate: every org `lint:*`, this repo's
  `lint:*`, `test`, and the coverage ratchet
- `mise run test` — the test suite (`cargo test --workspace
  --all-features --locked`)
- `cargo test -p iiif-core --test <name> <filter>` — one test file
- `mise run lint:docs` — rustdoc with warnings as errors, plus doc tests
- `mise run fix` — every write-mode fixer (rustfmt, taplo, shfmt, rumdl)
- `mise run coverage:check` — the committed `.coverage-floor` ratchet
- `mise run audit:fuzz` — the sanitized fuzz run (cron; needs the dated
  nightly pinned in `mise.toml`)
- `mise run audit:iiif-validate` — the official IIIF validators against a
  built image
- `mise run audit:deny` / `audit:links` — advisory feed, link liveness

Nothing is fingerprinted any more: the gate is cheap enough to run whole,
and a cache that decides what to skip is a second opinion about what
changed.

## Ground rules

- **No lint exemptions, ever — and the exception mechanism is
  `#[expect(..., reason = "...")]` and nothing else.** The org runs
  clippy at every group including `restriction`, minus nine named
  mechanical contradictions, with `-D warnings`. A crate-level
  `#![allow(clippy::<group>)]` is a hard error in the gate: it would
  silence every level the task sets and still exit 0.
- **Main is protected; all changes land by PR.** No direct pushes. The PR
  title becomes the permanent squash subject and is held to the commit
  canon — conventional, imperative, lowercase, 72 columns.
- **Every commit is signed off** (`git commit -s`); `lint:dco` refuses a
  commit whose sign-off does not match its author.
- **No closing keywords next to cross-repo references** in PR/commit
  prose ("fixes owner/repo#N" auto-closes the other repo's issue on
  merge). Use descriptive wording.
- **Dependencies must survive cargo-deny**: permissive licences only,
  wildcard bans, reasoned skips for duplicate versions. `deny.toml` is
  repo content — its skips describe this tree.
- The fuzz workspace (`fuzz/`) is excluded from the main workspace and
  carries its own committed `Cargo.lock`. `lint:fuzz-build` compiles the
  targets on stable in the gate; `audit:fuzz` runs them under
  AddressSanitizer on the dated nightly, on the Monday cron.
- **A new Rust or shell file carries the same two-line SPDX header every
  other one carries** — `SPDX-FileCopyrightText` and
  `SPDX-License-Identifier: AGPL-3.0-only`, above the `//!` docs in Rust
  and below the shebang in shell. Copy it from a neighbour. Every other
  file type is covered by the blanket entry in `REUSE.toml` and needs
  nothing; `lint:reuse` proves it.
- **The Dockerfile does not compile anything.** The binary is built by
  `scripts/oci-prepare.sh` in the mise-pinned toolchain and COPYed in
  (`.github#295`): the org's repro gate measured the in-container cargo
  build nondeterministic while the same crates built bit-for-bit
  natively. Keep the Dockerfile pure assembly over digest-pinned inputs.

## Architecture

Three workspace crates:

- **`crates/core` (iiif-core)** — everything spec-shaped and
  pixel-shaped: URL grammar parsing/printing
  ([grammar.rs](crates/core/src/grammar.rs), property-tested for
  round-trips and canonicalization), request evaluation
  ([eval.rs](crates/core/src/eval.rs)), the decode→transform→encode
  pipeline ([pipeline.rs](crates/core/src/pipeline.rs)), codec
  integrations under `codec/`, info.json generation
  ([info.rs](crates/core/src/info.rs)), and the 2.1 compatibility layer
  ([v2.rs](crates/core/src/v2.rs)). The `Source` abstraction
  ([source.rs](crates/core/src/source.rs)) is the seam to storage.
- **`crates/sources` (iiif-sources)** — storage backends: local
  filesystem and S3-compatible object stores (range reads; GCS/Azure work
  by S3 compatibility).
- **`crates/server` (iiif-server)** — the HTTP layer on hyper 1.x
  directly: routing/headers/conneg
  ([app.rs](crates/server/src/app.rs)), bounded decode pool with 503 +
  Retry-After backpressure, `/healthz` and Prometheus `/metrics`, and the
  CLI (`serve`, `check`, `healthcheck`).

Correctness is enforced three ways and new work should slot into them:
the official IIIF validators against the built image, golden/differential
tests pinning pixels against libvips/libjpeg/OpenJPEG (masters are
generated, gitignored, digest-verified — `scripts/gen_fixtures.sh`), and
property tests over the grammar. Correctness fixes want a failing test
first.

Performance-sensitive decode paths (notably JPEG 2000 via the `j2k`
crate) have benchmark harnesses under `tools/bench/` and
`scripts/bench_libvips.sh`; numbers get recorded in `docs/`, raw spike
output is gitignored.

## What is published

One artifact class: the container image, `ghcr.io/monumental-archive/
iiif-server`, built per architecture on native hardware, smoke-tested
before and after publication, signed and attested through the org signer.
The workspace crates are `publish = false` and no standalone binary is
distributed — declaring a second class would mean owing evidence for an
artifact nobody pulls.
