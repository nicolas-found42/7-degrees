//! Audit every admitted link against pinned counts or certified intervals.
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
fn rows(path: &Path) -> Vec<BTreeMap<String, String>> {
    csv::Reader::from_path(path)
        .unwrap()
        .deserialize()
        .map(Result::unwrap)
        .collect()
}
fn source_line<'a>(
    source: &str,
    rows: &'a [BTreeMap<String, String>],
) -> &'a BTreeMap<String, String> {
    let line: usize = source.rsplit_once(':').unwrap().1.parse().unwrap();
    &rows[line - 2]
}
fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (graph, metadata) = app_server::report_data::load(&root.join("docs/reports")).unwrap();
    let players = rows(&root.join("data/sumitrodatta/Player Totals.csv"));
    let teams = rows(&root.join("data/sumitrodatta/Team Totals.csv"));
    let mut aliases = BTreeMap::new();
    for row in rows(&root.join("docs/reports/t3/franchise-crosswalk.csv")) {
        if matches!(row["lg"].as_str(), "NBA" | "BAA") {
            aliases.insert(
                (
                    row["lg"].clone(),
                    row["season"].clone(),
                    row["abbreviation"].clone(),
                ),
                row["canonical_id"].clone(),
            );
        }
    }
    for count in &graph.appearance_counts {
        let p = source_line(&count.player_record, &players);
        let t = source_line(&count.team_record, &teams);
        assert_eq!(p["player_id"], count.player);
        assert_eq!(p["g"].parse::<u32>().unwrap(), count.games);
        assert_eq!(t["g"].parse::<u32>().unwrap(), count.team_games);
        assert_eq!(p["season"], count.season.to_string());
        assert_eq!(p["season"], t["season"]);
        assert_eq!(p["lg"], t["lg"]);
        assert_eq!(
            aliases[&(p["lg"].clone(), p["season"].clone(), p["team"].clone())],
            count.team
        );
        assert_eq!(
            aliases[&(
                t["lg"].clone(),
                t["season"].clone(),
                t["abbreviation"].clone()
            )],
            count.team
        );
    }
    let mut expected: BTreeSet<_> = graph
        .roster
        .teammate_edges()
        .into_iter()
        .map(|e| (e.a, e.b))
        .collect();
    let interval_pairs = expected.len();
    let mut groups = BTreeMap::<_, Vec<_>>::new();
    for count in &graph.appearance_counts {
        groups
            .entry((&count.team, count.season))
            .or_default()
            .push(count);
    }
    let mut appearance_pairs = BTreeSet::new();
    for group in groups.values() {
        for (i, a) in group.iter().enumerate() {
            for b in group.iter().skip(i + 1) {
                if u64::from(a.games) + u64::from(b.games) > u64::from(a.team_games) {
                    let pair = if a.player < b.player {
                        (a.player.clone(), b.player.clone())
                    } else {
                        (b.player.clone(), a.player.clone())
                    };
                    appearance_pairs.insert(pair);
                }
            }
        }
    }
    expected.extend(appearance_pairs.iter().cloned());
    let witness_rows = rows(&root.join("docs/reports/t4/game-witnesses.csv"));
    let witness_pairs: BTreeSet<_> = witness_rows
        .iter()
        .map(|w| (w["a"].clone(), w["b"].clone()))
        .collect();
    expected.extend(witness_pairs.iter().cloned());

    let edges = graph.edges();
    let actual: BTreeSet<_> = edges.iter().map(|e| (e.a.clone(), e.b.clone())).collect();
    assert_eq!(
        actual, expected,
        "missing proved links or unsupported extra links"
    );
    let positives = [
        ("mikange01", "pollaji01"), // normalized below; BAA/early NBA
        ("russebi01", "cousybo01"),
        ("pettibo01", "hagancl01"),
        ("maravpe01", "goodrga01"),
        ("goodrga01", "abdulka01"),
        ("maravpe01", "birdla01"),
        ("johnsma02", "abdulka01"),
        ("jordami01", "pippesc01"),
        ("bryanko01", "onealsh01"),
        ("jamesle01", "wadedw01"),
        ("curryst01", "thompkl01"),
        ("jokicni01", "murraja01"),
    ];
    let mut checks = Vec::new();
    for (a, b) in positives {
        let a = a.to_string();
        assert!(graph.contains_player(&a) && graph.contains_player(b));
        let pair = if a.as_str() < b {
            (a.clone(), b.to_string())
        } else {
            (b.to_string(), a.clone())
        };
        assert!(
            actual.contains(&pair),
            "missing known direct teammates: {a} {b}"
        );
        checks.push(json!({"from":a,"to":b,"expected":"direct_teammates","passed":true}));
    }
    for (a, b) in [("billuch01", "iversal01"), ("bellawa01", "debusda01")] {
        assert!(graph.contains_player(a) && graph.contains_player(b));
        let pair = if a < b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        };
        assert!(
            !actual.contains(&pair),
            "trade counterparts must not be linked by season membership"
        );
        checks.push(json!({"from":a,"to":b,"expected":"no_direct_edge","passed":true}));
    }
    let connected: BTreeSet<_> = edges.iter().flat_map(|e| [&e.a, &e.b]).collect();
    let isolates: Vec<_> = graph
        .roster
        .players
        .iter()
        .filter(|p| !connected.contains(&p.id))
        .map(|p| json!({"id":p.id,"name":p.name}))
        .collect();
    let stats = graph.statistics();
    let chain = graph.shortest_chain("maravpe01", "abdulka01").unwrap();
    assert!(matches!(chain, graph_core::Connection::Connected(ref c) if c.links.len() == 2));
    println!("{}", serde_json::to_string_pretty(&json!({
        "appearance_source_rows_verified":graph.appearance_counts.len(),
        "dated_witness_records":witness_rows.len(),"dated_witness_pairs":witness_pairs.len(),
        "interval_proven_pairs":interval_pairs,"appearance_proven_pairs":appearance_pairs.len(),
        "isolates":isolates,"total_edges":actual.len(),"unsupported_edges":0,"missing_proved_edges":0,
        "checks":checks,"coverage":metadata.coverage,
        "statistics":{"players":stats.players,"components":stats.components,"diameter":stats.diameter,
            "unreachable_pairs":stats.unreachable_pairs,"histogram":stats.histogram},
    })).unwrap());
}
