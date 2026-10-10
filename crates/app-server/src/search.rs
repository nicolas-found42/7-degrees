//! Bounded, deterministic player retrieval. Graph identity remains canonical ID.
use crate::{AppState, report_data::ReportMetadata};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use graph_core::TeammateGraph;
use serde::{Deserialize, Serialize};
use unicode_normalization::{UnicodeNormalization, char::is_combining_mark};

fn normalize(text: &str) -> String {
    text.nfkd()
        .filter(|c| !is_combining_mark(*c))
        .flat_map(char::to_lowercase)
        .filter_map(|c| {
            if c.is_alphanumeric() {
                Some(c)
            } else if matches!(c, '\'' | '’' | '.') {
                None
            } else {
                Some(' ')
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Restricted Damerau distance: insert/delete/substitute and adjacent swaps.
fn typo_distance(a: &str, b: &str, max: usize) -> Option<usize> {
    let a: Vec<_> = a.chars().collect();
    let b: Vec<_> = b.chars().collect();
    if a.len().abs_diff(b.len()) > max {
        return None;
    }
    let mut previous_previous = Vec::new();
    let mut previous: Vec<_> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut current = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            current[j] = (previous[j] + 1)
                .min(current[j - 1] + 1)
                .min(previous[j - 1] + usize::from(a[i - 1] != b[j - 1]));
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                current[j] = current[j].min(previous_previous[j - 2] + 1);
            }
        }
        previous_previous = previous;
        previous = current;
    }
    (previous[b.len()] <= max).then_some(previous[b.len()])
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchPlayer {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub first_season: Option<u32>,
    pub last_season: Option<u32>,
    pub teams: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SearchCandidate {
    pub player: SearchPlayer,
    pub match_kind: String,
    pub matched_name: String,
    pub score: u16,
}
#[derive(Clone, Debug, Serialize)]
pub struct SearchResult {
    pub query: String,
    pub status: String,
    pub candidates: Vec<SearchCandidate>,
    pub matches_total: usize,
    pub has_more: bool,
}
/// Reusable catalog for lexical retrieval and later bounded semantic resolution.
pub struct PlayerCatalog {
    players: Vec<SearchPlayer>,
}
/// Invalid input is rejected before scanning the catalog.
#[derive(Clone, Debug, Serialize)]
pub struct SearchError {
    pub error: &'static str,
}

impl PlayerCatalog {
    pub fn new(mut players: Vec<SearchPlayer>) -> Self {
        let mut aliases = csv::Reader::from_reader(
            include_str!("../../../docs/data/player-search-aliases.csv").as_bytes(),
        );
        for record in aliases.records() {
            let row = record.expect("curated alias record");
            if let Some(player) = players.iter_mut().find(|p| p.id == row[0])
                && !player.aliases.iter().any(|a| a == &row[1])
            {
                player.aliases.push(row[1].to_owned());
            }
        }
        players.sort_by(|a, b| a.id.cmp(&b.id));
        Self { players }
    }
    pub fn from_graph(graph: &TeammateGraph, reports: Option<&ReportMetadata>) -> Self {
        let players = graph
            .roster
            .players
            .iter()
            .map(|p| {
                if let Some(context) = reports.and_then(|r| r.players.get(&p.id)) {
                    SearchPlayer {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        aliases: context.aliases.clone(),
                        first_season: Some(context.first_season),
                        last_season: Some(context.last_season),
                        teams: context.teams.clone(),
                    }
                } else {
                    let mut teams: Vec<_> = graph
                        .roster
                        .tenures
                        .iter()
                        .filter(|t| t.player == p.id)
                        .map(|t| t.team.clone())
                        .collect();
                    teams.sort();
                    teams.dedup();
                    SearchPlayer {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        aliases: Vec::new(),
                        first_season: None,
                        last_season: None,
                        teams,
                    }
                }
            })
            .collect();
        Self::new(players)
    }
    pub fn player(&self, id: &str) -> Option<&SearchPlayer> {
        self.players.iter().find(|p| p.id == id)
    }
    pub fn lexical_shortlist(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<SearchResult, SearchError> {
        if !(1..=50).contains(&limit) {
            return Err(SearchError {
                error: "limit must be between 1 and 50",
            });
        }
        if query.chars().count() > 160 {
            return Err(SearchError {
                error: "query must contain at most 160 characters",
            });
        }
        let q = normalize(query);
        let mut candidates: Vec<_> = self
            .players
            .iter()
            .filter_map(|p| {
                if !q.is_empty() && normalize(&p.name) == q {
                    Some(SearchCandidate {
                        player: p.clone(),
                        match_kind: "exact_name".into(),
                        matched_name: p.name.clone(),
                        score: 1000,
                    })
                } else {
                    p.aliases
                        .iter()
                        .find(|a| {
                            !q.is_empty() && normalize(a).replace(' ', "") == q.replace(' ', "")
                        })
                        .map(|a| SearchCandidate {
                            player: p.clone(),
                            match_kind: "alias".into(),
                            matched_name: a.clone(),
                            score: 950,
                        })
                }
            })
            .collect();
        if candidates.is_empty() && q.chars().count() >= 2 {
            let max = if q.chars().count() >= 8 { 2 } else { 1 };
            candidates = self
                .players
                .iter()
                .filter_map(|p| {
                    std::iter::once(&p.name)
                        .chain(&p.aliases)
                        .filter_map(|name| {
                            let norm = normalize(name);
                            let single = q.split_whitespace().count() == 1;
                            let words: Vec<_> = norm.split_whitespace().collect();
                            let ranked = if single && words.contains(&q.as_str()) {
                                Some((850, "token"))
                            } else if norm.starts_with(&q) {
                                Some((800, "prefix"))
                            } else if single
                                && q.chars().count() >= 3
                                && words.iter().any(|word| word.starts_with(&q))
                            {
                                Some((750, "prefix"))
                            } else if q.chars().count() >= 4 {
                                let full = typo_distance(&q, &norm, max);
                                let token = if single {
                                    words
                                        .iter()
                                        .filter_map(|word| typo_distance(&q, word, 1))
                                        .min()
                                } else {
                                    None
                                };
                                full.map(|d| (600 - d as u16 * 20, "typo"))
                                    .or_else(|| token.map(|d| (550 - d as u16 * 20, "typo")))
                            } else {
                                None
                            };
                            ranked.map(|(score, kind)| SearchCandidate {
                                player: p.clone(),
                                match_kind: kind.into(),
                                matched_name: name.clone(),
                                score,
                            })
                        })
                        .max_by_key(|c| c.score)
                })
                .collect();
        }
        candidates.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.player.id.cmp(&b.player.id))
        });
        let matches_total = candidates.len();
        let status = match candidates.first() {
            None if q.is_empty() => "empty_query",
            None => "no_match",
            Some(first)
                if candidates
                    .get(1)
                    .is_some_and(|second| second.score == first.score) =>
            {
                "ambiguous"
            }
            Some(first) if first.match_kind == "exact_name" => "exact_match",
            Some(_) => "candidates",
        };
        candidates.truncate(limit);
        Ok(SearchResult {
            query: query.into(),
            status: status.into(),
            has_more: matches_total > candidates.len(),
            matches_total,
            candidates,
        })
    }
}
#[derive(Deserialize)]
pub(crate) struct SearchQuery {
    q: Option<String>,
    limit: Option<usize>,
}
pub(crate) async fn api(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Response {
    let catalog = PlayerCatalog::from_graph(&state.graph, state.reports.as_deref());
    match catalog.lexical_shortlist(query.q.as_deref().unwrap_or(""), query.limit.unwrap_or(20)) {
        Ok(result) => (StatusCode::OK, Json(result)).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(error)).into_response(),
    }
}

pub(crate) fn form(query: &str) -> String {
    format!(
        "<section class=\"player-search\"><h2>Find a player</h2><form class=\"controls\" method=\"get\" action=\"/search\"><label for=\"player-query\">Player name or nickname</label><input id=\"player-query\" name=\"q\" type=\"search\" maxlength=\"160\" value=\"{}\" placeholder=\"e.g. Michael Jordan or Shaq\"><button type=\"submit\">Search players</button></form></section>",
        crate::ui::escape(query)
    )
}
fn context(player: &SearchPlayer) -> String {
    let era = match (player.first_season, player.last_season) {
        (Some(first), Some(last)) => format!("{first}–{last}"),
        _ => "Era not recorded".into(),
    };
    let teams = if player.teams.is_empty() {
        "Teams not recorded".into()
    } else {
        player.teams.join(", ")
    };
    format!(
        "<p class=\"player-context\">Seasons: {} · Teams: {}</p>",
        crate::ui::escape(&era),
        crate::ui::escape(&teams)
    )
}
fn shell(state: &AppState, status: StatusCode, title: &str, body: &str) -> Response {
    crate::ui::document(
        status,
        title,
        &format!(
            "<nav><a href=\"/\">Back to explorer</a></nav>{}{}{}",
            crate::ui::semantic_status_line(state.jev.status()),
            crate::ui::coverage_line(state.reports.as_ref().map(|r| r.coverage.warning.as_str())),
            body
        ),
    )
}
pub(crate) async fn page(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Response {
    let q = query.q.as_deref().unwrap_or("");
    let catalog = PlayerCatalog::from_graph(&state.graph, state.reports.as_deref());
    let result = match catalog.lexical_shortlist(q, query.limit.unwrap_or(20)) {
        Ok(result) => result,
        Err(error) => {
            return shell(
                &state,
                StatusCode::BAD_REQUEST,
                "Invalid search — 7 Degrees",
                &format!(
                    "<h1>Invalid search</h1><p>{}</p>{}",
                    crate::ui::escape(error.error),
                    form(q)
                ),
            );
        }
    };
    let mut body = format!("<h1>Player search</h1>{}", form(q));
    if result.status == "empty_query" {
        body.push_str("<p>Enter a player name, nickname or spelling variation.</p>");
    } else if result.candidates.is_empty() {
        body.push_str("<p role=\"status\">No matching players found. Try a full name or a different spelling.</p>");
    } else {
        body.push_str("<h2>Choose the intended player</h2><p>Compare seasons and teams, then select a player.</p><ol class=\"search-candidates\">");
        for candidate in &result.candidates {
            body.push_str(&format!("<li><h3><a href=\"/players/{}\">{}</a></h3>{}<p><a href=\"/chain?from={}\">Start a connection with {}</a></p></li>",crate::ui::url_encode(&candidate.player.id),crate::ui::escape(&candidate.player.name),context(&candidate.player),crate::ui::url_encode(&candidate.player.id),crate::ui::escape(&candidate.player.name)));
        }
        body.push_str("</ol>");
        if result.has_more {
            body.push_str(&format!(
                "<p>Showing {} of {} candidates. Enter a fuller name to narrow the list.</p>",
                result.candidates.len(),
                result.matches_total
            ));
        }
    }
    shell(&state, StatusCode::OK, "Player search — 7 Degrees", &body)
}
pub(crate) async fn profile(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let catalog = PlayerCatalog::from_graph(&state.graph, state.reports.as_deref());
    let Some(player) = catalog.player(&id) else {
        return shell(
            &state,
            StatusCode::NOT_FOUND,
            "Player not found — 7 Degrees",
            "<h1>Player not found</h1><p>No player has that identity. <a href=\"/search\">Search players</a>.</p>",
        );
    };
    let alias_text = if player.aliases.is_empty() {
        String::new()
    } else {
        format!(
            "<p>Also known as: {}</p>",
            crate::ui::escape(&player.aliases.join(", "))
        )
    };
    shell(
        &state,
        StatusCode::OK,
        &format!("{} — 7 Degrees", player.name),
        &format!(
            "<h1>{}</h1>{}{}<p><a href=\"/chain?from={}\">Start a connection with {}</a></p><p><a href=\"/api/edges/{}\">View evidenced teammate links</a> · <a href=\"/api/coverage/{}\">View roster coverage</a></p>",
            crate::ui::escape(&player.name),
            context(player),
            alias_text,
            crate::ui::url_encode(&player.id),
            crate::ui::escape(&player.name),
            crate::ui::url_encode(&player.id),
            crate::ui::url_encode(&player.id)
        ),
    )
}
