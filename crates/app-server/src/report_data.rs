//! Canonical T3/T4 report import. Season brackets are coverage, never edge proof.
use graph_core::{Day, LocatedTenure, Player, RosterData, Team, TeammateGraph, Tenure};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Clone, Debug, Serialize)]
pub struct PlayerContext {
    pub id: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub first_season: u32,
    pub last_season: u32,
    pub teams: Vec<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct TenureRecord {
    pub record: String,
    pub player: String,
    pub team: String,
    pub season: u32,
    pub start_day: u32,
    pub end_day: u32,
    pub start_anchored: u8,
    pub end_anchored: u8,
    pub evidence_class: String,
    pub membership_source: String,
    pub arrival_days: String,
    pub departure_days: String,
    pub notes: String,
    pub source_manifest: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct CoverageGap {
    pub record: String,
    pub player: String,
    pub team: String,
    pub season: u32,
    pub evidence_class: String,
    pub start_day: Option<u32>,
    pub end_day: Option<u32>,
    pub reasons: String,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct Coverage {
    pub certified_tenures: usize,
    pub appearance_proven_edges: usize,
    pub game_witnessed_edges: usize,
    pub players_without_evidenced_edge: usize,
    pub excluded_tenures: BTreeMap<String, usize>,
    pub players_without_certified_tenure: usize,
    pub complete: bool,
    pub warning: String,
}
#[derive(Default)]
pub struct ReportMetadata {
    pub players: BTreeMap<String, PlayerContext>,
    pub records: BTreeMap<(String, String), Vec<TenureRecord>>,
    pub coverage: Coverage,
    pub gaps: BTreeMap<String, Vec<CoverageGap>>,
}
#[derive(Deserialize)]
struct PlayerRow {
    bbr_player_id: String,
    display_name: String,
    first_season: u32,
    last_season: u32,
    aba_only: String,
    #[serde(default)]
    s1_display_name: String,
}
#[derive(Deserialize)]
struct TenureRow {
    bbr_player_id: String,
    season: u32,
    lg: String,
    canonical_franchise: String,
    membership_source: String,
    evidence_class: String,
    start_day: Option<u32>,
    end_day: Option<u32>,
    start_anchored: u8,
    end_anchored: u8,
    #[serde(default)]
    unresolved_reasons: String,
    #[serde(default)]
    arrival_days: String,
    #[serde(default)]
    departure_days: String,
}

pub fn load(root: &Path) -> Result<(TeammateGraph, ReportMetadata), String> {
    let players =
        std::fs::File::open(root.join("t3/player-universe.csv")).map_err(|e| e.to_string())?;
    let tenures = std::fs::File::open(root.join("t4/tenures.csv")).map_err(|e| e.to_string())?;
    let (graph, mut metadata) = load_from_readers(players, tenures)?;
    let path = root.join("t4/appearance-counts.csv");
    let counts = if path.exists() {
        load_appearances(
            std::fs::File::open(path).map_err(|e| e.to_string())?,
            &mut metadata,
        )?
    } else {
        Vec::new()
    };
    let witness_path = root.join("t4/game-witnesses.csv");
    let witnesses = if witness_path.exists() {
        load_witnesses(
            std::fs::File::open(witness_path).map_err(|e| e.to_string())?,
            &metadata,
        )?
    } else {
        Vec::new()
    };
    let graph = TeammateGraph::build_with_evidence(graph.roster, counts, witnesses);
    let edges = graph.edges();
    let connected: BTreeSet<_> = edges.iter().flat_map(|e| [&e.a, &e.b]).collect();
    metadata.coverage.appearance_proven_edges = edges
        .iter()
        .filter(|e| e.evidence.iter().any(|v| v.appearance_proof.is_some()))
        .count();
    metadata.coverage.game_witnessed_edges = edges
        .iter()
        .filter(|e| e.evidence.iter().any(|v| v.game_witness.is_some()))
        .count();
    metadata.coverage.players_without_evidenced_edge = graph
        .roster
        .players
        .iter()
        .filter(|p| !connected.contains(&p.id))
        .count();
    metadata.coverage.warning = "Incomplete dated roster coverage, including 1946–1950 BAA evidence. Links require dated tenure overlap, an identified shared team game, or appearance counts that mathematically prove shared team games. Appearance proofs establish a relationship, but do not invent overlap dates or duration. Season-only membership, ambiguous counts and unresolved transaction brackets never create links. Degrees are exact within this evidenced graph; missing edges may shorten a historical chain or connect currently unreachable players.".into();
    Ok((graph, metadata))
}

/// Import canonical records with the same strict evidence rules for every
/// reader. No historical policy, acceptance flag or transformation lives here.
pub fn load_from_readers(
    players: impl std::io::Read,
    tenures: impl std::io::Read,
) -> Result<(TeammateGraph, ReportMetadata), String> {
    let mut roster = RosterData::default();
    let mut metadata = ReportMetadata::default();
    let mut players = csv::Reader::from_reader(players);
    for row in players.deserialize::<PlayerRow>() {
        let row = row.map_err(|e| e.to_string())?;
        if row.aba_only == "Y" {
            continue;
        }
        if row.bbr_player_id.is_empty() || metadata.players.contains_key(&row.bbr_player_id) {
            return Err(format!(
                "empty or duplicate canonical player id {:?}",
                row.bbr_player_id
            ));
        }
        roster.players.push(Player {
            id: row.bbr_player_id.clone(),
            name: row.display_name.clone(),
        });
        let aliases = if !row.s1_display_name.is_empty() && row.s1_display_name != row.display_name
        {
            vec![row.s1_display_name]
        } else {
            Vec::new()
        };
        metadata.players.insert(
            row.bbr_player_id.clone(),
            PlayerContext {
                id: row.bbr_player_id,
                name: row.display_name,
                aliases,
                first_season: row.first_season,
                last_season: row.last_season,
                teams: Vec::new(),
            },
        );
    }
    let mut teams = BTreeSet::new();
    let mut tenures = csv::Reader::from_reader(tenures);
    for (index, row) in tenures.deserialize::<TenureRow>().enumerate() {
        let row = row.map_err(|e| e.to_string())?;
        if !matches!(row.lg.as_str(), "NBA" | "BAA") {
            continue;
        }
        let Some(player) = metadata.players.get_mut(&row.bbr_player_id) else {
            return Err(format!(
                "tenure row {} references unknown canonical player {}",
                index + 2,
                row.bbr_player_id
            ));
        };
        if !player.teams.contains(&row.canonical_franchise) {
            player.teams.push(row.canonical_franchise.clone());
        }
        teams.insert(row.canonical_franchise.clone());
        if row.evidence_class != "directly-evidenced"
            || row.start_anchored != 1
            || row.end_anchored != 1
            || row
                .unresolved_reasons
                .split(';')
                .any(|reason| !reason.trim().is_empty())
        {
            metadata
                .gaps
                .entry(row.bbr_player_id.clone())
                .or_default()
                .push(CoverageGap {
                    record: format!("t4/tenures.csv:{}", index + 2),
                    player: row.bbr_player_id,
                    team: row.canonical_franchise,
                    season: row.season,
                    evidence_class: row.evidence_class.clone(),
                    start_day: row.start_day,
                    end_day: row.end_day,
                    reasons: row.unresolved_reasons,
                });
            *metadata
                .coverage
                .excluded_tenures
                .entry(row.evidence_class)
                .or_default() += 1;
            continue;
        }
        let (Some(start), Some(end)) = (row.start_day, row.end_day) else {
            return Err(format!("certified tenure row {} lacks dates", index + 2));
        };
        if start >= end || row.canonical_franchise.is_empty() {
            return Err(format!("invalid certified tenure row {}", index + 2));
        }
        roster.tenures.push(LocatedTenure {
            player: row.bbr_player_id.clone(),
            team: row.canonical_franchise.clone(),
            tenure: Tenure {
                start: Day(start),
                end: Day(end),
            },
        });
        metadata
            .records
            .entry((row.bbr_player_id.clone(), row.canonical_franchise.clone()))
            .or_default()
            .push(TenureRecord {
                record: format!("t4/tenures.csv:{}", index + 2),
                player: row.bbr_player_id,
                team: row.canonical_franchise,
                season: row.season,
                start_day: start,
                end_day: end,
                start_anchored: row.start_anchored,
                end_anchored: row.end_anchored,
                evidence_class: row.evidence_class,
                membership_source: row.membership_source,
                arrival_days: row.arrival_days,
                departure_days: row.departure_days,
                notes: row.unresolved_reasons,
                source_manifest:
                    "docs/data/source-manifest.md (S1 v238; S2 v56; BBR request ledger)".into(),
            });
        metadata.coverage.certified_tenures += 1;
    }
    for player in metadata.players.values_mut() {
        player.teams.sort();
    }
    let covered: BTreeSet<&str> = roster.tenures.iter().map(|t| t.player.as_str()).collect();
    metadata.coverage.players_without_certified_tenure = roster
        .players
        .iter()
        .filter(|p| !covered.contains(p.id.as_str()))
        .count();
    // The audit explicitly does not establish complete historical coverage,
    // even if a small input happens to contain no flagged row.
    metadata.coverage.warning = "Incomplete dated roster coverage, including 1946–1950 BAA evidence. Only tenures with both dated transaction boundaries create edges; inferred, cross-checked and unresolved brackets are excluded. A degree is exact within this evidenced graph; missing edges may shorten a historical chain or connect currently unreachable players.".into();
    roster.teams = teams
        .into_iter()
        .map(|id| Team {
            name: id.clone(),
            id,
        })
        .collect();
    Ok((TeammateGraph::build(roster), metadata))
}

#[derive(Deserialize)]
struct AppearanceRow {
    player: String,
    team: String,
    season: u32,
    lg: String,
    games: u32,
    team_games: u32,
    player_record: String,
    team_record: String,
}

fn load_appearances(
    reader: impl std::io::Read,
    metadata: &mut ReportMetadata,
) -> Result<Vec<graph_core::AppearanceCount>, String> {
    let mut counts = BTreeMap::new();
    let mut totals = BTreeMap::new();
    for row in csv::Reader::from_reader(reader).deserialize::<AppearanceRow>() {
        let row = row.map_err(|e| e.to_string())?;
        if !matches!(row.lg.as_str(), "NBA" | "BAA") {
            continue;
        }
        let player = metadata
            .players
            .get_mut(&row.player)
            .ok_or_else(|| format!("appearance row references unknown player {}", row.player))?;
        if row.team.is_empty()
            || !player.teams.contains(&row.team)
            || row.season < player.first_season
            || row.season > player.last_season
            || row.games == 0
            || row.games > row.team_games
            || row.team_games > 1000
            || row.player_record.is_empty()
            || row.team_record.is_empty()
        {
            return Err(format!(
                "invalid appearance count for {} {} {}",
                row.player, row.team, row.season
            ));
        }
        if let Some(total) = totals.insert((row.team.clone(), row.season), row.team_games)
            && total != row.team_games
        {
            return Err("conflicting team appearance totals".into());
        }
        if !player.teams.contains(&row.team) {
            player.teams.push(row.team.clone());
            player.teams.sort();
        }
        let key = (row.player.clone(), row.team.clone(), row.season);
        let count = graph_core::AppearanceCount {
            player: row.player,
            team: row.team,
            season: row.season,
            games: row.games,
            team_games: row.team_games,
            player_record: row.player_record,
            team_record: row.team_record,
        };
        if let Some(previous) = counts.insert(key, count.clone())
            && previous != count
        {
            return Err("conflicting player appearance records".into());
        }
    }
    Ok(counts.into_values().collect())
}

#[derive(Deserialize)]
struct WitnessRow {
    a: String,
    b: String,
    team: String,
    season: u32,
    game_id: String,
    date: String,
    a_source: String,
    b_source: String,
}
fn load_witnesses(
    reader: impl std::io::Read,
    metadata: &ReportMetadata,
) -> Result<Vec<graph_core::GameWitness>, String> {
    use chrono::Datelike;
    let mut witnesses = BTreeMap::new();
    for row in csv::Reader::from_reader(reader).deserialize::<WitnessRow>() {
        let row = row.map_err(|e| e.to_string())?;
        let date =
            chrono::NaiveDate::parse_from_str(&row.date, "%Y-%m-%d").map_err(|e| e.to_string())?;
        if row.a >= row.b
            || row.team.is_empty()
            || row.game_id.len() != 10
            || !row.game_id.bytes().all(|b| b.is_ascii_digit())
            || !(date.year() == row.season as i32 || date.year() == row.season as i32 - 1)
            || row.a_source.is_empty()
            || row.b_source.is_empty()
        {
            return Err("invalid dated game witness".into());
        }
        for id in [&row.a, &row.b] {
            let player = metadata
                .players
                .get(id)
                .ok_or("unknown game witness identity")?;
            if !player.teams.contains(&row.team)
                || row.season < player.first_season
                || row.season > player.last_season
            {
                return Err("game witness conflicts with canonical membership".into());
            }
        }
        let key = (row.a.clone(), row.b.clone(), row.team.clone(), row.season);
        let witness = graph_core::GameWitness {
            a: row.a,
            b: row.b,
            team: row.team,
            season: row.season,
            game_id: row.game_id,
            date: row.date,
            sources: [row.a_source, row.b_source],
        };
        if let Some(previous) = witnesses.insert(key, witness.clone())
            && previous != witness
        {
            return Err("conflicting duplicate game witnesses".into());
        }
    }
    Ok(witnesses.into_values().collect())
}
