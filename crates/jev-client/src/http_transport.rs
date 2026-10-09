//! Minimal HTTP transport for the TypeSafe System One API over OpenRouter.
//!
//! Request configuration verified live this session (see
//! `docs/reports/t11-jev-client.md` and the smoke-call record in the shared
//! notes):
//! - endpoint `POST https://openrouter.ai/api/v1/systemone`
//!   (TypeSafe docs: `POST /v1/systemone`; OpenRouter appends the path to its
//!   base URL `https://openrouter.ai/api`)
//! - header `Authorization: Bearer $OPENROUTER_API_KEY` plus
//!   `Content-Type: application/json`
//! - body `{ "model": "typesafe/jev-1.13", "state": ..., "questions": ... }`
//!   (state: string or structured value; questions: typed `noul`/`choice`/
//!   `score` with `instructions` and optional `criteria`)
//! - response envelope `{ "model", "answers": { id: { type, noul | choice +
//!   probabilities + confidence | score + legend + probabilities +
//!   confidence } }, "usage": ... }` (OpenRouter adds `id`, `provider`,
//!   `usage.cost` — pass-through, ignored)
//! - model routing: `typesafe/jev-1.13` is used as-is by OpenRouter (verified
//!   in the smoke response: `"model": "typesafe/jev-1.13-20260917"`)
//!
//! The transport holds the API key only in memory; it never logs the key or
//! the request body, and failures fail soft to
//! [`JevOutcome::Unavailable`](jev_client::JevOutcome::Unavailable).

use crate::{
    JevAnswer, JevClient, JevConfig, JevOutcome, JevQuestion, JevRequest, JevTransport,
    MAX_QUESTIONS, MAX_STATE_BYTES,
};
use log::warn;
use reqwest::Client;
use serde_json::{Map, Value, json};

/// The real `JevTransport`: async HTTP through `reqwest`, called from the
/// blocking-side wrapper.
#[derive(Clone)]
pub struct HttpJevTransport {
    http: Client,
}

impl std::fmt::Debug for HttpJevTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpJevTransport").finish_non_exhaustive()
    }
}

impl Default for HttpJevTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpJevTransport {
    /// A client with no timeouts wired (the config's timeout governs each
    /// request); connection pooling stays default.
    pub fn new() -> Self {
        Self {
            http: Client::new(),
        }
    }
}

impl JevTransport for HttpJevTransport {
    fn evaluate(&self, config: &JevConfig, request: &JevRequest) -> JevOutcome {
        self.evaluate_measured(config, request).outcome
    }
    fn evaluate_measured(&self, config: &JevConfig, request: &JevRequest) -> crate::JevEvaluation {
        // Prefer the ambient multi-thread runtime: `block_in_place` moves the
        // calling task off the worker so the blocking `block_on` is legal.
        if let Ok(handle) = tokio::runtime::Handle::try_current()
            && handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread
        {
            return tokio::task::block_in_place(|| {
                handle.block_on(self.evaluate_measured_async(config, request))
            });
        }
        // Outside a multi-thread runtime (tests on the current-thread flavor,
        // or plain threads): `block_on` is not legal here, so run the
        // blocking call on a dedicated OS thread. Every failure mode — a
        // runtime that fails to build, a panic inside the call — maps to
        // `Unavailable`; the transport must never panic on transport problems.
        let evaluated = std::thread::spawn({
            let this = self.clone();
            let config = config.clone();
            let request = request.clone();
            move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build();
                match runtime {
                    Ok(runtime) => {
                        runtime.block_on(this.evaluate_measured_async(&config, &request))
                    }
                    Err(_) => crate::JevEvaluation::unavailable(),
                }
            }
        })
        .join();
        evaluated.unwrap_or_else(|_| crate::JevEvaluation::unavailable())
    }
}

impl HttpJevTransport {
    /// One judgment call with the explicit timeout + one-retry policy:
    /// the request (including its retry) must fit the caller's timeout, so
    /// there are no unbounded hangs. Retries fire once on transport errors
    /// and on the documented retryable statuses (429 rate limit, 529
    /// overloaded); other statuses fail immediately.
    pub async fn evaluate_async(&self, config: &JevConfig, request: &JevRequest) -> JevOutcome {
        self.evaluate_measured_async(config, request).await.outcome
    }
    pub async fn evaluate_measured_async(
        &self,
        config: &JevConfig,
        request: &JevRequest,
    ) -> crate::JevEvaluation {
        let start = std::time::Instant::now();
        let mut evaluation = tokio::time::timeout(
            std::time::Duration::from_secs(config.timeout_secs.max(1)),
            self.evaluate_inner(config, request),
        )
        .await
        .unwrap_or_else(|_| {
            warn!("Jev provider call deadline exceeded");
            crate::JevEvaluation::unavailable()
        });
        evaluation.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        evaluation
    }
    async fn evaluate_inner(
        &self,
        config: &JevConfig,
        request: &JevRequest,
    ) -> crate::JevEvaluation {
        // Payload guardrails first: a too-large state or too many questions is
        // a programming error that must not silently reach the provider.
        let state_bytes = serde_json::to_vec(&request.state)
            .map(|bytes| bytes.len())
            .unwrap_or(usize::MAX);
        if state_bytes > MAX_STATE_BYTES || request.questions.is_empty() {
            return crate::JevEvaluation::unavailable();
        }
        if request.questions.len() > MAX_QUESTIONS {
            return crate::JevEvaluation::unavailable();
        }
        let Ok(state) = serde_json::to_value(&request.state) else {
            return crate::JevEvaluation::unavailable();
        };
        let url = format!("{}/v1/systemone", config.base_url.trim_end_matches('/'));
        let questions = wire_questions(request);
        // The documented request envelope: model, state, questions.
        let body = json!({
            "model": config.model,
            "state": state,
            "questions": questions,
        });
        let attempt = || {
            self.http
                .post(&url)
                .bearer_auth(&config.api_key)
                .json(&body)
                .timeout(std::time::Duration::from_secs(config.timeout_secs.max(1)))
                .send()
        };

        // One retry budget shared by all transport/status failure categories.
        // The outer deadline also covers retries and reading/parsing the body.
        for index in 0..2 {
            match attempt().await {
                Ok(response) if response.status().is_success() => {
                    return parse_answers(response).await;
                }
                Ok(response) if index == 0 && matches!(response.status().as_u16(), 429 | 529) => {
                    warn!("Jev provider retryable status; one retry remaining");
                }
                Ok(response) => {
                    warn!("Jev provider returned HTTP {}", response.status().as_u16());
                    return crate::JevEvaluation::unavailable();
                }
                Err(_) if index == 0 => {
                    warn!("Jev provider unreachable; one retry remaining");
                }
                Err(_) => {
                    warn!("Jev provider unreachable after one retry");
                    return crate::JevEvaluation::unavailable();
                }
            }
        }
        crate::JevEvaluation::unavailable()
    }
}

async fn parse_answers(response: reqwest::Response) -> crate::JevEvaluation {
    let text = match response.text().await {
        Ok(text) => text,
        Err(_) => return crate::JevEvaluation::unavailable(),
    };
    let outcome = parse_answers_from_str(&text);
    let metadata = serde_json::from_str::<Value>(&text)
        .ok()
        .filter(|_| outcome.is_available())
        .map(|v| crate::JevMetadata {
            model: v.get("model").and_then(Value::as_str).map(str::to_owned),
            provider: v.get("provider").and_then(Value::as_str).map(str::to_owned),
            request_id: v.get("id").and_then(Value::as_str).map(str::to_owned),
            input_tokens: v.pointer("/usage/input_tokens").and_then(Value::as_u64),
            output_tokens: v.pointer("/usage/output_tokens").and_then(Value::as_u64),
            cost_usd: v
                .pointer("/usage/cost")
                .and_then(Value::as_f64)
                .filter(|c| c.is_finite() && *c >= 0.0),
        })
        .filter(|m| m != &crate::JevMetadata::default());
    crate::JevEvaluation {
        outcome,
        metadata,
        elapsed_ms: 0.0,
    }
}

/// Fail-soft envelope parse: anything malformed (bad JSON, wrong envelope,
/// mistyped answers) yields `Unavailable` rather than a partial judgment.
pub fn parse_answers_from_str(text: &str) -> JevOutcome {
    let envelope: Value = match serde_json::from_str(text) {
        Ok(Value::Object(map)) => Value::Object(map),
        _ => return JevOutcome::Unavailable,
    };
    let Some(answers) = envelope.get("answers").and_then(Value::as_object) else {
        return JevOutcome::Unavailable;
    };
    if answers.is_empty() {
        return JevOutcome::Unavailable;
    }
    let mut parsed: Vec<(String, JevAnswer)> = Vec::with_capacity(answers.len());
    for (id, value) in answers {
        let Some(answer) = parse_answer(value) else {
            return JevOutcome::Unavailable;
        };
        parsed.push((id.clone(), answer));
    }
    JevOutcome::Answers(parsed)
}

fn parse_answer(value: &Value) -> Option<JevAnswer> {
    let kind = value.get("type")?.as_str()?;
    match kind {
        "noul" => Some(JevAnswer::Noul(value.get("noul")?.as_f64()?)),
        "choice" => {
            let choice = value.get("choice")?.as_str()?.to_string();
            let probabilities = string_distribution(value.get("probabilities")?)?;
            let confidence = value.get("confidence")?.as_f64()?;
            Some(JevAnswer::Choice(choice, probabilities, confidence))
        }
        "score" => {
            let score = value.get("score")?.as_f64()?;
            let probabilities = level_distribution(value.get("probabilities")?)?;
            let confidence = value.get("confidence")?.as_f64()?;
            Some(JevAnswer::Score(score, probabilities, confidence))
        }
        _ => None,
    }
}

/// Parse a Choice probability map: every key is an option id (string); any
/// malformed entry discards the whole answer.
fn string_distribution(value: &Value) -> Option<Vec<(String, f64)>> {
    let map = value.as_object()?;
    map.iter()
        .map(|(key, probability)| Some((key.to_string(), probability.as_f64()?)))
        .collect()
}

/// Parse a Score probability map: keys are level numbers (the documented
/// legend key format, e.g. `"0"`, `"1"`); any malformed entry discards the
/// whole answer.
fn level_distribution(value: &Value) -> Option<Vec<(u8, f64)>> {
    let map = value.as_object()?;
    let mut parsed: Vec<(u8, f64)> = Vec::with_capacity(map.len());
    for (key, probability) in map {
        let level = key.parse::<u8>().ok()?;
        parsed.push((level, probability.as_f64()?));
    }
    // Ordered by level for deterministic downstream consumption.
    parsed.sort_by_key(|(level, _)| *level);
    Some(parsed)
}

fn wire_questions(request: &JevRequest) -> Map<String, Value> {
    let mut questions = Map::new();
    for (id, question) in &request.questions {
        let wire = match question {
            JevQuestion::Noul {
                instructions,
                criteria,
            } => match criteria {
                Some(meanings) => json!({
                    "type": "noul",
                    "instructions": instructions,
                    "criteria": { "true": meanings.yes, "false": meanings.no },
                }),
                None => json!({
                    "type": "noul",
                    "instructions": instructions,
                }),
            },
            JevQuestion::Choice {
                instructions,
                criteria,
            } => json!({
                "type": "choice",
                "instructions": instructions,
                "criteria": criteria
                    .iter()
                    .map(|(option, description)| (option.clone(), json!(description)))
                    .collect::<Map<String, Value>>(),
            }),
            JevQuestion::Score {
                instructions,
                criteria,
            } => json!({
                "type": "score",
                "instructions": instructions,
                "criteria": criteria,
            }),
        };
        questions.insert(id.clone(), wire);
    }
    questions
}

/// Fail-soft constructor for the configured client: `None` when no env key is
/// present, so the server never holds a half-configured Jev client.
pub fn client_from_env() -> Option<JevClient<HttpJevTransport>> {
    match crate::config_from_env() {
        crate::EnvConfig::Configured(config, _source) => {
            Some(JevClient::new(config, HttpJevTransport::new()))
        }
        crate::EnvConfig::NoConfig => None,
    }
}
