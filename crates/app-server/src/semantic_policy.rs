//! Frozen calibration policy consumed by runtime defaults and experiment tooling.
//! The small labeled set does not establish general basketball-query accuracy.
use serde::Deserialize;
use std::sync::OnceLock;
#[derive(Deserialize)]
pub(crate) struct Policy {
    pub resolution: f64,
    pub existence: f64,
    pub query: f64,
    pub ranking: f64,
    pub margin: f64,
}
pub(crate) fn calibrated() -> &'static Policy {
    static POLICY: OnceLock<Policy> = OnceLock::new();
    POLICY.get_or_init(|| {
        serde_json::from_str(include_str!("../../../docs/evaluation/policy.json"))
            .expect("committed calibrated semantic policy")
    })
}
