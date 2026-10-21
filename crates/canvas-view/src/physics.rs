//! Local, anchored springs for direct manipulation. The layout is the rest pose;
//! only the dragged player and its immediate neighbors participate. No pins survive release.
use crate::visual::Point;

pub struct DragPhysics {
    home: Vec<Point>,
    velocity: Vec<Point>,
    active: Vec<usize>,
    held: Option<usize>,
}
impl DragPhysics {
    pub fn new(home: &[Point]) -> Self {
        Self {
            home: home.to_vec(),
            velocity: vec![(0.0, 0.0); home.len()],
            active: Vec::new(),
            held: None,
        }
    }
    pub fn begin(&mut self, index: usize, adjacency: &[Vec<usize>]) {
        self.held = Some(index);
        for i in std::iter::once(index).chain(adjacency[index].iter().copied()) {
            if !self.active.contains(&i) {
                self.active.push(i);
            }
        }
    }
    pub fn drag_to(&mut self, point: Point, points: &mut [Point]) {
        if let Some(i) = self.held {
            points[i] = point;
            self.velocity[i] = (0.0, 0.0);
        }
    }
    pub fn release(&mut self) {
        self.held = None;
    }
    pub fn reset(&mut self, points: &mut [Point]) {
        points.copy_from_slice(&self.home);
        self.velocity.fill((0.0, 0.0));
        self.active.clear();
        self.held = None;
    }
    pub fn is_active(&self) -> bool {
        !self.active.is_empty()
    }
    pub fn is_held(&self) -> bool {
        self.held.is_some()
    }
    /// Advance a damped spring with bounded substeps, independent of frame rate.
    pub fn step(&mut self, seconds: f64, points: &mut [Point]) {
        let seconds = seconds.clamp(0.0, 0.05);
        let steps = (seconds / (1.0 / 120.0)).ceil().max(1.0) as usize;
        let dt = seconds / steps as f64;
        for _ in 0..steps {
            let offset = self
                .held
                .map(|i| {
                    (
                        (points[i].0 - self.home[i].0) * 0.14,
                        (points[i].1 - self.home[i].1) * 0.14,
                    )
                })
                .unwrap_or((0.0, 0.0));
            for &i in &self.active {
                if self.held == Some(i) {
                    continue;
                }
                let target = (self.home[i].0 + offset.0, self.home[i].1 + offset.1);
                let v = &mut self.velocity[i];
                v.0 += ((target.0 - points[i].0) * 110.0 - v.0 * 21.0) * dt;
                v.1 += ((target.1 - points[i].1) * 110.0 - v.1 * 21.0) * dt;
                points[i].0 += v.0 * dt;
                points[i].1 += v.1 * dt;
            }
        }
        if self.held.is_none()
            && self.active.iter().all(|&i| {
                (points[i].0 - self.home[i].0).hypot(points[i].1 - self.home[i].1) < 0.02
                    && self.velocity[i].0.hypot(self.velocity[i].1) < 0.1
            })
        {
            self.reset(points);
        }
    }
}
