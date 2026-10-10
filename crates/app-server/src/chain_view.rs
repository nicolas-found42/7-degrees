//! Accessible SSR selection of deterministic shortest teammate chains.
use crate::ui;
use axum::{http::StatusCode, response::Response};
use graph_core::{ChainWithDegree, TeammateGraph};
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize)]
pub struct ChainQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub selected: Option<usize>,
    pub cursor: Option<String>,
    pub offset: Option<u64>,
    pub limit: Option<usize>,
}

/// The selected graph facts for later canvas and provenance views. The same
/// payload drives accessible text and the escaped HTML template below it.
#[derive(Clone, Debug, Serialize)]
pub struct SelectedChain {
    pub path: Vec<api_types::PlayerDto>,
    pub links: Vec<api_types::LinkDto>,
    pub degree: usize,
}
impl SelectedChain {
    pub fn from_chain(graph: &TeammateGraph, chain: &ChainWithDegree) -> Self {
        Self {
            path: chain
                .path
                .iter()
                .map(|id| api_types::PlayerDto {
                    id: id.clone(),
                    name: ui::display_name(&graph.roster.players, id).into(),
                })
                .collect(),
            links: chain
                .links
                .iter()
                .map(|link| api_types::LinkDto {
                    from: link.from.clone(),
                    to: link.to.clone(),
                    team: link.team.clone(),
                    overlap_days: link.overlap_days,
                    minimum_shared_games: link.minimum_shared_games,
                })
                .collect(),
            degree: chain.degree,
        }
    }
}

/// Render one selected shortest chain, its links and its server-owned payload.
pub fn render_selected_chain(chain: &SelectedChain) -> String {
    let items: String = chain
        .path
        .iter()
        .map(|p| {
            format!(
                "<li data-player-id=\"{}\">{}</li>",
                ui::escape(&p.id),
                ui::escape(&p.name)
            )
        })
        .collect();
    let name = |id: &str| {
        chain
            .path
            .iter()
            .find(|p| p.id == id)
            .map(|p| p.name.as_str())
            .unwrap_or("")
    };
    let links: String = chain
        .links
        .iter()
        .map(|link| {
            format!(
                "<li>{} → {} — teammates on {}, {} <a href=\"/edge?from={}&amp;to={}\" target=\"_blank\" rel=\"noopener\">Open overlap evidence</a></li>",
                ui::escape(name(&link.from)),
                ui::escape(name(&link.to)),
                ui::escape(&link.team),
                link.overlap_days.map(|days| format!("overlapping roster tenure: {days} day(s)")).unwrap_or_else(|| api_types::overlap_description(None, link.minimum_shared_games)),
                ui::url_encode(&link.from),
                ui::url_encode(&link.to),
            )
        })
        .collect();
    let ids: Vec<_> = chain.path.iter().map(|p| p.id.as_str()).collect();
    let path = serde_json::to_string(&ids).expect("player ids serialize");
    let payload = serde_json::to_string(chain).expect("selected chain serializes");
    let visualizer = crate::chain_map::render(chain);
    let relationship = match chain.degree {
        0 => "Same player; no teammate links are needed.",
        1 => "Direct teammates.",
        _ => "Indirect teammate connection.",
    };
    format!(
        "<section class=\"chain\" id=\"selected-chain\" data-degree=\"{}\" data-path=\"{}\"><h2>Shortest teammate chain</h2><p class=\"degree\">Degree of separation: <strong>{}</strong></p><p>{relationship}</p><p>Teammate links in this chain: <strong>{}</strong></p>{visualizer}<ol class=\"path\">{items}</ol><ul class=\"links\">{links}</ul><template id=\"selected-chain-payload\" data-payload=\"{}\"></template></section>",
        chain.degree,
        ui::escape(&path),
        chain.degree,
        chain.links.len(),
        ui::escape(&payload)
    )
}

pub fn page(
    graph: &TeammateGraph,
    query: ChainQuery,
    semantic_status: (bool, &'static str),
    coverage: Option<&str>,
) -> Response {
    let shell = format!(
        "<h1>7 Degrees</h1>{}{}<p><a href=\"/\">← Back to the explorer</a></p>",
        ui::semantic_status_line(semantic_status),
        ui::coverage_line(coverage)
    );
    let error = |status, message: String| {
        ui::document(
            status,
            "7 Degrees — Teammate Explorer",
            &format!("{shell}<p class=\"error\">{message}</p>"),
        )
    };
    for id in [query.from.as_deref(), query.to.as_deref()]
        .into_iter()
        .flatten()
    {
        if id.is_empty() {
            return error(
                StatusCode::BAD_REQUEST,
                "Choose a player for each endpoint.".into(),
            );
        }
        if !graph.contains_player(id) {
            return error(
                StatusCode::NOT_FOUND,
                format!("No player node for id \"{}\".", ui::escape(id)),
            );
        }
    }
    let limit = query.limit.unwrap_or(20);
    if !(1..=100).contains(&limit) {
        return error(
            StatusCode::BAD_REQUEST,
            "Choose a page size between 1 and 100.".into(),
        );
    }
    if query.cursor.is_some() && query.offset.is_some() {
        return error(
            StatusCode::BAD_REQUEST,
            "Use either cursor or offset.".into(),
        );
    }
    let (Some(from), Some(to)) = (query.from.as_deref(), query.to.as_deref()) else {
        if query.cursor.is_some() || query.offset.is_some() || query.selected.is_some() {
            return error(
                StatusCode::BAD_REQUEST,
                "Choose both players before paging or selecting a chain.".into(),
            );
        }
        return ui::document(
            StatusCode::OK,
            "7 Degrees — Teammate Explorer",
            &format!(
                "{shell}{}<p class=\"hint\">Pick two players and connect them.</p>",
                ui::connect_form(
                    &graph.roster.players,
                    query.from.as_deref(),
                    query.to.as_deref()
                )
            ),
        );
    };
    let page = match query.cursor.as_deref() {
        Some(cursor) => match graph.shortest_chains_page_cursor(from, to, cursor, limit) {
            Ok(page) => page.expect("players checked"),
            Err(message) => return error(StatusCode::BAD_REQUEST, ui::escape(&message)),
        },
        None => graph
            .shortest_chains_page(from, to, query.offset.unwrap_or(0), limit)
            .expect("players checked"),
    };
    let current_cursor = query
        .cursor
        .clone()
        .unwrap_or_else(|| format!("v1:{}", query.offset.unwrap_or(0)));
    let link = |cursor: &str, selected: usize| {
        format!(
            "/chain?from={}&amp;to={}&amp;cursor={}&amp;limit={limit}&amp;selected={selected}",
            ui::url_encode(from),
            ui::url_encode(to),
            ui::url_encode(cursor)
        )
    };
    if page.total_exact == "0" {
        let message = if coverage.is_some() {
            format!(
                "<p data-connection-certainty=\"unresolved_coverage\">No evidenced teammate chain connects {} and {}. Missing dated roster evidence may hide a historical connection.</p>",
                ui::escape(ui::display_name(&graph.roster.players, from)),
                ui::escape(ui::display_name(&graph.roster.players, to))
            )
        } else {
            format!(
                "<p>No teammate chain connects {} and {}.</p><p data-connection-certainty=\"verified_disconnected\">Verified disconnected within this synthetic fixture.</p>",
                ui::escape(ui::display_name(&graph.roster.players, from)),
                ui::escape(ui::display_name(&graph.roster.players, to))
            )
        };
        return ui::document(
            StatusCode::OK,
            "7 Degrees — Teammate Explorer",
            &format!(
                "{shell}<section class=\"chain\" data-result=\"disconnected\"><h2>Shortest teammate chain</h2>{message}</section>"
            ),
        );
    }
    if page.chains.is_empty() {
        return error(
            StatusCode::BAD_REQUEST,
            format!(
                "The requested page has no alternatives. <a href=\"{}\">Start with the first alternatives</a>.",
                link("v1:0", 0)
            ),
        );
    }
    let selected = query.selected.unwrap_or(0);
    let Some(chain) = page.chains.get(selected) else {
        return error(
            StatusCode::BAD_REQUEST,
            "The selected alternative is outside this page.".into(),
        );
    };
    let alternatives: String = page
        .chains
        .iter()
        .enumerate()
        .map(|(index, chain)| {
            let names = chain
                .path
                .iter()
                .map(|id| ui::display_name(&graph.roster.players, id))
                .collect::<Vec<_>>()
                .join(" → ");
            format!(
                "<li data-selected=\"{}\"><a href=\"{}\"{}>Select alternative {}</a>: {}</li>",
                index == selected,
                link(&current_cursor, index),
                if index == selected {
                    " aria-current=\"true\""
                } else {
                    ""
                },
                index + 1,
                ui::escape(&names)
            )
        })
        .collect();
    let next = page.next_cursor.as_deref().map(|cursor| format!("<a id=\"next-shortest-page\" href=\"{}\" rel=\"next\">Next shortest alternatives</a>", link(cursor, 0))).unwrap_or_default();
    let selected = SelectedChain::from_chain(graph, chain);
    let selected_number = query.selected.unwrap_or(0) + 1;
    let rank_link = format!(
        "<p><a href=\"/rank?from={}&amp;to={}&amp;cursor={}&amp;limit={}\">Rank up to 20 alternatives from this page by interest</a></p>",
        ui::url_encode(from),
        ui::url_encode(to),
        ui::url_encode(&current_cursor),
        limit.min(20)
    );
    let graph_link =
        link(&current_cursor, query.selected.unwrap_or(0)).replacen("/chain?", "/graph?", 1);
    let network_link = format!("{graph_link}&amp;all=true");
    ui::document(
        StatusCode::OK,
        "7 Degrees — Teammate Explorer",
        &format!(
            "{shell}{rank_link}<p><a href=\"{}\">Explore selected chain in the graph</a></p><p><a href=\"{}\">See all players and connections in one field</a></p><p>Selected alternative {selected_number} on this page.</p><div data-selected-index=\"{}\" data-cursor=\"{}\">{}</div><section class=\"alternatives\" data-total-shortest=\"{}\"><h2>Equally short alternatives</h2><p>{} shortest chain(s); each has degree {}.</p><p>Showing {} alternative(s) on this page.</p><ol>{alternatives}</ol>{next}<p><a href=\"{}\">First shortest alternatives</a></p></section>",
            graph_link,
            network_link,
            query.selected.unwrap_or(0),
            ui::escape(&current_cursor),
            render_selected_chain(&selected),
            ui::escape(&page.total_exact),
            ui::escape(&page.total_exact),
            selected.degree,
            page.chains.len(),
            link("v1:0", 0)
        ),
    )
}
