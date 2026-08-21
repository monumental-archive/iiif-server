// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! Plan execution: level selection → region decode → resample → quality →
//! mirror/rotate → encode. Pure compute; the caller owns threading and
//! backpressure.

#![expect(
    clippy::single_call_fn,
    reason = "each of these is a named step called once from the dispatch \
          above it. Inlining them to satisfy the lint would fold \
          separate formats, decode paths or parse stages into one long \
          body — the lint's own documentation calls it \"very \
          restrictive\", and here the single call site is the point: \
          one function per format is what makes the dispatch readable."
)]

use core::{error::Error, fmt};

use fast_image_resize as fir;
use num_traits::cast::ToPrimitive as _;

use crate::{
    codec::{CodecError, Master},
    encode::{EncodeError, encode},
    eval::Plan,
    grammar::Quality,
    image::{Raster, RasterError},
};

/// Pipeline failure, split by who caused it.
#[derive(Debug)]
#[non_exhaustive]
pub enum PipelineError {
    /// Decoding the master failed.
    Codec(CodecError),
    /// Encoding the output failed.
    Encode(EncodeError),
    /// Raster geometry/buffer invariant broke.
    Raster(RasterError),
    /// The resize step failed.
    Resize(String),
}

impl fmt::Display for PipelineError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Codec(err) => write!(f, "{err}"),
            Self::Encode(err) => write!(f, "{err}"),
            Self::Raster(err) => write!(f, "{err}"),
            Self::Resize(msg) => write!(f, "resample failure: {msg}"),
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
impl Error for PipelineError {}

impl From<CodecError> for PipelineError {
    #[inline]
    fn from(err: CodecError) -> Self {
        Self::Codec(err)
    }
}

impl From<EncodeError> for PipelineError {
    #[inline]
    fn from(err: EncodeError) -> Self {
        Self::Encode(err)
    }
}

impl From<RasterError> for PipelineError {
    #[inline]
    fn from(err: RasterError) -> Self {
        Self::Raster(err)
    }
}

/// Execute a plan against an opened master, returning encoded bytes.
///
/// # Errors
///
/// See [`PipelineError`].
#[inline]
#[expect(
    clippy::modulo_arithmetic,
    reason = "selects the quarter-turn fast path; degrees are `0..=360` \
              by the grammar, so the operand cannot be negative."
)]
pub fn execute(source: &mut dyn Master, plan: &Plan) -> Result<Vec<u8>, PipelineError> {
    // 1. Decode the crop with enough detail for the output scale; the codec picks
    //    its own cheapest path (pyramid level, reduced- resolution wavelet decode,
    //    or resident raster).
    let needed = f64::from(plan.crop.width) / f64::from(plan.out_w.max(1));
    let decoded = source.decode_crop(plan.crop, needed)?;

    // 2. Resample to the output size.
    let resized = resize(decoded, plan.out_w, plan.out_h)?;

    // 3. Quality.
    let mut recoloured = match plan.quality {
        Quality::Default | Quality::Color => resized,
        Quality::Gray => resized.into_gray(),
        Quality::Bitonal => resized.into_bitonal(),
    };

    // 4. Mirror, then rotate.
    if plan.mirror {
        recoloured.mirror();
    }
    let raster = if plan.degrees == 0.0_f64 {
        recoloured
    } else if plan.degrees % 90.0_f64 == 0.0_f64 {
        recoloured.rotate_quarters((plan.degrees / 90.0_f64).to_u8().unwrap_or(0))
    } else {
        recoloured.rotate_arbitrary(plan.degrees)
    };

    // 5. Encode.
    Ok(encode(&raster, plan.format)?)
}

/// Resample a raster to exactly `out_w` x `out_h`, Lanczos3 via
/// `fast_image_resize`.
///
/// Returns the input untouched when it is already that size, which is
/// the common `full/max` path.
///
/// # Errors
///
/// [`PipelineError::Internal`] if the resampler rejects the buffer or
/// the target dimensions.
fn resize(raster: Raster, out_w: u32, out_h: u32) -> Result<Raster, PipelineError> {
    if raster.width() == out_w && raster.height() == out_h {
        return Ok(raster);
    }
    let pixel_type = match &raster {
        Raster::Gray8 { .. } => fir::PixelType::U8,
        Raster::GrayA8 { .. } => fir::PixelType::U8x2,
        Raster::Rgb8 { .. } => fir::PixelType::U8x3,
        Raster::Rgba8 { .. } => fir::PixelType::U8x4,
    };
    let channels = raster.channels();
    let (width, height, data) = match raster {
        Raster::Gray8 {
            width,
            height,
            data,
        }
        | Raster::Rgb8 {
            width,
            height,
            data,
        }
        | Raster::GrayA8 {
            width,
            height,
            data,
        }
        | Raster::Rgba8 {
            width,
            height,
            data,
        } => (width, height, data),
    };
    let src = fir::images::Image::from_vec_u8(width, height, data, pixel_type)
        .map_err(|err| PipelineError::Resize(err.to_string()))?;
    let mut dst = fir::images::Image::new(out_w, out_h, pixel_type);
    let mut resizer = fir::Resizer::new();
    let options = fir::ResizeOptions::new()
        .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::Lanczos3));
    resizer
        .resize(&src, &mut dst, &options)
        .map_err(|err| PipelineError::Resize(err.to_string()))?;
    let data = dst.into_vec();
    Ok(match channels {
        1 => Raster::Gray8 {
            width: out_w,
            height: out_h,
            data,
        },
        2 => Raster::GrayA8 {
            width: out_w,
            height: out_h,
            data,
        },
        3 => Raster::Rgb8 {
            width: out_w,
            height: out_h,
            data,
        },
        _ => Raster::Rgba8 {
            width: out_w,
            height: out_h,
            data,
        },
    })
}
