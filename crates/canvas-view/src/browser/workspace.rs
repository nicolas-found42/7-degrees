//! Selection, view scope and inspector behavior for the map workspace.
use super::*;
use web_sys::HtmlElement;

impl CanvasView {
    pub(super) fn animate_camera_from(&mut self, from: Camera) {
        if !explorer::reduced_motion() {
            self.camera_move = Some(CameraMove {
                start: js_sys::Date::now(),
                from,
                to: self.camera,
            });
            self.camera = from;
        }
    }
    pub(super) fn locate_selection(&mut self) {
        self.settle_layout();
        let from = self.camera;
        if self.ego {
            self.fit_visible();
        } else {
            self.focus_player();
        }
        self.animate_camera_from(from);
    }
    pub(super) fn set_scope(&mut self, selected_only: bool) {
        if selected_only == self.ego
            || (selected_only && self.selected.is_empty() && self.edge.is_none())
        {
            return;
        }
        self.settle_layout();
        let from = self.camera;
        if selected_only {
            self.overview_camera = Some(from);
            self.ego = true;
            self.fit_visible();
        } else {
            self.ego = false;
            if let Some(previous) = self.overview_camera.take() {
                self.camera = previous;
            } else {
                self.fit_network();
            }
        }
        self.animate_camera_from(from);
    }
    pub(super) fn reset_view(&mut self) {
        self.settle_layout();
        let from = self.camera;
        self.physics.reset(&mut self.points);
        self.rebuild_spatial();
        self.overview_camera = None;
        self.ego = false;
        self.fit_network();
        self.animate_camera_from(from);
    }
    pub(super) fn render_player(&self) {
        let Some(&i) = self.indices.get(&self.selected) else {
            return;
        };
        let n = &self.graph.nodes[i];
        let escape = explorer::escape;
        let teams = n
            .teams
            .iter()
            .map(|t| format!("<span>{}</span>", escape(t)))
            .collect::<String>();
        let html = format!(
            "<h2>{}</h2><p>{}</p><div class=\"team-chips\">{teams}</div><p>{} evidenced teammates</p>",
            escape(&n.name),
            escape(&n.era),
            self.adjacency[i].len()
        );
        self.document
            .get_element_by_id("player-summary")
            .unwrap()
            .set_inner_html(&html);
        for id in ["player-actions", "teammates-details"] {
            let _ = self
                .document
                .get_element_by_id(id)
                .unwrap()
                .remove_attribute("hidden");
        }
        let mut neighbors = self.adjacency[i].clone();
        neighbors.sort_by(|a, b| self.graph.nodes[*a].name.cmp(&self.graph.nodes[*b].name));
        let list = neighbors
            .iter()
            .map(|&j| {
                let other = &self.graph.nodes[j];
                format!(
                    "<a data-evidence href=\"/edge?from={}&amp;to={}\">{}</a>",
                    explorer::encoded(&n.id),
                    explorer::encoded(&other.id),
                    escape(&other.name)
                )
            })
            .collect::<String>();
        self.document
            .get_element_by_id("teammates-list")
            .unwrap()
            .set_inner_html(&if list.is_empty() {
                "<p>No teammate evidence is available for this player.</p>".into()
            } else {
                list
            });
    }
}

pub(super) fn install(view: Rc<RefCell<CanvasView>>) -> Result<(), JsValue> {
    let document = view.borrow().document.clone();
    document
        .document_element()
        .unwrap()
        .class_list()
        .add_1("enhanced")?;
    let doc = document.clone();
    listen(document.as_ref(), "click", move |event| {
        if event
            .target()
            .and_then(|e| e.dyn_into::<web_sys::Element>().ok())
            .is_some_and(|e| e.closest("#advanced-controls").ok().flatten().is_none())
        {
            let _ = doc
                .get_element_by_id("advanced-controls")
                .unwrap()
                .remove_attribute("open");
        }
    })?;
    for id in ["scope-full", "find-connection", "dismiss-help"] {
        let state = view.clone();
        let doc = document.clone();
        listen(
            document.get_element_by_id(id).unwrap().as_ref(),
            "click",
            move |_| {
                match id {
                    "scope-full" => {
                        let mut v = state.borrow_mut();
                        v.set_scope(false);
                        v.draw();
                    }
                    "find-connection" => {
                        let _ = doc
                            .get_element_by_id("connection-controls")
                            .unwrap()
                            .set_attribute("open", "");
                        let _ = doc
                            .get_element_by_id("graph-target")
                            .unwrap()
                            .dyn_into::<HtmlElement>()
                            .unwrap()
                            .focus();
                    }
                    _ => {
                        let _ = doc
                            .get_element_by_id("graph-help")
                            .unwrap()
                            .set_attribute("hidden", "");
                    }
                }
                explorer::animate(state.clone());
            },
        )?;
    }
    Ok(())
}
