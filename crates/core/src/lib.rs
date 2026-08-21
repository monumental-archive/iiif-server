// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! Pure library for the IIIF Image API engine: URL grammar, identifier
//! rules, the byte-range source seam, info.json, and the image pipeline.
//!
//! The grammar layer does no I/O. Nothing in this crate touches the network
//! or filesystem; sources are abstracted behind [`source::ByteRangeSource`].

// Three restriction lints are expected CRATE-WIDE here, each named
// individually — never as a group, which the org gate refuses outright.
// Adjudicated 2026-08-21 during the import (monumental-archive/.github#671);
// the org ruled these three iiif-server's to answer for rather than an
// org-level exclusion, because they are wrong for THIS crate and right
// nearly everywhere else.
//
// What this crate is: the decode -> transform -> encode path of an image
// server. Its subject matter is arithmetic over pixel buffers. The three
// lints below each forbid one of the operations that arithmetic consists
// of, so a per-site expectation would appear on 156 lines and say the same
// sentence 156 times, which is a worse record than saying it once, here,
// where a reader meets the crate.
//
// `#[expect]` rather than `#[allow]` deliberately: each of these fails the
// build the moment the crate stops containing the thing it excuses, so if
// the pixel maths ever leaves this crate, these lines go red rather than
// rotting.
//
// None of the three is a licence to be careless, and the real defences are
// elsewhere and are tested: dimension and area ceilings are enforced before
// any allocation (`eval`), the decompression-bomb fixture in
// `tests/fixtures/` is a regression test for exactly the overflow case, and
// `#![forbid(unsafe_code)]` means an out-of-range index is a panic and never
// a memory-safety bug.
#![expect(
    clippy::arithmetic_side_effects,
    reason = "74 sites, all pixel and coordinate arithmetic: scale factors, \
              strides, tile origins, region intersections. Rewriting each to \
              checked_* would replace arithmetic whose operands are already \
              bounded by the size ceilings in `eval` with error paths that \
              cannot be reached, and would obscure the maths that is the \
              point of the code. The bounds are asserted where they enter, \
              not at every operator."
)]
#![expect(
    clippy::float_arithmetic,
    reason = "46 sites. The IIIF Image API specifies `pct:` regions and \
              sizes, and arbitrary-angle rotation, in decimal terms — the \
              spec's own arithmetic is floating point, so the \
              implementation's is too. A fixed-point rewrite would be a \
              different specification."
)]
#![expect(
    clippy::indexing_slicing,
    reason = "36 sites, all indexing into buffers this crate allocated \
              itself from dimensions it computed and checked. The panic this \
              lint prevents is the panic we want: a wrong index here is a \
              logic error that must fail loudly in a test, not a silently \
              handled None in a decode loop."
)]

pub mod codec;
pub mod encode;
pub mod eval;
pub mod grammar;
pub mod ident;
pub mod image;
pub mod info;
pub mod pipeline;
pub mod source;
pub mod v2;
