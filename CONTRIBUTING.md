# Contributing

Thanks for your interest. Two things to know before you open a PR.

## License and CLA

The project is licensed [AGPL-3.0-only](LICENSE). External contributions
require agreeing to the [Individual Contributor License Agreement](CLA.md)
(Apache ICLA-derived, including a relicensing grant to the maintainer). The
CLA-assistant bot will prompt you on your first pull request; agreement is
recorded once and covers subsequent contributions.

## Scope

Read [docs/design-spec.md](docs/design-spec.md) first. The feature surface is
deliberately complete-and-frozen: the full IIIF Image API 3.0 and 2.1 level 2
compliance tables, nothing else. The spec's "Pre-refusals" section lists
features that are declined in advance (AVIF/JXL outputs, auth in the engine,
Presentation API, per-image metadata, …) — PRs adding them will be closed
with a pointer to that section, kindly.

What is always welcome:

- correctness fixes, with a failing test first
- security fixes
- documentation accuracy
- new golden/property/fuzz coverage

## Development

Tooling is pinned with [mise](https://mise.jdx.dev): `mise install`, then
`task ci` runs exactly what CI runs (fmt + clippy + cargo-deny + linters +
tests). `lefthook install` wires the same checks as git hooks.

The workspace is `#![forbid(unsafe_code)]` throughout, and every dependency
must be permissively licensed (enforced by `cargo deny`). Zero C code parses
untrusted input anywhere in the product — dev-time fixture generation is the
only exemption.

## Requirements for an acceptable contribution

A change is ready to merge when all of the following hold. None of them are
negotiable, and all of them are enforced by CI rather than by review
attention:

- **`task ci` passes.** It runs exactly what CI runs, so a green local run
  and a red CI run should not be possible. If they diverge, that is itself a
  bug worth reporting.
- **Tests are part of the change, not a follow-up.** A correctness fix wants
  a failing test first — one that fails before the fix and passes after. A
  new code path wants coverage in the same pull request. "Tests to follow"
  is not an accepted state.
- **The commit history is conventional-commit formatted.** The commit-msg
  hook enforces this; it drives the changelog.
- **No lint exemptions.** See below.
- **Documentation that the change makes wrong is fixed in the same pull
  request.** Docs that disagree with the code are treated as defects.
- **The CLA is agreed** — the bot prompts on your first pull request.

## Coding standard

The standard is the enforced lint set, not a prose style guide: rustfmt on
the pinned nightly, and clippy with `all`, `pedantic`, `nursery`, `cargo` and
selected `restriction` lints, all with `-D warnings`. The configuration lives
in `Cargo.toml`, `rustfmt.toml` and `clippy.toml` and is the single source of
truth — this document deliberately does not restate it, because a restatement
is a copy that drifts.

**There are no lint exemptions.** Do not add `#[allow(...)]`, and do not add
carve-outs to the lint configuration. Fix the code instead. The handful of
allows recorded in `Cargo.toml` are canonical decisions inherited from the
shared configuration template and are not a precedent for adding more. A pull
request that silences a lint rather than satisfying it will be asked to
change, however reasonable the individual case looks — the value of the rule
is that it has no exceptions.

Beyond the linters: match the surrounding code. This repository comments
*why*, not *what*, and load-bearing decisions get written down where the next
person will trip over them.

## Code review

Every change lands by pull request; `main` is protected and takes no direct
pushes. Required status checks must pass, conversations must be resolved, and
history is linear.

Being honest about what review means here: this is a single-maintainer
project, so a contributor's pull request is reviewed by the maintainer, and
the maintainer's own changes are not reviewed by a second person. That is a
real limitation and it is why so much weight sits on mechanical enforcement —
the linters, the property tests, the fuzz targets, the official validators
and the golden/differential rigs do not get tired or defer to seniority. If
this project gains regular contributors, two-person review is the first thing
that changes.

## Small tasks

Issues labelled [`good first issue`][gfi] are scoped so that someone new to
the codebase can finish them without needing the whole design in their head —
documentation accuracy, additional test coverage, and small self-contained
correctness fixes. If none are open and you want somewhere to start, adding
golden or property coverage for an untested path is always welcome and never
wasted.

[gfi]: https://github.com/CarlAllenn/iiif-server/issues?q=is%3Aissue+is%3Aopen+label%3A%22good+first+issue%22

## Reporting problems

Bugs and feature discussion go to the [issue tracker][issues]. Suspected
vulnerabilities do **not** — follow [SECURITY.md](SECURITY.md), which routes
them privately and commits to a triage window.

Conduct expectations are in [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md); how
decisions get made, and what happens if the maintainer disappears, is in
[GOVERNANCE.md](GOVERNANCE.md).

[issues]: https://github.com/CarlAllenn/iiif-server/issues
