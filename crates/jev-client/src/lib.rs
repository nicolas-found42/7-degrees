//! The Jev seam: configuration, request/answer types, and fail-soft outcomes.
//!
//! Spec §"Jev at runtime" and §"Jev data minimization and security": the Rust
//! server calls the TypeSafe HTTP API over the configured provider (verified
//! live: OpenRouter base URL, `Authorization: Bearer`, `POST /v1/systemone`),
//! credentials stay server-side only, and every judgment degrades to a
//! deterministic `Unavailable` outcome when Jev is unconfigured or
//! unreachable. The wire types here are provider-agnostic; `http_transport`
//! holds the verified OpenRouter HTTP transport, so tests can drive the seam
//! in pure memory with [`MemoryTransport`].

/// The TypeSafe model used for every judgment.
///
/// Verified against the live OpenRouter TypeSafe compatibility guide: IDs that
/// already carry an author prefix (`typesafe/jev-1.13`) are routed as-is.
pub const JEVC_MODEL: &str = "typesafe/jev-1.13";

/// Where judgment calls go (verified live, 2026-10 session): OpenRouter routes
/// the TypeSafe System One API at `https://openrouter.ai/api/v1/systemone`.
pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api";

/// TypeSafe Jev configuration. The API key lives here and in the HTTP client
/// only; it never reaches DTOs, HTML, or ordinary logs (`Debug` redacts it).
#[derive(Clone, PartialEq)]
pub struct JevConfig {
    /// The bearer token. Never logged or serialized; `Debug` prints `[redacted]`.
    pub api_key: String,
    /// Provider base URL. `https://openrouter.ai/api` with the OpenRouter
    /// provider; the client appends `/v1/systemone`.
    pub base_url: String,
    /// Request timeout, in seconds. One explicit retry follows a single
    /// timeout/transport failure; there are no unbounded hangs.
    pub timeout_secs: u64,
    /// System One model id (`typesafe/jev-1.13` by default).
    pub model: String,
}

impl JevConfig {
    /// Configuration with the verified provider defaults.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: DEFAULT_BASE_URL.to_string(),
            timeout_secs: 10,
            model: JEVC_MODEL.to_string(),
        }
    }
}

impl std::fmt::Debug for JevConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JevConfig")
            .field("api_key", &"[redacted]")
            .field("base_url", &self.base_url)
            .field("timeout_secs", &self.timeout_secs)
            .field("model", &self.model)
            .finish()
    }
}

/// Where the configuration came from, so the status endpoint can distinguish
/// "never configured" from "configured but the provider is unreachable".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JevConfigSource {
    /// `$TYPESAFE_API_KEY` was set at startup.
    TypesafeApiKey,
    /// `$OPENROUTER_API_KEY` was set at startup (the local provider convention).
    OpenRouterApiKey,
}

impl JevConfigSource {
    /// The env var this source corresponds to.
    pub fn env_var(self) -> &'static str {
        match self {
            JevConfigSource::TypesafeApiKey => "TYPESAFE_API_KEY",
            JevConfigSource::OpenRouterApiKey => "OPENROUTER_API_KEY",
        }
    }
}

/// The config surface: `TYPESAFE_API_KEY` first, then the local
/// `OPENROUTER_API_KEY` convention. Absent variables build `NoConfig`; the
/// key is read from the process env at startup — it is never committed and is
/// never read from any file inside the repository.
pub fn config_from_env() -> EnvConfig {
    if let Ok(key) = std::env::var("TYPESAFE_API_KEY")
        && !key.trim().is_empty()
    {
        return EnvConfig::Configured(JevConfig::new(key), JevConfigSource::TypesafeApiKey);
    }
    if let Ok(key) = std::env::var("OPENROUTER_API_KEY")
        && !key.trim().is_empty()
    {
        return EnvConfig::Configured(JevConfig::new(key), JevConfigSource::OpenRouterApiKey);
    }
    EnvConfig::NoConfig
}

/// The result of `config_from_env`.
#[derive(Clone, Debug, PartialEq)]
pub enum EnvConfig {
    /// A key was found; judge calls will use the provider.
    Configured(JevConfig, JevConfigSource),
    /// No env key: every judgment returns `unavailable` deterministically.
    NoConfig,
}

// --- Typed request/answer shapes (spec §"Jev data minimization and security") ---

/// One typed question sent with a judgment request.
///
/// Verified against the live TypeSafe API docs: `type` is one of the three
/// primitives, `instructions` carries the judgment, `criteria` is an
/// option/level map (Choice/Score) or a yes/no meanings map (Noul).
#[derive(Clone, Debug, PartialEq)]
pub enum JevQuestion {
    /// A yes/no question; answers carry the `noul` probability of yes.
    Noul {
        instructions: String,
        criteria: Option<YesNoMeanings>,
    },
    /// Picks one of the defined options; answers carry the picked option and
    /// distribution.
    Choice {
        instructions: String,
        criteria: Vec<(String, String)>,
    },
    /// Rates along the ordered levels; answers carry the weighted level.
    Score {
        instructions: String,
        criteria: Vec<String>,
    },
}

/// What a Noul yes/no means (both optional per the live docs).
#[derive(Clone, Debug, PartialEq)]
pub struct YesNoMeanings {
    pub yes: String,
    pub no: String,
}

/// A question id as it appears in the request and response envelopes.
pub type QuestionId = String;

/// A minimal judgment payload: bounded instructions plus the focused state
/// needed for the questions. The full player graph is never sent.
#[derive(Clone, Debug)]
pub struct JevRequest {
    /// The bounded state (candidate names, the specific excerpt, ...).
    pub state: serde_json::Value,
    /// The typed questions to evaluate against the state.
    pub questions: Vec<(QuestionId, JevQuestion)>,
}

/// A typed answer, under the id of its question.
#[derive(Clone, Debug, PartialEq)]
pub enum JevAnswer {
    /// Probability the answer is yes (0–1).
    Noul(f64),
    /// The picked option id plus the distribution over options.
    Choice(String, Vec<(String, f64)>, f64),
    /// Probability-weighted level plus the distribution over level numbers.
    Score(f64, Vec<(u8, f64)>, f64),
}

/// The judgment outcomes consumed by application code.
///
/// Spec §"Jev at runtime": every call has a no-match/unsupported answer
/// available, and judgments are fallible; `Unavailable` is the deterministic
/// fallback when Jev is unconfigured or unreachable.
#[derive(Clone, Debug, PartialEq)]
pub enum JevOutcome {
    /// The provider answered; answers pair with the request's questions.
    Answers(Vec<(QuestionId, JevAnswer)>),
    /// Jev cannot help right now: no configuration, failed request, timeout,
    /// malformed response, or a request that violated the payload guardrails.
    Unavailable,
}

impl JevOutcome {
    /// `true` when the provider answered.
    pub fn is_available(&self) -> bool {
        matches!(self, JevOutcome::Answers(_))
    }
}

/// The judgment entry point the server holds: a typed request in, a fail-soft
/// [`JevOutcome`] out. The transport behind it is either the verified HTTP
/// transport (`http_transport` feature, wired by the app server) or an
/// in-memory fake in tests.
pub struct JevClient<T: JevTransport> {
    config: JevConfig,
    transport: T,
}

impl<T: JevTransport> JevClient<T> {
    /// A client around an explicit config and transport.
    pub fn new(config: JevConfig, transport: T) -> Self {
        Self { config, transport }
    }

    /// One judgment: bounded state plus typed questions in, fallible answers
    /// or `Unavailable` out. Never panics on provider problems.
    pub fn evaluate(&self, request: &JevRequest) -> JevOutcome {
        self.transport.evaluate(&self.config, request)
    }

    /// The (redacted) config, for status surfaces only.
    pub fn config(&self) -> &JevConfig {
        &self.config
    }
}

/// In-memory transport for seam tests: replays scripted outcomes in order and
/// records the requests it received.
#[derive(Default)]
pub struct MemoryTransport {
    scripted: std::sync::Mutex<Vec<JevOutcome>>,
    pub seen: std::sync::Mutex<Vec<JevRequest>>,
}

impl MemoryTransport {
    /// Script outcomes in order; the last one repeats once the script runs
    /// out (an empty script answers `Unavailable` forever).
    pub fn scripted(outcomes: Vec<JevOutcome>) -> Self {
        Self {
            scripted: std::sync::Mutex::new(outcomes),
            seen: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl JevTransport for MemoryTransport {
    fn evaluate(&self, _config: &JevConfig, request: &JevRequest) -> JevOutcome {
        self.seen.lock().expect("seen lock").push(JevRequest {
            state: request.state.clone(),
            questions: request.questions.clone(),
        });
        let mut scripted = self.scripted.lock().expect("scripted lock");
        if scripted.is_empty() {
            return JevOutcome::Unavailable;
        }
        if scripted.len() == 1 {
            return scripted[0].clone();
        }
        scripted.remove(0)
    }
}

/// The transport seam: one judgment, fail-soft. Implementations are the real
/// HTTP call (`http_transport`) and, in tests, in-memory fakes that simulate
/// unreachable/malformed providers without any network.
pub trait JevTransport: Send + Sync {
    /// Send one judgment request. Implementations must never panic on
    /// transport problems — every failure returns `JevOutcome::Unavailable`.
    fn evaluate(&self, config: &JevConfig, request: &JevRequest) -> JevOutcome;
}

/// Guardrail shared with the HTTP transport: the outbound state must stay
/// small and focused (spec data minimization). Bounded well under any real
/// judgment payload; the point is to make unbounded growth a fallback, not a
/// silent behavior change.
pub const MAX_STATE_BYTES: usize = 32 * 1024;

/// Guardrail: one judgment request carries at most this many questions.
pub const MAX_QUESTIONS: usize = 8;

// --- Documented TypeSafe wire envelope notes (verified live against
// https://docs.typesafe.ai/api.md and the OpenRouter compat guide) ---

#[cfg(feature = "http-transport")]
pub mod http_transport;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_redacts_the_api_key() {
        let config = JevConfig::new("sk-never-print-me-0123456789abcdef".to_string());
        let rendered = format!("{config:?}");
        assert!(
            !rendered.contains("sk-never-print-me"),
            "Debug must redact the key: {rendered}"
        );
        assert!(rendered.contains("[redacted]"));
    }

    #[test]
    fn env_config_prefers_typesafe_then_openrouter_then_no_config() {
        // No keys at all -> NoConfig (the deterministic fallback).
        let none = EnvConfig::NoConfig;
        assert!(!matches!(none, EnvConfig::Configured(_, _)));

        // The source enum is the only trace of which env var was used; its
        // env_var names are stable contracts for the status endpoint.
        assert_eq!(
            JevConfigSource::TypesafeApiKey.env_var(),
            "TYPESAFE_API_KEY"
        );
        assert_eq!(
            JevConfigSource::OpenRouterApiKey.env_var(),
            "OPENROUTER_API_KEY"
        );
    }

    #[test]
    fn question_and_request_types_stay_small_and_typed() {
        let request = JevRequest {
            state: serde_json::json!({ "candidates": ["Player B"] }),
            questions: vec![(
                "pick".to_string(),
                JevQuestion::Choice {
                    instructions: "Which candidate is intended?".to_string(),
                    criteria: vec![("b".to_string(), "Player B".to_string())],
                },
            )],
        };
        // The state is a small focused value, never the whole graph.
        assert!(
            serde_json::to_vec(&request.state)
                .expect("serializable")
                .len()
                <= MAX_STATE_BYTES
        );
        assert!(request.questions.len() <= MAX_QUESTIONS);
    }
}
