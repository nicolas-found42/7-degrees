//! The 7-degrees local app server: Axum routes over the fixture-backed
//! teammate graph.

mod fixture_data;

use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::get,
    Router,
};
use serde::Deserialize;

use api_types::ErrorResponse;

/// App state shared by all routes: the built teammate graph.
#[derive(Clone)]
pub struct AppState {
    graph: std::sync::Arc<graph_core::TeammateGraph>,
}

/// The router with the fixture graph preloaded, as a real data import would
/// load it. Integration tests drive this directly through `tower::ServiceExt`.
pub fn app_with_fixture_data() -> Router {
    let state = AppState {
        graph: std::sync::Arc::new(fixture_data::fixture_graph()),
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
        .fallback(not_found)
        .with_state(state)
}

async fn not_found() -> Response {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "not-found".to_string(),
            message: "unknown route".to_string(),
        }),
    )
        .into_response()
}

async fn fixture_summary(State(_state): State<AppState>) -> Json<api_types::FixtureSummary> {
    Json(fixture_data::fixture_summary())
}

async fn all_edges(State(_state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "edges": fixture_data::fixture_edges() }))
}

/// The teammate edges of one player, by id.
async fn player_edges(
    State(state): State<AppState>,
    axum::extract::Path(player): axum::extract::Path<String>,
) -> Response {
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
    from: String,
    to: String,
}

async fn connection(
    State(state): State<AppState>,
    Query(query): Query<ConnectQuery>,
) -> Response {
    connection_response(&state, &query.from, &query.to)
}

async fn all_paths(
    State(state): State<AppState>,
    Query(query): Query<ConnectQuery>,
) -> Response {
    let (from, to) = (&query.from, &query.to);
    if !state.graph.contains_player(from) || !state.graph.contains_player(to) {
        return player_not_found(from, to);
    }
    let chains = state.graph.all_shortest_chains(from, to).expect("players exist");
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

fn connection_response(state: &AppState, from: &str, to: &str) -> Response {
    if !state.graph.contains_player(from) || !state.graph.contains_player(to) {
        return player_not_found(from, to);
    }
    let response = fixture_data::fixture_connection(from, to).expect("players exist");
    Json(response).into_response()
}

fn player_not_found(from: &str, to: &str) -> Response {
    let missing = if fixture_data::FIXTURE_PLAYERS.contains(&from) { to } else { from };
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

/// Serve the fixture-backed app on localhost. Exposed for the binary entry
/// point (`src/main.rs`).
pub async fn main() {
    let state = AppState {
        graph: std::sync::Arc::new(fixture_data::fixture_graph()),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .expect("bind 127.0.0.1:3000");
    let router = app(state).into_make_service();
    println!("7-degrees listening on http://127.0.0.1:3000");
    axum::serve(listener, router).await.expect("server runs");
}