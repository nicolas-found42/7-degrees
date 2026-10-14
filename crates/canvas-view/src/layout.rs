//! Pluggable graph layout algorithms for the Rust graph visualization engine.
//!
//! Inspired by AntV G6's layout pipeline, these layouts take graph topology
//! and compute (x, y) 2D coordinates for nodes.

use crate::{GraphLink, GraphNode};
use std::collections::HashMap;

/// Layout mode selection matching G6-style layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LayoutMode {
    /// Horizontal timeline placement by explicit debut season, with stable ID
    /// hashing to spread nodes within each season lane.
    #[default]
    EraLane,
    /// Concentric circles arranged radially by degree of separation / hop distance
    /// from the active player, ideal for focal ego-network investigation.
    Concentric,
    /// Organic force-directed layout using spring-embedder simulation
    /// to cluster tightly connected teammates.
    Force,
}

impl LayoutMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EraLane => "era",
            Self::Concentric => "concentric",
            Self::Force => "force",
        }
    }

    pub fn from_str_name(name: &str) -> Option<Self> {
        match name {
            "era" | "lane" => Some(Self::EraLane),
            "concentric" | "radial" => Some(Self::Concentric),
            "force" | "spring" => Some(Self::Force),
            _ => None,
        }
    }
}

/// Computes coordinates for nodes based on graph topology and chosen layout mode.
pub fn compute_layout(
    nodes: &[GraphNode],
    links: &[GraphLink],
    mode: LayoutMode,
    focus: &str,
    path: &[String],
    full_network: bool,
    hop_distances: &HashMap<String, usize>,
) -> Vec<(f64, f64)> {
    match mode {
        LayoutMode::EraLane => compute_era_lane_layout(nodes),
        LayoutMode::Concentric => {
            compute_concentric_layout(nodes, links, focus, path, full_network, hop_distances)
        }
        LayoutMode::Force => compute_force_layout(nodes, links, focus, path, full_network),
    }
}

/// Era-lane layout: x encodes debut year (a real ordinal attribute the canvas
/// is the only place to show), y spreads players deterministically within the
/// lane. The formula is the long-standing canvas placement, kept here as the
/// single source of truth so the WASM renderer and tests agree on coordinates.
fn compute_era_lane_layout(nodes: &[GraphNode]) -> Vec<(f64, f64)> {
    nodes
        .iter()
        .map(|node| {
            let year = f64::from(node.debut_season.unwrap_or(1980));
            let hash = node.id.bytes().fold(2166136261u32, |h, b| {
                (h ^ u32::from(b)).wrapping_mul(16777619)
            });
            let jitter = f64::from(hash % 1000) / 1000.0;
            let lane = f64::from((hash / 1000) % 10000) / 10000.0;
            (
                (year - 1947.0) * 110.0 + jitter * 105.0,
                (lane - 0.5) * 5200.0,
            )
        })
        .collect()
}

/// Concentric circle layout: concentric rings arranged by degree of separation
/// (hop distance) from the focused player node at the center.
fn compute_concentric_layout(
    nodes: &[GraphNode],
    links: &[GraphLink],
    focus: &str,
    path: &[String],
    full_network: bool,
    hop_distances: &HashMap<String, usize>,
) -> Vec<(f64, f64)> {
    if !full_network {
        return compute_path_neighborhood_layout(nodes, focus, path);
    }

    let mut coords = vec![(0.0, 0.0); nodes.len()];
    let mut rings: HashMap<usize, Vec<usize>> = HashMap::new();

    for (i, node) in nodes.iter().enumerate() {
        let dist = if node.id == focus {
            0
        } else if let Some(&d) = hop_distances.get(&node.id) {
            d
        } else {
            3 // Unconnected or >2 hops
        };
        rings.entry(dist).or_default().push(i);
    }

    // Center the focus node at (0, 0)
    for &idx in rings.get(&0).unwrap_or(&Vec::new()) {
        coords[idx] = (0.0, 0.0);
    }

    let groups: HashMap<&str, &str> = links
        .iter()
        .filter_map(|e| {
            let other = if e.from == focus {
                e.to.as_str()
            } else if e.to == focus {
                e.from.as_str()
            } else {
                return None;
            };
            Some((
                other,
                e.franchises
                    .iter()
                    .map(String::as_str)
                    .min()
                    .unwrap_or(&e.team),
            ))
        })
        .collect();
    for (dist, indices) in &mut rings {
        if *dist == 0 {
            continue;
        }
        indices.sort_by_key(|&i| {
            (
                groups.get(nodes[i].id.as_str()).copied().unwrap_or("~"),
                nodes[i].debut_season.unwrap_or(u32::MAX),
                nodes[i].id.as_str(),
            )
        });
        let (inner, outer) = match dist {
            1 => (220.0_f64, 460.0_f64),
            2 => (620.0_f64, 1020.0_f64),
            _ => (1300.0_f64, 1900.0_f64),
        };
        let n = indices.len();
        for (k, &idx) in indices.iter().enumerate() {
            // Equal-area radial spread and a golden-angle increment avoid thin rings.
            let fraction = (k as f64 * 0.618033988749895).fract();
            let radius = (inner * inner + fraction * (outer * outer - inner * inner)).sqrt();
            let angle = std::f64::consts::TAU * k as f64 / n as f64;
            coords[idx] = (radius * angle.cos(), radius * angle.sin());
        }
    }

    coords
}

/// Force-directed layout: iterative spring-embedder layout that pulls connected
/// teammates close while repelling overlapping nodes.
fn compute_force_layout(
    nodes: &[GraphNode],
    links: &[GraphLink],
    focus: &str,
    path: &[String],
    full_network: bool,
) -> Vec<(f64, f64)> {
    if !full_network {
        return compute_path_neighborhood_layout(nodes, focus, path);
    }

    // Start with era-lane coordinates as initial positions
    let mut coords = compute_era_lane_layout(nodes);

    let id_to_index: HashMap<&str, usize> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    // Map links to index pairs
    let indexed_links: Vec<(usize, usize)> = links
        .iter()
        .filter_map(|l| {
            let a = id_to_index.get(l.from.as_str())?;
            let b = id_to_index.get(l.to.as_str())?;
            Some((*a, *b))
        })
        .collect();

    // Run a bounded, deterministic relaxation (e.g. 15 iterations)
    // to give organic clustering without freezing the thread
    let k_spring = 0.05;
    let ideal_length = 60.0;
    let iterations = 15;

    for _ in 0..iterations {
        let mut deltas = vec![(0.0f64, 0.0f64); nodes.len()];

        // Spring attractive forces along edges
        for &(a, b) in &indexed_links {
            let p1 = coords[a];
            let p2 = coords[b];
            let dx = p2.0 - p1.0;
            let dy = p2.1 - p1.1;
            let dist = dx.hypot(dy).max(1.0);
            let force = (dist - ideal_length) * k_spring;
            let fx = (dx / dist) * force;
            let fy = (dy / dist) * force;

            deltas[a].0 += fx;
            deltas[a].1 += fy;
            deltas[b].0 -= fx;
            deltas[b].1 -= fy;
        }

        // Apply clamped displacements
        for (i, d) in deltas.into_iter().enumerate() {
            coords[i].0 += d.0.clamp(-25.0, 25.0);
            coords[i].1 += d.1.clamp(-25.0, 25.0);
        }
    }

    coords
}

/// Fallback / focused path layout for neighborhood or shortest-chain views.
fn compute_path_neighborhood_layout(
    nodes: &[GraphNode],
    focus: &str,
    path: &[String],
) -> Vec<(f64, f64)> {
    let center = path.iter().position(|id| id == focus).unwrap_or(0) as f64 * 170.0;
    let off_path = nodes
        .iter()
        .filter(|n| !path.contains(&n.id) && !(path.is_empty() && n.id == focus))
        .count();
    let mut extra = 0;

    nodes
        .iter()
        .map(|node| {
            if let Some(index) = path.iter().position(|id| id == &node.id) {
                (index as f64 * 170.0, 0.0)
            } else if path.is_empty() && node.id == focus {
                (0.0, 0.0)
            } else {
                let count_in_ring = (off_path - (extra / 12) * 12).clamp(1, 12);
                let angle = std::f64::consts::TAU * (extra % 12) as f64 / count_in_ring as f64
                    - std::f64::consts::FRAC_PI_2;
                let radius = 150.0 + (extra / 12) as f64 * 100.0;
                extra += 1;
                (center + radius * angle.cos(), radius * angle.sin())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_mode_serialization_and_parsing() {
        assert_eq!(LayoutMode::EraLane.as_str(), "era");
        assert_eq!(LayoutMode::Concentric.as_str(), "concentric");
        assert_eq!(LayoutMode::Force.as_str(), "force");

        assert_eq!(LayoutMode::from_str_name("era"), Some(LayoutMode::EraLane));
        assert_eq!(
            LayoutMode::from_str_name("concentric"),
            Some(LayoutMode::Concentric)
        );
        assert_eq!(LayoutMode::from_str_name("force"), Some(LayoutMode::Force));
        assert_eq!(LayoutMode::from_str_name("unknown"), None);
    }

    #[test]
    fn era_lane_layout_uses_explicit_debut_season() {
        let nodes = vec![
            GraphNode {
                debut_season: Some(1980),
                degree: 0,
                id: "A".into(),
                name: "Player A".into(),
                era: "1980–1990 seasons".into(),
                teams: vec!["LAL".into()],
                distance: None,
            },
            GraphNode {
                debut_season: Some(1985),
                degree: 0,
                id: "B".into(),
                name: "Player B".into(),
                era: "1985–1995 seasons".into(),
                teams: vec!["LAL".into()],
                distance: None,
            },
        ];
        let links = vec![GraphLink {
            franchises: Vec::new(),
            from: "A".into(),
            to: "B".into(),
            team: "LAL".into(),
            overlap_days: Some(300),
            minimum_shared_games: Some(50),
            on_path: true,
        }];
        let hop: HashMap<String, usize> = HashMap::from([("A".into(), 0), ("B".into(), 1)]);
        let era_coords = compute_layout(&nodes, &links, LayoutMode::EraLane, "A", &[], true, &hop);
        // (x = (year − 1947) · 110 + jitter · 105, y = (lane − ½) · 5200).
        for (actual, expected) in era_coords
            .iter()
            .zip([(3673.26, 2141.36), (4208.25, 2314.52)])
        {
            assert!(
                (actual.0 - expected.0).abs() < 0.01 && (actual.1 - expected.1).abs() < 0.01,
                "era-lane drift: {actual:?} vs {expected:?}"
            );
        }
        // Era-lane x must stay ordered by debut year: the timeline is the layout.
        assert!(era_coords[0].0 < era_coords[1].0);
    }

    #[test]
    fn concentric_layout_rings_follow_hop_distance() {
        let nodes = vec![
            GraphNode {
                debut_season: Some(1980),
                degree: 0,
                id: "A".into(),
                name: "Player A".into(),
                era: "1980–1990 seasons".into(),
                teams: vec!["LAL".into()],
                distance: Some(0),
            },
            GraphNode {
                debut_season: Some(1985),
                degree: 0,
                id: "B".into(),
                name: "Player B".into(),
                era: "1985–1995 seasons".into(),
                teams: vec!["LAL".into()],
                distance: Some(1),
            },
        ];
        let links = vec![GraphLink {
            franchises: Vec::new(),
            from: "A".into(),
            to: "B".into(),
            team: "LAL".into(),
            overlap_days: Some(300),
            minimum_shared_games: Some(50),
            on_path: true,
        }];
        let hop: HashMap<String, usize> = HashMap::from([("A".into(), 0), ("B".into(), 1)]);
        let coords = compute_layout(&nodes, &links, LayoutMode::Concentric, "A", &[], true, &hop);
        assert_eq!(coords.len(), 2);
        assert_eq!(coords[0], (0.0, 0.0));
        let r_b = coords[1].0.hypot(coords[1].1);
        assert!((220.0..=460.0).contains(&r_b));
    }
}
