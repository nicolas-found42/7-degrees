//! Cached, deterministic rankings over the same admitted graph as connection search.
use crate::AppState;
use axum::{Json, extract::State};
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct Discovery {
    longest: Vec<Pair>,
    hubs: Vec<Hub>,
}
#[derive(Clone, Serialize)]
struct Pair {
    from: String,
    to: String,
    from_name: String,
    to_name: String,
    degree: usize,
}
#[derive(Clone, Serialize)]
struct Hub {
    id: String,
    name: String,
    degree: usize,
}

pub async fn api(State(state): State<AppState>) -> Result<Json<Discovery>, axum::http::StatusCode> {
    // All-pairs BFS must never run on an async server worker. OnceLock caches
    // this immutable snapshot and serializes concurrent first requests.
    tokio::task::spawn_blocking(move || {
        state
            .discovery
            .get_or_init(|| {
                let nodes = crate::graph_view::discovery_nodes(&state);
                let adj = canvas_view::orbit::adjacency(&nodes, &state.visual_links);
                let longest = canvas_view::orbit::longest_pairs(&nodes, &adj)
                    .into_iter()
                    .map(|(a, b, degree)| Pair {
                        from: nodes[a].id.clone(),
                        to: nodes[b].id.clone(),
                        from_name: nodes[a].name.clone(),
                        to_name: nodes[b].name.clone(),
                        degree,
                    })
                    .collect();
                let mut hubs: Vec<_> = nodes
                    .iter()
                    .enumerate()
                    .map(|(i, n)| Hub {
                        id: n.id.clone(),
                        name: n.name.clone(),
                        degree: adj[i].len(),
                    })
                    .collect();
                hubs.sort_by(|a, b| b.degree.cmp(&a.degree).then_with(|| a.id.cmp(&b.id)));
                hubs.truncate(10);
                Discovery { longest, hubs }
            })
            .clone()
    })
    .await
    .map(Json)
    .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
}
