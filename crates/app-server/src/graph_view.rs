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
    let all_edges = state.graph.edges();
    let mut adjacency: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for edge in &all_edges {
        adjacency.entry(&edge.a).or_default().insert(&edge.b);
        adjacency.entry(&edge.b).or_default().insert(&edge.a);
    }
    let mut distances = BTreeMap::from([(player.to_string(), 0)]);
    let mut queue = VecDeque::from([player.to_string()]);
    while let Some(id) = queue.pop_front() {
        let next = distances[&id] + 1;
        if next > depth {
            continue;
        }
        for neighbor in adjacency.get(id.as_str()).into_iter().flatten() {
            if !distances.contains_key(*neighbor) {
                distances.insert((*neighbor).into(), next);
                queue.push_back((*neighbor).into());
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
    for edge in all_edges
        .into_iter()
        .filter(|e| ids.contains(&e.a) && ids.contains(&e.b))
    {
        let on_path = path
            .windows(2)
            .any(|p| (p[0] == edge.a && p[1] == edge.b) || (p[1] == edge.a && p[0] == edge.b));
        if links.len() >= 600 && !on_path {
            truncated = true;
            continue;
        }
        let evidence = &edge.evidence[0];
        links.push(GraphLink {
            from: edge.a,
            to: edge.b,
            team: evidence.team.clone(),
            overlap_days: evidence.overlap_days,
            on_path,
        });
    }
    GraphPayload {
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

#[derive(Default, Deserialize)]
pub struct GraphQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub selected: Option<usize>,
    pub cursor: Option<String>,
    pub offset: Option<u64>,
    pub limit: Option<usize>,
    pub player: Option<String>,
    pub depth: Option<usize>,
    pub neighbor_limit: Option<usize>,
}

struct GraphSelection {
    chain: crate::chain_view::ChainQuery,
    player: Option<String>,
    depth: Option<usize>,
    neighbor_limit: Option<usize>,
}

pub async fn page(State(state): State<AppState>, Query(query): Query<GraphQuery>) -> Response {
    let query = GraphSelection {
        chain: crate::chain_view::ChainQuery {
            from: query.from,
            to: query.to,
            selected: query.selected,
            cursor: query.cursor,
            offset: query.offset,
            limit: query.limit,
        },
        player: query.player,
        depth: query.depth,
        neighbor_limit: query.neighbor_limit,
    };
    let failure = |status, message: &str| {
        ui::document(
            status,
            "7 Degrees — Graph",
            &format!(
                "<h1>Graph exploration</h1><p class=\"error\">{}</p><p><a href=\"/\">Back to explorer</a></p>",
                ui::escape(message)
            ),
        )
    };
    let depth = query.depth.unwrap_or(0);
    let neighbor_limit = query.neighbor_limit.unwrap_or(60);
    let limit = query.chain.limit.unwrap_or(20);
    if depth > 2
        || !(1..=200).contains(&neighbor_limit)
        || !(1..=100).contains(&limit)
        || (query.chain.cursor.is_some() && query.chain.offset.is_some())
    {
        return failure(
            StatusCode::BAD_REQUEST,
            "Invalid graph page: depth is 0–2, neighborhood limit is 1–200, chain page size is 1–100; use either cursor or offset.",
        );
    }
    let cursor = query
        .chain
        .cursor
        .clone()
        .unwrap_or_else(|| format!("v1:{}", query.chain.offset.unwrap_or(0)));
    let mut selected = None;
    match (query.chain.from.as_deref(), query.chain.to.as_deref()) {
        (Some(from), Some(to)) => {
            if !state.graph.contains_player(from) || !state.graph.contains_player(to) {
                return failure(StatusCode::NOT_FOUND, "Unknown endpoint player.");
            }
            let page = match state
                .graph
                .shortest_chains_page_cursor(from, to, &cursor, limit)
            {
                Ok(Some(page)) => page,
                Err(message) => return failure(StatusCode::BAD_REQUEST, &message),
                _ => return failure(StatusCode::NOT_FOUND, "Unknown endpoint player."),
            };
            if page.total_exact == "0" {
                return crate::chain_view::page(
                    &state.graph,
                    query.chain,
                    state.jev.status(),
                    state.reports.as_ref().map(|r| r.coverage.warning.as_str()),
                );
            }
            let Some(chain) = page.chains.get(query.chain.selected.unwrap_or(0)) else {
                return failure(
                    StatusCode::BAD_REQUEST,
                    "The selected alternative is outside this page.",
                );
            };
            selected = Some(crate::chain_view::SelectedChain::from_chain(
                &state.graph,
                chain,
            ));
        }
        (None, None)
            if query.player.is_some()
                && query.chain.selected.is_none()
                && query.chain.cursor.is_none()
                && query.chain.offset.is_none() => {}
        _ => {
            return failure(
                StatusCode::BAD_REQUEST,
                "Choose both chain endpoints or a single neighborhood player.",
            );
        }
    }
    let focus = query
        .player
        .as_deref()
        .or_else(|| {
            selected
                .as_ref()
                .and_then(|s| s.path.first().map(|p| p.id.as_str()))
        })
        .expect("selection validated");
    if !state.graph.contains_player(focus) {
        return failure(StatusCode::NOT_FOUND, "Unknown neighborhood player.");
    }
    let path: Vec<_> = selected
        .as_ref()
        .map(|s| s.path.iter().map(|p| p.id.clone()).collect())
        .unwrap_or_default();
    let payload = neighborhood(&state, focus, depth, neighbor_limit, &path);
    let json = serde_json::to_string(&payload).expect("graph facts serialize");
    let hidden = |name: &str, value: &str| {
        format!(
            "<input type=\"hidden\" name=\"{name}\" value=\"{}\">",
            ui::escape(value)
        )
    };
    let mut fields = String::new();
    let chain_link = if let (Some(from), Some(to)) = (&query.chain.from, &query.chain.to) {
        fields.push_str(&hidden("from", from));
        fields.push_str(&hidden("to", to));
        fields.push_str(&hidden("cursor", &cursor));
        fields.push_str(&hidden("limit", &limit.to_string()));
        fields.push_str(&hidden(
            "selected",
            &query.chain.selected.unwrap_or(0).to_string(),
        ));
        format!(
            "<p><a href=\"/chain?from={}&amp;to={}&amp;cursor={}&amp;limit={limit}&amp;selected={}\">Compare shortest alternatives</a></p>",
            ui::url_encode(from),
            ui::url_encode(to),
            ui::url_encode(&cursor),
            query.chain.selected.unwrap_or(0)
        )
    } else {
        String::new()
    };
    fields.push_str(&hidden("neighbor_limit", &neighbor_limit.to_string()));
    let options: String = payload
        .nodes
        .iter()
        .map(|n| {
            format!(
                "<option value=\"{}\"{}>{} — {}; {}</option>",
                ui::escape(&n.id),
                if n.id == focus { " selected" } else { "" },
                ui::escape(&n.name),
                ui::escape(&n.era),
                ui::escape(&n.teams.join(", "))
            )
        })
        .collect();
    let players:String=payload.nodes.iter().enumerate().map(|(index,n)|format!("<li><button id=\"select-node-{index}\" type=\"button\" data-select-player=\"{}\">{}</button> — {}; teams: {}</li>",ui::escape(&n.id),ui::escape(&n.name),ui::escape(&n.era),ui::escape(&n.teams.join(", ")))).collect();
    let edges:String=payload.links.iter().enumerate().map(|(index,e)|format!("<li><button id=\"select-edge-{index}\" type=\"button\" data-select-edge-from=\"{}\" data-select-edge-to=\"{}\">{} ↔ {}</button> — {}, {} overlap day(s){}</li>",ui::escape(&e.from),ui::escape(&e.to),ui::escape(ui::display_name(&state.graph.roster.players,&e.from)),ui::escape(ui::display_name(&state.graph.roster.players,&e.to)),ui::escape(&e.team),e.overlap_days,if e.on_path{"; selected chain link"}else{""})).collect();
    let buttons: String = [
        ("zoom-in", "Zoom in"),
        ("zoom-out", "Zoom out"),
        ("pan-left", "Pan left"),
        ("pan-right", "Pan right"),
        ("pan-up", "Pan up"),
        ("pan-down", "Pan down"),
        ("refocus", "Refocus selected chain"),
    ]
    .iter()
    .map(|(id, name)| format!("<button type=\"button\" id=\"{id}\">{name}</button>"))
    .collect();
    let selected_text = selected
        .as_ref()
        .map(crate::chain_view::render_selected_chain)
        .unwrap_or_else(|| {
            "<p>Neighborhood exploration; no player-to-player chain requested.</p>".into()
        });
    let canvas_status = if state.canvas_assets.available() {
        "Canvas loading; the text lists remain available."
    } else {
        "Canvas unavailable; use the accessible player and relationship lists."
    };
    ui::document(
        StatusCode::OK,
        "7 Degrees — Graph",
        &format!(
            "<h1>Graph exploration</h1>{}<p><a href=\"/\">Back to explorer</a></p>{chain_link}{selected_text}<section class=\"graph-view\"><h2>Selected chain and neighborhood</h2><p>{}</p>{}<canvas id=\"graph-canvas\" width=\"1000\" height=\"600\" tabindex=\"0\" aria-label=\"Interactive teammate graph\" aria-describedby=\"graph-help graph-selection\">Use the player and relationship lists below to explore.</canvas><p id=\"graph-help\">Drag to pan; scroll to zoom. Arrow keys pan; + and − zoom. Select a player or relationship in the canvas or lists.</p><p id=\"canvas-status\" role=\"status\" data-ready=\"false\">{canvas_status}</p><div class=\"controls\">{buttons}</div><form id=\"graph-expand\" class=\"controls\" action=\"/graph\" method=\"get\">{fields}<label for=\"graph-player\">Selected player</label><select id=\"graph-player\" name=\"player\">{options}</select><button name=\"depth\" value=\"1\">Expand direct connections</button><button name=\"depth\" value=\"2\">Expand nearby connections</button></form><p id=\"graph-selection\" role=\"status\"></p><p id=\"selected-edge\" role=\"status\">Select a teammate relationship to inspect its graph facts.</p><h3>Visible players</h3><ul id=\"graph-players\">{players}</ul><h3>Visible relationships</h3><ul id=\"graph-links\">{edges}</ul><template id=\"graph-payload\" data-payload=\"{}\"></template><script type=\"module\" src=\"/assets/canvas-loader.js\"></script></section>",
            ui::semantic_status_line(state.jev.status()),
            ui::escape(&payload.coverage),
            if payload.truncated {
                "<p class=\"hint\" data-truncated=\"true\">This bounded view omits some neighbors or relationships. Select a visible player to explore its neighborhood; the selected chain is retained.</p>"
            } else {
                ""
            },
            ui::escape(&json)
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
