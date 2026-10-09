//! The 7-degrees local app server: Axum routes over the fixture-backed
//! teammate graph, plus the minimal server-rendered UI.

mod fixture_data;
mod jev;
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

/// App state shared by all routes: the built teammate graph plus the Jev
/// client handle (credentials live only inside the handle's client).
#[derive(Clone)]
pub struct AppState {
    graph: std::sync::Arc<graph_core::TeammateGraph>,
    jev: JevHandle,
}

/// The router with the fixture graph preloaded and Jev deterministically
/// unconfigured. Integration tests drive this directly through
/// `tower::ServiceExt`. Tests must not depend on the developer shell's
/// `OPENROUTER_API_KEY`, so this builder never reads the environment; the
/// binary's `main()` builds the env-driven state via
/// `app_with_jev(JevHandle::from_env())`.
pub fn app_with_fixture_data() -> Router {
    app_with_jev(JevHandle::unconfigured())
}

/// The router over an explicit Jev handle (the test seam: scripted
/// transports prove fallback and credential isolation without a network).
pub fn app_with_jev(jev: JevHandle) -> Router {
    let state = AppState {
        graph: std::sync::Arc::new(fixture_data::fixture_graph()),
        jev,
    };
    app(state)
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/fixture", get(fixture_summary))
        .route("/api/edges", get(all_edges))
        .route("/api/edges/{player}", get(player_edges))
        .route("/api/connection", get(connection))
        .route("/api/paths", get(all_paths))
        .route("/api/stats", get(stats))
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

async fn fixture_summary(State(_state): State<AppState>) -> Json<api_types::FixtureSummary> {
    Json(fixture_data::fixture_summary())
}

async fn all_edges(State(_state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "edges": fixture_data::fixture_edges() }))
}

/// The teammate edges of one player, by id.
async fn player_edges(State(state): State<AppState>, Path(player): Path<String>) -> Response {
    if !state.graph.contains_player(&player) {
        return player_not_found(&player, &player);
    }
    let edges: Vec<api_types::TeammateEdgeDto> = state
        .graph
        .edges()
        .into_iter()
        .filter(|edge| edge.a == player || edge.b == player)
        .map(|edge| {
            let overlap_days = edge.overlap_days();
            api_types::TeammateEdgeDto {
                a: edge.a,
                b: edge.b,
                overlap_days,
            }
        })
        .collect();
    Json(serde_json::json!({ "edges": edges })).into_response()
}

#[derive(Deserialize)]
struct ConnectQuery {
    from: Option<String>,
    to: Option<String>,
}

async fn connection(State(state): State<AppState>, Query(query): Query<ConnectQuery>) -> Response {
    connection_response(&state, query.from.as_deref(), query.to.as_deref())
}

/// All shortest chains between the queried pair (alternative connections).
async fn all_paths(State(state): State<AppState>, Query(query): Query<ConnectQuery>) -> Response {
    connection_response_all(&state, query.from.as_deref(), query.to.as_deref())
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
        return player_not_found(from, to);
    }
    let response = fixture_data::fixture_connection(from, to).expect("players exist");
    Json(response).into_response()
}

fn connection_response_all(state: &AppState, from: Option<&str>, to: Option<&str>) -> Response {
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
        return player_not_found(from, to);
    }
    let chains = state
        .graph
        .all_shortest_chains(from, to)
        .expect("players exist");
    let paths: Vec<api_types::PathDto> = chains
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
    Json(serde_json::json!({ "paths": paths })).into_response()
}

fn player_not_found(from: &str, to: &str) -> Response {
    let missing = if fixture_data::FIXTURE_PLAYERS.contains(&from) {
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
    ui::home(&state.graph, state.jev.status())
}

/// The `/chain` UI page: the shortest teammate chain between the queried pair
/// as ordered players and links, with its degree of separation.
#[derive(Deserialize)]
struct ChainQuery {
    from: Option<String>,
    to: Option<String>,
}

async fn chain_page(State(state): State<AppState>, Query(query): Query<ChainQuery>) -> Response {
    ui::chain(&state.graph, query.from, query.to, state.jev.status())
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

/// Serve the fixture-backed app on localhost. Exposed for the binary entry
/// point (`src/main.rs`). Jev reads its key from the process env at startup
/// (fail-soft: unconfigured runs keep every deterministic feature).
pub async fn main() {
    jev::install_capture_logger();
    jev::init_logging();
    let state = AppState {
        graph: std::sync::Arc::new(fixture_data::fixture_graph()),
        jev: JevHandle::from_env(),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("bind 127.0.0.1:3000");
    let router = app(state).into_make_service();
    println!("7-degrees listening on http://127.0.0.1:3000");
    axum::serve(listener, router).await.expect("server runs");
}
