//! Fixture-backed data-to-graph adapter: maps the synthetic roster fixture onto
//! the API DTOs through `graph_core`, the same path a real data import (T2)
//! will take from bulk source records to the served graph.

use fixture::{self as fixture_crate, FixtureConnection, FixturePlayer};

use api_types::{ConnectionResponse, FixtureSummary, LinkDto, TeammateEdgeDto};

/// The fixture's player ids in stable order.
pub const FIXTURE_PLAYERS: [&str; 6] = ["A", "B", "C", "C2", "D", "E"];

/// The fixture teams as `(id, name)` pairs (spec: Teams Red/Blue; Green exists
/// only as the second team of the repeated-overlap player E).
pub fn fixture_teams() -> Vec<(String, String)> {
    vec![
        ("Red".to_string(), "Team Red".to_string()),
        ("Blue".to_string(), "Team Blue".to_string()),
        ("Green".to_string(), "Team Green".to_string()),
    ]
}

/// The display name the UI shows for a fixture player.
pub fn fixture_display_name(id: &str) -> String {
    match id {
        "A" => "Player A".to_string(),
        "B" => "Player B".to_string(),
        "C" => "Player C".to_string(),
        "C2" => "Player C2".to_string(),
        "D" => "Player D".to_string(),
        "E" => "Player E".to_string(),
        other => other.to_string(),
    }
}

fn fixture_players() -> Vec<FixturePlayer> {
    FIXTURE_PLAYERS
        .iter()
        .map(|id| FixturePlayer { id: (*id).to_string(), name: fixture_display_name(id) })
        .collect()
}

/// Build the fixture teammate graph over the fixture players and teams.
pub fn fixture_graph() -> graph_core::TeammateGraph {
    let players = fixture_players();
    graph_core::TeammateGraph::build(fixture_crate::roster_data(&players, &fixture_teams()))
}

/// The fixture summary: players and the full teammate edge list.
pub fn fixture_summary() -> FixtureSummary {
    let players = fixture_players();
    let summary = fixture_crate::fixture_summary(&players);
    FixtureSummary {
        players: summary
            .players
            .iter()
            .map(|p| api_types::PlayerDto { id: p.id.clone(), name: p.name.clone() })
            .collect(),
        edges: summary
            .edges
            .iter()
            .map(|edge| TeammateEdgeDto {
                a: edge.a.clone(),
                b: edge.b.clone(),
                overlap_days: edge.overlap_days,
            })
            .collect(),
    }
}

/// All teammate edges in the fixture as DTOs.
pub fn fixture_edges() -> Vec<TeammateEdgeDto> {
    fixture_crate::fixture_edge_list()
        .iter()
        .map(|edge| TeammateEdgeDto {
            a: edge.a.clone(),
            b: edge.b.clone(),
            overlap_days: edge.overlap_days,
        })
        .collect()
}

/// Connection result for `from`–`to` over the fixture graph. `None` means
/// either player is unknown to the fixture (a 404 at the API layer).
pub fn fixture_connection(from: &str, to: &str) -> Option<ConnectionResponse> {
    let players = fixture_players();
    match fixture_crate::fixture_connection(&players, from, to)? {
        FixtureConnection::Connected { path, links, degree } => {
            Some(ConnectionResponse::Connected {
                path,
                links: links
                    .into_iter()
                    .map(|l| LinkDto {
                        from: l.from,
                        to: l.to,
                        team: l.team,
                        overlap_days: l.overlap_days,
                    })
                    .collect(),
                degree,
            })
        }
        FixtureConnection::Disconnected => Some(ConnectionResponse::Disconnected),
    }
}