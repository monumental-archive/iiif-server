// SPDX-FileCopyrightText: 2026 Carl Allen
// SPDX-License-Identifier: AGPL-3.0-only

//! The server binary. hyper 1.x directly on tokio — no web framework; the
//! IIIF grammar in `iiif-core` *is* the router.
//!
//! Near-zero config: `iiif-server serve ./images` just works. The only
//! deployment-varying values are the numeric limits and pool sizing.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "server binary's check subcommand is a CLI: stdout is output, stderr diagnostics"
)]
#![expect(
    clippy::std_instead_of_core,
    reason = "`core::io` and friends are not stable on this toolchain; the \
              suggestion does not compile (E0658, `core_io`)."
)]
#![expect(
    clippy::single_call_fn,
    reason = "each is a named step of `main`'s dispatch — one per \
              subcommand, one for the shutdown future, one for the arg \
              parse. Folding them into `main` would put the whole binary \
              in one body."
)]

extern crate alloc;

use hyper::{server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;
use iiif_core::{codec::open_master, info::Limits};
use iiif_server::{
    app::{App, SourceRoot},
    metrics::Metrics,
};

/// Bench-decided allocator (docs/spikes/alloc-bench.md): musl's malloc
/// contends badly under concurrent decode; mimalloc measured ~2×.
#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
use alloc::sync::Arc;
use core::{convert::Infallible, future, num::NonZero, time::Duration};
use std::{env, fs, net::SocketAddr, path::Path, process::ExitCode, thread};

use iiif_sources::LocalRoot;
use tokio::{
    io::{AsyncReadExt as _, AsyncWriteExt as _},
    net::{TcpListener, TcpStream},
    runtime,
    signal::{
        ctrl_c,
        unix::{SignalKind, signal},
    },
    sync::{Semaphore, mpsc, watch},
    time::timeout,
};
use tracing::{error, info};

/// Deployment knobs, all optional. Parsed by hand: seven flags do not
/// justify a dependency.
struct Config {
    /// The image root: a local path, or an `s3://bucket/prefix` URL.
    root: String,
    /// Listen address.
    bind: SocketAddr,
    /// Published and enforced maximum output width.
    max_width: u32,
    /// Published and enforced maximum output height.
    max_height: u32,
    /// Published and enforced maximum output area.
    max_area: u64,
    /// Concurrent decode bound — the size of the decode pool.
    workers: usize,
    /// Admitted waiters beyond the workers; overflow answers 503.
    queue_depth: usize,
    /// Scheme+authority for `id`/`@id`, when the `Host` header is wrong
    /// (behind a proxy that does not preserve it).
    public_base: Option<String>,
    /// S3-compatible endpoint URL, for non-AWS object stores.
    endpoint: Option<String>,
}

/// The `--help` text, also printed beside a flag error.
const USAGE: &str = "usage: iiif-server serve <root> [--bind ADDR] [--max-width N] \
[--max-height N] [--max-area N] [--workers N] [--queue-depth N] [--public-base URL] \
[--endpoint URL]
    iiif-server check <file-or-directory>
    iiif-server healthcheck [ADDR]
    iiif-server --version | --help

<root> is a local directory or s3://bucket/prefix (credentials from the
environment; --endpoint for S3-compatible services). `check` inspects
masters offline and prints serving advice. `healthcheck` probes a running
server's /healthz (default 127.0.0.1:6363) and exits 0 when it answers 200 \u{2014}
it exists so the container image, which holds nothing but this binary, can
still declare a HEALTHCHECK.";

/// How long the self-probe waits for the whole exchange. Deliberately short:
/// the orchestrator's own healthcheck timeout is the real budget, and a probe
/// that outlives it is worse than one that fails.
const HEALTHCHECK_TIMEOUT: Duration = Duration::from_secs(3);

/// Ceiling on draining in-flight connections at shutdown.
///
/// Keep-alive means a
/// polite client can otherwise hold a connection open indefinitely, so the
/// drain cannot be unbounded; Docker's default stop timeout is 10s and
/// Kubernetes' default grace period 30s, so this expires first and the exit
/// stays ours rather than becoming a SIGKILL.
const DRAIN_TIMEOUT: Duration = Duration::from_secs(8);

/// `iiif-server --version`: the version, plus the build revision when the
/// release pipeline injected one.
fn version_line() -> String {
    if iiif_server::REVISION == "unknown" {
        format!("iiif-server {}", iiif_server::VERSION)
    } else {
        format!(
            "iiif-server {} ({})",
            iiif_server::VERSION,
            iiif_server::REVISION
        )
    }
}

/// `iiif-server healthcheck [ADDR]`.
///
/// One HTTP/1.1 GET of `/healthz` over a raw
/// socket, no client dependency. The image is `FROM scratch` — there is no
/// shell and no curl to call, so the binary probes itself.
///
/// # Errors
///
/// A message describing why the probe failed: no connection, no answer
/// inside [`HEALTHCHECK_TIMEOUT`], or a status line that is not 200.
#[expect(
    clippy::indexing_slicing,
    clippy::arithmetic_side_effects,
    clippy::shadow_reuse,
    reason = "reads into a fixed 256-byte buffer whose `filled` cursor is \
              bounded by its own length at every step, so the slices cannot \
              be out of range and the total cannot overflow. The status \
              line is narrowed from the whole buffer to its first line, \
              which is the same value refined."
)]
async fn probe_health(addr: SocketAddr) -> Result<(), String> {
    let mut stream = TcpStream::connect(addr)
        .await
        .map_err(|err| format!("connect {addr}: {err}"))?;
    let request = format!(
        "GET /healthz HTTP/1.1\r\nHost: {addr}\r\nUser-Agent: iiif-server-healthcheck\r\n\
        Connection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .map_err(|err| format!("write {addr}: {err}"))?;
    // The status line is all that matters, and it arrives first; read until
    // the end of it rather than draining a body we will not look at.
    let mut buffer = [0_u8; 128];
    let mut filled = 0;
    while filled < buffer.len() {
        let read = stream
            .read(&mut buffer[filled..])
            .await
            .map_err(|err| format!("read {addr}: {err}"))?;
        if read == 0 {
            break;
        }
        filled += read;
        if buffer[..filled].contains(&b'\n') {
            break;
        }
    }
    let status_line = String::from_utf8_lossy(&buffer[..filled]);
    let status_line = status_line.lines().next().unwrap_or_default();
    // "HTTP/1.1 200 OK" — the code is the second token.
    match status_line.split_whitespace().nth(1) {
        Some("200") => Ok(()),
        Some(other) => Err(format!("/healthz answered {other}")),
        None => Err("no status line in response".to_owned()),
    }
}

/// Resolves when the process is asked to stop.
///
/// SIGTERM is what orchestrators
/// send, and it matters more than it looks: as PID 1 in a container a process
/// receives no default signal disposition, so an unhandled SIGTERM is ignored
/// outright and every `docker stop` becomes a ten-second wait ending in
/// SIGKILL.
#[expect(
    clippy::integer_division_remainder_used,
    reason = "`tokio::select!` expands to arithmetic over its branch \
              index; none of it is this function's own."
)]
async fn shutdown_signal() {
    #[cfg(unix)]
    let terminate = async {
        match signal(SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(err) => {
                error!("SIGTERM handler registration failed, falling back to SIGINT only: {err}");
                future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    let interrupt = async {
        if let Err(err) = ctrl_c().await {
            error!("interrupt handler registration failed: {err}");
            future::pending::<()>().await;
        }
    };

    tokio::select! {
        () = terminate => {},
        () = interrupt => {},
    }
}

/// `iiif-server check <path>`: offline master inspection — the operator
/// tool that turns serving-time surprises into setup-time advice.
#[expect(
    clippy::arithmetic_side_effects,
    clippy::indexing_slicing,
    reason = "counters over a directory listing, and `tiles[0]` is the \
              base level every `ImageDescription` carries by construction."
)]
fn run_check(path: &Path) -> ExitCode {
    let mut failures = 0_u32;
    let mut checked = 0_u32;
    let mut walk = vec![path.to_path_buf()];
    while let Some(entry) = walk.pop() {
        if entry.is_dir() {
            match fs::read_dir(&entry) {
                Ok(children) => {
                    for child in children.flatten() {
                        walk.push(child.path());
                    }
                }
                Err(err) => {
                    eprintln!("{}: unreadable directory: {err}", entry.display());
                    failures += 1;
                }
            }
            continue;
        }
        checked += 1;
        let opened = fs::File::open(&entry)
            .map_err(|err| format!("unreadable: {err}"))
            .and_then(|file| open_master(file).map_err(|err| err.to_string()));
        match opened {
            Ok(master) => {
                let (width, height) = master.dimensions();
                let description = master.describe();
                let structure = if description.tiles.is_empty() {
                    "untiled".to_owned()
                } else {
                    format!(
                        "{}px tiles, scale factors {:?}",
                        description.tiles[0].width, description.tiles[0].scale_factors
                    )
                };
                println!("{}: OK — {width}×{height}, {structure}", entry.display());
                for advisory in master.advisories() {
                    println!("  advice: {advisory}");
                }
            }
            Err(message) => {
                println!("{}: REJECTED — {message}", entry.display());
                failures += 1;
            }
        }
    }
    println!("checked {checked} file(s), {failures} rejected");
    if failures == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Parse the flags after the subcommand into a [`Config`].
///
/// # Errors
///
/// A message naming the offending flag, ready to print beside [`USAGE`].
fn parse_args(args: &[String]) -> Result<Config, String> {
    let mut it = args.iter();
    match it.next().map(String::as_str) {
        Some("serve") => {}
        _ => return Err(USAGE.to_owned()),
    }
    let root = it.next().ok_or(USAGE)?.clone();
    let mut config = Config {
        root,
        bind: SocketAddr::from(([127, 0, 0, 1], 6363)),
        max_width: 8192,
        max_height: 8192,
        #[expect(
            clippy::decimal_literal_representation,
            reason = "32 megapixels, and the comment says so. `0x0200_0000` \
                      would hide the one property a reader checks."
        )]
        max_area: 33_554_432, // 32 megapixels
        workers: thread::available_parallelism().map_or(4, NonZero::get),
        queue_depth: 64,
        public_base: None,
        endpoint: None,
    };
    while let Some(flag) = it.next() {
        let value = it.next().ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--bind" => config.bind = value.parse().map_err(|err| format!("--bind: {err}"))?,
            "--max-width" => {
                config.max_width = value.parse().map_err(|err| format!("--max-width: {err}"))?;
            }
            "--max-height" => {
                config.max_height = value
                    .parse()
                    .map_err(|err| format!("--max-height: {err}"))?;
            }
            "--max-area" => {
                config.max_area = value.parse().map_err(|err| format!("--max-area: {err}"))?;
            }
            "--workers" => {
                config.workers = value.parse().map_err(|err| format!("--workers: {err}"))?;
            }
            "--queue-depth" => {
                config.queue_depth = value
                    .parse()
                    .map_err(|err| format!("--queue-depth: {err}"))?;
            }
            "--public-base" => config.public_base = Some(value.clone()),
            "--endpoint" => config.endpoint = Some(value.clone()),
            other => return Err(format!("unknown flag {other}\n{USAGE}")),
        }
    }
    if config.workers == 0 {
        return Err("--workers must be at least 1".to_owned());
    }
    Ok(config)
}

#[expect(
    clippy::use_debug,
    reason = "`Duration` has no `Display`; `{:?}` is how a timeout is \
              rendered for a human on stderr."
)]
fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    // Answered before the subscriber exists: `--version` is parsed by scripts
    // and read by bug reports, and neither wants a log line in the way.
    match args.first().map(String::as_str) {
        Some("--version" | "-V") => {
            println!("{}", version_line());
            return ExitCode::SUCCESS;
        }
        Some("--help" | "-h") => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {}
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    if args.first().map(String::as_str) == Some("healthcheck") {
        let addr = match args.get(1) {
            Some(raw) => match raw.parse::<SocketAddr>() {
                Ok(addr) => addr,
                Err(err) => {
                    eprintln!("healthcheck: {raw}: {err}");
                    return ExitCode::FAILURE;
                }
            },
            None => SocketAddr::from(([127, 0, 0, 1], 6363)),
        };
        // A current-thread runtime: one socket, one request, no pool.
        let runtime = match runtime::Builder::new_current_thread().enable_all().build() {
            Ok(runtime) => runtime,
            Err(err) => {
                eprintln!("healthcheck: runtime startup failed: {err}");
                return ExitCode::FAILURE;
            }
        };
        // The timer has to be constructed inside the runtime, not handed to
        // block_on ready-made — `Sleep::new` needs the reactor to exist.
        let probe = async move { timeout(HEALTHCHECK_TIMEOUT, probe_health(addr)).await };
        return match runtime.block_on(probe) {
            Ok(Ok(())) => ExitCode::SUCCESS,
            Ok(Err(message)) => {
                eprintln!("healthcheck: {message}");
                ExitCode::FAILURE
            }
            Err(_) => {
                eprintln!("healthcheck: no answer from {addr} within {HEALTHCHECK_TIMEOUT:?}");
                ExitCode::FAILURE
            }
        };
    }
    if args.first().map(String::as_str) == Some("check") {
        let Some(target) = args.get(1) else {
            eprintln!("usage: iiif-server check <file-or-directory>");
            return ExitCode::FAILURE;
        };
        return run_check(Path::new(target));
    }
    let config = match parse_args(&args) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };
    let runtime = match runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(err) => {
            error!("runtime startup failed: {err}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(serve(config)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            error!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// Bind, serve, and drain on shutdown.
///
/// # Errors
///
/// A message describing whichever of bind, root resolution or the
/// accept loop failed.
#[expect(
    clippy::arithmetic_side_effects,
    clippy::shadow_reuse,
    clippy::integer_division_remainder_used,
    reason = "`tokio::select!` expands to arithmetic over its branch index; \
              `workers + queue_depth` is the admission bound, validated \
              when the flags were parsed; the rebindings are per-connection \
              `Arc::clone`s that deliberately keep the name of what they \
              clone."
)]
async fn serve(config: Config) -> Result<(), String> {
    let root = if config.root.starts_with("s3://") {
        iiif_sources::init_tls();
        SourceRoot::Object(iiif_sources::ObjectRoot::new(
            &config.root,
            config.endpoint.as_deref(),
        )?)
    } else {
        SourceRoot::Local(
            LocalRoot::new(Path::new(&config.root))
                .map_err(|err| format!("source root {}: {err}", config.root))?,
        )
    };
    let app = Arc::new(App {
        root,
        limits: Limits::new(config.max_width, config.max_height, config.max_area),
        public_base: config.public_base,
        admission: Arc::new(Semaphore::new(config.workers + config.queue_depth)),
        decode_permits: Arc::new(Semaphore::new(config.workers)),
        workers: config.workers,
        queue_depth: config.queue_depth,
        metrics: Arc::new(Metrics::default()),
    });
    let listener = TcpListener::bind(config.bind)
        .await
        .map_err(|err| format!("bind {}: {err}", config.bind))?;
    info!(
        "serving {} on http://{} ({} workers, queue {})",
        config.root, config.bind, config.workers, config.queue_depth
    );

    // Shutdown plumbing. `shutdown` fans the stop request out to every live
    // connection; `drain` is a channel nobody sends on — each connection task
    // holds a sender, so the receiver resolving to None means the last one
    // finished. Both are tokio primitives already in the tree.
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (drain_tx, mut drain_rx) = mpsc::channel::<()>(1);

    let shutdown = shutdown_signal();
    tokio::pin!(shutdown);
    loop {
        let (stream, _peer) = tokio::select! {
            () = &mut shutdown => {
                info!("shutdown signal received; draining in-flight requests");
                break;
            },
            accepted = listener.accept() => match accepted {
                Ok(accepted) => accepted,
                Err(err) => {
                    error!("accept: {err}");
                    continue;
                },
            },
        };
        let app = Arc::clone(&app);
        let mut connection_shutdown = shutdown_rx.clone();
        let connection_drain = drain_tx.clone();
        tokio::spawn(async move {
            let service = service_fn(move |req| {
                let app = Arc::clone(&app);
                async move { Ok::<_, Infallible>(app.handle(req).await) }
            });
            let connection = http1::Builder::new().serve_connection(TokioIo::new(stream), service);
            tokio::pin!(connection);
            tokio::select! {
                result = connection.as_mut() => {
                    if let Err(err) = result {
                        // Client disconnects are routine; log at debug level only.
                        tracing::debug!("connection ended: {err}");
                    }
                },
                _ = connection_shutdown.changed() => {
                    // Finish the request in flight, refuse further ones on this
                    // keep-alive connection, then close.
                    connection.as_mut().graceful_shutdown();
                    if let Err(err) = connection.await {
                        tracing::debug!("connection ended during shutdown: {err}");
                    }
                },
            }
            drop(connection_drain);
        });
    }

    // Stop listening first, so nothing new is admitted while we drain.
    drop(listener);
    // A closed receiver means every worker already stopped, which is the
    // outcome this send is asking for.
    #[expect(
        clippy::let_underscore_must_use,
        clippy::let_underscore_untyped,
        reason = "the only error this can return is `SendError`, meaning no \
                  receiver is left — which is the state the send is trying \
                  to reach. `drop()` is refused in turn, because the result \
                  is `Copy` (`dropping_copy_types`)."
    )]
    let _ = shutdown_tx.send(true);
    drop(drain_tx);
    if timeout(DRAIN_TIMEOUT, drain_rx.recv()).await.is_ok() {
        info!("drained cleanly");
    } else {
        info!("drain timed out after {DRAIN_TIMEOUT:?}; exiting anyway");
    }
    Ok(())
}

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::missing_panics_doc,
    clippy::inline_modules,
    reason = "test code: a panic here IS the failure signal, not a crash \
              path, and a `#[cfg(test)] mod tests` beside its subject is \
              how Rust unit tests are written"
)]
mod tests {
    use super::*;

    /// One-shot server that answers whatever status line it is given. Returns
    /// the address it bound, so the test never guesses a free port.
    async fn canned_response(response: &'static str) -> SocketAddr {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut scratch = [0_u8; 512];
                drop(stream.read(&mut scratch).await);
                drop(stream.write_all(response.as_bytes()).await);
            }
        });
        addr
    }

    #[tokio::test]
    async fn probe_accepts_200() {
        let addr = canned_response("HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nok\n").await;
        probe_health(addr).await.unwrap();
    }

    /// A saturated server answers 503 on the image routes but /healthz stays
    /// 200; anything else means unhealthy, and the container must be restarted
    /// rather than left serving errors.
    #[tokio::test]
    async fn probe_rejects_non_200() {
        let addr = canned_response("HTTP/1.1 503 Service Unavailable\r\n\r\n").await;
        let error = probe_health(addr)
            .await
            .expect_err("503 must fail the probe");
        assert!(error.contains("503"), "{error}");
    }

    #[tokio::test]
    async fn probe_rejects_garbage() {
        let addr = canned_response("this is not http\r\n\r\n").await;
        assert!(probe_health(addr).await.is_err());
    }

    /// Nothing listening is the ordinary "still starting up" case, and it must
    /// fail rather than hang.
    #[tokio::test]
    async fn probe_rejects_closed_port() {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        drop(listener);
        assert!(probe_health(addr).await.is_err());
    }

    #[test]
    fn version_line_reports_the_crate_version() {
        assert!(version_line().starts_with(&format!("iiif-server {}", iiif_server::VERSION)));
    }
}
