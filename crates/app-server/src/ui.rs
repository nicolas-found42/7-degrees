//! The minimal UI: server-rendered HTML authored entirely in Rust.
//!
//! Accessible HTML is rendered on the server from deterministic graph data.
//! The optional graph canvas is authored in Rust and compiled to WASM;
//! generated wasm-bindgen glue loads it without a JavaScript UI/graph library.

use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};

use graph_core::Player;

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
pub(crate) fn display_name<'a>(players: &'a [Player], id: &'a str) -> &'a str {
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
         rel=\"stylesheet\" href=\"/style.css\"><link rel=\"icon\" href=\"data:,\"></head>\n<body><main \
         class=\"seven-degrees\">{}</main></body>\n</html>\n",
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

/// The 404 page for unknown non-API routes.
pub fn not_found_page() -> Response {
    document(
        StatusCode::NOT_FOUND,
        "7 Degrees — Page not found",
        "<h1>7 Degrees</h1><p class=\"error\">This page does not exist. <a href=\"/\">Back to \
         the graph</a>.</p>",
    )
}
