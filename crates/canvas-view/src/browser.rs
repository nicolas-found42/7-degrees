//! All application canvas rendering and interaction are Rust.
use crate::GraphPayload;
use crate::layout::{LayoutMode, compute_layout};
use crate::visual::{self, Curve, LabelCandidate, LabelPriority, Rect, ZoomTier};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{
    CanvasRenderingContext2d, Document, Event, HtmlCanvasElement, KeyboardEvent, PointerEvent,
    WheelEvent,
};

mod explorer;

struct Transition {
    start: f64,
    from: Vec<(f64, f64)>,
    to: Vec<(f64, f64)>,
    camera_from: (f64, f64, f64),
    camera_to: (f64, f64, f64),
}

struct CanvasView {
    document: Document,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    graph: GraphPayload,
    layout: LayoutMode,
    points: Vec<(f64, f64)>,
    indexed_links: Vec<(usize, usize)>,
    /// Adjacency list over node indices, built once at startup.
    adjacency: Vec<Vec<usize>>,
    indices: HashMap<String, usize>,
    spatial: HashMap<(i32, i32), Vec<usize>>,
    scale: f64,
    x: f64,
    y: f64,
    selected: String,
    edge: Option<usize>,
    drag: Option<(f64, f64, f64, f64, bool)>,
    frames: usize,
    status_text: Option<String>,
    /// Hop distance to the selected player (BFS capped at 2), recomputed on
    /// selection client-side so focus never waits on a server round-trip.
    hop: HashMap<String, usize>,
    /// Node index under the pointer, for hover labels.
    hovered: Option<usize>,
    hovered_edge: Option<usize>,
    minimap_drag: Option<(f64, f64)>,
    pending_layout: bool,
    eras: [bool; 8],
    franchise: Option<String>,
    franchise_nodes: std::collections::HashSet<usize>,
    ego: bool,
    chain_generation: u64,
    transition: Option<Transition>,
    animation_scheduled: bool,
    pointers: HashMap<i32, (f64, f64)>,
    pinch: Option<((f64, f64), f64)>,
    gesture_moved: bool,
    label_widths: Vec<f64>,
    landmarks: Vec<bool>,
    backbone_edges: Vec<usize>,
    foreground_edges: Vec<usize>,
    focus_weights: Vec<f64>,
    trunks: HashMap<usize, Curve>,
    trunk_plan: visual::TrunkPlan,
    auto_focus: Option<String>,
}
impl CanvasView {
    fn fit(&mut self) {
        let ids: Vec<_> = if self.graph.path.is_empty() {
            vec![self.selected.as_str()]
        } else {
            self.graph.path.iter().map(String::as_str).collect()
        };
        let points: Vec<_> = self
            .graph
            .nodes
            .iter()
            .zip(&self.points)
            .filter(|(n, _)| ids.contains(&n.id.as_str()))
            .map(|(_, p)| *p)
            .collect();
        let lo = points.iter().map(|p| p.0).reduce(f64::min).unwrap_or(0.0);
        let hi = points.iter().map(|p| p.0).reduce(f64::max).unwrap_or(0.0);
        let ylo = points.iter().map(|p| p.1).reduce(f64::min).unwrap_or(0.0);
        let yhi = points.iter().map(|p| p.1).reduce(f64::max).unwrap_or(0.0);
        self.scale = (800.0 / (hi - lo + 200.0))
            .min(400.0 / (yhi - ylo + 200.0))
            .clamp(0.000001, 1.6);
        self.x = 500.0 - (lo + hi) / 2.0 * self.scale;
        self.y = 300.0 - (ylo + yhi) / 2.0 * self.scale;
    }
    fn zoom(&mut self, factor: f64, anchor: (f64, f64)) {
        let previous = self.scale;
        self.scale = (self.scale * factor).clamp(0.000001, 4.0);
        self.x = anchor.0 - (anchor.0 - self.x) * self.scale / previous;
        self.y = anchor.1 - (anchor.1 - self.y) * self.scale / previous;
    }
    fn at(&self, id: &str) -> (f64, f64) {
        self.points[self.indices[id]]
    }
    fn fit_network(&mut self) {
        let lo = self
            .points
            .iter()
            .map(|p| p.0)
            .reduce(f64::min)
            .unwrap_or(0.0);
        let hi = self
            .points
            .iter()
            .map(|p| p.0)
            .reduce(f64::max)
            .unwrap_or(0.0);
        let bottom = self
            .points
            .iter()
            .map(|p| p.1)
            .reduce(f64::min)
            .unwrap_or(0.0);
        let top = self
            .points
            .iter()
            .map(|p| p.1)
            .reduce(f64::max)
            .unwrap_or(0.0);
        self.scale = (900.0 / (hi - lo + 100.0))
            .min(500.0 / (top - bottom + 100.0))
            .clamp(0.000001, 1.6);
        self.x = 500.0 - (lo + hi) / 2.0 * self.scale;
        self.y = 300.0 - (bottom + top) / 2.0 * self.scale;
    }
    /// Recompute node coordinates for a new layout mode, then rebuild the
    /// spatial index and fit the network. Concentric rings follow the current
    /// hop distances, which must already be computed for the selection.
    fn relayout(&mut self, mode: LayoutMode) {
        if mode == LayoutMode::Concentric && self.selected.is_empty() {
            self.pending_layout = true;
            if let Some(picker) = self
                .document
                .get_element_by_id("graph-player")
                .and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok())
            {
                let _ = picker.focus();
            }
            self.document
                .get_element_by_id("graph-selection")
                .unwrap()
                .set_text_content(Some("Select a player to use Concentric rings."));
            return;
        }
        self.pending_layout = false;
        self.transition = None;
        self.layout = mode;
        self.points = compute_layout(
            &self.graph.nodes,
            &self.graph.links,
            mode,
            &self.selected,
            &self.graph.path,
            self.graph.full_network,
            &self.hop,
        );
        self.rebuild_spatial();
        if self.graph.full_network {
            self.fit_network();
        } else {
            self.fit();
        }
        self.report_layout();
    }
    /// Mirror the active layout in the status element so tests and assistive
    /// technology can observe it without reading canvas pixels.
    fn report_layout(&self) {
        if let Some(status) = self.document.get_element_by_id("canvas-status") {
            let _ = status.set_attribute("data-layout", self.layout.as_str());
            for (id, mode) in [
                ("layout-era", LayoutMode::EraLane),
                ("layout-concentric", LayoutMode::Concentric),
                ("layout-force", LayoutMode::Force),
            ] {
                if let Some(button) = self.document.get_element_by_id(id) {
                    let _ = button.set_attribute(
                        "aria-pressed",
                        if self.layout == mode { "true" } else { "false" },
                    );
                }
            }
        }
    }
    fn focus_player(&mut self) {
        if !self.indices.contains_key(&self.selected) {
            return;
        }
        let point = self.at(&self.selected);
        self.scale = 1.0;
        self.x = 500.0 - point.0;
        self.y = 300.0 - point.1;
    }
    fn screen(&self, p: (f64, f64)) -> (f64, f64) {
        (p.0 * self.scale + self.x, p.1 * self.scale + self.y)
    }
    fn pointer(&self, e: &PointerEvent) -> (f64, f64) {
        let r = self.canvas.get_bounding_client_rect();
        (
            (f64::from(e.client_x()) - r.left()) * 1000.0 / r.width(),
            (f64::from(e.client_y()) - r.top()) * 600.0 / r.height(),
        )
    }
    fn select(&mut self, id: &str) {
        self.settle_layout();
        let Some(n) = self.graph.nodes.iter().find(|n| n.id == id).cloned() else {
            return;
        };
        let changed = self.selected != id;
        self.selected = id.into();
        self.auto_focus = None;
        if changed {
            self.franchise = None;
            self.franchise_nodes.clear();
            self.clear_highlight();
            self.chain_generation += 1;
        }
        explorer::set_picker(&self.document, "graph-player", id, &n.name);
        self.render_franchises();
        self.edge = None;
        self.recompute_hop();
        self.trunk_plan =
            visual::TrunkPlan::new(&self.graph.nodes, &self.graph.links, &self.selected);
        if self.layout == LayoutMode::Concentric || self.pending_layout {
            self.ego = true;
            self.relayout(LayoutMode::Concentric);
        }
        if self.ego && self.layout == LayoutMode::Concentric {
            self.fit_visible();
        } else {
            self.focus_player();
        }
        if let Some(element) = self.document.get_element_by_id("graph-selection") {
            element.set_text_content(Some(&format!(
                "Selected player: {} — {}; teams: {}; {} teammate connections",
                n.name,
                n.era,
                n.teams.join(", "),
                self.adjacency[self.indices[id]].len(),
            )));
            let _ = element.set_attribute("data-player", id);
        }
    }
    /// BFS from the selected player over the adjacency list, capped at depth 2.
    /// Client-side so selecting never waits on a server round-trip (Liu & Heer
    /// 2014: feedback under ~100 ms keeps exploration fluid).
    fn recompute_hop(&mut self) {
        self.hop.clear();
        self.foreground_edges.clear();
        self.focus_weights.fill(0.0);
        let Some(&start) = self.indices.get(&self.selected) else {
            return;
        };
        let mut queue = std::collections::VecDeque::from([(start, 0usize)]);
        self.hop.insert(self.selected.clone(), 0);
        while let Some((node, depth)) = queue.pop_front() {
            if depth >= 2 {
                continue;
            }
            for &other in &self.adjacency[node] {
                let id = &self.graph.nodes[other].id;
                if !self.hop.contains_key(id) {
                    self.hop.insert(id.clone(), depth + 1);
                    queue.push_back((other, depth + 1));
                }
            }
        }
        self.foreground_edges = self
            .graph
            .links
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                let direct = e.from == self.selected || e.to == self.selected;
                if direct {
                    let other = if e.from == self.selected {
                        &e.to
                    } else {
                        &e.from
                    };
                    self.focus_weights[self.indices[other]] = e.weight;
                }
                let nearby = self.hop.contains_key(&e.from) && self.hop.contains_key(&e.to);
                (direct || nearby).then_some(i)
            })
            .collect();
    }

    fn select_edge(&mut self, index: usize) {
        self.edge = Some(index);
        let e = &self.graph.links[index];
        let url = format!(
            "/edge?from={}&to={}",
            js_sys::encode_uri_component(&e.from).as_string().unwrap(),
            js_sys::encode_uri_component(&e.to).as_string().unwrap()
        );
        explorer::dispatch_evidence(&self.document, &url);
    }

    /// Nearest node to a logical-space point within the pick radius, if any.
    /// Shared by click hit-testing and hover tracking.
    fn nearest_node(&self, p: (f64, f64)) -> Option<usize> {
        let radius = if self.graph.full_network { 9.0 } else { 22.0 };
        let rect = self.canvas.get_bounding_client_rect();
        let css_x = 1000.0 / rect.width().max(1.0);
        let css_y = 600.0 / rect.height().max(1.0);
        let world = ((p.0 - self.x) / self.scale, (p.1 - self.y) / self.scale);
        let r = radius * css_x.max(css_y) / self.scale;
        // Extremely zoomed-out hit tests scan the finite node set instead of
        // iterating an unbounded rectangle of empty spatial cells.
        let candidates: Vec<usize> = if (r / 40.0 + 3.0).powi(2) > self.spatial.len() as f64 {
            (0..self.points.len()).collect()
        } else {
            let mut candidates = Vec::new();
            for cx in
                (((world.0 - r) / 80.0).floor() as i32)..=(((world.0 + r) / 80.0).floor() as i32)
            {
                for cy in (((world.1 - r) / 80.0).floor() as i32)
                    ..=(((world.1 + r) / 80.0).floor() as i32)
                {
                    if let Some(indices) = self.spatial.get(&(cx, cy)) {
                        candidates.extend(indices.iter().copied());
                    }
                }
            }
            candidates
        };
        candidates
            .into_iter()
            .map(|i| {
                let point = self.screen(self.points[i]);
                (i, ((point.0 - p.0) / css_x).hypot((point.1 - p.1) / css_y))
            })
            .filter(|(i, d)| *d <= radius && self.visible(*i))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }
    fn hit(&mut self, p: (f64, f64)) {
        if let Some(i) = self.nearest_node(p) {
            let id = self.graph.nodes[i].id.clone();
            self.select(&id);
            return;
        }
        if let Some(index) = self.nearest_edge(p) {
            self.select_edge(index);
        }
    }
    fn nearest_edge(&self, p: (f64, f64)) -> Option<usize> {
        let rect = self.canvas.get_bounding_client_rect();
        let css_x = 1000.0 / rect.width().max(1.0);
        let css_y = 600.0 / rect.height().max(1.0);
        let p = (p.0 / css_x, p.1 / css_y);
        // The nearest link within reach wins, and in the complete network only links that are
        // drawn prominently (selected chain, selected player's links, current selection) are
        // targets, so a click in empty space never selects a faint background link.
        let full_network = self.graph.full_network;
        self.graph
            .links
            .iter()
            .enumerate()
            .filter(|(i, e)| {
                !full_network
                    || e.on_path
                    || e.from == self.selected
                    || e.to == self.selected
                    || self.edge == Some(*i)
                    || self.hovered_edge == Some(*i)
            })
            .filter_map(|(i, e)| {
                if !self.visible(self.indices[&e.from]) || !self.visible(self.indices[&e.to]) {
                    return None;
                }
                let curve = self.edge_curve(i, true).map(|point| {
                    let point = self.screen(point);
                    (point.0 / css_x, point.1 / css_y)
                });
                let distance = curve.distance(p);
                (distance < 8.0).then_some((i, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }
    fn clear_highlight(&mut self) {
        self.graph.path.clear();
        self.graph.degree = None;
        for e in &mut self.graph.links {
            e.on_path = false;
        }
        if let Some(chain) = self.document.get_element_by_id("graph-chain") {
            chain.set_inner_html("");
            let _ = chain.set_attribute("data-state", "idle");
        }
    }
    fn visible(&self, index: usize) -> bool {
        !self.ego || self.hop.contains_key(&self.graph.nodes[index].id) || self.important(index)
    }
    fn important(&self, index: usize) -> bool {
        let id = &self.graph.nodes[index].id;
        id == &self.selected
            || id == &self.graph.target
            || self.graph.path.contains(id)
            || self.hovered == Some(index)
    }
    fn emphasized(&self, index: usize) -> bool {
        let n = &self.graph.nodes[index];
        let era = n
            .debut_season
            .map(|y| self.eras[(y.saturating_sub(1940) / 10).min(7) as usize])
            .unwrap_or(self.eras.iter().all(|x| *x));
        era && (self.franchise.is_none()
            || self.franchise_nodes.contains(&index)
            || n.id == self.selected)
    }
    fn edge_curve(&self, index: usize, curved: bool) -> Curve {
        let e = &self.graph.links[index];
        let (a, b) = self.indexed_links[index];
        if !e.on_path
            && curved
            && let Some(curve) = self.trunks.get(&index)
        {
            return *curve;
        }
        if curved {
            visual::quadratic(self.points[a], self.points[b])
        } else {
            Curve::Line(self.points[a], self.points[b])
        }
    }
    fn stroke_edge(&self, index: usize, curved: bool) -> bool {
        let (a, b) = self.indexed_links[index];
        if !self.visible(a) || !self.visible(b) {
            return false;
        }
        let curve = self.edge_curve(index, curved).map(|p| self.screen(p));
        // Include spline control points in viewport culling, not only its chord.
        let points = match curve {
            Curve::Line(a, b) => [a, b, a, b],
            Curve::Quadratic(a, c, b) => [a, c, b, b],
            Curve::Cubic(a, c, d, b) => [a, c, d, b],
        };
        if !self.graph.links[index].on_path
            && (points.iter().all(|p| p.0 < 0.0)
                || points.iter().all(|p| p.0 > 1000.0)
                || points.iter().all(|p| p.1 < 0.0)
                || points.iter().all(|p| p.1 > 600.0))
        {
            return false;
        }
        let a = curve.point(0.0);
        self.ctx.move_to(a.0, a.1);
        match curve {
            Curve::Line(_, b) => self.ctx.line_to(b.0, b.1),
            Curve::Quadratic(_, c, b) => self.ctx.quadratic_curve_to(c.0, c.1, b.0, b.1),
            Curve::Cubic(_, c, d, b) => self.ctx.bezier_curve_to(c.0, c.1, d.0, d.1, b.0, b.1),
        }
        true
    }
    fn draw(&mut self) {
        let rect = self.canvas.get_bounding_client_rect();
        let css_x = 1000.0 / rect.width().max(1.0);
        let css_y = 600.0 / rect.height().max(1.0);
        let stroke_scale = (css_x * css_y).sqrt();
        let started = js_sys::Date::now();
        let zoom_tier = ZoomTier::at(self.scale);
        let reduced = explorer::reduced_motion();
        let micro_blend = visual::zoom_blend(self.scale, 0.75, reduced);
        let meso_blend = visual::zoom_blend(self.scale, 0.25, reduced);
        self.trunks = if self.layout == LayoutMode::Concentric {
            HashMap::new()
        } else {
            self.trunk_plan
                .curves(&self.graph.links, &self.points, |i| self.visible(i))
        };
        let mut drawn_background = 0;
        let mut drawn_focus = 0;
        let mut drawn_chain = 0;
        const COLORS: [&str; 8] = [
            "#1b9e77", "#d95f02", "#7570b3", "#e7298a", "#65a41e", "#bb8b02", "#a6761d", "#666666",
        ];
        self.ctx.set_global_alpha(1.0);
        self.ctx.set_fill_style_str("#ffffff");
        self.ctx.fill_rect(0.0, 0.0, 1000.0, 600.0);
        if self.layout == LayoutMode::EraLane {
            for (i, color) in COLORS.iter().enumerate() {
                let year = 1940 + i * 10;
                let x = (year as f64 - 1947.0) * 110.0 * self.scale + self.x;
                let end = if i == 7 {
                    1000.0
                } else {
                    x + 1100.0 * self.scale
                };
                self.ctx.set_global_alpha(0.045);
                self.ctx.set_fill_style_str(color);
                self.ctx.fill_rect(x, 0.0, end - x, 600.0);
                self.ctx.set_global_alpha(1.0);
                self.ctx.set_fill_style_str("#0f172a");
                self.ctx.set_text_align("left");
                self.ctx.set_font("bold 13px sans-serif");
                if end > 0.0 && x < 1000.0 {
                    let _ = self.ctx.fill_text(
                        &format!("{year}s{}", if i == 7 { "+" } else { "" }),
                        x.max(4.0) + 4.0,
                        20.0,
                    );
                }
            }
        }
        let active = !self.graph.path.is_empty();
        // Overview iterates the cached backbone, rather than four full edge passes.
        for local in [false, true] {
            if local && micro_blend == 0.0 {
                continue;
            }
            self.ctx.set_stroke_style_str("#426d7a");
            self.ctx.set_line_width(0.5 * stroke_scale);
            self.ctx.begin_path();
            self.ctx.set_global_alpha(
                (0.02 + 0.02 * meso_blend)
                    * if local { micro_blend } else { 1.0 }
                    * if active { 0.05 } else { 1.0 },
            );
            let candidates: Box<dyn Iterator<Item = usize>> = if local {
                Box::new(0..self.graph.links.len())
            } else {
                Box::new(self.backbone_edges.iter().copied())
            };
            for i in candidates {
                let e = &self.graph.links[i];
                if e.on_path
                    || self.edge == Some(i)
                    || self.hovered_edge == Some(i)
                    || (local && e.in_visual_backbone)
                {
                    continue;
                }
                if self.hop.contains_key(&e.from) && self.hop.contains_key(&e.to) {
                    continue;
                }
                if self.stroke_edge(i, false) {
                    drawn_background += 1;
                }
            }
            self.ctx.stroke();
        }
        for &i in &self.foreground_edges {
            let e = &self.graph.links[i];
            if e.on_path || self.edge == Some(i) || self.hovered_edge == Some(i) {
                continue;
            }
            let (a, b) = self.indexed_links[i];
            let direct = e.from == self.selected || e.to == self.selected;
            let alpha = if direct {
                0.18 + 0.10 * (e.weight.clamp(0.1, 1.0) - 0.1) / 0.9
            } else {
                0.055
            };
            self.ctx.set_global_alpha(
                alpha
                    * if active || !self.emphasized(a) || !self.emphasized(b) {
                        0.05
                    } else {
                        1.0
                    },
            );
            self.ctx.set_stroke_style_str("#b04f00");
            self.ctx.set_line_width(if direct {
                (0.65 + 0.85 * e.weight) * stroke_scale
            } else {
                0.5 * stroke_scale
            });
            self.ctx.begin_path();
            if self.stroke_edge(i, true) {
                drawn_focus += 1;
            }
            self.ctx.stroke();
        }
        // Chain and inspection always bypass backbone, filters and semantic zoom.
        let mut highlighted: Vec<_> = self
            .graph
            .path
            .windows(2)
            .filter_map(|p| {
                self.graph
                    .links
                    .binary_search_by(|e| {
                        let pair = if p[0] < p[1] {
                            (&p[0], &p[1])
                        } else {
                            (&p[1], &p[0])
                        };
                        (&e.from, &e.to).cmp(&pair)
                    })
                    .ok()
            })
            .collect();
        // Custom/fixture payloads need not arrive in pair order.
        if highlighted.len() != self.graph.path.len().saturating_sub(1) {
            highlighted = self
                .graph
                .links
                .iter()
                .enumerate()
                .filter_map(|(i, e)| e.on_path.then_some(i))
                .collect();
        }
        highlighted.extend(self.edge);
        highlighted.extend(self.hovered_edge);
        highlighted.sort_unstable();
        highlighted.dedup();
        self.ctx.set_global_alpha(1.0);
        for i in highlighted {
            let e = &self.graph.links[i];
            self.ctx
                .set_stroke_style_str(if e.on_path { "#176896" } else { "#b04f00" });
            self.ctx.set_line_width(2.2 * stroke_scale);
            self.ctx.begin_path();
            if self.stroke_edge(i, true) && e.on_path {
                drawn_chain += 1;
            }
            self.ctx.stroke();
        }
        // Nodes and labels use CSS pixels even when the logical canvas is resized.
        self.ctx.save();
        let _ = self.ctx.scale(css_x, css_y);
        self.ctx.set_font("15px sans-serif");
        self.ctx.set_text_align("center");
        for (i, (n, p)) in self.graph.nodes.iter().zip(&self.points).enumerate() {
            if !self.visible(i) {
                continue;
            }
            let logical = self.screen(*p);
            let p = (logical.0 / css_x, logical.1 / css_y);
            if p.0 < -150.0
                || p.0 > rect.width() + 150.0
                || p.1 < -50.0
                || p.1 > rect.height() + 50.0
            {
                continue;
            }
            let important = self.important(i);
            let alpha = if important {
                1.0
            } else if active
                || !self.emphasized(i)
                || (self.layout == LayoutMode::Concentric && !self.hop.contains_key(&n.id))
            {
                0.05
            } else {
                match self.hop.get(&n.id) {
                    Some(2) => 0.55,
                    _ => 1.0,
                }
            };
            let landmark = self.landmarks[i] || important;
            let macro_radius = ZoomTier::Macro.radius(self.adjacency[i].len(), landmark);
            let meso_radius = ZoomTier::Meso.radius(self.adjacency[i].len(), landmark);
            let micro_radius = ZoomTier::Micro.radius(self.adjacency[i].len(), landmark);
            let radius = (macro_radius * (1.0 - meso_blend) + meso_radius * meso_blend)
                * (1.0 - micro_blend)
                + micro_radius * micro_blend;
            let alpha = alpha
                * if !important && !self.landmarks[i] {
                    0.45 + 0.55 * meso_blend
                } else {
                    1.0
                };
            self.ctx.set_global_alpha(alpha);
            self.ctx
                .set_fill_style_str(if self.graph.path.contains(&n.id) {
                    "#176896"
                } else if n.id == self.selected {
                    "#b04f00"
                } else {
                    n.debut_season
                        .map(|y| COLORS[(y.saturating_sub(1940) / 10).min(7) as usize])
                        .unwrap_or("#94a3b8")
                });
            self.ctx.begin_path();
            let _ = self
                .ctx
                .arc(p.0, p.1, radius.max(1.3), 0.0, std::f64::consts::TAU);
            self.ctx.fill();
        }
        self.ctx.set_global_alpha(1.0);
        let mut reserved = Vec::new();
        for selector in [".camera-controls", "#graph-minimap", "#minimap-toggle"] {
            if let Ok(Some(overlay)) = self.document.query_selector(selector) {
                let bounds = overlay.get_bounding_client_rect();
                if bounds.width() > 0.0 && bounds.height() > 0.0 {
                    reserved.push(Rect {
                        left: bounds.left() - rect.left(),
                        top: bounds.top() - rect.top(),
                        right: bounds.right() - rect.left(),
                        bottom: bounds.bottom() - rect.top(),
                    });
                }
            }
        }
        if self.layout == LayoutMode::Concentric {
            self.ctx.set_font("bold 12px sans-serif");
            self.ctx.set_fill_style_str("#334155");
            for sector in crate::layout::concentric_sectors(&self.graph.links, &self.selected) {
                let angle = (sector.start + sector.end) / 2.0;
                let p = self.screen((500.0 * angle.cos(), 500.0 * angle.sin()));
                let (x, y) = (p.0 / css_x, p.1 / css_y);
                if x < 0.0 || x > rect.width() || y < 25.0 || y > rect.height() - 25.0 {
                    continue;
                }
                let text = if sector.franchise.is_empty() {
                    "Unknown"
                } else {
                    &sector.franchise
                };
                let w = self
                    .ctx
                    .measure_text(text)
                    .map(|m| m.width())
                    .unwrap_or(90.0);
                reserved.push(Rect {
                    left: x - w / 2.0,
                    top: y - 14.0,
                    right: x + w / 2.0,
                    bottom: y + 4.0,
                });
                let _ = self.ctx.fill_text(text, x, y);
            }
        }
        let mut candidates = Vec::new();
        for (i, n) in self.graph.nodes.iter().enumerate() {
            if !self.visible(i) {
                continue;
            }
            let important = self.important(i);
            if !important && (active || (zoom_tier == ZoomTier::Macro && !self.landmarks[i])) {
                continue;
            }
            let logical = self.screen(self.points[i]);
            let point = (logical.0 / css_x, logical.1 / css_y);
            if !important
                && (point.0 < 0.0
                    || point.0 > rect.width()
                    || point.1 < 35.0
                    || point.1 > rect.height() - 30.0)
            {
                continue;
            }
            candidates.push(LabelCandidate {
                index: i,
                id: &n.id,
                priority: if important {
                    LabelPriority::Important
                } else if self.hop.get(&n.id) == Some(&1) {
                    LabelPriority::DirectTeammate
                } else {
                    LabelPriority::Context
                },
                weight: self.focus_weights[i],
                count: self.adjacency[i].len(),
                matching: self.emphasized(i),
                point,
                width: self.label_widths[i] + 14.0,
            });
        }
        let cap = zoom_tier.label_cap(rect.width());
        let labels =
            visual::place_labels(&mut candidates, rect.width(), rect.height(), cap, &reserved);
        for label in &labels {
            let n = &self.graph.nodes[label.index];
            let important = label.priority == LabelPriority::Important;
            self.ctx.set_global_alpha(
                if important || label.priority == LabelPriority::DirectTeammate {
                    1.0
                } else if self.emphasized(label.index) {
                    0.7
                } else {
                    0.16
                },
            );
            self.ctx
                .set_font(if label.priority <= LabelPriority::DirectTeammate {
                    "bold 15px sans-serif"
                } else {
                    "15px sans-serif"
                });
            if important || (zoom_tier == ZoomTier::Macro && self.landmarks[label.index]) {
                self.ctx.set_fill_style_str("#172e41");
                self.ctx.begin_path();
                let _ = self.ctx.round_rect_with_f64(
                    label.bounds.left,
                    label.bounds.top,
                    label.bounds.right - label.bounds.left,
                    26.0,
                    6.0,
                );
                self.ctx.fill();
                self.ctx.set_stroke_style_str(
                    n.debut_season
                        .map(|v| COLORS[(v.saturating_sub(1940) / 10).min(7) as usize])
                        .unwrap_or("#94a3b8"),
                );
                self.ctx.set_line_width(1.5);
                self.ctx.stroke();
                self.ctx.set_fill_style_str("#ffffff");
            } else {
                self.ctx.set_stroke_style_str("#ffffff");
                self.ctx.set_line_width(3.0);
                self.ctx.set_line_join("round");
                let _ = self.ctx.stroke_text(&n.name, label.x, label.y);
                self.ctx.set_fill_style_str("#0f172a");
            }
            let _ = self.ctx.fill_text(&n.name, label.x, label.y);
        }
        self.ctx.set_global_alpha(1.0);
        self.ctx.restore();
        let _ = self
            .document
            .get_element_by_id("canvas-status")
            .unwrap()
            .set_attribute(
                "data-transition",
                if self.transition.is_some() {
                    "running"
                } else {
                    "settled"
                },
            );
        self.frames += 1;
        let status = self.document.get_element_by_id("canvas-status").unwrap();
        // role="status" is an atomic live region: rewriting identical text on every
        // frame floods screen readers during a drag, so only changed content is set.
        let text = format!(
            "Canvas ready — {} players, {} relationships. Zoom {:.0}%.{}",
            self.graph.nodes.len(),
            self.graph.links.len(),
            self.scale * 100.0,
            self.auto_focus
                .as_ref()
                .map(|name| format!(" Default focal player: {name}, chosen by teammate count."))
                .unwrap_or_default()
        );
        if self.transition.is_none()
            && self.pointers.is_empty()
            && self.status_text.as_deref() != Some(text.as_str())
        {
            self.status_text = Some(text.clone());
            status.set_text_content(Some(&text));
        }
        self.report_filters();
        self.report_hover();
        self.draw_minimap();
        for (key, value) in [
            ("data-tier", zoom_tier.name().into()),
            ("data-label-cap", cap.to_string()),
            (
                "data-labels",
                labels
                    .iter()
                    .map(|l| self.graph.nodes[l.index].id.as_str())
                    .collect::<Vec<_>>()
                    .join(","),
            ),
            (
                "data-ordinary-labels",
                labels
                    .iter()
                    .filter(|l| l.priority != LabelPriority::Important)
                    .count()
                    .to_string(),
            ),
            ("data-background-edges", drawn_background.to_string()),
            ("data-focus-edges", drawn_focus.to_string()),
            ("data-chain-edges", drawn_chain.to_string()),
            ("data-bundled-edges", self.trunks.len().to_string()),
            (
                "data-draw-ms",
                format!("{:.2}", js_sys::Date::now() - started),
            ),
            ("data-ready", "true".into()),
            ("data-zoom", format!("{:.6}", self.scale)),
            ("data-pan-x", format!("{:.2}", self.x)),
            ("data-pan-y", format!("{:.2}", self.y)),
            ("data-frame", self.frames.to_string()),
            ("data-players", self.graph.nodes.len().to_string()),
            ("data-relationships", self.graph.links.len().to_string()),
            ("data-full-network", self.graph.full_network.to_string()),
        ] {
            let _ = status.set_attribute(key, &value);
        }
    }
    fn request_redraw(&mut self) {
        self.draw();
    }
    /// Hover tooltip: mirror the hovered player's facts into the DOM overlay
    /// so pointer users get details on demand and screen-reader users get the
    /// same facts through the live selection region.
    fn report_hover(&self) {
        let Some(tip) = self.document.get_element_by_id("graph-tooltip") else {
            return;
        };
        let hovered = self.hovered.or_else(|| {
            self.hovered_edge.map(|i| {
                let e = &self.graph.links[i];
                self.indices[if e.from == self.selected {
                    &e.to
                } else {
                    &e.from
                }]
            })
        });
        let Some(index) = hovered else {
            tip.set_text_content(None);
            let _ = tip.set_attribute("hidden", "");
            let _ = tip.remove_attribute("data-player");
            return;
        };
        let node = &self.graph.nodes[index];
        let degree = self.adjacency[index].len();
        let text = format!(
            "{} — {}; teams: {}; {} teammate connections",
            node.name,
            node.era,
            node.teams.join(", "),
            degree,
        );
        if tip.text_content().as_deref() != Some(text.as_str()) {
            tip.set_text_content(Some(&text));
        }
        let _ = tip.remove_attribute("hidden");
        let _ = tip.set_attribute("data-player", &node.id);
    }
    /// Minimap (overview+detail): draw every node as a dot on the small
    /// overview canvas and frame the current viewport rectangle.
    fn draw_minimap(&self) {
        let Some(map) = self.document.get_element_by_id("graph-minimap") else {
            return;
        };
        let Ok(map) = map.dyn_into::<HtmlCanvasElement>() else {
            return;
        };
        let Some(ctx_obj) = map.get_context("2d").unwrap_or(None) else {
            return;
        };
        let Ok(mctx) = ctx_obj.dyn_into::<CanvasRenderingContext2d>() else {
            return;
        };
        let dpr = web_sys::window().unwrap().device_pixel_ratio().max(1.0);
        if map.width() != (200.0 * dpr) as u32 || map.height() != (120.0 * dpr) as u32 {
            map.set_width((200.0 * dpr) as u32);
            map.set_height((120.0 * dpr) as u32);
        }
        let _ = mctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
        let (mw, mh) = (200.0, 120.0);
        if self.points.is_empty() {
            return;
        }
        let (mut lo_x, mut hi_x, mut lo_y, mut hi_y) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for p in &self.points {
            lo_x = lo_x.min(p.0);
            hi_x = hi_x.max(p.0);
            lo_y = lo_y.min(p.1);
            hi_y = hi_y.max(p.1);
        }
        let span_x = (hi_x - lo_x).max(1.0);
        let span_y = (hi_y - lo_y).max(1.0);
        mctx.set_fill_style_str("#ffffff");
        mctx.fill_rect(0.0, 0.0, mw, mh);
        mctx.set_fill_style_str("#23405a");
        for (i, p) in self.points.iter().enumerate() {
            mctx.set_global_alpha(if self.important(i) {
                1.0
            } else if !self.visible(i) || !self.emphasized(i) {
                0.03
            } else {
                0.15
            });
            mctx.set_fill_style_str(if self.graph.path.contains(&self.graph.nodes[i].id) {
                "#176896"
            } else if self.graph.nodes[i].id == self.selected {
                "#b04f00"
            } else {
                "#23405a"
            });
            mctx.fill_rect(
                (p.0 - lo_x) / span_x * (mw - 4.0) + 2.0,
                (p.1 - lo_y) / span_y * (mh - 4.0) + 2.0,
                1.5,
                1.5,
            );
        }
        mctx.set_global_alpha(1.0);
        // Viewport rectangle: invert the main camera into world units.
        let vx0 = (0.0 - self.x) / self.scale;
        let vy0 = (0.0 - self.y) / self.scale;
        let vx1 = (1000.0 - self.x) / self.scale;
        let vy1 = (600.0 - self.y) / self.scale;
        mctx.set_stroke_style_str("#b04f00");
        mctx.set_line_width(1.5);
        mctx.stroke_rect(
            (vx0 - lo_x) / span_x * (mw - 4.0) + 2.0,
            (vy0 - lo_y) / span_y * (mh - 4.0) + 2.0,
            (vx1 - vx0) / span_x * (mw - 4.0),
            (vy1 - vy0) / span_y * (mh - 4.0),
        );
        let _ = map.set_attribute("data-frame", &self.frames.to_string());
    }
}
fn listen(
    element: &web_sys::EventTarget,
    event: &str,
    handler: impl FnMut(Event) + 'static,
) -> Result<(), JsValue> {
    let callback = Closure::<dyn FnMut(Event)>::new(handler);
    element.add_event_listener_with_callback(event, callback.as_ref().unchecked_ref())?;
    callback.forget();
    Ok(())
}
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let document = web_sys::window()
        .ok_or("window missing")?
        .document()
        .ok_or("document missing")?;
    let Some(payload) = document.get_element_by_id("graph-payload") else {
        return Ok(());
    };
    let graph: GraphPayload = serde_json::from_str(
        &payload
            .get_attribute("data-payload")
            .or_else(|| payload.text_content())
            .ok_or("payload missing")?,
    )
    .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let canvas = document
        .get_element_by_id("graph-canvas")
        .ok_or("canvas missing")?
        .dyn_into::<HtmlCanvasElement>()?;
    // Size the bitmap to the element's logical 1000×600 box times the device
    // pixel ratio so the canvas is not under-sampled on Retina displays (MDN,
    // devicePixelRatio). All drawing and hit-testing stays in the fixed logical
    // space; the context transform maps logical to device pixels.
    let dpr = web_sys::window()
        .map(|w| w.device_pixel_ratio())
        .unwrap_or(1.0)
        .max(1.0);
    canvas.set_width((1000.0 * dpr) as u32);
    canvas.set_height((600.0 * dpr) as u32);
    let ctx = canvas
        .get_context("2d")?
        .ok_or("2d canvas unavailable")?
        .dyn_into::<CanvasRenderingContext2d>()?;
    ctx.scale(dpr, dpr)?;
    // Initial coordinates come from the shared layout engine: the default
    // era-lane placement is byte-identical to the long-standing canvas
    // formula, so existing screenshots and hit-testing are preserved.
    let points: Vec<(f64, f64)> = compute_layout(
        &graph.nodes,
        &graph.links,
        LayoutMode::EraLane,
        &graph.focus,
        &graph.path,
        graph.full_network,
        &HashMap::new(),
    );
    let selected = graph.focus.clone();
    let indices: HashMap<_, _> = graph
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.clone(), i))
        .collect();
    let mut spatial: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    for (i, p) in points.iter().enumerate() {
        spatial
            .entry(((p.0 / 80.0).floor() as i32, (p.1 / 80.0).floor() as i32))
            .or_default()
            .push(i);
    }
    let indexed_links: Vec<(usize, usize)> = graph
        .links
        .iter()
        .map(|e| (indices[&e.from], indices[&e.to]))
        .collect();
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); graph.nodes.len()];
    for &(a, b) in &indexed_links {
        adjacency[a].push(b);
        adjacency[b].push(a);
    }
    ctx.set_font("bold 15px sans-serif");
    let label_widths = graph
        .nodes
        .iter()
        .map(|n| {
            ctx.measure_text(&n.name)
                .map(|m| m.width())
                .unwrap_or(100.0)
        })
        .collect();
    let landmarks = visual::landmarks(&graph.nodes);
    let backbone_edges = graph
        .links
        .iter()
        .enumerate()
        .filter_map(|(i, e)| e.in_visual_backbone.then_some(i))
        .collect();
    let focus_weights = vec![0.0; graph.nodes.len()];
    let view = Rc::new(RefCell::new(CanvasView {
        document: document.clone(),
        canvas: canvas.clone(),
        ctx,
        graph,
        layout: LayoutMode::EraLane,
        points,
        indexed_links,
        adjacency,
        indices,
        spatial,
        scale: 1.0,
        x: 0.0,
        y: 0.0,
        selected: selected.clone(),
        edge: None,
        drag: None,
        frames: 0,
        status_text: None,
        hop: HashMap::new(),
        hovered: None,
        hovered_edge: None,
        minimap_drag: None,
        pending_layout: false,
        eras: [true; 8],
        franchise: None,
        franchise_nodes: Default::default(),
        ego: false,
        chain_generation: 0,
        transition: None,
        animation_scheduled: false,
        pointers: HashMap::new(),
        pinch: None,
        gesture_moved: false,
        label_widths,
        landmarks,
        backbone_edges,
        focus_weights,
        foreground_edges: Vec::new(),
        trunks: HashMap::new(),
        trunk_plan: visual::TrunkPlan::default(),
        auto_focus: None,
    }));
    {
        let mut v = view.borrow_mut();
        if v.graph.full_network {
            v.fit_network();
        } else {
            v.fit();
        }
        v.select(&selected);
        if !v.graph.path.is_empty() {
            let points: Vec<_> = v.graph.path.iter().map(|id| v.at(id)).collect();
            v.frame_points(&points);
            let _ = v
                .document
                .get_element_by_id("graph-chain")
                .unwrap()
                .set_attribute("data-state", "connected");
        }
        v.report_layout();
        v.draw();
    }
    for (id, dx, dy, factor) in [
        ("pan-left", 60.0, 0.0, 1.0),
        ("pan-right", -60.0, 0.0, 1.0),
        ("pan-up", 0.0, 60.0, 1.0),
        ("pan-down", 0.0, -60.0, 1.0),
        ("zoom-in", 0.0, 0.0, 1.25),
        ("zoom-out", 0.0, 0.0, 0.8),
    ] {
        let state = view.clone();
        listen(
            document
                .get_element_by_id(id)
                .ok_or("control missing")?
                .as_ref(),
            "click",
            move |_| {
                let mut v = state.borrow_mut();
                v.settle_layout();
                v.x += dx;
                v.y += dy;
                v.zoom(factor, (500.0, 300.0));
                v.request_redraw();
            },
        )?;
    }
    if let Some(picker) = document
        .get_element_by_id("graph-player")
        .and_then(|e| e.dyn_into::<web_sys::HtmlSelectElement>().ok())
    {
        let state = view.clone();
        let picker_target: web_sys::EventTarget = picker.clone().into();
        listen(&picker_target, "change", move |_| {
            let mut v = state.borrow_mut();
            v.select(&picker.value());
            v.request_redraw();
        })?;
    }
    for id in ["fit-network", "focus-player"] {
        if let Some(element) = document.get_element_by_id(id) {
            let state = view.clone();
            listen(element.as_ref(), "click", move |_| {
                let mut v = state.borrow_mut();
                v.settle_layout();
                if id == "fit-network" {
                    v.ego = false;
                    v.fit_network();
                } else {
                    v.focus_player();
                }
                v.request_redraw();
            })?;
        }
    }
    // G6-style layout switcher: recompute coordinates through the shared
    // layout engine, then refit. Buttons are optional; the default era-lane
    // view renders when the page ships no switcher.
    for (id, mode) in [
        ("layout-era", LayoutMode::EraLane),
        ("layout-concentric", LayoutMode::Concentric),
        ("layout-force", LayoutMode::Force),
    ] {
        if let Some(element) = document.get_element_by_id(id) {
            let state = view.clone();
            listen(element.as_ref(), "click", move |_| {
                explorer::change_layout(state.clone(), mode);
            })?;
        }
    }
    // Minimap click-to-navigate: jump the main camera to the clicked
    // overview position.
    if let Some(map) = document
        .get_element_by_id("graph-minimap")
        .map(|el| el.dyn_into::<HtmlCanvasElement>())
        .transpose()
        .unwrap_or(None)
    {
        let state = view.clone();
        let map_target: web_sys::EventTarget = map.clone().into();
        listen(&map_target, "pointerdown", move |e| {
            let e = e.dyn_into::<PointerEvent>().unwrap();
            let r = map.get_bounding_client_rect();
            let mut v = state.borrow_mut();
            v.settle_layout();
            let _ = map.set_pointer_capture(e.pointer_id());
            let fx =
                ((f64::from(e.client_x()) - r.left()) / r.width().max(1.0) * 200.0 - 2.0) / 196.0;
            let fy =
                ((f64::from(e.client_y()) - r.top()) / r.height().max(1.0) * 120.0 - 2.0) / 116.0;
            let (mut lo_x, mut hi_x, mut lo_y, mut hi_y) = (
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
            );
            for p in &v.points {
                lo_x = lo_x.min(p.0);
                hi_x = hi_x.max(p.0);
                lo_y = lo_y.min(p.1);
                hi_y = hi_y.max(p.1);
            }
            let wx = lo_x + fx * (hi_x - lo_x).max(1.0);
            let wy = lo_y + fy * (hi_y - lo_y).max(1.0);
            let center = ((500.0 - v.x) / v.scale, (300.0 - v.y) / v.scale);
            if (wx - center.0).abs() <= 500.0 / v.scale && (wy - center.1).abs() <= 300.0 / v.scale
            {
                v.minimap_drag = Some((wx - center.0, wy - center.1));
            } else {
                v.minimap_drag = Some((0.0, 0.0));
                v.x = 500.0 - wx * v.scale;
                v.y = 300.0 - wy * v.scale;
            }
            v.request_redraw();
        })?;
    }
    if let Some(map) = document
        .get_element_by_id("graph-minimap")
        .and_then(|e| e.dyn_into::<HtmlCanvasElement>().ok())
    {
        let state = view.clone();
        let map_copy = map.clone();
        listen(map.as_ref(), "pointermove", move |e| {
            let e = e.dyn_into::<PointerEvent>().unwrap();
            if e.buttons() == 0 {
                return;
            }
            let r = map_copy.get_bounding_client_rect();
            let mut v = state.borrow_mut();
            let fx = ((f64::from(e.client_x()) - r.left()) / r.width() * 200.0 - 2.0) / 196.0;
            let fy = ((f64::from(e.client_y()) - r.top()) / r.height() * 120.0 - 2.0) / 116.0;
            let lo = v.points.iter().map(|p| p.0).reduce(f64::min).unwrap_or(0.0);
            let hi = v.points.iter().map(|p| p.0).reduce(f64::max).unwrap_or(0.0);
            let bottom = v.points.iter().map(|p| p.1).reduce(f64::min).unwrap_or(0.0);
            let top = v.points.iter().map(|p| p.1).reduce(f64::max).unwrap_or(0.0);
            let offset = v.minimap_drag.unwrap_or((0.0, 0.0));
            v.x = 500.0 - (lo + fx * (hi - lo).max(1.0) - offset.0) * v.scale;
            v.y = 300.0 - (bottom + fy * (top - bottom).max(1.0) - offset.1) * v.scale;
            v.draw();
        })?;
    }
    explorer::install(view.clone())?;
    let state = view.clone();
    listen(canvas.as_ref(), "pointerdown", move |event| {
        let e = event.dyn_into::<PointerEvent>().unwrap();
        if e.button() != 0 {
            return;
        }
        let mut v = state.borrow_mut();
        v.settle_layout();
        let p = v.pointer(&e);
        let fresh = v.pointers.is_empty();
        v.pointers.insert(e.pointer_id(), p);
        if fresh {
            v.gesture_moved = false;
            v.drag = Some((p.0, p.1, v.x, v.y, false));
        }
        if v.pointers.len() == 2 {
            let ps: Vec<_> = v.pointers.values().copied().collect();
            let centroid = ((ps[0].0 + ps[1].0) / 2.0, (ps[0].1 + ps[1].1) / 2.0);
            let distance = (ps[0].0 - ps[1].0).hypot(ps[0].1 - ps[1].1).max(1.0);
            v.pinch = Some((centroid, distance));
            v.gesture_moved = true;
            v.drag = None;
        }
        let _ = v.canvas.set_pointer_capture(e.pointer_id());
    })?;
    let state = view.clone();
    listen(canvas.as_ref(), "pointermove", move |event| {
        let e = event.dyn_into::<PointerEvent>().unwrap();
        let mut v = state.borrow_mut();
        let p = v.pointer(&e);
        if let std::collections::hash_map::Entry::Occupied(mut e) = v.pointers.entry(e.pointer_id())
        {
            e.insert(p);
            if v.pointers.len() >= 2 {
                let ps: Vec<_> = v.pointers.values().copied().collect();
                let centroid = ((ps[0].0 + ps[1].0) / 2.0, (ps[0].1 + ps[1].1) / 2.0);
                let distance = (ps[0].0 - ps[1].0).hypot(ps[0].1 - ps[1].1).max(1.0);
                if let Some((old, previous)) = v.pinch {
                    v.zoom(distance / previous, old);
                    v.x += centroid.0 - old.0;
                    v.y += centroid.1 - old.1;
                }
                v.pinch = Some((centroid, distance));
                v.gesture_moved = true;
            } else if let Some((sx, sy, ox, oy, moved)) = v.drag {
                let moved = moved || (p.0 - sx).hypot(p.1 - sy) > 5.0;
                v.drag = Some((sx, sy, ox, oy, moved));
                if moved {
                    v.x = ox + p.0 - sx;
                    v.y = oy + p.1 - sy;
                    v.gesture_moved = true;
                }
            }
            v.draw();
        } else if v.pointers.is_empty() {
            let hovered = v.nearest_node(p);
            let edge = if hovered.is_none() {
                v.nearest_edge(p)
            } else {
                None
            };
            if hovered != v.hovered || edge != v.hovered_edge {
                v.hovered = hovered;
                v.hovered_edge = edge;
                v.draw();
            }
        }
    })?;
    for name in ["pointerup", "pointercancel", "lostpointercapture"] {
        let state = view.clone();
        listen(canvas.as_ref(), name, move |event| {
            let e = event.dyn_into::<PointerEvent>().unwrap();
            let mut v = state.borrow_mut();
            if v.pointers.remove(&e.pointer_id()).is_none() {
                return;
            }
            let old = v.selected.clone();
            if v.pointers.is_empty() {
                if name == "pointerup" && !v.gesture_moved {
                    let p = v.pointer(&e);
                    v.hit(p);
                }
                v.drag = None;
                v.pinch = None;
                v.gesture_moved = false;
            } else {
                v.gesture_moved = true;
                v.pinch = None;
                let p = *v.pointers.values().next().unwrap();
                v.drag = Some((p.0, p.1, v.x, v.y, true));
            }
            let _ = v.canvas.release_pointer_capture(e.pointer_id());
            v.draw();
            let changed = v.selected != old;
            drop(v);
            if changed {
                explorer::request_chain(state.clone());
            }
        })?;
    }
    let state = view.clone();
    listen(canvas.as_ref(), "pointerleave", move |_| {
        let mut v = state.borrow_mut();
        if v.pointers.is_empty() {
            v.hovered = None;
            v.hovered_edge = None;
            v.draw();
        }
    })?;
    let state = view.clone();
    listen(web_sys::window().unwrap().as_ref(), "resize", move |_| {
        let mut v = state.borrow_mut();
        let ids: Vec<_> = v.pointers.drain().map(|(id, _)| id).collect();
        v.drag = None;
        v.pinch = None;
        v.gesture_moved = false;
        v.minimap_drag = None;
        for id in ids {
            let _ = v.canvas.release_pointer_capture(id);
        }
        let dpr = web_sys::window().unwrap().device_pixel_ratio().max(1.0);
        v.canvas.set_width((1000.0 * dpr) as u32);
        v.canvas.set_height((600.0 * dpr) as u32);
        let _ = v.ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
        v.draw();
    })?;
    let state = view.clone();
    let wheel = Closure::<dyn FnMut(WheelEvent)>::new(move |e: WheelEvent| {
        e.prevent_default();
        let mut v = state.borrow_mut();
        let r = v.canvas.get_bounding_client_rect();
        let p = (
            (f64::from(e.client_x()) - r.left()) * 1000.0 / r.width(),
            (f64::from(e.client_y()) - r.top()) * 600.0 / r.height(),
        );
        v.settle_layout();
        v.zoom(if e.delta_y() < 0.0 { 1.15 } else { 1.0 / 1.15 }, p);
        v.request_redraw();
    });
    let options = web_sys::AddEventListenerOptions::new();
    options.set_passive(false);
    canvas.add_event_listener_with_callback_and_add_event_listener_options(
        "wheel",
        wheel.as_ref().unchecked_ref(),
        &options,
    )?;
    wheel.forget();
    let state = view.clone();
    listen(canvas.as_ref(), "keydown", move |e| {
        let e = e.dyn_into::<KeyboardEvent>().unwrap();
        let mut v = state.borrow_mut();
        v.settle_layout();
        match e.key().as_str() {
            "ArrowLeft" => v.x += 60.0,
            "ArrowRight" => v.x -= 60.0,
            "ArrowUp" => v.y += 60.0,
            "ArrowDown" => v.y -= 60.0,
            "+" | "=" => v.zoom(1.25, (500.0, 300.0)),
            "-" => v.zoom(0.8, (500.0, 300.0)),
            "f" => {
                v.ego = false;
                v.fit_network();
            }
            _ => return,
        }
        e.prevent_default();
        v.request_redraw();
    })?;
    Ok(())
}
