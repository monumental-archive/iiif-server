// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! Library surface of the server crate: the HTTP application, exposed so
//! integration tests exercise exact response semantics without sockets.

pub mod app;
pub mod metrics;

/// Release version, from the crate manifest — the single source. Reported by
/// `--version` and by the `iiif_build_info` metric.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Commit the binary was built from, or `unknown`.
///
/// Read at compile time from `GITHUB_SHA` — the full forty-hex commit,
/// which every Actions step already carries and which is identical on both
/// legs of the release's reproducibility gate. It used to come from
/// `IIIF_BUILD_REVISION`, exported by a prepare script this repository no
/// longer has: the oci-image class builds the binary from a declaration now
/// (`.github#775`), and a declaration cannot carry a repository's own
/// compile-time variable. Inventing an organisation-wide name for one would
/// only help code that opted into the name, where the platform already has
/// one. Absent in ordinary `cargo build`s, where there is no meaningful
/// answer — a working tree is not a revision.
pub const REVISION: &str = match option_env!("GITHUB_SHA") {
    Some(revision) => revision,
    None => "unknown",
};
