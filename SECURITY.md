# Security policy

## Reporting a vulnerability

Report privately through GitHub's [security advisory form][advisory].
That keeps the report private until a fix ships and gives you credit on
the published advisory.

[advisory]: https://github.com/monumental-archive/iiif-server/security/advisories/new

Please do not open a public issue for a suspected vulnerability.

If you cannot use GitHub advisories, email the maintainer address on the
commits in this repository.

## What to expect

Advisories against the tracked dependency class, and defects in this code, are
triaged within **7 days** and released as signed patch releases. That window
is the commitment made in [MAINTENANCE.md](MAINTENANCE.md), which also
explains why a project that is feature-complete by design still needs a
security response process.

## Supported versions

The latest release only. This project is scope-frozen by design rather than
long-lived-branch-maintained: fixes land on the current version and a new
patch release follows.

## The security property, and its edges

**Zero C parses untrusted input, anywhere in the product.** Request URLs are
hostile and are fuzzed — the grammar and identifier resolution both. Source
masters are operator-curated and semi-trusted: a malicious master is an
availability problem, not remote code execution, because every decoder is
memory-safe Rust.

Things that are therefore *not* vulnerabilities in this project, and will be
closed as such:

- **Resource exhaustion from a master an operator chose to serve.** The
  bounded decode pool, the published `maxWidth`/`maxHeight`/`maxArea` limits
  and the decompression-bomb guards are the mitigation; a master that decodes
  slowly is a capacity question.
- **Anything requiring access to the source root.** An operator who can write
  to the image directory is already inside the trust boundary.
- **Missing authentication or TLS.** Both are deliberately out of scope and
  belong to the reverse proxy; see MAINTENANCE.md's pre-refusals and the
  forward-auth recipe in [docs/deployment.md](docs/deployment.md).

Reports in those categories are still welcome as issues if the behaviour looks
wrong — they are just not embargoed.

## Verifying what you run

Released images carry SLSA build provenance, minted by the organisation's
**signer** — a repository that runs no caller-supplied code and holds the
only `id-token: write` on the release path. The signing identity is that
signer's workflow, not this repository's, which is the whole point of the
split: an identity that never executes the code it signs for.

```console
gh attestation verify oci://ghcr.io/monumental-archive/iiif-server:v0.2.0 \
  --owner monumental-archive \
  --signer-workflow monumental-archive/signer/.github/workflows/sign.yml \
  --source-ref refs/tags/v0.2.0 \
  --deny-self-hosted-runners
```

Pin `--signer-digest <signer-commit>` as well to require one reviewed
version of the signer rather than any revision of it; the commit is
recorded in the release's evidence bundle.

A signature nobody checks does no work — if you deploy this, wire that
into your pipeline.

**The v0.1.0 signature is a different shape and does not verify this
way.** It was produced by the pre-import pipeline under the previous
owner: cosign keyless, with the certificate identity naming that
repository's own `publish.yml`. Transfers do not carry attestations, and
the OIDC subject claim changes at transfer, so nothing from before the
import can be verified against the organisation's identity. v0.2.0 is
the first release verifiable as above.
