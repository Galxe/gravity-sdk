use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use pprof::{protos::Message, ProfilerGuard};
use serde::Deserialize;
use std::{sync::Arc, thread, time::Duration};
use tokio::{net::TcpListener, sync::Mutex};
use tracing::{info, warn};

/// Process-wide lock so only one profile request runs at a time. `pprof` uses
/// the global SIGPROF handler, and overlapping `ProfilerGuard`s produce
/// meaningless data (or outright fail). This static is shared across every
/// `serve()` instance in the process so two concurrent servers cannot race.
static PROFILING_LOCK: once_cell::sync::Lazy<tokio::sync::Mutex<()>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(()));

// Kept for symmetry / future state that may live per-server (e.g. last report
// timestamp); currently empty but the type name stays in one place.
type ProfilingLock = Arc<Mutex<()>>;

#[derive(Debug, Deserialize)]
struct ProfileParams {
    /// Profile duration in seconds. Capped at 300s.
    #[serde(default = "default_seconds")]
    seconds: u64,
    /// Sampling frequency in Hz (default 99, matches Go's pprof).
    #[serde(default = "default_freq")]
    frequency: i32,
}

fn default_seconds() -> u64 {
    30
}
fn default_freq() -> i32 {
    99
}

const MAX_SECONDS: u64 = 300;

pub async fn serve(addr: String) -> Result<(), anyhow::Error> {
    let socket_addr: std::net::SocketAddr = addr.parse().map_err(|e| {
        anyhow::anyhow!("invalid --pprof_addr {addr}: {e}")
    })?;
    let lock: ProfilingLock = Arc::new(Mutex::new(()));
    let app = Router::new()
        .route("/", get(index))
        .route("/debug/pprof/profile", get(profile))
        .with_state(lock);

    let listener = TcpListener::bind(socket_addr).await.map_err(|e| {
        anyhow::anyhow!("failed to bind pprof server on {socket_addr}: {e}")
    })?;
    info!("pprof HTTP server listening on http://{socket_addr}");
    axum::serve(listener, app).await.map_err(|e| anyhow::anyhow!("pprof server error: {e}"))
}

async fn index() -> impl IntoResponse {
    let body = "gravity-node pprof endpoint\n\
                \n\
                GET /debug/pprof/profile?seconds=N[&frequency=Hz]\n\
                    Returns a protobuf CPU profile (content-type application/octet-stream).\n\
                    Consume with `go tool pprof <url>` or `pprof -http=:8080 file.pb`.\n\
                    seconds   default 30, max 300\n\
                    frequency default 99 Hz\n";
    (StatusCode::OK, [(header::CONTENT_TYPE, "text/plain")], body)
}

async fn profile(
    State(_local): State<ProfilingLock>,
    Query(params): Query<ProfileParams>,
) -> Response {
    let seconds = params.seconds.clamp(1, MAX_SECONDS);
    let frequency = params.frequency.clamp(1, 1000);

    // Serialize overlapping requests (process-wide) so we never hold two
    // ProfilerGuards. The State lock is unused — kept for future per-server
    // state that doesn't need to be global.
    let Ok(_guard) = PROFILING_LOCK.try_lock() else {
        warn!("pprof request rejected: another profile is already running");
        return (
            StatusCode::CONFLICT,
            "another profile is already running; wait for it to finish\n",
        )
            .into_response();
    };

    info!("pprof: collecting {seconds}s CPU profile at {frequency} Hz");

    // pprof uses SIGPROF + thread-local data; run the blocking collection off
    // the async runtime so guard lifetimes never cross an await.
    let result: Result<Vec<u8>, String> = tokio::task::spawn_blocking(move || {
        let profiler =
            ProfilerGuard::new(frequency).map_err(|e| format!("ProfilerGuard::new: {e}"))?;
        thread::sleep(Duration::from_secs(seconds));
        let report = profiler.report().build().map_err(|e| format!("report.build: {e}"))?;
        let pb = report.pprof().map_err(|e| format!("report.pprof: {e}"))?;
        let mut buf = Vec::with_capacity(1 << 16);
        pb.write_to_vec(&mut buf).map_err(|e| format!("write_to_vec: {e}"))?;
        Ok(buf)
    })
    .await
    .unwrap_or_else(|e| Err(format!("spawn_blocking join: {e}")));

    match result {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (header::CONTENT_DISPOSITION, "attachment; filename=\"profile.pb\""),
            ],
            Body::from(bytes),
        )
            .into_response(),
        Err(err) => {
            warn!("pprof collection failed: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("profile collection failed: {err}\n"))
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Combined end-to-end: index, profile, and concurrency lock — all in
    /// one test because `pprof` uses process-wide SIGPROF and splitting into
    /// multiple `#[tokio::test]` functions makes them race on global profiler
    /// state (cargo runs them in parallel by default).
    #[tokio::test]
    async fn pprof_server_end_to_end() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);

        tokio::spawn(async move {
            let _ = serve(addr.to_string()).await;
        });
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Index
        let idx = reqwest::get(format!("http://{addr}/")).await.unwrap();
        assert!(idx.status().is_success());
        assert!(idx.text().await.unwrap().contains("/debug/pprof/profile"));

        // Profile — short, low frequency to keep the test fast
        let url = format!("http://{addr}/debug/pprof/profile?seconds=1&frequency=50");
        let client =
            reqwest::Client::builder().timeout(Duration::from_secs(20)).build().unwrap();
        let resp = client.get(&url).send().await.unwrap();
        let status = resp.status();
        let headers = resp.headers().clone();
        let bytes = resp.bytes().await.unwrap();
        assert_eq!(status, StatusCode::OK, "body: {}", String::from_utf8_lossy(&bytes));
        assert_eq!(headers.get(header::CONTENT_TYPE).unwrap(), "application/octet-stream");
        assert!(!bytes.is_empty(), "profile response must not be empty");
        // Protobuf tag byte: field 1 wire-type 2 = 0x0a, field 5 wire-type 0 = 0x28
        assert!(
            matches!(bytes[0], 0x0a | 0x28),
            "unexpected first byte 0x{:02x}; body len={}",
            bytes[0],
            bytes.len()
        );

        // Concurrency: overlap two requests, expect 200 then 409
        let url2 = format!("http://{addr}/debug/pprof/profile?seconds=2&frequency=50");
        let h1 = tokio::spawn({
            let c = client.clone();
            let u = url2.clone();
            async move { c.get(&u).send().await.map(|r| r.status()) }
        });
        tokio::time::sleep(Duration::from_millis(300)).await;
        let r2 = client.get(&url2).send().await.unwrap();
        assert_eq!(r2.status(), StatusCode::CONFLICT);
        let s1 = h1.await.unwrap().unwrap();
        assert_eq!(s1, StatusCode::OK);
    }
}
