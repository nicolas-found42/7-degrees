//! Live-provider smoke test: runs the REAL HTTP transport against the REAL
//! configured provider with the REAL key from the process environment.
//!
//! Ignored by default (needs a real key + network); run explicitly with:
//!
//! ```sh
//! cargo test -p app-server --test live_smoke -- --ignored
//! ```
//!
//! The key is never printed: the assertion reads the env var into the config
//! and only the outcome's availability is asserted.

use jev_client::{JevAnswer, JevClient, JevOutcome, JevQuestion, JevRequest};
use serde_json::json;

fn real_config() -> Option<jev_client::JevConfig> {
    // The same precedence the server uses at startup.
    match jev_client::config_from_env() {
        jev_client::EnvConfig::Configured(config, _source) => Some(config),
        jev_client::EnvConfig::NoConfig => None,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "live provider call: needs a real key + network; run with -- --ignored"]
async fn live_provider_answers_one_small_judgment() {
    let Some(config) = real_config() else {
        panic!("no TypeSafe/OpenRouter key in the environment; this smoke test needs one");
    };
    let client = JevClient::new(config, jev_client::http_transport::HttpJevTransport::new());

    // One small judgment (costs a fraction of a cent): a Noul over a tiny
    // fixture-sized state, exactly the shape future tickets will send.
    let request = JevRequest {
        state: json!({
            "candidates": [
                { "id": "b", "display_name": "Player B", "teams": ["Red"] },
                { "id": "c", "display_name": "Player C", "teams": ["Blue"] }
            ],
            "query": "the player who was on Team Red"
        }),
        questions: vec![(
            "pick_candidate".to_string(),
            JevQuestion::Choice {
                instructions: "Which candidate in `candidates` matches the player in `query`?"
                    .to_string(),
                criteria: vec![
                    ("b".to_string(), "Player B, on Team Red".to_string()),
                    ("c".to_string(), "Player C, on Team Blue".to_string()),
                    (
                        "no_match".to_string(),
                        "No candidate matches the query".to_string(),
                    ),
                ],
            },
        )],
    };

    let outcome = client.evaluate(&request);
    let JevOutcome::Answers(answers) = outcome else {
        panic!(
            "live provider unreachable or misconfigured; see the warn! record for the transport error"
        );
    };
    let picked = answers
        .iter()
        .find(|(id, _)| id == "pick_candidate")
        .expect("the pick_candidate answer comes back");
    let JevAnswer::Choice(choice, _probabilities, confidence) = &picked.1 else {
        panic!("the choice question returns a choice answer");
    };
    assert_eq!(
        choice, "b",
        "the live model picks the Team Red candidate from the bounded state"
    );
    assert!(
        *confidence > 0.5,
        "an unambiguous two-candidate judgment is confident: {confidence}"
    );
}
