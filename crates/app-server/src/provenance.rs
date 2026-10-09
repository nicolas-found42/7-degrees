//! Evidence display uses admitted graph edges and loaded source references only.
use crate::{AppState, ui};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Response,
};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
pub struct EdgeQuery {
    pub from: String,
    pub to: String,
}
#[derive(Deserialize)]
pub struct SourceQuery {
    pub record: String,
}

pub(crate) fn fixture_overlaps(
    state: &AppState,
    edge: &graph_core::TeammateEdge,
    team: &str,
) -> Vec<Value> {
    let records = |id: &str| {
        state.graph.roster.tenures.iter().enumerate().filter(|(_,t)|t.player==id&&t.team==team).map(|(index,t)|json!({"record":format!("fixture:tenure:{}",index+1),"player":t.player,"team":t.team,"start_day":t.tenure.start.0,"end_day":t.tenure.end.0,"evidence_class":"synthetic-fixture","membership_source":"crates/fixture/src/lib.rs"})).collect::<Vec<_>>()
    };
    let mut overlaps = Vec::new();
    for a in records(&edge.a) {
        for b in records(&edge.b) {
            let start = a["start_day"]
                .as_u64()
                .unwrap()
                .max(b["start_day"].as_u64().unwrap());
            let end = a["end_day"]
                .as_u64()
                .unwrap()
                .min(b["end_day"].as_u64().unwrap());
            if start < end {
                overlaps.push(json!({"start_day":start,"end_day":end,"records":[a,b]}));
            }
        }
    }
    overlaps
}
fn reference(record: &str) -> String {
    format!(
        "<a href=\"/sources/tenure?record={}\" target=\"_blank\" rel=\"noopener\">{}</a>",
        ui::url_encode(record),
        ui::escape(record)
    )
}
fn day(value: &Value) -> String {
    value
        .as_u64()
        .and_then(|day| {
            chrono::NaiveDate::from_ymd_opt(1946, 1, 1)?.checked_add_days(chrono::Days::new(day))
        })
        .map(|date| date.to_string())
        .unwrap_or_else(|| format!("unavailable calendar date (day {value})"))
}
fn interval(start: &Value, end: &Value, synthetic: bool) -> String {
    if synthetic {
        format!("[day {start}, day {end})")
    } else {
        format!("[{}, {})", day(start), day(end))
    }
}
fn season(value: &Value) -> String {
    value
        .as_u64()
        .map(|end| format!("{}–{:02} (ending {end})", end.saturating_sub(1), end % 100))
        .unwrap_or_else(|| "season unavailable".into())
}
fn anchors(value: &Value) -> String {
    value
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.split(',')
                .map(|part| {
                    part.parse::<u64>()
                        .map(|v| day(&json!(v)))
                        .unwrap_or_else(|_| format!("unresolved value: {}", ui::escape(part)))
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "source event date unavailable".into())
}
fn record_markup(record: &Value, state: &AppState, synthetic: bool) -> String {
    let name = ui::escape(ui::display_name(
        &state.graph.roster.players,
        record["player"].as_str().unwrap_or(""),
    ));
    let identity = reference(record["record"].as_str().unwrap_or(""));
    if synthetic {
        return format!(
            "<li>Source record {identity}: {name} — {}; evidence class: synthetic-fixture. Source: crates/fixture/src/lib.rs. Interval boundaries are defined by this example, not historical transaction anchors.</li>",
            interval(&record["start_day"], &record["end_day"], true)
        );
    }
    let boundary = |key: &str| {
        if record[key].as_u64() == Some(1) {
            "transaction anchored"
        } else {
            "unresolved; not an admitted boundary"
        }
    };
    format!(
        "<li>Source record {identity}: {name} — canonical franchise {}; Season {}; roster tenure {} (end exclusive).<dl><dt>Evidence class</dt><dd>{}</dd><dt>Membership source</dt><dd>{}</dd><dt>Arrival boundary: {}</dt><dd>Arrival events: {}</dd><dt>Departure boundary: {}</dt><dd>Departure events: {}</dd><dt>Source notes</dt><dd>{}</dd><dt>Source/version manifest</dt><dd>{} — <a href=\"/sources/manifest\" target=\"_blank\" rel=\"noopener\">Open source/version manifest</a>; <a href=\"/sources/transactions\" target=\"_blank\" rel=\"noopener\">Pinned transaction page inventory</a></dd></dl></li>",
        ui::escape(record["team"].as_str().unwrap_or("")),
        season(&record["season"]),
        interval(&record["start_day"], &record["end_day"], false),
        ui::escape(record["evidence_class"].as_str().unwrap_or("")),
        ui::escape(record["membership_source"].as_str().unwrap_or("")),
        boundary("start_anchored"),
        anchors(&record["arrival_days"]),
        boundary("end_anchored"),
        anchors(&record["departure_days"]),
        ui::escape(
            record["notes"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("None reported")
        ),
        ui::escape(
            record["source_manifest"]
                .as_str()
                .unwrap_or("Manifest unavailable")
        )
    )
}
fn gap_markup(gap: &crate::report_data::CoverageGap, state: &AppState) -> String {
    let interval = match (gap.start_day, gap.end_day) {
        (Some(a), Some(b)) => interval(&json!(a), &json!(b), false),
        _ => "dated interval unavailable".into(),
    };
    format!(
        "<li>Excluded from teammate evidence — source record {}: {}; canonical franchise {}; Season {}; {}; evidence class: {}; reason: {}</li>",
        reference(&gap.record),
        ui::escape(ui::display_name(&state.graph.roster.players, &gap.player)),
        ui::escape(&gap.team),
        season(&json!(gap.season)),
        interval,
        ui::escape(&gap.evidence_class),
        ui::escape(if gap.reasons.is_empty() {
            "No certified pair of dated transaction boundaries"
        } else {
            &gap.reasons
        })
    )
}
fn coverage(state: &AppState, from: &str, to: &str) -> String {
    let Some(reports) = &state.reports else {
        return "<p>Synthetic fixture: evidence is complete within this worked example.</p>".into();
    };
    let mut body = format!(
        "<section class=\"coverage\"><h2>Source evidence is incomplete</h2><p>Missing evidence does not prove a historical non-relationship.</p><p>{}</p><h3>Coverage gaps for these players</h3><ul>",
        ui::escape(&reports.coverage.warning)
    );
    let mut count = 0;
    for id in [from, to] {
        if from == to && count > 0 {
            break;
        }
        for gap in reports.gaps.get(id).into_iter().flatten().take(100) {
            body.push_str(&gap_markup(gap, state));
            count += 1;
        }
        if reports.gaps.get(id).is_some_and(|g| g.len() > 100) {
            body.push_str("<li>Showing the first 100 source coverage gaps for this player.</li>");
        }
    }
    if count == 0 {
        body.push_str("<li>No additional excluded tenure rows are listed for these endpoints. The snapshot still does not establish complete historical coverage.</li>");
    }
    body.push_str("</ul></section>");
    body
}
pub async fn page(State(state): State<AppState>, Query(query): Query<EdgeQuery>) -> Response {
    if !state.graph.contains_player(&query.from) || !state.graph.contains_player(&query.to) {
        return ui::document(
            StatusCode::NOT_FOUND,
            "Overlap evidence",
            "<h1>Overlap evidence</h1><p class=\"error\">Unknown player endpoint.</p>",
        );
    }
    let edge =
        state.graph.edges().into_iter().find(|e| {
            (e.a == query.from && e.b == query.to) || (e.b == query.from && e.a == query.to)
        });
    let Some(edge) = edge else {
        return ui::document(
            StatusCode::OK,
            "Overlap evidence",
            &format!(
                "<h1>Overlap evidence</h1><p>No admitted direct teammate edge for {} and {}.</p>{}",
                ui::escape(ui::display_name(&state.graph.roster.players, &query.from)),
                ui::escape(ui::display_name(&state.graph.roster.players, &query.to)),
                coverage(&state, &query.from, &query.to)
            ),
        );
    };
    let data = crate::edge_json(&state, edge);
    let synthetic = state.reports.is_none();
    let mut evidence = String::new();
    for item in data["evidence"].as_array().unwrap() {
        evidence.push_str(&format!(
            "<section><h2>Canonical franchise: {}</h2>",
            ui::escape(item["team"].as_str().unwrap())
        ));
        for overlap in item["overlaps"].as_array().unwrap() {
            evidence.push_str(&format!(
                "<p>Overlap: {} — {} day(s); end exclusive.</p><ul>",
                interval(&overlap["start_day"], &overlap["end_day"], synthetic),
                overlap["end_day"].as_u64().unwrap() - overlap["start_day"].as_u64().unwrap()
            ));
            for record in overlap["records"].as_array().unwrap() {
                evidence.push_str(&record_markup(record, &state, synthetic));
            }
            evidence.push_str("</ul>");
        }
        evidence.push_str("</section>");
    }
    evidence.push_str(&coverage(&state, &query.from, &query.to));
    ui::document(
        StatusCode::OK,
        "Overlap evidence",
        &format!(
            "<h1>Teammate overlap evidence</h1><p>{} ↔ {}</p>{}{evidence}",
            ui::escape(ui::display_name(&state.graph.roster.players, &query.from)),
            ui::escape(ui::display_name(&state.graph.roster.players, &query.to)),
            if synthetic {
                "<p>Synthetic fixture: these are worked example intervals, not historical records.</p>"
            } else {
                "<p>Positive dated tenure overlap admitted by the deterministic graph.</p>"
            }
        ),
    )
}
pub async fn source_record(
    State(state): State<AppState>,
    Query(query): Query<SourceQuery>,
) -> Response {
    let found = if let Some(reports) = &state.reports {
        reports
            .records
            .values()
            .flatten()
            .find(|r| r.record == query.record)
            .map(|r| serde_json::to_value(r).unwrap())
    } else {
        state.graph.roster.tenures.iter().enumerate().find(|(index,_)|format!("fixture:tenure:{}",index+1)==query.record).map(|(index,t)|json!({"record":format!("fixture:tenure:{}",index+1),"player":t.player,"team":t.team,"start_day":t.tenure.start.0,"end_day":t.tenure.end.0}))
    };
    let Some(record) = found else {
        if let Some(gap) = state
            .reports
            .as_ref()
            .and_then(|r| r.gaps.values().flatten().find(|g| g.record == query.record))
        {
            return ui::document(
                StatusCode::OK,
                "Excluded source record",
                &format!(
                    "<h1>Source record {}</h1><p>Source evidence is incomplete; this row was excluded from teammate edge construction.</p><ul>{}</ul>",
                    ui::escape(&query.record),
                    gap_markup(gap, &state)
                ),
            );
        }
        return ui::document(
            StatusCode::NOT_FOUND,
            "Source record",
            "<h1>Source record unavailable</h1>",
        );
    };
    ui::document(
        StatusCode::OK,
        "Source record",
        &format!(
            "<h1>Source record {}</h1><ul>{}</ul><pre>{}</pre>",
            ui::escape(&query.record),
            record_markup(&record, &state, state.reports.is_none()),
            ui::escape(&serde_json::to_string_pretty(&record).unwrap())
        ),
    )
}
pub async fn manifest() -> Response {
    ui::document(
        StatusCode::OK,
        "Source/version manifest",
        &format!(
            "<h1>Source/version manifest</h1><pre>{}</pre>",
            ui::escape(include_str!("../../../docs/data/source-manifest.md"))
        ),
    )
}

pub async fn transactions() -> Response {
    ui::document(
        StatusCode::OK,
        "Pinned transaction sources",
        &format!(
            "<h1>Pinned transaction page inventory</h1><p>Source URLs, snapshot SHA-256 and parsed date windows retained by T4.</p><pre>{}</pre>",
            ui::escape(include_str!(
                "../../../docs/reports/t4/transaction-source-pages.csv"
            ))
        ),
    )
}
