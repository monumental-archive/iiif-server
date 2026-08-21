# iiif-server (working name)

[![gate](https://github.com/monumental-archive/iiif-server/actions/workflows/ci.yml/badge.svg)](https://github.com/monumental-archive/iiif-server/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/monumental-archive/iiif-server/badge)](https://scorecard.dev/viewer/?uri=github.com/monumental-archive/iiif-server)
[![release](https://img.shields.io/github/v/release/monumental-archive/iiif-server)](https://github.com/monumental-archive/iiif-server/releases/latest)
[![image size](https://img.shields.io/badge/image-%3C25%20MB-blue)](scripts/check_image_size.sh)
[![REUSE status](https://api.reuse.software/badge/github.com/monumental-archive/iiif-server)](https://api.reuse.software/info/github.com/monumental-archive/iiif-server)
[![license](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](LICENSE)

A complete, correct, boring implementation of the [IIIF Image
API](https://iiif.io/api/image/3.0/) — **3.0 and 2.1, level 2 plus the
entire optional feature table** — as one static binary. Pure Rust
including JPEG 2000/HTJ2K decode; **zero C parses untrusted input
anywhere in the product**; stateless; scope-frozen at 1.0.

```bash
docker run --rm -p 6363:6363 -v ./masters:/imageroot:ro \
    ghcr.io/monumental-archive/iiif-server
```

```bash
iiif-server serve s3://bucket/prefix --endpoint https://objects.example.com
```

The image is one static binary and a certificate bundle on nothing else
— no shell, no package manager, no distro — about 6 MB to pull and 16–18
MB unpacked depending on architecture, against the incumbent's 769 MB.
That comparison is a gate, not a boast: every release builds the image
per architecture and refuses to publish one over 25 MB unpacked, on the
bytes pulled back from the registry as well as the local build.

**The image is the whole distribution.** Standalone binaries and the
installer script are not published: the released artifact is the
container image, signed with build provenance you can verify
([SECURITY.md](SECURITY.md)). Recipes, including a hardened compose file,
are in [docs/deployment.md](docs/deployment.md).

That is the whole configuration story: one root, numeric limits, pool
sizing. No properties file, no feature toggles — capability is baked in
and info.json is generated from what the binary actually does, identical
for every image.

## What ships

| Surface | Status |
| --- | --- |
| Image API 3.0, level 2 + all optional features | official validator: 33/33 (run on demand, see below) |
| Image API 2.1, all 18 named features | official validator: 30/30 (run on demand, see below) |
| Regions: `full`, `square`, px, `pct:` | complete |
| Sizes incl. every `^` upscaling form | complete |
| Rotation: 90° steps, mirroring, arbitrary angles | complete (transparent corners on PNG/WebP) |
| Qualities: `default`, `color`, `gray`, `bitonal` | complete |
| Outputs: `jpg png tif gif jp2 pdf webp` | complete (webp lossless-only — the one asterisk¹) |
| Sources: pyramidal/tiled TIFF (incl. JPEG-in-TIFF), JP2 + HTJ2K, plain JPEG/PNG | complete |
| Local filesystem + S3-compatible object stores | complete (GCS/Azure by construction) |
| ETags, conditionals, CORS, content negotiation, canonical links | complete |
| Bounded decode pool with honest backpressure (503 + Retry-After) | complete |
| `/healthz`, Prometheus `/metrics` | complete |
| `iiif-server check` — offline master inspection with copy-paste fixes | complete |

¹ Lossy webp requires C libwebp; valid `image/webp` is served losslessly
instead, at larger byte sizes. That is the compliance table's single
footnote.

## Why it stays finished

The spec surface is frozen (3.0 unchanged since 2020, 2.1 since 2016;
codecs are frozen file formats), so *complete* is reachable — and after
1.0 the feature set never grows. What remains forever is a nine-crate
tracked dependency class, all pure Rust, handled as routine bumps. The
full doctrine, response window, and **pre-refusals** (AVIF/JXL, auth,
Presentation API, per-image metadata — declined in advance, with
rationale) live in [MAINTENANCE.md](MAINTENANCE.md).

Correctness is enforced three ways: the **official IIIF validators**
(`mise run audit:iiif-validate`, run against a built image, both API
versions — **on demand, not on a schedule**, which is why the
conformance figures above wear no shield: nothing runs them
automatically yet, and a shield over an unrun check is a claim with no
mechanism), **golden/differential tests** pin pixels against libvips,
libjpeg, and
OpenJPEG — bit-exact where the math says bit-exact — and **property
tests** cover the grammar (parse↔print round-trips, canonicalization,
totality). The differential/fuzz rig has caught and contained three
real defects before any user could hit them — two upstream decoder
bugs and a 25 GB decompression bomb
([session report](docs/session-report-2026-07-26.md)).

How this stacks up against the incumbent server — latency,
conformance, and ops, measured with the same no-subtractions
methodology and including where the incumbent wins — is in
[docs/bench/cantaloupe-eval.md](docs/bench/cantaloupe-eval.md).

## Building and developing

Toolchain is pinned with [mise](https://mise.jdx.dev); MSRV is Rust 1.96.
CI, linters and release machinery are the organisation's, consumed from
[monumental-archive/.github](https://github.com/monumental-archive/.github)
— this repository carries a six-line gate caller and nothing else.

```bash
mise install && mise run hooks:install
mise run ci                  # exactly what CI runs, same tools, same order
mise run audit:iiif-validate # official IIIF validators against an image
```

The workspace is `#![forbid(unsafe_code)]` throughout, clippy runs at
every group including `restriction` with `-D warnings` and no `allow`
attributes, and every dependency is permissively licensed (enforced by
`cargo deny`). See
[CONTRIBUTING.md](CONTRIBUTING.md) (contributions are signed off under
the DCO) and [docs/design-spec.md](docs/design-spec.md) — the founding
document this build follows.

Also: [GOVERNANCE.md](GOVERNANCE.md) (how decisions get made, and what
happens if the maintainer disappears),
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md),
[docs/roadmap.md](docs/roadmap.md) (what this will and will not do), and
[docs/assurance-case.md](docs/assurance-case.md) — the threat model, trust
boundaries and the argument that the security requirements in
[SECURITY.md](SECURITY.md) actually hold.

Deployment recipes (CDN caching, forward-auth, systemd):
[docs/deployment.md](docs/deployment.md).

## Status

The founding spec's engineering milestones are built and continuously
verified, with one exception recorded honestly: ICC colour management (M2,
via `moxcms`) is not implemented
([#45](https://github.com/monumental-archive/iiif-server/issues/45)).

Releases are signed and published through the organisation's release
path: a versioned, multi-architecture image, built per architecture on
native hardware, proved against the bytes pulled back from the registry,
signed and attested by the org signer. **Product naming and the first
announcement remain deferred to the launch milestone**; publishing under
the working name is deliberate, because a GHCR path can be renamed later
at the cost of one line in a consumer's compose file, whereas the
repository that builds and signs the artifacts is what a verification
policy actually names. Nothing is published to crates.io, permanently —
the reasoning is in
[docs/release-engineering.md](docs/release-engineering.md).

## Licensing

Licensed [AGPL-3.0-only](LICENSE). Running it — including as a public
service, at any scale — obliges you to publish nothing. The network clause
applies only if you *modify* the server and offer your modified version
over a network, and then only to the users of that service.

It does not reach your viewer, manifests or discovery layer: those talk to
this server over HTTP as separate processes, so they keep whatever licences
they already have, proprietary included. That is the question most AGPL
hesitancy turns out to be about.

Alternative terms: ask. The contributor agreement that used to carry a
blanket relicensing grant is withdrawn — contributions are signed off
under the DCO now, and a sign-off grants nothing beyond the AGPL — so
what can be offered depends on whose code is involved.
[docs/licensing.md](docs/licensing.md) sets out both directions.

The full FAQ, including why AGPL rather than a permissive licence, is
[docs/licensing.md](docs/licensing.md).
