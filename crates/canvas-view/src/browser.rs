//! All application canvas rendering and interaction are Rust.
use crate::GraphPayload;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{
    CanvasRenderingContext2d, Document, Event, HtmlCanvasElement, HtmlSelectElement, KeyboardEvent,
    PointerEvent, WheelEvent,
};

struct CanvasView {
    document: Document,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    graph: GraphPayload,
    points: Vec<(f64, f64)>,
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
        self.points[self
            .graph
            .nodes
            .iter()
            .position(|n| n.id == id)
            .expect("server link endpoints exist")]
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
            self.document
                .get_element_by_id("graph-player")
                .unwrap()
                .dyn_into::<HtmlSelectElement>()
                .unwrap()
                .set_value(id);
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
        if let Some(id) = self
            .graph
            .nodes
            .iter()
            .zip(&self.points)
            .rev()
            .find(|(_, point)| {
                let s = self.screen(**point);
                (s.0 - p.0).hypot(s.1 - p.1) <= 22.0
            })
            .map(|(n, _)| n.id.clone())
        {
            self.select(&id);
            return;
        }
        let edge = self
            .graph
            .links
            .iter()
            .enumerate()
            .find(|(_, e)| {
                let a = self.screen(self.at(&e.from));
                let b = self.screen(self.at(&e.to));
                let vx = b.0 - a.0;
                let vy = b.1 - a.1;
                let t = ((p.0 - a.0) * vx + (p.1 - a.1) * vy) / (vx * vx + vy * vy);
                let t = t.clamp(0.0, 1.0);
                (p.0 - a.0 - t * vx).hypot(p.1 - a.1 - t * vy) < 8.0
            })
            .map(|(i, _)| i);
        if let Some(index) = edge {
            self.select_edge(index);
        }
    }
    fn draw(&mut self) {
        self.ctx.set_fill_style_str("#ffffff");
        self.ctx.fill_rect(0.0, 0.0, 1000.0, 600.0);
        for (index, edge) in self.graph.links.iter().enumerate() {
            let a = self.screen(self.at(&edge.from));
            let b = self.screen(self.at(&edge.to));
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
            let _ = self.ctx.arc(p.0, p.1, 18.0, 0.0, std::f64::consts::TAU);
            self.ctx.set_fill_style_str(if node.id == self.selected {
                "#b04f00"
            } else if self.graph.path.contains(&node.id) {
                "#176896"
            } else {
                "#657c6b"
            });
            self.ctx.fill();
            self.ctx.set_fill_style_str("#172e41");
            self.ctx.set_font("15px sans-serif");
            self.ctx.set_text_align("center");
            let _ = self.ctx.fill_text(&node.name, p.0, p.1 + 36.0);
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
    let points = graph
        .nodes
        .iter()
        .map(|node| {
            if let Some(index) = graph.path.iter().position(|id| id == &node.id) {
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
    let view = Rc::new(RefCell::new(CanvasView {
        document: document.clone(),
        canvas: canvas.clone(),
        ctx,
        graph,
        points,
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
        v.fit();
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
        listen(
            document
                .get_element_by_id(&format!("select-node-{index}"))
                .unwrap()
                .as_ref(),
            "click",
            move |_| {
                let mut v = state.borrow_mut();
                v.select(&id);
                v.draw();
            },
        )?;
    }
    for index in 0..view.borrow().graph.links.len() {
        let state = view.clone();
        listen(
            document
                .get_element_by_id(&format!("select-edge-{index}"))
                .unwrap()
                .as_ref(),
            "click",
            move |_| {
                let mut v = state.borrow_mut();
                v.select_edge(index);
                v.draw();
            },
        )?;
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
        state.borrow_mut().drag = None;
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
