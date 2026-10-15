use app_server::graph_view::edge_visual_weight;
use graph_core::{AppearanceProof, EdgeEvidence, GameWitness, TeammateEdge};

fn edge(evidence: Vec<EdgeEvidence>) -> TeammateEdge {
    TeammateEdge {
        a: "a".into(),
        b: "b".into(),
        evidence,
    }
}
fn dated(team: &str, days: u32) -> EdgeEvidence {
    EdgeEvidence {
        team: team.into(),
        overlap_days: Some(days),
        appearance_proof: None,
        game_witness: None,
    }
}
fn appearance(season: u32, a: u32, b: u32, n: u32) -> EdgeEvidence {
    EdgeEvidence {
        team: "RED".into(),
        overlap_days: None,
        game_witness: None,
        appearance_proof: Some(AppearanceProof {
            season,
            a_games: a,
            b_games: b,
            team_games: n,
            minimum_shared_games: a + b - n,
            player_records: ["a".into(), "b".into()],
            team_record: "r".into(),
        }),
    }
}
#[test]
fn all_admitted_proofs_contribute_without_inventing_roster_dates() {
    let witness = EdgeEvidence {
        team: "RED".into(),
        overlap_days: None,
        appearance_proof: None,
        game_witness: Some(GameWitness {
            a: "a".into(),
            b: "b".into(),
            team: "RED".into(),
            season: 2000,
            game_id: "official-game".into(),
            date: "2000-01-01".into(),
            sources: ["a".into(), "b".into()],
        }),
    };
    assert_eq!(edge_visual_weight(&edge(vec![witness.clone()])).0, 0.15);
    let short = edge_visual_weight(&edge(vec![dated("RED", 10)])).0;
    let long = edge_visual_weight(&edge(vec![dated("RED", 1000)])).0;
    assert!(long > short && short > 0.15);
    let one = appearance(1999, 20, 25, 40); // Shortened season: five guaranteed games.
    let two = appearance(2000, 20, 25, 40);
    let games = edge_visual_weight(&edge(vec![one.clone(), two.clone()])).0;
    assert!(games > edge_visual_weight(&edge(vec![one.clone()])).0);
    assert_eq!(edge_visual_weight(&edge(vec![one, two, witness])).0, games);
    assert_eq!(
        edge_visual_weight(&edge(vec![
            dated("RED", 1000),
            appearance(2000, 20, 25, 40)
        ]))
        .0,
        long
    );
    let (weight, primary) = edge_visual_weight(&edge(vec![dated("RED", 10), dated("BLUE", 1000)]));
    assert!(weight >= long);
    assert_eq!(primary, "BLUE");
    assert_eq!(
        edge_visual_weight(&edge(vec![dated("RED", 10), dated("BLUE", 10)])).1,
        "BLUE"
    );
}
