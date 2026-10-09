//! Deterministic statistics page over the same immutable graph as `/api/stats`.
use axum::response::Response;
use graph_core::TeammateGraph;

pub(crate) fn page(
    graph: &TeammateGraph,
    semantic_status: (bool, &'static str),
    coverage: Option<&str>,
) -> Response {
    let stats = graph.statistics();
    let reachable: usize = stats.histogram.values().sum();
    let empty = if stats.histogram.is_empty() {
        "<p>No reachable distinct player pairs in this snapshot.</p>"
    } else {
        ""
    };
    let rows: String = stats
        .histogram
        .iter()
        .map(|(degree, pairs)| format!("<tr><th scope=\"row\">{degree}</th><td>{pairs}</td></tr>"))
        .collect();
    let status = crate::ui::semantic_status_line(semantic_status);
    let scope = if coverage.is_some() {
        "<p>These statistics describe the evidenced graph, not complete historical connectivity.</p>"
    } else {
        ""
    };
    let coverage = crate::ui::coverage_line(coverage);
    let snapshot = if coverage.is_empty() {
        "Synthetic fixture"
    } else {
        "NBA/BAA canonical report snapshot"
    };
    let body = format!(
        "<h1>Network statistics</h1>{status}<p>{snapshot}</p>{coverage}{scope}<section class=\"stats\"><h2>Graph summary</h2><dl>\
         <dt>Player nodes</dt><dd>{}</dd>\
         <dt>Connected components</dt><dd>{}</dd>\
         <dt>Maximum finite diameter</dt><dd>{}</dd>\
         <dt>Reachable unordered pairs</dt><dd>{reachable}</dd>\
         <dt>Unreachable unordered pairs</dt><dd>{}</dd></dl></section>\
         <section class=\"stats\"><h2>Separation histogram</h2><table>\
         <caption>Reachable unordered player pairs by degree of separation</caption>\
         <thead><tr><th scope=\"col\">Degree</th><th scope=\"col\">Pairs</th></tr></thead>\
         <tbody>{rows}</tbody></table>{empty}</section>\
         <section class=\"stats\"><h2>How to read these statistics</h2>\
         <p>These counts are computed by ordinary deterministic Rust over the current immutable graph.</p>\
         <p>Self-pairs are excluded. Unordered pairs are counted once: A–B and B–A are the same pair. \
         Histogram bins count reachable pairs by their shortest degree, regardless of how many shortest chains exist.</p>\
         <p>Every isolated player is a component with diameter 0 and contributes no reachable distinct pairs. \
         Pairs of players in different components are unreachable. Unreachable pairs are excluded from the histogram \
         and counted separately; reachable and unreachable counts together cover all distinct unordered player pairs.</p>\
         <p>The displayed diameter is the maximum finite shortest-path length across all components, \
         rather than treating unreachable pairs as infinitely distant. If there are no reachable distinct pairs, the diameter is 0. \
         An empty graph has 0 components.</p>\
         <p><a href=\"/api/stats\">Statistics as JSON</a> · <a href=\"/api/coverage\">Source coverage details</a></p></section>",
        stats.players, stats.components, stats.diameter, stats.unreachable_pairs
    );
    crate::ui::document(
        axum::http::StatusCode::OK,
        "7 Degrees — Network statistics",
        &body,
    )
}
