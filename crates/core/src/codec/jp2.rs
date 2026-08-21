// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! JP2 / HTJ2K masters via the pure-Rust `j2k` crate — the capability no
//! `OpenJPEG`-based incumbent has, validated bit-exact against `OpenJPEG` by
//! SPIKE 2.

#![expect(
    clippy::single_call_fn,
    reason = "each of these is a named step called once from the dispatch \
          above it. Inlining them to satisfy the lint would fold \
          separate formats, decode paths or parse stages into one long \
          body — the lint's own documentation calls it \"very \
          restrictive\", and here the single call site is the point: \
          one function per format is what makes the dispatch readable."
)]

use core::fmt;

use j2k::{CpuDecodeParallelism, J2kDecoder, J2kScratchPool, PixelFormat, Rect};

use super::{CodecError, Master};
use crate::{
    eval::CropRect,
    image::Raster,
    info::{ImageDescription, SizeEntry, TileSet},
};

/// Wrap raw interleaved samples in the right raster variant.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "matches an upstream `#[non_exhaustive]` enum, so the wildcard \
              is REQUIRED rather than chosen — measured: deleting it \
              gives `error[E0004]: non-exhaustive patterns: `_` not \
              covered`. The lint asks for something the compiler refuses."
)]
const fn raster_of(fmt: PixelFormat, width: u32, height: u32, data: Vec<u8>) -> Raster {
    match fmt {
        PixelFormat::Gray8 => Raster::Gray8 {
            width,
            height,
            data,
        },
        _ => Raster::Rgb8 {
            width,
            height,
            data,
        },
    }
}

/// Default tile size advertised for untiled codestreams: reduced-
/// resolution decode makes any aligned request natively cheap, so the
/// advertised grid is a viewer hint, not a constraint.
const DEFAULT_TILE: u32 = 1024;

/// An opened JP2/HTJ2K master.
///
/// Owns the compressed bytes; decoders borrow them per request (parse state is
/// cheap relative to pixel work, and a fresh decoder per decode keeps the type
/// `Send` for the worker pool).
#[expect(
    clippy::module_name_repetitions,
    reason = "`jp2::Master` and `simple::Master` would SHADOW `codec::Master`, \
          the trait they implement, in any scope importing both. The \
          prefix is what keeps the implementation distinguishable from \
          the interface."
)]
pub struct Jp2Master {
    /// The whole compressed codestream. The `Debug` impl below skips it
    /// deliberately — it is megabytes.
    bytes: Vec<u8>,
    /// Full image width in pixels, from the codestream header.
    width: u32,
    /// Full image height in pixels, from the codestream header.
    height: u32,
    /// Component count: 1 for gray, 3 for colour.
    components: u16,
    /// Resolution levels the codestream carries, which bounds how far a
    /// power-of-two downscale can be served by decoding less.
    resolution_levels: u8,
    /// Tile dimensions, or the image dimensions when untiled.
    tile: (u32, u32),
    /// Live pool-pressure hint; see `Master::set_internal_parallelism`.
    internal_parallelism: bool,
}

impl fmt::Debug for Jp2Master {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Jp2Master")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("components", &self.components)
            .field("resolution_levels", &self.resolution_levels)
            .field("tile", &self.tile)
            .finish_non_exhaustive()
    }
}

impl Jp2Master {
    /// Parse the header and survey the codestream structure.
    ///
    /// # Errors
    ///
    /// [`CodecError::Corrupt`] when the stream does not parse;
    /// [`CodecError::Unsupported`] for component layouts outside the
    /// current matrix.
    #[inline]
    pub fn new(bytes: Vec<u8>) -> Result<Self, CodecError> {
        let decoder = J2kDecoder::new(&bytes)
            .map_err(|err| CodecError::Corrupt(format!("JP2 parse: {err}")))?;
        let info = decoder.info();
        let (width, height) = info.dimensions;
        let components = info.components;
        if !matches!(components, 1 | 3) {
            return Err(CodecError::Unsupported(format!(
                "{components}-component JP2 is not yet in the supported matrix"
            )));
        }
        let resolution_levels = info.resolution_levels.max(1);
        let tile = info
            .tile_layout
            .as_ref()
            .map_or((DEFAULT_TILE, DEFAULT_TILE), |tile| {
                (tile.tile_width, tile.tile_height)
            });
        Ok(Self {
            bytes,
            width,
            height,
            components,
            resolution_levels,
            tile,
            internal_parallelism: false,
        })
    }

    /// The decoder's output format for this master: gray for a
    /// single-component codestream, RGB for anything else.
    const fn pixel_format(&self) -> PixelFormat {
        if self.components == 1 {
            PixelFormat::Gray8
        } else {
            PixelFormat::Rgb8
        }
    }

    /// The deepest cheap downscale, as a resolution-halving count:
    /// bounded only by the codestream's own resolution ladder. The old
    /// 1/8 ceiling came from the shared `Downscale` enum (a DCT-geometry
    /// limit foreign to wavelets); `decode_region_scaled_pow2_into` walks
    /// the whole ladder, so a 165 MP master rendered at 512 px decodes at
    /// 1/16–1/32 instead of decoding 1/8 and resampling.
    fn levels_for(&self, needed: f64) -> u8 {
        let max_level = self.resolution_levels - 1;
        let mut choice = 0_u8;
        for level in 1..=max_level {
            if f64::from(1_u32 << u32::from(level).min(31)) <= needed {
                choice = level;
            }
        }
        choice
    }
}

/// The smallest `1/denom`-scaled rectangle covering `rect`.
///
/// Floor the origin, ceil the far edge. Mirrors the covering contract of
/// `decode_region_scaled_pow2_into`, which sizes its output this way but
/// only reports the rect after decoding — we need it first to size the
/// buffer.
#[expect(
    clippy::integer_division,
    clippy::integer_division_remainder_used,
    reason = "power-of-two downscale: the origin truncates toward the \
              containing sample and the far edge uses div_ceil, which is \
              what makes the result COVER the request rather than clip it."
)]
const fn scaled_covering_pow2(rect: Rect, denom: u32) -> Rect {
    let x0 = rect.x / denom;
    let y0 = rect.y / denom;
    let x1 = rect.x.saturating_add(rect.w).div_ceil(denom);
    let y1 = rect.y.saturating_add(rect.h).div_ceil(denom);
    Rect {
        x: x0,
        y: y0,
        w: x1.saturating_sub(x0),
        h: y1.saturating_sub(y0),
    }
}

impl Master for Jp2Master {
    #[inline]
    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    #[inline]
    fn describe(&self) -> ImageDescription {
        // scaleFactors mirror the codestream's real resolution ladder.
        let scale_factors: Vec<u32> = (0..self.resolution_levels)
            .map(|level| 1_u32 << level)
            .collect();
        let mut sizes: Vec<SizeEntry> = scale_factors
            .iter()
            .map(|factor| SizeEntry {
                width: self.width.div_ceil(*factor),
                height: self.height.div_ceil(*factor),
            })
            .collect();
        sizes.reverse();
        ImageDescription {
            width: self.width,
            height: self.height,
            tiles: vec![TileSet {
                width: self.tile.0,
                height: if self.tile.1 == self.tile.0 {
                    None
                } else {
                    Some(self.tile.1)
                },
                scale_factors,
            }],
            sizes,
        }
    }

    #[inline]
    fn set_internal_parallelism(&mut self, allow: bool) {
        self.internal_parallelism = allow;
    }

    #[inline]
    fn advisories(&self) -> Vec<String> {
        let mut notes = Vec::new();
        if self.resolution_levels <= 1 && u64::from(self.width) * u64::from(self.height) > 4_000_000
        {
            notes.push(
                "large JP2 with a single resolution level: zoomed-out requests decode the \
                full image. Re-encode with resolution levels, e.g.: opj_compress -n 6"
                    .to_owned(),
            );
        }
        notes
    }

    #[inline]
    #[expect(
        clippy::as_conversions,
        reason = "widening `u32`/`u8` to `usize` for buffer indexing, lossless on \
              every target this ships to (musl x86_64 and aarch64). The \
              alternative is measurably worse and was measured: \
              `usize::try_from(w).unwrap()` trips `clippy::unwrap_used`, which \
              this same lint set forbids, so it trades one enabled \
              restriction for another and adds an error path that cannot \
              be reached."
    )]
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "matches an upstream `#[non_exhaustive]` enum, so the wildcard \
              is REQUIRED rather than chosen — measured: deleting it \
              gives `error[E0004]: non-exhaustive patterns: `_` not \
              covered`. The lint asks for something the compiler refuses."
    )]
    fn decode_crop(&mut self, crop: CropRect, needed: f64) -> Result<Raster, CodecError> {
        let mut decoder = J2kDecoder::new(&self.bytes)
            .map_err(|err| CodecError::Corrupt(format!("JP2 parse: {err}")))?;
        // Pool pressure decides: an idle pool wants the codec's internal
        // parallelism (1.7× lower latency), a saturated one does not
        // (oversubscription costs ~16% throughput). See
        // `Master::set_internal_parallelism`.
        decoder.set_cpu_decode_parallelism(if self.internal_parallelism {
            CpuDecodeParallelism::Auto
        } else {
            CpuDecodeParallelism::Serial
        });
        let levels = self.levels_for(needed);
        let fmt = self.pixel_format();
        let bpp = match fmt {
            PixelFormat::Gray8 => 1_usize,
            _ => 3_usize,
        };
        let roi = Rect {
            x: crop.x,
            y: crop.y,
            w: crop.width,
            h: crop.height,
        };
        let scaled = scaled_covering_pow2(roi, 1_u32 << u32::from(levels).min(31));
        let mut pool = J2kScratchPool::new();
        let stride = scaled.w as usize * bpp;
        let mut out = vec![0_u8; stride * scaled.h as usize];
        decoder
            .decode_region_scaled_pow2_into(&mut pool, &mut out, stride, fmt, roi, levels)
            .map_err(|err| CodecError::Corrupt(format!("JP2 decode: {err}")))?;
        Ok(raster_of(fmt, scaled.w, scaled.h, out))
    }
}
