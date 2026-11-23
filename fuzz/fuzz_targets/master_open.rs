// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! Every decoder boundary: hostile bytes may be rejected, never panic.
//! Successful opens additionally serve one bounded crop through the full
//! pipeline (decode → resize → encode).
#![no_main]
#![expect(
    clippy::std_instead_of_core,
    reason = "open_master needs Read + Seek over the fuzz input; the \
              core::io::Cursor the lint suggests is unstable (core_io), \
              and the lint ignores an expectation on the use item"
)]

use std::io::Cursor;

use iiif_core::codec::open_master;
use iiif_core::eval::evaluate;
use iiif_core::grammar::ImageRequest;
use iiif_core::info::Limits;
use iiif_core::pipeline::execute;
use libfuzzer_sys::fuzz_target;

/// Pixel budget for one fuzzed master; the decompression-bomb guard
/// mirrors the server's limits.
const MAX_PIXELS: u64 = 16_000_000;

/// The crop's width and height bound, and the area it implies.
const MAX_EDGE: u32 = 512;

fuzz_target!(|data: &[u8]| {
    let Ok(mut master) = open_master(Cursor::new(data.to_vec())) else {
        return;
    };
    let (width, height) = master.dimensions();
    let too_big = u64::from(width)
        .checked_mul(u64::from(height))
        .is_none_or(|pixels| pixels > MAX_PIXELS);
    if width == 0 || height == 0 || too_big {
        return;
    }
    drop(master.describe());
    let limits = Limits::new(MAX_EDGE, MAX_EDGE, u64::from(MAX_EDGE).pow(2));
    let Ok(request) = ImageRequest::parse("full/!64,64/90/gray.png") else {
        return;
    };
    if let Ok(plan) = evaluate(&request, width, height, limits) {
        drop(execute(master.as_mut(), &plan));
    }
});
