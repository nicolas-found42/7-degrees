//! Runtime telemetry: what this process is, what it served, and what the
//! semantic provider did.
//!
//! Every answer this server gives is a function of (a) the code that answered,
//! (b) the data snapshot it loaded, and (c) the request itself. This module
//! records (b) and (c) so a past answer can be attributed, and it keeps a
//! bounded in-memory history so the newest requests are always inspectable
//! through `/api/diagnostics` without any external tooling.
//!
//! Nothing here may carry credential material: request records hold the
//! method, path, status and duration only — never headers, bodies or query
//! strings. The provider receipt (model, provider request id, tokens, cost)
//! stays on the server: it is written to the operator's log file and served
//! only in the receipt-free view.

use serde::Serialize;
use std::collections::VecDeque;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Instant;

/// The commit this binary was built from (`unknown` outside a git checkout).
pub const BUILD_COMMIT: &str = env!("SEVEN_DEGREES_COMMIT");
/// Whether the source tree was dirty when this binary was built.
pub fn build_dirty() -> bool {
    env!("SEVEN_DEGREES_DIRTY") == "true"
}

/// How many request records the in-memory ring keeps.
const REQUEST_HISTORY: usize = 128;
/// How many client reports the in-memory ring keeps.
const CLIENT_HISTORY: usize = 64;
/// How many semantic events the in-memory ring keeps.
const SEMANTIC_HISTORY: usize = 64;

/// The identity of the snapshot this process serves: the graph it answers
/// from and the files that graph was built from.
#[derive(Clone, Debug, Serialize)]
pub struct SnapshotIdentity {
    /// How many player identities the loaded graph holds.
    pub players: usize,
    /// How many teammate edges the loaded graph holds.
    pub edges: usize,
    /// The canonical data files, with their content digests.
    pub sources: Vec<FileDigest>,
    /// The served canvas assets, with their content digests.
    pub assets: Vec<FileDigest>,
}

/// The commit this binary answers for.
///
/// `NBA_BUILD_COMMIT` names the commit of the run when the operator sets it
/// (the verification capture does); otherwise the commit baked in at build
/// time answers.
pub fn build_commit() -> String {
    std::env::var("NBA_BUILD_COMMIT")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| BUILD_COMMIT.to_string())
}

/// One file of the running snapshot, with its content digest.
#[derive(Clone, Debug, Serialize)]
pub struct FileDigest {
    /// The file's path relative to its snapshot root.
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}

/// One completed request.
#[derive(Clone, Debug, Serialize)]
pub struct RequestRecord {
    pub ts: String,
    /// This server's identifier for the request; quoted back to operators.
    pub request_id: String,
    pub method: String,
    /// The request path without its query string: query strings carry free
    /// text, and this log must stay free of user content.
    pub path: String,
    pub status: u16,
    pub duration_ms: f64,
}

/// One client-side failure report relayed by the canvas.
#[derive(Clone, Debug, Serialize)]
pub struct ClientReport {
    pub ts: String,
    pub kind: String,
    pub message: String,
    /// The failing URL without its query string.
    pub url: String,
    pub context: serde_json::Value,
}

/// One semantic judgment.
///
/// The receipt fields name the provider's model and request id. They are
/// recorded for the operator's log file and never served to a browser.
#[derive(Clone, Debug, Serialize)]
pub struct SemanticEvent {
    pub ts: String,
    /// `available` when the provider answered, `unavailable` when it did not,
    /// `rejected` when the provider answered but the application refused the
    /// reply.
    pub outcome: &'static str,
    /// The stable failure code, when the call did not produce a usable answer.
    pub failure: Option<String>,
    pub elapsed_ms: f64,
    /// The provider receipt, absent for calls that never reached a provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<SemanticReceipt>,
}

/// The provider receipt of one call: model, provider, request id, usage and
/// cost. Never part of a browser-facing body.
#[derive(Clone, Debug, Serialize)]
pub struct SemanticReceipt {
    pub model: Option<String>,
    pub provider: Option<String>,
    pub provider_request_id: Option<String>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
}

impl SemanticEvent {
    /// One judgment: its outcome, the cause of a failure when there is one,
    /// and the latency of the call.
    pub fn new(
        ts: String,
        outcome: &'static str,
        failure: Option<String>,
        elapsed_ms: f64,
    ) -> Self {
        Self {
            ts,
            outcome,
            failure,
            elapsed_ms,
            receipt: None,
        }
    }

    /// The browser-safe view of one event: outcome, cause and latency only.
    ///
    /// The receipt is deliberately absent: the diagnostics body is served to
    /// any browser that can reach the local server.
    pub fn public_view(&self) -> serde_json::Value {
        serde_json::json!({
            "ts": self.ts,
            "outcome": self.outcome,
            "failure": self.failure,
            "elapsed_ms": self.elapsed_ms,
        })
    }
}

/// The append-only JSONL file every runtime record is written to.
///
/// The file is the operator's artifact: it outlives the process and answers
/// "what actually happened" for a run whose output is no longer on screen.
/// One handle per process keeps the lines of concurrent writers intact.
#[derive(Debug)]
pub struct LogSink {
    file: Mutex<Option<std::fs::File>>,
}

impl LogSink {
    /// The process-wide sink, opened from `NBA_RUNTIME_LOG_PATH` on first
    /// use. Without that variable the sink keeps no file and only the
    /// in-memory history remains.
    pub fn shared() -> &'static LogSink {
        static SINK: LazyLock<LogSink> = LazyLock::new(|| LogSink {
            file: Mutex::new(LogSink::open_from_env()),
        });
        &SINK
    }

    fn open_from_env() -> Option<std::fs::File> {
        let path = std::env::var_os("NBA_RUNTIME_LOG_PATH")?;
        let path = PathBuf::from(path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .ok()
    }

    /// Appends one JSON object as a line.
    pub fn write(&self, value: &serde_json::Value) {
        let Ok(line) = serde_json::to_string(value) else {
            return;
        };
        let mut file = self.file.lock().expect("log sink lock");
        if let Some(file) = file.as_mut() {
            let _ = writeln!(file, "{line}");
            let _ = file.flush();
        }
    }

    /// Appends one record under a `kind` discriminator.
    pub fn write_kind(&self, kind: &str, value: &serde_json::Value) {
        if let Some(object) = value.as_object() {
            let mut line = serde_json::Map::with_capacity(object.len() + 1);
            line.insert("kind".into(), serde_json::Value::from(kind));
            line.extend(
                object
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            );
            self.write(&serde_json::Value::Object(line));
        }
    }
}

#[derive(Debug)]
struct Inner {
    requests: VecDeque<RequestRecord>,
    client_reports: VecDeque<ClientReport>,
    total_requests: u64,
}

/// The telemetry store: a bounded history plus the operator's log file.
#[derive(Debug)]
pub struct Telemetry {
    inner: Mutex<Inner>,
    next_request_id: AtomicU64,
    started_at: String,
    start: Instant,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}

impl Telemetry {
    /// A store writing to the process-wide sink.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                requests: VecDeque::new(),
                client_reports: VecDeque::new(),
                total_requests: 0,
            }),
            next_request_id: AtomicU64::new(1),
            started_at: now_rfc3339(),
            start: Instant::now(),
        }
    }

    /// The instant this process started, as an RFC 3339 timestamp.
    pub fn started_at(&self) -> &str {
        &self.started_at
    }

    /// Seconds since this process started.
    pub fn uptime_seconds(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    /// A fresh request identifier, unique within this process.
    pub fn next_request_id(&self) -> String {
        format!("r-{}", self.next_request_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Record a completed request in the history and in the log file.
    pub fn record_request(&self, record: RequestRecord) {
        {
            let mut inner = self.inner.lock().expect("telemetry lock");
            inner.total_requests += 1;
            if inner.requests.len() >= REQUEST_HISTORY {
                inner.requests.pop_front();
            }
            inner.requests.push_back(record.clone());
        }
        if let Ok(value) = serde_json::to_value(&record) {
            LogSink::shared().write_kind("request", &value);
        }
    }

    /// The number of requests this process has served.
    pub fn total_requests(&self) -> u64 {
        self.inner.lock().expect("telemetry lock").total_requests
    }

    /// The most recent request records, oldest first.
    pub fn recent_requests(&self) -> Vec<RequestRecord> {
        self.inner
            .lock()
            .expect("telemetry lock")
            .requests
            .iter()
            .cloned()
            .collect()
    }

    /// Record a client-side failure report in the history and the log file.
    pub fn record_client_report(&self, report: ClientReport) {
        {
            let mut inner = self.inner.lock().expect("telemetry lock");
            if inner.client_reports.len() >= CLIENT_HISTORY {
                inner.client_reports.pop_front();
            }
            inner.client_reports.push_back(report.clone());
        }
        if let Ok(value) = serde_json::to_value(&report) {
            LogSink::shared().write_kind("client_report", &value);
        }
    }

    /// The most recent client-side failure reports, oldest first.
    pub fn recent_client_reports(&self) -> Vec<ClientReport> {
        self.inner
            .lock()
            .expect("telemetry lock")
            .client_reports
            .iter()
            .cloned()
            .collect()
    }
}

/// The bounded history of semantic judgments.
#[derive(Clone, Debug, Default)]
pub struct SemanticLog {
    events: Arc<Mutex<VecDeque<SemanticEvent>>>,
}

impl SemanticLog {
    /// An empty history.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one judgment and write it to the operator's log file.
    pub fn record(&self, event: SemanticEvent) {
        {
            let mut events = self.events.lock().expect("semantic log lock");
            if events.len() >= SEMANTIC_HISTORY {
                events.pop_front();
            }
            events.push_back(event.clone());
        }
        if let Ok(value) = serde_json::to_value(&event) {
            LogSink::shared().write_kind("semantic", &value);
        }
    }

    /// The most recent judgments, oldest first.
    pub fn recent(&self) -> Vec<SemanticEvent> {
        self.events
            .lock()
            .expect("semantic log lock")
            .iter()
            .cloned()
            .collect()
    }
}

/// The current time as an RFC 3339 timestamp with millisecond precision.
pub fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// The digest of a file on disk, or `None` when it cannot be read.
pub fn digest_file(path: &Path, name: &str) -> Option<FileDigest> {
    use sha2::{Digest, Sha256};
    let bytes = std::fs::read(path).ok()?;
    Some(FileDigest {
        name: name.to_string(),
        bytes: bytes.len() as u64,
        sha256: format!("{:x}", Sha256::digest(&bytes)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sink over one temporary file.
    fn sink(path: &Path) -> LogSink {
        let file = std::fs::File::create(path).expect("temporary log file");
        LogSink {
            file: Mutex::new(Some(file)),
        }
    }

    /// A temporary path that no other test uses.
    fn temporary(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "7-degrees-{name}-{}-{:?}.log",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn the_log_sink_writes_one_json_object_per_line() {
        let path = temporary("sink");
        let sink = sink(&path);
        sink.write_kind("request", &serde_json::json!({"path": "/api/stats"}));
        sink.write_kind("semantic", &serde_json::json!({"outcome": "unavailable"}));
        let text = std::fs::read_to_string(&path).expect("log file");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "one line per record: {text}");
        let first: serde_json::Value = serde_json::from_str(lines[0]).expect("JSON line");
        assert_eq!(first["kind"], "request");
        assert_eq!(first["path"], "/api/stats");
        let second: serde_json::Value = serde_json::from_str(lines[1]).expect("JSON line");
        assert_eq!(second["kind"], "semantic");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_request_history_is_bounded() {
        let telemetry = Telemetry::new();
        for index in 0..(REQUEST_HISTORY + 10) {
            telemetry.record_request(RequestRecord {
                ts: now_rfc3339(),
                request_id: format!("r-{index}"),
                method: "GET".into(),
                path: "/api/stats".into(),
                status: 200,
                duration_ms: 1.0,
            });
        }
        assert_eq!(telemetry.recent_requests().len(), REQUEST_HISTORY);
        assert_eq!(telemetry.total_requests(), (REQUEST_HISTORY + 10) as u64);
        // The newest record is the one that stays.
        assert_eq!(
            telemetry
                .recent_requests()
                .last()
                .map(|r| r.request_id.clone()),
            Some(format!("r-{}", REQUEST_HISTORY + 9))
        );
    }

    #[test]
    fn a_digest_names_the_file_and_its_content() {
        let path = temporary("digest");
        std::fs::write(&path, b"7 degrees").expect("temporary file");
        let digest = digest_file(&path, "t4/tenures.csv").expect("digest");
        assert_eq!(digest.name, "t4/tenures.csv");
        assert_eq!(digest.bytes, 9);
        assert_eq!(digest.sha256.len(), 64, "a hex sha256 digest");
        assert!(
            digest.sha256.chars().all(|c| c.is_ascii_hexdigit()),
            "the digest is hex: {}",
            digest.sha256
        );
        // The digest follows the content, not the file name.
        let other = digest_file(&path, "t4/appearance-counts.csv").expect("digest");
        assert_eq!(other.sha256, digest.sha256);
        assert_eq!(other.name, "t4/appearance-counts.csv");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_missing_file_has_no_digest() {
        assert!(digest_file(Path::new("/nonexistent/7-degrees.csv"), "x").is_none());
    }
}
