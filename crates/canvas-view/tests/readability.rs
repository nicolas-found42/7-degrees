use canvas_view::{GraphLink, GraphNode, LayoutMode, layout::compute_layout, visual::*};
use std::collections::{HashMap, HashSet};
fn node(id: &str) -> GraphNode {
    GraphNode {
        id: id.into(),
        name: id.into(),
        era: String::new(),
        teams: vec![],
        distance: None,
        debut_season: Some(2000),
        degree: 4,
    }
}
fn link(a: &str, b: &str, w: f64, team: &str) -> GraphLink {
    GraphLink {
        from: a.into(),
        to: b.into(),
        team: team.into(),
        franchises: vec![team.into()],
        overlap_days: None,
        minimum_shared_games: None,
        on_path: false,
        weight: w,
        in_visual_backbone: false,
        primary_franchise: team.into(),
    }
}
#[test]
fn backbone_thins_a_clique_preserves_components_and_top_three() {
    let mut nodes: Vec<_> = (0..8).map(|i| node(&i.to_string())).collect();
    nodes.extend([node("isolate"), node("x"), node("y")]);
    let mut links = vec![link("x", "y", 0.1, "OTHER")];
    for a in 0..8 {
        for b in a + 1..8 {
            links.push(link(
                &a.to_string(),
                &b.to_string(),
                0.2 + (a + b) as f64 / 20.0,
                "RED",
            ));
        }
    }
    mark_backbone(&nodes, &mut links);
    assert!(links.iter().filter(|e| e.in_visual_backbone).count() < links.len());
    for n in &nodes {
        let mut incident: Vec<_> = links
            .iter()
            .filter(|e| e.from == n.id || e.to == n.id)
            .collect();
        incident.sort_by(|a, b| {
            b.weight
                .total_cmp(&a.weight)
                .then_with(|| (&a.from, &a.to).cmp(&(&b.from, &b.to)))
        });
        assert!(incident.iter().take(3).all(|e| e.in_visual_backbone));
    }
    let mut reached = HashSet::from(["0".to_owned()]);
    loop {
        let before = reached.len();
        for e in links.iter().filter(|e| e.in_visual_backbone) {
            if reached.contains(&e.from) {
                reached.insert(e.to.clone());
            }
            if reached.contains(&e.to) {
                reached.insert(e.from.clone());
            }
        }
        if before == reached.len() {
            break;
        }
    }
    assert_eq!(reached.len(), 8);
    assert!(!reached.contains("isolate"));
    assert!(links[0].in_visual_backbone);
    let first: Vec<_> = links
        .iter()
        .map(|e| ((&e.from, &e.to), e.in_visual_backbone))
        .collect();
    let mut reversed = links.clone();
    reversed.reverse();
    mark_backbone(&nodes, &mut reversed);
    reversed.reverse();
    assert_eq!(
        first,
        reversed
            .iter()
            .map(|e| ((&e.from, &e.to), e.in_visual_backbone))
            .collect::<Vec<_>>()
    );
}
#[test]
fn labels_rank_proof_strength_reserve_padding_and_exempt_important_badges() {
    let candidate = |index, id, priority, weight, point| LabelCandidate {
        index,
        id,
        priority: match priority {
            0 => LabelPriority::Important,
            1 => LabelPriority::DirectTeammate,
            _ => LabelPriority::Context,
        },
        weight,
        count: 10,
        matching: true,
        point,
        width: 60.0,
    };
    let mut labels = vec![
        candidate(0, "chain", 0, 0.1, (100.0, 100.0)),
        candidate(1, "weak", 1, 0.2, (300.0, 100.0)),
        candidate(2, "strong", 1, 0.9, (300.0, 100.0)),
        candidate(3, "context", 2, 1.0, (302.0, 100.0)),
    ];
    let placed = place_labels(&mut labels, 600.0, 400.0, 1, &[]);
    assert_eq!(placed.iter().map(|l| l.index).collect::<Vec<_>>(), [0, 2]);
    assert!(
        !placed[0]
            .bounds
            .padded()
            .overlaps(placed[1].bounds.padded())
    );
    let mut important = vec![
        candidate(0, "a", 0, 0.1, (100.0, 100.0)),
        candidate(1, "b", 0, 0.1, (100.0, 100.0)),
    ];
    assert_eq!(place_labels(&mut important, 600.0, 400.0, 0, &[]).len(), 2);
    assert_eq!(ZoomTier::at(0.1).label_cap(1000.0), 10);
    assert_eq!(ZoomTier::at(0.4).label_cap(390.0), 12);
    assert_eq!(ZoomTier::at(0.8).label_cap(1000.0), 50);
}
#[test]
fn concentric_sectors_separate_franchises_and_encode_proof_in_radius() {
    let nodes = vec![node("a"), node("b"), node("c"), node("d")];
    let links = vec![
        link("a", "b", 0.2, "RED"),
        link("a", "c", 0.9, "RED"),
        link("a", "d", 0.5, "BLUE"),
    ];
    let hop = HashMap::from([
        ("a".into(), 0),
        ("b".into(), 1),
        ("c".into(), 1),
        ("d".into(), 1),
    ]);
    let coords = compute_layout(&nodes, &links, LayoutMode::Concentric, "a", &[], true, &hop);
    assert!(coords[2].0.hypot(coords[2].1) < coords[1].0.hypot(coords[1].1));
    assert_eq!(
        coords,
        compute_layout(&nodes, &links, LayoutMode::Concentric, "a", &[], true, &hop)
    );
}
#[test]
fn force_layout_is_weight_sensitive_and_reproducible() {
    let nodes = vec![node("a"), node("b"), node("c"), node("d")];
    let links = vec![
        link("a", "b", 1.0, "RED"),
        link("c", "d", 1.0, "BLUE"),
        link("b", "c", 0.15, "BRIDGE"),
    ];
    let run = |links: &[GraphLink]| {
        compute_layout(
            &nodes,
            links,
            LayoutMode::Force,
            "",
            &[],
            true,
            &HashMap::new(),
        )
    };
    let strong = run(&links);
    assert_eq!(strong, run(&links));
    let mut light = links.clone();
    light[0].weight = 0.15;
    let weak = run(&light);
    let distance = |points: &[(f64, f64)], a: usize, b: usize| {
        (points[a].0 - points[b].0).hypot(points[a].1 - points[b].1)
    };
    assert!(distance(&strong, 0, 1) < distance(&weak, 0, 1));
    assert!(distance(&strong, 0, 1) < distance(&strong, 1, 2));
}
#[test]
fn bundled_curves_keep_exact_endpoints_and_leave_chains_and_small_groups_unbundled() {
    let nodes = vec![node("a"), node("b"), node("c"), node("d"), node("e")];
    let points = vec![
        (0.0, 0.0),
        (600.0, 200.0),
        (400.0, 500.0),
        (700.0, 500.0),
        (500.0, -300.0),
    ];
    let mut links = vec![
        link("a", "b", 0.8, "RED"),
        link("a", "c", 0.6, "RED"),
        link("a", "d", 0.4, "RED"),
        link("a", "e", 0.3, "BLUE"),
    ];
    let curves = franchise_trunks(&nodes, &links, &points, "a", &[true; 5], false);
    assert_eq!(curves.len(), 3);
    let curve = curves[&0];
    assert_eq!(curve.point(0.0), points[0]);
    assert_eq!(curve.point(1.0), points[1]);
    assert!(curve.distance(curve.point(0.9)) < 0.1);
    assert!(curve.distance((300.0, 100.0)) > 8.0);
    links[0].on_path = true;
    assert!(franchise_trunks(&nodes, &links, &points, "a", &[true; 5], false).is_empty());
    assert!(franchise_trunks(&nodes, &links, &points, "a", &[true; 5], true).is_empty());
}
#[test]
fn old_payloads_have_compatible_rendering_defaults() {
    let edge: GraphLink = serde_json::from_str(
        r#"{"from":"a","to":"b","team":"RED","overlap_days":null,"on_path":false}"#,
    )
    .unwrap();
    assert!(edge.in_visual_backbone);
    assert_eq!(edge.weight, 0.15);
    assert_eq!(edge.primary_franchise(), "RED");
}

#[test]
fn unknown_franchise_sectors_follow_known_franchises() {
    let links = vec![link("a", "b", 0.2, ""), link("a", "c", 0.9, "RED")];
    let sectors = canvas_view::layout::concentric_sectors(&links, "a");
    assert_eq!(sectors[0].franchise, "RED");
    assert!(sectors[1].franchise.is_empty());
    assert!(sectors[0].end < sectors[1].start);
}

#[test]
fn crowded_chain_badges_spread_horizontally_before_overlapping() {
    let mut candidates: Vec<_> = ["a", "b", "c", "d", "e"]
        .iter()
        .enumerate()
        .map(|(index, &id)| LabelCandidate {
            index,
            id,
            priority: LabelPriority::Important,
            weight: 0.1,
            count: 0,
            matching: true,
            point: (100.0, 100.0),
            width: 90.0,
        })
        .collect();
    let labels = place_labels(&mut candidates, 600.0, 180.0, 0, &[]);
    assert_eq!(labels.len(), 5);
    for (i, a) in labels.iter().enumerate() {
        for b in &labels[i + 1..] {
            assert!(!a.bounds.padded().overlaps(b.bounds.padded()));
        }
    }
}
