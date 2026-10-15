use canvas_view::{GraphLink, GraphNode, LayoutMode, layout::compute_layout, visual::*};
use std::collections::{HashMap, HashSet};

#[test]
fn camera_keeps_world_circles_round_and_inverts_input_on_phone_and_desktop() {
    use canvas_view::camera::Viewport;
    for (width, height) in [(355.0, 420.0), (1086.0, 652.0)] {
        let viewport = Viewport::new(width, height);
        let camera = canvas_view::camera::Camera {
            scale: 0.5,
            x: 500.0,
            y: 300.0,
        };
        let center = viewport.screen((0.0, 0.0), camera);
        let x = viewport.screen((100.0, 0.0), camera);
        let y = viewport.screen((0.0, 100.0), camera);
        let css_x = (x.0 - center.0) * width / 1000.0;
        let css_y = (y.1 - center.1) * height / 600.0;
        assert!((css_x - css_y).abs() < 1e-9);
        let p = viewport.world(viewport.screen((27.0, -94.0), camera), camera);
        assert!((p.0 - 27.0).abs() < 1e-9 && (p.1 + 94.0).abs() < 1e-9);
    }
}
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
fn force_separates_heavy_roster_cliques_joined_by_brief_bridge_edges() {
    let nodes: Vec<_> = (0..30).map(|i| node(&format!("player{i:02}"))).collect();
    let mut links = Vec::new();
    for team in 0..3 {
        for a in team * 10..team * 10 + 10 {
            for b in a + 1..team * 10 + 10 {
                links.push(link(
                    &nodes[a].id,
                    &nodes[b].id,
                    0.95,
                    &format!("TEAM{team}"),
                ));
            }
        }
    }
    links.extend([
        link(&nodes[0].id, &nodes[10].id, 0.15, "BRIDGE"),
        link(&nodes[10].id, &nodes[20].id, 0.15, "BRIDGE"),
    ]);
    let points = compute_layout(
        &nodes,
        &links,
        LayoutMode::Force,
        "",
        &[],
        true,
        &HashMap::new(),
    );
    let mut clusters = Vec::new();
    for team in 0..3 {
        let members = &points[team * 10..team * 10 + 10];
        let center = (
            members.iter().map(|p| p.0).sum::<f64>() / 10.0,
            members.iter().map(|p| p.1).sum::<f64>() / 10.0,
        );
        let radius = members
            .iter()
            .map(|p| (p.0 - center.0).hypot(p.1 - center.1))
            .fold(0.0, f64::max);
        clusters.push((center, radius));
    }
    for (i, (a, ra)) in clusters.iter().enumerate() {
        for (b, rb) in &clusters[i + 1..] {
            assert!(
                (a.0 - b.0).hypot(a.1 - b.1) > ra + rb + 20.0,
                "roster islands overlap: {clusters:?}"
            );
        }
    }
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

#[test]
fn important_badges_find_a_free_gap_between_reserved_hud_bands() {
    let reserved = [
        Rect {
            left: 0.0,
            top: 0.0,
            right: 600.0,
            bottom: 22.0,
        },
        Rect {
            left: 0.0,
            top: 90.0,
            right: 600.0,
            bottom: 100.0,
        },
    ];
    let mut candidates = [LabelCandidate {
        index: 0,
        id: "a",
        priority: LabelPriority::Important,
        weight: 1.0,
        count: 1,
        matching: true,
        point: (300.0, 60.0),
        width: 60.0,
    }];
    let labels = place_labels(&mut candidates, 600.0, 100.0, 0, &reserved);
    assert_eq!(labels.len(), 1);
    assert!(
        reserved
            .iter()
            .all(|b| !b.padded().overlaps(labels[0].bounds.padded()))
    );
}

#[test]
fn unbundled_undirected_curves_keep_the_same_bend_when_endpoints_reverse() {
    let edge = link("a", "b", 0.1, "RED");
    let reversed = link("b", "a", 0.1, "RED");
    let curve = unbundled_curve(&edge, (0.0, 0.0), (100.0, 0.0));
    let reversed = unbundled_curve(&reversed, (100.0, 0.0), (0.0, 0.0));
    for t in [0.1, 0.5, 0.9] {
        assert_eq!(curve.point(t), reversed.point(t));
    }
}

#[test]
fn curve_picking_covers_the_full_spline_in_screen_space_at_high_zoom() {
    let curve = Curve::Cubic(
        (0.0, 0.0),
        (0.0, 30000.0),
        (30000.0, -30000.0),
        (30000.0, 0.0),
    );
    for t in [0.001, 0.023, 0.137, 0.245, 0.731, 0.997] {
        assert!(curve.distance(curve.point(t)) < 0.3, "missed spline at {t}");
    }
}

#[test]
fn detailed_labels_fade_at_zoom_boundaries_and_settle_under_reduced_motion() {
    assert_eq!(label_zoom_alpha(0.25, 0, false, 1086.0, false), 0.0);
    assert!(label_zoom_alpha(0.27, 0, false, 1086.0, false) > 0.0);
    assert!(label_zoom_alpha(0.27, 0, false, 1086.0, false) < 1.0);
    assert_eq!(label_zoom_alpha(0.75, 24, false, 1086.0, false), 0.0);
    assert!(label_zoom_alpha(0.79, 24, false, 1086.0, false) < 1.0);
    assert_eq!(label_zoom_alpha(0.75, 24, false, 1086.0, true), 1.0);
}

#[test]
fn tied_proofs_spread_direct_teammates_through_the_concentric_band() {
    let nodes: Vec<_> = (0..20).map(|i| node(&format!("p{i:02}"))).collect();
    let links: Vec<_> = (1..20)
        .map(|i| link(&nodes[0].id, &nodes[i].id, 1.0, "RED"))
        .collect();
    let hop: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), usize::from(i != 0)))
        .collect();
    let points = compute_layout(
        &nodes,
        &links,
        LayoutMode::Concentric,
        &nodes[0].id,
        &[],
        true,
        &hop,
    );
    let radii: Vec<_> = points[1..].iter().map(|p| p.0.hypot(p.1)).collect();
    assert!(
        radii.iter().copied().fold(0.0, f64::max)
            - radii.iter().copied().fold(f64::INFINITY, f64::min)
            > 200.0
    );
}

#[test]
fn rotating_a_mobile_pinch_keeps_its_css_distance() {
    let viewport = canvas_view::camera::Viewport::new(355.0, 420.0);
    let center = viewport.logical((177.5, 210.0));
    let horizontal = viewport.logical((277.5, 210.0));
    let vertical = viewport.logical((177.5, 310.0));
    assert!((viewport.distance(center, horizontal) - 100.0).abs() < 1e-9);
    assert!((viewport.distance(center, vertical) - 100.0).abs() < 1e-9);
}
