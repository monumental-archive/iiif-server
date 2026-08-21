# Changelog

All notable changes to this project are recorded here.

<!-- rumdl-disable MD013 -->
<!-- entries are commit subjects, verbatim: a recorded subject's
length is a fact about history, not prose to reflow -->

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
over the surface named in [MAINTENANCE.md](MAINTENANCE.md) — the HTTP API, the
CLI flags and the container contract. Internal Rust APIs are not covered:
nothing in this workspace is published to a registry.

## [0.2.1](https://github.com/monumental-archive/iiif-server/compare/v0.2.0...v0.2.1) - 2026-08-21

### Fixed

- update http-body-util to 0.1 ([#126](https://github.com/monumental-archive/iiif-server/pull/126))
- declare the image binary and pin canon v1.58.1 ([#128](https://github.com/monumental-archive/iiif-server/pull/128))

### Dependencies

- update alpine to v3.24 ([#130](https://github.com/monumental-archive/iiif-server/pull/130))

## [0.2.0](https://github.com/monumental-archive/iiif-server/compare/v0.1.0...v0.2.0) - 2026-08-21

### Added

- cooldown 14 days -> 7 (mise leg, three-layer policy) ([#79](https://github.com/monumental-archive/iiif-server/pull/79))
- add CodeQL SAST workflow (rust + actions, build-mode none) ([#97](https://github.com/monumental-archive/iiif-server/pull/97))
- run OpenSSF Scorecard weekly and publish the result ([#99](https://github.com/monumental-archive/iiif-server/pull/99))

### Fixed

- commit fuzz/Cargo.lock and gate the harness --locked ([#84](https://github.com/monumental-archive/iiif-server/pull/84))
- changelog template emits malformed markdown from the second release on ([#96](https://github.com/monumental-archive/iiif-server/pull/96))
- unjam the mise-action comments in scheduled.yml ([#100](https://github.com/monumental-archive/iiif-server/pull/100))
- derive egress allowlists for codeql and scorecard, switch to block ([#108](https://github.com/monumental-archive/iiif-server/pull/108))
- restore egress-policy block across the release path from audit data ([#110](https://github.com/monumental-archive/iiif-server/pull/110))

### Documentation

- add CLAUDE.md (agent guidance: commands, ground rules, architecture) ([#85](https://github.com/monumental-archive/iiif-server/pull/85))
- Best Practices doc pack (CoC, governance, roadmap, assurance case) ([#98](https://github.com/monumental-archive/iiif-server/pull/98))
- add the README badge row now that each one is backed ([#101](https://github.com/monumental-archive/iiif-server/pull/101))
- fix two stale references ([#109](https://github.com/monumental-archive/iiif-server/pull/109))
- the licensing pass — AGPL FAQ, plus the task ci correction ([#112](https://github.com/monumental-archive/iiif-server/pull/112))
- state the Branch-Protection ceiling; unbreak lychee on the AGPL text ([#114](https://github.com/monumental-archive/iiif-server/pull/114))

### CI

- assert the image size so the under-25MB claim cannot rot ([#113](https://github.com/monumental-archive/iiif-server/pull/113))
- conform to the org gate and declare the oci-image class ([#119](https://github.com/monumental-archive/iiif-server/pull/119))

### Dependencies

- update eclipse-temurin:25-jre-noble docker digest to fbcf915 ([#86](https://github.com/monumental-archive/iiif-server/pull/86))
- sweep every pin to current — actions, mise tools, both lockfiles ([#94](https://github.com/monumental-archive/iiif-server/pull/94))
- update dependency jdx/mise to v2026.8.0 ([#116](https://github.com/monumental-archive/iiif-server/pull/116))
- update mise tools ([#117](https://github.com/monumental-archive/iiif-server/pull/117))
- update docker/dockerfile to ecfaec9 ([#121](https://github.com/monumental-archive/iiif-server/pull/121))

## [0.1.0](https://github.com/monumental-archive/iiif-server/releases/tag/v0.1.0) - 2026-08-01

### Added

- workspace skeleton — typed grammar, identifier rules, source seam
- the first pixel path — pyramid TIFF to HTTP tile
- official IIIF validator wired from git — v3 level 2 green, 33/33
- JPEG-in-TIFF verified — and it caught a real color bug
- j2k vs OpenJPEG — bit-exact, fast, Plan B stays shelved
- object-store range-read profile vs MinIO
- M1 Link headers + HTTP semantics tests; mimalloc ships (bench-decided)
- the codec seam — JP2/HTJ2K serving, plain JPEG/PNG masters
- the v2.1 endpoint — both official validators green
- the completionist sweep — full compliance table shipped
- HTTP caching correctness — ETags, conditionals, Cache-Control
- /metrics — the frozen observability surface
- object-store serving — s3:// roots, pixel-verified e2e
- iiif-server check — offline master inspection
- fuzzing — and it found a real 25 GB decompression bomb
- adopt shared preset (github>CarlAllenn/renovate-config) (#17) ([#17](https://github.com/monumental-archive/iiif-server/pull/17))
- every tile grid takes the region fast path (#22) ([#22](https://github.com/monumental-archive/iiif-server/pull/22))
- task baseline de-drift — ci umbrella, per-tool lint tasks, fingerprint every gate (#24) ([#24](https://github.com/monumental-archive/iiif-server/pull/24))
- lefthook baseline — universal lint layer consumed live via remotes (#27) ([#27](https://github.com/monumental-archive/iiif-server/pull/27))
- adopt cargo-deny supply-chain baseline (renovate-config#5) (#30) ([#30](https://github.com/monumental-archive/iiif-server/pull/30))
- harden-runner block mode (renovate-config#6) (#31) ([#31](https://github.com/monumental-archive/iiif-server/pull/31))
- scanner baseline — gitleaks, machete, lychee (#7) (#32) ([#32](https://github.com/monumental-archive/iiif-server/pull/32))
- #8 tooling baseline — full lint canon, rustfmt, fuzz CI, SBOM (#34) ([#34](https://github.com/monumental-archive/iiif-server/pull/34))
- JP2 zoom-outs decode to the ladder's full depth, not the 1/8 cap (#40) ([#40](https://github.com/monumental-archive/iiif-server/pull/40))
- official container image, release pipeline, and installer (#49) ([#49](https://github.com/monumental-archive/iiif-server/pull/49))

### CI

- lint+test on x86-64 and arm64, digest-pinned, MSRV job
- validator job name reflects that both API versions run

### Dependencies

- pin dependencies (#6) ([#6](https://github.com/monumental-archive/iiif-server/pull/6))
- update actions/upload-artifact digest to 043fb46 (#7) ([#7](https://github.com/monumental-archive/iiif-server/pull/7))
- update dependency aqua:taiki-e/cargo-llvm-cov to v0.8.7 (#35) ([#35](https://github.com/monumental-archive/iiif-server/pull/35))

### Documentation

- founding design spec from scoping session
- amend founding spec — full dependency evaluation, public-from-start
- license decision — AGPL-3.0-only, CLA for external contributions
- close M0 ambiguities — CLA mechanism, test-fixture provenance
- AGPL-3.0-only license + Apache-ICLA-derived CLA
- reflow prose to 120 columns for markdownlint
- session build report
- record the filed upstream issue and the fast-path-only caveat (#5) ([#5](https://github.com/monumental-archive/iiif-server/pull/5))
- head-to-head eval vs Cantaloupe — the partial-grid numbers (#11) ([#11](https://github.com/monumental-archive/iiif-server/pull/11))
- freshness pass — three catches not one, gate revisit happened, j2k bug filed (#16) ([#16](https://github.com/monumental-archive/iiif-server/pull/16))
- drop stale fallback-path wording in eval corpus comment (#23) ([#23](https://github.com/monumental-archive/iiif-server/pull/23))
- ci badge in README (#26) ([#26](https://github.com/monumental-archive/iiif-server/pull/26))
- eval rerun after the decode-stack changes; sharpen the HTJ2K claim (#42) ([#42](https://github.com/monumental-archive/iiif-server/pull/42))

### Fixed

- conda fixture tools get their own config root
- validator script installs fixture tools explicitly
- never fingerprint lint:deny — RustSec DB is an unseen input; weekly scheduled gate (#25) ([#25](https://github.com/monumental-archive/iiif-server/pull/25))
- drop en-GB_to_en-US dictionary — the house dialect is en-GB (#29) ([#29](https://github.com/monumental-archive/iiif-server/pull/29))
- taplo-stable comment style in [tools] — Renovate bumps broke alignment (#36) ([#36](https://github.com/monumental-archive/iiif-server/pull/36))
- resource-ceiling refusals answer 403, not 500 corrupt master (#12) (#39) ([#39](https://github.com/monumental-archive/iiif-server/pull/39))
- sign the Release PR commit via the GitHub API (#51) ([#51](https://github.com/monumental-archive/iiif-server/pull/51))
- lowercase the image name; reset for a clean 0.1.0 (#54) ([#54](https://github.com/monumental-archive/iiif-server/pull/54))
- audit egress for one run; drop unused tool installs (#56) ([#56](https://github.com/monumental-archive/iiif-server/pull/56))
- scan via mise-pinned trivy on PRs; cosign from mise (#58) ([#58](https://github.com/monumental-archive/iiif-server/pull/58))
- real conformance evidence, enforced egress (#60) ([#60](https://github.com/monumental-archive/iiif-server/pull/60))
- export STAGING to the embedded python (#61) ([#61](https://github.com/monumental-archive/iiif-server/pull/61))
- use the observed endpoints, not a filtered version (#63) ([#63](https://github.com/monumental-archive/iiif-server/pull/63))
- use the complete observed endpoint list (#65) ([#65](https://github.com/monumental-archive/iiif-server/pull/65))
- audit egress everywhere; binaries job mise hosts (#67) ([#67](https://github.com/monumental-archive/iiif-server/pull/67))

### Miscellaneous

- tooling standup — pinned, locked, max enforcement
- port the proven pieces from edtf and monumental-archive (#9) ([#9](https://github.com/monumental-archive/iiif-server/pull/9))
- rust toolchain surfaces as a PR, not a dashboard line (#10) ([#10](https://github.com/monumental-archive/iiif-server/pull/10))
- adopt max-enforcement linter baseline from renovate-config (#28) ([#28](https://github.com/monumental-archive/iiif-server/pull/28))
- de-drift the mise settings block to canon (#43) ([#43](https://github.com/monumental-archive/iiif-server/pull/43))

### Performance

- adaptive codec parallelism; M2 libvips gate measured honestly
