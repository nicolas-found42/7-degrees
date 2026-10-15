//! Deterministic rendering hints. These never change graph membership.
use crate::{GraphLink, GraphNode};
use std::collections::{BTreeMap, HashMap};

/// Maximum spanning forest union each player's three heaviest incident edges.
pub fn mark_backbone(nodes: &[GraphNode], links: &mut [GraphLink]) {
    let ids: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let mut order: Vec<_> = (0..links.len()).collect();
    order.sort_by(|&a, &b| {
        links[b]
            .weight
            .total_cmp(&links[a].weight)
            .then_with(|| canonical_pair(&links[a]).cmp(&canonical_pair(&links[b])))
    });
    let mut parents: Vec<_> = (0..nodes.len()).collect();
    let mut sizes = vec![1; nodes.len()];
    let mut incident = vec![0; nodes.len()];
    fn root(parents: &mut [usize], mut i: usize) -> usize {
        while parents[i] != i {
            parents[i] = parents[parents[i]];
            i = parents[i];
        }
        i
    }
    for i in order {
        let edge = &mut links[i];
        edge.in_visual_backbone = false;
        let (Some(&a), Some(&b)) = (ids.get(edge.from.as_str()), ids.get(edge.to.as_str())) else {
            continue;
        };
        if a == b {
            continue;
        }
        let (mut ra, mut rb) = (root(&mut parents, a), root(&mut parents, b));
        if ra != rb {
            edge.in_visual_backbone = true;
            if sizes[ra] < sizes[rb] {
                std::mem::swap(&mut ra, &mut rb);
            }
            parents[rb] = ra;
            sizes[ra] += sizes[rb];
        }
        edge.in_visual_backbone |= incident[a] < 3 || incident[b] < 3;
        incident[a] += 1;
        incident[b] += 1;
    }
}
fn canonical_pair(edge: &GraphLink) -> (&str, &str) {
    if edge.from < edge.to {
        (&edge.from, &edge.to)
    } else {
        (&edge.to, &edge.from)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomTier {
    Macro,
    Meso,
    Micro,
}
impl ZoomTier {
    pub fn at(scale: f64) -> Self {
        if scale < 0.25 {
            Self::Macro
        } else if scale < 0.75 {
            Self::Meso
        } else {
            Self::Micro
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Macro => "macro",
            Self::Meso => "meso",
            Self::Micro => "micro",
        }
    }
    pub fn label_cap(self, width: f64) -> usize {
        match (self, width <= 640.0) {
            (Self::Macro, _) => 10,
            (Self::Meso, true) => 12,
            (Self::Meso, false) => 24,
            (Self::Micro, true) => 30,
            (Self::Micro, false) => 50,
        }
    }
    pub fn radius(self, count: usize, landmark: bool) -> f64 {
        let (base, slope) = match self {
            Self::Macro => (1.1, 0.13),
            Self::Meso => (1.8, 0.4),
            Self::Micro => (2.3, 0.5),
        };
        (base + (1.0 + count as f64).log2() * slope + if landmark { 1.4 } else { 0.0 })
            .clamp(1.1, 9.0)
    }
}
/// Smooth scale-dependent blend, settled immediately under reduced motion.
pub fn zoom_blend(scale: f64, boundary: f64, reduced: bool) -> f64 {
    if reduced {
        return if scale >= boundary { 1.0 } else { 0.0 };
    }
    let t = ((scale - boundary) / 0.08).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
/// Newly admitted ordinary labels cross-fade; important badges bypass this.
pub fn label_zoom_alpha(scale: f64, rank: usize, landmark: bool, width: f64, reduced: bool) -> f64 {
    let meso = if landmark {
        1.0
    } else {
        zoom_blend(scale, 0.25, reduced)
    };
    meso * if rank >= ZoomTier::Meso.label_cap(width) {
        zoom_blend(scale, 0.75, reduced)
    } else {
        1.0
    }
}

/// Known debut decades each contribute their top 3%, at least one landmark.
pub fn landmarks(nodes: &[GraphNode]) -> Vec<bool> {
    let mut decades: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (i, node) in nodes.iter().enumerate() {
        if let Some(y) = node.debut_season {
            decades.entry(y / 10).or_default().push(i);
        }
    }
    let mut result = vec![false; nodes.len()];
    for indices in decades.values_mut() {
        indices.sort_by(|&a, &b| {
            nodes[b]
                .degree
                .cmp(&nodes[a].degree)
                .then_with(|| nodes[a].id.cmp(&nodes[b].id))
        });
        for &i in indices.iter().take((indices.len() * 3).div_ceil(100)) {
            result[i] = true;
        }
    }
    result
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}
impl Rect {
    pub fn padded(self) -> Self {
        Self {
            left: self.left - 14.0,
            right: self.right + 14.0,
            top: self.top - 8.0,
            bottom: self.bottom + 8.0,
        }
    }
    pub fn overlaps(self, b: Self) -> bool {
        self.left < b.right && self.right > b.left && self.top < b.bottom && self.bottom > b.top
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LabelPriority {
    Important,
    DirectTeammate,
    Context,
}

#[derive(Clone, Debug)]
pub struct LabelCandidate<'a> {
    pub index: usize,
    pub id: &'a str,
    pub priority: LabelPriority,
    pub weight: f64,
    pub count: usize,
    pub matching: bool,
    pub point: (f64, f64),
    pub width: f64,
}
#[derive(Clone, Debug)]
pub struct PlacedLabel {
    pub index: usize,
    pub priority: LabelPriority,
    pub bounds: Rect,
    pub x: f64,
    pub y: f64,
}
#[derive(Default)]
struct Occupancy {
    occupied: Vec<Rect>,
    cells: HashMap<(i32, i32), Vec<Rect>>,
}
impl Occupancy {
    fn cells(b: Rect) -> impl Iterator<Item = (i32, i32)> {
        let (x0, x1, y0, y1) = (
            (b.left / 64.0).floor() as i32,
            (b.right / 64.0).floor() as i32,
            (b.top / 64.0).floor() as i32,
            (b.bottom / 64.0).floor() as i32,
        );
        (x0..=x1).flat_map(move |x| (y0..=y1).map(move |y| (x, y)))
    }
    fn insert(&mut self, b: Rect) {
        self.occupied.push(b);
        for c in Self::cells(b) {
            self.cells.entry(c).or_default().push(b);
        }
    }
    fn overlaps(&self, b: Rect) -> bool {
        Self::cells(b).any(|c| {
            self.cells
                .get(&c)
                .is_some_and(|v| v.iter().any(|a| a.overlaps(b)))
        })
    }
}
/// Important labels are cap-exempt; other labels require a padded free screen box.
pub fn place_labels(
    candidates: &mut [LabelCandidate<'_>],
    width: f64,
    height: f64,
    cap: usize,
    reserved: &[Rect],
) -> Vec<PlacedLabel> {
    candidates.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| {
                if a.priority == LabelPriority::DirectTeammate {
                    b.weight.total_cmp(&a.weight)
                } else {
                    b.matching.cmp(&a.matching)
                }
            })
            .then_with(|| b.count.cmp(&a.count))
            .then_with(|| a.id.cmp(b.id))
    });
    let mut occupancy = Occupancy::default();
    for &b in reserved {
        occupancy.insert(b.padded());
    }
    let mut result = Vec::new();
    let mut ordinary = 0;
    for c in candidates {
        if c.priority != LabelPriority::Important && ordinary >= cap {
            continue;
        }
        let w = c.width.min((width - 8.0).max(1.0));
        let clamp_x = |x: f64| x.clamp(w / 2.0 + 4.0, (width - w / 2.0 - 4.0).max(w / 2.0 + 4.0));
        let clamp_y = |y: f64| y.clamp(26.0, (height - 10.0).max(26.0));
        let (mut x, mut y) = (clamp_x(c.point.0), clamp_y(c.point.1 - 14.0));
        let bounds = |x, y| Rect {
            left: x - w / 2.0,
            right: x + w / 2.0,
            top: y - 20.0,
            bottom: y + 6.0,
        };
        if c.priority == LabelPriority::Important {
            let mut xs = vec![x, clamp_x(w / 2.0 + 4.0), clamp_x(width - w / 2.0 - 4.0)];
            let mut ys = vec![y, 26.0, (height - 10.0).max(26.0)];
            for r in &occupancy.occupied {
                xs.extend([
                    clamp_x(r.left - w / 2.0 - 14.0),
                    clamp_x(r.right + w / 2.0 + 14.0),
                ]);
                ys.extend([clamp_y(r.top - 14.0), clamp_y(r.bottom + 28.0)]);
            }
            xs.sort_by(|a, b| {
                (a - x)
                    .abs()
                    .total_cmp(&(b - x).abs())
                    .then_with(|| a.total_cmp(b))
            });
            xs.dedup();
            ys.sort_by(|a, b| {
                (a - y)
                    .abs()
                    .total_cmp(&(b - y).abs())
                    .then_with(|| a.total_cmp(b))
            });
            ys.dedup();
            'placement: for &candidate_x in &xs {
                for &candidate_y in &ys {
                    if !occupancy.overlaps(bounds(candidate_x, candidate_y).padded()) {
                        x = candidate_x;
                        y = candidate_y;
                        break 'placement;
                    }
                }
            }
        } else if occupancy.overlaps(bounds(x, y).padded()) {
            continue;
        }
        let b = bounds(x, y);
        occupancy.insert(b.padded());
        result.push(PlacedLabel {
            index: c.index,
            priority: c.priority,
            bounds: b,
            x,
            y,
        });
        if c.priority != LabelPriority::Important {
            ordinary += 1;
        }
    }
    result
}

pub type Point = (f64, f64);
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum Curve {
    Line(Point, Point),
    Quadratic(Point, Point, Point),
    Cubic(Point, Point, Point, Point),
}
impl Curve {
    pub fn point(self, t: f64) -> Point {
        let u = 1.0 - t;
        match self {
            Self::Line(a, b) => (u * a.0 + t * b.0, u * a.1 + t * b.1),
            Self::Quadratic(a, c, b) => (
                u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0,
                u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1,
            ),
            Self::Cubic(a, c, d, b) => (
                u.powi(3) * a.0 + 3.0 * u * u * t * c.0 + 3.0 * u * t * t * d.0 + t.powi(3) * b.0,
                u.powi(3) * a.1 + 3.0 * u * u * t * c.1 + 3.0 * u * t * t * d.1 + t.powi(3) * b.1,
            ),
        }
    }
    pub fn map(self, f: impl Fn(Point) -> Point) -> Self {
        match self {
            Self::Line(a, b) => Self::Line(f(a), f(b)),
            Self::Quadratic(a, c, b) => Self::Quadratic(f(a), f(c), f(b)),
            Self::Cubic(a, c, d, b) => Self::Cubic(f(a), f(c), f(d), f(b)),
        }
    }
    pub fn distance(self, p: Point) -> f64 {
        fn midpoint(a: Point, b: Point) -> Point {
            ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0)
        }
        fn line_distance(p: Point, a: Point, b: Point) -> f64 {
            let v = (b.0 - a.0, b.1 - a.1);
            let squared = v.0 * v.0 + v.1 * v.1;
            let t = if squared == 0.0 {
                0.0
            } else {
                ((p.0 - a.0) * v.0 + (p.1 - a.1) * v.1) / squared
            }
            .clamp(0.0, 1.0);
            (p.0 - a.0 - t * v.0).hypot(p.1 - a.1 - t * v.1)
        }
        fn visit(curve: Curve, p: Point, depth: usize, best: &mut f64) {
            match curve {
                Curve::Line(a, b) => *best = best.min(line_distance(p, a, b)),
                Curve::Quadratic(a, c, b) => {
                    if depth == 16 || line_distance(c, a, b) < 0.2 {
                        *best = best.min(line_distance(p, a, b));
                        return;
                    }
                    let (ac, cb) = (midpoint(a, c), midpoint(c, b));
                    let m = midpoint(ac, cb);
                    visit(Curve::Quadratic(a, ac, m), p, depth + 1, best);
                    visit(Curve::Quadratic(m, cb, b), p, depth + 1, best);
                }
                Curve::Cubic(a, c, d, b) => {
                    if depth == 16 || line_distance(c, a, b).max(line_distance(d, a, b)) < 0.2 {
                        *best = best.min(line_distance(p, a, b));
                        return;
                    }
                    let (ac, cd, db) = (midpoint(a, c), midpoint(c, d), midpoint(d, b));
                    let (left, right) = (midpoint(ac, cd), midpoint(cd, db));
                    let m = midpoint(left, right);
                    visit(Curve::Cubic(a, ac, left, m), p, depth + 1, best);
                    visit(Curve::Cubic(m, right, db, b), p, depth + 1, best);
                }
            }
        }
        let mut distance = f64::INFINITY;
        visit(self, p, 0, &mut distance);
        distance
    }
}
pub fn quadratic(a: Point, b: Point) -> Curve {
    Curve::Quadratic(
        a,
        (
            (a.0 + b.0) / 2.0 - (b.1 - a.1) * 0.075,
            (a.1 + b.1) / 2.0 + (b.0 - a.0) * 0.075,
        ),
        b,
    )
}
/// Canonical endpoint IDs fix the bend of an undirected edge.
pub fn unbundled_curve(edge: &GraphLink, a: Point, b: Point) -> Curve {
    if edge.from <= edge.to {
        quadratic(a, b)
    } else {
        quadratic(b, a)
    }
}

/// A focus-dependent grouping; computed once per selection, not per frame.
#[derive(Default)]
pub struct TrunkPlan {
    groups: Vec<Vec<(usize, usize)>>,
    focus: Option<usize>,
}
impl TrunkPlan {
    pub fn new(nodes: &[GraphNode], links: &[GraphLink], focus: &str) -> Self {
        let ids: HashMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();
        let Some(&f) = ids.get(focus) else {
            return Self::default();
        };
        let mut groups: BTreeMap<&str, Vec<(usize, usize)>> = BTreeMap::new();
        for (i, e) in links.iter().enumerate() {
            let other = if e.from == focus {
                &e.to
            } else if e.to == focus {
                &e.from
            } else {
                continue;
            };
            let Some(&o) = ids.get(other.as_str()) else {
                continue;
            };
            if o == f || e.primary_franchise().is_empty() {
                continue;
            }
            groups
                .entry(e.primary_franchise())
                .or_default()
                .push((i, o));
        }
        Self {
            groups: groups.into_values().collect(),
            focus: Some(f),
        }
    }
    pub fn curves(
        &self,
        links: &[GraphLink],
        points: &[Point],
        visible: impl Fn(usize) -> bool,
    ) -> HashMap<usize, Curve> {
        let mut result = HashMap::new();
        let Some(f) = self.focus else {
            return result;
        };
        if !visible(f) {
            return result;
        }
        for members in &self.groups {
            let group: Vec<_> = members
                .iter()
                .copied()
                .filter(|&(i, o)| !links[i].on_path && visible(o))
                .collect();
            if group.len() < 3 {
                continue;
            }
            let (mut x, mut y, mut total) = (0.0, 0.0, 0.0);
            for &(i, o) in &group {
                let w = links[i].weight.clamp(0.1, 1.0);
                x += points[o].0 * w;
                y += points[o].1 * w;
                total += w;
            }
            let a = points[f];
            let trunk = (
                a.0 + (x / total - a.0) * 0.35,
                a.1 + (y / total - a.1) * 0.35,
            );
            for &(i, o) in &group {
                let b = points[o];
                result.insert(
                    i,
                    Curve::Cubic(a, trunk, ((trunk.0 + b.0) / 2.0, (trunk.1 + b.1) / 2.0), b),
                );
            }
        }
        result
    }
}
/// Shared franchise controls for native callers using the complete payload seam.
pub fn franchise_trunks(
    nodes: &[GraphNode],
    links: &[GraphLink],
    points: &[Point],
    focus: &str,
    visible: &[bool],
    concentric: bool,
) -> HashMap<usize, Curve> {
    if concentric {
        HashMap::new()
    } else {
        TrunkPlan::new(nodes, links, focus).curves(links, points, |i| visible[i])
    }
}
