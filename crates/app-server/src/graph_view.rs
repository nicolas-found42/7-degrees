//! Bounded deterministic neighborhoods and the SSR canvas surface.
use crate::{AppState, ui};
use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use canvas_view::{GraphLink, GraphNode, GraphPayload};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Canvas assets belong to one router. Tests can independently exercise a built
/// directory or an unavailable canvas without mutating process configuration.
#[derive(Clone, Debug)]
pub enum CanvasAssets {
    Directory(std::path::PathBuf),
    Unavailable,
}
impl Default for CanvasAssets {
    fn default() -> Self {
        Self::Directory(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/canvas-web"),
        )
    }
}
impl CanvasAssets {
    pub(crate) fn from_environment() -> Self {
        std::env::var_os("NBA_CANVAS_ASSET_DIR")
            .map(std::path::PathBuf::from)
            .map(Self::Directory)
            .unwrap_or_default()
    }
    fn root(&self) -> Option<&std::path::Path> {
        match self {
            Self::Directory(root) => Some(root),
            Self::Unavailable => None,
        }
    }
    fn available(&self) -> bool {
        self.root().is_some_and(|root| {
            root.join("canvas_view.js").is_file() && root.join("canvas_view_bg.wasm").is_file()
        })
    }
}

#[derive(Deserialize)]
pub struct NeighborhoodQuery {
    pub player: String,
    pub depth: Option<usize>,
    pub limit: Option<usize>,
}

fn node(state: &AppState, id: &str, distance: Option<usize>) -> GraphNode {
    if let Some(context) = state.reports.as_ref().and_then(|r| r.players.get(id)) {
        return GraphNode {
            id: id.into(),
            name: context.name.clone(),
            era: format!("{}–{} seasons", context.first_season, context.last_season),
            teams: context.teams.clone(),
            distance,
        };
    }
    let tenures: Vec<_> = state
        .graph
        .roster
        .tenures
        .iter()
        .filter(|t| t.player == id)
        .collect();
    let teams: BTreeSet<_> = tenures.iter().map(|t| t.team.clone()).collect();
    let era = match (
        tenures.iter().map(|t| t.tenure.start.0).min(),
        tenures.iter().map(|t| t.tenure.end.0).max(),
    ) {
        (Some(start), Some(end)) => format!("Synthetic fixture days {start}–{end}"),
        _ => "Synthetic fixture; no roster tenure".into(),
    };
    GraphNode {
        id: id.into(),
        name: ui::display_name(&state.graph.roster.players, id).into(),
        era,
        teams: teams.into_iter().collect(),
        distance,
    }
}

fn neighborhood(
    state: &AppState,
    player: &str,
    depth: usize,
    limit: usize,
    path: &[String],
) -> GraphPayload {
    let mut distances = BTreeMap::from([(player.to_string(), 0)]);
    let mut queue = VecDeque::from([player.to_string()]);
    while let Some(id) = queue.pop_front() {
        let next = distances[&id] + 1;
        if next > depth {
            continue;
        }
        for neighbor in state.graph.neighbor_ids(&id) {
            if !distances.contains_key(neighbor) {
                distances.insert(neighbor.into(), next);
                queue.push_back(neighbor.into());
            }
        }
    }
    let mut ranked: Vec<_> = distances.iter().collect();
    ranked.sort_by_key(|(id, d)| (**d, *id));
    let mut ids: BTreeSet<String> = path.iter().cloned().collect();
    ids.insert(player.into());
    let mut truncated = false;
    for (index, (id, _)) in ranked.into_iter().enumerate() {
        if index < limit {
            ids.insert(id.clone());
        } else if !ids.contains(id) {
            truncated = true;
        }
    }
    let nodes = ids
        .iter()
        .map(|id| node(state, id, distances.get(id).copied()))
        .collect();
    let mut links = Vec::new();
    // Only the visible players' own adjacency is read; ids and neighbors are both sorted, so
    // the links keep the stable (a, b) order of the full edge list.
    let visible_edges = ids.iter().flat_map(|id| {
        state
            .graph
            .neighbor_ids(id)
            .into_iter()
            .filter(|other| id.as_str() < *other && ids.contains(*other))
            .filter_map(|other| state.graph.edge_between(id, other))
            .collect::<Vec<_>>()
    });
    for edge in visible_edges {
        let on_path = path
            .windows(2)
            .any(|p| (p[0] == edge.a && p[1] == edge.b) || (p[1] == edge.a && p[0] == edge.b));
        if links.len() >= 600 && !on_path {
            truncated = true;
            continue;
        }
        let evidence = edge.strongest_evidence();
        let (team, overlap_days, minimum_shared_games) = (
            evidence.team.clone(),
            evidence.overlap_days,
            evidence.minimum_shared_games(),
        );
        links.push(GraphLink {
            from: edge.a,
            to: edge.b,
            team,
            overlap_days,
            minimum_shared_games,
            on_path,
        });
    }
    GraphPayload {
        full_network: false,
        nodes,
        links,
        path: path.to_vec(),
        degree: (!path.is_empty()).then(|| path.len() - 1),
        focus: player.into(),
        coverage: state
            .reports
            .as_ref()
            .map(|r| r.coverage.warning.clone())
            .unwrap_or_else(|| "Synthetic fixture; complete within this example.".into()),
        truncated,
    }
}

pub async fn api(
    State(state): State<AppState>,
    Query(query): Query<NeighborhoodQuery>,
) -> Response {
    let depth = query.depth.unwrap_or(1);
    let limit = query.limit.unwrap_or(60);
    if !(1..=2).contains(&depth) || !(1..=200).contains(&limit) {
        return (StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"invalid-neighborhood","message":"depth must be 1 or 2; limit must be between 1 and 200"}))).into_response();
    }
    if !state.graph.contains_player(&query.player) {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error":"unknown-player"})),
        )
            .into_response();
    }
    Json(neighborhood(&state, &query.player, depth, limit, &[])).into_response()
}

fn network(state: &AppState, focus: &str, path: &[String]) -> GraphPayload {
    let nodes = state
        .graph
        .roster
        .players
        .iter()
        .map(|p| node(state, &p.id, None))
        .collect();
    let links = state
        .graph
        .edges()
        .into_iter()
        .map(|edge| {
            let proof = edge.strongest_evidence();
            let on_path = path
                .windows(2)
                .any(|p| (p[0] == edge.a && p[1] == edge.b) || (p[1] == edge.a && p[0] == edge.b));
            GraphLink {
                team: proof.team.clone(),
                overlap_days: proof.overlap_days,
                minimum_shared_games: proof.minimum_shared_games(),
                on_path,
                from: edge.a,
                to: edge.b,
            }
        })
        .collect();
    GraphPayload {
        full_network: true,
        nodes,
        links,
        path: path.to_vec(),
        degree: (!path.is_empty()).then(|| path.len() - 1),
        focus: focus.into(),
        coverage: state
            .reports
            .as_ref()
            .map(|r| r.coverage.warning.clone())
            .unwrap_or_else(|| "Synthetic fixture; complete within this example.".into()),
        truncated: false,
    }
}

pub async fn network_api(State(state): State<AppState>) -> Response {
    let focus = state
        .graph
        .roster
        .players
        .first()
        .map(|p| p.id.as_str())
        .unwrap_or("");
    Json(network(&state, focus, &[])).into_response()
}

/// The graph exploration page: the one page this application serves. It renders
/// every evidenced player and every teammate connection in a single canvas,
/// with the same complete data as an accessible text fallback. Player-specific
/// navigation (chains, alternatives, neighborhoods) is deliberately out of
/// scope here.
///
/// The canvas is initialised from the embedded `#graph-payload`; the WASM
/// renderer requires it, so an unavailable canvas leaves an explanatory note.
pub async fn page(State(state): State<AppState>) -> Response {
    if state.graph.roster.players.is_empty() {
        return ui::document(
            StatusCode::OK,
            "7 Degrees — Graph",
            "<h1>Graph exploration</h1><p>0 players · 0 connections. No players are present in this snapshot.</p>",
        );
    }
    let payload = network(&state, "", &[]);
    let json = serde_json::to_string(&payload).expect("graph facts serialize");
    let players: String = payload
        .nodes
        .iter()
        .enumerate()
        .map(|(index, n)| {
            format!(
                "<li><button id=\"select-node-{index}\" type=\"button\" data-select-player=\"{id}\">{name}</button> — {era}; teams: {teams}</li>",
                id = ui::escape(&n.id),
                name = ui::escape(&n.name),
                era = ui::escape(&n.era),
                teams = ui::escape(&n.teams.join(", "))
            )
        })
        .collect();
    let buttons: String = [
        ("zoom-in", "Zoom in"),
        ("zoom-out", "Zoom out"),
        ("pan-left", "Pan left"),
        ("pan-right", "Pan right"),
        ("pan-up", "Pan up"),
        ("pan-down", "Pan down"),
        ("fit-network", "Fit whole network"),
        ("focus-player", "Zoom to selected player"),
    ]
    .iter()
    .map(|(id, name)| format!("<button type=\"button\" id=\"{id}\">{name}</button>"))
    .collect();
    let canvas_status = if state.canvas_assets.available() {
        "Canvas loading; the text lists remain available."
    } else {
        "Canvas unavailable; use the accessible player list below."
    };
    let canvas_loader = if state.canvas_assets.available() {
        "<script type=\"module\" src=\"/assets/canvas-loader.js\"></script>"
    } else {
        ""
    };
    ui::document(
        StatusCode::OK,
        "7 Degrees — Graph",
        &format!(
            "<h1>Graph exploration</h1>{}<section class=\"graph-view\" data-full-network=\"true\"><h2>All players · all teammate connections</h2><details><summary>Source coverage is incomplete</summary><p>{coverage}</p></details><p class=\"network-legend\">Every player and every evidenced connection is loaded. Players are arranged by debut season; colors mark eras. Zoom in to read names, then select a player in the list below or a relationship on the canvas.</p><canvas id=\"graph-canvas\" width=\"1000\" height=\"600\" tabindex=\"0\" aria-label=\"Interactive teammate graph\" aria-describedby=\"graph-help graph-selection\">Use the player list below to explore; relationships are selected on the canvas.</canvas><p id=\"graph-help\">Drag to pan; scroll to zoom. Arrow keys pan; + and − zoom. Select a player or relationship in the canvas or lists.</p><p id=\"canvas-status\" role=\"status\" data-ready=\"false\">{canvas_status}</p><div class=\"controls\">{buttons}</div><p id=\"graph-selection\" role=\"status\"></p><p id=\"selected-edge\" role=\"status\">Select a teammate relationship to inspect its graph facts.</p><section id=\"edge-provenance-panel\"><h3>Selected relationship evidence</h3><p>Select a relationship to load its overlap evidence here, or open any evidence link in a separate page.</p><p><a id=\"open-selected-edge\" href=\"about:blank\" target=\"_blank\" rel=\"noopener\">Open selected relationship evidence in a separate page</a></p><iframe id=\"edge-provenance-frame\" title=\"Selected teammate overlap evidence\" src=\"about:blank\" sandbox=\"allow-popups allow-popups-to-escape-sandbox\"></iframe></section><h3>Players</h3><ul id=\"graph-players\">{players}</ul><h3>Connections of selected player</h3><ul id=\"network-player-links\"></ul><script type=\"application/json\" id=\"graph-payload\">{serialized}</script>{canvas_loader}</section>",
            ui::semantic_status_line(state.jev.status()),
            coverage = ui::escape(&payload.coverage),
            serialized = json
                .replace('<', "\\u003c")
                .replace('>', "\\u003e")
                .replace('&', "\\u0026"),
        ),
    )
}

/// Generated wasm-bindgen assets only; arbitrary filesystem paths are rejected.
pub async fn asset(
    State(state): State<AppState>,
    axum::extract::Path(asset): axum::extract::Path<String>,
) -> Response {
    if asset == "canvas-loader.js" {
        return (
            StatusCode::OK,
            [(
                axum::http::header::CONTENT_TYPE,
                "text/javascript; charset=utf-8",
            )],
            "import init from '/assets/canvas_view.js'; await init();",
        )
            .into_response();
    }
    let mime = match asset.as_str() {
        "canvas_view.js" => "text/javascript; charset=utf-8",
        "canvas_view_bg.wasm" => "application/wasm",
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let bytes = state
        .canvas_assets
        .root()
        .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound))
        .and_then(|root| std::fs::read(root.join(asset)));
    match bytes {
        Ok(bytes) => (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, mime)],
            bytes,
        )
            .into_response(),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "Canvas assets unavailable; accessible lists remain available.",
        )
            .into_response(),
    }
}
