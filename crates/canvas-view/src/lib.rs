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
    #[serde(default = "default_weight")]
    pub weight: f64,
    #[serde(default = "default_backbone")]
    pub in_visual_backbone: bool,
    #[serde(default)]
    pub primary_franchise: String,
}
fn default_weight() -> f64 {
    0.15
}
fn default_backbone() -> bool {
    true
}

impl GraphLink {
    pub fn primary_franchise(&self) -> &str {
        if !self.primary_franchise.is_empty() {
            &self.primary_franchise
        } else {
            self.franchises
                .iter()
                .min()
                .map(String::as_str)
                .unwrap_or(&self.team)
        }
    }
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
pub mod visual;
pub use layout::LayoutMode;

#[cfg(target_arch = "wasm32")]
mod browser;
