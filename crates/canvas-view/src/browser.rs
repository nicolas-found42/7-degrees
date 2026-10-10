//! All application canvas rendering and interaction are Rust.
use crate::GraphPayload;
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{
    CanvasRenderingContext2d, Document, Event, HtmlCanvasElement, HtmlInputElement,
    HtmlSelectElement, KeyboardEvent, PointerEvent, WheelEvent,
};

struct CanvasView {
    document: Document,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    graph: GraphPayload,
    points: Vec<(f64, f64)>,
    indexed_links: Vec<(usize, usize)>,
    indices: HashMap<String, usize>,
    spatial: HashMap<(i32, i32), Vec<usize>>,
    scale: f64,
    x: f64,
    y: f64,
    selected: String,
    edge: Option<usize>,
    drag: Option<(f64, f64, f64, f64, bool)>,
    frames: usize,
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
    fn focus_player(&mut self) {
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
        if self.graph.full_network && self.selected != id {
            self.edge = None;
            if let Some(element) = self.document.get_element_by_id("selected-edge") {
                element.set_text_content(Some(
                    "Select a teammate relationship to inspect its graph facts.",
                ));
                let _ = element.remove_attribute("data-from");
                let _ = element.remove_attribute("data-to");
            }
            if let Some(frame) = self.document.get_element_by_id("edge-provenance-frame") {
                let _ = frame.set_attribute("src", "about:blank");
            }
            if let Some(link) = self.document.get_element_by_id("open-selected-edge") {
                let _ = link.remove_attribute("href");
                let _ = link.set_attribute("hidden", "");
            }
        }
        self.selected = id.into();
        if let Some(n) = self.graph.nodes.iter().find(|n| n.id == id) {
            let element = self.document.get_element_by_id("graph-selection").unwrap();
            element.set_text_content(Some(&format!(
                "Selected player: {} — {}; teams: {}",
                n.name,
                n.era,
                n.teams.join(", ")
            )));
            let _ = element.set_attribute("data-player", id);
            self.player_connections();
            self.document
                .get_element_by_id("graph-player")
                .unwrap()
                .dyn_into::<HtmlSelectElement>()
                .unwrap()
                .set_value(id);
        }
    }
    fn player_connections(&self) {
        let Some(list) = self.document.get_element_by_id("network-player-links") else {
            return;
        };
        list.set_text_content(None);
        if let Some(profile) = self.document.get_element_by_id("focus-player-profile") {
            let _ = profile.set_attribute(
                "href",
                &format!(
                    "/players/{}",
                    js_sys::encode_uri_component(&self.selected)
                        .as_string()
                        .unwrap()
                ),
            );
        }
        for (i, e) in self
            .graph
            .links
            .iter()
            .enumerate()
            .filter(|(_, e)| e.from == self.selected || e.to == self.selected)
        {
            let other = if e.from == self.selected {
                &e.to
            } else {
                &e.from
            };
            let item = self.document.create_element("li").unwrap();
            let button = self.document.create_element("button").unwrap();
            let _ = button.set_attribute("type", "button");
            let _ = button.set_attribute("data-network-edge-index", &i.to_string());
            button.set_text_content(Some(&format!(
                "{} — {}",
                self.graph.nodes[self.indices[other]].name, e.team
            )));
            let _ = item.append_child(&button);
            let _ = list.append_child(&item);
        }
    }
    fn select_edge(&mut self, index: usize) {
        self.edge = Some(index);
        let e = &self.graph.links[index];
        let element = self.document.get_element_by_id("selected-edge").unwrap();
        let name = |id: &str| {
            self.graph
                .nodes
                .iter()
                .find(|n| n.id == id)
                .map(|n| n.name.as_str())
                .unwrap_or("")
        };
        element.set_text_content(Some(&format!(
            "Selected relationship: {} ↔ {} — {}, {}. {}",
            name(&e.from),
            name(&e.to),
            e.team,
            e.overlap_days
                .map(|d| format!("{d} overlap day(s)"))
                .unwrap_or_else(|| api_types::overlap_description(None, e.minimum_shared_games)),
            self.graph.coverage
        )));
        let _ = element.set_attribute("data-from", &e.from);
        let _ = element.set_attribute("data-to", &e.to);
        let url = format!(
            "/edge?from={}&to={}",
            js_sys::encode_uri_component(&e.from).as_string().unwrap(),
            js_sys::encode_uri_component(&e.to).as_string().unwrap()
        );
        if let Some(frame) = self.document.get_element_by_id("edge-provenance-frame") {
            let _ = frame.set_attribute("src", &url);
        }
        if let Some(link) = self.document.get_element_by_id("open-selected-edge") {
            let _ = link.set_attribute("href", &url);
            let _ = link.remove_attribute("hidden");
        }
        let detail = web_sys::CustomEventInit::new();
        detail.set_detail(&JsValue::from_str(&serde_json::to_string(e).unwrap()));
        if let Ok(event) =
            web_sys::CustomEvent::new_with_event_init_dict("teammate-edge-selected", &detail)
        {
            let _ = self.document.dispatch_event(&event);
        }
    }
    fn hit(&mut self, p: (f64, f64)) {
        let radius = if self.graph.full_network { 7.0 } else { 22.0 };
        let world = ((p.0 - self.x) / self.scale, (p.1 - self.y) / self.scale);
        let r = radius / self.scale;
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
        let nearest = candidates
            .into_iter()
            .map(|i| {
                let point = self.screen(self.points[i]);
                (i, (point.0 - p.0).hypot(point.1 - p.1))
            })
            .filter(|(_, d)| *d <= radius)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        if let Some(i) = nearest {
            let id = self.graph.nodes[i].id.clone();
            self.select(&id);
            return;
        }
        // The nearest link within reach wins, and in the complete network only links that are
        // drawn prominently (selected chain, selected player's links, current selection) are
        // targets, so a click in empty space never selects a faint background link.
        let full_network = self.graph.full_network;
        let edge = self
            .graph
            .links
            .iter()
            .enumerate()
            .filter(|(i, e)| {
                !full_network
                    || e.on_path
                    || e.from == self.selected
                    || e.to == self.selected
                    || self.edge == Some(*i)
            })
            .filter_map(|(i, e)| {
                let a = self.screen(self.at(&e.from));
                let b = self.screen(self.at(&e.to));
                let vx = b.0 - a.0;
                let vy = b.1 - a.1;
                let length = vx * vx + vy * vy;
                let t = if length == 0.0 {
                    0.0
                } else {
                    (((p.0 - a.0) * vx + (p.1 - a.1) * vy) / length).clamp(0.0, 1.0)
                };
                let distance = (p.0 - a.0 - t * vx).hypot(p.1 - a.1 - t * vy);
                (distance < 8.0).then_some((i, distance))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        if let Some(index) = edge {
            self.select_edge(index);
        }
    }
    fn draw(&mut self) {
        self.ctx.set_fill_style_str("#ffffff");
        self.ctx.fill_rect(0.0, 0.0, 1000.0, 600.0);
        if self.graph.full_network {
            self.ctx.set_stroke_style_str("rgba(66,109,122,0.075)");
            self.ctx.set_line_width(0.6);
            self.ctx.begin_path();
            // The faint background lattice is the expensive part of a 100k-link frame: it is
            // skipped while a drag is moving (the pointer-up redraw restores it) and links
            // wholly off one side of the canvas are never submitted.
            let dragging = matches!(self.drag, Some((_, _, _, _, true)));
            if !dragging {
                for &(a, b) in &self.indexed_links {
                    let a = self.screen(self.points[a]);
                    let b = self.screen(self.points[b]);
                    if (a.0 < 0.0 && b.0 < 0.0)
                        || (a.0 > 1000.0 && b.0 > 1000.0)
                        || (a.1 < 0.0 && b.1 < 0.0)
                        || (a.1 > 600.0 && b.1 > 600.0)
                    {
                        continue;
                    }
                    self.ctx.move_to(a.0, a.1);
                    self.ctx.line_to(b.0, b.1);
                }
            }
            self.ctx.stroke();
            self.ctx.set_stroke_style_str("rgba(176,79,0,0.45)");
            self.ctx.set_line_width(1.3);
            self.ctx.begin_path();
            for (i, _e) in self
                .graph
                .links
                .iter()
                .enumerate()
                .filter(|(_, e)| e.from == self.selected || e.to == self.selected)
            {
                let (a, b) = self.indexed_links[i];
                let a = self.screen(self.points[a]);
                let b = self.screen(self.points[b]);
                self.ctx.move_to(a.0, a.1);
                self.ctx.line_to(b.0, b.1);
            }
            self.ctx.stroke();
        }
        for (index, edge) in self.graph.links.iter().enumerate() {
            if self.graph.full_network && !edge.on_path && self.edge != Some(index) {
                continue;
            }
            let (a, b) = self.indexed_links[index];
            let a = self.screen(self.points[a]);
            let b = self.screen(self.points[b]);
            self.ctx.set_stroke_style_str(if self.edge == Some(index) {
                "#b04f00"
            } else if edge.on_path {
                "#176896"
            } else {
                "#a4b4bd"
            });
            self.ctx
                .set_line_width(if edge.on_path || self.edge == Some(index) {
                    4.0
                } else {
                    1.3
                });
            self.ctx.begin_path();
            self.ctx.move_to(a.0, a.1);
            self.ctx.line_to(b.0, b.1);
            self.ctx.stroke();
        }
        for (node, point) in self.graph.nodes.iter().zip(&self.points) {
            let p = self.screen(*point);
            self.ctx.begin_path();
            let important = node.id == self.selected || self.graph.path.contains(&node.id);
            let radius = if self.graph.full_network {
                if important {
                    5.0
                } else {
                    (5.0 * self.scale).clamp(1.3, 8.0)
                }
            } else {
                18.0
            };
            let _ = self.ctx.arc(p.0, p.1, radius, 0.0, std::f64::consts::TAU);
            self.ctx.set_fill_style_str(if node.id == self.selected {
                "#b04f00"
            } else if self.graph.path.contains(&node.id) {
                "#176896"
            } else if self.graph.full_network {
                let year = node
                    .era
                    .get(..4)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(1980);
                [
                    "#64748b", "#8b5cf6", "#c05b9c", "#ca734b", "#bc9a2c", "#679a50", "#2d9f91",
                    "#3880b6", "#5a68bd",
                ][(year.saturating_sub(1940) / 10).min(8)]
            } else {
                "#657c6b"
            });
            self.ctx.fill();
            self.ctx.set_fill_style_str("#172e41");
            self.ctx.set_font("15px sans-serif");
            self.ctx.set_text_align("center");
            if !self.graph.full_network || important || self.scale >= 0.7 {
                let _ = self.ctx.fill_text(
                    &node.name,
                    p.0,
                    p.1 + if self.graph.full_network { 20.0 } else { 36.0 },
                );
            }
        }
        self.frames += 1;
        let status = self.document.get_element_by_id("canvas-status").unwrap();
        status.set_text_content(Some(&format!(
            "Canvas ready — {} players, {} relationships. Zoom {:.0}%.",
            self.graph.nodes.len(),
            self.graph.links.len(),
            self.scale * 100.0
        )));
        for (key, value) in [
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
    let ctx = canvas
        .get_context("2d")?
        .ok_or("2d canvas unavailable")?
        .dyn_into::<CanvasRenderingContext2d>()?;
    let center = graph
        .path
        .iter()
        .position(|id| id == &graph.focus)
        .unwrap_or(0) as f64
        * 170.0;
    let off_path = graph
        .nodes
        .iter()
        .filter(|n| !graph.path.contains(&n.id) && !(graph.path.is_empty() && n.id == graph.focus))
        .count();
    let mut extra = 0;
    let points: Vec<(f64, f64)> = graph
        .nodes
        .iter()
        .map(|node| {
            if graph.full_network {
                let year = node
                    .era
                    .get(..4)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1980.0);
                let hash = node.id.bytes().fold(2166136261u32, |h, b| {
                    (h ^ u32::from(b)).wrapping_mul(16777619)
                });
                let jitter = f64::from(hash % 1000) / 1000.0;
                let lane = f64::from((hash / 1000) % 10000) / 10000.0;
                (
                    (year - 1947.0) * 110.0 + jitter * 105.0,
                    (lane - 0.5) * 5200.0,
                )
            } else if let Some(index) = graph.path.iter().position(|id| id == &node.id) {
                (index as f64 * 170.0, 0.0)
            } else if graph.path.is_empty() && node.id == graph.focus {
                (0.0, 0.0)
            } else {
                let angle = std::f64::consts::TAU * (extra % 12) as f64
                    / (off_path - (extra / 12) * 12).min(12) as f64
                    - std::f64::consts::FRAC_PI_2;
                let radius = 150.0 + (extra / 12) as f64 * 100.0;
                extra += 1;
                (center + radius * angle.cos(), radius * angle.sin())
            }
        })
        .collect();
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
    let indexed_links = graph
        .links
        .iter()
        .map(|e| (indices[&e.from], indices[&e.to]))
        .collect();
    let view = Rc::new(RefCell::new(CanvasView {
        document: document.clone(),
        canvas: canvas.clone(),
        ctx,
        graph,
        points,
        indexed_links,
        indices,
        spatial,
        scale: 1.0,
        x: 0.0,
        y: 0.0,
        selected: selected.clone(),
        edge: None,
        drag: None,
        frames: 0,
    }));
    {
        let mut v = view.borrow_mut();
        if v.graph.full_network {
            v.fit_network();
        } else {
            v.fit();
        }
        v.select(&selected);
        v.draw();
    }
    for (id, dx, dy, factor) in [
        ("pan-left", 60.0, 0.0, 1.0),
        ("pan-right", -60.0, 0.0, 1.0),
        ("pan-up", 0.0, 60.0, 1.0),
        ("pan-down", 0.0, -60.0, 1.0),
        ("zoom-in", 0.0, 0.0, 1.25),
        ("zoom-out", 0.0, 0.0, 0.8),
        ("refocus", 0.0, 0.0, 1.0),
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
                if id == "refocus" {
                    v.fit();
                } else {
                    v.x += dx;
                    v.y += dy;
                    v.zoom(factor, (500.0, 300.0));
                }
                v.draw();
            },
        )?;
    }
    let node_ids: Vec<_> = view
        .borrow()
        .graph
        .nodes
        .iter()
        .map(|n| n.id.clone())
        .collect();
    for (index, id) in node_ids.into_iter().enumerate() {
        let state = view.clone();
        let Some(element) = document.get_element_by_id(&format!("select-node-{index}")) else {
            continue;
        };
        listen(element.as_ref(), "click", move |_| {
            let mut v = state.borrow_mut();
            v.select(&id);
            v.draw();
        })?;
    }
    let edge_indices: Vec<_> = view
        .borrow()
        .graph
        .links
        .iter()
        .enumerate()
        .filter(|(_, e)| !view.borrow().graph.full_network || e.on_path)
        .map(|(i, _)| i)
        .collect();
    for index in edge_indices {
        let state = view.clone();
        let Some(element) = document.get_element_by_id(&format!("select-edge-{index}")) else {
            continue;
        };
        listen(element.as_ref(), "click", move |_| {
            let mut v = state.borrow_mut();
            v.select_edge(index);
            v.draw();
        })?;
    }
    let state = view.clone();
    listen(
        document.get_element_by_id("graph-player").unwrap().as_ref(),
        "change",
        move |e| {
            if let Some(select) = e
                .target()
                .and_then(|t| t.dyn_into::<HtmlSelectElement>().ok())
            {
                let mut v = state.borrow_mut();
                v.select(&select.value());
                v.draw();
            }
        },
    )?;
    if let Some(form) = document.get_element_by_id("network-search-form") {
        let state = view.clone();
        listen(form.as_ref(), "submit", move |event| {
            event.prevent_default();
            let mut v = state.borrow_mut();
            let input = v
                .document
                .get_element_by_id("network-search")
                .unwrap()
                .dyn_into::<HtmlInputElement>()
                .unwrap()
                .value();
            let matches: Vec<_> = v
                .graph
                .nodes
                .iter()
                .filter(|n| {
                    n.name.eq_ignore_ascii_case(input.trim())
                        || format!("{} · {}", n.name, n.id) == input.trim()
                        || n.id == input.trim()
                })
                .map(|n| n.id.clone())
                .collect();
            let message = if matches.len() == 1 {
                v.select(&matches[0]);
                v.focus_player();
                v.draw();
                "Player found. Their connections are highlighted; the whole network remains loaded."
            } else {
                "Choose a player from the suggestions, including their ID when names are shared."
            };
            v.document
                .get_element_by_id("network-search-status")
                .unwrap()
                .set_text_content(Some(message));
        })?;
    }
    for id in ["fit-network", "focus-player"] {
        if let Some(element) = document.get_element_by_id(id) {
            let state = view.clone();
            listen(element.as_ref(), "click", move |_| {
                let mut v = state.borrow_mut();
                if id == "fit-network" {
                    v.fit_network();
                } else {
                    v.focus_player();
                }
                v.draw();
            })?;
        }
    }
    if let Some(list) = document.get_element_by_id("network-player-links") {
        let state = view.clone();
        listen(list.as_ref(), "click", move |event| {
            if let Some(index) = event
                .target()
                .and_then(|e| e.dyn_into::<web_sys::Element>().ok())
                .and_then(|e| e.get_attribute("data-network-edge-index"))
                .and_then(|i| i.parse::<usize>().ok())
            {
                let mut v = state.borrow_mut();
                if index < v.graph.links.len() {
                    v.select_edge(index);
                    v.draw();
                }
            }
        })?;
    }
    let state = view.clone();
    listen(canvas.as_ref(), "pointerdown", move |e| {
        let e = e.dyn_into::<PointerEvent>().unwrap();
        if e.button() != 0 {
            return;
        }
        let mut v = state.borrow_mut();
        let p = v.pointer(&e);
        v.drag = Some((p.0, p.1, v.x, v.y, false));
        let _ = v.canvas.set_pointer_capture(e.pointer_id());
    })?;
    let state = view.clone();
    listen(canvas.as_ref(), "pointermove", move |e| {
        let e = e.dyn_into::<PointerEvent>().unwrap();
        let mut v = state.borrow_mut();
        if let Some((sx, sy, ox, oy, moved)) = v.drag {
            let p = v.pointer(&e);
            let moved = moved || (p.0 - sx).hypot(p.1 - sy) > 5.0;
            v.drag = Some((sx, sy, ox, oy, moved));
            if moved {
                v.x = ox + p.0 - sx;
                v.y = oy + p.1 - sy;
                v.draw();
            }
        }
    })?;
    let state = view.clone();
    listen(canvas.as_ref(), "pointerup", move |e| {
        let e = e.dyn_into::<PointerEvent>().unwrap();
        let mut v = state.borrow_mut();
        if let Some((_, _, _, _, moved)) = v.drag.take() {
            if !moved {
                let p = v.pointer(&e);
                v.hit(p);
            }
            let _ = v.canvas.release_pointer_capture(e.pointer_id());
            v.draw();
        }
    })?;
    let state = view.clone();
    listen(canvas.as_ref(), "pointercancel", move |_| {
        let mut v = state.borrow_mut();
        v.drag = None;
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
        v.zoom(if e.delta_y() < 0.0 { 1.15 } else { 1.0 / 1.15 }, p);
        v.draw();
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
        match e.key().as_str() {
            "ArrowLeft" => v.x += 60.0,
            "ArrowRight" => v.x -= 60.0,
            "ArrowUp" => v.y += 60.0,
            "ArrowDown" => v.y -= 60.0,
            "+" | "=" => v.zoom(1.25, (500.0, 300.0)),
            "-" => v.zoom(0.8, (500.0, 300.0)),
            "f" => v.fit(),
            _ => return,
        }
        e.prevent_default();
        v.draw();
    })?;
    Ok(())
}
