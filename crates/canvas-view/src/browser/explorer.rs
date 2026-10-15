//! Enhanced controls use the existing deterministic HTTP boundaries.
use super::*;
use api_types::ExplorerConnection;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    HtmlDialogElement, HtmlElement, HtmlInputElement, HtmlSelectElement, MouseEvent, Response,
};

type View = Rc<RefCell<CanvasView>>;
fn element(document: &Document, id: &str) -> web_sys::Element {
    document.get_element_by_id(id).expect("explorer contract")
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
fn encoded(text: &str) -> String {
    js_sys::encode_uri_component(text).as_string().unwrap()
}
async fn fetch_text(url: &str) -> Result<String, JsValue> {
    let response = JsFuture::from(web_sys::window().unwrap().fetch_with_str(url))
        .await?
        .dyn_into::<Response>()?;
    if !response.ok() {
        return Err(JsValue::from_str(&format!("HTTP {}", response.status())));
    }
    JsFuture::from(response.text()?)
        .await?
        .as_string()
        .ok_or_else(|| JsValue::from_str("response text unavailable"))
}
pub(super) fn set_picker(document: &Document, id: &str, value: &str, name: &str) {
    let picker = element(document, id);
    let _ = picker.set_attribute("data-selected-id", value);
    if let Some(input) = picker.dyn_ref::<HtmlInputElement>() {
        input.set_value(name);
    } else if let Some(select) = picker.dyn_ref::<HtmlSelectElement>() {
        select.set_value(value);
    }
}
impl CanvasView {
    pub(super) fn report_filters(&self) {
        let matching = self
            .graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| self.emphasized(*i))
            .count();
        let visible = self
            .graph
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| self.visible(*i))
            .count();
        let eras = if self.eras.iter().all(|v| *v) {
            "All eras emphasized".to_owned()
        } else if self.eras.iter().all(|v| !*v) {
            "No eras emphasized".to_owned()
        } else {
            format!(
                "Debut eras: {}",
                self.eras
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| **v)
                    .map(|(i, _)| format!("{}s{}", 1940 + i * 10, if i == 7 { "+" } else { "" }))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        let text = format!(
            "{eras}; franchise: {}; {matching} matching of {} loaded players; {visible} visible. {}",
            self.franchise.as_deref().unwrap_or("all"),
            self.graph.nodes.len(),
            if self.ego {
                "Ego network: complete 1–2 hops; active chain remains included beyond two hops."
            } else {
                "Whole network visible."
            }
        );
        let status = element(&self.document, "filter-status");
        if status.text_content().as_deref() != Some(&text) {
            status.set_text_content(Some(&text));
        }
        let _ = status.set_attribute("data-matching", &matching.to_string());
        let _ = status.set_attribute("data-visible", &visible.to_string());
        let _ = element(&self.document, "ego-toggle")
            .set_attribute("aria-pressed", if self.ego { "true" } else { "false" });
        let _ = element(&self.document, "ego-toggle")
            .toggle_attribute_with_force("disabled", self.selected.is_empty());
        for i in 0..8 {
            let _ = element(&self.document, &format!("era-{i}"))
                .set_attribute("aria-pressed", if self.eras[i] { "true" } else { "false" });
        }
    }
    pub(super) fn settle_layout(&mut self) {
        if let Some(transition) = self.transition.take() {
            self.points = transition.to;
            self.camera = transition.camera_to;
            self.rebuild_spatial();
        }
    }
    pub(super) fn render_franchises(&self) {
        let html=self.indices.get(&self.selected).map(|i|self.graph.nodes[*i].teams.iter().map(|team|format!("<button type=\"button\" data-franchise=\"{}\" aria-pressed=\"{}\">{}</button>",escape(team),self.franchise.as_ref()==Some(team),escape(team))).collect::<String>()).unwrap_or_default();
        element(&self.document, "franchise-filters").set_inner_html(&html);
    }
    pub(super) fn fit_visible(&mut self) {
        let points: Vec<_> = self
            .points
            .iter()
            .enumerate()
            .filter(|(i, _)| self.visible(*i))
            .map(|(_, p)| *p)
            .collect();
        self.frame_points(&points);
    }
    pub(super) fn frame_points(&mut self, points: &[(f64, f64)]) {
        let controls = element(&self.document, "zoom-in")
            .parent_element()
            .map(|e| e.get_bounding_client_rect().height())
            .unwrap_or(100.0);
        let mut area = Rect {
            left: 24.0,
            right: (self.viewport.width - 24.0).max(25.0),
            top: 58.0,
            bottom: (self.viewport.height - controls - 24.0).max(100.0),
        };
        // Drawer navigation itself preserves the camera. A newly loaded chain
        // must fit in the part of the desktop canvas still visible beside it.
        let drawer = element(&self.document, "edge-evidence-drawer");
        if drawer.has_attribute("open") {
            let right = drawer.get_bounding_client_rect().left()
                - self.canvas.get_bounding_client_rect().left()
                - 24.0;
            if right > area.left + 100.0 {
                area.right = area.right.min(right);
            }
        }
        if let Some(camera) = self.viewport.frame(points, area) {
            self.camera = camera;
        }
    }
    pub(super) fn rebuild_spatial(&mut self) {
        self.spatial.clear();
        for (i, p) in self.points.iter().enumerate() {
            self.spatial
                .entry(((p.0 / 80.0).floor() as i32, (p.1 / 80.0).floor() as i32))
                .or_default()
                .push(i);
        }
    }
}

pub(super) fn request_chain(view: View) {
    let (generation, from, to) = {
        let mut v = view.borrow_mut();
        v.chain_generation += 1;
        v.clear_highlight();
        let chain = element(&v.document, "graph-chain");
        if v.selected.is_empty() || v.graph.target.is_empty() {
            v.draw();
            return;
        }
        chain.set_text_content(Some("Loading shortest teammate chain…"));
        let _ = chain.set_attribute("data-state", "loading");
        v.draw();
        (
            v.chain_generation,
            v.selected.clone(),
            v.graph.target.clone(),
        )
    };
    spawn_local(async move {
        let answer = fetch_text(&format!(
            "/api/connection?from={}&to={}",
            encoded(&from),
            encoded(&to)
        ))
        .await
        .and_then(|text| {
            serde_json::from_str::<ExplorerConnection>(&text)
                .map_err(|e| JsValue::from_str(&e.to_string()))
        });
        let mut v = view.borrow_mut();
        if v.chain_generation != generation || v.selected != from || v.graph.target != to {
            return;
        }
        let chain = element(&v.document, "graph-chain");
        match answer {
            Ok(result) if result.result == "connected" => {
                // Accept only server paths over the currently loaded admitted graph.
                if result.path.iter().any(|id| !v.indices.contains_key(id))
                    || result.degree != Some(result.links.len())
                    || result.path.len() != result.links.len() + 1
                {
                    chain.set_inner_html("<p>Invalid chain response. <button type=\"button\" data-retry-chain>Retry chain</button></p>");
                    let _ = chain.set_attribute("data-state", "error");
                    v.draw();
                    return;
                }
                v.graph.path = result.path.clone();
                v.graph.degree = result.degree;
                for e in &mut v.graph.links {
                    e.on_path = result.path.windows(2).any(|p| {
                        (p[0] == e.from && p[1] == e.to) || (p[1] == e.from && p[0] == e.to)
                    });
                }
                let mut html = format!(
                    "<h3>Shortest teammate chain · degree {}</h3><ol>",
                    result.degree.unwrap()
                );
                for (i, id) in result.path.iter().enumerate() {
                    html.push_str(&format!(
                        "<li><button type=\"button\" data-center-player=\"{}\">{}</button>",
                        escape(id),
                        escape(&v.graph.nodes[v.indices[id]].name)
                    ));
                    if let Some(link) = result.links.get(i) {
                        html.push_str(&format!(
                            " <a data-evidence href=\"/edge?from={}&amp;to={}\">{} · {}</a>",
                            encoded(&link.from),
                            encoded(&link.to),
                            escape(&link.team),
                            escape(&api_types::overlap_description(
                                link.overlap_days,
                                link.minimum_shared_games
                            ))
                        ));
                    }
                    html.push_str("</li>");
                }
                html.push_str("</ol>");
                chain.set_inner_html(&html);
                let _ = chain.set_attribute("data-state", "connected");
                v.settle_layout();
                let points: Vec<_> = result.path.iter().map(|id| v.at(id)).collect();
                v.frame_points(&points);
            }
            Ok(result) => {
                chain.set_text_content(Some(if result.certainty.as_deref()==Some("unresolved_coverage") {"No teammate chain in this evidenced graph. Missing evidence may connect these players historically."} else {"No teammate chain in this complete synthetic fixture."}));
                let _ = chain.set_attribute("data-state", "disconnected");
            }
            Err(_) => {
                chain.set_inner_html("<p>Unable to load the chain. <button type=\"button\" data-retry-chain>Retry chain</button></p>");
                let _ = chain.set_attribute("data-state", "error");
            }
        }
        v.draw();
    });
}

pub(super) fn reduced_motion() -> bool {
    web_sys::window()
        .unwrap()
        .match_media("(prefers-reduced-motion: reduce)")
        .ok()
        .flatten()
        .is_some_and(|m| m.matches())
}
pub(super) fn change_layout(view: View, mode: LayoutMode) {
    let mut recompute_chain = false;
    {
        let mut v = view.borrow_mut();
        let old = v.points.clone();
        let camera = v.camera;
        if mode == LayoutMode::Concentric {
            if v.selected.is_empty() {
                let default = v
                    .graph
                    .nodes
                    .iter()
                    .enumerate()
                    .max_by(|(a, na), (b, nb)| {
                        v.adjacency[*a]
                            .len()
                            .cmp(&v.adjacency[*b].len())
                            .then_with(|| nb.id.cmp(&na.id))
                    })
                    .map(|(_, n)| (n.id.clone(), n.name.clone()));
                if let Some((id, name)) = default {
                    v.select(&id);
                    v.auto_focus = Some(name);
                    recompute_chain = !v.graph.target.is_empty();
                }
            }
            v.ego = !v.selected.is_empty();
        }
        v.relayout(mode);
        if v.pending_layout {
            v.draw();
            return;
        }
        if v.ego {
            v.fit_visible();
        } else if mode == LayoutMode::Concentric {
            v.focus_player();
            v.camera.scale = 0.55;
        }
        if !reduced_motion() {
            let target = v.points.clone();
            let camera_to = v.camera;
            v.points = old.clone();
            v.camera = camera;
            v.rebuild_spatial();
            v.transition = Some(Transition {
                start: js_sys::Date::now(),
                from: old,
                to: target,
                camera_from: camera,
                camera_to,
            });
        }
        v.draw();
    }
    if recompute_chain {
        request_chain(view.clone());
    }
    animate(view);
}
pub(super) fn animate(view: View) {
    {
        let mut v = view.borrow_mut();
        if v.transition.is_none() || v.animation_scheduled {
            return;
        }
        v.animation_scheduled = true;
    }
    let callback = Closure::once_into_js(move || {
        {
            let mut v = view.borrow_mut();
            v.animation_scheduled = false;
            let Some(t) = v.transition.take() else {
                return;
            };
            let p = ((js_sys::Date::now() - t.start) / 250.0).clamp(0.0, 1.0);
            let eased = p * p * (3.0 - 2.0 * p);
            v.points = t
                .from
                .iter()
                .zip(&t.to)
                .map(|(a, b)| (a.0 + (b.0 - a.0) * eased, a.1 + (b.1 - a.1) * eased))
                .collect();
            v.camera.scale =
                t.camera_from.scale + (t.camera_to.scale - t.camera_from.scale) * eased;
            v.camera.x = t.camera_from.x + (t.camera_to.x - t.camera_from.x) * eased;
            v.camera.y = t.camera_from.y + (t.camera_to.y - t.camera_from.y) * eased;
            if p < 1.0 {
                v.transition = Some(t);
            }
            v.rebuild_spatial();
            let _ = element(&v.document, "canvas-status").set_attribute(
                "data-transition",
                if v.transition.is_some() {
                    "running"
                } else {
                    "settled"
                },
            );
            v.draw();
        }
        animate(view);
    });
    let _ = web_sys::window()
        .unwrap()
        .request_animation_frame(callback.unchecked_ref());
}

#[derive(Default)]
struct PickerState {
    generation: u64,
    candidates: Vec<(String, String)>,
    active: Option<usize>,
}
fn install_picker(view: View, id: &'static str) -> Result<(), JsValue> {
    let document = view.borrow().document.clone();
    let native = element(&document, id).dyn_into::<HtmlSelectElement>()?;
    let chosen = native.value();
    let name = {
        let v = view.borrow();
        v.indices
            .get(&chosen)
            .map(|i| v.graph.nodes[*i].name.clone())
            .unwrap_or_default()
    };
    // Native GET controls remain the fallback; enhancement replaces them with an explicit combobox.
    let input = document
        .create_element("input")?
        .dyn_into::<HtmlInputElement>()?;
    input.set_id(id);
    input.set_value(&name);
    input.set_attribute("type", "text")?;
    input.set_attribute("autocomplete", "off")?;
    input.set_attribute("role", "combobox")?;
    input.set_attribute("aria-autocomplete", "list")?;
    input.set_attribute("aria-expanded", "false")?;
    input.set_attribute("data-selected-id", &chosen)?;
    let list_id = format!("{id}-results");
    input.set_attribute("aria-controls", &list_id)?;
    native.replace_with_with_node_1(&input)?;
    let parent = input.parent_element().unwrap();
    let list = document.create_element("div")?;
    list.set_id(&list_id);
    list.set_attribute("role", "listbox")?;
    list.set_attribute(
        "aria-label",
        if id == "graph-player" {
            "Player A suggestions"
        } else {
            "Player B suggestions"
        },
    )?;
    list.set_attribute("hidden", "")?;
    parent.append_child(&list)?;
    let notice = document.create_element("p")?;
    notice.set_id(&format!("{id}-search-status"));
    notice.set_attribute("role", "status")?;
    parent.append_child(&notice)?;
    let era = document
        .create_element("select")?
        .dyn_into::<HtmlSelectElement>()?;
    era.set_id(&format!("{id}-era"));
    era.set_attribute(
        "aria-label",
        if id == "graph-player" {
            "Player A debut decade"
        } else {
            "Player B debut decade"
        },
    )?;
    era.set_inner_html(&format!(
        "<option value=\"\">Any debut decade</option>{}",
        (0..8)
            .map(|i| format!(
                "<option value=\"{}\">{}s{}</option>",
                1940 + i * 10,
                1940 + i * 10,
                if i == 7 { "+" } else { "" }
            ))
            .collect::<String>()
    ));
    parent.append_child(&era)?;
    let franchise = document
        .create_element("select")?
        .dyn_into::<HtmlSelectElement>()?;
    franchise.set_id(&format!("{id}-franchise"));
    franchise.set_attribute(
        "aria-label",
        if id == "graph-player" {
            "Player A canonical franchise"
        } else {
            "Player B canonical franchise"
        },
    )?;
    let teams: std::collections::BTreeSet<_> = view
        .borrow()
        .graph
        .nodes
        .iter()
        .flat_map(|n| n.teams.clone())
        .collect();
    franchise.set_inner_html(&format!(
        "<option value=\"\">Any franchise</option>{}",
        teams
            .iter()
            .map(|t| format!("<option value=\"{}\">{}</option>", escape(t), escape(t)))
            .collect::<String>()
    ));
    parent.append_child(&franchise)?;
    let state = Rc::new(RefCell::new(PickerState::default()));
    for (target, event) in [
        (input.clone().into(), "input"),
        (era.clone().into(), "change"),
        (franchise.clone().into(), "change"),
    ] {
        let view = view.clone();
        let state = state.clone();
        let input = input.clone();
        let era = era.clone();
        let franchise = franchise.clone();
        let teams = teams.clone();
        listen(&target, event, move |_| {
            if event == "input" {
                let mut v = view.borrow_mut();
                v.chain_generation += 1;
                v.clear_highlight();
                let _ = input.set_attribute("data-selected-id", "");
                if id == "graph-player" {
                    v.selected.clear();
                    v.recompute_hop();
                    v.trunk_plan = visual::TrunkPlan::default();
                    v.sector_headers.clear();
                    v.auto_focus = None;
                    v.franchise = None;
                    v.franchise_nodes.clear();
                    v.ego = false;
                    v.render_franchises();
                    let summary = element(&v.document, "graph-selection");
                    let _ = summary.remove_attribute("data-player");
                    summary.set_text_content(Some("Choose a Player A search result explicitly."));
                } else {
                    v.graph.target.clear();
                }
                v.draw();
            }
            let generation = {
                let mut s = state.borrow_mut();
                s.generation += 1;
                s.candidates.clear();
                s.active = None;
                s.generation
            };
            let document = view.borrow().document.clone();
            let list = element(&document, &format!("{id}-results"));
            list.set_inner_html("");
            let _ = list.remove_attribute("hidden");
            let _ = input.set_attribute("aria-expanded", "true");
            let _ = input.remove_attribute("aria-activedescendant");
            element(&view.borrow().document, &format!("{id}-search-status"))
                .set_text_content(Some("Searching players…"));
            let mut q = input.value();
            let mut decade = era.value();
            let mut team = franchise.value();
            if decade.is_empty()
                && let Some(token) = q
                    .trim()
                    .strip_suffix('s')
                    .and_then(|s| s.parse::<u32>().ok())
                    .filter(|n| (1940..=2010).contains(n) && n % 10 == 0)
            {
                decade = token.to_string();
                q.clear();
            }
            if team.is_empty() && teams.contains(q.trim()) {
                team = q.trim().to_owned();
                q.clear();
            }
            let url = format!(
                "/api/search?q={}&limit=20{}{}",
                encoded(&q),
                if decade.is_empty() {
                    String::new()
                } else {
                    format!("&era={decade}")
                },
                if team.is_empty() {
                    String::new()
                } else {
                    format!("&franchise={}", encoded(&team))
                }
            );
            let state = state.clone();
            let input = input.clone();
            spawn_local(async move {
                let answer = fetch_text(&url).await.and_then(|s| {
                    serde_json::from_str::<serde_json::Value>(&s)
                        .map_err(|e| JsValue::from_str(&e.to_string()))
                });
                let mut s = state.borrow_mut();
                if s.generation != generation {
                    return;
                }
                let notice = element(&document, &format!("{id}-search-status"));
                match answer {
                    Ok(data) => {
                        let mut html = String::new();
                        for (i, c) in data["candidates"]
                            .as_array()
                            .unwrap_or(&Vec::new())
                            .iter()
                            .enumerate()
                        {
                            let p = &c["player"];
                            let pid = p["id"].as_str().unwrap_or("");
                            let name = p["name"].as_str().unwrap_or("");
                            s.candidates.push((pid.into(), name.into()));
                            let seasons = if p["first_season"].is_null() {
                                "Unknown debut".into()
                            } else {
                                format!("{}–{} seasons", p["first_season"], p["last_season"])
                            };
                            let affiliations = p["teams"]
                                .as_array()
                                .map(|ts| {
                                    ts.iter()
                                        .filter_map(|t| t.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                })
                                .unwrap_or_default();
                            html.push_str(&format!("<button type=\"button\" role=\"option\" aria-selected=\"false\" tabindex=\"-1\" id=\"{id}-option-{i}\" data-candidate=\"{i}\" data-player-id=\"{}\">{} — {}; {} ({})</button>",escape(pid),escape(name),escape(&seasons),escape(&affiliations),escape(pid)));
                        }
                        list.set_inner_html(&html);
                        notice.set_text_content(Some(&format!(
                            "{} match(es). {}",
                            data["matches_total"],
                            if data["has_more"] == true {
                                "Showing 20; narrow your search."
                            } else if s.candidates.is_empty() {
                                "No matching player; change your query."
                            } else {
                                "Choose a result explicitly."
                            }
                        )));
                    }
                    Err(_) => notice
                        .set_text_content(Some("Search unavailable. Change the query to retry.")),
                }
                let _ = input.set_attribute("aria-expanded", "true");
            });
        })?;
    }
    let choose = {
        let view = view.clone();
        let state = state.clone();
        let input = input.clone();
        move |index: usize| {
            let Some((pid, name)) = state.borrow().candidates.get(index).cloned() else {
                return;
            };
            {
                let mut s = state.borrow_mut();
                s.generation += 1;
                s.active = None;
            }
            input.set_value(&name);
            let _ = input.set_attribute("data-selected-id", &pid);
            let _ = input.set_attribute("aria-expanded", "false");
            let _ = input.remove_attribute("aria-activedescendant");
            element(&view.borrow().document, &format!("{id}-search-status"))
                .set_text_content(Some(&format!("Selected: {name}.")));
            {
                let mut v = view.borrow_mut();
                let _ = element(&v.document, &format!("{id}-results")).set_attribute("hidden", "");
                if id == "graph-player" {
                    v.select(&pid);
                } else {
                    v.graph.target = pid;
                }
                v.draw();
            }
            request_chain(view.clone());
            animate(view.clone());
        }
    };
    let choose = Rc::new(choose);
    let click_choose = choose.clone();
    listen(list.as_ref(), "click", move |e| {
        if let Some(index) = e
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            .and_then(|t| t.closest("[data-candidate]").ok().flatten())
            .and_then(|t| t.get_attribute("data-candidate"))
            .and_then(|s| s.parse().ok())
        {
            click_choose(index);
        }
    })?;
    let document = document.clone();
    let state = state.clone();
    let input_copy = input.clone();
    listen(input.as_ref(), "keydown", move |e| {
        let e = e.dyn_into::<KeyboardEvent>().unwrap();
        match e.key().as_str() {
            "ArrowDown" | "ArrowUp" => {
                e.prevent_default();
                let mut s = state.borrow_mut();
                let n = s.candidates.len();
                if n == 0 {
                    return;
                }
                let _ = element(&document, &format!("{id}-results")).remove_attribute("hidden");
                let _ = input_copy.set_attribute("aria-expanded", "true");
                let i = match (s.active, e.key().as_str()) {
                    (Some(i), "ArrowDown") => (i + 1) % n,
                    (Some(i), _) => (i + n - 1) % n,
                    (None, "ArrowDown") => 0,
                    _ => n - 1,
                };
                s.active = Some(i);
                for j in 0..n {
                    let _ = element(&document, &format!("{id}-option-{j}"))
                        .set_attribute("aria-selected", if i == j { "true" } else { "false" });
                }
                let _ =
                    input_copy.set_attribute("aria-activedescendant", &format!("{id}-option-{i}"));
            }
            "Enter" => {
                e.prevent_default();
                let index = if input_copy.get_attribute("aria-expanded").as_deref() == Some("true")
                {
                    state.borrow().active
                } else {
                    None
                };
                if let Some(i) = index {
                    choose(i);
                }
            }
            "Escape" => {
                e.prevent_default();
                state.borrow_mut().generation += 1;
                state.borrow_mut().active = None;
                let _ = element(&document, &format!("{id}-results")).set_attribute("hidden", "");
                let _ = input_copy.set_attribute("aria-expanded", "false");
                let _ = input_copy.remove_attribute("aria-activedescendant");
            }
            _ => {}
        }
    })?;
    Ok(())
}

pub(super) fn dispatch_evidence(document: &Document, url: &str) {
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&JsValue::from_str(url));
    if let Ok(event) = web_sys::CustomEvent::new_with_event_init_dict("explorer-evidence", &init) {
        let _ = document.dispatch_event(&event);
    }
}
struct Drawer {
    generation: u64,
    url: Option<String>,
    return_focus: Option<HtmlElement>,
}
fn load_drawer(document: Document, state: Rc<RefCell<Drawer>>, url: String) {
    let generation = {
        let mut s = state.borrow_mut();
        s.generation += 1;
        s.url = Some(url.clone());
        s.generation
    };
    let dialog = element(&document, "edge-evidence-drawer")
        .dyn_into::<HtmlDialogElement>()
        .unwrap();
    if !dialog.open() {
        state.borrow_mut().return_focus = document
            .active_element()
            .and_then(|e| e.dyn_into::<HtmlElement>().ok());
        let _ = dialog.show_modal();
    }
    let _ = element(&document, "close-evidence")
        .dyn_into::<HtmlElement>()
        .unwrap()
        .focus();
    element(&document, "drawer-status").set_text_content(Some("Loading relationship evidence…"));
    element(&document, "drawer-content").set_text_content(Some("Loading relationship evidence…"));
    spawn_local(async move {
        let answer = fetch_text(&format!("{url}&view=drawer")).await;
        let s = state.borrow();
        if s.generation != generation || s.url.as_deref() != Some(&url) || !dialog.open() {
            return;
        }
        let content = element(&document, "drawer-content");
        match answer {
            Ok(html) => {
                content.set_inner_html(&html);
                element(&document, "drawer-status")
                    .set_text_content(Some("Relationship evidence loaded."));
            }
            Err(_) => {
                element(&document, "drawer-status").set_text_content(Some(
                    "Evidence loading failed. Retry or open standalone evidence.",
                ));
                content.set_inner_html(&format!("<p>Unable to load evidence. <button type=\"button\" id=\"retry-evidence\">Retry evidence</button> <a href=\"{}\">Open standalone evidence</a></p>", escape(&url)));
            }
        }
    });
}
fn close_drawer(document: &Document, state: &Rc<RefCell<Drawer>>) {
    let mut s = state.borrow_mut();
    if s.url.is_none()
        && !element(document, "edge-evidence-drawer")
            .dyn_into::<HtmlDialogElement>()
            .unwrap()
            .open()
    {
        return;
    }
    s.generation += 1;
    s.url = None;
    element(document, "edge-evidence-drawer")
        .dyn_into::<HtmlDialogElement>()
        .unwrap()
        .close();
    let target = s
        .return_focus
        .take()
        .filter(|e| e.is_connected())
        .unwrap_or_else(|| element(document, "graph-canvas").dyn_into().unwrap());
    let _ = target.focus();
}
fn install_drawer(view: View) -> Result<(), JsValue> {
    let document = view.borrow().document.clone();
    let state = Rc::new(RefCell::new(Drawer {
        generation: 0,
        url: None,
        return_focus: None,
    }));
    let doc = document.clone();
    let drawer = state.clone();
    listen(document.as_ref(), "explorer-evidence", move |event| {
        let Some(url) = event
            .dyn_into::<web_sys::CustomEvent>()
            .ok()
            .and_then(|e| e.detail().as_string())
        else {
            return;
        };
        let window = web_sys::window().unwrap();
        let history = window.history().unwrap();
        if drawer.borrow().url.is_some() {
            let _ = history.replace_state_with_url(&JsValue::from_str(&url), "", Some(&url));
        } else {
            drawer.borrow_mut().return_focus = doc
                .active_element()
                .and_then(|e| e.dyn_into::<HtmlElement>().ok());
            let _ = history.replace_state_with_url(&JsValue::from_str("explorer"), "", None);
            let _ = history.push_state_with_url(&JsValue::from_str(&url), "", Some(&url));
        }
        load_drawer(doc.clone(), drawer.clone(), url);
    })?;
    let doc = document.clone();
    let drawer = state.clone();
    listen(document.as_ref(), "click", move |event| {
        let Ok(mouse) = event.clone().dyn_into::<MouseEvent>() else {
            return;
        };
        if mouse.button() != 0
            || mouse.ctrl_key()
            || mouse.meta_key()
            || mouse.shift_key()
            || mouse.alt_key()
        {
            return;
        }
        let Some(target) = event
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
        else {
            return;
        };
        if let Some(anchor) = target.closest("a[data-evidence]").ok().flatten()
            && let Some(url) = anchor.get_attribute("href")
        {
            event.prevent_default();
            dispatch_evidence(&doc, &url);
        }
        if target.id() == "retry-evidence" {
            let url = drawer.borrow().url.clone();
            if let Some(url) = url {
                load_drawer(doc.clone(), drawer.clone(), url);
            }
        }
    })?;
    for (id, event) in [
        ("close-evidence", "click"),
        ("edge-evidence-drawer", "cancel"),
    ] {
        let doc = document.clone();
        let drawer = state.clone();
        listen(element(&document, id).as_ref(), event, move |e| {
            e.prevent_default();
            close_drawer(&doc, &drawer);
            let _ = web_sys::window().unwrap().history().unwrap().back();
        })?;
    }
    let doc = document.clone();
    let drawer = state;
    listen(web_sys::window().unwrap().as_ref(), "popstate", move |_| {
        let window = web_sys::window().unwrap();
        let url = window.location().pathname().unwrap_or_default()
            + &window.location().search().unwrap_or_default();
        if url.starts_with("/edge?") {
            load_drawer(doc.clone(), drawer.clone(), url);
        } else {
            close_drawer(&doc, &drawer);
        }
    })?;
    Ok(())
}

pub(super) fn install(view: View) -> Result<(), JsValue> {
    let document = view.borrow().document.clone();
    install_picker(view.clone(), "graph-player")?;
    install_picker(view.clone(), "graph-target")?;
    let state = view.clone();
    listen(
        element(&document, "pair-form").as_ref(),
        "submit",
        move |event| {
            event.prevent_default();
            request_chain(state.clone());
        },
    )?;
    let state = view.clone();
    listen(
        element(&document, "graph-chain").as_ref(),
        "click",
        move |event| {
            let Some(target) = event
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
            else {
                return;
            };
            if let Some(id) = target.get_attribute("data-center-player") {
                let mut v = state.borrow_mut();
                if v.indices.contains_key(&id) {
                    v.settle_layout();
                    let p = v.at(&id);
                    v.camera.scale = 1.0;
                    v.camera.x = 500.0 - p.0;
                    v.camera.y = 265.0 - p.1 * v.viewport.aspect_y();
                    v.draw();
                }
            } else if target.has_attribute("data-retry-chain") {
                request_chain(state.clone());
                animate(state.clone());
            }
        },
    )?;
    for id in ["random-pair", "clear-chain", "reset-filters", "ego-toggle"] {
        let state = view.clone();
        listen(element(&document, id).as_ref(), "click", move |_| {
            let mut v = state.borrow_mut();
            match id {
                "random-pair" => {
                    let n = v.graph.nodes.len();
                    if n < 2 {
                        return;
                    }
                    let a = (js_sys::Math::random() * n as f64) as usize;
                    let mut b = (js_sys::Math::random() * (n - 1) as f64) as usize;
                    if b >= a {
                        b += 1;
                    }
                    let from = v.graph.nodes[a].id.clone();
                    let to = v.graph.nodes[b].clone();
                    v.select(&from);
                    v.graph.target = to.id.clone();
                    set_picker(&v.document, "graph-target", &to.id, &to.name);
                }
                "clear-chain" => {
                    v.graph.target.clear();
                    v.chain_generation += 1;
                    v.clear_highlight();
                    set_picker(&v.document, "graph-target", "", "");
                }
                "reset-filters" => {
                    v.eras = [true; 8];
                    v.franchise = None;
                    v.franchise_nodes.clear();
                    v.ego = false;
                    v.render_franchises();
                }
                "ego-toggle" => {
                    if v.selected.is_empty() {
                        return;
                    }
                    v.settle_layout();
                    v.ego = !v.ego;
                    if v.ego {
                        v.fit_visible();
                    } else {
                        v.fit_network();
                    }
                }
                _ => {}
            }
            v.draw();
            drop(v);
            if id == "random-pair" {
                request_chain(state.clone());
                animate(state.clone());
            }
        })?;
    }
    for i in 0..8 {
        let state = view.clone();
        listen(
            element(&document, &format!("era-{i}")).as_ref(),
            "click",
            move |_| {
                let mut v = state.borrow_mut();
                v.eras[i] = !v.eras[i];
                v.draw();
            },
        )?;
    }
    let state = view.clone();
    listen(
        element(&document, "franchise-filters").as_ref(),
        "click",
        move |e| {
            let Some(team) = e
                .target()
                .and_then(|t| t.dyn_into::<web_sys::Element>().ok())
                .and_then(|t| t.get_attribute("data-franchise"))
            else {
                return;
            };
            let mut v = state.borrow_mut();
            v.franchise = if v.franchise.as_ref() == Some(&team) {
                None
            } else {
                Some(team.clone())
            };
            v.franchise_nodes = if v.franchise.is_some() {
                v.graph
                    .links
                    .iter()
                    .filter(|e| {
                        e.franchises.contains(&team) && (e.from == v.selected || e.to == v.selected)
                    })
                    .flat_map(|e| [v.indices[&e.from], v.indices[&e.to]])
                    .collect()
            } else {
                Default::default()
            };
            v.render_franchises();
            v.draw();
        },
    )?;
    let mobile = web_sys::window()
        .unwrap()
        .match_media("(max-width: 640px)")?
        .is_some_and(|m| m.matches());
    let minimap = element(&document, "graph-minimap");
    let toggle = element(&document, "minimap-toggle");
    minimap.toggle_attribute_with_force("hidden", mobile)?;
    toggle.set_attribute("aria-expanded", if mobile { "false" } else { "true" })?;
    let doc = document.clone();
    listen(toggle.as_ref(), "click", move |_| {
        let map = element(&doc, "graph-minimap");
        let show = map.has_attribute("hidden");
        let _ = map.toggle_attribute_with_force("hidden", !show);
        let _ = element(&doc, "minimap-toggle")
            .set_attribute("aria-expanded", if show { "true" } else { "false" });
    })?;
    install_drawer(view)?;
    Ok(())
}
