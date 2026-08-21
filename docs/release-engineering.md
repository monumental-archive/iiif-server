# Release engineering

How this repository releases, and the decisions that are ITS OWN rather
than the organisation's.

**The pipeline is not described here any more.** Since the import
(`.github#671`) this repository releases through
[monumental-archive/.github](https://github.com/monumental-archive/.github):
`docs/release.md` there is the specification, `docs/runbook.md` is the
operating manual, and this repository's `release.yml` and `publish.yml`
are stubs that call the shared workflows by pinned SHA. Anything about
phases, tags, drafts, provenance, signing or recovery is answered
there, once, for every repository — and answering it a second time here
would be a copy that can disagree with the machinery.

What follows is only what is true of *this* repository and could not be
delivered by a shared file.

## The deliverable

| Artifact | Where | Who it is for |
| --- | --- | --- |
| Container image, multi-architecture | `ghcr.io/monumental-archive/iiif-server` | deployment — and it is the whole distribution |

**One artifact class, `oci-image`.** Standalone binaries, their
checksums, `install.sh` and the attached validator report were published
by the pre-import pipeline and are not published now. An artifact class
is a standing promise of evidence — provenance, an SBOM, a signature,
a reproducibility check, per release, forever — and a class whose
artifacts nobody pulls is evidence owed for nothing. Declaring a second
class later is additive and costs no history.

Nothing is published to crates.io. See below — a standing decision, not
a naming delay.

## Why not crates.io

Publishing `iiif-core` and `iiif-sources` is refused, permanently, and
not because of the naming question:

- Those crates exist to separate concerns inside one binary. They have
  no independent consumers, and the workspace split is an architectural
  choice, not a distribution one.
- Publishing them creates a **public Rust API with semver obligations**,
  on a project whose entire thesis is a minimal maintenance surface —
  and the compatibility promise this project actually makes covers the
  HTTP surface, the CLI flags and the container contract, explicitly not
  internal Rust APIs.
- crates.io names are permanent and unreclaimable. A GHCR path can be
  abandoned; a crate name cannot.

Revisit only if a real consumer asks — demand, not tooling convenience.

## Why the image is `FROM scratch`

The design spec called for it, and the obstacle turned out to be
removable.

Verifying an S3 connection needs a trusted-certificate bundle. The spec
chose bundled `webpki-roots` precisely so a scratch image could do that
— but `reqwest` 0.13 removed the option entirely, and every rustls path
now resolves to `rustls-platform-verifier`, i.e. the operating system's
trust store, which a scratch image does not have. Rather than dependency
surgery, the bundle ships as a file with `SSL_CERT_FILE` pointing at it.

Worth knowing how that failure would have presented: serving local files
works fine without it. Only `s3://` deployments break.

The bundle comes from a digest-pinned `alpine` stage rather than from
the build runner's own `/etc/ssl`, because the runner image rolls
between builds and its bundle is not an input anything pins — copying it
would put an unpinned file inside a signed artifact.

The size that buys — about 6 MB to pull, 16 MB unpacked on arm64 and 18
MB on amd64, against the incumbent's 769 MB — is asserted rather than
remembered. `scripts/check_image_size.sh` runs from
`scripts/image-checks.sh`, the class's smoke test, which the release
runs twice: on the locally built image before anything is published, and
again on the bytes pulled back from the registry by digest. It fails
above 25 MB unpacked, about 40% above the amd64 build. It measures with
`docker export`, because `docker image inspect .Size` means the unpacked
total under one image store and the compressed total under the other.
The ceiling is meant to be raised when growth is real; the gate exists
so raising it is a decision recorded in a diff, alongside the figures in
README.md and docs/deployment.md, rather than a claim that quietly
stopped being true.

## Why nothing is compiled inside the Dockerfile

The organisation's reproducibility gate measured the in-container cargo
build nondeterministic while the same crates built bit-for-bit under a
pinned native toolchain (`.github#295`). So the binary is built by
`scripts/oci-prepare.sh`, natively per architecture, in the toolchain
`mise.toml` pins, and the Dockerfile only assembles pinned inputs. One
toolchain, not two; determinism by construction rather than by audit.

The build revision the binary reports through `--version` and the
`iiif_build_info` metric is read at compile time by `option_env!`, so
`oci-prepare.sh` derives it from the checkout — the release checks out
the tag, which is what keeps two rebuilds of one tag identical.

## Why `cargo auditable` is not optional

Rust discards dependency information at compile time. A scanner pointed
at a stripped static Rust binary sees **one file and zero packages** —
which is exactly the zero-crates result monumental-archive hit when
scanning an image built from this repository's source.

`cargo auditable` embeds a compressed dependency list that syft, trivy
and grype all read. Verified rather than assumed: trivy identifies the
binary as `rustbinary`, and the SBOM lists 190 packages.

Two consequences:

- The build **must not strip** the binary. `strip` discards the
  non-allocated section the data lives in, silently undoing the SBOM.
  The class sets `CARGO_PROFILE_RELEASE_STRIP: "false"` for exactly
  this.
- The image SBOM the release attaches is only meaningful because of
  this. Without it the SBOM would be durable, digest-attached and
  completely useless, while *looking* like diligence.

## Owner-side prerequisites

The pre-import list — `RELEASE_TOKEN`, a hand-made tag ruleset — is
gone: tags are minted by the organisation's App under org-level rulesets
that cover a repository the moment it lands, and the release path holds
its own credentials. What is left is one thing nobody else can do:

- **GHCR package visibility** set to public after the first publish.

## History

The pre-import pipeline, the four failed release attempts that shaped
it, the audit-derived egress allowlists and the v0.1.0 post-mortem are
recorded in this repository's git history and in the issues they cite.
They are not repeated here, because none of that machinery exists any
more: the lessons that generalised are written into the organisation's
release canon, which is where a future change to them belongs.
