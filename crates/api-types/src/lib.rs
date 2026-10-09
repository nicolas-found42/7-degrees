//! JSON DTOs shared by the Axum server and the Leptos WASM UI.

use serde::{Deserialize, Serialize};

/// A player node: stable id plus display name.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerDto {
    pub id: String,
    pub name: String,
}

/// One teammate edge: ordered endpoints and summed overlap days.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeammateEdgeDto {
    pub a: String,
    pub b: String,
    pub overlap_days: u32,
}

/// One teammate link in a connection result, including its evidence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkDto {
    pub from: String,
    pub to: String,
    pub team: String,
    pub overlap_days: u32,
}

/// Answer to "connect from → to": the shortest teammate chain as ordered
/// players and links, with its degree of separation.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum ConnectionResponse {
    /// A shortest teammate chain exists; `links.len() == degree`.
    Connected {
        path: Vec<String>,
        links: Vec<LinkDto>,
        degree: usize,
    },
    /// Both players are known but no teammate chain connects them.
    Disconnected,
}

/// One alternative shortest chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathDto {
    pub path: Vec<String>,
    pub links: Vec<LinkDto>,
    pub degree: usize,
}

/// Fixture summary served at `/api/fixture`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FixtureSummary {
    pub players: Vec<PlayerDto>,
    pub edges: Vec<TeammateEdgeDto>,
}

/// Error body for a request naming an unknown player.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}