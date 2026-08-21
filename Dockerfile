# syntax=docker/dockerfile:1@sha256:ecfaec9ed6d810b56388c508f4121597bfbba70d41a6dfeee4d8cad5f295fc32

# The official image: one static musl binary and a certificate bundle, on
# nothing at all. No distro, no shell, no package manager, no writable
# filesystem — the container-shaped twin of "one static binary".
#
# NO COMPILE HAPPENS HERE, by rule (.github#295). The binary is built by
# scripts/oci-prepare.sh in this repository's mise-pinned toolchain,
# natively per architecture, and COPYed in: the org's repro gate measured
# the in-container cargo build nondeterministic while the same crates
# built bit-for-bit under the pinned native toolchain, so a Dockerfile
# that compiles IS the failure mode. What is left is pure assembly over
# pinned inputs, which is why every stage below is digest-pinned and
# `scratch` is the runtime.

# The TLS trust store, and the only reason any stage exists above
# `scratch`. rustls reads the bundle through SSL_CERT_FILE, which is what
# lets a scratch image talk to S3 at all: there is no operating system
# here to hold a default one, so the bundle is shipped as a file. Serving
# local files would work without it; every s3:// deployment would not.
#
# Taken from a digest-pinned base rather than from the runner's own
# /etc/ssl, deliberately: the runner image rolls between builds and its
# bundle is not an input anything pins, so copying it would put an
# unpinned file inside a signed artifact. Measured 2026-08-21 — this
# digest ships etc/ssl/certs/ca-certificates.crt as a regular file,
# 179359 bytes. Renovate rolls tag and digest together.
FROM alpine:3.22@sha256:14358309a308569c32bdc37e2e0e9694be33a9d99e68afb0f5ff33cc1f695dce AS certs

FROM scratch

COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY LICENSE /LICENSE
# Built outside, per architecture, by scripts/oci-prepare.sh — with
# cargo-auditable, so the .dep-v0 section keeps this image's Rust
# dependency surface visible to a scanner reading the published bytes.
COPY dist/iiif-server /iiif-server

ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

# Numeric, because scratch has no /etc/passwd to name a user in. 65532 is
# the conventional non-root uid for distroless-style images.
USER 65532:65532

EXPOSE 6363

# The image holds no shell and no curl, so the binary probes itself.
HEALTHCHECK --interval=10s --timeout=5s --retries=6 --start-period=5s \
    CMD ["/iiif-server", "healthcheck", "127.0.0.1:6363"]

ENTRYPOINT ["/iiif-server"]
CMD ["serve", "/imageroot", "--bind", "0.0.0.0:6363"]
