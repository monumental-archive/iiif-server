# Governance

## Model

A single maintainer ([@CarlAllenn](https://github.com/CarlAllenn)) makes all
final decisions — the "benevolent dictator" model. Documenting a committee
that does not exist would be less honest than documenting the arrangement
that does.

Proposals, disagreements and design discussion happen in the open, on the
issue tracker and in pull requests. What makes this workable rather than
arbitrary is that the decisions are written down before they are needed:

- [docs/design-spec.md](docs/design-spec.md) — the scope, and the
  **pre-refusals**: features declined in advance, so a refusal is a quote
  rather than a debate.
- [MAINTENANCE.md](MAINTENANCE.md) — what the project claims and does not
  claim, what a version number promises, the response window.
- [docs/release-engineering.md](docs/release-engineering.md) — how releases
  are built, signed and verified, including the failures that shaped it.
- [CLAUDE.md](CLAUDE.md) — the enforced ground rules (no lint exemptions,
  PR-only changes, where lint canon lives).

A decision that is not in one of those is not yet a decision.

## Roles and responsibilities

- **Maintainer** — currently the only role held. Triages issues, reviews and
  merges pull requests, cuts releases through the publish pipeline, responds
  to security reports per [SECURITY.md](SECURITY.md) within the 7-day window
  committed in MAINTENANCE.md, and owns the decision registers above.
- **Contributors** — anyone submitting issues or pull requests under the
  requirements in [CONTRIBUTING.md](CONTRIBUTING.md). Contributions are
  covered by the [Individual Contributor License Agreement](CLA.md) rather
  than a DCO sign-off; the CLA-assistant bot records agreement once.

Should the project gain regular contributors, committer status and this
document evolve with it.

## Access continuity

The project must survive the maintainer becoming unavailable. The measures
below are stated as they actually are, including the one gap:

- **Everything needed to build, test and release is in the repository.** The
  toolchain is pinned with mise, every workflow runs the same `task` targets
  a contributor runs locally, and the release path is documented in
  [docs/release-engineering.md](docs/release-engineering.md). A stranger with
  push access can reproduce the setup end to end. The recovery paths — what
  to do when a release fails partway — are tracked separately in #70 and are
  the weaker half of that document today.
- **Publishing uses OIDC throughout.** Container signing (cosign keyless),
  build provenance and registry authentication all derive from the workflow's
  own identity at the tag. There is no registry credential to inherit, and
  the signing identity is a workflow path rather than a key someone holds.
- **One long-lived credential exists, and it is not load-bearing for
  security.** `RELEASE_TOKEN` is a fine-grained PAT used only so that the
  Release PR triggers CI — pull requests opened with the default
  `GITHUB_TOKEN` do not. If it is lost or expires, a successor recreates it
  with `contents: write` and `pull-requests: write` on this repository alone;
  nothing published is signed with it and no artifact depends on it.
- **GitHub's succession path applies.** The maintainer's estate arrangements
  cover credential succession for the GitHub account, and GitHub's
  [deceased user policy](https://docs.github.com/site-policy/other-site-policies/github-deceased-user-policy)
  provides a fallback for transferring the repository.
- **The licence guarantees a fork can continue the work.** AGPL-3.0-only
  means that in the worst case — no transfer, no access, no response —
  anyone can fork and continue without any legal step at all. Consumers
  verifying releases against the signing identity would need to re-point at
  the fork's workflow path; that is the intended and only coupling.

The contributor-facing consequence: a release can be cut, an issue can be
closed, and a security fix can ship within a week of the maintainer becoming
unavailable, by anyone who inherits or forks the repository.
