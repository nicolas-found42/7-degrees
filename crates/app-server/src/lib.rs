//! The 7-degrees local app server: Axum routes over a deterministic
//! teammate graph, plus the minimal server-rendered UI.

pub mod chain_view;
mod fixture_data;
mod jev;
pub mod query;
pub mod report_data;
pub mod search;
mod stats_view;
mod ui;

use axum::{
    Router,
    extract::{Path, Query, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Json, Response},
    routing::get,
};
use serde::Deserialize;

use api_types::ErrorResponse;

pub use jev::JevHandle;

/// The log-capture buffer, available only in test-capture builds
/// (`RUSTFLAGS='--cfg test_capture'`). The integration tests assert
/// credential non-appearance against it.
#[cfg(test_capture)]
pub use jev::test_log_capture;

/// Installs the capture logger in test-capture builds (a no-op otherwise).
/// The capture test calls this itself: test binaries install no logger, so
/// the proof would be vacuous without an explicit install.
#[cfg(test_capture)]
pub use jev::install_capture_logger;

/// App state shared by all routes: the built teammate graph plus the Jev
/// client handle (credentials live only inside the handle's client).
#[derive(Clone)]
pub struct AppState {
    graph: std::sync::Arc<graph_core::TeammateGraph>,
    jev: JevHandle,
    reports: Option<std::sync::Arc<report_data::ReportMetadata>>,
}

/// The router with the fixture graph preloaded and Jev deterministically
/// unconfigured. Integration tests drive this directly through
/// `tower::ServiceExt`. Tests must not depend on the developer shell's
/// `OPENROUTER_API_KEY`, so this builder never reads the environment; the
/// binary's `main()` uses canonical reports and an env-driven Jev handle.
pub fn app_with_fixture_data() -> Router {
    app_with_jev(JevHandle::unconfigured())
}

/// The router over an explicit Jev handle (the test seam: scripted
/// transports prove fallback and credential isolation without a network).
pub fn app_with_jev(jev: JevHandle) -> Router {
    let state = AppState {
        graph: std::sync::Arc::new(fixture_data::fixture_graph()),
        jev,
        reports: None,
    };
    app(state)
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/fixture", get(fixture_summary))
        .route("/api/graph", get(fixture_summary))
        .route("/api/edges", get(all_edges))
        .route("/api/edges/{player}", get(player_edges))
        .route("/api/connection", get(connection))
        .route("/api/paths", get(all_paths))
        .route("/api/stats", get(stats))
        .route("/stats", get(statistics_page))
        .route("/api/coverage", get(coverage))
        .route("/api/coverage/{player}", get(player_coverage))
        .route("/api/players", get(players))
        .route("/api/query", get(query::api))
        .route("/query", get(query::page))
        .route("/api/search", get(search::api))
        .route("/search", get(search::page))
        .route("/players/{id}", get(search::profile))
        .route("/api/semantic-status", get(semantic_status))
        .route("/", get(home))
        .route("/chain", get(chain_page))
        .route("/style.css", get(style_css))
        .fallback(not_found)
        .with_state(state)
}

/// Unknown routes: JSON 404s under `/api/`, an HTML 404 page elsewhere.
async fn not_found(uri: Uri) -> Response {
    if uri.path().starts_with("/api/") {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "not-found".to_string(),
                message: "unknown API route".to_string(),
            }),
        )
            .into_response();
    }
    ui::not_found_page()
}

/// Supported real-data import seam; `root` contains T3 and T4 report folders.
pub fn app_with_report_data(
    root: impl AsRef<std::path::Path>,
    jev: JevHandle,
) -> Result<Router, String> {
    let (graph, reports) = report_data::load(root.as_ref())?;
    Ok(app(AppState {
        graph: std::sync::Arc::new(graph),
        jev,
        reports: Some(std::sync::Arc::new(reports)),
    }))
}

fn edge_json(state: &AppState, edge: graph_core::TeammateEdge) -> serde_json::Value {
    let evidence: Vec<_> = edge.evidence.iter().map(|e| {
        let mut overlaps = Vec::new();
        if let Some(reports) = &state.reports {
            let a = reports.records.get(&(edge.a.clone(), e.team.clone()));
            let b = reports.records.get(&(edge.b.clone(), e.team.clone()));
            for ra in a.into_iter().flatten() {
                for rb in b.into_iter().flatten() {
                    let start = ra.start_day.max(rb.start_day);
                    let end = ra.end_day.min(rb.end_day);
                    if start < end { overlaps.push(serde_json::json!({"start_day":start, "end_day":end, "records":[ra,rb]})); }
                }
            }
        }
        // Each evidence item identifies a precise overlapping record pair.
        serde_json::json!({"team":e.team, "overlap_days":e.overlap_days,
            "records": overlaps.first().and_then(|v| v.get("records")).cloned().unwrap_or_else(|| serde_json::json!([])),
            "overlaps":overlaps})
    }).collect();
    serde_json::json!({"a":edge.a,"b":edge.b,"overlap_days":edge.overlap_days(),"evidence":evidence})
}
async fn fixture_summary(State(state): State<AppState>) -> Json<serde_json::Value> {
    let players: Vec<_> = state
        .graph
        .roster
        .players
        .iter()
        .map(|p| serde_json::json!({"id":p.id,"name":p.name}))
        .collect();
    let edges: Vec<_> = state
        .graph
        .edges()
        .into_iter()
        .map(|e| edge_json(&state, e))
        .collect();
    Json(serde_json::json!({"players":players,"edges":edges}))
}
async fn all_edges(State(state): State<AppState>) -> Json<serde_json::Value> {
    let edges: Vec<_> = state
        .graph
        .edges()
        .into_iter()
        .map(|e| edge_json(&state, e))
        .collect();
    Json(serde_json::json!({"edges":edges}))
}

/// The teammate edges of one player, by id.
async fn player_edges(State(state): State<AppState>, Path(player): Path<String>) -> Response {
    if !state.graph.contains_player(&player) {
        return player_not_found(&state.graph, &player, &player);
    }
    let edges: Vec<_> = state
        .graph
        .edges()
        .into_iter()
        .filter(|edge| edge.a == player || edge.b == player)
        .map(|edge| edge_json(&state, edge))
        .collect();
    Json(serde_json::json!({ "edges": edges })).into_response()
}

#[derive(Deserialize)]
struct ConnectQuery {
    from: Option<String>,
    to: Option<String>,
    offset: Option<u64>,
    cursor: Option<String>,
    limit: Option<usize>,
}

async fn connection(State(state): State<AppState>, Query(query): Query<ConnectQuery>) -> Response {
    connection_response(&state, query.from.as_deref(), query.to.as_deref())
}

/// All shortest chains between the queried pair (alternative connections).
async fn all_paths(State(state): State<AppState>, Query(query): Query<ConnectQuery>) -> Response {
    let limit = query.limit.unwrap_or(100);
    if !(1..=500).contains(&limit) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid-limit".into(),
                message: "limit must be between 1 and 500".into(),
            }),
        )
            .into_response();
    }
    if query.cursor.is_some() && query.offset.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "invalid-cursor".into(),
                message: "use either cursor or offset".into(),
            }),
        )
            .into_response();
    }
    connection_response_all(
        &state,
        query.from.as_deref(),
        query.to.as_deref(),
        query.offset.unwrap_or(0),
        query.cursor.as_deref(),
        limit,
    )
}

fn connection_response(state: &AppState, from: Option<&str>, to: Option<&str>) -> Response {
    let (Some(from), Some(to)) = (from, to) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "missing-parameter".to_string(),
                message: "both from and to are required".to_string(),
            }),
        )
            .into_response();
    };
    if !state.graph.contains_player(from) || !state.graph.contains_player(to) {
        return player_not_found(&state.graph, from, to);
    }
    match state.graph.shortest_chain(from, to).expect("players exist") {
        graph_core::Connection::Connected(chain) => {
            let degree = chain.links.len();
            Json(serde_json::json!({"result":"connected", "path":chain.path,
                "degree":degree, "links": chain.links.into_iter().map(|l| api_types::LinkDto {
                    from:l.from,to:l.to,team:l.team,overlap_days:l.overlap_days,
                }).collect::<Vec<_>>(), "coverage": state.reports.as_ref().map(|r| &r.coverage)})).into_response()
        }
        graph_core::Connection::Disconnected => {
            Json(serde_json::json!({"result":"disconnected", "coverage":state.reports.as_ref().map(|r| &r.coverage),
                "certainty":if state.reports.is_some() {"unresolved_coverage"} else {"verified_disconnected"}})).into_response()
        }
    }
}

fn connection_response_all(
    state: &AppState,
    from: Option<&str>,
    to: Option<&str>,
    offset: u64,
    cursor: Option<&str>,
    limit: usize,
) -> Response {
    let (Some(from), Some(to)) = (from, to) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "missing-parameter".to_string(),
                message: "both from and to are required".to_string(),
            }),
        )
            .into_response();
    };
    if !state.graph.contains_player(from) || !state.graph.contains_player(to) {
        return player_not_found(&state.graph, from, to);
    }
    // Both players exist, but the pair may be unreachable (`None`), e.g.
    // spec fixture A–D: same franchise, non-overlapping tenures. A missing
    // reachability answer is a defined empty result, not a panic.
    let page = match cursor {
        Some(cursor) => match state
            .graph
            .shortest_chains_page_cursor(from, to, cursor, limit)
        {
            Ok(page) => page.expect("players exist"),
            Err(message) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "invalid-cursor".into(),
                        message,
                    }),
                )
                    .into_response();
            }
        },
        None => state
            .graph
            .shortest_chains_page(from, to, offset, limit)
            .expect("players exist"),
    };
    let paths: Vec<api_types::PathDto> = page
        .chains
        .into_iter()
        .map(|chain| api_types::PathDto {
            path: chain.path,
            links: chain
                .links
                .into_iter()
                .map(|l| api_types::LinkDto {
                    from: l.from,
                    to: l.to,
                    team: l.team,
                    overlap_days: l.overlap_days,
                })
                .collect(),
            degree: chain.degree,
        })
        .collect();
    Json(serde_json::json!({ "paths": paths, "total":page.total, "total_saturated":page.total_saturated,
        "total_exact":page.total_exact, "next_offset":page.next_offset, "next_cursor":page.next_cursor,
        "coverage":state.reports.as_ref().map(|r| &r.coverage) })).into_response()
}

fn player_not_found(graph: &graph_core::TeammateGraph, from: &str, to: &str) -> Response {
    let missing = if graph.contains_player(from) {
        to
    } else {
        from
    };
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "player-not-found".to_string(),
            message: format!("no player node for id {missing:?}"),
        }),
    )
        .into_response()
}

async fn statistics_page(State(state): State<AppState>) -> Response {
    stats_view::page(
        &state.graph,
        state.jev.status(),
        state.reports.as_ref().map(|r| r.coverage.warning.as_str()),
    )
}

async fn stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    let stats = state.graph.statistics();
    let histogram: serde_json::Map<String, serde_json::Value> = stats
        .histogram
        .iter()
        .map(|(length, pairs)| (length.to_string(), serde_json::json!(pairs)))
        .collect();
    Json(serde_json::json!({
        "players": stats.players,
        "components": stats.components,
        "diameter": stats.diameter,
        "unreachable_pairs": stats.unreachable_pairs,
        "histogram": histogram,
    }))
}

/// Historical uncertainty is served independently of graph facts.
async fn coverage(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(match &state.reports {
        Some(reports) => serde_json::to_value(&reports.coverage).expect("coverage serializes"),
        None => serde_json::json!({"complete":true,"warning":"Synthetic fixture coverage only"}),
    })
}
async fn player_coverage(State(state): State<AppState>, Path(player): Path<String>) -> Response {
    if !state.graph.contains_player(&player) {
        return player_not_found(&state.graph, &player, &player);
    }
    let gaps = state
        .reports
        .as_ref()
        .and_then(|r| r.gaps.get(&player))
        .cloned()
        .unwrap_or_default();
    Json(serde_json::json!({"player":player,"records":gaps,
        "complete":state.reports.is_none(),"coverage":state.reports.as_ref().map(|r| &r.coverage)}))
    .into_response()
}
async fn players(State(state): State<AppState>) -> Json<serde_json::Value> {
    let players: Vec<_> = match &state.reports {
        Some(reports) => reports
            .players
            .values()
            .map(|p| serde_json::to_value(p).expect("player serializes"))
            .collect(),
        None => state
            .graph
            .roster
            .players
            .iter()
            .map(|p| serde_json::json!({"id":p.id,"name":p.name,"aliases":[],"teams":[]}))
            .collect(),
    };
    Json(serde_json::json!({"players":players}))
}

/// Semantic-feature availability (acceptance criterion: the UI reports
/// semantic features as unavailable when Jev is unconfigured/unreachable).
/// The body carries only the availability tuple — never any credential
/// material, env var names, or provider identifiers.
async fn semantic_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let (available, reason) = state.jev.status();
    Json(serde_json::json!({
        "available": available,
        "reason": reason,
    }))
}

/// The minimal UI home page: the connect form and the fixture edge list,
/// rendered server-side in Rust.
async fn home(State(state): State<AppState>) -> Response {
    ui::home(
        &state.graph,
        state.jev.status(),
        state.reports.as_ref().map(|r| r.coverage.warning.as_str()),
    )
}

/// The `/chain` UI page: the shortest teammate chain between the queried pair
/// as ordered players and links, with its degree of separation.
async fn chain_page(
    State(state): State<AppState>,
    Query(query): Query<chain_view::ChainQuery>,
) -> Response {
    chain_view::page(
        &state.graph,
        query,
        state.jev.status(),
        state.reports.as_ref().map(|r| r.coverage.warning.as_str()),
    )
}

/// The UI stylesheet, authored in the server crate and served as a static
/// asset. No JavaScript ships anywhere in the UI.
async fn style_css() -> Response {
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
        ui::STYLE_CSS,
    )
        .into_response()
}

/// Serve the canonical NBA/BAA graph on localhost (explicit fixture mode for tests). Exposed for the binary entry
/// point (`src/main.rs`). Jev reads its key from the process env at startup
/// (fail-soft: unconfigured runs keep every deterministic feature).
pub async fn main() {
    jev::install_capture_logger();
    jev::init_logging();
    let jev = JevHandle::from_env();
    let router = if std::env::var("NBA_DATA_MODE").as_deref() == Ok("fixture") {
        app_with_jev(jev)
    } else {
        let root = std::env::var("NBA_REPORT_DIR").unwrap_or_else(|_| "docs/reports".into());
        app_with_report_data(root, jev).expect("load canonical T3/T4 report data")
    };
    let port: u16 = std::env::var("NBA_PORT")
        .unwrap_or_else(|_| "3000".into())
        .parse()
        .expect("valid NBA_PORT");
    let address = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .expect("bind local app port");
    let router = router.into_make_service();
    println!("7-degrees listening on http://{address}");
    axum::serve(listener, router).await.expect("server runs");
}
