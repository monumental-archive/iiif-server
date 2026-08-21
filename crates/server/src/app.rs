// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! The HTTP application: routing (the IIIF grammar *is* the router),
//! spec-mandated response semantics, CORS, content negotiation, and
//! backpressure.
//!
//! Pure request→response logic; `main.rs` owns sockets and runtime.

#![expect(
    clippy::std_instead_of_core,
    reason = "`core::io` is not stable on this toolchain — measured: the \
              suggestion is marked machine-applicable and does not compile \
              (E0658, `core_io`). Revisit when core::io stabilises."
)]
#![expect(
    clippy::pattern_type_mismatch,
    reason = "matches on a `&self` or `&err` receiver using default binding \
          modes, the edition-2021/2024 idiom. `match *self` does not \
          compile on these types — `error[E0507]: cannot move out of \
          a shared reference` — and what satisfies the lint is `ref` \
          bindings, the pre-2018 style default binding modes replaced."
)]
#![expect(
    clippy::single_call_fn,
    reason = "the response helpers and the route classifier are each called \
          once, from the one handler that needs them. Folding them into \
          it would put routing, CORS and every error mapping in one \
          body."
)]

extern crate alloc;

use alloc::sync::Arc;
use core::{
    fmt,
    hash::{Hash as _, Hasher as _},
};
use std::{
    collections::hash_map::DefaultHasher,
    fs,
    io::{self, Cursor, Read, Seek, SeekFrom},
    time::Instant,
};

use bytes::Bytes;
use http_body_util::Full;
use hyper::{
    Method, Request, Response, StatusCode,
    header::HOST,
    header::{
        ACCEPT, ALLOW, CACHE_CONTROL, CONTENT_TYPE, ETAG, HeaderValue, IF_NONE_MATCH, LINK,
        LOCATION, RETRY_AFTER, VARY,
    },
    http::Result as HttpResult,
};
use iiif_core::{
    codec::{CodecError, open_master},
    encode::EncodeError,
    eval::{EvalError, Plan, evaluate},
    grammar::{ImageRequest, ParseError},
    ident::Identifier,
    info::{Info, Limits},
    pipeline::{self, PipelineError},
    source::SourceError,
    v2,
};
use iiif_sources::{LocalFile, LocalRoot, ObjectRoot};
use tokio::{sync::Semaphore, task};

use crate::metrics::{Family, Metrics};

/// JSON-LD media type with the required profile parameter.
const LD_JSON: &str = "application/ld+json;profile=\"http://iiif.io/api/image/3/context.json\"";
/// The `Content-Type` a v2.1 info.json is served with.
const LD_JSON_V2: &str = "application/ld+json;profile=\"http://iiif.io/api/image/2/context.json\"";

/// M5 cache posture: strong validator (`ETag`) plus a modest freshness
/// window — the CDN/proxy in front owns long-lived caching policy; our
/// job is correct revalidation semantics.
const CACHE_CONTROL_VALUE: &str = "public, max-age=3600";

/// The M5 `ETag` definition.
///
/// A hash of (source identity, source version [mtime+size], canonical
/// request URI, binary version). Cheap, correct, no state. Two `DefaultHasher` passes with domain separation give 128
/// bits against accidental collision; `DefaultHasher::new()` is
/// deterministic across runs of the same binary, and the binary version
/// is part of the input.
fn etag_for(identifier: &str, source_version: (u64, u64), canonical: &str) -> String {
    let mut halves = [0_u64; 2];
    for (domain, half) in halves.iter_mut().enumerate() {
        let mut hasher = DefaultHasher::new();
        domain.hash(&mut hasher);
        identifier.hash(&mut hasher);
        source_version.hash(&mut hasher);
        canonical.hash(&mut hasher);
        env!("CARGO_PKG_VERSION").hash(&mut hasher);
        *half = hasher.finish();
    }
    format!("\"{:016x}{:016x}\"", halves[0], halves[1])
}

/// True when an `If-None-Match` header matches this `ETag` (or is `*`).
fn if_none_match_hits<B>(req: &Request<B>, etag: &str) -> bool {
    req.headers()
        .get(IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value == "*"
                || value
                    .split(',')
                    .any(|candidate| candidate.trim().trim_start_matches("W/") == etag)
        })
}

/// Finish a response builder whose inputs are statically valid.
///
/// The builder
/// only errors on malformed status codes or header values; every call site
/// passes constants or already-validated strings, so the fallback exists
/// purely to keep the server panic-free if that invariant ever breaks.
fn built(res: HttpResult<Response<Full<Bytes>>>) -> Response<Full<Bytes>> {
    res.unwrap_or_else(|_| {
        let mut fallback = Response::new(Full::new(Bytes::new()));
        *fallback.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        fallback
    })
}

/// A bare 304 carrying the `ETag` that matched.
fn not_modified(etag: &str) -> Response<Full<Bytes>> {
    built(
        Response::builder()
            .status(StatusCode::NOT_MODIFIED)
            .header(ETAG, etag)
            .header(CACHE_CONTROL, CACHE_CONTROL_VALUE)
            .body(Full::new(Bytes::new())),
    )
}

/// The compliance-level profile documents, sent as a Link header on every
/// image and info.json response (optional feature `profileLinkHeader`).
const PROFILE_LINK: &str = "<http://iiif.io/api/image/3/level2.json>;rel=\"profile\"";
/// The `Link: …; rel="profile"` header a v2.1 response carries.
const PROFILE_LINK_V2: &str = "<http://iiif.io/api/image/2/level2.json>;rel=\"profile\"";

/// Which API family a request addresses. Both mount over the same engine
/// (design spec: the v2.1 endpoint is a translation layer, M3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Version {
    /// Image API 2.1.
    V2,
    /// Image API 3.0.
    V3,
}

impl Version {
    /// The path segment this version is routed under.
    const fn prefix(self) -> &'static str {
        match self {
            Self::V2 => "iiif/2",
            Self::V3 => "iiif/3",
        }
    }

    /// The `rel="profile"` link header for this version.
    const fn profile_link(self) -> &'static str {
        match self {
            Self::V2 => PROFILE_LINK_V2,
            Self::V3 => PROFILE_LINK,
        }
    }

    /// The JSON-LD `Content-Type` for this version's info.json.
    const fn ld_json(self) -> &'static str {
        match self {
            Self::V2 => LD_JSON_V2,
            Self::V3 => LD_JSON,
        }
    }
}

/// Where masters come from: a local directory or an S3-compatible
/// object store — the prefix→root map whose default size is one.
#[derive(Debug)]
#[non_exhaustive]
pub enum SourceRoot {
    /// Masters under a local directory.
    Local(LocalRoot),
    /// Masters in an S3-compatible object store.
    Object(ObjectRoot),
}

/// One resolved master, ready for the sync decoder bridge.
enum Resolved {
    /// A file on disk, read by range on the blocking pool.
    Local(LocalFile),
    /// An object fetched whole, with its (mtime, length) version pair.
    Object(Bytes, (u64, u64)),
}

impl SourceRoot {
    /// Resolve an identifier against this root, whichever backend it is.
    ///
    /// # Errors
    ///
    /// [`SourceError::NotFound`] when the identifier resolves to nothing,
    /// and [`SourceError::Io`] for anything the backend could not read.
    async fn resolve(&self, id: &Identifier) -> Result<Resolved, SourceError> {
        match self {
            Self::Local(root) => root.resolve(id).map(Resolved::Local),
            Self::Object(root) => root
                .resolve(id)
                .await
                .map(|(bytes, version)| Resolved::Object(bytes, version)),
        }
    }
}

impl Resolved {
    /// The (mtime seconds, byte length) pair the `ETag` hashes.
    const fn source_version(&self) -> (u64, u64) {
        match self {
            Self::Local(file) => file.source_version(),
            Self::Object(_, version) => *version,
        }
    }

    /// Surrender a synchronous reader for the decoder bridge.
    ///
    /// # Errors
    ///
    /// Any [`io::Error`] from handing over the underlying file.
    fn into_reader(self) -> io::Result<SourceReader> {
        Ok(match self {
            Self::Local(file) => SourceReader::File(file.into_std_file()?),
            Self::Object(bytes, _) => SourceReader::Memory(Cursor::new(bytes)),
        })
    }
}

/// The sync `Read + Seek` bridge the codecs consume.
enum SourceReader {
    /// Reads go straight to the file handle.
    File(fs::File),
    /// Reads come from bytes already in memory.
    Memory(Cursor<Bytes>),
}

#[expect(
    clippy::missing_trait_methods,
    reason = "unsatisfiable on stable, measured with rustc: `read_buf` is \
          `error[E0658]: use of unstable library feature `read_buf``, and \
          its `BorrowedCursor` argument is `core_io_borrowed_buf`. The \
          provided methods this lint asks for are the ones std reserves \
          to itself; `read` and `seek` are the whole contract a caller \
          uses."
)]
impl Read for SourceReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::File(file) => Read::read(file, buf),
            Self::Memory(cursor) => Read::read(cursor, buf),
        }
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "unsatisfiable on stable, measured with rustc: `read_buf` is \
          `error[E0658]: use of unstable library feature `read_buf``, and \
          its `BorrowedCursor` argument is `core_io_borrowed_buf`. The \
          provided methods this lint asks for are the ones std reserves \
          to itself; `read` and `seek` are the whole contract a caller \
          uses."
)]
impl Seek for SourceReader {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match self {
            Self::File(file) => Seek::seek(file, pos),
            Self::Memory(cursor) => Seek::seek(cursor, pos),
        }
    }
}

/// Shared server state: source root, limits, and the bounded decode pool.
#[expect(
    clippy::module_name_repetitions,
    reason = "`app::App` is the application itself; the crate refers to it \
              as `App` everywhere and a synonym would name nothing."
)]
#[expect(
    clippy::exhaustive_structs,
    reason = "the application's own wiring, built exactly once by this \
              crate's binary from parsed CLI flags. `#[non_exhaustive]` \
              would buy nothing — there is no external constructor to \
              protect — and would cost an eight-parameter constructor in \
              place of a struct literal whose field names are the \
              documentation."
)]
pub struct App {
    /// Where masters are resolved from.
    pub root: SourceRoot,
    /// Deployment size limits, published in info.json and enforced.
    pub limits: Limits,
    /// Public `scheme://authority/prefix` used to build `id` values,
    /// derived from the request Host header when absent.
    pub public_base: Option<String>,
    /// Admission permits = workers + queue depth: a failed try-acquire
    /// means the queue is full → 503 with Retry-After.
    pub admission: Arc<Semaphore>,
    /// Execution permits = workers: bounds concurrent pixel work; waiting
    /// here is bounded because admission already capped the waiters.
    pub decode_permits: Arc<Semaphore>,
    /// Worker-pool sizing, kept for the queue-depth gauges.
    pub workers: usize,
    /// Admission queue length beyond the worker pool.
    pub queue_depth: usize,
    /// The frozen metric set (design spec, Observability).
    pub metrics: Arc<Metrics>,
}

impl fmt::Debug for App {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("App")
            .field("root", &self.root)
            .field("limits", &self.limits)
            .field("public_base", &self.public_base)
            .field("workers", &self.workers)
            .field("queue_depth", &self.queue_depth)
            .finish_non_exhaustive()
    }
}

impl App {
    /// Route and answer one request. Infallible at the HTTP layer: every
    /// failure becomes a spec-mandated status.
    ///
    /// # Panics
    ///
    /// Only if `hyper`'s response builder rejects statically valid
    /// header/status combinations — structurally impossible.
    #[inline]
    pub async fn handle<B>(self: Arc<Self>, req: Request<B>) -> Response<Full<Bytes>>
    where
        B: Send + Sync,
    {
        let started = Instant::now();
        #[expect(
            clippy::wildcard_enum_match_arm,
            reason = "only the two counted families have distinct labels; \
                      every other route is `Other` by definition, and \
                      naming them would be a list to keep in step with the \
                      router for no gain."
        )]
        let family = match Route::of(req.uri().path()) {
            Route::InfoJson { .. } => Family::Info,
            Route::Image { .. } => Family::Image,
            _ => Family::Other,
        };
        let response = self.handle_inner(req).await;
        self.metrics
            .observe(family, response.status().as_u16(), started.elapsed());
        response
    }

    /// Route and serve one request. The outer `handle` wraps this with
    /// metrics and CORS so every exit path is counted exactly once.
    async fn handle_inner<B>(self: &Arc<Self>, req: Request<B>) -> Response<Full<Bytes>>
    where
        B: Send + Sync,
    {
        let method = req.method().clone();
        if method == Method::OPTIONS {
            return preflight();
        }
        if !matches!(method, Method::GET | Method::HEAD) {
            return error(StatusCode::METHOD_NOT_ALLOWED, "only GET and HEAD");
        }
        let path = req.uri().path().to_owned();
        let mut response = match Route::of(&path) {
            Route::Health => built(
                Response::builder()
                    .status(StatusCode::OK)
                    .header(CONTENT_TYPE, "text/plain")
                    .body(Full::new(Bytes::from_static(b"ok\n"))),
            ),
            #[expect(
                clippy::arithmetic_side_effects,
                clippy::as_conversions,
                reason = "`workers + queue_depth` is the admission bound this \
                          server configured and validated at startup, and the \
                          gauges widen `usize` to `u64` for the Prometheus \
                          text format — lossless on every target this ships \
                          to."
            )]
            Route::Metrics => {
                let in_flight = self
                    .workers
                    .saturating_sub(self.decode_permits.available_permits());
                let admitted = (self.workers + self.queue_depth)
                    .saturating_sub(self.admission.available_permits());
                let queued = admitted.saturating_sub(in_flight);
                let body = self.metrics.render(in_flight as u64, queued as u64);
                built(
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(CONTENT_TYPE, "text/plain; version=0.0.4")
                        .body(Full::new(Bytes::from(body))),
                )
            }
            Route::BaseRedirect {
                version,
                identifier,
            } => Identifier::decode(identifier).map_or_else(
                |_| error(StatusCode::NOT_FOUND, "unknown identifier"),
                |id| {
                    built(
                        Response::builder()
                            .status(StatusCode::SEE_OTHER)
                            .header(
                                LOCATION,
                                format!("/{}/{}/info.json", version.prefix(), id.encoded()),
                            )
                            .body(Full::new(Bytes::new())),
                    )
                },
            ),
            Route::InfoJson {
                version,
                identifier,
            } => self.info_json(version, identifier, &req).await,
            Route::Image {
                version,
                identifier,
                rest,
            } => {
                let base = self.base_uri(&req);
                let if_none_match = req
                    .headers()
                    .get(IF_NONE_MATCH)
                    .and_then(|value| value.to_str().ok())
                    .map(str::to_owned);
                self.image(version, identifier, rest, &base, if_none_match)
                    .await
            }
            Route::None => error(StatusCode::NOT_FOUND, "no such resource"),
        };
        add_cors(&mut response);
        if method == Method::HEAD {
            *response.body_mut() = Full::new(Bytes::new());
        }
        response
    }

    /// Serve an info.json for one identifier, at either API version.
    async fn info_json<B>(
        &self,
        version: Version,
        raw_id: &str,
        req: &Request<B>,
    ) -> Response<Full<Bytes>>
    where
        B: Sync,
    {
        let Ok(id) = Identifier::decode(raw_id) else {
            return error(StatusCode::NOT_FOUND, "unknown identifier");
        };
        let source = match self.root.resolve(&id).await {
            Ok(source) => source,
            Err(err) => return source_error(&err),
        };
        // ETag first: a revalidation hit never opens the master at all.
        let etag = etag_for(
            id.as_path(),
            source.source_version(),
            &format!("{}/info.json", version.prefix()),
        );
        if if_none_match_hits(req, &etag) {
            return not_modified(&etag);
        }
        let opened = task::spawn_blocking(move || {
            let reader = source
                .into_reader()
                .map_err(|err| CodecError::Corrupt(format!("source handle: {err}")))?;
            open_master(reader).map(|master| master.describe())
        })
        .await;
        let description = match opened {
            Ok(Ok(description)) => description,
            Ok(Err(err)) => return codec_error(&err),
            Err(_) => return error(StatusCode::INTERNAL_SERVER_ERROR, "decode task failed"),
        };
        let base = self.base_uri(req);
        let document_id = format!("{base}/{}/{}", version.prefix(), id.encoded());
        let body = match version {
            Version::V3 => Info::new(document_id, &description, self.limits).to_json(),
            Version::V2 => v2::info_json(&document_id, &description, self.limits),
        };
        // Content negotiation (§5.2): ld+json when asked for, with Vary.
        let accept = req
            .headers()
            .get(ACCEPT)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        let content_type = if accept.contains("application/ld+json") {
            version.ld_json()
        } else {
            "application/json"
        };
        built(
            Response::builder()
                .status(StatusCode::OK)
                .header(CONTENT_TYPE, content_type)
                .header(VARY, "Accept")
                .header(LINK, version.profile_link())
                .header(ETAG, etag)
                .header(CACHE_CONTROL, CACHE_CONTROL_VALUE)
                .body(Full::new(Bytes::from(body))),
        )
    }

    /// Serve one image request: resolve, decode, transform, encode.
    async fn image(
        &self,
        version: Version,
        raw_id: &str,
        rest: &str,
        base: &str,
        if_none_match: Option<String>,
    ) -> Response<Full<Bytes>> {
        let Ok(id) = Identifier::decode(raw_id) else {
            return error(StatusCode::NOT_FOUND, "unknown identifier");
        };
        let (request, v2_spelling) = match version {
            Version::V3 => match ImageRequest::parse(rest) {
                Ok(request) => (request, None),
                Err(err) => return parse_error(&err),
            },
            Version::V2 => match v2::parse_image_request(rest) {
                Ok(parsed) => (parsed.as_v3, Some(parsed)),
                Err(err) => return parse_error(&err),
            },
        };
        let source = match self.root.resolve(&id).await {
            Ok(source) => source,
            Err(err) => return source_error(&err),
        };
        // Backpressure: admission bounds the queue (full → 503),
        // execution bounds concurrent decode work.
        let Ok(admission) = Arc::clone(&self.admission).try_acquire_owned() else {
            return overloaded();
        };
        let Ok(permit) = Arc::clone(&self.decode_permits).acquire_owned().await else {
            return error(StatusCode::INTERNAL_SERVER_ERROR, "pool closed");
        };
        let limits = self.limits;
        // Spare workers → let codecs use their own internal parallelism.
        let pool_idle = self.decode_permits.available_permits() > 0;
        let source_version = source.source_version();
        let identifier_path = id.as_path().to_owned();
        let result = task::spawn_blocking(move || {
            let _permit = permit; // held for the duration of the decode
            let _admission = admission;
            let reader = source.into_reader().map_err(|err| {
                ImageFailure::Codec(CodecError::Corrupt(format!("source handle: {err}")))
            })?;
            let mut master = open_master(reader)?;
            master.set_internal_parallelism(pool_idle);
            let (full_w, full_h) = master.dimensions();
            let plan = evaluate(&request, full_w, full_h, limits).map_err(ImageFailure::Eval)?;
            let canonical_path = v2_spelling
                .as_ref()
                .map_or_else(|| plan.canonical_path(), |v2| v2::canonical_path(&plan, v2));
            // ETag is derived from the canonical URI, so every spelling of
            // the same request revalidates against the same tag — and a
            // hit skips all pixel work.
            let etag = etag_for(&identifier_path, source_version, &canonical_path);
            if let Some(candidates) = &if_none_match
                && (candidates == "*"
                    || candidates
                        .split(',')
                        .any(|candidate| candidate.trim().trim_start_matches("W/") == etag))
            {
                return Ok(ImageOutcome::NotModified { etag });
            }
            let bytes =
                pipeline::execute(master.as_mut(), &plan).map_err(ImageFailure::Pipeline)?;
            Ok::<_, ImageFailure>(ImageOutcome::Fresh {
                bytes,
                plan,
                canonical_path,
                etag,
            })
        })
        .await;
        match result {
            Ok(Ok(ImageOutcome::NotModified { etag })) => not_modified(&etag),
            Ok(Ok(ImageOutcome::Fresh {
                bytes,
                plan,
                canonical_path,
                etag,
            })) => {
                // Optional features `canonicalLinkHeader` + `profileLinkHeader`.
                let canonical = format!(
                    "<{base}/{}/{}/{canonical_path}>;rel=\"canonical\", {}",
                    version.prefix(),
                    id.encoded(),
                    version.profile_link()
                );
                built(
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(CONTENT_TYPE, plan.format.media_type())
                        .header(LINK, canonical)
                        .header(ETAG, etag)
                        .header(CACHE_CONTROL, CACHE_CONTROL_VALUE)
                        .body(Full::new(Bytes::from(bytes))),
                )
            }
            Ok(Err(failure)) => failure.into_response(),
            Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "decode task failed"),
        }
    }

    /// The scheme+authority the response's `id`/`@id` are built from —
    /// `--public-base` when set, otherwise the request's `Host`.
    fn base_uri<B>(&self, req: &Request<B>) -> String {
        if let Some(base) = &self.public_base {
            return base.clone();
        }
        let host = req
            .headers()
            .get(HOST)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("localhost");
        format!("http://{host}")
    }
}

/// What the blocking image task produced.
enum ImageOutcome {
    /// Revalidation hit: no pixel work was done.
    NotModified {
        /// The `ETag` the client's `If-None-Match` matched.
        etag: String,
    },
    /// The image was produced.
    Fresh {
        /// The encoded image.
        bytes: Vec<u8>,
        /// The plan that produced it, for the canonical link header.
        plan: Plan,
        /// The spec's canonical form of this request.
        canonical_path: String,
        /// The `ETag` for these bytes.
        etag: String,
    },
}

/// Failures on the image path, unified for status mapping.
enum ImageFailure {
    /// The master could not be opened or decoded.
    Codec(CodecError),
    /// The request is legal syntax but not against this image.
    Eval(EvalError),
    /// Decode, transform or encode failed.
    Pipeline(PipelineError),
}

impl From<CodecError> for ImageFailure {
    fn from(err: CodecError) -> Self {
        Self::Codec(err)
    }
}

impl ImageFailure {
    /// Map the failure onto its spec-mandated status and message.
    fn into_response(self) -> Response<Full<Bytes>> {
        match self {
            Self::Eval(err) => error(StatusCode::BAD_REQUEST, &err.to_string()),
            Self::Codec(err) => codec_error(&err),
            Self::Pipeline(PipelineError::Encode(EncodeError::DimensionsBeyondFormat {
                ..
            })) => error(StatusCode::BAD_REQUEST, "output too large for this format"),
            Self::Pipeline(err) => error(StatusCode::INTERNAL_SERVER_ERROR, &err.to_string()),
        }
    }
}

/// The resource shapes under `/iiif/3/` and `/iiif/2/`.
enum Route<'path> {
    /// `GET /healthz`.
    Health,
    /// `GET /metrics`.
    Metrics,
    /// A bare identifier, which the spec redirects to its info.json.
    BaseRedirect {
        /// Which API version the path was routed under.
        version: Version,
        /// The percent-encoded identifier, still unvalidated.
        identifier: &'path str,
    },
    /// An info.json request.
    InfoJson {
        /// Which API version the path was routed under.
        version: Version,
        /// The percent-encoded identifier, still unvalidated.
        identifier: &'path str,
    },
    /// An image request; `rest` is the unparsed IIIF parameter path.
    Image {
        /// Which API version the path was routed under.
        version: Version,
        /// The percent-encoded identifier, still unvalidated.
        identifier: &'path str,
        /// The remaining `{region}/{size}/{rotation}/{quality}.{format}`.
        rest: &'path str,
    },
    /// Nothing this server routes.
    None,
}

impl<'path> Route<'path> {
    /// Classify a request path. The IIIF grammar IS the router, so
    /// this is the whole routing table.
    fn of(path: &'path str) -> Self {
        if path == "/healthz" {
            return Self::Health;
        }
        if path == "/metrics" {
            return Self::Metrics;
        }
        let (version, rest) = if let Some(rest) = path.strip_prefix("/iiif/3/") {
            (Version::V3, rest)
        } else if let Some(rest) = path.strip_prefix("/iiif/2/") {
            (Version::V2, rest)
        } else {
            return Self::None;
        };
        // Exact segment shapes only. An identifier containing a raw
        // (unescaped) slash changes the segment count and falls through
        // to 404, as the spec requires for unescaped special characters.
        let segments: Vec<&str> = rest.split('/').collect();
        match segments.as_slice() {
            [identifier] if !identifier.is_empty() => Self::BaseRedirect {
                version,
                identifier,
            },
            [identifier, "info.json"] => Self::InfoJson {
                version,
                identifier,
            },
            #[expect(
                clippy::string_slice,
                clippy::arithmetic_side_effects,
                reason = "`rest` was split on '/' to produce `identifier`, so \
                          `identifier.len() + 1` is a character boundary in \
                          `rest` by construction — the slice cannot panic and \
                          the addition cannot overflow a path that already \
                          fits in memory."
            )]
            [identifier, _region, _size, _rotation, _file] => Self::Image {
                version,
                identifier,
                rest: &rest[identifier.len() + 1..],
            },
            _ => Self::None,
        }
    }
}

/// Add the CORS headers every response carries, per the spec.
fn add_cors(response: &mut Response<Full<Bytes>>) {
    response
        .headers_mut()
        .insert("access-control-allow-origin", HeaderValue::from_static("*"));
}

/// The `OPTIONS` preflight response.
fn preflight() -> Response<Full<Bytes>> {
    let mut response = built(
        Response::builder()
            .status(StatusCode::NO_CONTENT)
            .header(ALLOW, "GET, HEAD, OPTIONS")
            .header("access-control-allow-methods", "GET, HEAD, OPTIONS")
            .header("access-control-allow-headers", "Accept")
            .header("access-control-max-age", "86400")
            .body(Full::new(Bytes::new())),
    );
    add_cors(&mut response);
    response
}

/// A plain-text error response.
fn error(status: StatusCode, message: &str) -> Response<Full<Bytes>> {
    built(
        Response::builder()
            .status(status)
            .header(CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Full::new(Bytes::from(format!("{message}\n")))),
    )
}

/// The 503 the admission bound returns, with `Retry-After`.
fn overloaded() -> Response<Full<Bytes>> {
    let mut response = error(StatusCode::SERVICE_UNAVAILABLE, "decode pool saturated");
    response
        .headers_mut()
        .insert(RETRY_AFTER, HeaderValue::from_static("2"));
    response
}

/// Map a grammar failure onto its 400.
fn parse_error(err: &ParseError) -> Response<Full<Bytes>> {
    error(StatusCode::BAD_REQUEST, &err.to_string())
}

/// Map a source failure onto 404 or 500.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "only NotFound has a distinct status; everything else a source \
              can fail with is a 500 to the client, and enumerating them \
              would leak backend detail into the response mapping."
)]
fn source_error(err: &SourceError) -> Response<Full<Bytes>> {
    match err {
        SourceError::NotFound => error(StatusCode::NOT_FOUND, "unknown identifier"),
        _ => error(StatusCode::INTERNAL_SERVER_ERROR, "source read failed"),
    }
}

/// Map a codec failure onto its status; a ceiling refusal is 403.
fn codec_error(err: &CodecError) -> Response<Full<Bytes>> {
    match err {
        // An operator-side master problem: the identifier exists but is
        // outside the supported matrix. 500-class (the client did nothing
        // wrong), message actionable.
        CodecError::Unsupported(msg) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("unsupported master: {msg}"),
        ),
        CodecError::Corrupt(msg) => error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("corrupt master: {msg}"),
        ),
        // A deliberate resource-ceiling refusal: the master is fine and
        // the server is healthy, so a 500 would misdirect both operators
        // and monitoring. 403 is the Image API's status for a refused
        // operation (also the incumbent's answer for its max_pixels
        // ceiling); the message carries the conversion advice.
        CodecError::LimitExceeded(msg) => {
            error(StatusCode::FORBIDDEN, &format!("limit exceeded: {msg}"))
        }
        // Raster failures and any future variant are both "the pipeline
        // could not produce an image", which is a 500 either way.
        CodecError::Raster(_) | _ => error(StatusCode::INTERNAL_SERVER_ERROR, "pipeline failure"),
    }
}
