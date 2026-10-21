//! Orbit-only manual exploration; start/end queries still use the server boundary.
use super::*;

impl CanvasView {
    pub(super) fn begin_walk(&mut self) {
        let Some(&root) = self.indices.get(&self.selected) else {
            return;
        };
        self.chain_generation += 1;
        self.walk = vec![root];
        self.walk_distances = crate::orbit::distances(&self.adjacency, root);
        self.walk_target_distances = self
            .indices
            .get(&self.graph.target)
            .map(|&end| crate::orbit::distances(&self.adjacency, end));
        self.layout = LayoutMode::Orbit;
        self.update_walk();
    }
    pub(super) fn walk_to(&mut self, index: usize) {
        if let Some(step) = self.walk.iter().position(|&i| i == index) {
            self.walk.truncate(step + 1);
        } else if self.walk_next.contains(&index) {
            self.walk.push(index);
        } else {
            return;
        }
        self.update_walk();
    }
    fn update_walk(&mut self) {
        self.settle_layout();
        let from = self.points.clone();
        let camera_from = self.camera;
        self.walk_next = crate::orbit::next_steps(
            &self.adjacency,
            &self.walk,
            &self.walk_distances,
            self.walk_target_distances.as_deref(),
        );
        self.graph.path = self
            .walk
            .iter()
            .map(|&i| self.graph.nodes[i].id.clone())
            .collect();
        self.graph.degree = Some(self.walk.len() - 1);
        for edge in &mut self.graph.links {
            edge.on_path = self.graph.path.windows(2).any(|p| {
                (edge.from == p[0] && edge.to == p[1]) || (edge.to == p[0] && edge.from == p[1])
            });
        }
        let current = *self.walk.last().unwrap();
        self.recompute_hop();
        self.points = crate::orbit::layout(
            &self.graph.nodes,
            &self.graph.links,
            &self.graph.nodes[current].id,
            &[],
        );
        // A curved trail behind the centered current player, never a bottom row.
        let count = self.walk.len() - 1;
        for (k, &i) in self.walk[..count].iter().enumerate() {
            let back = (count - k) as f64;
            let angle = 2.4 + back * 0.68;
            let radius = 560.0 + back * 65.0;
            self.points[i] = (radius * angle.cos(), radius * angle.sin());
        }
        self.layout_signature = layout_signature(&self.points);
        self.rebuild_spatial();
        self.fit_visible();
        self.report_layout();
        let mut html = String::new();
        for &i in &self.walk_next {
            html.push_str(&format!(
                "<li><button type=\"button\" data-walk-player=\"{}\">{}</button></li>",
                i,
                explorer::escape(&self.graph.nodes[i].name)
            ));
        }
        if html.is_empty() {
            html.push_str(if self.graph.nodes[current].id == self.graph.target {
                "<li>Destination reached on a shortest chain.</li>"
            } else {
                "<li>No shortest extension here. Return to an earlier player, or choose a new pair.</li>"
            });
        }
        if let Some(list) = self.document.get_element_by_id("orbit-next") {
            list.set_inner_html(&html);
        }
        let mut html = format!("<h3>Shortest exploration · {} bounces</h3><ol>", count);
        for (k, &i) in self.walk.iter().enumerate() {
            html.push_str(&format!(
                "<li><button type=\"button\" data-walk-player=\"{}\">{}</button>",
                i,
                explorer::escape(&self.graph.nodes[i].name)
            ));
            if let Some(&next) = self.walk.get(k + 1) {
                html.push_str(&format!(
                    " <a data-evidence href=\"/edge?from={}&amp;to={}\">Relationship evidence</a>",
                    explorer::encoded(&self.graph.nodes[i].id),
                    explorer::encoded(&self.graph.nodes[next].id)
                ));
            }
            html.push_str("</li>");
        }
        html.push_str("</ol>");
        if let Some(chain) = self.document.get_element_by_id("graph-chain") {
            chain.set_inner_html(&html);
            let _ = chain.set_attribute("data-state", "exploring");
        }
        if let Some(status) = self.document.get_element_by_id("canvas-status") {
            let _ = status.set_attribute("data-walk-next", &self.walk_next.len().to_string());
        }
        if !explorer::reduced_motion() {
            let to = self.points.clone();
            let camera_to = self.camera;
            self.points = from.clone();
            self.camera = camera_from;
            self.rebuild_spatial();
            self.transition = Some(Transition {
                start: js_sys::Date::now(),
                from,
                to,
                camera_from,
                camera_to,
            });
        }
        self.draw();
    }
}
