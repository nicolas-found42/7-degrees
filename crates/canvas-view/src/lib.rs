//! Server-owned graph facts shared with the Rust WASM canvas.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub name: String,
    pub era: String,
    pub teams: Vec<String>,
    pub distance: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphLink {
    pub from: String,
    pub to: String,
    pub team: String,
    pub overlap_days: Option<u32>,
    #[serde(default)]
    pub minimum_shared_games: Option<u32>,
    pub on_path: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphPayload {
    #[serde(default)]
    pub full_network: bool,
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
    pub path: Vec<String>,
    pub degree: Option<usize>,
    pub focus: String,
    pub coverage: String,
    pub truncated: bool,
}

#[cfg(target_arch = "wasm32")]
mod browser;
