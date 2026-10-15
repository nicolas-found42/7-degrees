use canvas_view::{GraphPayload, LayoutMode, layout::compute_layout};
use std::collections::HashMap;
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let p: GraphPayload = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let era = compute_layout(
        &p.nodes,
        &p.links,
        LayoutMode::EraLane,
        "",
        &[],
        true,
        &HashMap::new(),
    );
    let start = std::time::Instant::now();
    let old = compute_layout(
        &p.nodes,
        &p.links,
        LayoutMode::Force,
        "",
        &[],
        true,
        &HashMap::new(),
    );
    eprintln!("extended: {}ms", start.elapsed().as_millis());
    let ids: HashMap<_, _> = p
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let mut outputs = vec![("existing".to_owned(), old)];
    for (name, hub, power, repulsion, steps) in [
        ("fa2-base", false, 1.0, 2.0, 600),
        ("fa2-hub", true, 2.0, 2.0, 600),
    ] {
        let mut data = fa2::FA2Data::<f64>::with_capacity(p.nodes.len(), p.links.len());
        for &(x, y) in &era {
            data.add_node_with_position(x * 0.04, y * 0.04);
        }
        for e in &p.links {
            let (a, b) = (ids[e.from.as_str()], ids[e.to.as_str()]);
            let weight = e.weight.powf(power)
                / if hub {
                    (((p.nodes[a].degree + 1) * (p.nodes[b].degree + 1)) as f64).sqrt()
                } else {
                    1.0
                };
            data.add_edge_with_weight(a, b, weight);
        }
        let mut layout = fa2::FA2Settings::default()
            .parallel(false)
            .with_barnes_hut()
            .gravity(0.1)
            .scaling_ratio(repulsion)
            .build(data);
        let start = std::time::Instant::now();
        layout.run(steps);
        let positions: Vec<_> = layout.data().positions().collect();
        eprintln!("{name}: {}ms", start.elapsed().as_millis());
        outputs.push((name.into(), positions));
    }
    std::fs::write("outputs.json", serde_json::to_vec(&outputs).unwrap()).unwrap();
}
