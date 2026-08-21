// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! M0 allocator bench: the concurrent decode→resize→encode workload that
//! musl's malloc is known to serialize on. Built twice — default allocator
//! and `--features mimalloc` — inside a musl container by
//! `scripts/spike_alloc.sh`; the deltas decide whether mimalloc ships.
//!
//! Environment: `ALLOC_BENCH_FIXTURE` (path to a pyramidal TIFF;
//! defaults to the spike1 4:2:0 fixture), `ALLOC_BENCH_THREADS`
//! (defaults to available parallelism), `ALLOC_BENCH_ITERS` (per
//! thread, defaults to 40).

#![expect(
    clippy::absolute_paths,
    clippy::arithmetic_side_effects,
    clippy::decimal_literal_representation,
    clippy::expect_used,
    clippy::float_arithmetic,
    clippy::integer_division_remainder_used,
    clippy::min_ident_chars,
    clippy::missing_assert_message,
    clippy::missing_docs_in_private_items,
    clippy::missing_panics_doc,
    clippy::print_stdout,
    clippy::shadow_same,
    clippy::single_call_fn,
    reason = "test and example code. A panic IS the failure signal, so \
              `# Panics` sections and assertion messages would describe \
              the mechanism the harness works by; fixtures are indexed and \
              scaled with arithmetic over constants in the file above them. \
              The crate under test is held to every one of these."
)]

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::{fs::File, time::Instant};

use iiif_core::{
    codec::{Master as _, TiffPyramid},
    eval::evaluate,
    grammar::ImageRequest,
    info::Limits,
    pipeline,
};

const LIMITS: Limits = Limits::new(8192, 8192, 67_108_864);

/// Deterministic per-iteration request mix: different regions and output
/// sizes so allocation patterns vary like real traffic.
fn request_for(iteration: usize) -> ImageRequest {
    let step = iteration % 6;
    let x = (iteration * 293) % 1536;
    let y = (iteration * 181) % 1024;
    let path = match step {
        0 => format!("{x},{y},512,512/256,/0/default.jpg"),
        1 => format!("{x},{y},256,256/128,/0/default.jpg"),
        2 => "full/512,/0/default.jpg".to_owned(),
        3 => format!("{x},{y},1024,768/300,/90/gray.jpg"),
        4 => "square/!400,400/0/default.png".to_owned(),
        _ => format!("{x},{y},640,480/320,/!0/bitonal.jpg"),
    };
    ImageRequest::parse(&path).expect("bench requests are valid")
}

fn main() {
    let fixture = std::env::var("ALLOC_BENCH_FIXTURE")
        .unwrap_or_else(|_| "tests/fixtures/generated/spike1_ycbcr420.tif".to_owned());
    let threads: usize = std::env::var("ALLOC_BENCH_THREADS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(4, core::num::NonZero::get));
    let iters: usize = std::env::var("ALLOC_BENCH_ITERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(40);

    let allocator = if cfg!(feature = "mimalloc") {
        "mimalloc"
    } else {
        "system/musl"
    };

    // Warm the page cache so the measurement is compute+alloc, not disk.
    let bytes = std::fs::read(&fixture).expect("fixture readable (run: task spike1)");
    drop(bytes);

    let started = Instant::now();
    std::thread::scope(|scope| {
        for thread in 0..threads {
            let fixture = &fixture;
            scope.spawn(move || {
                for i in 0..iters {
                    // Fresh open per request — the stateless serving
                    // pattern, and the allocation-heavy path.
                    let file = File::open(fixture).expect("open");
                    let mut tiff = TiffPyramid::open(file).expect("parse");
                    let (width, height) = tiff.dimensions();
                    let request = request_for(thread * iters + i);
                    let plan = evaluate(&request, width, height, LIMITS).expect("evaluate");
                    let encoded = pipeline::execute(&mut tiff, &plan).expect("pipeline");
                    assert!(!encoded.is_empty());
                }
            });
        }
    });
    let elapsed = started.elapsed();
    let total = threads * iters;
    let total_f = f64::from(u32::try_from(total).expect("bench sizes are small"));
    println!(
        "allocator={allocator} threads={threads} iters/thread={iters} total_ops={total} \
        wall={:.3}s ops/s={:.1}",
        elapsed.as_secs_f64(),
        total_f / elapsed.as_secs_f64()
    );
}
