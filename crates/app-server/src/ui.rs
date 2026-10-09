//! The minimal UI: server-rendered HTML authored entirely in Rust.
//!
//! The spec's Rust constraint rules out JavaScript UI/graph libraries; the
//! plain Axum-served HTML option is fully compliant and needs no WASM tooling
//! (trunk is not installed here), so this module renders the UI on the server
//! from `graph_core` data. No JavaScript ships.

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use graph_core::{Chain, Connection, Player, TeammateGraph};

/// The stylesheet served at `/style.css`.
pub const STYLE_CSS: &str = include_str!("style.css");

/// Escape text for safe inclusion in HTML content and attribute values.
pub(crate) fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Percent-encode a query-string value (RFC 3986 unreserved characters pass
/// through; everything else is escaped).
pub(crate) fn url_encode(value: &str) -> String {
    value
        .bytes()
        .map(|c| {
            let c = c as char;
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect()
}

/// The display name for a player id, falling back to the raw id.
fn display_name<'a>(players: &'a [Player], id: &'a str) -> &'a str {
    players
        .iter()
        .find(|player| player.id == id)
        .map(|player| player.name.as_str())
        .unwrap_or(id)
}

/// The document shell: status, escaped title, stylesheet link, and body.
pub(crate) fn document(status: StatusCode, title: &str, body: &str) -> Response {
    let html = format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head><meta charset=\"utf-8\"><meta \
         name=\"viewport\" content=\"width=device-width, initial-scale=1\"><title>{}</title><link \
         rel=\"stylesheet\" href=\"/style.css\"></head>\n<body><main \
         class=\"seven-degrees\"><nav aria-label=\"Main navigation\"><a href=\"/\">Explorer</a> · <a href=\"/stats\">Network statistics</a></nav>{}</main></body>\n</html>\n",
        escape(title),
        body
    );
    (
        status,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response()
}

/// One `<select>` for the connect form, preselecting `selected`.
fn player_select(players: &[Player], field: &str, label: &str, selected: &str) -> String {
    let options: String = players
        .iter()
        .map(|player| {
            let is_selected = if player.id == selected {
                " selected"
            } else {
                ""
            };
            format!(
                "<option value=\"{}\"{}>{}</option>",
                escape(&player.id),
                is_selected,
                escape(&player.name)
            )
        })
        .collect();
    format!(
        "<label for=\"{field}\">{label}</label><select id=\"{field}\" \
         name=\"{field}\">{options}</select>",
        field = field,
        label = label,
        options = options
    )
}

/// The GET form that drives `/chain`, defaulting to the A→C demo pair.
fn connect_form(players: &[Player], from: Option<&str>, to: Option<&str>) -> String {
    format!(
        "<form method=\"get\" action=\"/chain\" \
         class=\"controls\">{}{}<button type=\"submit\">Connect</button></form>",
        player_select(
            players,
            "from",
            "Player from",
            from.unwrap_or_else(|| if players.iter().any(|p| p.id == "A") {
                "A"
            } else {
                players.first().map(|p| p.id.as_str()).unwrap_or("")
            })
        ),
        player_select(
            players,
            "to",
            "Player to",
            to.unwrap_or_else(|| if players.iter().any(|p| p.id == "C") {
                "C"
            } else {
                players.get(1).map(|p| p.id.as_str()).unwrap_or("")
            })
        ),
    )
}

/// The shortest-chain section: ordered players and links with evidence.
fn chain_section(chain: &Chain, players: &[Player]) -> String {
    let items: String = chain
        .path
        .iter()
        .map(|id| format!("<li>{}</li>", escape(display_name(players, id))))
        .collect();
    let links: String = chain
        .links
        .iter()
        .map(|link| {
            format!(
                "<li>{} → {} — teammates on {}, overlapping roster tenure: {} day(s)</li>",
                escape(display_name(players, &link.from)),
                escape(display_name(players, &link.to)),
                escape(&link.team),
                link.overlap_days
            )
        })
        .collect();
    format!(
        "<section class=\"chain\"><h2>Shortest teammate \
         chain</h2><p class=\"degree\">Degree of separation: <strong>{}</strong></p><ol \
         class=\"path\">{}</ol><ul class=\"links\">{}</ul></section>",
        chain.links.len(),
        items,
        links
    )
}

/// The semantic-features availability line, rendered at the top of every
/// page (spec: "the UI reports semantic features as unavailable" when Jev is
/// unconfigured or unreachable). `data-semantic-status` keeps it
/// machine-readable for tests and E2E seams.
///
/// The rendered text carries only the two availability words — never a
/// provider name, env var, or any credential material.
pub(crate) fn semantic_status_line(status: (bool, &'static str)) -> String {
    let (available, reason) = status;
    let text = if available {
        "Semantic features (Jev): available"
    } else {
        "Semantic features (Jev): unavailable"
    };
    format!(
        "<p class=\"semantic-status\" data-semantic-status=\"{reason}\">{}</p>",
        escape(text)
    )
}

pub(crate) fn coverage_line(coverage: Option<&str>) -> String {
    coverage
        .map(|text| format!("<p class=\"coverage-warning\">{}</p>", escape(text)))
        .unwrap_or_default()
}

/// The home page: the connect form plus the whole fixture edge list.
pub fn home(
    graph: &TeammateGraph,
    semantic_status: (bool, &'static str),
    coverage: Option<&str>,
) -> Response {
    let players = &graph.roster.players;
    let edges = graph.edges();
    let edge_items: String = edges
        .iter()
        .take(100)
        .map(|edge| {
            let evidence: String = edge
                .evidence
                .iter()
                .map(|e| format!("{} ({} day(s))", escape(&e.team), e.overlap_days))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "<li><a href=\"/chain?from={}&amp;to={}\">{} — {}</a>: teammate edge on \
                 {}</li>",
                url_encode(&edge.a),
                url_encode(&edge.b),
                escape(display_name(players, &edge.a)),
                escape(display_name(players, &edge.b)),
                evidence
            )
        })
        .collect();
    let source = if coverage.is_some() {
        "NBA/BAA canonical report snapshot"
    } else {
        "fixture demo (Players A–D, Teams Red/Blue)"
    };
    let demo_from = if coverage.is_some() {
        edges.first().map(|e| e.a.as_str()).unwrap_or("")
    } else {
        "A"
    };
    let demo_to = if coverage.is_some() {
        edges.first().map(|e| e.b.as_str()).unwrap_or("")
    } else {
        "C"
    };
    let body = format!(
        "<h1>7 Degrees</h1>{}<p class=\"subtitle\">NBA teammate degrees of separation — {source}</p>{}<h2>Teammate edges</h2><p>The graph \
         has {} teammate edges (showing at most 100):</p><ul class=\"edges\">{}</ul><h2>JSON \
         API</h2><p><a href=\"/api/fixture\">/api/fixture</a> · <a \
         href=\"/api/connection?from={}&amp;to={}\">Example connection</a> · <a \
         href=\"/api/semantic-status\">/api/semantic-status</a></p>",
        format_args!(
            "{}{}",
            semantic_status_line(semantic_status),
            coverage_line(coverage)
        ),
        format_args!(
            "{}{}",
            crate::search::form(""),
            connect_form(players, Some(demo_from), Some(demo_to))
        ),
        edges.len(),
        edge_items,
        url_encode(demo_from),
        url_encode(demo_to)
    );
    document(StatusCode::OK, "7 Degrees — Teammate Explorer", &body)
}

/// The `/chain` page: the connect form plus the shortest teammate chain
/// between the queried pair, an explanation when none exists, or an error
/// when a player id is unknown. Every variant carries the semantic-feature
/// status line.
pub fn chain(
    graph: &TeammateGraph,
    from: Option<String>,
    to: Option<String>,
    semantic_status: (bool, &'static str),
    coverage: Option<&str>,
) -> Response {
    let players = &graph.roster.players;
    let status_line = format!(
        "{}{}",
        semantic_status_line(semantic_status),
        coverage_line(coverage)
    );
    let back = "<p><a href=\"/\">← Back to the explorer</a></p>";
    let (status, body) = match (from.as_deref(), to.as_deref()) {
        (None, None) => (
            StatusCode::OK,
            format!(
                "<h1>7 Degrees</h1>{}{}<p class=\"hint\">Pick two players and connect \
                 them.</p>",
                status_line,
                connect_form(players, None, None)
            ),
        ),
        (from, to) => {
            let from_id = from.unwrap_or_else(|| {
                if players.iter().any(|p| p.id == "A") {
                    "A"
                } else {
                    players.first().map(|p| p.id.as_str()).unwrap_or("")
                }
            });
            let to_id = to.unwrap_or_else(|| {
                if players.iter().any(|p| p.id == "C") {
                    "C"
                } else {
                    players.get(1).map(|p| p.id.as_str()).unwrap_or("")
                }
            });
            if !graph.contains_player(from_id) || !graph.contains_player(to_id) {
                let missing = if graph.contains_player(from_id) {
                    to_id
                } else {
                    from_id
                };
                (
                    StatusCode::NOT_FOUND,
                    format!(
                        "<h1>7 Degrees</h1>{}{}{}<p class=\"error\">No player node for id \
                         {:?}.</p>",
                        status_line,
                        connect_form(players, Some(from_id), Some(to_id)),
                        back,
                        escape(missing)
                    ),
                )
            } else {
                match graph.shortest_chain(from_id, to_id) {
                    Some(Connection::Connected(chain)) => (
                        StatusCode::OK,
                        format!(
                            "<h1>7 Degrees</h1>{}{}{}",
                            status_line,
                            back,
                            chain_section(&chain, players)
                        ),
                    ),
                    Some(Connection::Disconnected) | None => (
                        StatusCode::OK,
                        format!(
                            "<h1>7 Degrees</h1>{}{}<section class=\"chain\"><h2>Shortest \
                             teammate chain</h2><p class=\"hint\">No teammate chain connects {} \
                             and {}.</p></section>",
                            status_line,
                            back,
                            escape(display_name(players, from_id)),
                            escape(display_name(players, to_id))
                        ),
                    ),
                }
            }
        }
    };
    document(status, "7 Degrees — Teammate Explorer", &body)
}

/// The 404 page for unknown non-API routes.
pub fn not_found_page() -> Response {
    document(
        StatusCode::NOT_FOUND,
        "7 Degrees — Page not found",
        "<h1>7 Degrees</h1><p class=\"error\">This page does not exist. <a href=\"/\">Back to \
         the fixture</a>.</p>",
    )
}
