// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! The source-read seam — a founding interface (see design spec,
//! Architecture).
//!
//! Everything that can hold a master image implements this: local files via
//! read/mmap in `iiif-sources`, object stores via ranged GETs later. Decoders
//! are sync and bridge at the boundary.

use core::{error::Error, fmt, future::Future, pin::Pin};
use std::io;

use bytes::Bytes;

/// Boxed future alias: the trait must be dyn-safe (sources are chosen at
/// runtime), so methods return boxed futures rather than using AFIT.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A byte-addressable, immutable-for-the-duration source of one master
/// image file.
pub trait ByteRangeSource: Send + Sync {
    /// Read exactly `len` bytes starting at `offset`. Short reads are
    /// errors: callers always know the byte layout they are asking for.
    fn read_range(&self, offset: u64, len: u64) -> BoxFuture<'_, Result<Bytes, SourceError>>;

    /// Total length of the source in bytes.
    fn length(&self) -> BoxFuture<'_, Result<u64, SourceError>>;
}

/// Source-layer failure. `NotFound` maps to HTTP 404; everything else is a
/// 5xx (the master exists but could not be read).
#[derive(Debug)]
#[non_exhaustive]
pub enum SourceError {
    /// The identifier resolves to nothing in this source.
    NotFound,
    /// The requested range extends beyond the end of the source — always a
    /// caller bug or a truncated/changed master, never a client error.
    OutOfRange {
        /// Requested start offset.
        offset: u64,
        /// Requested byte count.
        len: u64,
        /// Actual source length.
        source_len: u64,
    },
    /// Underlying I/O failure.
    Io(io::Error),
}

impl fmt::Display for SourceError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("source not found"),
            Self::OutOfRange {
                offset,
                len,
                source_len,
            } => write!(
                f,
                "range {offset}+{len} out of bounds for source of {source_len} bytes"
            ),
            Self::Io(err) => write!(f, "source I/O error: {err}"),
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "unsatisfiable on stable, measured with rustc rather than argued: \
              `provide` is E0658 `error_generic_member_access`, and \
              `type_id` is E0658 `error_type_id` — \"this is memory-unsafe \
              to override in user code\". `source` is implemented where \
              this type has one; `description` and `cause` are deprecated \
              and are left to the standard library's own implementations."
)]
impl Error for SourceError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::NotFound | Self::OutOfRange { .. } => None,
        }
    }
}

impl From<io::Error> for SourceError {
    #[inline]
    fn from(err: io::Error) -> Self {
        if err.kind() == io::ErrorKind::NotFound {
            Self::NotFound
        } else {
            Self::Io(err)
        }
    }
}
