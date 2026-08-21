// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! Hand-rolled Prometheus text exposition — the fixed, frozen metric set from
//! the design spec: request counts, latency histogram, worker-queue depth, 503
//! count, plus build info.
//!
//! Zero dependencies, zero knobs, permanent surface. `iiif_build_info` joined
//! that set when versioned artifacts started shipping: once a release can be
//! pulled by tag, "which build is actually running?" has to be answerable from
//! monitoring rather than by shelling into a container that has no shell. It
//! is metadata about the binary, not a feature — the frozen scope is intact.

use core::{
    fmt::{self, Write as _},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

/// Upper bounds (seconds) of the latency histogram buckets, plus +Inf.
const BUCKETS: [f64; 10] = [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 10.0];

/// Process-lifetime counters. All relaxed: metrics never synchronize
/// anything.
#[derive(Debug, Default)]
#[expect(
    clippy::module_name_repetitions,
    reason = "`metrics::Metrics` is the registry itself; `metrics::Registry` \
              would name a Prometheus concept this type is not, and the \
              crate calls it `Metrics` throughout."
)]
pub struct Metrics {
    /// info.json requests served, whichever API version.
    requests_info: AtomicU64,
    /// Image requests served, whichever API version.
    requests_image: AtomicU64,
    /// Everything else: health, metrics, redirects, unroutable paths.
    requests_other: AtomicU64,
    /// Responses with a 2xx status.
    responses_2xx: AtomicU64,
    /// Responses with a 3xx status.
    responses_3xx: AtomicU64,
    /// Responses with a 4xx status.
    responses_4xx: AtomicU64,
    /// Responses with a 5xx status.
    responses_5xx: AtomicU64,
    /// Requests refused by the admission bound, counted separately from
    /// the 5xx total so backpressure stays distinguishable from failure.
    overload_503: AtomicU64,
    /// Cumulative histogram counts, one per bound in [`BUCKETS`] plus a
    /// final `+Inf` bucket.
    latency_buckets: [AtomicU64; BUCKETS.len() + 1],
    /// Total observed latency in microseconds — the histogram's `_sum`.
    latency_sum_micros: AtomicU64,
    /// Number of observations — the histogram's `_count`.
    latency_count: AtomicU64,
}

/// Which request family a hit belongs to, for the counter labels.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum Family {
    /// info.json requests.
    Info,
    /// Image (pixel) requests.
    Image,
    /// Everything else (health, favicon, errors).
    Other,
}

impl Metrics {
    /// Record one finished request in its family's counters/histogram.
    #[inline]
    #[expect(
        clippy::integer_division,
        clippy::integer_division_remainder_used,
        clippy::indexing_slicing,
        reason = "`status / 100` IS the status class, and the bucket index \
                  comes from a search over BUCKETS so it is in range by \
                  construction. Both are the arithmetic the histogram is \
                  made of."
    )]
    pub fn observe(&self, family: Family, status: u16, elapsed: Duration) {
        match family {
            Family::Info => &self.requests_info,
            Family::Image => &self.requests_image,
            Family::Other => &self.requests_other,
        }
        .fetch_add(1, Ordering::Relaxed);
        match status / 100 {
            2 => &self.responses_2xx,
            3 => &self.responses_3xx,
            4 => &self.responses_4xx,
            _ => &self.responses_5xx,
        }
        .fetch_add(1, Ordering::Relaxed);
        if status == 503 {
            self.overload_503.fetch_add(1, Ordering::Relaxed);
        }
        let seconds = elapsed.as_secs_f64();
        let index = BUCKETS
            .iter()
            .position(|&bound| seconds <= bound)
            .unwrap_or(BUCKETS.len());
        self.latency_buckets[index].fetch_add(1, Ordering::Relaxed);
        let micros = u64::try_from(elapsed.as_micros()).unwrap_or(u64::MAX);
        self.latency_sum_micros.fetch_add(micros, Ordering::Relaxed);
        self.latency_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Render the Prometheus text format. `in_flight` and `queued` are
    /// point-in-time gauges the caller derives from its semaphores.
    #[must_use]
    #[inline]
    #[expect(
        clippy::integer_division,
        clippy::integer_division_remainder_used,
        clippy::indexing_slicing,
        clippy::arithmetic_side_effects,
        clippy::let_underscore_must_use,
        reason = "Prometheus wants the histogram `_sum` as whole seconds \
                  plus microseconds, which is what the division and \
                  remainder produce; the cumulative counts only ever add \
                  counters that already fit u64; and the `+Inf` bucket is \
                  indexed by a constant this module owns. The discarded \
                  `fmt::Result`s are `writeln!` into a `String`, which \
                  cannot fail — `drop()` is refused there in turn, because \
                  `fmt::Result` is `Copy` and dropping it is a no-op \
                  (`dropping_copy_types`, measured)."
    )]
    pub fn render(&self, in_flight: u64, queued: u64) -> String {
        let mut out = String::with_capacity(2048);
        // Build metadata as the conventional info-gauge: identity lives in the
        // labels, the value is always 1.
        let _: fmt::Result = writeln!(
            out,
            "# HELP iiif_build_info Build identity of the running binary; the value is always 1.\n\
            # TYPE iiif_build_info gauge\n\
            iiif_build_info{{version=\"{}\",revision=\"{}\"}} 1",
            crate::VERSION,
            crate::REVISION,
        );
        let _: fmt::Result = writeln!(
            out,
            "# HELP iiif_requests_total Requests received, by endpoint family.\n\
            # TYPE iiif_requests_total counter\n\
            iiif_requests_total{{family=\"info\"}} {}\n\
            iiif_requests_total{{family=\"image\"}} {}\n\
            iiif_requests_total{{family=\"other\"}} {}",
            self.requests_info.load(Ordering::Relaxed),
            self.requests_image.load(Ordering::Relaxed),
            self.requests_other.load(Ordering::Relaxed),
        );
        let _: fmt::Result = writeln!(
            out,
            "# HELP iiif_responses_total Responses sent, by status class.\n\
            # TYPE iiif_responses_total counter\n\
            iiif_responses_total{{class=\"2xx\"}} {}\n\
            iiif_responses_total{{class=\"3xx\"}} {}\n\
            iiif_responses_total{{class=\"4xx\"}} {}\n\
            iiif_responses_total{{class=\"5xx\"}} {}",
            self.responses_2xx.load(Ordering::Relaxed),
            self.responses_3xx.load(Ordering::Relaxed),
            self.responses_4xx.load(Ordering::Relaxed),
            self.responses_5xx.load(Ordering::Relaxed),
        );
        let _: fmt::Result = writeln!(
            out,
            "# HELP iiif_overload_total Requests shed with 503.\n\
            # TYPE iiif_overload_total counter\n\
            iiif_overload_total {}",
            self.overload_503.load(Ordering::Relaxed),
        );
        let _: fmt::Result = writeln!(
            out,
            "# HELP iiif_request_duration_seconds Request latency.\n\
            # TYPE iiif_request_duration_seconds histogram"
        );
        let mut cumulative = 0_u64;
        for (bound, bucket) in BUCKETS.iter().zip(&self.latency_buckets) {
            cumulative += bucket.load(Ordering::Relaxed);
            let _: fmt::Result = writeln!(
                out,
                "iiif_request_duration_seconds_bucket{{le=\"{bound}\"}} {cumulative}"
            );
        }
        cumulative += self.latency_buckets[BUCKETS.len()].load(Ordering::Relaxed);
        let _: fmt::Result = writeln!(
            out,
            "iiif_request_duration_seconds_bucket{{le=\"+Inf\"}} {cumulative}\n\
            iiif_request_duration_seconds_sum {}.{:06}\n\
            iiif_request_duration_seconds_count {}",
            self.latency_sum_micros.load(Ordering::Relaxed) / 1_000_000,
            self.latency_sum_micros.load(Ordering::Relaxed) % 1_000_000,
            self.latency_count.load(Ordering::Relaxed),
        );
        let _: fmt::Result = writeln!(
            out,
            "# HELP iiif_decode_in_flight Decodes currently executing.\n\
            # TYPE iiif_decode_in_flight gauge\n\
            iiif_decode_in_flight {in_flight}\n\
            # HELP iiif_decode_queued Requests admitted and waiting for a worker.\n\
            # TYPE iiif_decode_queued gauge\n\
            iiif_decode_queued {queued}"
        );
        out
    }
}
