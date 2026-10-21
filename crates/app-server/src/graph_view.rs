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
    /// The asset files this canvas serves, with their paths.
    ///
    /// The diagnostics surface names them with their content digests, so an
    /// answer can be attributed to the exact bundle that produced it.
    pub fn files(&self) -> Vec<(String, std::path::PathBuf)> {
        match self.root() {
            Some(root) => ["canvas_view.js", "canvas_view_bg.wasm"]
                .into_iter()
                .map(|name| (name.to_string(), root.join(name)))
                .collect(),
            None => Vec::new(),
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
            debut_season: Some(context.first_season),
            degree: state.graph.neighbor_ids(id).len(),
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
        debut_season: None,
        degree: state.graph.neighbor_ids(id).len(),
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
        let index = state
            .visual_links
            .binary_search_by(|link| {
                (link.from.as_str(), link.to.as_str()).cmp(&(edge.a.as_str(), edge.b.as_str()))
            })
            .expect("complete cached visual graph");
        let mut link = state.visual_links[index].clone();
        link.on_path = on_path;
        links.push(link);
    }
    GraphPayload {
        full_network: false,
        nodes,
        links,
        path: path.to_vec(),
        degree: (!path.is_empty()).then(|| path.len() - 1),
        focus: player.into(),
        target: String::new(),
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

/// Fixed saturation scales: five roster years or three full appearance seasons.
/// Units remain separate visual mappings; neither mapping certifies duration.
pub fn edge_visual_weight(edge: &graph_core::TeammateEdge) -> (f64, String) {
    fn mapped(count: u64, saturation: f64) -> f64 {
        if count == 0 {
            0.1
        } else {
            (0.15 + 0.85 * (count as f64).ln_1p() / saturation.ln_1p()).clamp(0.1, 1.0)
        }
    }
    let mut groups: BTreeMap<&str, (u64, u64, bool)> = BTreeMap::new();
    for proof in &edge.evidence {
        let group = groups.entry(&proof.team).or_default();
        group.0 += u64::from(proof.overlap_days.unwrap_or(0));
        if let Some(p) = &proof.appearance_proof {
            // N is the admitted season's actual total, including shortened seasons.
            group.1 += (u64::from(p.a_games) + u64::from(p.b_games))
                .saturating_sub(u64::from(p.team_games));
        }
        group.2 |= proof.game_witness.is_some();
    }
    let strength = |days, games, witness| {
        mapped(days, 1825.0)
            .max(mapped(games, 246.0))
            .max(if witness { 0.15 } else { 0.1 })
    };
    let mut primary = "";
    let mut best = 0.0;
    let (mut days, mut games, mut witness) = (0, 0, false);
    for (team, (d, g, w)) in groups {
        let weight = strength(d, g, w);
        if weight > best {
            best = weight;
            primary = team;
        }
        days += d;
        games += g;
        witness |= w;
    }
    (strength(days, games, witness), primary.to_owned())
}

/// Derived once from the complete evidence, before any view or filter exists.
pub(crate) fn visual_links(graph: &graph_core::TeammateGraph) -> Vec<GraphLink> {
    let mut links: Vec<_> = graph
        .edges()
        .into_iter()
        .map(|edge| {
            let proof = edge.strongest_evidence();
            let (weight, primary_franchise) = edge_visual_weight(&edge);
            GraphLink {
                team: proof.team.clone(),
                overlap_days: proof.overlap_days,
                minimum_shared_games: proof.minimum_shared_games(),
                franchises: edge
                    .evidence
                    .iter()
                    .map(|e| e.team.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                from: edge.a,
                to: edge.b,
                on_path: false,
                weight,
                primary_franchise,
                in_visual_backbone: false,
            }
        })
        .collect();
    let nodes: Vec<_> = graph
        .roster
        .players
        .iter()
        .map(|p| GraphNode {
            id: p.id.clone(),
            name: p.name.clone(),
            era: String::new(),
            teams: Vec::new(),
            distance: None,
            debut_season: None,
            degree: graph.neighbor_ids(&p.id).len(),
        })
        .collect();
    canvas_view::visual::mark_backbone(&nodes, &mut links);
    links.sort_by(|a, b| (&a.from, &a.to).cmp(&(&b.from, &b.to)));
    links
}

pub(crate) fn discovery_nodes(state: &AppState) -> Vec<GraphNode> {
    state
        .graph
        .roster
        .players
        .iter()
        .map(|p| node(state, &p.id, None))
        .collect()
}

fn network(state: &AppState, focus: &str, path: &[String]) -> GraphPayload {
    let nodes = state
        .graph
        .roster
        .players
        .iter()
        .map(|p| node(state, &p.id, None))
        .collect();
    let mut links = state.visual_links.as_ref().clone();
    for link in &mut links {
        link.on_path = path.windows(2).any(|p| {
            (p[0] == link.from && p[1] == link.to) || (p[1] == link.from && p[0] == link.to)
        });
    }
    GraphPayload {
        full_network: true,
        nodes,
        links,
        path: path.to_vec(),
        degree: (!path.is_empty()).then(|| path.len() - 1),
        focus: focus.into(),
        target: String::new(),
        coverage: state
            .reports
            .as_ref()
            .map(|r| r.coverage.warning.clone())
            .unwrap_or_else(|| "Synthetic fixture; complete within this example.".into()),
        truncated: false,
    }
}

pub async fn network_api(State(state): State<AppState>) -> Response {
    Json(network(&state, "", &[])).into_response()
}

#[derive(Default, Deserialize)]
pub struct ExplorerQuery {
    player: Option<String>,
    from: Option<String>,
    to: Option<String>,
}

/// The compact runtime identity of this answer: which build answers, and how
/// large the snapshot it answers from is. The full identity, with the data
/// digests, is served by `/api/diagnostics`.
fn runtime_identity(state: &AppState) -> String {
    let commit = crate::telemetry::build_commit();
    let short: String = commit.chars().take(7).collect();
    format!(
        "Build <code>{}</code> · {} players · {} edges · <a href=\"/api/diagnostics\">diagnostics</a>",
        ui::escape(&short),
        state.graph.roster.players.len(),
        state.graph.edge_count()
    )
}

/// Whole-network explorer with a native GET-form fallback for exact chains.
pub async fn page(State(state): State<AppState>, Query(query): Query<ExplorerQuery>) -> Response {
    let from = query.from.or(query.player).unwrap_or_default();
    let to = query.to.unwrap_or_default();
    let mut path = Vec::new();
    let mut chain = String::new();
    if (!from.is_empty() && !state.graph.contains_player(&from))
        || (!to.is_empty() && !state.graph.contains_player(&to))
    {
        chain
            .push_str("<p class=\"error\">Unknown player endpoint. Choose an existing player.</p>");
    } else if !from.is_empty() && !to.is_empty() {
        match state.graph.shortest_chain(&from, &to).expect("validated players") {
            graph_core::Connection::Connected(result) => {
                path = result.path;
                chain.push_str(&format!("<h3>Shortest teammate chain · degree {}</h3><ol>", result.links.len()));
                for (index, id) in path.iter().enumerate() {
                    chain.push_str(&format!("<li><button type=\"button\" data-center-player=\"{}\">{}</button>", ui::escape(id), ui::escape(ui::display_name(&state.graph.roster.players, id))));
                    if let Some(link) = result.links.get(index) {
                        chain.push_str(&format!(" <a href=\"/edge?from={}&amp;to={}\" data-evidence> {} · {}</a>", ui::url_encode(&link.from), ui::url_encode(&link.to), ui::escape(&link.team), ui::escape(&api_types::overlap_description(link.overlap_days, link.minimum_shared_games))));
                    }
                    chain.push_str("</li>");
                }
                chain.push_str("</ol>");
            }
            graph_core::Connection::Disconnected => chain.push_str(if state.reports.is_some() {"<p>No teammate chain in this evidenced graph. Missing evidence may connect these players historically.</p>"} else {"<p>No teammate chain in this complete synthetic fixture.</p>"}),
        }
    }
    let valid_from = if state.graph.contains_player(&from) {
        from.as_str()
    } else {
        ""
    };
    let mut payload = network(&state, valid_from, &path);
    payload.target = if state.graph.contains_player(&to) {
        to.clone()
    } else {
        String::new()
    };
    let json = serde_json::to_string(&payload)
        .expect("graph facts serialize")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026");
    let options = |selected: &str| -> String {
        payload
            .nodes
            .iter()
            .map(|n| {
                format!(
                    "<option value=\"{}\"{}>{} — {}; {} ({})</option>",
                    ui::escape(&n.id),
                    if n.id == selected { " selected" } else { "" },
                    ui::escape(&n.name),
                    ui::escape(&n.era),
                    ui::escape(&n.teams.join(", ")),
                    ui::escape(&n.id)
                )
            })
            .collect()
    };
    let button = |id: &str, name: &str| {
        format!("<button type=\"button\" id=\"{id}\" aria-label=\"{name}\">{name}</button>")
    };
    let camera: String = [
        ("zoom-in", "Zoom in"),
        ("zoom-out", "Zoom out"),
        ("pan-left", "Pan left"),
        ("pan-right", "Pan right"),
        ("pan-up", "Pan up"),
        ("pan-down", "Pan down"),
        ("fit-network", "Reset view"),
    ]
    .iter()
    .map(|(id, name)| button(id, name))
    .collect();
    let layouts: String = [
        ("layout-orbit", "Orbit layout"),
        ("layout-era", "Era lanes layout"),
        ("layout-concentric", "Concentric rings layout"),
        ("layout-force", "Force-directed layout"),
    ]
    .iter()
    .map(|(id, name)| button(id, name))
    .collect();
    let layouts = if payload.nodes.is_empty() {
        layouts.replace("type=\"button\"", "type=\"button\" disabled")
    } else {
        layouts
    };
    let colors = [
        "#1b9e77", "#d95f02", "#7570b3", "#e7298a", "#65a41e", "#bb8b02", "#a6761d", "#666666",
    ];
    let eras: String = colors.iter().enumerate().map(|(i,color)|format!("<li><button type=\"button\" id=\"era-{i}\" aria-pressed=\"true\"><span class=\"swatch\" style=\"background:{color}\" aria-hidden=\"true\"></span>{}{} debut</button></li>",1940+i*10,if i==7 {"s+"} else {"s"})).collect();
    let available = state.canvas_assets.available();
    let disabled = if payload.nodes.len() < 2 {
        " disabled title=\"At least two players are required\""
    } else {
        ""
    };
    let selection = payload
        .nodes
        .iter()
        .find(|n| n.id == valid_from)
        .map(|n| {
            format!(
                "Selected player: {} — {}; teams: {}; {} teammate connections",
                ui::escape(&n.name),
                ui::escape(&n.era),
                ui::escape(&n.teams.join(", ")),
                n.degree
            )
        })
        .unwrap_or_default();
    let empty = if payload.nodes.is_empty() {
        "<p>0 players · 0 connections. No players are present in this snapshot.</p>"
    } else {
        ""
    };
    ui::document(
        StatusCode::OK,
        "7 Degrees — Graph",
        &format!(
            r#"<header class="app-header"><a class="brand" href="/" aria-label="7 Degrees home"><span class="brand-mark">7</span><span>Degrees<span class="brand-caption">THE TEAMMATE NETWORK</span></span></a><p>Every career has a connection.</p><details class="about-network"><summary>About this network</summary><div><p class="coverage-warning">{coverage}</p>{semantic}<p class="runtime-identity">{identity}</p><p>Lines represent evidenced NBA / BAA teammates, not friendship or time played together.</p></div></details></header>
<section class="graph-view" data-full-network="true" aria-label="Graph exploration"><div class="workspace-toolbar"><form method="get" action="/graph" id="pair-form"><div class="picker primary-picker"><label for="graph-player">Find a player</label><select name="from" id="graph-player"><option value="">Search the teammate network</option>{a}</select></div><button class="native-submit" type="submit">Find player</button></form><div class="scope-switch" role="group" aria-label="Network view"><button type="button" id="scope-full" aria-pressed="true">Full network</button><button type="button" id="ego-toggle" aria-pressed="false" disabled>Selected</button></div><details id="advanced-controls"><summary>Filters &amp; layout</summary><div class="advanced-panel"><h3>Layout</h3><div class="controls" aria-label="Graph layout">{layouts}</div><h3>Emphasize debut eras</h3><ul class="legend" aria-label="Debut era filters">{eras}</ul><h3>Selected player's franchises</h3><div id="franchise-filters" class="controls" aria-label="Selected player franchises"></div><button type="button" id="reset-filters">Reset filters</button><p id="filter-status" role="status">All eras emphasized; whole network visible.</p></div></details></div>
<div class="workspace"><div class="canvas-container"><div class="map-heading"><span class="eyebrow">NBA / BAA · TEAMMATE ATLAS</span><h1>A league of connections.</h1><p id="scope-description">The whole network. Start with a name, or follow your curiosity.</p></div>{empty}<canvas id="graph-canvas" width="1000" height="600" tabindex="0" aria-label="Interactive teammate graph" aria-describedby="graph-help graph-selection">Use the player pickers to explore.</canvas><div class="map-legend"><span><i class="legend-player"></i>Player</span><span><i class="legend-line"></i>Teammates</span><span><i class="legend-selected"></i>Selected</span></div><button type="button" id="minimap-toggle" aria-expanded="false" aria-controls="graph-minimap">Overview map</button><canvas id="graph-minimap" width="200" height="120" aria-label="Network overview" hidden></canvas><div class="controls camera-controls">{camera}</div><div id="graph-help"><button type="button" id="dismiss-help" aria-label="Dismiss map guidance">×</button><strong>Dots are players. Lines connect teammates.</strong><span>Drag the background to pan; scroll to zoom. Drag a player to gently move their connections. Arrow keys pan; + / − zoom; F resets.</span></div><p id="graph-tooltip" role="status" hidden></p></div>
<aside id="inspector" aria-label="Player and relationship details"><div id="player-content"><span class="eyebrow">YOUR STARTING POINT</span><div id="player-summary"><h2>Find your player.</h2><p>Explore the careers that cross paths. Search a name above or select a dot on the map.</p></div><p id="graph-selection" class="sr-only" role="status" data-player="{focus}">{selection}</p><div id="player-actions" hidden><button type="button" id="focus-player">Focus on player</button><button type="button" id="find-connection" class="primary">Find connection</button></div><details id="connection-controls"><summary>Connect two players</summary><div class="picker"><label for="graph-target">Connect to</label><select name="to" form="pair-form" id="graph-target"><option value="">Find another player</option>{b}</select></div><div class="controls"><button type="submit" form="pair-form" class="primary">Connect players</button><button type="button" id="clear-chain">Clear chain</button><button type="button" id="random-pair"{disabled}>Random pair</button></div></details><section id="graph-chain" aria-label="Shortest teammate chain" role="status">{chain}</section><section class="orbit-walk"><button type="button" id="orbit-explore">Explore from start player</button><p>Choose a teammate to build a shortest chain. Earlier players and their teammates are unavailable.</p><ol id="orbit-next"></ol></section><section class="orbit-discovery" aria-label="Network discoveries"><h3>Discover</h3><div class="controls"><button type="button" id="discover-longest">Longest 10</button><button type="button" id="discover-hubs">Most connected 10</button></div><p id="discovery-status" role="status">Choose a ranking to explore.</p><ol id="discovery-list"></ol></section><details id="teammates-details" hidden><summary>Teammates &amp; evidence</summary><div id="teammates-list"></div></details></div><dialog id="edge-evidence-drawer" aria-labelledby="drawer-title"><button type="button" id="close-evidence">Back to player</button><h2 id="drawer-title">Relationship evidence</h2><p id="drawer-status" role="status"></p><div id="drawer-content"></div></dialog><div class="inspector-note">One link. One shared team.<br>Decades of basketball history.</div></aside></div><footer class="workspace-footer"><p id="canvas-status" role="status" data-ready="false">{canvas_status}</p><span>7 DEGREES · EXPLORE THE GAME</span></footer><script type="application/json" id="graph-payload">{json}</script>{loader}</section>"#,
            semantic = ui::semantic_status_line(state.jev.status()),
            coverage = ui::escape(&payload.coverage),
            identity = runtime_identity(&state),
            a = options(valid_from),
            b = options(&to),
            focus = ui::escape(valid_from),
            canvas_status = if available {
                "Canvas loading; the player picker remains available."
            } else {
                "Canvas unavailable; use the player picker below."
            },
            loader = if available {
                "<script type=\"module\" src=\"/assets/canvas-loader.js\"></script>"
            } else {
                ""
            }
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
            "Canvas assets unavailable; the player picker remains available.",
        )
            .into_response(),
    }
}
