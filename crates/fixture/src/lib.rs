//! Synthetic roster fixture, from the spec's Concrete Example (binding).
//!
//! Spec fixture core (binding): Player A on Team Red `[day 1, day 5)`, B on
//! Team Red `[day 2, day 4)`, B on Team Blue `[day 10, day 13)`, C on Team
//! Blue `[day 11, day 12)`; expected edges A–B and B–C with A→C at 2 degrees
//! unless a direct A–C edge exists — and there is none.
//!
//! Fixture-only extensions (synthetic, for rule coverage):
//! - C2: Team Blue `[11, 15)` — overlaps B's Blue stint; the mid-season-move
//!   rule means B links to C2 through the Blue stint, while A only reaches C2
//!   indirectly through B
//! - A also plays Team Green `[6, 11)`
//! - E: Team Green `[6, 8)` and again `[9, 11)` — two overlaps with A that
//!   must collapse to exactly one A–E edge
//! - D: Team Red `[20, 25)` — joins Red only after A's and B's Red tenures
//!   end, so shared Red franchise history creates no edge
//!
//! Expected teammate edges (5, undirected, deduplicated):
//! - A–B (Red, 2 days), B–C (Blue, 1 day), B–C2 (Blue, 2 days), C–C2
//!   (Blue, 1 day), A–E (Green, 2 + 2 = 4 days across two stints, one edge)

use graph_core::{LocatedTenure, Player, RosterData, Team, TeammateEdge, Tenure};

/// A fixture player: stable id plus the display name the UI shows.
#[derive(Clone, Debug)]
pub struct FixturePlayer {
    pub id: String,
    pub name: String,
}

/// The fixture's teams as `(id, name)` pairs (spec: Teams Red and Blue; Green
/// exists only as the second team of the repeated-overlap player A/E).
pub fn fixture_teams() -> Vec<(String, String)> {
    vec![
        ("Red".to_string(), "Team Red".to_string()),
        ("Blue".to_string(), "Team Blue".to_string()),
        ("Green".to_string(), "Team Green".to_string()),
    ]
}

/// The fixture's roster tenures (half-open day intervals).
pub fn fixture_tenures() -> Vec<LocatedTenure> {
    vec![
        loc("A", "Red", 1, 5),
        loc("A", "Green", 6, 11),
        loc("B", "Red", 2, 4),
        loc("B", "Blue", 10, 13),
        loc("C", "Blue", 11, 12),
        loc("C2", "Blue", 11, 15),
        loc("D", "Red", 20, 25),
        loc("E", "Green", 6, 8),
        loc("E", "Green", 9, 11),
    ]
}

fn loc(player: &str, team: &str, start: u32, end: u32) -> LocatedTenure {
    LocatedTenure {
        player: player.to_string(),
        team: team.to_string(),
        tenure: Tenure {
            start: graph_core::Day(start),
            end: graph_core::Day(end),
        },
    }
}

/// Build `RosterData` for a player list and the fixture teams.
///
/// The fixture owns the data-to-graph input path: `RosterData` is the same
/// struct a real data import (T2) will fill from bulk source records, so the
/// API integration tests exercise the data-build path rather than graph
/// internals.
pub fn roster_data(players: &[FixturePlayer], teams: &[(String, String)]) -> RosterData {
    RosterData {
        players: players
            .iter()
            .map(|p| Player {
                id: p.id.clone(),
                name: p.name.clone(),
            })
            .collect(),
        teams: teams
            .iter()
            .map(|(id, name)| Team {
                id: id.clone(),
                name: name.clone(),
            })
            .collect(),
        tenures: fixture_tenures(),
    }
}

/// Expected teammate edge pairs after the dedup rule, as `(a, b, summed
/// overlap days)`, in the stable `(a, b)` order.
pub fn expected_edges() -> Vec<(&'static str, &'static str, u32)> {
    vec![
        ("A", "B", 2),
        ("A", "E", 4),
        ("B", "C", 1),
        ("B", "C2", 2),
        ("C", "C2", 1),
    ]
}

/// One edge row: ordered endpoints and the summed overlap days over all their
/// overlapping stints.
#[derive(Clone, Debug)]
pub struct EdgeRow {
    pub a: String,
    pub b: String,
    pub overlap_days: u32,
}

/// Summary of the fixture graph as seen through the app interface.
#[derive(Clone, Debug)]
pub struct FixtureSummaryData {
    pub players: Vec<FixturePlayer>,
    pub edges: Vec<EdgeRow>,
}

/// Build the full fixture edge list via `RosterData::teammate_edges` (the
/// data-to-graph path).
pub fn fixture_edge_list() -> Vec<EdgeRow> {
    let roster = RosterData {
        players: Vec::new(),
        teams: fixture_teams()
            .into_iter()
            .map(|(id, name)| Team { id, name })
            .collect(),
        tenures: fixture_tenures(),
    };
    roster
        .teammate_edges()
        .iter()
        .map(|TeammateEdge { a, b, evidence }| EdgeRow {
            a: a.clone(),
            b: b.clone(),
            overlap_days: evidence.iter().map(|e| e.overlap_days).sum(),
        })
        .collect()
}

/// The connection result returned through the app interface.
#[derive(Clone, Debug)]
pub enum FixtureConnection {
    Connected {
        path: Vec<String>,
        links: Vec<LinkRow>,
        degree: usize,
    },
    Disconnected,
}

/// One link row in a connection result.
#[derive(Clone, Debug)]
pub struct LinkRow {
    pub from: String,
    pub to: String,
    pub team: String,
    pub overlap_days: u32,
}

/// Answer "connect from → to" over the fixture graph. `None` when either player
/// id is unknown to the fixture.
pub fn fixture_connection(
    players: &[FixturePlayer],
    from: &str,
    to: &str,
) -> Option<FixtureConnection> {
    let known: Vec<&str> = players.iter().map(|p| p.id.as_str()).collect();
    if !known.contains(&from) || !known.contains(&to) {
        return None;
    }
    let graph = graph_core::TeammateGraph::build(roster_data(players, &fixture_teams()));
    match graph.shortest_chain(from, to) {
        Some(graph_core::Connection::Connected(chain)) => {
            let degree = chain.links.len();
            Some(FixtureConnection::Connected {
                path: chain.path,
                links: chain
                    .links
                    .into_iter()
                    .map(|l| LinkRow {
                        from: l.from,
                        to: l.to,
                        team: l.team,
                        overlap_days: l.overlap_days,
                    })
                    .collect(),
                degree,
            })
        }
        Some(graph_core::Connection::Disconnected) | None => Some(FixtureConnection::Disconnected),
    }
}

/// Build the fixture summary (players + edges).
pub fn fixture_summary(players: &[FixturePlayer]) -> FixtureSummaryData {
    FixtureSummaryData {
        players: players.to_vec(),
        edges: fixture_edge_list(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture's documentation (`expected_edges`) must stay consistent with
    /// what the data-to-graph path actually computes.
    #[test]
    fn computed_edges_match_documented_expectations() {
        let computed = fixture_edge_list();
        let expected = expected_edges();
        assert_eq!(computed.len(), expected.len(), "edge count matches docs");
        for row in &computed {
            let match_found = expected
                .iter()
                .any(|(a, b, days)| row.a == *a && row.b == *b && row.overlap_days == *days);
            assert!(
                match_found,
                "edge {:?} is documented in expected_edges",
                row
            );
        }
    }
}
