# Security assurance case

Why this project's security requirements are met: the threat model, the
trust boundaries, the design argument, and the argument that common
implementation weaknesses are countered.

The requirements themselves — what a user can and cannot expect — are
stated in the threat model below. This document is the evidence
that they hold, and it is deliberately written to be falsifiable: every
claim below names the mechanism that enforces it and the place it is
checked, so a reader can go and disagree with the specific thing rather
than with the posture.

## Threat model

Ordered by exposure — how close each threat sits to an unauthenticated
attacker.

**Request URLs are fully hostile.** The server is designed to sit on the
public internet with no authentication in front of it (auth is the reverse
proxy's job, see below). Every byte of the request path — the IIIF
region/size/rotation/quality/format grammar, the identifier, the v2.1
translation layer — is attacker-controlled. This is the primary threat and
receives the most machinery: a typed grammar, property tests over
round-trips and canonicalization, and three of the four fuzz targets
(`url_grammar`, `v2_grammar`, `identifier`).

**Identifier resolution is an escape-target.** An identifier maps to a
path or object key. Path traversal, absolute-path injection and
encoding-confusion are the obvious attacks, and they are the reason
identifier resolution is fuzzed separately from the grammar rather than as
part of it.

**Source masters are semi-trusted.** An operator curates what is in the
image root or the bucket. A hostile master is therefore an availability
problem rather than a code-execution problem — but only because every
decoder is memory-safe Rust. The `master_open` fuzz target covers this
path. If a decoder here were C, this row would be the most serious in the
table instead of the least.

**Supply-chain substitution.** Someone convinces an operator to run bytes
we did not build — a forged image, a tampered binary, a poisoned
dependency, a compromised CI run laundering an artifact.

**Build-time compromise.** A malicious or compromised action, a cache
poisoned between build and sign, or an exfiltration attempt from a job
that holds a token.

**Explicitly out of scope**, and closed as such when reported:

- Volume denial of service. Bounded concurrency and published limits are
  the mitigation; absorbing arbitrary traffic is the deployment's job.
- Resource exhaustion from a master an operator chose to serve.
- Anything requiring write access to the source root — that is already
  inside the trust boundary.
- Confidentiality of served content. The server holds no secrets, has no
  users, and stores no state; there is nothing to keep confidential.
- Missing authentication and missing TLS, both deliberately out of scope.

## Trust boundaries

**Request → typed value.** The outermost boundary. Untrusted text becomes
a typed request only by passing the grammar in
[grammar.rs](../crates/core/src/grammar.rs); nothing downstream accepts a
string where a parsed value is expected. There is no second parser and no
lenient path — a request either parses into the type or is rejected with a
spec-defined error.

**Identifier → source location.** Resolution
([ident.rs](../crates/core/src/ident.rs)) is the only place an
attacker-influenced string becomes a filesystem path or object key, and it
is fuzzed as its own target.

**Storage seam.** The `Source` trait
([source.rs](../crates/core/src/source.rs)) is the boundary between the
pixel pipeline and storage. Bytes arriving from a local file or an
object-store range read are untrusted-in-format even when the operator is
trusted, and cross into the decoders only through the codec layer.

**Process boundary.** The server is stateless and holds no credentials for
the data it serves beyond the object-store configuration it is given. The
container image is `FROM scratch` with no shell, so a hypothetical
code-execution bug lands in a process with no interpreter, no package
manager and nothing to pivot into.

**Publish boundary.** What CI builds versus what a consumer pulls. The
publish workflow proves these identical before anything signs them: the
image is pushed, pulled back by digest, smoke-tested, and run against the
official IIIF validators, and only then signed and attested. Attestation
happens last and only on proof, because Sigstore is append-only and a
wrong attestation is permanent.

## Secure design principles

**Economy of mechanism.** The feature surface is frozen by design. The
attack surface cannot grow through feature creep, because features are
pre-refused in writing rather than declined case by case. There are no
feature toggles, no plugin mechanism, no embedded scripting and no
configuration language — the things that most often turn a parser bug into
a compromise are absent by construction rather than by discipline.

**Fail-safe defaults.** Parsing is fail-closed: unknown, malformed or
out-of-range input is rejected with a spec-defined error rather than
coerced. Limits (`maxWidth`, `maxHeight`, `maxArea`) are published in
`info.json`, so a client is told the boundary rather than discovering it
by being cut off. Backpressure is explicit: admission and decode
concurrency are bounded by semaphores in
[app.rs](../crates/server/src/app.rs), and a full queue returns 503 with
`Retry-After` rather than queueing without bound or dying.

**Complete mediation.** One implementation of the grammar, used by both
the 3.0 and 2.1 surfaces — the v2.1 layer
([v2.rs](../crates/core/src/v2.rs)) translates into the same typed request
rather than parsing independently. There is no bypass path, no
"compatibility" parser, and no cache that could serve an unvalidated
response.

**Least privilege.** Workflow tokens are read-only at the top level with
write scopes declared per job. Egress is controlled per job with
step-security/harden-runner. Publishing uses OIDC — the signing identity
is the workflow path at the tag, so there is no long-lived signing key to
steal, and a consumer can require exactly that identity. Jobs whose output
gets signed disable the tool cache, so a poisoned cache entry cannot be
laundered into an attested artifact.

**Defence stated at its true strength.** The security property is "zero C
parses untrusted input, anywhere in the product" — not "no C anywhere".
The optional mimalloc allocator and the ring crypto cores are C that
computes over our own data and never parses hostile bytes. Stating the
property this precisely is itself part of the design: an overclaim that
fails audit destroys the credibility of the claims that are true.

## Common implementation weaknesses, countered

| Weakness class | Countermeasure | Evidence |
| --- | --- | --- |
| Memory corruption (CWE-119 family) | `unsafe_code = "forbid"` workspace-wide; every decoder is pure Rust including JPEG 2000/HTJ2K | `Cargo.toml`; the dependency doctrine in design-spec.md |
| Crash, hang or panic on hostile input | Four `cargo-fuzz` targets — `url_grammar`, `v2_grammar`, `identifier`, `master_open` — run in CI and on a schedule | `fuzz/fuzz_targets/`, `.github/workflows/fuzz.yml` |
| Parser differentials between the two API versions | One grammar, one typed request; v2.1 translates rather than re-parses; property tests over round-trip and canonicalization | `grammar.rs`, `v2.rs` |
| Spec-conformance drift presenting as a security bug | Official IIIF validators (3.0 and 2.1, level 2) gate every PR and re-run against the published image | `ci.yml`, `publish.yml` |
| Path traversal / identifier confusion | Dedicated resolution layer with its own fuzz target | `ident.rs`, `fuzz/fuzz_targets/identifier.rs` |
| Decompression bombs and unbounded work | Published `maxWidth`/`maxHeight`/`maxArea`, bomb guards, bounded decode pool with 503 + `Retry-After` | `info.rs`, `app.rs` |
| Known-vulnerable dependencies | cargo-deny advisories on every PR and weekly against an unchanged main; Renovate for updates; Trivy against the image | `deny.toml`, `ci.yml`, `scheduled.yml` |
| Permissive-licence and dependency-substitution risk | cargo-deny licence, bans and sources checks; committed lockfiles for both workspaces; the fuzz workspace gated `--locked` | `deny.toml`, `Cargo.lock`, `fuzz/Cargo.lock` |
| Undetected code-level defects | clippy at all/pedantic/nursery/cargo plus restriction opt-ins, `-D warnings`, and no `#[allow]` exemptions anywhere; CodeQL for `rust` and `actions` | `Cargo.toml` lint table, `codeql.yml` |
| Workflow-level attacks (injection, cache poisoning, impostor actions) | zizmor at pedantic persona, offline on every commit and online weekly; every action pinned by SHA | `ci.yml`, `scheduled.yml` |
| Leaked credentials | gitleaks on staged changes and over history in CI | `lefthook.yml`, `ci.yml` |
| Artifact substitution | org-signer Sigstore attestation, SLSA build provenance, pull-back-and-verify before signing, and a documented consumer verification command | `publish.yml`, `docs/deployment.md` |

## Why this is believed sufficient, and what remains

The argument is not that no defect exists. It is that the classes of
defect that turn into remote code execution in comparable software —
memory-unsafe image decoding, lenient parsers with bypass paths,
unpinned build inputs, unverifiable artifacts — are structurally absent
rather than merely untriggered, and that the classes that remain fail
toward a visible error rather than toward silence.

Residual risks, stated plainly:

- **A logic defect in the pixel pipeline** could produce a wrong image
  without crashing. Golden and differential tests against independent
  implementations are the mitigation, and they are the reason those tests
  exist rather than only unit tests.
- **The storage layer is under-tested.** The `Source` implementations,
  particularly the object-store path, carry materially less test coverage
  than the rest of the workspace. This is a real gap at a trust boundary,
  it is known, and closing it is on the roadmap.
- **The tracked dependency class can still produce advisories.** The
  honest claim in MAINTENANCE.md is a handful of interventions a year, not
  zero. Pure-Rust decoders demote those from RCE-class to mostly
  DoS-class; they do not eliminate them.
- **A compromise of the maintainer's GitHub account** would defeat the
  publish-boundary controls, because the signing identity is a workflow in
  this repository. Branch protection, required status checks and tag
  immutability raise the cost; they do not remove it.
- **The published Branch-Protection score stops short of full marks, and
  cannot be raised.** `main` is governed by rulesets with no bypass
  actors: pull requests required, strict status checks, linear history,
  signed commits, no force-push, no deletion. Those rules live in a
  ruleset rather than in classic branch protection specifically so that
  OpenSSF Scorecard can read them — the ruleset API is public, whereas
  the classic settings need an admin token the workflow deliberately does
  not hold. The score still stops at 4/10, because the remaining points
  require approving reviewers, CODEOWNERS review and last-push approval,
  none of which a single-maintainer project can satisfy. The gap is the
  review tier, not an absent control.

This document is reviewed when the threat model changes — a new input
class, a new trust boundary, or a new publishing path — and at minimum
whenever a security review is performed.
