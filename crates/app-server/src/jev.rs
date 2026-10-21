//! Server-side Jev client plumbing for the app state.
//!
//! Spec §"Jev data minimization and security": TypeSafe credentials stay on
//! the server side, and graph facts never depend on Jev. AppState holds the
//! key only inside the [`JevHandle`]'s client — never in DTOs, HTML, or
//! logs — and with no env key the handle is [`JevState::Unconfigured`], so
//! every judgment is a deterministic `Unavailable`.

use crate::telemetry::{self, SemanticEvent, SemanticLog, SemanticReceipt};
use jev_client::{
    EnvConfig, JevClient, JevEvaluation, JevFailure, JevMetadata, JevOutcome, JevRequest,
    JevTransport,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// The Jev side of the app state: absent, configured, or degraded.
#[derive(Clone)]
enum JevState {
    /// No key in the environment: semantic features are off by construction.
    Unconfigured,
    /// A client exists; `degraded` follows the most recent call: set when
    /// the provider fails or a consumer rejects its reply, cleared by the
    /// next successful judgment (fail-soft status for the UI line).
    Client {
        handle: Arc<HandleInner>,
        degraded: Arc<AtomicBool>,
    },
}

impl JevState {
    /// `true` when the configured provider's most recent call failed.
    fn degraded(&self) -> bool {
        match self {
            JevState::Unconfigured => false,
            JevState::Client { degraded, .. } => degraded.load(Ordering::Relaxed),
        }
    }
}

/// The wrapped client plus the last recorded judgment outcome.
struct HandleInner {
    client: JevClient<ErasedJudge>,
    last_outcome: Mutex<Option<JevOutcome>>,
}

impl HandleInner {
    /// One judgment: records the outcome for the status surface. Never
    /// panics on provider problems (the transport fails soft).
    fn judge_measured(&self, request: &JevRequest) -> JevEvaluation {
        let evaluation = self.client.evaluate_measured(request);
        if !evaluation.outcome.is_available() {
            self.last_outcome
                .lock()
                .expect("last_outcome lock")
                .replace(evaluation.outcome.clone());
        }
        evaluation
    }
}

/// Cloneable Jev judge handle shared by the routes.
#[derive(Clone)]
pub struct JevHandle {
    inner: JevState,
    /// The bounded history of this handle's judgments: what the semantic
    /// provider did, with the cause of every failure.
    log: SemanticLog,
}

impl JevHandle {
    /// The startup-configured handle: `Unconfigured` without an env key, a
    /// real HTTP client otherwise (key read from the process env at startup,
    /// never from a file or a committed constant).
    pub fn from_env() -> Self {
        match jev_client::config_from_env() {
            EnvConfig::Configured(config, _source) => {
                let transport = jev_client::http_transport::HttpJevTransport::new();
                Self::from_client(JevClient::new(config, transport))
            }
            EnvConfig::NoConfig => Self {
                inner: JevState::Unconfigured,
                log: SemanticLog::new(),
            },
        }
    }

    /// A handle over any client — the seam tests use (scripted in-memory
    /// transports: no network, no real key).
    pub fn from_client<T: JevTransport + 'static>(client: JevClient<T>) -> Self {
        Self {
            inner: JevState::Client {
                handle: Arc::new(HandleInner {
                    client: JevClient::new(client.config().clone(), ErasedJudge::new(client)),
                    last_outcome: Mutex::new(None),
                }),
                degraded: Arc::new(AtomicBool::new(false)),
            },
            log: SemanticLog::new(),
        }
    }

    /// A deterministically unconfigured handle (no provider): every judgment
    /// is `Unavailable` and the status is `unconfigured`, independent of the
    /// process environment.
    pub fn unconfigured() -> Self {
        Self {
            inner: JevState::Unconfigured,
            log: SemanticLog::new(),
        }
    }

    /// The bounded judgment history of this handle.
    pub fn semantic_log(&self) -> SemanticLog {
        self.log.clone()
    }

    /// One judgment through the underlying client; `Unavailable` without a
    /// configured provider. The degraded status tracks the latest call.
    pub fn judge(&self, request: &JevRequest) -> JevOutcome {
        self.judge_measured(request).outcome
    }
    /// Receipt and latency belong to the returned call, never global status state.
    pub fn judge_measured(&self, request: &JevRequest) -> JevEvaluation {
        match &self.inner {
            JevState::Unconfigured => {
                // The call never reached a provider, and the cause is named:
                // an unnamed failure cannot be told apart from an outage.
                let evaluation = JevEvaluation::failed(JevFailure::Unconfigured);
                self.log.record(SemanticEvent::new(
                    telemetry::now_rfc3339(),
                    "unavailable",
                    Some(JevFailure::Unconfigured.code()),
                    evaluation.elapsed_ms,
                ));
                evaluation
            }
            JevState::Client { handle, degraded } => {
                let evaluation = handle.judge_measured(request);
                degraded.store(!evaluation.outcome.is_available(), Ordering::SeqCst);
                self.log.record(event(&evaluation));
                evaluation
            }
        }
    }
    /// A consumer rejected a typed but invalid response; preserve fail-soft status.
    pub fn reject_response(&self) {
        if let JevState::Client { degraded, .. } = &self.inner {
            degraded.store(true, Ordering::SeqCst);
        }
        // The provider answered, but the application refused the reply: a
        // distinct outcome from a provider failure, and worth its own record.
        self.log.record(SemanticEvent::new(
            telemetry::now_rfc3339(),
            "rejected",
            None,
            0.0,
        ));
    }

    /// The semantic-feature availability tuple for the status surfaces:
    /// `(available, reason)` with reason `unconfigured` | `degraded` |
    /// `available`.
    pub fn status(&self) -> (bool, &'static str) {
        match &self.inner {
            JevState::Unconfigured => (false, "unconfigured"),
            JevState::Client { .. } if self.inner.degraded() => (false, "degraded"),
            JevState::Client { .. } => (true, "available"),
        }
    }

    /// `true` when a provider is configured (even if currently unreachable).
    pub fn configured(&self) -> bool {
        matches!(self.inner, JevState::Client { .. })
    }
}

/// One judgment as a telemetry event: the outcome, the named cause of a
/// failure, the latency, and the provider receipt for the operator's log.
fn event(evaluation: &JevEvaluation) -> SemanticEvent {
    let mut event = SemanticEvent::new(
        telemetry::now_rfc3339(),
        if evaluation.outcome.is_available() {
            "available"
        } else {
            "unavailable"
        },
        evaluation.failure.map(|failure| failure.code()),
        evaluation.elapsed_ms,
    );
    event.receipt = evaluation.metadata.as_ref().map(receipt);
    event
}

/// The provider receipt of one call. It names the provider's model and
/// request id and stays on the server: it is written to the operator's log
/// file and never served to a browser.
fn receipt(metadata: &JevMetadata) -> SemanticReceipt {
    SemanticReceipt {
        model: metadata.model.clone(),
        provider: metadata.provider.clone(),
        provider_request_id: metadata.request_id.clone(),
        input_tokens: metadata.input_tokens,
        output_tokens: metadata.output_tokens,
        cost_usd: metadata.cost_usd,
    }
}

/// Type-erased judge transport: closes over a concrete client (any backing
/// transport, erased in a closure) so the handle holds one concrete inner
/// type.
struct ErasedJudge {
    evaluate: Box<dyn Fn(&JevRequest) -> JevEvaluation + Send + Sync>,
}

impl ErasedJudge {
    fn new<T: JevTransport + 'static>(client: JevClient<T>) -> Self {
        Self {
            evaluate: Box::new(move |request| client.evaluate_measured(request)),
        }
    }
}

impl JevTransport for ErasedJudge {
    fn evaluate(&self, _config: &jev_client::JevConfig, request: &JevRequest) -> JevOutcome {
        (self.evaluate)(request).outcome
    }
    fn evaluate_measured(
        &self,
        _config: &jev_client::JevConfig,
        request: &JevRequest,
    ) -> JevEvaluation {
        (self.evaluate)(request)
    }
}

impl std::fmt::Debug for JevHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Never any key material in Debug output.
        match &self.inner {
            JevState::Unconfigured => f.debug_tuple("JevHandle::Unconfigured").finish(),
            JevState::Client { degraded, .. } => f
                .debug_struct("JevHandle::Client")
                .field("degraded", &degraded.load(Ordering::Relaxed))
                .finish_non_exhaustive(),
        }
    }
}

/// Captured log output for the in-process tests: every `log` record the app
/// emits is recorded here in test-capture builds. Compiled only under
/// `--cfg test_capture` (enable with `RUSTFLAGS='--cfg test_capture'`), so
/// production builds have no test-only surface at all.
#[cfg(test_capture)]
pub fn test_log_capture() -> &'static Mutex<String> {
    use std::sync::OnceLock;
    static CAPTURE: OnceLock<Mutex<String>> = OnceLock::new();
    CAPTURE.get_or_init(|| Mutex::new(String::new()))
}

/// Records every `log` record's rendered message into the capture buffer
/// under `--cfg test_capture`; installed by the binary entry point.
#[cfg(test_capture)]
struct CaptureLogger;

#[cfg(test_capture)]
impl log::Log for CaptureLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        if let Ok(mut captured) = test_log_capture().lock() {
            captured.push_str(&record.level().to_string());
            captured.push(' ');
            captured.push_str(&record.args().to_string());
            captured.push('\n');
        }
    }

    fn flush(&self) {}
}

#[cfg(test_capture)]
static CAPTURE_LOGGER: CaptureLogger = CaptureLogger;

/// Installs the capture logger in test-capture builds (`set_boxed_logger`
/// fails harmlessly if a logger is already installed by the harness).
#[cfg(test_capture)]
pub fn install_capture_logger() {
    let _ = log::set_boxed_logger(Box::new(CaptureLogger));
    log::set_max_level(log::LevelFilter::Debug);
}

/// A no-op in normal builds (production installs its own logger in
/// [`init_logging`]).
#[cfg(not(test_capture))]
pub fn install_capture_logger() {}

/// Production log level policy: records stay at `warn` or rarer and never
/// carry credential material (the client logs only status summaries).
///
/// Without a logger installed the `log` facade discards every record, so a
/// production run would keep no trace of a failed provider call. The
/// environment logger gives those records a destination on stderr; `RUST_LOG`
/// sets the level for an operator who wants more, and the default stays at
/// the `warn` policy.
pub fn init_logging() {
    #[cfg(not(test_capture))]
    {
        // The environment logger owns the level: `RUST_LOG` decides, and the
        // default stays at the `warn` policy.
        let installed =
            env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
                .format_timestamp_millis()
                .try_init()
                .is_ok();
        if !installed {
            log::set_max_level(log::LevelFilter::Warn);
        }
    }
    #[cfg(test_capture)]
    {
        // The capture logger records everything the app emits, so its level
        // stays at debug for the in-process proofs.
        log::set_max_level(log::LevelFilter::Debug);
    }
}
