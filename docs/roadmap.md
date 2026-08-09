# Roadmap

What this project intends to do, and not do, over the next year. The
feature surface is not on the list: it is finished by design, and the
reasoning for that is in [MAINTENANCE.md](../MAINTENANCE.md) and the
pre-refusals in [design-spec.md](design-spec.md).

This document exists so that "will you add X?" has an answer that does not
depend on catching the maintainer in a particular mood.

## Where the project is

Milestones M0 through M7 are built: the full IIIF Image API 3.0 and 2.1
level-2 compliance tables plus the entire optional feature surface, the
source-format matrix, object-store sources, HTTP caching semantics, the
completionist output sweep, and hardening. The official validators gate
every pull request and run again against the published container image at
release time.

M8 — packaging and launch — is partly done. Signed, attested container
images and static binaries publish from a tagged release. What remains of
M8 is naming, the launch positioning work, and the assurance surface
below.

## Will do

**Supply-chain assurance and provenance.** OpenSSF Scorecard and Best
Practices, REUSE licensing compliance, CodeQL static analysis, and the
badges that make the result legible. Published images need to carry OCI
labels and index annotations, which they currently do not. Release
artifacts need their Sigstore bundles attached so the provenance is
visible to consumers and scanners rather than only to the attestation
store.

**Closing the release loop.** Restoring enforced egress control on every
CI job from the audit data the first release produced, re-enabling tag
immutability, and writing the recovery half of the release runbook — what
to do when a publish fails partway, which is the half that matters and the
half that is currently thin.

**Test coverage to the standard the sibling projects hold.** Statement
coverage is measured on every pull request but nothing acts on it. The
storage layer — the seam where untrusted remote bytes enter — is the
least-tested part of the codebase and will be addressed first, along with
extracting the testable parts of the server binary's entry point so they
can be tested rather than excused.

**Correctness work already identified.** ICC colour management is
specified and not yet implemented. The differential JP2 rig claims three
implementations and currently has two. A robustness corpus mined from the
incumbent's bug history. Deep-ladder zoom-out decode cost on very large
masters is a codec-level follow-up.

**Platform and packaging breadth.** Windows support, which needs Windows
identifier-resolution path semantics first. Broader CI platform coverage
so contributor dev builds are guaranteed.

**Adoption surface.** A licensing FAQ answering the AGPL question before
an evaluator has to ask it, and a public demo instance.

**1.0.** Not a routine increment: it is the scope-freeze commitment in
MAINTENANCE.md, made deliberately and once. The release tooling is
configured so that reaching 1.0.0 by accident is impossible.

## Will not do

These are refused in advance, permanently, so that a refusal is a citation
rather than an argument. Each has a reason recorded in
[design-spec.md](design-spec.md) or [MAINTENANCE.md](../MAINTENANCE.md):

- **AVIF and JPEG-XL output.** Spec-legal via `extraFormats`, and exactly
  how scope-frozen projects die. Modern-format transcoding is the CDN
  layer's business.
- **Authentication or access control in the engine.** That is the reverse
  proxy's job; the forward-auth recipe is in
  [deployment.md](deployment.md). A seam exists if reality overrules, but
  the default answer is no.
- **Presentation API, manifest generation, viewers.** This is the Image
  API box only — the pixel layer. Manifests come from the application that
  owns the objects; viewers are embedded JavaScript consuming both.
- **Per-image metadata in `info.json`**, including `rights` and
  attribution. It requires per-image configuration, which breaks both
  zero-config operation and the property that every image's `info.json` is
  generated the same way.
- **Video or PDF sources, embedded scripting, derivative caches,
  in-process TLS, lossy WebP, Image API v1, and feature toggles of any
  kind.** Lossy WebP would require C libwebp, which the security property
  forbids outright.
- **crates.io publication.** Refused permanently and independently of
  naming — nothing in this workspace is a library, so there is no API to
  promise anything about.

Contributions in the welcome categories — correctness fixes with a failing
test, security fixes, documentation accuracy, additional golden, property
or fuzz coverage — are wanted regardless of anything on this list. See
[CONTRIBUTING.md](../CONTRIBUTING.md).

## How this document changes

The "will not do" list is stable by construction: changing it means
changing the design spec, which is a deliberate act with its reasoning
written down. The "will do" list tracks the issue tracker and is expected
to move. If the two ever disagree, the design spec wins.
