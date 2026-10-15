//! Bounded logarithmic attraction with global Barnes–Hut repulsion.
//! No inferred communities or rendering backbone are used by this solver.
use crate::{GraphLink, GraphNode, visual::Point};
use std::collections::HashMap;

struct Cell {
    left: f64,
    top: f64,
    size: f64,
    mass: f64,
    center: Point,
    children: Vec<Cell>,
    members: Vec<usize>,
}
impl Cell {
    fn build(
        points: &[Point],
        charges: &[f64],
        members: Vec<usize>,
        left: f64,
        top: f64,
        size: f64,
        depth: usize,
    ) -> Self {
        let mass = members.iter().map(|&i| charges[i]).sum::<f64>();
        let center = (
            members
                .iter()
                .map(|&i| points[i].0 * charges[i])
                .sum::<f64>()
                / mass,
            members
                .iter()
                .map(|&i| points[i].1 * charges[i])
                .sum::<f64>()
                / mass,
        );
        let mut cell = Self {
            left,
            top,
            size,
            mass,
            center,
            children: Vec::new(),
            members,
        };
        if cell.members.len() > 4 && depth < 16 {
            let half = size / 2.0;
            let mut quadrants: [Vec<usize>; 4] = std::array::from_fn(|_| Vec::new());
            for i in cell.members.drain(..) {
                let p = points[i];
                let q = usize::from(p.0 >= left + half) + 2 * usize::from(p.1 >= top + half);
                quadrants[q].push(i);
            }
            for (q, members) in quadrants.into_iter().enumerate() {
                if !members.is_empty() {
                    cell.children.push(Self::build(
                        points,
                        charges,
                        members,
                        left + (q % 2) as f64 * half,
                        top + (q / 2) as f64 * half,
                        half,
                        depth + 1,
                    ));
                }
            }
        }
        cell
    }
    fn repel(&self, i: usize, points: &[Point], charges: &[f64]) -> Point {
        let p = points[i];
        let (dx, dy) = (p.0 - self.center.0, p.1 - self.center.1);
        let squared = dx * dx + dy * dy;
        let contains = p.0 >= self.left
            && p.0 <= self.left + self.size
            && p.1 >= self.top
            && p.1 <= self.top + self.size;
        if !contains && self.size * self.size < squared * 0.7 * 0.7 {
            return repulsion(p, self.center, charges[i] * self.mass);
        }
        let mut force = (0.0, 0.0);
        for child in &self.children {
            let f = child.repel(i, points, charges);
            force.0 += f.0;
            force.1 += f.1;
        }
        for &other in &self.members {
            if other != i {
                let f = repulsion(p, points[other], charges[i] * charges[other]);
                force.0 += f.0;
                force.1 += f.1;
            }
        }
        force
    }
}
fn repulsion(a: Point, b: Point, mass: f64) -> Point {
    let (dx, dy) = (a.0 - b.0, a.1 - b.1);
    let squared = dx * dx + dy * dy;
    let strength = 1200.0 * mass / (squared + 25.0).sqrt() / (squared + 25.0);
    (dx * strength, dy * strength)
}
pub(super) fn compute(
    nodes: &[GraphNode],
    links: &[GraphLink],
    mut points: Vec<Point>,
) -> Vec<Point> {
    if points.is_empty() {
        return points;
    }
    let ids: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let mut edges: Vec<_> = links
        .iter()
        .filter_map(|e| {
            let (a, b) = (*ids.get(e.from.as_str())?, *ids.get(e.to.as_str())?);
            (a != b).then_some((a, b, e.weight.clamp(0.1, 1.0)))
        })
        .collect();
    edges.sort_by_key(|&(a, b, _)| {
        (
            nodes[a].id.as_str().min(nodes[b].id.as_str()),
            nodes[a].id.as_str().max(nodes[b].id.as_str()),
        )
    });
    let mut counts = vec![0; nodes.len()];
    for &(a, b, _) in &edges {
        counts[a] += 1;
        counts[b] += 1;
    }
    let charges: Vec<_> = counts.iter().map(|&d| ((d + 1) as f64).sqrt()).collect();
    let center = (
        points.iter().map(|p| p.0).sum::<f64>() / points.len() as f64,
        points.iter().map(|p| p.1).sum::<f64>() / points.len() as f64,
    );
    for p in &mut points {
        p.0 = (p.0 - center.0) * 0.15;
        p.1 = (p.1 - center.1) * 0.15;
    }
    for step in 0..360 {
        let (mut left, mut top, mut right, mut bottom) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for &p in &points {
            left = left.min(p.0);
            top = top.min(p.1);
            right = right.max(p.0);
            bottom = bottom.max(p.1);
        }
        let tree = Cell::build(
            &points,
            &charges,
            (0..nodes.len()).collect(),
            left - 1.0,
            top - 1.0,
            (right - left).max(bottom - top).max(1.0) + 2.0,
            0,
        );
        let mut deltas: Vec<_> = (0..nodes.len())
            .map(|i| tree.repel(i, &points, &charges))
            .collect();
        for &(a, b, weight) in &edges {
            let (dx, dy) = (points[b].0 - points[a].0, points[b].1 - points[a].1);
            let distance = dx.hypot(dy).max(1.0);
            let force = 80.0 * (distance / 40.0).ln_1p() * weight.powi(3);
            let (fx, fy) = (dx / distance * force, dy / distance * force);
            deltas[a].0 += fx / (counts[a] + 1) as f64;
            deltas[a].1 += fy / (counts[a] + 1) as f64;
            deltas[b].0 -= fx / (counts[b] + 1) as f64;
            deltas[b].1 -= fy / (counts[b] + 1) as f64;
        }
        let cooling = 1.0 - 0.8 * step as f64 / 360.0;
        for (i, d) in deltas.into_iter().enumerate() {
            points[i].0 += (d.0 - points[i].0 * 0.003).clamp(-20.0, 20.0) * cooling;
            points[i].1 += (d.1 - points[i].1 * 0.003).clamp(-20.0, 20.0) * cooling;
        }
    }
    points
}
