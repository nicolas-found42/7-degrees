//! Pluggable graph layout algorithms for the Rust graph visualization engine.
//!
//! Inspired by AntV G6's layout pipeline, these layouts take graph topology
//! and compute (x, y) 2D coordinates for nodes.

use crate::{GraphLink, GraphNode};
use std::collections::{BTreeMap, HashMap};
mod force;

/// Layout mode selection matching G6-style layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LayoutMode {
    /// Horizontal timeline placement by explicit debut season, with stable ID
    /// hashing to spread nodes within each season lane.
    #[default]
    EraLane,
    /// Chain in an inner arc, surrounding context in a quiet halo.
    Orbit,
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
            Self::Orbit => "orbit",
            Self::Concentric => "concentric",
            Self::Force => "force",
        }
    }

    pub fn from_str_name(name: &str) -> Option<Self> {
        match name {
            "orbit" => Some(Self::Orbit),
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
        LayoutMode::Orbit => crate::orbit::layout(nodes, links, focus, path),
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

    let (groups, weights) = focus_groups(nodes, links, focus, hop_distances);
    let sectors = concentric_sectors(links, focus);
    let mut weighted = rings.get(&1).cloned().unwrap_or_default();
    weighted.sort_by(|&a, &b| {
        weights
            .get(nodes[b].id.as_str())
            .copied()
            .unwrap_or(0.1)
            .total_cmp(&weights.get(nodes[a].id.as_str()).copied().unwrap_or(0.1))
            .then_with(|| nodes[a].debut_season.cmp(&nodes[b].debut_season))
            .then_with(|| nodes[a].id.cmp(&nodes[b].id))
    });
    let ranks: HashMap<_, _> = weighted
        .iter()
        .enumerate()
        .map(|(rank, &i)| {
            (
                i,
                rank as f64 / weighted.len().saturating_sub(1).max(1) as f64,
            )
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
        let mut grouped: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for &i in indices.iter() {
            grouped
                .entry(groups.get(nodes[i].id.as_str()).copied().unwrap_or("~"))
                .or_default()
                .push(i);
        }
        let mut offset = 0;
        for (team, members) in grouped {
            for (k, &i) in members.iter().enumerate() {
                let fraction = if *dist == 1 {
                    ranks[&i]
                } else {
                    (k as f64 * 0.618033988749895).fract()
                };
                let radius = if *dist == 1 {
                    inner + fraction * (outer - inner)
                } else {
                    (inner * inner + fraction * (outer * outer - inner * inner)).sqrt()
                };
                let angle = if *dist <= 2 {
                    sectors
                        .iter()
                        .find(|sector| franchise_key(&sector.franchise) == team)
                        .map(|sector| {
                            sector.start
                                + (sector.end - sector.start) * (k as f64 + 0.5)
                                    / members.len() as f64
                        })
                        .unwrap_or(
                            std::f64::consts::TAU * (offset + k) as f64 / indices.len() as f64,
                        )
                } else {
                    std::f64::consts::TAU * (offset + k) as f64 / indices.len() as f64
                };
                coords[i] = (radius * angle.cos(), radius * angle.sin());
            }
            offset += members.len();
        }
    }
    coords
}

#[derive(Clone, Debug)]
pub struct FranchiseSector {
    pub franchise: String,
    pub start: f64,
    pub end: f64,
}

fn franchise_key(team: &str) -> &str {
    if team.is_empty() { "~" } else { team }
}

type FocusGroups<'a> = (HashMap<&'a str, &'a str>, HashMap<&'a str, f64>);
fn focus_groups<'a>(
    nodes: &'a [GraphNode],
    links: &'a [GraphLink],
    focus: &str,
    hop: &HashMap<String, usize>,
) -> FocusGroups<'a> {
    let mut groups = HashMap::new();
    let mut weights = HashMap::new();
    for e in links {
        let other = if e.from == focus {
            e.to.as_str()
        } else if e.to == focus {
            e.from.as_str()
        } else {
            continue;
        };
        groups.insert(other, franchise_key(e.primary_franchise()));
        weights.insert(other, e.weight.clamp(0.1, 1.0));
    }
    // Assign two-hop players through the first evidenced predecessor in stable ID order.
    let mut direct: Vec<_> = nodes
        .iter()
        .filter(|n| hop.get(&n.id) == Some(&1))
        .collect();
    direct.sort_by_key(|n| &n.id);
    for n in direct {
        let team = groups.get(n.id.as_str()).copied().unwrap_or("~");
        for e in links {
            let other = if e.from == n.id {
                e.to.as_str()
            } else if e.to == n.id {
                e.from.as_str()
            } else {
                continue;
            };
            if hop.get(other) == Some(&2) {
                groups.entry(other).or_insert(team);
            }
        }
    }
    (groups, weights)
}
/// Labeled angular intervals shared by placement and rendering.
pub fn concentric_sectors(links: &[GraphLink], focus: &str) -> Vec<FranchiseSector> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for e in links {
        if e.from == focus || e.to == focus {
            *counts.entry(e.primary_franchise()).or_default() += 1;
        }
    }
    let n = counts.len();
    if n == 0 {
        return Vec::new();
    }
    let gap = 0.055_f64.min(1.0 / n as f64);
    let available = std::f64::consts::TAU - gap * n as f64;
    let minimum = 0.18_f64.min(available / (2.0 * n as f64));
    let total = counts.values().sum::<usize>() as f64;
    let mut start = -std::f64::consts::FRAC_PI_2;
    let mut ordered: Vec<_> = counts.into_iter().collect();
    ordered.sort_by_key(|(team, _)| (team.is_empty(), *team));
    ordered
        .into_iter()
        .map(|(team, count)| {
            let end = start + minimum + (available - minimum * n as f64) * count as f64 / total;
            let sector = FranchiseSector {
                franchise: team.into(),
                start,
                end,
            };
            start = end + gap;
            sector
        })
        .collect()
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

    force::compute(nodes, links, compute_era_lane_layout(nodes))
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
            weight: 0.8,
            in_visual_backbone: true,
            primary_franchise: "LAL".into(),
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
            weight: 0.8,
            in_visual_backbone: true,
            primary_franchise: "LAL".into(),
        }];
        let hop: HashMap<String, usize> = HashMap::from([("A".into(), 0), ("B".into(), 1)]);
        let coords = compute_layout(&nodes, &links, LayoutMode::Concentric, "A", &[], true, &hop);
        assert_eq!(coords.len(), 2);
        assert_eq!(coords[0], (0.0, 0.0));
        let r_b = coords[1].0.hypot(coords[1].1);
        assert!((220.0 - 1e-9..=460.0 + 1e-9).contains(&r_b));
    }
}
