//! Server-owned graph facts and the G6-inspired layout engine shared with the
//! Rust WASM canvas. Layouts compute coordinates; `browser` renders them.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub name: String,
    pub era: String,
    pub teams: Vec<String>,
    pub distance: Option<usize>,
    #[serde(default)]
    pub debut_season: Option<u32>,
    #[serde(default)]
    pub degree: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GraphLink {
    pub from: String,
    pub to: String,
    pub team: String,
    #[serde(default)]
    pub franchises: Vec<String>,
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
    #[serde(default)]
    pub target: String,
    pub coverage: String,
    pub truncated: bool,
}

pub mod layout;
pub use layout::LayoutMode;

#[cfg(target_arch = "wasm32")]
mod browser;
