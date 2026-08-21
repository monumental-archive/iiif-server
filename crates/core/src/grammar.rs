// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! Typed IIIF Image API 3.0 request grammar.
//!
//! Parsing is strict: every accepted string is spec-legal, every value is
//! range-checked at parse time where the check needs no image dimensions.
//! Checks that need the image (region clipping, non-`^` size overflow,
//! server limits) happen at evaluation, not here.
//!
//! `Display` prints the *literal* canonical spelling of a value (integers
//! without leading zeros, floats per the spec's float-formatting rules).
//! Canonicalization against a concrete image (region → pixels, size →
//! `w,h`) is a separate evaluation-layer concern.

use core::{error::Error, fmt};

/// The image region to extract, per §4.1.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Region {
    /// `full` — the entire image.
    Full,
    /// `square` — centered square, side = the shorter image dimension.
    Square,
    /// `x,y,w,h` — pixel coordinates. `w`, `h` are non-zero (parse-time
    /// rule: zero width or height is always a 400).
    Pixels {
        /// Left edge, pixels from the left of the full image.
        x: u32,
        /// Top edge, pixels from the top of the full image.
        y: u32,
        /// Region width in pixels (non-zero).
        width: u32,
        /// Region height in pixels (non-zero).
        height: u32,
    },
    /// `pct:x,y,w,h` — percentages of full-image dimensions.
    Percent {
        /// Left edge as a percentage of full-image width.
        x: f64,
        /// Top edge as a percentage of full-image height.
        y: f64,
        /// Region width as a percentage of full-image width.
        width: f64,
        /// Region height as a percentage of full-image height.
        height: f64,
    },
}

/// How the scaled dimensions are derived, per §4.2 (without the `^` flag,
/// which lives on [`Size`]).
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum SizeKind {
    /// `max` — maximum size available under server limits.
    Max,
    /// `w,` — exact width, aspect preserved.
    Width(u32),
    /// `,h` — exact height, aspect preserved.
    Height(u32),
    /// `pct:n` — scale by percentage of the extracted region.
    Percent(f64),
    /// `w,h` — exact dimensions, aspect NOT preserved.
    WidthHeight(u32, u32),
    /// `!w,h` — best fit inside `w`×`h`, aspect preserved.
    Confined(u32, u32),
}

/// The size parameter: an optional `^` upscale flag plus a [`SizeKind`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Size {
    /// `^` prefix: upscaling beyond the extracted region is permitted.
    pub upscale: bool,
    /// How the scaled dimensions are derived.
    pub kind: SizeKind,
}

/// The rotation parameter, per §4.3: optional mirror, then clockwise
/// degrees in `0..=360`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct Rotation {
    /// `!` prefix: mirror on the vertical axis before rotating.
    pub mirror: bool,
    /// Clockwise rotation in degrees, `0..=360`.
    pub degrees: f64,
}

/// The quality parameter, per §4.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Quality {
    /// `default` — the image as stored, no colorspace change.
    Default,
    /// `color` — full colour.
    Color,
    /// `gray` — greyscale.
    Gray,
    /// `bitonal` — one bit per pixel.
    Bitonal,
}

/// The format parameter, per §4.5. All spec-enumerated formats parse; which
/// ones the server can *encode* is a capability question answered
/// elsewhere (unsupported-but-well-formed → 400 per spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Format {
    /// `jpg` — JPEG.
    Jpg,
    /// `tif` — TIFF.
    Tif,
    /// `png` — PNG.
    Png,
    /// `gif` — GIF.
    Gif,
    /// `jp2` — JPEG 2000.
    Jp2,
    /// `pdf` — single-page PDF wrapping the raster.
    Pdf,
    /// `webp` — WebP.
    Webp,
}

/// A complete parsed image request:
/// `{region}/{size}/{rotation}/{quality}.{format}`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ImageRequest {
    /// The region parameter (first path segment).
    pub region: Region,
    /// The size parameter (second path segment).
    pub size: Size,
    /// The rotation parameter (third path segment).
    pub rotation: Rotation,
    /// The quality parameter (final segment, before the dot).
    pub quality: Quality,
    /// The format parameter (final segment, after the dot).
    pub format: Format,
}

/// Which request component failed to parse. Every variant maps to a 400.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Component {
    /// The region segment failed to parse.
    Region,
    /// The size segment failed to parse.
    Size,
    /// The rotation segment failed to parse.
    Rotation,
    /// The quality segment failed to parse.
    Quality,
    /// The format suffix failed to parse.
    Format,
    /// The path didn't have the `region/size/rotation/quality.format` shape.
    Structure,
}

/// A parse failure: the offending component and the input that failed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseError {
    /// Which component rejected the input.
    pub component: Component,
    /// The exact segment text that failed to parse.
    pub input: String,
}

impl ParseError {
    /// A parse failure naming the component that rejected `input`.
    fn new(component: Component, input: &str) -> Self {
        Self {
            component,
            input: input.to_owned(),
        }
    }
}

impl fmt::Display for ParseError {
    #[inline]
    #[expect(
        clippy::use_debug,
        reason = "`{:?}` deliberately: `self.input` is the client's raw \
                      path segment, and Debug's escaping is what keeps a \
                      control character or a newline out of a log line or \
                      an error body verbatim. Display here would echo \
                      attacker bytes unquoted."
    )]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let what = match self.component {
            Component::Region => "region",
            Component::Size => "size",
            Component::Rotation => "rotation",
            Component::Quality => "quality",
            Component::Format => "format",
            Component::Structure => "request path",
        };
        write!(f, "malformed {what}: {:?}", self.input)
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
impl Error for ParseError {}

/// Strict unsigned decimal integer: one or more ASCII digits, nothing else.
///
/// Leading zeros are accepted (the canonical print normalizes them away);
/// values that overflow `u32` are rejected — every legal pixel value fits.
fn parse_u32(input: &str) -> Option<u32> {
    if input.is_empty() || !input.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    input.parse().ok()
}

/// Strict non-negative decimal float per the spec's float rules: decimal
/// digits and at most one `.`, with digits on both sides (`0.9`, not `.9`
/// or `9.` or `+0.9` or `9e1`).
fn parse_f64(input: &str) -> Option<f64> {
    let valid = match input.split_once('.') {
        Some((int, frac)) => {
            !int.is_empty()
                && !frac.is_empty()
                && int.bytes().all(|byte| byte.is_ascii_digit())
                && frac.bytes().all(|byte| byte.is_ascii_digit())
        }
        None => !input.is_empty() && input.bytes().all(|byte| byte.is_ascii_digit()),
    };
    if !valid {
        return None;
    }
    // Grammar-valid decimal strings always parse; enormous ones saturate to
    // a finite f64 (no exponent syntax exists to produce inf/NaN spellings).
    let value: f64 = input.parse().ok()?;
    value.is_finite().then_some(value)
}

/// Print a float per the spec's canonical float rules: integer spelling if
/// the value is integral, otherwise shortest round-trip decimal.
///
/// Rust's `Display` for `f64` already implements exactly these rules:
/// shortest round-trip decimal, no exponent notation, integral values
/// without a trailing `.0`, sub-one values with the leading `0`, and never
/// a trailing zero.
pub(crate) fn fmt_f64(value: f64) -> String {
    format!("{value}")
}

impl Region {
    /// # Errors
    ///
    /// Returns a [`ParseError`] (HTTP 400) when the input is not a
    /// spec-legal region.
    #[inline]
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let err = || ParseError::new(Component::Region, input);
        match input {
            "full" => return Ok(Self::Full),
            "square" => return Ok(Self::Square),
            _ => {}
        }
        if let Some(rest) = input.strip_prefix("pct:") {
            let mut fields = rest.split(',');
            let (x, y, width, height) = (
                fields.next().and_then(parse_f64).ok_or_else(err)?,
                fields.next().and_then(parse_f64).ok_or_else(err)?,
                fields.next().and_then(parse_f64).ok_or_else(err)?,
                fields.next().and_then(parse_f64).ok_or_else(err)?,
            );
            if fields.next().is_some()
                || width <= 0.0_f64
                || height <= 0.0_f64
                || x >= 100.0_f64
                || y >= 100.0_f64
            {
                return Err(err());
            }
            return Ok(Self::Percent {
                x,
                y,
                width,
                height,
            });
        }
        let mut fields = input.split(',');
        let (x, y, width, height) = (
            fields.next().and_then(parse_u32).ok_or_else(err)?,
            fields.next().and_then(parse_u32).ok_or_else(err)?,
            fields.next().and_then(parse_u32).ok_or_else(err)?,
            fields.next().and_then(parse_u32).ok_or_else(err)?,
        );
        if fields.next().is_some() || width == 0 || height == 0 {
            return Err(err());
        }
        Ok(Self::Pixels {
            x,
            y,
            width,
            height,
        })
    }
}

impl fmt::Display for Region {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Full => f.write_str("full"),
            Self::Square => f.write_str("square"),
            Self::Pixels {
                x,
                y,
                width,
                height,
            } => write!(f, "{x},{y},{width},{height}"),
            Self::Percent {
                x,
                y,
                width,
                height,
            } => write!(
                f,
                "pct:{},{},{},{}",
                fmt_f64(x),
                fmt_f64(y),
                fmt_f64(width),
                fmt_f64(height)
            ),
        }
    }
}

impl Size {
    /// A size parameter from its two parts.
    ///
    /// A constructor rather than a struct literal because the type is
    /// `#[non_exhaustive]`; the parser is the usual producer, and this is
    /// for callers that build one directly.
    #[inline]
    #[must_use]
    pub const fn new(upscale: bool, kind: SizeKind) -> Self {
        Self { upscale, kind }
    }

    /// # Errors
    ///
    /// Returns a [`ParseError`] (HTTP 400) when the input is not a
    /// spec-legal size, including `pct:` values over 100 without the `^`
    /// upscale flag.
    #[inline]
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let err = || ParseError::new(Component::Size, input);
        let (upscale, rest) = input
            .strip_prefix('^')
            .map_or((false, input), |rest| (true, rest));
        let kind = if rest == "max" {
            SizeKind::Max
        } else if let Some(pct) = rest.strip_prefix("pct:") {
            let n = parse_f64(pct).ok_or_else(err)?;
            // Without `^`, pct > 100 would upscale — always a 400.
            if n <= 0.0_f64 || (!upscale && n > 100.0_f64) {
                return Err(err());
            }
            SizeKind::Percent(n)
        } else if let Some(confined) = rest.strip_prefix('!') {
            let (width_text, height_text) = confined.split_once(',').ok_or_else(err)?;
            let (width, height) = (
                parse_u32(width_text).ok_or_else(err)?,
                parse_u32(height_text).ok_or_else(err)?,
            );
            if width == 0 || height == 0 {
                return Err(err());
            }
            SizeKind::Confined(width, height)
        } else {
            let (width_text, height_text) = rest.split_once(',').ok_or_else(err)?;
            match (width_text.is_empty(), height_text.is_empty()) {
                (true, true) => return Err(err()),
                (false, true) => {
                    let width = parse_u32(width_text).ok_or_else(err)?;
                    if width == 0 {
                        return Err(err());
                    }
                    SizeKind::Width(width)
                }
                (true, false) => {
                    let height = parse_u32(height_text).ok_or_else(err)?;
                    if height == 0 {
                        return Err(err());
                    }
                    SizeKind::Height(height)
                }
                (false, false) => {
                    let (width, height) = (
                        parse_u32(width_text).ok_or_else(err)?,
                        parse_u32(height_text).ok_or_else(err)?,
                    );
                    if width == 0 || height == 0 {
                        return Err(err());
                    }
                    SizeKind::WidthHeight(width, height)
                }
            }
        };
        Ok(Self { upscale, kind })
    }
}

impl fmt::Display for Size {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.upscale {
            f.write_str("^")?;
        }
        match self.kind {
            SizeKind::Max => f.write_str("max"),
            SizeKind::Width(width) => write!(f, "{width},"),
            SizeKind::Height(height) => write!(f, ",{height}"),
            SizeKind::Percent(n) => write!(f, "pct:{}", fmt_f64(n)),
            SizeKind::WidthHeight(width, height) => write!(f, "{width},{height}"),
            SizeKind::Confined(width, height) => write!(f, "!{width},{height}"),
        }
    }
}

impl Rotation {
    /// A rotation parameter from its two parts.
    ///
    /// A constructor rather than a struct literal because the type is
    /// `#[non_exhaustive]`; the parser is the usual producer, and this is
    /// for callers that build one directly.
    #[inline]
    #[must_use]
    pub const fn new(mirror: bool, degrees: f64) -> Self {
        Self { mirror, degrees }
    }

    /// # Errors
    ///
    /// Returns a [`ParseError`] (HTTP 400) when the input is not a
    /// spec-legal rotation (optional `!`, then degrees in `0..=360`).
    #[inline]
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        let err = || ParseError::new(Component::Rotation, input);
        let (mirror, rest) = input
            .strip_prefix('!')
            .map_or((false, input), |rest| (true, rest));
        let degrees = parse_f64(rest).ok_or_else(err)?;
        // "any floating point number from 0 to 360" — inclusive.
        if degrees > 360.0_f64 {
            return Err(err());
        }
        Ok(Self { mirror, degrees })
    }

    /// Whether this rotation is a quarter-turn (0/90/180/270/360), the set
    /// core milestones implement natively; anything else is arbitrary
    /// rotation.
    #[must_use]
    #[inline]
    #[expect(
        clippy::modulo_arithmetic,
        reason = "the question IS whether the angle is a multiple of 90, \
                  and degrees are `0..=360` by construction."
    )]
    pub fn is_quarter_turn(&self) -> bool {
        self.degrees % 90.0 == 0.0
    }
}

impl fmt::Display for Rotation {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.mirror {
            f.write_str("!")?;
        }
        f.write_str(&fmt_f64(self.degrees))
    }
}

impl Quality {
    /// # Errors
    ///
    /// Returns a [`ParseError`] (HTTP 400) when the input is not one of
    /// the four v3 quality names.
    #[inline]
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        match input {
            "default" => Ok(Self::Default),
            "color" => Ok(Self::Color),
            "gray" => Ok(Self::Gray),
            "bitonal" => Ok(Self::Bitonal),
            _ => Err(ParseError::new(Component::Quality, input)),
        }
    }

    /// The spec's lowercase parameter spelling.
    #[must_use]
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Color => "color",
            Self::Gray => "gray",
            Self::Bitonal => "bitonal",
        }
    }
}

impl fmt::Display for Quality {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Format {
    /// # Errors
    ///
    /// Returns a [`ParseError`] (HTTP 400) when the input is not one of
    /// the seven spec-enumerated format names.
    #[inline]
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        match input {
            "jpg" => Ok(Self::Jpg),
            "tif" => Ok(Self::Tif),
            "png" => Ok(Self::Png),
            "gif" => Ok(Self::Gif),
            "jp2" => Ok(Self::Jp2),
            "pdf" => Ok(Self::Pdf),
            "webp" => Ok(Self::Webp),
            _ => Err(ParseError::new(Component::Format, input)),
        }
    }

    /// The spec's lowercase extension spelling.
    #[must_use]
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Jpg => "jpg",
            Self::Tif => "tif",
            Self::Png => "png",
            Self::Gif => "gif",
            Self::Jp2 => "jp2",
            Self::Pdf => "pdf",
            Self::Webp => "webp",
        }
    }

    /// The Content-Type this format is served with.
    #[must_use]
    #[inline]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::Jpg => "image/jpeg",
            Self::Tif => "image/tiff",
            Self::Png => "image/png",
            Self::Gif => "image/gif",
            Self::Jp2 => "image/jp2",
            Self::Pdf => "application/pdf",
            Self::Webp => "image/webp",
        }
    }
}

impl fmt::Display for Format {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ImageRequest {
    /// A complete request from its five parsed components, in the order
    /// they appear in the path.
    ///
    /// A constructor rather than a struct literal because the type is
    /// `#[non_exhaustive]`; [`ImageRequest::parse`] is the usual
    /// producer, and this is for callers that build one directly.
    #[inline]
    #[must_use]
    pub const fn new(
        region: Region,
        size: Size,
        rotation: Rotation,
        quality: Quality,
        format: Format,
    ) -> Self {
        Self {
            region,
            size,
            rotation,
            quality,
            format,
        }
    }

    /// Parse the four path segments after the identifier:
    /// `{region}/{size}/{rotation}/{quality}.{format}`.
    ///
    /// The input is the raw path suffix, already split from the identifier
    /// by the router; it must contain exactly three `/` separators.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] (HTTP 400) naming the first component that
    /// failed, or [`Component::Structure`] when the path shape itself is
    /// wrong.
    #[inline]
    pub fn parse(path: &str) -> Result<Self, ParseError> {
        let mut segs = path.split('/');
        let (region, size, rotation, last) = (
            segs.next()
                .ok_or_else(|| ParseError::new(Component::Structure, path))?,
            segs.next()
                .ok_or_else(|| ParseError::new(Component::Structure, path))?,
            segs.next()
                .ok_or_else(|| ParseError::new(Component::Structure, path))?,
            segs.next()
                .ok_or_else(|| ParseError::new(Component::Structure, path))?,
        );
        if segs.next().is_some() {
            return Err(ParseError::new(Component::Structure, path));
        }
        // rsplit: quality values never contain '.', so the first '.' from
        // the right separates quality from format.
        let (quality, format) = last
            .rsplit_once('.')
            .ok_or_else(|| ParseError::new(Component::Structure, path))?;
        Ok(Self {
            region: Region::parse(region)?,
            size: Size::parse(size)?,
            rotation: Rotation::parse(rotation)?,
            quality: Quality::parse(quality)?,
            format: Format::parse(format)?,
        })
    }
}

impl fmt::Display for ImageRequest {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}/{}/{}/{}.{}",
            self.region, self.size, self.rotation, self.quality, self.format
        )
    }
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::unwrap_used,
        clippy::missing_panics_doc,
        reason = "test code: a panic here IS the failure signal, not a crash \
                  path, so documenting one under `# Panics` would describe the \
                  mechanism a test works by"
    )]
    #![expect(
        clippy::shadow_unrelated,
        reason = "test code: rebinding `parsed`/`plan` down a short arrange \
                  -> act -> assert body keeps each assertion next to the value \
                  it is about; distinct names would number them instead"
    )]
    #![expect(
        clippy::inline_modules,
        reason = "a `#[cfg(test)] mod tests` beside its subject is how Rust \
                  unit tests are written, and moving it to its own file would \
                  put it outside the privacy boundary it exists to test"
    )]

    use super::*;

    #[track_caller]
    fn region(input: &str) -> Region {
        Region::parse(input).unwrap()
    }

    #[test]
    fn region_valid() {
        assert_eq!(region("full"), Region::Full);
        assert_eq!(region("square"), Region::Square);
        assert_eq!(
            region("0,0,1,1"),
            Region::Pixels {
                x: 0,
                y: 0,
                width: 1,
                height: 1
            }
        );
        assert_eq!(
            region("125,15,120,140"),
            Region::Pixels {
                x: 125,
                y: 15,
                width: 120,
                height: 140
            }
        );
        // Leading zeros are accepted; canonical print normalizes.
        assert_eq!(
            region("007,0,1,1"),
            Region::Pixels {
                x: 7,
                y: 0,
                width: 1,
                height: 1
            }
        );
        assert_eq!(
            region("pct:41.6,7.5,40,70"),
            Region::Percent {
                x: 41.6_f64,
                y: 7.5_f64,
                width: 40.0_f64,
                height: 70.0_f64
            }
        );
        assert_eq!(
            region("pct:0,0,100,100"),
            Region::Percent {
                x: 0.0_f64,
                y: 0.0_f64,
                width: 100.0_f64,
                height: 100.0_f64
            }
        );
    }

    #[test]
    fn region_invalid() {
        for input in [
            "",
            "fully",
            "Full",
            "FULL",
            " full",
            "full ",
            "1,2,3",
            "1,2,3,4,5",
            "1,2,3,",
            ",1,2,3",
            "-1,0,1,1",
            "0,0,0,1",
            "0,0,1,0",
            "1.5,0,1,1",
            "0x1,0,1,1",
            "pct:1,2,3",
            "pct:1,2,3,4,5",
            "pct:0,0,0,100",
            "pct:0,0,100,0",
            "pct:100,0,1,1",
            "pct:0,100,1,1",
            "pct:.5,0,1,1",
            "pct:0.,0,1,1",
            "pct:+1,0,1,1",
            "pct:1e2,0,1,1",
            "pct:NaN,0,1,1",
            "pct:inf,0,1,1",
            "4294967296,0,1,1",
            "pct:-0.1,0,1,1",
        ] {
            assert!(Region::parse(input).is_err(), "should reject {input:?}");
        }
    }

    #[track_caller]
    fn size(input: &str) -> Size {
        Size::parse(input).unwrap()
    }

    #[test]
    fn size_valid() {
        assert_eq!(
            size("max"),
            Size {
                upscale: false,
                kind: SizeKind::Max
            }
        );
        assert_eq!(
            size("^max"),
            Size {
                upscale: true,
                kind: SizeKind::Max
            }
        );
        assert_eq!(
            size("150,"),
            Size {
                upscale: false,
                kind: SizeKind::Width(150)
            }
        );
        assert_eq!(
            size("^360,"),
            Size {
                upscale: true,
                kind: SizeKind::Width(360)
            }
        );
        assert_eq!(
            size(",150"),
            Size {
                upscale: false,
                kind: SizeKind::Height(150)
            }
        );
        assert_eq!(
            size("pct:50"),
            Size {
                upscale: false,
                kind: SizeKind::Percent(50.0)
            }
        );
        assert_eq!(
            size("pct:100"),
            Size {
                upscale: false,
                kind: SizeKind::Percent(100.0)
            }
        );
        assert_eq!(
            size("^pct:120"),
            Size {
                upscale: true,
                kind: SizeKind::Percent(120.0)
            }
        );
        assert_eq!(
            size("225,100"),
            Size {
                upscale: false,
                kind: SizeKind::WidthHeight(225, 100)
            }
        );
        assert_eq!(
            size("!225,100"),
            Size {
                upscale: false,
                kind: SizeKind::Confined(225, 100)
            }
        );
        assert_eq!(
            size("^!360,360"),
            Size {
                upscale: true,
                kind: SizeKind::Confined(360, 360)
            }
        );
    }

    #[test]
    fn size_invalid() {
        for input in [
            "",
            "^",
            "full",
            "Max",
            "^^max",
            "pct:0",
            "pct:101",
            "pct:100.001",
            "pct:",
            "^pct:",
            "pct:-1",
            "pct:1e1",
            "0,",
            ",0",
            "0,100",
            "100,0",
            ",",
            "!,",
            "!100,",
            "!,100",
            "!0,1",
            "150",
            "^150",
            "150,,",
            ",,150",
            "1.5,",
            ",1.5",
            "%5Emax",
        ] {
            assert!(Size::parse(input).is_err(), "should reject {input:?}");
        }
    }

    #[test]
    fn rotation_valid() {
        assert_eq!(
            Rotation::parse("0").unwrap(),
            Rotation {
                mirror: false,
                degrees: 0.0
            }
        );
        assert_eq!(
            Rotation::parse("90").unwrap(),
            Rotation {
                mirror: false,
                degrees: 90.0
            }
        );
        assert_eq!(
            Rotation::parse("360").unwrap(),
            Rotation {
                mirror: false,
                degrees: 360.0
            }
        );
        assert_eq!(
            Rotation::parse("22.5").unwrap(),
            Rotation {
                mirror: false,
                degrees: 22.5
            }
        );
        assert_eq!(
            Rotation::parse("!0").unwrap(),
            Rotation {
                mirror: true,
                degrees: 0.0
            }
        );
        assert_eq!(
            Rotation::parse("!337.5").unwrap(),
            Rotation {
                mirror: true,
                degrees: 337.5
            }
        );
    }

    #[test]
    fn rotation_invalid() {
        for input in [
            "", "!", "-0", "-90", "360.001", "361", "1e2", ".5", "90.", "+90", "9O",
        ] {
            assert!(Rotation::parse(input).is_err(), "should reject {input:?}");
        }
    }

    #[test]
    fn quarter_turns() {
        for (input, expect) in [
            ("0", true),
            ("90", true),
            ("180", true),
            ("270", true),
            ("360", true),
            ("22.5", false),
            ("90.1", false),
        ] {
            assert_eq!(
                Rotation::parse(input).unwrap().is_quarter_turn(),
                expect,
                "{input}"
            );
        }
    }

    #[test]
    fn quality_and_format() {
        assert_eq!(Quality::parse("default").unwrap(), Quality::Default);
        assert_eq!(Quality::parse("bitonal").unwrap(), Quality::Bitonal);
        Quality::parse("native").unwrap_err(); // v2 name, not v3
        Quality::parse("grey").unwrap_err();
        assert_eq!(Format::parse("jpg").unwrap(), Format::Jpg);
        assert_eq!(Format::parse("webp").unwrap(), Format::Webp);
        Format::parse("jpeg").unwrap_err();
        Format::parse("tiff").unwrap_err();
        Format::parse("JPG").unwrap_err();
    }

    #[test]
    fn full_request() {
        let req = ImageRequest::parse("full/max/0/default.jpg").unwrap();
        assert_eq!(req.to_string(), "full/max/0/default.jpg");
        let req = ImageRequest::parse("125,15,120,140/90,/!345.3/gray.png").unwrap();
        assert_eq!(req.to_string(), "125,15,120,140/90,/!345.3/gray.png");
        for input in [
            "full/max/0/default",
            "full/max/0.jpg",
            "full/max/0/default.jpg/extra",
            "/full/max/0/default.jpg",
            "full/max/0/default.jpg.png",
        ] {
            assert!(
                ImageRequest::parse(input).is_err(),
                "should reject {input:?}"
            );
        }
    }

    #[test]
    fn float_canonical_print() {
        assert_eq!(fmt_f64(0.0), "0");
        assert_eq!(fmt_f64(90.0), "90");
        assert_eq!(fmt_f64(22.5), "22.5");
        assert_eq!(fmt_f64(0.9), "0.9");
        assert_eq!(fmt_f64(360.0), "360");
    }
}
